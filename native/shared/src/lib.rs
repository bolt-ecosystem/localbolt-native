//! bolt-native-bridge — C-ABI FFI bridge for native platform shells.
//!
//! Exposes bolt-app-core functionality to non-Rust shells (SwiftUI, Kotlin, etc.)
//! via a stable C ABI. Platform shells link against the static/dynamic library
//! produced by this crate.
//!
//! # Design
//!
//! - All public functions use `extern "C"` with C-compatible types
//! - Strings passed as null-terminated C strings, returned as owned allocations
//! - Caller is responsible for freeing returned strings via `bolt_free_string`
//! - Opaque handles for stateful objects (signaling client, daemon process)
//! - No panics across FFI boundary — all functions return error codes or null
//!
//! # Current surface (minimal scaffold)
//!
//! - `bolt_generate_peer_code` — generate a secure peer code
//! - `bolt_platform_data_dir` — platform-specific data directory
//! - `bolt_platform_ipc_path` — platform-specific IPC socket path
//! - `bolt_free_string` — free a string returned by any bolt_ function

use std::ffi::{CStr, CString};
use std::io::{BufRead, BufReader};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

mod daemon;
mod ipc;
mod signaling;
pub use daemon::*;
pub use ipc::*;
pub use signaling::*;

/// Generate a secure peer code. Caller must free with `bolt_free_string`.
#[no_mangle]
pub extern "C" fn bolt_generate_peer_code() -> *mut c_char {
    let code = bolt_core::peer_code::generate_secure_peer_code();
    match CString::new(code) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get the platform-specific data directory. Caller must free with `bolt_free_string`.
#[no_mangle]
pub extern "C" fn bolt_platform_data_dir() -> *mut c_char {
    let dir = bolt_app_core::platform::default_data_dir();
    match CString::new(dir) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Get the platform-specific IPC socket path. Caller must free with `bolt_free_string`.
#[no_mangle]
pub extern "C" fn bolt_platform_ipc_path() -> *mut c_char {
    let path = bolt_app_core::platform::default_ipc_path();
    match CString::new(path) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Probe signal server health. Returns 1 if healthy, 0 if not.
#[no_mangle]
pub extern "C" fn bolt_probe_signal_health() -> i32 {
    if bolt_app_core::signal_monitor::probe_signal_health() {
        1
    } else {
        0
    }
}

/// Free a string returned by any bolt_ function.
///
/// # Safety
/// `ptr` must have been returned by a bolt_ function and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn bolt_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

#[cfg(test)]
mod contract_parity;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_peer_code_returns_non_null() {
        let ptr = bolt_generate_peer_code();
        assert!(!ptr.is_null());
        unsafe {
            let code = CStr::from_ptr(ptr).to_str().unwrap();
            assert_eq!(code.len(), 6); // peer codes are 6 chars
            bolt_free_string(ptr);
        }
    }

    #[test]
    fn platform_data_dir_returns_non_null() {
        let ptr = bolt_platform_data_dir();
        assert!(!ptr.is_null());
        unsafe {
            let dir = CStr::from_ptr(ptr).to_str().unwrap();
            assert!(!dir.is_empty());
            bolt_free_string(ptr);
        }
    }

    #[test]
    fn platform_ipc_path_returns_non_null() {
        let ptr = bolt_platform_ipc_path();
        assert!(!ptr.is_null());
        unsafe {
            let path = CStr::from_ptr(ptr).to_str().unwrap();
            assert!(!path.is_empty());
            bolt_free_string(ptr);
        }
    }

    #[test]
    fn free_null_is_safe() {
        unsafe {
            bolt_free_string(std::ptr::null_mut());
        }
    }
}
