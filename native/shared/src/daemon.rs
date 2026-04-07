//! Daemon lifecycle FFI — spawn, monitor, and stop bolt-daemon from native shells.

use std::ffi::{CStr, CString};
use std::io::{BufRead, BufReader};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

/// Opaque daemon handle. Owns the child process and stderr buffer.
pub struct BoltDaemon {
    child: Mutex<Option<Child>>,
    pid: u32,
    stderr_lines: Arc<Mutex<Vec<String>>>,
    ws_port: u16,
    data_dir: String,
    socket_path: String,
}

/// Find the bolt-daemon binary. Searches known paths.
/// Returns null if not found. Caller must free with bolt_free_string.
#[no_mangle]
pub extern "C" fn bolt_daemon_find_binary() -> *mut c_char {
    let search_paths = vec![
        // Sibling of this process
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("bolt-daemon")))
            .unwrap_or_default(),
        // Ecosystem workspace paths
        PathBuf::from(
            std::env::var("HOME").unwrap_or_default()
        ).join("Desktop/the9ines.com/bolt-ecosystem/bolt-daemon/target/release/bolt-daemon"),
    ];

    for path in &search_paths {
        if path.exists() {
            if let Some(s) = path.to_str() {
                return CString::new(s).map(|cs| cs.into_raw()).unwrap_or(std::ptr::null_mut());
            }
        }
    }
    std::ptr::null_mut()
}

/// Create and start a daemon process.
/// `daemon_bin` — path to bolt-daemon binary (C string)
/// `ws_port` — port for WS endpoint (0 = auto-assign based on PID)
/// Returns opaque handle, or null on failure.
///
/// # Safety
/// `daemon_bin` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_start(
    daemon_bin: *const c_char,
    ws_port: u16,
) -> *mut BoltDaemon {
    if daemon_bin.is_null() {
        return std::ptr::null_mut();
    }
    let bin_path = match CStr::from_ptr(daemon_bin).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let port = if ws_port == 0 {
        9100 + (std::process::id() % 900) as u16
    } else {
        ws_port
    };

    let pid = std::process::id();
    let data_dir = format!("/tmp/bolt-native-{pid}");
    let socket_path = format!("/tmp/bolt-native-{pid}.sock");
    let ws_listen = format!("0.0.0.0:{port}");

    let _ = std::fs::create_dir_all(&data_dir);
    // Daemon requires data_dir to be mode 0700 (identity key protection)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700));
    }

    let mut child = match Command::new(bin_path)
        .args(&[
            "--mode", "ws-endpoint",
            "--ws-listen", &ws_listen,
            "--socket-path", &socket_path,
            "--data-dir", &data_dir,
            "--pairing-policy", "allow",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[NATIVE_BRIDGE] daemon spawn failed: {e}");
            return std::ptr::null_mut();
        }
    };

    let child_pid = child.id();
    let stderr_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let lines_clone = Arc::clone(&stderr_lines);

    // Capture stderr on background thread
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if let Ok(mut buf) = lines_clone.lock() {
                    buf.push(line);
                    // Cap at 1000 lines
                    if buf.len() > 1000 {
                        buf.drain(..500);
                    }
                }
            }
        });
    }

    eprintln!("[NATIVE_BRIDGE] daemon started: pid={child_pid} ws=0.0.0.0:{port}");

    Box::into_raw(Box::new(BoltDaemon {
        child: Mutex::new(Some(child)),
        pid: child_pid,
        stderr_lines,
        ws_port: port,
        data_dir,
        socket_path,
    }))
}

/// Check if the daemon is still running. Returns 1 if running, 0 if not.
///
/// # Safety
/// `handle` must be a valid pointer returned by `bolt_daemon_start`.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_is_running(handle: *mut BoltDaemon) -> i32 {
    if handle.is_null() { return 0; }
    let daemon = &*handle;
    let mut guard = daemon.child.lock().unwrap();
    match guard.as_mut() {
        Some(child) => {
            if child.try_wait().ok().flatten().is_none() { 1 } else { 0 }
        }
        None => 0,
    }
}

/// Get the daemon's WS port.
///
/// # Safety
/// `handle` must be a valid pointer returned by `bolt_daemon_start`.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_ws_port(handle: *mut BoltDaemon) -> u16 {
    if handle.is_null() { return 0; }
    (*handle).ws_port
}

/// Get the daemon's PID.
///
/// # Safety
/// `handle` must be a valid pointer returned by `bolt_daemon_start`.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_pid(handle: *mut BoltDaemon) -> u32 {
    if handle.is_null() { return 0; }
    (*handle).pid
}

