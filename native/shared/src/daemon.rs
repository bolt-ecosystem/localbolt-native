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

/// How far up from the executable to look for a sibling `bolt-daemon` checkout.
/// A dev build sits at `<repo>/native/macos/.build/release/LocalBolt`, so the
/// ecosystem root is 5 levels up; a little headroom covers other layouts without
/// walking to `/`.
const DEV_LOOKUP_MAX_ANCESTORS: usize = 8;

/// Ordered candidate paths for the bolt-daemon binary.
///
/// Pure: takes the running executable rather than reading the environment, so the
/// lookup order is unit-testable without touching the filesystem.
///
/// Order, highest priority first:
///   1. `BOLT_DAEMON_PATH` — explicit override, for pointing at any local build.
///   2. Sibling of the executable — the app-bundle sidecar at
///      `LocalBolt.app/Contents/MacOS/bolt-daemon`, installed by `build-app.sh`.
///      This is the only path a shipped install relies on.
///   3. A sibling `bolt-daemon` checkout, found by walking up from the executable.
///      This keeps developer builds (`swift build`, no bundle) working without
///      baking any absolute home directory into the binary.
pub(crate) fn daemon_candidate_paths(
    current_exe: Option<&std::path::Path>,
    explicit_override: Option<&str>,
) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(explicit) = explicit_override {
        if !explicit.is_empty() {
            candidates.push(PathBuf::from(explicit));
        }
    }

    if let Some(exe_dir) = current_exe.and_then(|p| p.parent()) {
        // Production: the sidecar inside the app bundle.
        candidates.push(exe_dir.join("bolt-daemon"));

        // Development: a sibling bolt-daemon checkout somewhere above us. Relative to
        // the executable, so it never encodes a specific user's home directory.
        for ancestor in exe_dir.ancestors().take(DEV_LOOKUP_MAX_ANCESTORS) {
            candidates.push(
                ancestor
                    .join("bolt-daemon")
                    .join("target/release/bolt-daemon"),
            );
            candidates.push(ancestor.join("bolt-daemon").join("target/debug/bolt-daemon"));
        }
    }

    candidates
}

/// First candidate that exists on disk, or None.
pub(crate) fn resolve_daemon_binary(
    current_exe: Option<&std::path::Path>,
    explicit_override: Option<&str>,
) -> Option<PathBuf> {
    let candidates = daemon_candidate_paths(current_exe, explicit_override);
    for path in &candidates {
        if path.exists() {
            return Some(path.clone());
        }
    }
    eprintln!("[bolt-daemon-lookup] no bolt-daemon binary found. Checked, in order:");
    for path in &candidates {
        eprintln!("[bolt-daemon-lookup]   {}", path.display());
    }
    None
}

/// Find the bolt-daemon binary. Searches known paths.
/// Returns null if not found. Caller must free with bolt_free_string.
#[no_mangle]
pub extern "C" fn bolt_daemon_find_binary() -> *mut c_char {
    let exe = std::env::current_exe().ok();
    let explicit = std::env::var("BOLT_DAEMON_PATH").ok();
    match resolve_daemon_binary(exe.as_deref(), explicit.as_deref()) {
        Some(path) => path
            .to_str()
            .and_then(|s| CString::new(s).ok())
            .map(|cs| cs.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        None => std::ptr::null_mut(),
    }
}

/// Build the argv for spawning bolt-daemon. Extracted so the spawn arguments — in
/// particular the pairing policy — are unit-testable without launching a process.
///
/// `--pairing-policy ask`: now that the daemon trust path fails closed (EA2 legacy
/// closure, EA3 WT gate, item-2 fail-closed `trust_config`), the native app must NOT
/// launch with `allow`, which accepted any LAN peer with no prompt and no SAS. `ask`
/// denies unpinned inbound by default. Authorization hardening only — this does not add
/// an interactive prompt or any "verified"/pin behavior (that is EA1/EA4).
fn daemon_spawn_args<'a>(
    ws_listen: &'a str,
    socket_path: &'a str,
    data_dir: &'a str,
) -> [&'a str; 10] {
    [
        "--mode",
        "ws-endpoint",
        "--ws-listen",
        ws_listen,
        "--socket-path",
        socket_path,
        "--data-dir",
        data_dir,
        "--pairing-policy",
        "ask",
    ]
}

