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
///
/// # Safety
/// All string parameters must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn bolt_signaling_start(
    local_url: *const c_char,
    cloud_url: *const c_char,
    peer_code: *const c_char,
    device_name: *const c_char,
) -> *mut BoltSignaling {
    let local = cstr_to_string(local_url);
    let cloud = if cloud_url.is_null() { None } else { Some(cstr_to_string(cloud_url)) };
    let code = cstr_to_string(peer_code);
    let name = cstr_to_string(device_name);

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
    Box::into_raw(Box::new(BoltPeer {
        peer_code: CString::new(peer.peer_code.as_str()).unwrap().into_raw(),
        device_name: CString::new(peer.device_name.as_str()).unwrap().into_raw(),
        device_type: CString::new(peer.device_type.as_str()).unwrap().into_raw(),
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