/// Get recent stderr lines from the daemon (last N lines, newline-separated).
/// Caller must free with `bolt_free_string`.
///
/// # Safety
/// `handle` must be a valid pointer returned by `bolt_daemon_start`.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_recent_stderr(
    handle: *mut BoltDaemon,
    last_n: u32,
) -> *mut c_char {
    if handle.is_null() { return std::ptr::null_mut(); }
    let daemon = &*handle;
    let lines = daemon.stderr_lines.lock().unwrap();
    let n = last_n as usize;
    let start = lines.len().saturating_sub(n);
    let joined = lines[start..].join("\n");
    CString::new(joined).map(|cs| cs.into_raw()).unwrap_or(std::ptr::null_mut())
}

/// Get the daemon's IPC socket path. Caller must free with `bolt_free_string`.
///
/// # Safety
/// `handle` must be a valid pointer returned by `bolt_daemon_start`.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_socket_path(handle: *mut BoltDaemon) -> *mut c_char {
    if handle.is_null() { return std::ptr::null_mut(); }
    let daemon = &*handle;
    CString::new(daemon.socket_path.as_str())
        .map(|cs| cs.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Get the daemon's data directory. Caller must free with `bolt_free_string`.
///
/// # Safety
/// `handle` must be a valid pointer returned by `bolt_daemon_start`.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_data_dir(handle: *mut BoltDaemon) -> *mut c_char {
    if handle.is_null() { return std::ptr::null_mut(); }
    let daemon = &*handle;
    CString::new(daemon.data_dir.as_str())
        .map(|cs| cs.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Trigger a file send via the daemon's send_file.signal mechanism.
/// `file_path` — absolute path to the file to send (C string).
/// Returns 1 on success, 0 on failure.
///
/// # Safety
/// `handle` must be valid. `file_path` must be a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_send_file(
    handle: *mut BoltDaemon,
    file_path: *const c_char,
) -> i32 {
    if handle.is_null() || file_path.is_null() { return 0; }
    let daemon = &*handle;
    let path = match CStr::from_ptr(file_path).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let signal_path = format!("{}/send_file.signal", daemon.data_dir);
    match std::fs::write(&signal_path, path) {
        Ok(()) => {
            eprintln!("[NATIVE_BRIDGE] wrote send_file.signal: {path}");
            1
        }
        Err(e) => {
            eprintln!("[NATIVE_BRIDGE] failed to write send_file.signal: {e}");
            0
        }
    }
}

/// Request the daemon to disconnect the active session (NATIVE-SESSION-UX-2).
/// Returns 1 on success, 0 on failure.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_disconnect_session(
    handle: *mut BoltDaemon,
) -> i32 {
    if handle.is_null() { return 0; }
    let daemon = &*handle;
    let signal_path = format!("{}/disconnect_session.signal", daemon.data_dir);
    match std::fs::write(&signal_path, "disconnect") {
        Ok(()) => {
            eprintln!("[NATIVE_BRIDGE] wrote disconnect_session.signal");
            1
        }
        Err(e) => {
            eprintln!("[NATIVE_BRIDGE] failed to write disconnect_session.signal: {e}");
            0
        }
    }
}

/// Trigger an outbound WS connection to a remote daemon (NATIVE-CONNECT-1).
/// `ws_url` — remote daemon wsUrl, e.g. "ws://192.168.4.36:9100" (C string).
/// Returns 1 on success, 0 on failure.
///
/// # Safety
/// `handle` must be valid. `ws_url` must be a null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_connect_remote(
    handle: *mut BoltDaemon,
    ws_url: *const c_char,
) -> i32 {
    if handle.is_null() || ws_url.is_null() { return 0; }
    let daemon = &*handle;
    let url = match CStr::from_ptr(ws_url).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let signal_path = format!("{}/connect_remote.signal", daemon.data_dir);
    match std::fs::write(&signal_path, url) {
        Ok(()) => {
            eprintln!("[NATIVE_BRIDGE] wrote connect_remote.signal: {url}");
            1
        }
        Err(e) => {
            eprintln!("[NATIVE_BRIDGE] failed to write connect_remote.signal: {e}");
            0
        }
    }
}

/// Stop the daemon and free the handle.
///
/// # Safety
/// `handle` must be a valid pointer returned by `bolt_daemon_start`.
/// After this call, `handle` is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_stop(handle: *mut BoltDaemon) {
    if handle.is_null() { return; }
    let daemon = Box::from_raw(handle);
    if let Ok(mut guard) = daemon.child.lock() {
        if let Some(ref mut child) = *guard {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    // Clean up temp files
    let _ = std::fs::remove_dir_all(&daemon.data_dir);
    eprintln!("[NATIVE_BRIDGE] daemon stopped: pid={}", daemon.pid);
}
