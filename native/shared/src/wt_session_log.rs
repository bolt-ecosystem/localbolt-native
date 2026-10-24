//! WebTransport session lifecycle as printed by bolt-daemon stderr.
//!
//! The browser path (Studio Chrome → Mac app) is the WebTransport answerer in
//! `wt_endpoint.rs`. Unlike the WebSocket answerer, that path does **not** emit
//! `session.connected` / `session.sas` on IPC. The Mac shell therefore watches
//! stderr. It used to look for the Rust static's name, `ACTIVE_SESSION registered`.
//! The daemon never prints that. It prints:
//!
//! ```text
//! [SAS] 5BE04E
//! [WT_SESSION] <peer> session established, entering shared session loop
//! [SESSION] <peer> active session handle registered
//! ```
//!
//! and, when the shared loop exits:
//!
//! ```text
//! [SESSION] <peer> active session handle cleared
//! ```
//!
//! `[WT_HELLO] … HELLO ok` is only the handshake. The UI must stay on
//! Establishing until one of the session lines above appears.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use serde_json::json;

/// Latest session lifecycle in one stderr tail.
///
/// `active` is `None` when this tail has no lifecycle line, so the caller can
/// keep a latched value after the line scrolls out of the window. `sas` is the
/// code paired with the latest establish line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WtSessionTail {
    pub active: Option<bool>,
    pub sas: Option<String>,
}

impl WtSessionTail {
    fn json(&self) -> String {
        json!({
            "active": self.active,
            "sas": self.sas,
        })
        .to_string()
    }
}

/// Parse a daemon stderr tail. Lines are visited in order; the latest lifecycle
/// line wins, so a stale `cleared` above a new session does not stick.
pub fn parse_wt_session_stderr(stderr: &str) -> WtSessionTail {
    let mut active: Option<bool> = None;
    let mut pending_sas: Option<String> = None;
    let mut established_sas: Option<String> = None;

    for line in stderr.lines() {
        if let Some(code) = sas_code(line) {
            pending_sas = Some(code.clone());
            // The code is normally printed just before the establish line. If a
            // tail has them in the other order, keep the code anyway.
            if active == Some(true) && established_sas.is_none() {
                established_sas = Some(code);
            }
        }
        if is_clear(line) {
            active = Some(false);
            pending_sas = None;
            established_sas = None;
        } else if is_establish(line) {
            active = Some(true);
            established_sas = pending_sas.clone();
        }
    }

    WtSessionTail {
        active,
        sas: if active == Some(true) {
            established_sas
        } else {
            None
        },
    }
}

fn is_establish(line: &str) -> bool {
    // Shared loop, every transport, including WebTransport. This is the line
    // the old shell meant by "ACTIVE_SESSION registered".
    line.contains("active session handle registered")
        || line.contains("ACTIVE_SESSION registered")
        // WebTransport answerer, logged immediately before the shared loop.
        // WS/QUIC say "session established" under a different tag and also emit
        // IPC; do not treat those as this latch by themselves.
        || (line.contains("[WT_SESSION]") && line.contains("session established"))
}

fn is_clear(line: &str) -> bool {
    line.contains("active session handle cleared") || line.contains("ACTIVE_SESSION cleared")
}

/// `[SAS] XXXXXX` — bolt SAS codes are 6 alphanumeric characters.
fn sas_code(line: &str) -> Option<String> {
    let rest = line.split_once("[SAS] ")?.1;
    let code: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    if code.len() == 6 {
        Some(code)
    } else {
        None
    }
}

