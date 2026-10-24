//! Which daemon endpoints and peer names are safe to show or advertise.
//!
//! `wt_info.json` and `quic_info.json` live in the persistent daemon data directory.
//! The native shell used to read whichever file was already on disk at startup and
//! then keep that snapshot. The daemon's WS port is `9100 + (app_pid % 900)` and
//! WebTransport listens on WS+1, so a leftover file from the previous launch points
//! browsers at a port nothing is bound to. Browser peers (localbolt.app) dial the
//! `wtUrl` / `certHash` in `connection_request` and `connection_accepted`, and an
//! HTTPS page cannot fall back to `ws://`, so that stale port hangs the transfer.
//!
//! A record is live only when its bytes differ from the pre-spawn snapshot (or the
//! file did not exist yet) AND the port is the one this process asked the daemon to
//! bind. Peer display names follow the same rule in the other direction: a bind
//! address such as `0.0.0.0` (what `Host.localizedName` returns on some Macs) is
//! never shown when a real device name is available. A socket label
//! (`192.168.4.25:55170`, `0.0.0.0:0`, `::`) is the same kind of placeholder:
//! the daemon's pairing IPC puts `peer_addr` in `remote_device_name`, and the
//! Mac sheet must keep the name signaling already advertised.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointAdvertisement {
    pub port: u16,
    pub cert_hash: String,
    pub url: String,
}

/// True when `current_json` is a non-empty document written after `baseline_json`.
///
/// Identical bytes are the previous launch's file. A missing baseline means the
/// file was created by this launch.
pub fn metadata_is_current(current_json: Option<&str>, baseline_json: Option<&str>) -> bool {
    let Some(current) = nonempty(current_json) else {
        return false;
    };
    match nonempty(baseline_json) {
        None => true,
        Some(baseline) => current != baseline,
    }
}

pub fn select_wt_advertisement(
    current_json: Option<&str>,
    baseline_json: Option<&str>,
    ws_port: u16,
    host: &str,
) -> Option<EndpointAdvertisement> {
    select_endpoint(EndpointQuery {
        current_json,
        baseline_json,
        ws_port,
        port_offset: 1,
        port_key: "wt_port",
        hash_key: "wt_cert_hash",
        host,
        https: true,
    })
}

pub fn select_quic_advertisement(
    current_json: Option<&str>,
    baseline_json: Option<&str>,
    ws_port: u16,
    host: &str,
) -> Option<EndpointAdvertisement> {
    select_endpoint(EndpointQuery {
        current_json,
        baseline_json,
        ws_port,
        port_offset: 2,
        port_key: "quic_port",
        hash_key: "quic_cert_hash",
        host,
        https: false,
    })
}

struct EndpointQuery<'a> {
    current_json: Option<&'a str>,
    baseline_json: Option<&'a str>,
    ws_port: u16,
    port_offset: u16,
    port_key: &'a str,
    hash_key: &'a str,
    host: &'a str,
    https: bool,
}

fn select_endpoint(query: EndpointQuery<'_>) -> Option<EndpointAdvertisement> {
    if !metadata_is_current(query.current_json, query.baseline_json) {
        return None;
    }
    let expected = query.ws_port.checked_add(query.port_offset)?;
    let value: Value = serde_json::from_str(query.current_json?.trim()).ok()?;
    let port = json_port(value.get(query.port_key)?)?;
    if port != expected {
        return None;
    }
    let hash = value.get(query.hash_key)?.as_str()?.trim();
    if !is_cert_hash(hash) {
        return None;
    }
    let url = endpoint_url(query.host, port, query.https)?;
    Some(EndpointAdvertisement {
        port,
        cert_hash: hash.to_string(),
        url,
    })
}

fn json_port(value: &Value) -> Option<u16> {
    if let Some(n) = value.as_u64() {
        return u16::try_from(n).ok();
    }
    if let Some(n) = value.as_i64() {
        return u16::try_from(n).ok();
    }
    if let Some(s) = value.as_str() {
        return s.trim().parse().ok();
    }
    None
}

