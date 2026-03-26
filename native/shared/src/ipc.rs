//! IPC bridge FFI — connects native shell to daemon event stream.
//!
//! Wraps `IpcBridgeCore` from bolt-app-core with C-ABI FFI functions.
//! Events are queued as JSON strings for polling by the platform shell.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::sync::{Arc, Mutex};

use bolt_app_core::ipc_bridge_core::IpcBridgeCore;
use bolt_app_core::ipc_types::IpcMessage;

/// Opaque IPC bridge handle.
pub struct BoltIpc {
    bridge: Arc<IpcBridgeCore>,
    events: Arc<Mutex<Vec<String>>>,
}

/// Start IPC bridge. Connects to daemon socket, performs version handshake,
/// begins event loop. Returns opaque handle or null on failure.
///
/// `socket_path`: path to daemon IPC socket (C string).
/// `app_version`: app version for handshake (C string).
#[no_mangle]
pub extern "C" fn bolt_ipc_start(
    socket_path: *const c_char,
    app_version: *const c_char,
) -> *mut BoltIpc {
    let socket_path = match unsafe { validate_cstr(socket_path) } {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    let app_version = match unsafe { validate_cstr(app_version) } {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };

    let bridge = Arc::new(IpcBridgeCore::new());
    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Register event callback that queues JSON events
    let events_clone = Arc::clone(&events);
    bridge.set_event_callback(Box::new(move |event_name, payload| {
        let event = serde_json::json!({
            "event": event_name,
            "payload": payload,
        });
        if let Ok(json) = serde_json::to_string(&event) {
            let mut q = events_clone.lock().unwrap();
            // Cap queue at 1000 to prevent unbounded growth
            if q.len() < 1000 {
                q.push(json);
            }
        }
    }));

    // Connect and handshake
    if let Err(e) = bridge.start(Path::new(&socket_path), &app_version) {
        eprintln!("[IPC_FFI] start failed: {e}");
        return std::ptr::null_mut();
    }

    let handle = Box::new(BoltIpc {
        bridge,
        events,
    });
    Box::into_raw(handle)
}

/// Check if IPC bridge is connected. Returns 1 if connected, 0 if not.
#[no_mangle]
pub extern "C" fn bolt_ipc_is_connected(handle: *mut BoltIpc) -> i32 {
    let Some(h) = (unsafe { handle.as_ref() }) else { return 0 };
    if h.bridge.is_connected() { 1 } else { 0 }
}

/// Drain all pending events as a newline-separated JSON string.
/// Each line is a JSON object: {"event": "...", "payload": {...}}
/// Returns null if no events. Caller must free with bolt_free_string.
#[no_mangle]
pub extern "C" fn bolt_ipc_drain_events(handle: *mut BoltIpc) -> *mut c_char {
    let Some(h) = (unsafe { handle.as_ref() }) else { return std::ptr::null_mut() };

    let mut q = h.events.lock().unwrap();
    if q.is_empty() {
        return std::ptr::null_mut();
    }

    let joined = q.join("\n");
    q.clear();

    match CString::new(joined) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Send a decision to the daemon.
/// `msg_type`: IPC message type (e.g. "pairing.decision"), C string.
/// `payload_json`: JSON payload string, C string.
/// Returns 1 on success, 0 on failure.
#[no_mangle]
pub extern "C" fn bolt_ipc_send_decision(
    handle: *mut BoltIpc,
    msg_type: *const c_char,
    payload_json: *const c_char,
) -> i32 {
    let Some(h) = (unsafe { handle.as_ref() }) else { return 0 };
    let msg_type = match unsafe { validate_cstr(msg_type) } {
        Some(s) => s,
        None => return 0,
    };
    let payload_json = match unsafe { validate_cstr(payload_json) } {
        Some(s) => s,
        None => return 0,
    };

    let payload: serde_json::Value = match serde_json::from_str(&payload_json) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[IPC_FFI] invalid payload JSON: {e}");
            return 0;
        }
    };

    let msg = IpcMessage::new_decision(&msg_type, payload);
    match h.bridge.send_decision(msg) {
        Ok(()) => 1,
        Err(e) => {
            eprintln!("[IPC_FFI] send_decision failed: {e}");
            0
        }
    }
}

/// Stop the IPC bridge and free the handle.
/// After this call, handle is invalid.
#[no_mangle]
pub extern "C" fn bolt_ipc_stop(handle: *mut BoltIpc) {
    if handle.is_null() { return; }
    let h = unsafe { Box::from_raw(handle) };
    h.bridge.shutdown();
}

/// Helper: validate a C string pointer and convert to owned String.
unsafe fn validate_cstr(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() { return None; }
    CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_with_bad_path_returns_null() {
        let path = CString::new("/nonexistent/socket").unwrap();
        let ver = CString::new("0.1.0").unwrap();
        let handle = bolt_ipc_start(path.as_ptr(), ver.as_ptr());
        assert!(handle.is_null());
    }

    #[test]
    fn stop_null_is_safe() {
        bolt_ipc_stop(std::ptr::null_mut());
    }

    #[test]
    fn is_connected_null_returns_zero() {
        assert_eq!(bolt_ipc_is_connected(std::ptr::null_mut()), 0);
    }

    #[test]
    fn drain_events_null_returns_null() {
        assert!(bolt_ipc_drain_events(std::ptr::null_mut()).is_null());
    }
}