/// JSON `{"active": true|false|null, "sas": "XXXXXX"|null}` for the Swift shell.
///
/// Null `active` means this tail has no lifecycle line. Caller frees a non-null
/// return with `bolt_free_string`.
///
/// # Safety
/// `stderr` must be null or a valid nul-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn bolt_parse_wt_session_stderr(stderr: *const c_char) -> *mut c_char {
    if stderr.is_null() {
        return std::ptr::null_mut();
    }
    let text = match CStr::from_ptr(stderr).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    CString::new(parse_wt_session_stderr(text).json())
        .map(|cs| cs.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    /// Oct 24, 2026 LAN capture: Studio Chrome → MacBook, after AllowAlways.
    /// The shell stayed on "Waiting for encrypted channel" because none of
    /// these lines contain `ACTIVE_SESSION registered`.
    fn browser_wt_established() -> &'static str {
        "\
[WT_HELLO] 192.168.4.25:55170 HELLO ok: identity=abc, caps=[\"bolt.transfer-ratchet-v1\"]
[SAS] 5BE04E
[WT_SESSION] 192.168.4.25:55170 session established, entering shared session loop
[BTR] 192.168.4.25:55170 engine initialized (generation=0)
[SESSION] 192.168.4.25:55170 active session handle registered
"
    }

    #[test]
    fn browser_wt_session_is_active_with_sas_without_the_old_needle() {
        let sample = browser_wt_established();
        assert!(
            !sample.contains("ACTIVE_SESSION registered"),
            "the daemon does not print the needle the shell used to require"
        );
        assert!(!sample.contains("ACTIVE_SESSION cleared"));

        let parsed = parse_wt_session_stderr(sample);
        assert_eq!(parsed.active, Some(true));
        assert_eq!(parsed.sas.as_deref(), Some("5BE04E"));
    }

    #[test]
    fn early_wt_logs_are_not_a_session() {
        // These are printed before pairing and before the shared loop. Treating
        // them as "up" would leave Establishing while the ask sheet is still open,
        // then stick on Connected if the user denies.
        let sample = "\
[WT_SESSION] 10.0.0.1:1 session request: authority=mac.local, path=/
[WT_SESSION] 10.0.0.1:1 session accepted
[WT_SESSION] 10.0.0.1:1 bidi stream opened
[WT_HELLO] 10.0.0.1:1 HELLO ok: identity=abc, caps=[]
[PAIRING_DENIED] WT_SESSION 10.0.0.1:1 policy denied identity=abc
";
        assert_eq!(parse_wt_session_stderr(sample).active, None);
    }

    #[test]
    fn hello_ok_alone_is_not_a_session() {
        let sample = "\
[WT_HELLO] 192.168.4.25:55170 HELLO ok: identity=abc, caps=[]
[SAS] 5BE04E
";
        let parsed = parse_wt_session_stderr(sample);
        assert_eq!(parsed.active, None);
        assert_eq!(parsed.sas, None);
    }

    #[test]
    fn wt_established_line_alone_is_enough() {
        let sample = "\
[SAS] 5BE04E
[WT_SESSION] 10.0.0.8:443 session established, entering shared session loop
";
        let parsed = parse_wt_session_stderr(sample);
        assert_eq!(parsed.active, Some(true));
        assert_eq!(parsed.sas.as_deref(), Some("5BE04E"));
    }

    #[test]
    fn shared_registered_line_alone_is_enough() {
        let sample = "[SESSION] 10.0.0.8:443 active session handle registered\n";
        let parsed = parse_wt_session_stderr(sample);
        assert_eq!(parsed.active, Some(true));
        assert_eq!(parsed.sas, None);
    }

    #[test]
    fn websocket_established_line_is_not_this_latch() {
        // WS emits session.connected on IPC and uses a different tag. Matching
        // it here would race the IPC path. The shared registered line, which
        // follows it, is still a real session.
        let only_ws = "[WS_SESSION] 10.0.0.8:9 session established, entering message loop\n";
        assert_eq!(parse_wt_session_stderr(only_ws).active, None);

        let with_register = "\
[WS_SESSION] 10.0.0.8:9 session established, entering message loop
[SESSION] 10.0.0.8:9 active session handle registered
";
        assert_eq!(parse_wt_session_stderr(with_register).active, Some(true));
    }

    #[test]
    fn clear_after_establish_ends_the_session() {
        let sample = format!(
            "{}\
[SESSION] 192.168.4.25:55170 message loop ended
[SESSION] 192.168.4.25:55170 active session handle cleared
",
            browser_wt_established()
        );
        let parsed = parse_wt_session_stderr(&sample);
        assert_eq!(parsed.active, Some(false));
        assert_eq!(parsed.sas, None);
    }

    #[test]
    fn latest_establish_wins_over_a_stale_clear() {
        let sample = "\
[SESSION] 10.0.0.1:1 active session handle cleared
[SAS] AABBCC
[WT_SESSION] 10.0.0.2:2 session established, entering shared session loop
[SESSION] 10.0.0.2:2 active session handle registered
";
        let parsed = parse_wt_session_stderr(sample);
        assert_eq!(parsed.active, Some(true));
        assert_eq!(parsed.sas.as_deref(), Some("AABBCC"));
    }

    #[test]
    fn historical_active_session_token_still_matches() {
        let sample = "\
[SAS] 00FF11
[WT_SESSION] ACTIVE_SESSION registered
";
        // The second line also contains [WT_SESSION] but not "session established".
        // The historical token must still latch.
        let parsed = parse_wt_session_stderr(sample);
        assert_eq!(parsed.active, Some(true));
        assert_eq!(parsed.sas.as_deref(), Some("00FF11"));

        let cleared = "[SESSION] ACTIVE_SESSION cleared\n";
        assert_eq!(parse_wt_session_stderr(cleared).active, Some(false));
    }

    #[test]
    fn sas_after_establish_is_kept() {
        let sample = "\
[WT_SESSION] 10.0.0.2:2 session established, entering shared session loop
[SAS] 5BE04E
";
        let parsed = parse_wt_session_stderr(sample);
        assert_eq!(parsed.active, Some(true));
        assert_eq!(parsed.sas.as_deref(), Some("5BE04E"));
    }

    #[test]
    fn empty_tail_has_no_lifecycle() {
        let parsed = parse_wt_session_stderr("");
        assert_eq!(parsed.active, None);
        assert_eq!(parsed.sas, None);
    }

    #[test]
    fn ffi_json_for_browser_wt_session() {
        let raw = CString::new(browser_wt_established()).unwrap();
        let ptr = unsafe { bolt_parse_wt_session_stderr(raw.as_ptr()) };
        assert!(!ptr.is_null());
        let json = unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() };
        unsafe { crate::bolt_free_string(ptr) };
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["active"], true);
        assert_eq!(value["sas"], "5BE04E");
    }

    #[test]
    fn ffi_null_input_returns_null() {
        let ptr = unsafe { bolt_parse_wt_session_stderr(std::ptr::null()) };
        assert!(ptr.is_null());
    }
}
