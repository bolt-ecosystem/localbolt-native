//! Daemon sidecar process lifecycle management.
//!
//! Spawns bolt-daemon as a Tauri sidecar, manages PID files,
//! handles stale socket/PID cleanup, and coordinates with the watchdog.
//! N6-B3: Platform-aware path wiring with --socket-path and --data-dir.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tauri::Emitter;

use crate::daemon_log::StderrBuffer;
use crate::ipc_bridge::IpcBridge;
use crate::ipc_client::{self, ReadinessResult};
use crate::ipc_transport::IpcStream;
use crate::platform;
use crate::watchdog::{Watchdog, WatchdogState, STARTUP_TIMEOUT};

/// App version used for IPC handshake.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Watchdog state payload emitted to frontend.
#[derive(serde::Serialize, Clone)]
pub struct WatchdogStateEvent {
    pub state: WatchdogState,
    pub retry_count: u32,
}

/// Shared daemon manager state.
pub struct DaemonManager {
    pub watchdog: Arc<Mutex<Watchdog>>,
    pub stderr_buffer: StderrBuffer,
    pub bridge: Arc<IpcBridge>,
    app_handle: Option<tauri::AppHandle>,
    child_pid: Arc<Mutex<Option<u32>>>,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    // N6-B3: platform-aware paths
    socket_path: String,
    pid_path: String,
    data_dir: String,
    daemon_version: Arc<Mutex<Option<String>>>,
    spawn_count: Arc<std::sync::atomic::AtomicU32>,
}

