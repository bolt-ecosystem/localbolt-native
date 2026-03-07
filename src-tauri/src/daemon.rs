//! Daemon sidecar process lifecycle management.
//!
//! Spawns bolt-daemon as a Tauri sidecar, manages PID files,
//! handles stale socket/PID cleanup, and coordinates with the watchdog.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::daemon_log::StderrBuffer;
use crate::ipc_client::{self, ReadinessResult, DAEMON_SOCKET_PATH};
use crate::watchdog::{Watchdog, WatchdogState, STARTUP_TIMEOUT};

/// PID file location (matches N1 spec for macOS/Linux dev).
const PID_FILE_PATH: &str = "/tmp/bolt-daemon.pid";

/// App version used for IPC handshake.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Shared daemon manager state.
pub struct DaemonManager {
    pub watchdog: Arc<Mutex<Watchdog>>,
    pub stderr_buffer: StderrBuffer,
    child_pid: Arc<Mutex<Option<u32>>>,
    shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
}

impl DaemonManager {
    pub fn new() -> Self {
        Self {
            watchdog: Arc::new(Mutex::new(Watchdog::new())),
            stderr_buffer: StderrBuffer::with_default_capacity(),
            child_pid: Arc::new(Mutex::new(None)),
            shutdown_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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

        // Attempt spawn
        let socket_path = Path::new(DAEMON_SOCKET_PATH);

        match self.spawn_daemon() {
            Ok(pid) => {
                tracing::info!("[WATCHDOG] daemon spawned (pid={pid})");
                *self.child_pid.lock().unwrap() = Some(pid);
                self.write_pid_file(pid);

                // Wait briefly for daemon to initialize, then probe
                std::thread::sleep(std::time::Duration::from_millis(500));

                // Probe with timeout
                let deadline = std::time::Instant::now() + STARTUP_TIMEOUT;
                loop {
                    if std::time::Instant::now() >= deadline {
                        let delay = self.watchdog.lock().unwrap().on_startup_timeout();
                        if let Some(d) = delay {
                            std::thread::sleep(d);
                        }
                        return;
                    }

                    if !socket_path.exists() {
                        std::thread::sleep(std::time::Duration::from_millis(250));
                        continue;
                    }

                    match ipc_client::probe_readiness(socket_path, APP_VERSION) {
                        ReadinessResult::Ready { daemon_version, .. } => {
                            tracing::info!(
                                "[WATCHDOG] readiness confirmed: daemon v{daemon_version}"
                            );
                            self.watchdog.lock().unwrap().on_daemon_ready();
                            return;
                        }
                        ReadinessResult::Incompatible { daemon_version } => {
                            tracing::warn!("[WATCHDOG] daemon incompatible: v{daemon_version}");
                            self.watchdog.lock().unwrap().on_version_incompatible();
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
            }
        }
    }

    /// Spawn bolt-daemon as a child process.
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

        // Detach the child handle — we track via PID and use libc::kill for signals.
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

            // Check if process is still alive
            let alive = unsafe { libc::kill(pid as i32, 0) == 0 };
            if !alive {
                let exit_code = self.get_exit_code(pid);
                tracing::warn!("[WATCHDOG] daemon exited (pid={pid}, code={exit_code:?})");
                *self.child_pid.lock().unwrap() = None;

                let delay = self.watchdog.lock().unwrap().on_daemon_exit(exit_code);
                if let Some(d) = delay {
                    // Write crash snapshot
                    let retry_count = self.watchdog.lock().unwrap().retry_count();
                    let log_dir = self.crash_log_dir();
                    let _ = crate::daemon_log::write_crash_snapshot(
                        &self.stderr_buffer,
                        &log_dir,
                        exit_code,
                        Some(pid),
                        retry_count,
                    );
                    std::thread::sleep(d);
                } else {
                    // Entering degraded — write final crash snapshot
                    let retry_count = self.watchdog.lock().unwrap().retry_count();
                    let log_dir = self.crash_log_dir();
                    let _ = crate::daemon_log::write_crash_snapshot(
                        &self.stderr_buffer,
                        &log_dir,
                        exit_code,
                        Some(pid),
                        retry_count,
                    );
                }
                return;
            }

            // Also check retry reset while ready
            self.watchdog.lock().unwrap().maybe_reset_retries();

            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    fn get_exit_code(&self, _pid: u32) -> Option<i32> {
        // On Unix, waitpid would give us exit code, but we forked via
        // std::process::Command and forgot the handle. We can't waitpid
        // a non-child. Return None for now.
        None
    }

    fn crash_log_dir(&self) -> PathBuf {
        // macOS: ~/Library/Logs/LocalBolt/
        // Linux: $XDG_STATE_HOME/localbolt/ or ~/.local/state/localbolt/
        #[cfg(target_os = "macos")]
        {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home).join("Library/Logs/LocalBolt");
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if let Ok(state) = std::env::var("XDG_STATE_HOME") {
                return PathBuf::from(state).join("localbolt");
            }
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home).join(".local/state/localbolt");
            }
        }
        PathBuf::from("/tmp/localbolt-logs")
    }

    // ── Cleanup ────────────────────────────────────────────────

    /// Pre-spawn cleanup: remove stale PID/socket files.
    pub fn run_cleanup(&self) {
        let socket_path = Path::new(DAEMON_SOCKET_PATH);
        let pid_path = Path::new(PID_FILE_PATH);

        // Step 1: Check PID file
        if pid_path.exists() {
            if let Ok(content) = std::fs::read_to_string(pid_path) {
                if let Ok(pid) = content.trim().parse::<i32>() {
                    let alive = unsafe { libc::kill(pid, 0) == 0 };
                    if alive {
                        if socket_path.exists() && ipc_client::socket_probe(socket_path) {
                            tracing::info!(
                                "[WATCHDOG] existing daemon alive (pid={pid}), will connect"
                            );
                            return;
                        }
                        // Process alive but socket missing/unresponsive — kill
                        tracing::warn!(
                            "[WATCHDOG] daemon alive (pid={pid}) but socket missing, killing"
                        );
                        unsafe {
                            libc::kill(pid, libc::SIGTERM);
                        }
                        // Brief wait then SIGKILL
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        let still_alive = unsafe { libc::kill(pid, 0) == 0 };
                        if still_alive {
                            unsafe {
                                libc::kill(pid, libc::SIGKILL);
                            }
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                    }
                }
            }
            let _ = std::fs::remove_file(pid_path);
            tracing::info!("[WATCHDOG] cleaned stale PID file");
        }

        // Step 2: Check stale socket
        if socket_path.exists() {
            if ipc_client::socket_probe(socket_path) {
                tracing::info!("[WATCHDOG] responsive daemon found via socket probe");
                return;
            }
            let _ = std::fs::remove_file(socket_path);
            tracing::info!("[WATCHDOG] removed stale socket: {}", socket_path.display());
        }
    }

    fn write_pid_file(&self, pid: u32) {
        let pid_path = Path::new(PID_FILE_PATH);
        if let Err(e) = std::fs::write(pid_path, pid.to_string()) {
            tracing::warn!("[WATCHDOG] failed to write PID file: {e}");
        }
    }

    // ── Shutdown ───────────────────────────────────────────────

    /// Initiate clean daemon shutdown (SIGTERM -> 5s grace -> SIGKILL).
    pub fn shutdown(&self) {
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

        // SIGTERM
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }

        // Wait up to 5s for clean exit
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let alive = unsafe { libc::kill(pid as i32, 0) == 0 };
            if !alive {
                tracing::info!("[WATCHDOG] daemon exited cleanly (pid={pid})");
                break;
            }
            if std::time::Instant::now() >= deadline {
                tracing::warn!("[WATCHDOG] daemon did not exit in 5s, sending SIGKILL (pid={pid})");
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Cleanup PID and socket files
        let _ = std::fs::remove_file(PID_FILE_PATH);
        let _ = std::fs::remove_file(DAEMON_SOCKET_PATH);
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

        let content = std::fs::read_to_string(PID_FILE_PATH).unwrap();
        assert_eq!(content, "99999");

        let _ = std::fs::remove_file(PID_FILE_PATH);
    }

    #[test]
    fn resolve_daemon_binary_fails_gracefully() {
        let mgr = DaemonManager::new();
        // In test environment, binary likely not in bin/ — should handle gracefully
        let result = mgr.resolve_daemon_binary();
        // May succeed or fail depending on environment; just ensure no panic
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
        // lifecycle_loop should exit immediately
        mgr.lifecycle_loop();
    }
}