fn is_cert_hash(hash: &str) -> bool {
    (32..=128).contains(&hash.len()) && hash.chars().all(|c| c.is_ascii_hexdigit())
}

fn endpoint_url(host: &str, port: u16, https: bool) -> Option<String> {
    let host = host.trim();
    if !host_is_usable(host) {
        return None;
    }
    if https {
        Some(format!("https://{host}:{port}"))
    } else {
        Some(format!("{host}:{port}"))
    }
}

pub fn host_is_usable(host: &str) -> bool {
    let host = host.trim();
    !host.is_empty() && !is_bind_address(host)
}

pub fn is_bind_address(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "0.0.0.0" | "::" | "::0" | "https://example.org/b/zenith" | "[::]" | "[::0]" | "*"
    )
}

/// Empty names, bind addresses, and socket labels are not human device names.
///
/// `0.0.0.0` is the wildcard `Host.localizedName` returns on some Macs.
/// `0.0.0.0:0` and `192.168.4.25:55170` are what bolt-daemon writes into
/// `remote_device_name` (a synthetic WS address, or `peer_addr`). A LAN address
/// is still a usable WebTransport host — that check is [`host_is_usable`].
pub fn device_name_is_unusable(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() || is_bind_address(trimmed) || looks_like_ip_or_socket(trimmed) {
        return true;
    }
    if let Some(host) = host_of_endpoint_shaped_name(trimmed) {
        return is_bind_address(host) || looks_like_ip_or_socket(host);
    }
    false
}

/// True for an IP address or `ip:port`, including bracketed IPv6.
fn looks_like_ip_or_socket(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.parse::<std::net::SocketAddr>().is_ok()
        || trimmed.parse::<std::net::IpAddr>().is_ok()
    {
        return true;
    }
    if let Some(inner) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return inner.parse::<std::net::IpAddr>().is_ok() || is_bind_address(inner);
    }
    false
}

fn host_of_endpoint_shaped_name(name: &str) -> Option<&str> {
    let without_scheme = name.split_once("://").map(|(_, rest)| rest).unwrap_or(name);
    let hostport = without_scheme.split(['/', '?', '#']).next()?.trim();
    if hostport.is_empty() {
        return None;
    }
    if let Some(rest) = hostport.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        if host.is_empty() {
            return None;
        }
        if after.is_empty() {
            return Some(host);
        }
        let port = after.strip_prefix(':')?;
        if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        return Some(host);
    }
    let (host, port) = hostport.rsplit_once(':')?;
    if host.is_empty() || port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // A bare IPv6 literal has more than one colon and no brackets.
    if host.contains(':') {
        return None;
    }
    Some(host)
}

/// Daemon pairing IPC sends `remote_device_type: "unknown"`. That is not a
/// type the peer advertised.
pub fn device_type_is_placeholder(type_name: &str) -> bool {
    matches!(
        type_name.trim().to_ascii_lowercase().as_str(),
        "" | "unknown"
    )
}

/// Keep a real current type. A placeholder yields a candidate the peer advertised.
pub fn prefer_device_type<'a>(current: &'a str, candidate: &'a str) -> &'a str {
    if device_type_is_placeholder(current) && !device_type_is_placeholder(candidate) {
        candidate.trim()
    } else {
        current
    }
}

/// Type label for the ask sheet. Placeholders become empty so the UI can omit them.
pub fn display_device_type(raw: &str) -> String {
    let trimmed = raw.trim();
    if device_type_is_placeholder(trimmed) {
        String::new()
    } else {
        trimmed.to_string()
    }
}

/// Keep a usable current name. An unusable current name yields a usable candidate.
pub fn prefer_device_name<'a>(current: &'a str, candidate: &'a str) -> &'a str {
    if device_name_is_unusable(current) && !device_name_is_unusable(candidate) {
        candidate.trim()
    } else {
        current
    }
}