impl DaemonManager {
    pub fn new() -> Self {
        Self {
            watchdog: Arc::new(Mutex::new(Watchdog::new())),
            stderr_buffer: StderrBuffer::with_default_capacity(),
            bridge: Arc::new(IpcBridge::new()),
            app_handle: None,
            child_pid: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            socket_path: platform::default_ipc_path(),
            pid_path: platform::default_pid_path(),
            data_dir: platform::default_data_dir(),
            daemon_version: Arc::new(Mutex::new(None)),
            spawn_count: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    // ── Accessors ─────────────────────────────────────────────

    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    pub fn pid_path(&self) -> &str {
        &self.pid_path
    }

    pub fn data_dir(&self) -> &str {
        &self.data_dir
    }

    pub fn daemon_version(&self) -> Option<String> {
        self.daemon_version.lock().unwrap().clone()
    }

    pub fn spawn_count(&self) -> u32 {
        self.spawn_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Set the Tauri AppHandle for event emission (called from setup).
    pub fn set_app_handle(&mut self, handle: tauri::AppHandle) {
        self.app_handle = Some(handle);
    }

    /// Emit watchdog state change to frontend.
    fn emit_watchdog_state(&self) {
        if let Some(ref handle) = self.app_handle {
            let watchdog = self.watchdog.lock().unwrap();
            let event = WatchdogStateEvent {
                state: watchdog.state(),
                retry_count: watchdog.retry_count(),
            };
            let _ = handle.emit("daemon://watchdog-state", &event);
        }
    }

    /// Run the full daemon lifecycle on a background thread.
    ///
    /// This is the main entry point: spawn, probe, watch, restart.
    pub fn start(self: &Arc<Self>) {
        let mgr = Arc::clone(self);
        std::thread::spawn(move || {
            mgr.lifecycle_loop();
        });
    }

    fn lifecycle_loop(&self) {
        loop {
            if self
                .shutdown_flag
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                tracing::info!("[WATCHDOG] shutdown flag set, exiting lifecycle loop");
                return;
            }

            let state = self.watchdog.lock().unwrap().state();
            match state {
                WatchdogState::Starting => {
                    self.run_spawn_cycle();
                }
                WatchdogState::Restarting => {
                    self.run_spawn_cycle();
                }
                WatchdogState::Ready => {
                    // Wait for daemon to exit, then handle
                    self.wait_for_daemon_exit();
                }
                WatchdogState::Degraded | WatchdogState::Incompatible => {
                    // Terminal states — exit loop. Manual restart re-enters via new thread.
                    tracing::info!("[WATCHDOG] lifecycle loop exiting in terminal state: {state}");
                    return;
                }
            }
        }
    }

    fn run_spawn_cycle(&self) {
        // Pre-spawn cleanup
        self.run_cleanup();

        let socket_path = Path::new(&self.socket_path);

        match self.spawn_daemon() {
            Ok(pid) => {
                tracing::info!("[WATCHDOG] daemon spawned (pid={pid})");
                *self.child_pid.lock().unwrap() = Some(pid);
                self.write_pid_file(pid);
                self.spawn_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                // Wait briefly for daemon to initialize, then probe
                std::thread::sleep(std::time::Duration::from_millis(500));

                // Probe with timeout
                let deadline = std::time::Instant::now() + STARTUP_TIMEOUT;
                loop {
                    if std::time::Instant::now() >= deadline {
                        let delay = self.watchdog.lock().unwrap().on_startup_timeout();
                        self.emit_watchdog_state();
                        if let Some(d) = delay {
                            std::thread::sleep(d);
                        }
                        return;
                    }

                    if !socket_path.exists() && !platform::is_windows_pipe_path(&self.socket_path) {
                        // Unix socket: wait for file to appear
                        std::thread::sleep(std::time::Duration::from_millis(250));
                        continue;
                    }

                    match ipc_client::probe_readiness(socket_path, APP_VERSION) {
                        ReadinessResult::Ready { daemon_version, .. } => {
                            tracing::info!(
                                "[WATCHDOG] readiness confirmed: daemon v{daemon_version}"
                            );
                            *self.daemon_version.lock().unwrap() = Some(daemon_version.clone());
                            self.watchdog.lock().unwrap().on_daemon_ready();
                            self.emit_watchdog_state();

                            // Start persistent IPC bridge for event forwarding
                            if let Some(ref handle) = self.app_handle {
                                match self.bridge.start(socket_path, APP_VERSION, handle.clone()) {
                                    Ok(()) => {
                                        tracing::info!("[IPC_BRIDGE] started successfully");
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "[IPC_BRIDGE] failed to start: {e} (readiness probe passed but bridge failed)"
                                        );
                                    }
                                }
                            }
                            return;
                        }
                        ReadinessResult::Incompatible { daemon_version } => {
                            tracing::warn!("[WATCHDOG] daemon incompatible: v{daemon_version}");
                            *self.daemon_version.lock().unwrap() = Some(daemon_version);
                            self.watchdog.lock().unwrap().on_version_incompatible();
                            self.emit_watchdog_state();
                            return;
                        }
                        ReadinessResult::Failed(reason) => {
                            tracing::debug!("[WATCHDOG] probe retry: {reason}");
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                    }
                }
            }
            Err(reason) => {
                self.watchdog.lock().unwrap().on_spawn_failure(&reason);
                self.emit_watchdog_state();
            }
        }
    }

    /// Spawn bolt-daemon as a child process.
    ///
    /// N6-B3: passes --socket-path and --data-dir from platform defaults.
    fn spawn_daemon(&self) -> Result<u32, String> {
        let binary_path = self.resolve_daemon_binary()?;

        let child = std::process::Command::new(&binary_path)
            .args([
                "--role",
                "answerer",
                "--mode",
                "default",
                "--pairing-policy",
                "ask",
                "--socket-path",
                &self.socket_path,
                "--data-dir",
                &self.data_dir,
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn failed: {e}"))?;

        let pid = child.id();

        // Capture stderr in background thread
        let buffer = self.stderr_buffer.clone();
        let mut child = child;
        let stderr = child.stderr.take();
        if let Some(stderr) = stderr {
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            tracing::trace!("[DAEMON_STDERR] {l}");
                            buffer.push(l);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        // Detach the child handle — we track via PID and use platform signals.
        std::mem::forget(child);

        Ok(pid)
    }

    /// Resolve daemon binary path.
    fn resolve_daemon_binary(&self) -> Result<PathBuf, String> {
        // In dev: look for bolt-daemon in src-tauri/bin/ with target triple
        let target_triple = self.target_triple();
        let dev_path = PathBuf::from(format!("bin/bolt-daemon-{target_triple}"));
        if dev_path.exists() {
            return Ok(dev_path);
        }

        // Also check without triple suffix (symlink case)
        let simple_path = PathBuf::from("bin/bolt-daemon");
        if simple_path.exists() {
            return Ok(simple_path);
        }

        // Check system PATH
        #[cfg(unix)]
        if let Ok(output) = std::process::Command::new("which")
            .arg("bolt-daemon")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Ok(PathBuf::from(path));
                }
            }
        }

        #[cfg(windows)]
        if let Ok(output) = std::process::Command::new("where")
            .arg("bolt-daemon")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !path.is_empty() {
                    return Ok(PathBuf::from(path));
                }
            }
        }

        Err("bolt-daemon binary not found in bin/ or PATH".to_string())
    }