/// The daemon data directory: the persistent, platform-default location for the
/// identity key + TOFU trust store. EA8: replaces the predictable, world-adjacent
/// `/tmp/bolt-native-<pid>` dir that regenerated identity on every launch (degrading
/// TOFU/MITM protection). Persisting to the platform default lets identity + pins
/// survive restarts. NOTE: the `bolt-daemon` identity_store hardening (reject symlink
/// parent, verify uid, O_EXCL 0600) is a separate EA8 sub-item in the bolt-daemon repo,
/// out of this native-app change.
fn daemon_data_dir() -> String {
    bolt_app_core::platform::default_data_dir()
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
    let data_dir = daemon_data_dir();
    // The IPC socket stays under /tmp: it is ephemeral, and a socket path inside the
    // platform data dir can exceed the AF_UNIX sun_path limit (~104 bytes on macOS).
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
        .args(daemon_spawn_args(&ws_listen, &socket_path, &data_dir))
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
    if handle.is_null() {
        return 0;
    }
    let daemon = &*handle;
    let mut guard = daemon.child.lock().unwrap();
    match guard.as_mut() {
        Some(child) => {
            if child.try_wait().ok().flatten().is_none() {
                1
            } else {
                0
            }
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
    if handle.is_null() {
        return 0;
    }
    (*handle).ws_port
}

/// Get the daemon's PID.
///
/// # Safety
/// `handle` must be a valid pointer returned by `bolt_daemon_start`.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_pid(handle: *mut BoltDaemon) -> u32 {
    if handle.is_null() {
        return 0;
    }
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
    if handle.is_null() {
        return std::ptr::null_mut();
    }
    let daemon = &*handle;
    let lines = daemon.stderr_lines.lock().unwrap();
    let n = last_n as usize;
    let start = lines.len().saturating_sub(n);
    let joined = lines[start..].join("\n");
    CString::new(joined)
        .map(|cs| cs.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// Get the daemon's IPC socket path. Caller must free with `bolt_free_string`.
///
/// # Safety
/// `handle` must be a valid pointer returned by `bolt_daemon_start`.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_socket_path(handle: *mut BoltDaemon) -> *mut c_char {
    if handle.is_null() {
        return std::ptr::null_mut();
    }
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
    if handle.is_null() {
        return std::ptr::null_mut();
    }
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
    if handle.is_null() || file_path.is_null() {
        return 0;
    }
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
pub unsafe extern "C" fn bolt_daemon_disconnect_session(handle: *mut BoltDaemon) -> i32 {
    if handle.is_null() {
        return 0;
    }
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
    if handle.is_null() || ws_url.is_null() {
        return 0;
    }
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

/// Trigger an outbound native connection with structured WS/QUIC metadata.
///
/// The daemon routes complete QUIC metadata to the QUIC app-session adapter and
/// keeps `wsUrl` as the fallback when QUIC metadata is missing or connect fails.
///
/// # Safety
/// `handle` must be valid. Non-null string pointers must be null-terminated.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_connect_remote_v2(
    handle: *mut BoltDaemon,
    ws_url: *const c_char,
    quic_addr: *const c_char,
    quic_cert_hash: *const c_char,
) -> i32 {
    if handle.is_null() {
        return 0;
    }
    let daemon = &*handle;

    let ws_url = match optional_cstr(ws_url) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let quic_addr = match optional_cstr(quic_addr) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let quic_cert_hash = match optional_cstr(quic_cert_hash) {
        Ok(value) => value,
        Err(_) => return 0,
    };

    if ws_url.is_none() && quic_addr.is_none() {
        return 0;
    }

    let payload = serde_json::json!({
        "wsUrl": ws_url,
        "quicAddr": quic_addr,
        "quicCertHash": quic_cert_hash,
    });
    let signal_path = format!("{}/connect_remote.signal", daemon.data_dir);
    match std::fs::write(&signal_path, payload.to_string()) {
        Ok(()) => {
            eprintln!("[NATIVE_BRIDGE] wrote structured connect_remote.signal");
            1
        }
        Err(e) => {
            eprintln!("[NATIVE_BRIDGE] failed to write connect_remote.signal: {e}");
            0
        }
    }
}

/// Allow a remote QUIC client certificate hash for the next inbound session.
///
/// The native shell calls this on the acceptor side before sending
/// `connection_accepted`, using the requester's `quicCertHash` from
/// `connection_request`. The daemon consumes this signal into its dynamic QUIC
/// client-cert allowlist.
///
/// # Safety
/// `handle` must be valid. `quic_cert_hash` must be a null-terminated string.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_allow_quic_peer_cert_hash(
    handle: *mut BoltDaemon,
    quic_cert_hash: *const c_char,
) -> i32 {
    if handle.is_null() || quic_cert_hash.is_null() {
        return 0;
    }
    let daemon = &*handle;
    let hash = match CStr::from_ptr(quic_cert_hash).to_str() {
        Ok(value) => value.trim(),
        Err(_) => return 0,
    };
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return 0;
    }

    let signal_path = format!("{}/allow_quic_peer.signal", daemon.data_dir);
    match std::fs::write(&signal_path, hash) {
        Ok(()) => {
            eprintln!("[NATIVE_BRIDGE] wrote allow_quic_peer.signal");
            1
        }
        Err(e) => {
            eprintln!("[NATIVE_BRIDGE] failed to write allow_quic_peer.signal: {e}");
            0
        }
    }
}

unsafe fn optional_cstr(ptr: *const c_char) -> Result<Option<String>, std::str::Utf8Error> {
    if ptr.is_null() {
        return Ok(None);
    }
    let value = CStr::from_ptr(ptr).to_str()?.trim().to_string();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

/// Request the daemon to pause the active transfer (DAEMON-TRANSFER-CONTROL-1).
/// Returns 1 on success, 0 on failure.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_pause_transfer(handle: *mut BoltDaemon) -> i32 {
    if handle.is_null() {
        return 0;
    }
    let daemon = &*handle;
    let signal_path = format!("{}/transfer_pause.signal", daemon.data_dir);
    match std::fs::write(&signal_path, "pause") {
        Ok(()) => {
            eprintln!("[NATIVE_BRIDGE] wrote transfer_pause.signal");
            1
        }
        Err(e) => {
            eprintln!("[NATIVE_BRIDGE] failed to write transfer_pause.signal: {e}");
            0
        }
    }
}

/// Request the daemon to resume the active transfer (DAEMON-TRANSFER-CONTROL-1).
/// Returns 1 on success, 0 on failure.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn bolt_daemon_resume_transfer(handle: *mut BoltDaemon) -> i32 {
    if handle.is_null() {
        return 0;
    }
    let daemon = &*handle;
    let signal_path = format!("{}/transfer_resume.signal", daemon.data_dir);
    match std::fs::write(&signal_path, "resume") {
        Ok(()) => {
            eprintln!("[NATIVE_BRIDGE] wrote transfer_resume.signal");
            1
        }
        Err(e) => {
            eprintln!("[NATIVE_BRIDGE] failed to write transfer_resume.signal: {e}");
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
    if handle.is_null() {
        return;
    }
    let daemon = Box::from_raw(handle);
    if let Ok(mut guard) = daemon.child.lock() {
        if let Some(ref mut child) = *guard {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    // EA8: data_dir is now the PERSISTENT platform identity + trust store — do NOT
    // delete it on stop (that would regenerate identity every launch and wipe TOFU
    // pins). Only the ephemeral IPC socket is cleaned up here.
    let _ = std::fs::remove_file(&daemon.socket_path);
    eprintln!("[NATIVE_BRIDGE] daemon stopped: pid={}", daemon.pid);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Daemon binary lookup ─────────────────────────────────────────────
    //
    // Production installs get the daemon as a bundle sidecar at
    // LocalBolt.app/Contents/MacOS/bolt-daemon (installed by build-app.sh), which the
    // exe-sibling candidate finds. Developer builds run the bare Swift binary from
    // .build/release, where there is no sidecar, so the lookup must also find the
    // sibling bolt-daemon checkout without baking in anyone's home directory.

    /// Unique scratch dir for a lookup test.
    fn lookup_tmp(tag: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "bolt-lookup-{tag}-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch_exec(path: &std::path::Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"#!/bin/sh\nexit 0\n").unwrap();
    }

    /// The stale `~/Desktop/the9ines.com` developer path must not be consulted: the
    /// ecosystem no longer lives there, and a shipped .app must never depend on a
    /// path inside someone's home directory.
    #[test]
    fn lookup_has_no_stale_desktop_fallback() {
        let exe = PathBuf::from("/Applications/LocalBolt.app/Contents/MacOS/LocalBolt");
        let candidates = daemon_candidate_paths(Some(&exe), None);

        for candidate in &candidates {
            let s = candidate.to_string_lossy();
            assert!(
                !s.contains("Desktop/the9ines.com"),
                "stale Desktop fallback must be gone, found: {s}"
            );
        }
    }

    /// No candidate may hardcode a specific user's home directory.
    #[test]
    fn lookup_hardcodes_no_absolute_home() {
        let exe = PathBuf::from("/Applications/LocalBolt.app/Contents/MacOS/LocalBolt");
        for candidate in daemon_candidate_paths(Some(&exe), None) {
            let s = candidate.to_string_lossy();
            assert!(
                !s.contains("/Users/oberfelder"),
                "no candidate may hardcode a developer home: {s}"
            );
        }
    }

    /// Production: the daemon sidecar beside the app executable is found.
    #[test]
    fn lookup_finds_bundled_sidecar() {
        let root = lookup_tmp("bundle");
        let macos_dir = root.join("LocalBolt.app/Contents/MacOS");
        let exe = macos_dir.join("LocalBolt");
        let sidecar = macos_dir.join("bolt-daemon");
        touch_exec(&exe);
        touch_exec(&sidecar);

        let found = resolve_daemon_binary(Some(&exe), None);

        assert_eq!(
            found.as_deref(),
            Some(sidecar.as_path()),
            "must find the Contents/MacOS/bolt-daemon sidecar"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// Developer build: running .build/release/LocalBolt from an ecosystem checkout
    /// must find the sibling bolt-daemon repo's release binary, discovered by walking
    /// up from the executable rather than from any hardcoded absolute path.
    #[test]
    fn lookup_finds_dev_repo_relative_daemon() {
        let root = lookup_tmp("dev");
        let exe = root.join("bolt-ecosystem/localbolt-app/native/macos/.build/release/LocalBolt");
        let daemon = root.join("bolt-ecosystem/bolt-daemon/target/release/bolt-daemon");
        touch_exec(&exe);
        touch_exec(&daemon);

        let found = resolve_daemon_binary(Some(&exe), None);

        assert_eq!(
            found.as_deref(),
            Some(daemon.as_path()),
            "must find the sibling bolt-daemon checkout from a dev build"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// An explicit override wins over everything else, so a developer can point at
    /// any build without editing source.
    #[test]
    fn lookup_explicit_override_wins() {
        let root = lookup_tmp("override");
        let macos_dir = root.join("LocalBolt.app/Contents/MacOS");
        let exe = macos_dir.join("LocalBolt");
        let sidecar = macos_dir.join("bolt-daemon");
        let chosen = root.join("custom/bolt-daemon");
        touch_exec(&exe);
        touch_exec(&sidecar);
        touch_exec(&chosen);

        let found = resolve_daemon_binary(Some(&exe), Some(chosen.to_str().unwrap()));

        assert_eq!(
            found.as_deref(),
            Some(chosen.as_path()),
            "explicit override must take priority over the bundled sidecar"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// Nothing found: return None rather than a bogus path.
    #[test]
    fn lookup_returns_none_when_absent() {
        let root = lookup_tmp("absent");
        let exe = root.join("LocalBolt.app/Contents/MacOS/LocalBolt");
        touch_exec(&exe);

        assert!(resolve_daemon_binary(Some(&exe), None).is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    /// The native app must launch bolt-daemon with the fail-closed `ask` pairing
    /// policy, never `allow` (which accepted any LAN peer with no prompt / no SAS).
    #[test]
    fn spawn_args_use_ask_not_allow() {
        let args = daemon_spawn_args("0.0.0.0:9100", "/tmp/x.sock", "/tmp/data");
        let pos = args
            .iter()
            .position(|a| *a == "--pairing-policy")
            .expect("spawn args must set --pairing-policy");
        assert_eq!(
            args[pos + 1],
            "ask",
            "native app must launch the daemon with `ask`, not `allow`"
        );
        assert!(
            !args.contains(&"allow"),
            "no spawn arg may be `allow` (fail-closed default only)"
        );
    }

    /// EA8: the identity + trust store must live at the persistent platform default,
    /// never the predictable `/tmp/bolt-native-<pid>` path it used before.
    #[test]
    fn data_dir_is_platform_default_not_tmp() {
        let dir = daemon_data_dir();
        assert!(
            !dir.starts_with("/tmp/bolt-native-"),
            "identity/trust dir must not be a predictable /tmp/bolt-native-<pid> path, got: {dir}"
        );
        assert_eq!(
            dir,
            bolt_app_core::platform::default_data_dir(),
            "identity/trust dir must be the platform default data dir"
        );
    }
}
