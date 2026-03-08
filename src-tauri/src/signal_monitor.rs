//! Signal server health monitor (N8 — observability only).
//!
//! Probes the embedded signal server via TCP connect to 127.0.0.1:3001.
//! Emits state transitions to the frontend. No transfer gating changes.
//! Shutdown-aware: suppresses probe transitions during app exit.

use serde::Serialize;
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::Emitter;

/// Signal health states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalStatus {
    Unknown,
    Active,
    Degraded,
    Offline,
}

impl std::fmt::Display for SignalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Active => write!(f, "active"),
            Self::Degraded => write!(f, "degraded"),
            Self::Offline => write!(f, "offline"),
        }
    }
}

/// Probe interval between health checks.
const PROBE_INTERVAL: Duration = Duration::from_secs(5);

/// TCP connect timeout per probe attempt.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Consecutive failures before transitioning to offline.
const OFFLINE_THRESHOLD: u32 = 3;

/// Signal server address (matches lib.rs embedded server bind).
const SIGNAL_ADDR: &str = "127.0.0.1:3001";

/// Event name emitted to frontend on status transitions.
const EVENT_NAME: &str = "signal://status";

/// Payload emitted to frontend on signal status change.
#[derive(Serialize, Clone)]
pub struct SignalStatusEvent {
    pub status: SignalStatus,
    pub consecutive_failures: u32,
}

/// Signal health monitor state machine.
pub struct SignalMonitor {
    status: SignalStatus,
    consecutive_failures: u32,
    shutdown_flag: Arc<AtomicBool>,
}

impl SignalMonitor {
    pub fn new(shutdown_flag: Arc<AtomicBool>) -> Self {
        Self {
            status: SignalStatus::Unknown,
            consecutive_failures: 0,
            shutdown_flag,
        }
    }

    #[cfg(test)]
    pub fn status(&self) -> SignalStatus {
        self.status
    }

    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Process a probe result. Returns Some(new_status) if a transition occurred.
    pub fn on_probe_result(&mut self, success: bool) -> Option<SignalStatus> {
        if self.shutdown_flag.load(Ordering::Relaxed) {
            if self.status != SignalStatus::Unknown {
                self.status = SignalStatus::Unknown;
                self.consecutive_failures = 0;
                tracing::info!("[SIGNAL_PROBE_SUPPRESSED] shutdown in progress");
                return Some(SignalStatus::Unknown);
            }
            return None;
        }

        if success {
            self.consecutive_failures = 0;
            let old = self.status;
            if old != SignalStatus::Active {
                self.status = SignalStatus::Active;
                match old {
                    SignalStatus::Offline => {
                        tracing::info!("[SIGNAL_RECOVERED] offline -> active");
                    }
                    _ => {
                        tracing::info!("[SIGNAL_PROBE_OK] {old} -> active");
                    }
                }
                return Some(SignalStatus::Active);
            }
            None
        } else {
            self.consecutive_failures += 1;
            let old = self.status;

            if self.consecutive_failures >= OFFLINE_THRESHOLD {
                if old != SignalStatus::Offline {
                    self.status = SignalStatus::Offline;
                    tracing::warn!(
                        "[SIGNAL_OFFLINE] {} consecutive failures, {old} -> offline",
                        self.consecutive_failures
                    );
                    return Some(SignalStatus::Offline);
                }
                None
            } else if old == SignalStatus::Active || old == SignalStatus::Unknown {
                self.status = SignalStatus::Degraded;
                tracing::warn!("[SIGNAL_PROBE_FAIL] {old} -> degraded");
                Some(SignalStatus::Degraded)
            } else {
                None
            }
        }
    }
}

/// Execute a single TCP connect probe against the signal server.
pub fn probe_signal_health() -> bool {
    let addr: SocketAddr = SIGNAL_ADDR.parse().expect("invalid signal addr constant");
    TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).is_ok()
}