pub fn display_device_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if device_name_is_unusable(trimmed) {
        "Mac".to_string()
    } else {
        trimmed.to_string()
    }
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|s| !s.is_empty())
}

fn advertisement_json(ad: &EndpointAdvertisement) -> String {
    serde_json::json!({
        "url": ad.url,
        "certHash": ad.cert_hash,
        "port": ad.port,
    })
    .to_string()
}

fn cstr_owned(ptr: *const std::os::raw::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

fn owned_c_string(value: String) -> *mut std::os::raw::c_char {
    std::ffi::CString::new(value)
        .map(|cs| cs.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// JSON `{"url","certHash","port"}` for the live WT listener, or null.
///
/// # Safety
/// String pointers must be null or valid nul-terminated UTF-8. The caller frees
/// a non-null return with `bolt_free_string`.
#[no_mangle]
pub unsafe extern "C" fn bolt_select_wt_advertisement(
    current_json: *const std::os::raw::c_char,
    baseline_json: *const std::os::raw::c_char,
    ws_port: u16,
    host: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    let current_owned = cstr_owned(current_json);
    let baseline_owned = cstr_owned(baseline_json);
    let host_owned = cstr_owned(host);
    let current = nonempty(Some(current_owned.as_str()));
    let baseline = nonempty(Some(baseline_owned.as_str()));
    match select_wt_advertisement(current, baseline, ws_port, host_owned.as_str()) {
        Some(ad) => owned_c_string(advertisement_json(&ad)),
        None => std::ptr::null_mut(),
    }
}

/// JSON `{"url","certHash","port"}` for the live QUIC listener, or null.
/// `url` is `host:port` (no scheme), matching the native `quicAddr` field.
///
/// # Safety
/// String pointers must be null or valid nul-terminated UTF-8. The caller frees
/// a non-null return with `bolt_free_string`.
#[no_mangle]
pub unsafe extern "C" fn bolt_select_quic_advertisement(
    current_json: *const std::os::raw::c_char,
    baseline_json: *const std::os::raw::c_char,
    ws_port: u16,
    host: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    let current_owned = cstr_owned(current_json);
    let baseline_owned = cstr_owned(baseline_json);
    let host_owned = cstr_owned(host);
    let current = nonempty(Some(current_owned.as_str()));
    let baseline = nonempty(Some(baseline_owned.as_str()));
    match select_quic_advertisement(current, baseline, ws_port, host_owned.as_str()) {
        Some(ad) => owned_c_string(advertisement_json(&ad)),
        None => std::ptr::null_mut(),
    }
}

/// 1 when `name` must not be shown as a device name.
///
/// # Safety
/// `name` must be null or a valid nul-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn bolt_device_name_is_unusable(name: *const std::os::raw::c_char) -> i32 {
    if device_name_is_unusable(&cstr_owned(name)) {
        1
    } else {
        0
    }
}

/// Device name to display. Socket labels, bind addresses, and blanks become `"Mac"`.
///
/// # Safety
/// `raw` must be null or a valid nul-terminated UTF-8 string. Caller frees the
/// result with `bolt_free_string`.
#[no_mangle]
pub unsafe extern "C" fn bolt_device_display_name(
    raw: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    owned_c_string(display_device_name(&cstr_owned(raw)))
}

/// Usable `current` wins. An unusable `current` is replaced by a usable `candidate`.
///
/// # Safety
/// Pointers must be null or valid nul-terminated UTF-8. Caller frees the result
/// with `bolt_free_string`.
#[no_mangle]
pub unsafe extern "C" fn bolt_prefer_device_name(
    current: *const std::os::raw::c_char,
    candidate: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    let current = cstr_owned(current);
    let candidate = cstr_owned(candidate);
    owned_c_string(prefer_device_name(&current, &candidate).to_string())
}

/// 1 when `host` may be advertised in a WebTransport or QUIC URL.
/// A LAN address is usable here even though it is not a device name.
///
/// # Safety
/// `host` must be null or a valid nul-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn bolt_host_is_usable(host: *const std::os::raw::c_char) -> i32 {
    if host_is_usable(&cstr_owned(host)) {
        1
    } else {
        0
    }
}

/// 1 when `type_name` is blank or the daemon placeholder `"unknown"`.
///
/// # Safety
/// `type_name` must be null or a valid nul-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn bolt_device_type_is_placeholder(
    type_name: *const std::os::raw::c_char,
) -> i32 {
    if device_type_is_placeholder(&cstr_owned(type_name)) {
        1
    } else {
        0
    }
}

