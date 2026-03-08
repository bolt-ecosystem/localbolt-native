//! Platform-aware path defaults for daemon IPC and data directories.
//!
//! Centralizes all platform-specific path logic for N6-B3 daemon wiring.
//! Supports Unix domain sockets (macOS/Linux) and Windows named pipes.

use std::path::PathBuf;

/// Default IPC socket/pipe path.
pub fn default_ipc_path() -> String {
    #[cfg(windows)]
    {
        r"\\.\pipe\bolt-daemon".to_string()
    }
    #[cfg(not(windows))]
    {
        "/tmp/bolt-daemon.sock".to_string()
    }
}

/// Default PID file path.
pub fn default_pid_path() -> String {
    #[cfg(windows)]
    {
        std::env::temp_dir()
            .join("bolt-daemon.pid")
            .to_string_lossy()
            .to_string()
    }
    #[cfg(not(windows))]
    {
        "/tmp/bolt-daemon.pid".to_string()
    }
}

/// Default data directory for daemon identity and trust data.
pub fn default_data_dir() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library/Application Support/LocalBolt/daemon")
                .to_string_lossy()
                .to_string();
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(appdata)
                .join(r"LocalBolt\daemon")
                .to_string_lossy()
                .to_string();
        }
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg)
                .join("localbolt/daemon")
                .to_string_lossy()
                .to_string();
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".local/share/localbolt/daemon")
                .to_string_lossy()
                .to_string();
        }
    }
    "/tmp/localbolt-daemon-data".to_string()
}

/// Crash log directory for daemon crash snapshots.
pub fn crash_log_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Library/Logs/LocalBolt");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(appdata).join(r"LocalBolt\logs");
        }
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
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

/// Support bundle output directory.
pub fn support_bundle_dir() -> PathBuf {
    std::env::temp_dir().join("localbolt-support")
}

/// Check if a path is a Windows named pipe format.
pub fn is_windows_pipe_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.starts_with(r"\\.\pipe\")
}

// ── Process management helpers ────────────────────────────

/// Check if a process is alive.
pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        tracing::debug!("[PLATFORM] process_alive: no-op on this platform");
        false
    }
}

/// Send graceful termination signal (SIGTERM on Unix).
pub fn process_terminate(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        tracing::debug!("[PLATFORM] process_terminate: no-op on this platform");
    }
}

/// Send forced termination signal (SIGKILL on Unix).
pub fn process_force_kill(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        tracing::debug!("[PLATFORM] process_force_kill: no-op on this platform");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ipc_path_nonempty() {
        assert!(!default_ipc_path().is_empty());
    }

    #[test]
    fn default_pid_path_nonempty() {
        assert!(!default_pid_path().is_empty());
    }

    #[test]
    fn default_data_dir_nonempty() {
        assert!(!default_data_dir().is_empty());
    }

    #[test]
    fn crash_log_dir_nonempty() {
        assert!(!crash_log_dir().as_os_str().is_empty());
    }

    #[test]
    fn support_bundle_dir_nonempty() {
        assert!(!support_bundle_dir().as_os_str().is_empty());
    }

    #[test]
    fn windows_pipe_detection() {
        assert!(is_windows_pipe_path(r"\\.\pipe\bolt-daemon"));
        assert!(is_windows_pipe_path(r"\\.\PIPE\bolt-daemon"));
        assert!(is_windows_pipe_path(r"\\.\pipe\the9ines\bolt"));
        assert!(!is_windows_pipe_path("/tmp/bolt-daemon.sock"));
        assert!(!is_windows_pipe_path(""));
        assert!(!is_windows_pipe_path("bolt-daemon"));
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_default_is_socket() {
        assert!(default_ipc_path().ends_with(".sock"));
    }

    #[test]
    fn process_alive_nonexistent_pid() {
        // PID 4294967 is very unlikely to exist
        assert!(!process_alive(4_294_967));
    }
}