/// Spawn the signal monitor loop on a background thread.
///
/// Probes every 5 seconds, emitting `signal://status` events on transitions.
pub fn start_signal_monitor(app_handle: tauri::AppHandle, shutdown_flag: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut monitor = SignalMonitor::new(shutdown_flag.clone());

        // Brief initial delay to let the signal server bind.
        std::thread::sleep(Duration::from_secs(2));

        loop {
            if shutdown_flag.load(Ordering::Relaxed) {
                // Final transition to unknown on shutdown
                if let Some(new_status) = monitor.on_probe_result(false) {
                    let _ = app_handle.emit(
                        EVENT_NAME,
                        SignalStatusEvent {
                            status: new_status,
                            consecutive_failures: monitor.consecutive_failures(),
                        },
                    );
                }
                tracing::info!("[SIGNAL_MONITOR] exiting — shutdown");
                return;
            }

            let success = probe_signal_health();
            if let Some(new_status) = monitor.on_probe_result(success) {
                let _ = app_handle.emit(
                    EVENT_NAME,
                    SignalStatusEvent {
                        status: new_status,
                        consecutive_failures: monitor.consecutive_failures(),
                    },
                );
            }

            std::thread::sleep(PROBE_INTERVAL);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_monitor() -> SignalMonitor {
        SignalMonitor::new(Arc::new(AtomicBool::new(false)))
    }

    #[test]
    fn initial_state_is_unknown() {
        let m = make_monitor();
        assert_eq!(m.status(), SignalStatus::Unknown);
        assert_eq!(m.consecutive_failures(), 0);
    }

    #[test]
    fn unknown_to_active_on_success() {
        let mut m = make_monitor();
        let t = m.on_probe_result(true);
        assert_eq!(t, Some(SignalStatus::Active));
        assert_eq!(m.status(), SignalStatus::Active);
        assert_eq!(m.consecutive_failures(), 0);
    }

    #[test]
    fn active_stays_active_on_success() {
        let mut m = make_monitor();
        m.on_probe_result(true);
        let t = m.on_probe_result(true);
        assert_eq!(t, None);
        assert_eq!(m.status(), SignalStatus::Active);
    }

    #[test]
    fn active_to_degraded_on_single_failure() {
        let mut m = make_monitor();
        m.on_probe_result(true);
        let t = m.on_probe_result(false);
        assert_eq!(t, Some(SignalStatus::Degraded));
        assert_eq!(m.status(), SignalStatus::Degraded);
        assert_eq!(m.consecutive_failures(), 1);
    }

    #[test]
    fn degraded_to_offline_on_three_failures() {
        let mut m = make_monitor();
        m.on_probe_result(true); // active
        m.on_probe_result(false); // degraded (1)
        m.on_probe_result(false); // still degraded (2)
        let t = m.on_probe_result(false); // offline (3)
        assert_eq!(t, Some(SignalStatus::Offline));
        assert_eq!(m.status(), SignalStatus::Offline);
        assert_eq!(m.consecutive_failures(), 3);
    }

    #[test]
    fn offline_stays_offline_on_more_failures() {
        let mut m = make_monitor();
        m.on_probe_result(true);
        for _ in 0..3 {
            m.on_probe_result(false);
        }
        assert_eq!(m.status(), SignalStatus::Offline);
        let t = m.on_probe_result(false);
        assert_eq!(t, None); // no transition
        assert_eq!(m.consecutive_failures(), 4);
    }

    #[test]
    fn offline_to_active_on_success() {
        let mut m = make_monitor();
        m.on_probe_result(true);
        for _ in 0..3 {
            m.on_probe_result(false);
        }
        assert_eq!(m.status(), SignalStatus::Offline);
        let t = m.on_probe_result(true);
        assert_eq!(t, Some(SignalStatus::Active));
        assert_eq!(m.status(), SignalStatus::Active);
        assert_eq!(m.consecutive_failures(), 0);
    }

    #[test]
    fn degraded_to_active_on_success() {
        let mut m = make_monitor();
        m.on_probe_result(true); // active
        m.on_probe_result(false); // degraded
        let t = m.on_probe_result(true);
        assert_eq!(t, Some(SignalStatus::Active));
        assert_eq!(m.status(), SignalStatus::Active);
    }

    #[test]
    fn shutdown_suppresses_transitions() {
        let flag = Arc::new(AtomicBool::new(false));
        let mut m = SignalMonitor::new(flag.clone());
        m.on_probe_result(true); // active

        flag.store(true, Ordering::Relaxed);
        let t = m.on_probe_result(false);
        // Should transition to unknown (suppressed), not degraded
        assert_eq!(t, Some(SignalStatus::Unknown));
        assert_eq!(m.status(), SignalStatus::Unknown);
    }

    #[test]
    fn shutdown_no_transition_if_already_unknown() {
        let flag = Arc::new(AtomicBool::new(true));
        let mut m = SignalMonitor::new(flag);
        // Already unknown + shutdown = no transition
        let t = m.on_probe_result(false);
        assert_eq!(t, None);
    }

    #[test]
    fn unknown_to_degraded_on_failure() {
        let mut m = make_monitor();
        let t = m.on_probe_result(false);
        assert_eq!(t, Some(SignalStatus::Degraded));
        assert_eq!(m.status(), SignalStatus::Degraded);
    }

    #[test]
    fn display_impl() {
        assert_eq!(SignalStatus::Unknown.to_string(), "unknown");
        assert_eq!(SignalStatus::Active.to_string(), "active");
        assert_eq!(SignalStatus::Degraded.to_string(), "degraded");
        assert_eq!(SignalStatus::Offline.to_string(), "offline");
    }

    #[test]
    fn serialize_snake_case() {
        let json = serde_json::to_string(&SignalStatus::Active).unwrap();
        assert_eq!(json, "\"active\"");
        let json = serde_json::to_string(&SignalStatus::Unknown).unwrap();
        assert_eq!(json, "\"unknown\"");
    }

    #[test]
    fn status_event_serializes() {
        let evt = SignalStatusEvent {
            status: SignalStatus::Degraded,
            consecutive_failures: 2,
        };
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("\"degraded\""));
        assert!(json.contains("\"consecutive_failures\":2"));
    }

    #[test]
    fn failure_counter_resets_on_success() {
        let mut m = make_monitor();
        m.on_probe_result(true);
        m.on_probe_result(false); // 1
        m.on_probe_result(false); // 2
        assert_eq!(m.consecutive_failures(), 2);
        m.on_probe_result(true);
        assert_eq!(m.consecutive_failures(), 0);
    }
}