/// Keep a real `current` type. Replace `"unknown"` with an advertised `candidate`.
///
/// # Safety
/// Pointers must be null or valid nul-terminated UTF-8. Caller frees the result
/// with `bolt_free_string`.
#[no_mangle]
pub unsafe extern "C" fn bolt_prefer_device_type(
    current: *const std::os::raw::c_char,
    candidate: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    let current = cstr_owned(current);
    let candidate = cstr_owned(candidate);
    owned_c_string(prefer_device_type(&current, &candidate).to_string())
}

/// Type to show. `"unknown"` and blanks become an empty string.
///
/// # Safety
/// `raw` must be null or a valid nul-terminated UTF-8 string. Caller frees the
/// result with `bolt_free_string`.
#[no_mangle]
pub unsafe extern "C" fn bolt_device_display_type(
    raw: *const std::os::raw::c_char,
) -> *mut std::os::raw::c_char {
    owned_c_string(display_device_type(&cstr_owned(raw)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    fn wt(port: u16, hash: &str) -> String {
        format!(r#"{{"wt_port":{port},"wt_cert_hash":"{hash}"}}"#)
    }

    fn quic(port: u16, hash: &str) -> String {
        format!(r#"{{"quic_port":{port},"quic_cert_hash":"{hash}"}}"#)
    }

    #[test]
    fn leftover_wt_file_is_not_advertised() {
        // Same port as this launch (pid collision) but the cert bytes never changed.
        let stale = wt(9101, HASH);
        assert!(select_wt_advertisement(Some(&stale), Some(&stale), 9100, "10.0.0.5").is_none());
    }

    #[test]
    fn rewritten_wt_file_matching_ws_plus_one_is_advertised() {
        let baseline = wt(9001, OTHER);
        let live = wt(9101, HASH);
        let ad = select_wt_advertisement(Some(&live), Some(&baseline), 9100, "10.0.0.5").unwrap();
        assert_eq!(ad.port, 9101);
        assert_eq!(ad.cert_hash, HASH);
        assert_eq!(ad.url, "https://10.0.0.5:9101");
    }

    #[test]
    fn fresh_file_with_the_wrong_port_is_rejected() {
        let baseline = wt(9001, OTHER);
        let live = wt(9999, HASH);
        assert!(select_wt_advertisement(Some(&live), Some(&baseline), 9100, "10.0.0.5").is_none());
    }

    #[test]
    fn file_created_this_launch_is_advertised() {
        let live = wt(9101, HASH);
        let ad = select_wt_advertisement(Some(&live), None, 9100, "192.168.1.20").unwrap();
        assert_eq!(ad.url, "https://192.168.1.20:9101");
    }

    #[test]
    fn missing_or_invalid_wt_file_is_not_advertised() {
        assert!(select_wt_advertisement(None, None, 9100, "10.0.0.5").is_none());
        assert!(select_wt_advertisement(Some(""), Some(""), 9100, "10.0.0.5").is_none());
        assert!(select_wt_advertisement(Some("{"), None, 9100, "10.0.0.5").is_none());
        assert!(select_wt_advertisement(Some(&wt(9101, "zz")), None, 9100, "10.0.0.5").is_none());
        assert!(select_wt_advertisement(Some(&wt(9101, "abc")), None, 9100, "10.0.0.5").is_none());
    }

    #[test]
    fn bind_address_host_is_not_advertised() {
        let live = wt(9101, HASH);
        assert!(select_wt_advertisement(Some(&live), None, 9100, "0.0.0.0").is_none());
        assert!(select_wt_advertisement(Some(&live), None, 9100, "::").is_none());
        assert!(select_wt_advertisement(Some(&live), None, 9100, "  ").is_none());
    }

    #[test]
    fn quic_uses_ws_plus_two_and_has_no_scheme() {
        let baseline = quic(1, OTHER);
        let live = quic(9102, HASH);
        let ad = select_quic_advertisement(Some(&live), Some(&baseline), 9100, "10.0.0.8").unwrap();
        assert_eq!(ad.url, "10.0.0.8:9102");
        assert_eq!(ad.cert_hash, HASH);
        assert!(select_quic_advertisement(Some(&live), Some(&live), 9100, "10.0.0.8").is_none());
    }

    #[test]
    fn port_string_and_overflow_are_handled() {
        let live = format!(r#"{{"wt_port":"9101","wt_cert_hash":"{HASH}"}}"#);
        assert!(select_wt_advertisement(Some(&live), None, 9100, "10.0.0.5").is_some());
        assert!(select_wt_advertisement(Some(&wt(0, HASH)), None, u16::MAX, "10.0.0.5").is_none());
    }

    #[test]
    fn whitespace_only_change_does_not_count_as_rewritten() {
        let pretty = format!("\n  {{\"wt_port\":9101,\"wt_cert_hash\":\"{HASH}\"}}  \n");
        let compact = wt(9101, HASH);
        assert!(!metadata_is_current(Some(&pretty), Some(&compact)));
    }

    #[test]
    fn bind_address_device_name_loses_to_a_real_name() {
        assert!(device_name_is_unusable("0.0.0.0"));
        assert!(device_name_is_unusable("  ::  "));
        assert!(device_name_is_unusable(""));
        assert!(device_name_is_unusable("https://0.0.0.0:9101"));
        assert!(device_name_is_unusable("0.0.0.0:4433"));
        assert!(!device_name_is_unusable("Evan's Mac Studio"));
        assert!(!device_name_is_unusable("Mac"));
        assert_eq!(
            prefer_device_name("0.0.0.0", "Evan's Mac Studio"),
            "Evan's Mac Studio"
        );
        assert_eq!(
            prefer_device_name("Evan's Mac Studio", "0.0.0.0"),
            "Evan's Mac Studio"
        );
        assert_eq!(prefer_device_name("", "Mac Studio"), "Mac Studio");
        assert_eq!(
            prefer_device_name("MacBook Pro", "Mac Studio"),
            "MacBook Pro"
        );
        assert_eq!(display_device_name("0.0.0.0"), "Mac");
        assert_eq!(display_device_name("  "), "Mac");
        assert_eq!(display_device_name("  Mac Studio  "), "Mac Studio");
    }

    #[test]
    fn socket_address_device_name_loses_to_the_advertised_name() {
        assert!(device_name_is_unusable("0.0.0.0:0"));
        assert!(device_name_is_unusable("192.168.4.25:55170"));
        assert!(device_name_is_unusable("192.168.4.25"));
        assert!(device_name_is_unusable("::"));
        assert!(device_name_is_unusable("::1"));
        assert!(device_name_is_unusable("[::1]:55170"));
        assert!(device_name_is_unusable("https://192.168.4.25:9101"));
        assert!(device_name_is_unusable("https://[::1]:9101"));
        assert!(!device_name_is_unusable("Evan's MacBook Pro"));
        assert!(!device_name_is_unusable("Evan\u{2019}s MacBook Pro"));
        assert!(!device_name_is_unusable("Mac"));
        assert_eq!(
            prefer_device_name("192.168.4.25:55170", "Evan\u{2019}s MacBook Pro"),
            "Evan\u{2019}s MacBook Pro"
        );
        assert_eq!(
            prefer_device_name("0.0.0.0:0", "Evan's MacBook Pro"),
            "Evan's MacBook Pro"
        );
        assert_eq!(
            prefer_device_name("Evan's MacBook Pro", "192.168.4.25:55170"),
            "Evan's MacBook Pro"
        );
        assert_eq!(display_device_name("192.168.4.25:55170"), "Mac");
        assert_eq!(display_device_name("0.0.0.0:0"), "Mac");
        // A LAN address is a bad device name and a good WebTransport host.
        assert!(host_is_usable("192.168.4.25"));
        assert!(host_is_usable("10.0.0.1"));
        assert!(!host_is_usable("0.0.0.0"));
        assert!(!host_is_usable("::"));
    }

    #[test]
    fn unknown_device_type_loses_to_the_advertised_type() {
        assert!(device_type_is_placeholder("unknown"));
        assert!(device_type_is_placeholder("  "));
        assert!(!device_type_is_placeholder("desktop"));
        assert!(!device_type_is_placeholder("laptop"));
        assert_eq!(prefer_device_type("unknown", "laptop"), "laptop");
        assert_eq!(prefer_device_type("desktop", "unknown"), "desktop");
        assert_eq!(prefer_device_type("unknown", "unknown"), "unknown");
        assert_eq!(display_device_type("unknown"), "");
        assert_eq!(display_device_type(" desktop "), "desktop");
    }

    #[test]
    fn ffi_select_wt_rejects_stale_and_returns_live_json() {
        let stale = wt(9101, HASH);
        let live = wt(9101, OTHER);
        unsafe {
            assert!(bolt_select_wt_advertisement(
                std::ffi::CString::new(stale.as_str()).unwrap().as_ptr(),
                std::ffi::CString::new(stale.as_str()).unwrap().as_ptr(),
                9100,
                std::ffi::CString::new("10.0.0.5").unwrap().as_ptr(),
            )
            .is_null());

            let current = std::ffi::CString::new(live.as_str()).unwrap();
            let baseline = std::ffi::CString::new(stale.as_str()).unwrap();
            let host = std::ffi::CString::new("10.0.0.5").unwrap();
            let ptr = bolt_select_wt_advertisement(
                current.as_ptr(),
                baseline.as_ptr(),
                9100,
                host.as_ptr(),
            );
            assert!(!ptr.is_null());
            let json = std::ffi::CStr::from_ptr(ptr).to_str().unwrap();
            assert!(json.contains("https://10.0.0.5:9101"));
            assert!(json.contains(OTHER));
            crate::bolt_free_string(ptr);
        }
    }

    #[test]
    fn ffi_display_name_never_returns_bind_address() {
        unsafe {
            let raw = std::ffi::CString::new("0.0.0.0").unwrap();
            let ptr = bolt_device_display_name(raw.as_ptr());
            let shown = std::ffi::CStr::from_ptr(ptr).to_str().unwrap().to_string();
            crate::bolt_free_string(ptr);
            assert_eq!(shown, "Mac");

            let current = std::ffi::CString::new("0.0.0.0").unwrap();
            let candidate = std::ffi::CString::new("Mac Studio").unwrap();
            let preferred = bolt_prefer_device_name(current.as_ptr(), candidate.as_ptr());
            let name = std::ffi::CStr::from_ptr(preferred)
                .to_str()
                .unwrap()
                .to_string();
            crate::bolt_free_string(preferred);
            assert_eq!(name, "Mac Studio");
            assert_eq!(bolt_device_name_is_unusable(raw.as_ptr()), 1);
        }
    }
}
