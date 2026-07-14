//! Signaling client FFI — peer discovery and connection signals.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};

use bolt_app_core::signaling_client::{
    self, DiscoveryEvent, Plane, PeerInfo, SignalingConfig, SignalingHandle,
};

/// Discovered peer (C-compatible).
#[repr(C)]
pub struct BoltPeer {
    pub peer_code: *mut c_char,
    pub device_name: *mut c_char,
    pub device_type: *mut c_char,
    /// WebTransport URL (null if not available).
    pub wt_url: *mut c_char,
    /// WebTransport TLS cert SHA-256 hash hex (null if not available).
    pub wt_cert_hash: *mut c_char,
}

/// Signaling event type.
#[repr(C)]
pub enum BoltSignalingEventType {
    PeerJoined = 0,
    PeerLeft = 1,
    Connected = 2,
    Disconnected = 3,
}

/// A signaling event (C-compatible).
#[repr(C)]
pub struct BoltSignalingEvent {
    pub event_type: BoltSignalingEventType,
    pub peer_code: *mut c_char,
    pub device_name: *mut c_char,
    pub plane: i32, // 0=local, 1=cloud
}

/// Opaque signaling handle.
pub struct BoltSignaling {
    handle: SignalingHandle,
    peer_code: String,
    events: Arc<Mutex<Vec<SignalingEventInternal>>>,
    peers: Arc<Mutex<Vec<PeerInfo>>>,
    connected: Arc<std::sync::atomic::AtomicBool>,
    /// Incoming signals queued for the shell to read (JSON strings).
    incoming_signals: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone)]
enum SignalingEventInternal {
    PeerJoined(PeerInfo, Plane),
    PeerLeft(String, Plane),
    Connected(Plane),
    Disconnected(String, Plane),
}

/// Start signaling client. Returns opaque handle.
/// `local_url` — local signaling server URL (e.g. ws://127.0.0.1:3001)
/// `cloud_url` — cloud signaling URL (e.g. wss://bolt-rendezvous.fly.dev), or null
/// `peer_code` — this device's peer code
/// `device_name` — this device's name
/// `wt_url` — WebTransport URL to advertise (null if WT not available)
/// `wt_cert_hash` — WebTransport cert hash hex (null if WT not available)
///
/// # Safety
/// All string parameters must be valid null-terminated C strings (or null for optional).
#[no_mangle]
pub unsafe extern "C" fn bolt_signaling_start(
    local_url: *const c_char,
    cloud_url: *const c_char,
    peer_code: *const c_char,
    device_name: *const c_char,
    wt_url: *const c_char,
    wt_cert_hash: *const c_char,
) -> *mut BoltSignaling {
    let local = cstr_to_string(local_url);
    let cloud = if cloud_url.is_null() { None } else { Some(cstr_to_string(cloud_url)) };
    let code = cstr_to_string(peer_code);
    let name = cstr_to_string(device_name);
    let wt_url_opt = if wt_url.is_null() { None } else { Some(cstr_to_string(wt_url)) };
    let wt_hash_opt = if wt_cert_hash.is_null() { None } else { Some(cstr_to_string(wt_cert_hash)) };

    let events: Arc<Mutex<Vec<SignalingEventInternal>>> = Arc::new(Mutex::new(Vec::new()));
    let peers: Arc<Mutex<Vec<PeerInfo>>> = Arc::new(Mutex::new(Vec::new()));
    let connected: Arc<std::sync::atomic::AtomicBool> = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let incoming_signals: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let events_cb = Arc::clone(&events);
    let peers_cb = Arc::clone(&peers);
    let connected_cb = Arc::clone(&connected);
    let signals_cb = Arc::clone(&incoming_signals);
    let code_clone = code.clone();

    let config = SignalingConfig {
        server_url: local,
        cloud_url: cloud,
        peer_code: code.clone(),
        device_name: name,
        device_type: "desktop".to_string(),
        wt_url: wt_url_opt,
        wt_cert_hash: wt_hash_opt,
    };

    let handle = signaling_client::spawn_signaling_client(
        config,
        Box::new(move |event| {
            match &event {
                DiscoveryEvent::PeerList(list, _plane) => {
                    let mut p = peers_cb.lock().unwrap();
                    // Merge, skip self
                    for peer in list {
                        if peer.peer_code != code_clone
                            && !p.iter().any(|existing| existing.peer_code == peer.peer_code)
                        {
                            p.push(peer.clone());
                        }
                    }
                }
                DiscoveryEvent::PeerJoined(peer, plane) => {
                    if peer.peer_code != code_clone {
                        let mut p = peers_cb.lock().unwrap();
                        if !p.iter().any(|existing| existing.peer_code == peer.peer_code) {
                            p.push(peer.clone());
                        }
                        events_cb.lock().unwrap().push(
                            SignalingEventInternal::PeerJoined(peer.clone(), *plane)
                        );
                    }
                }
                DiscoveryEvent::PeerLeft(code, plane) => {
                    let mut p = peers_cb.lock().unwrap();
                    p.retain(|peer| peer.peer_code != *code);
                    events_cb.lock().unwrap().push(
                        SignalingEventInternal::PeerLeft(code.clone(), *plane)
                    );
                }
                DiscoveryEvent::Connected(plane) => {
                    connected_cb.store(true, std::sync::atomic::Ordering::Relaxed);
                    events_cb.lock().unwrap().push(
                        SignalingEventInternal::Connected(*plane)
                    );
                }
                DiscoveryEvent::Disconnected(reason, plane) => {
                    // Only mark disconnected if both planes are down
                    // (one plane disconnecting shouldn't mark offline)
                    events_cb.lock().unwrap().push(
                        SignalingEventInternal::Disconnected(reason.clone(), *plane)
                    );
                }
                DiscoveryEvent::Signal(sig, _plane) => {
                    // Queue incoming signals as JSON for the shell to process
                    if let Ok(json) = serde_json::to_string(&serde_json::json!({
                        "from": sig.from,
                        "signal_type": sig.signal_type,
                        "data": sig.data,
                    })) {
                        signals_cb.lock().unwrap().push(json);
                    }
                }
                DiscoveryEvent::Error(msg) => {
                    eprintln!("[NATIVE_SIGNALING] error: {msg}");
                }
            }
        }),
    );

    Box::into_raw(Box::new(BoltSignaling {
        handle,
        peer_code: code,
        events,
        peers,
        connected,
        incoming_signals,
    }))
}