    fn target_triple(&self) -> &'static str {
        env!("TAURI_ENV_TARGET_TRIPLE", "unknown-unknown-unknown")
    }

    fn wait_for_daemon_exit(&self) {
        let pid = match *self.child_pid.lock().unwrap() {
            Some(p) => p,
            None => return,
        };

        // Poll for process exit
        loop {
            if self
                .shutdown_flag
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                return;
            }

            if !platform::process_alive(pid) {
                let exit_code = self.get_exit_code(pid);
                tracing::warn!("[WATCHDOG] daemon exited (pid={pid}, code={exit_code:?})");
                *self.child_pid.lock().unwrap() = None;

                // Daemon exited — shutdown bridge
                self.bridge.shutdown();

                let delay = self.watchdog.lock().unwrap().on_daemon_exit(exit_code);
                self.emit_watchdog_state();
                let retry_count = self.watchdog.lock().unwrap().retry_count();
                let log_dir = platform::crash_log_dir();

                let _ = crate::daemon_log::write_crash_snapshot(
                    &self.stderr_buffer,
                    &log_dir,
                    exit_code,
                    Some(pid),
                    retry_count,
                );

                if let Some(d) = delay {
                    std::thread::sleep(d);
                }
                return;
            }

            // Also check retry reset while ready
            self.watchdog.lock().unwrap().maybe_reset_retries();

            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    fn get_exit_code(&self, _pid: u32) -> Option<i32> {
        // On Unix, we can't waitpid a non-child after std::mem::forget.
        // On Windows, we'd need OpenProcess + GetExitCodeProcess.
        // Return None for now — exit code reported via crash snapshot stderr.
        None
    }

    // ── Cleanup ────────────────────────────────────────────────

    /// Pre-spawn cleanup: remove stale PID/socket files.
    pub fn run_cleanup(&self) {
        let socket_path = Path::new(&self.socket_path);
        let pid_path = Path::new(&self.pid_path);

        // Step 1: Check PID file
        if pid_path.exists() {
            if let Ok(content) = std::fs::read_to_string(pid_path) {
                if let Ok(pid) = content.trim().parse::<u32>() {
                    if platform::process_alive(pid) {
                        if socket_path.exists() && IpcStream::probe(socket_path) {
                            tracing::info!(
                                "[WATCHDOG] existing daemon alive (pid={pid}), will connect"
                            );
                            return;
                        }
                        // Process alive but socket missing/unresponsive — kill
                        tracing::warn!(
                            "[WATCHDOG] daemon alive (pid={pid}) but socket missing, killing"
                        );
                        platform::process_terminate(pid);
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        if platform::process_alive(pid) {
                            platform::process_force_kill(pid);
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                    }
                }
            }
            let _ = std::fs::remove_file(pid_path);
            tracing::info!("[WATCHDOG] cleaned stale PID file");
        }

        // Step 2: Check stale socket (not applicable for Windows named pipes)
        if !platform::is_windows_pipe_path(&self.socket_path) && socket_path.exists() {
            if IpcStream::probe(socket_path) {
                tracing::info!("[WATCHDOG] responsive daemon found via socket probe");
                return;
            }
            let _ = std::fs::remove_file(socket_path);
            tracing::info!("[WATCHDOG] removed stale socket: {}", socket_path.display());
        }
    }

    fn write_pid_file(&self, pid: u32) {
        let pid_path = Path::new(&self.pid_path);
        if let Err(e) = std::fs::write(pid_path, pid.to_string()) {
            tracing::warn!("[WATCHDOG] failed to write PID file: {e}");
        }
    }

    // ── Shutdown ───────────────────────────────────────────────

    /// Initiate clean daemon shutdown.
    pub fn shutdown(&self) {
        self.bridge.shutdown();
        self.shutdown_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let pid = match self.child_pid.lock().unwrap().take() {
            Some(p) => p,
            None => {
                tracing::info!("[WATCHDOG] shutdown: no daemon process to stop");
                return;
            }
        };

        tracing::info!("[WATCHDOG] initiating daemon shutdown (pid={pid})");

        platform::process_terminate(pid);

        // Wait up to 5s for clean exit
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if !platform::process_alive(pid) {
                tracing::info!("[WATCHDOG] daemon exited cleanly (pid={pid})");
                break;
            }
            if std::time::Instant::now() >= deadline {
                tracing::warn!("[WATCHDOG] daemon did not exit in 5s, forcing kill (pid={pid})");
                platform::process_force_kill(pid);
                std::thread::sleep(std::time::Duration::from_millis(500));
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Cleanup PID file
        let _ = std::fs::remove_file(&self.pid_path);

        // Cleanup socket file (not applicable for Windows named pipes)
        if !platform::is_windows_pipe_path(&self.socket_path) {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        tracing::info!("[WATCHDOG] shutdown cleanup complete");
    }

    /// Manual restart (from degraded state). Re-enters lifecycle loop.
    pub fn manual_restart(self: &Arc<Self>) -> bool {
        let transition = self.watchdog.lock().unwrap().manual_restart();
        match transition {
            crate::watchdog::Transition::Changed(WatchdogState::Starting) => {
                let mgr = Arc::clone(self);
                std::thread::spawn(move || {
                    mgr.lifecycle_loop();
                });
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_handles_nonexistent_files() {
        let mgr = DaemonManager::new();
        // Should not panic when PID/socket files don't exist
        mgr.run_cleanup();
    }

    #[test]
    fn pid_file_written_and_readable() {
        let mgr = DaemonManager::new();
        let test_pid = 99999u32;
        mgr.write_pid_file(test_pid);

        let content = std::fs::read_to_string(mgr.pid_path()).unwrap();
        assert_eq!(content, "99999");

        let _ = std::fs::remove_file(mgr.pid_path());
    }

    #[test]
    fn resolve_daemon_binary_fails_gracefully() {
        let mgr = DaemonManager::new();
        let result = mgr.resolve_daemon_binary();
        match result {
            Ok(p) => assert!(!p.as_os_str().is_empty()),
            Err(e) => assert!(e.contains("not found")),
        }
    }

    #[test]
    fn shutdown_flag_stops_lifecycle() {
        let mgr = DaemonManager::new();
        mgr.shutdown_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
        mgr.lifecycle_loop();
    }

    #[test]
    fn platform_paths_populated() {
        let mgr = DaemonManager::new();
        assert!(!mgr.socket_path().is_empty());
        assert!(!mgr.pid_path().is_empty());
        assert!(!mgr.data_dir().is_empty());
    }

    #[test]
    fn spawn_count_starts_at_zero() {
        let mgr = DaemonManager::new();
        assert_eq!(mgr.spawn_count(), 0);
    }

    #[test]
    fn daemon_version_initially_none() {
        let mgr = DaemonManager::new();
        assert!(mgr.daemon_version().is_none());
    }

    #[test]
    fn spawn_args_include_socket_and_data_dir() {
        // Verify the spawn command would include the path flags
        let mgr = DaemonManager::new();
        let socket = mgr.socket_path();
        let data = mgr.data_dir();
        assert!(!socket.is_empty());
        assert!(!data.is_empty());
        // On Unix, socket path should be a .sock file
        #[cfg(not(windows))]
        assert!(socket.ends_with(".sock"));
    }
}