/// Get the number of currently discovered peers.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn bolt_signaling_peer_count(handle: *mut BoltSignaling) -> u32 {
    if handle.is_null() { return 0; }
    (*handle).peers.lock().unwrap().len() as u32
}

/// Get a discovered peer by index. Returns null if out of bounds.
/// Caller must free the returned BoltPeer with bolt_peer_free.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn bolt_signaling_get_peer(
    handle: *mut BoltSignaling,
    index: u32,
) -> *mut BoltPeer {
    if handle.is_null() { return std::ptr::null_mut(); }
    let peers = (*handle).peers.lock().unwrap();
    let i = index as usize;
    if i >= peers.len() { return std::ptr::null_mut(); }

    let peer = &peers[i];
    let wt_url_ptr = peer.wt_url.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok())
        .map(|cs| cs.into_raw())
        .unwrap_or(std::ptr::null_mut());
    let wt_hash_ptr = peer.wt_cert_hash.as_ref()
        .and_then(|s| CString::new(s.as_str()).ok())
        .map(|cs| cs.into_raw())
        .unwrap_or(std::ptr::null_mut());
    Box::into_raw(Box::new(BoltPeer {
        // EA9: peer_code/device_name/device_type come from untrusted rendezvous JSON.
        // An interior NUL made CString::new().unwrap() abort the whole app (zero-
        // interaction remote DoS). Mirror the wt_url handling above: a malformed field
        // becomes a null pointer (the Swift host null-coalesces it) instead of aborting.
        peer_code: CString::new(peer.peer_code.as_str())
            .map(|cs| cs.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        device_name: CString::new(peer.device_name.as_str())
            .map(|cs| cs.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        device_type: CString::new(peer.device_type.as_str())
            .map(|cs| cs.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        wt_url: wt_url_ptr,
        wt_cert_hash: wt_hash_ptr,
    }))
}

/// Free a BoltPeer returned by bolt_signaling_get_peer.
///
/// # Safety
/// `peer` must have been returned by bolt_signaling_get_peer.
#[no_mangle]
pub unsafe extern "C" fn bolt_peer_free(peer: *mut BoltPeer) {
    if peer.is_null() { return; }
    let p = Box::from_raw(peer);
    if !p.peer_code.is_null() { let _ = CString::from_raw(p.peer_code); }
    if !p.device_name.is_null() { let _ = CString::from_raw(p.device_name); }
    if !p.device_type.is_null() { let _ = CString::from_raw(p.device_type); }
    if !p.wt_url.is_null() { let _ = CString::from_raw(p.wt_url); }
    if !p.wt_cert_hash.is_null() { let _ = CString::from_raw(p.wt_cert_hash); }
}

/// Check if signaling is connected to at least one plane.
/// Returns 1 if connected, 0 if not.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn bolt_signaling_is_connected(handle: *mut BoltSignaling) -> i32 {
    if handle.is_null() { return 0; }
    if (*handle).connected.load(std::sync::atomic::Ordering::Relaxed) { 1 } else { 0 }
}

/// Drain pending events and return count. Use bolt_signaling_peer_count and
/// bolt_signaling_get_peer to read the current peer list after draining.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn bolt_signaling_drain_events(handle: *mut BoltSignaling) -> u32 {
    if handle.is_null() { return 0; }
    let mut events = (*handle).events.lock().unwrap();
    let count = events.len() as u32;
    events.clear();
    count
}

/// Drain incoming signals as newline-separated JSON.
/// Each line: {"from":"...", "signal_type":"...", "data":{...}}
/// Returns null if no signals. Caller must free with bolt_free_string.
///
/// # Safety
/// `handle` must be valid.
#[no_mangle]
pub unsafe extern "C" fn bolt_signaling_drain_signals(handle: *mut BoltSignaling) -> *mut c_char {
    if handle.is_null() { return std::ptr::null_mut(); }
    let mut sigs = (*handle).incoming_signals.lock().unwrap();
    if sigs.is_empty() { return std::ptr::null_mut(); }
    let joined = sigs.join("\n");
    sigs.clear();
    CString::new(joined).map(|cs| cs.into_raw()).unwrap_or(std::ptr::null_mut())
}

/// Send a signal to a peer (connection initiation).
/// `to_peer_code` — target peer code (C string).
/// `signal_type` — signal type string (C string, e.g. "connect-request").
/// `data_json` — JSON payload string (C string), or null for empty object.
/// Returns 1 on success, 0 on failure.
///
/// # Safety
/// `handle` must be valid. String parameters must be null-terminated.
#[no_mangle]
pub unsafe extern "C" fn bolt_signaling_send_signal(
    handle: *mut BoltSignaling,
    to_peer_code: *const c_char,
    signal_type: *const c_char,
    data_json: *const c_char,
) -> i32 {
    if handle.is_null() || to_peer_code.is_null() || signal_type.is_null() {
        return 0;
    }
    let sig = &*handle;
    let to = cstr_to_string(to_peer_code);
    let sig_type = cstr_to_string(signal_type);
    let data: serde_json::Value = if data_json.is_null() {
        serde_json::json!({})
    } else {
        let json_str = cstr_to_string(data_json);
        serde_json::from_str(&json_str).unwrap_or(serde_json::json!({}))
    };

    sig.handle.send_signal(&to, &sig_type, data, &sig.peer_code);
    1
}

/// Stop signaling and free the handle.
///
/// # Safety
/// `handle` must be valid. After this call, handle is invalid.
#[no_mangle]
pub unsafe extern "C" fn bolt_signaling_stop(handle: *mut BoltSignaling) {
    if handle.is_null() { return; }
    let sig = Box::from_raw(handle);
    sig.handle.shutdown();
}

fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() { return String::new(); }
    unsafe { CStr::from_ptr(ptr).to_str().unwrap_or("").to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: construct a BoltPeer from PeerInfo and verify field values.
    /// Exercises the same code path as bolt_signaling_get_peer without needing
    /// a live signaling handle.
    fn make_bolt_peer(peer: &PeerInfo) -> *mut BoltPeer {
        let wt_url_ptr = peer.wt_url.as_ref()
            .and_then(|s| CString::new(s.as_str()).ok())
            .map(|cs| cs.into_raw())
            .unwrap_or(std::ptr::null_mut());
        let wt_hash_ptr = peer.wt_cert_hash.as_ref()
            .and_then(|s| CString::new(s.as_str()).ok())
            .map(|cs| cs.into_raw())
            .unwrap_or(std::ptr::null_mut());
        Box::into_raw(Box::new(BoltPeer {
            // EA9: mirror the production bolt_signaling_get_peer boundary handling
            // (null pointer on a NUL-bearing field) so the adversarial tests below
            // exercise the real fixed behavior instead of aborting in the harness.
            peer_code: CString::new(peer.peer_code.as_str())
                .map(|cs| cs.into_raw())
                .unwrap_or(std::ptr::null_mut()),
            device_name: CString::new(peer.device_name.as_str())
                .map(|cs| cs.into_raw())
                .unwrap_or(std::ptr::null_mut()),
            device_type: CString::new(peer.device_type.as_str())
                .map(|cs| cs.into_raw())
                .unwrap_or(std::ptr::null_mut()),
            wt_url: wt_url_ptr,
            wt_cert_hash: wt_hash_ptr,
        }))
    }

    #[test]
    fn bolt_peer_without_wt_has_null_pointers() {
        let peer = PeerInfo {
            peer_code: "BROWSER1".into(),
            device_name: "Chrome".into(),
            device_type: "browser".into(),
            wt_url: None,
            wt_cert_hash: None,
        };
        let ptr = make_bolt_peer(&peer);
        unsafe {
            let p = &*ptr;
            assert!(!p.peer_code.is_null());
            assert!(!p.device_name.is_null());
            assert!(p.wt_url.is_null(), "wt_url must be null for browser peer");
            assert!(p.wt_cert_hash.is_null(), "wt_cert_hash must be null for browser peer");
            bolt_peer_free(ptr);
        }
    }

    #[test]
    fn bolt_peer_with_wt_has_valid_pointers() {
        let peer = PeerInfo {
            peer_code: "NATIVE1".into(),
            device_name: "Mac Studio".into(),
            device_type: "desktop".into(),
            wt_url: Some("https://192.168.1.10:9871".into()),
            wt_cert_hash: Some("abcdef1234567890".into()),
        };
        let ptr = make_bolt_peer(&peer);
        unsafe {
            let p = &*ptr;
            assert!(!p.wt_url.is_null(), "wt_url must be non-null for WT peer");
            assert!(!p.wt_cert_hash.is_null(), "wt_cert_hash must be non-null for WT peer");
            let url = CStr::from_ptr(p.wt_url).to_str().unwrap();
            let hash = CStr::from_ptr(p.wt_cert_hash).to_str().unwrap();
            assert_eq!(url, "https://192.168.1.10:9871");
            assert_eq!(hash, "abcdef1234567890");
            bolt_peer_free(ptr);
        }
    }

    #[test]
    fn bolt_peer_repeated_construction_is_stable() {
        let peer = PeerInfo {
            peer_code: "REPEAT1".into(),
            device_name: "Test".into(),
            device_type: "browser".into(),
            wt_url: None,
            wt_cert_hash: None,
        };
        // Simulate 50 poll cycles — each constructs and frees a BoltPeer
        for _ in 0..50 {
            let ptr = make_bolt_peer(&peer);
            unsafe {
                let p = &*ptr;
                assert!(p.wt_url.is_null());
                assert!(p.wt_cert_hash.is_null());
                bolt_peer_free(ptr);
            }
        }
    }

    #[test]
    fn peer_info_deserializes_without_wt_fields() {
        let json = r#"{"peer_code":"X","device_name":"Phone","device_type":"phone"}"#;
        let peer: PeerInfo = serde_json::from_str(json).unwrap();
        assert_eq!(peer.wt_url, None);
        assert_eq!(peer.wt_cert_hash, None);
    }

    #[test]
    fn peer_info_deserializes_with_wt_fields() {
        let json = r#"{"peer_code":"Y","device_name":"Mac","device_type":"desktop","wt_url":"https://10.0.0.1:9871","wt_cert_hash":"deadbeef"}"#;
        let peer: PeerInfo = serde_json::from_str(json).unwrap();
        assert_eq!(peer.wt_url.as_deref(), Some("https://10.0.0.1:9871"));
        assert_eq!(peer.wt_cert_hash.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn bolt_peer_with_nul_in_device_name_yields_null_ptr_no_abort() {
        // EA9 adversarial: rendezvous is untrusted. Peer metadata carrying an interior
        // NUL byte must NOT abort the process at the FFI boundary. The offending field
        // maps to a null pointer (mirroring wt_url); the Swift host null-coalesces it to
        // "" (BoltBridge.swift get-peer loop). Clean fields still convert.
        let peer = PeerInfo {
            peer_code: "GOODCODE".into(),
            device_name: "Evil\0Name".into(),
            device_type: "browser".into(),
            wt_url: None,
            wt_cert_hash: None,
        };
        let ptr = make_bolt_peer(&peer);
        unsafe {
            let p = &*ptr;
            assert!(p.device_name.is_null(), "NUL device_name must map to null ptr");
            assert!(!p.peer_code.is_null(), "clean peer_code must convert");
            assert!(!p.device_type.is_null(), "clean device_type must convert");
            bolt_peer_free(ptr); // must not abort on the null field
        }
    }

    #[test]
    fn bolt_peer_with_nul_in_peer_code_yields_null_ptr() {
        // The identity field is equally attacker-controlled; a NUL there must be
        // contained, not abort the app.
        let peer = PeerInfo {
            peer_code: "BAD\0CODE".into(),
            device_name: "Phone".into(),
            device_type: "phone".into(),
            wt_url: None,
            wt_cert_hash: None,
        };
        let ptr = make_bolt_peer(&peer);
        unsafe {
            let p = &*ptr;
            assert!(p.peer_code.is_null(), "NUL peer_code must map to null ptr");
            assert!(!p.device_name.is_null(), "clean device_name must convert");
            bolt_peer_free(ptr);
        }
    }
}
