//! Daemon process watchdog state machine.
//!
//! Implements N3 supervision spec: 5-state machine with retry/backoff,
//! degraded mode, and version incompatibility detection.

use serde::Serialize;
use std::time::{Duration, Instant};

/// Backoff delays for retries 0, 1, 2.
const BACKOFF_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(3),
    Duration::from_secs(10),
];

/// Max retries before entering degraded state.
const MAX_RETRIES: u32 = 3;

/// Duration of stable `ready` before retry counter resets.
const RETRY_RESET_WINDOW: Duration = Duration::from_secs(60);

/// Startup timeout before entering degraded state.
pub const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Watchdog states per N3 spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchdogState {
    Starting,
    Ready,
    Restarting,
    Degraded,
    Incompatible,
}

impl std::fmt::Display for WatchdogState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Ready => write!(f, "ready"),
            Self::Restarting => write!(f, "restarting"),
            Self::Degraded => write!(f, "degraded"),
            Self::Incompatible => write!(f, "incompatible"),
        }
    }
}

/// Watchdog state machine.
pub struct Watchdog {
    state: WatchdogState,
    retry_count: u32,
    ready_since: Option<Instant>,
}

/// Result of a state transition.
#[derive(Debug, PartialEq, Eq)]
pub enum Transition {
    /// State changed.
    Changed(WatchdogState),
    /// State unchanged (no-op).
    Unchanged,
}

impl Watchdog {
    pub fn new() -> Self {
        tracing::info!("[WATCHDOG] initialized in state: starting");
        Self {
            state: WatchdogState::Starting,
            retry_count: 0,
            ready_since: None,
        }
    }

    pub fn state(&self) -> WatchdogState {
        self.state
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }

    /// Transition to `ready` upon receiving daemon.status.
    pub fn on_daemon_ready(&mut self) -> Transition {
        match self.state {
            WatchdogState::Starting | WatchdogState::Restarting => {
                let old = self.state;
                self.state = WatchdogState::Ready;
                self.ready_since = Some(Instant::now());
                tracing::info!("[WATCHDOG] {old} -> ready");
                Transition::Changed(WatchdogState::Ready)
            }
            _ => Transition::Unchanged,
        }
    }

    /// Transition to `incompatible` upon version mismatch.
    pub fn on_version_incompatible(&mut self) -> Transition {
        if self.state == WatchdogState::Incompatible {
            return Transition::Unchanged;
        }
        let old = self.state;
        self.state = WatchdogState::Incompatible;
        self.ready_since = None;
        tracing::warn!("[WATCHDOG] {old} -> incompatible (version mismatch)");
        Transition::Changed(WatchdogState::Incompatible)
    }

    /// Handle daemon process exit (crash or unexpected termination).
    ///
    /// Returns the backoff delay if transitioning to `restarting`,
    /// or `None` if entering `degraded`.
    pub fn on_daemon_exit(&mut self, exit_code: Option<i32>) -> Option<Duration> {
        match self.state {
            WatchdogState::Incompatible => {
                // Incompatible is terminal; don't restart.
                return None;
            }
            WatchdogState::Degraded => {
                // Already degraded; don't restart.
                return None;
            }
            _ => {}
        }

        if self.retry_count >= MAX_RETRIES {
            let old = self.state;
            self.state = WatchdogState::Degraded;
            self.ready_since = None;
            tracing::error!(
                "[WATCHDOG] {old} -> degraded (retries exhausted after {} attempts, last exit_code={:?})",
                self.retry_count,
                exit_code
            );
            return None;
        }

        let delay = BACKOFF_DELAYS[self.retry_count as usize];
        self.retry_count += 1;
        let old = self.state;
        self.state = WatchdogState::Restarting;
        self.ready_since = None;
        tracing::warn!(
            "[WATCHDOG] {old} -> restarting (retry {}/{MAX_RETRIES}, delay {}s, exit_code={:?})",
            self.retry_count,
            delay.as_secs(),
            exit_code
        );
        Some(delay)
    }

    /// Handle binary missing / spawn failure.
    /// Goes directly to degraded without consuming retries.
    pub fn on_spawn_failure(&mut self, reason: &str) -> Transition {
        if self.state == WatchdogState::Degraded {
            return Transition::Unchanged;
        }
        let old = self.state;
        self.state = WatchdogState::Degraded;
        self.ready_since = None;
        tracing::error!("[WATCHDOG] {old} -> degraded (spawn failure: {reason})");
        Transition::Changed(WatchdogState::Degraded)
    }

    /// Handle startup timeout (10s without daemon.status).
    pub fn on_startup_timeout(&mut self) -> Option<Duration> {
        if self.state == WatchdogState::Starting {
            tracing::warn!(
                "[WATCHDOG] startup timeout ({}s)",
                STARTUP_TIMEOUT.as_secs()
            );
            self.on_daemon_exit(None)
        } else {
            None
        }
    }

    /// Check if retry counter should reset (60s stable ready).
    /// Call periodically (e.g. every 10s heartbeat).
    pub fn maybe_reset_retries(&mut self) {
        if self.state != WatchdogState::Ready {
            return;
        }
        if let Some(since) = self.ready_since {
            if since.elapsed() >= RETRY_RESET_WINDOW && self.retry_count > 0 {
                tracing::info!(
                    "[WATCHDOG] success window reached ({}s), retry counter reset ({} -> 0)",
                    RETRY_RESET_WINDOW.as_secs(),
                    self.retry_count
                );
                self.retry_count = 0;
            }
        }
    }

    /// Manual restart from degraded state. Resets to starting.
    pub fn manual_restart(&mut self) -> Transition {
        match self.state {
            WatchdogState::Degraded => {
                self.state = WatchdogState::Starting;
                self.retry_count = 0;
                self.ready_since = None;
                tracing::info!("[WATCHDOG] degraded -> starting (manual restart)");
                Transition::Changed(WatchdogState::Starting)
            }
            WatchdogState::Incompatible => {
                // Incompatible is terminal; manual restart not allowed.
                tracing::warn!("[WATCHDOG] manual restart rejected: incompatible is terminal");
                Transition::Unchanged
            }
            _ => Transition::Unchanged,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_starting() {
        let w = Watchdog::new();
        assert_eq!(w.state(), WatchdogState::Starting);
        assert_eq!(w.retry_count(), 0);
    }

    #[test]
    fn starting_to_ready_on_daemon_status() {
        let mut w = Watchdog::new();
        let t = w.on_daemon_ready();
        assert_eq!(t, Transition::Changed(WatchdogState::Ready));
        assert_eq!(w.state(), WatchdogState::Ready);
    }

    #[test]
    fn ready_ignores_duplicate_daemon_ready() {
        let mut w = Watchdog::new();
        w.on_daemon_ready();
        let t = w.on_daemon_ready();
        assert_eq!(t, Transition::Unchanged);
        assert_eq!(w.state(), WatchdogState::Ready);
    }

    #[test]
    fn daemon_exit_from_ready_transitions_to_restarting() {
        let mut w = Watchdog::new();
        w.on_daemon_ready();
        let delay = w.on_daemon_exit(Some(1));
        assert_eq!(delay, Some(Duration::from_secs(1)));
        assert_eq!(w.state(), WatchdogState::Restarting);
        assert_eq!(w.retry_count(), 1);
    }

    #[test]
    fn backoff_sequence_1s_3s_10s() {
        let mut w = Watchdog::new();
        w.on_daemon_ready();

        let d1 = w.on_daemon_exit(Some(1)).unwrap();
        assert_eq!(d1, Duration::from_secs(1));
        w.on_daemon_ready();

        let d2 = w.on_daemon_exit(Some(1)).unwrap();
        assert_eq!(d2, Duration::from_secs(3));
        w.on_daemon_ready();

        let d3 = w.on_daemon_exit(Some(1)).unwrap();
        assert_eq!(d3, Duration::from_secs(10));
    }

    #[test]
    fn retries_exhausted_enters_degraded() {
        let mut w = Watchdog::new();
        w.on_daemon_ready();

        // Exhaust 3 retries
        for _ in 0..3 {
            w.on_daemon_exit(Some(1));
            w.on_daemon_ready();
        }
        // 4th exit -> degraded
        let delay = w.on_daemon_exit(Some(1));
        assert!(delay.is_none());
        assert_eq!(w.state(), WatchdogState::Degraded);
    }

    #[test]
    fn degraded_no_further_restarts() {
        let mut w = Watchdog::new();
        w.on_spawn_failure("binary not found");
        assert_eq!(w.state(), WatchdogState::Degraded);

        let delay = w.on_daemon_exit(Some(1));
        assert!(delay.is_none());
        assert_eq!(w.state(), WatchdogState::Degraded);
    }

    #[test]
    fn spawn_failure_goes_to_degraded_directly() {
        let mut w = Watchdog::new();
        let t = w.on_spawn_failure("binary not found");
        assert_eq!(t, Transition::Changed(WatchdogState::Degraded));
        assert_eq!(w.retry_count(), 0); // no retries consumed
    }

    #[test]
    fn version_incompatible_is_terminal() {
        let mut w = Watchdog::new();
        let t = w.on_version_incompatible();
        assert_eq!(t, Transition::Changed(WatchdogState::Incompatible));

        // Cannot restart from incompatible
        let delay = w.on_daemon_exit(None);
        assert!(delay.is_none());
        assert_eq!(w.state(), WatchdogState::Incompatible);

        // Manual restart rejected
        let t = w.manual_restart();
        assert_eq!(t, Transition::Unchanged);
    }

    #[test]
    fn manual_restart_from_degraded() {
        let mut w = Watchdog::new();
        w.on_spawn_failure("missing");
        assert_eq!(w.state(), WatchdogState::Degraded);

        let t = w.manual_restart();
        assert_eq!(t, Transition::Changed(WatchdogState::Starting));
        assert_eq!(w.retry_count(), 0);
    }

    #[test]
    fn manual_restart_ignored_when_ready() {
        let mut w = Watchdog::new();
        w.on_daemon_ready();
        let t = w.manual_restart();
        assert_eq!(t, Transition::Unchanged);
    }

    #[test]
    fn restarting_to_ready() {
        let mut w = Watchdog::new();
        w.on_daemon_ready();
        w.on_daemon_exit(Some(1));
        assert_eq!(w.state(), WatchdogState::Restarting);

        let t = w.on_daemon_ready();
        assert_eq!(t, Transition::Changed(WatchdogState::Ready));
    }

    #[test]
    fn startup_timeout_triggers_restart_sequence() {
        let mut w = Watchdog::new();
        let delay = w.on_startup_timeout();
        assert_eq!(delay, Some(Duration::from_secs(1)));
        assert_eq!(w.state(), WatchdogState::Restarting);
    }

    #[test]
    fn retry_reset_after_60s_stable() {
        let mut w = Watchdog::new();
        w.on_daemon_ready();
        w.on_daemon_exit(Some(1)); // retry_count = 1
        w.on_daemon_ready();

        // Simulate 60s elapsed by directly setting ready_since
        w.ready_since = Some(Instant::now() - RETRY_RESET_WINDOW - Duration::from_secs(1));
        w.maybe_reset_retries();
        assert_eq!(w.retry_count(), 0);
    }

    #[test]
    fn retry_reset_does_not_fire_before_60s() {
        let mut w = Watchdog::new();
        w.on_daemon_ready();
        w.on_daemon_exit(Some(1));
        w.on_daemon_ready();

        // ready_since is just now, so < 60s
        w.maybe_reset_retries();
        assert_eq!(w.retry_count(), 1);
    }

    #[test]
    fn display_impl() {
        assert_eq!(WatchdogState::Starting.to_string(), "starting");
        assert_eq!(WatchdogState::Ready.to_string(), "ready");
        assert_eq!(WatchdogState::Restarting.to_string(), "restarting");
        assert_eq!(WatchdogState::Degraded.to_string(), "degraded");
        assert_eq!(WatchdogState::Incompatible.to_string(), "incompatible");
    }

    #[test]
    fn serialize_state_snake_case() {
        let json = serde_json::to_string(&WatchdogState::Starting).unwrap();
        assert_eq!(json, "\"starting\"");
        let json = serde_json::to_string(&WatchdogState::Incompatible).unwrap();
        assert_eq!(json, "\"incompatible\"");
    }

    /// Reconnect stress drill: 10 connect/disconnect/reconnect cycles.
    /// Asserts no stuck state and deterministic behavior at every step.
    #[test]
    fn reconnect_stress_10_cycles() {
        let mut w = Watchdog::new();

        for cycle in 0..10 {
            // Connect
            let t = w.on_daemon_ready();
            assert_eq!(
                w.state(),
                WatchdogState::Ready,
                "cycle {cycle}: should be ready after on_daemon_ready"
            );

            // Simulate 60s stable to reset retries (via direct field set)
            w.ready_since = Some(Instant::now() - RETRY_RESET_WINDOW - Duration::from_secs(1));
            w.maybe_reset_retries();
            assert_eq!(w.retry_count(), 0, "cycle {cycle}: retries should reset");

            // Disconnect (daemon exit)
            let delay = w.on_daemon_exit(Some(0));
            assert!(delay.is_some(), "cycle {cycle}: should get restart delay");
            assert_eq!(
                w.state(),
                WatchdogState::Restarting,
                "cycle {cycle}: should be restarting"
            );
        }

        // After 10 cycles with retry resets, should never reach degraded
        assert_ne!(w.state(), WatchdogState::Degraded);
    }

    /// Rapid crash without stable window: hits degraded deterministically.
    #[test]
    fn rapid_crash_no_stable_window_degrades() {
        let mut w = Watchdog::new();
        w.on_daemon_ready();

        // 3 rapid exits without stable ready window
        for i in 0..3 {
            w.on_daemon_exit(Some(1));
            if i < 2 {
                w.on_daemon_ready(); // restart succeeds but crashes again immediately
            }
        }

        // 4th exit should degrade
        let delay = w.on_daemon_exit(Some(1));
        assert!(delay.is_none(), "should be degraded, no more restarts");
        assert_eq!(w.state(), WatchdogState::Degraded);
    }

    /// Manual restart from degraded + full recovery cycle.
    #[test]
    fn degraded_manual_restart_full_recovery() {
        let mut w = Watchdog::new();
        w.on_spawn_failure("test");
        assert_eq!(w.state(), WatchdogState::Degraded);

        // Manual restart
        w.manual_restart();
        assert_eq!(w.state(), WatchdogState::Starting);
        assert_eq!(w.retry_count(), 0);

        // Recovery
        w.on_daemon_ready();
        assert_eq!(w.state(), WatchdogState::Ready);
    }

    /// No stuck state: every state has at least one valid transition out.
    #[test]
    fn no_stuck_state() {
        // Starting -> Ready or Restarting (via timeout)
        let mut w = Watchdog::new();
        assert_eq!(w.state(), WatchdogState::Starting);
        w.on_daemon_ready();
        assert_eq!(w.state(), WatchdogState::Ready);

        // Ready -> Restarting (via exit)
        w.on_daemon_exit(Some(0));
        assert_eq!(w.state(), WatchdogState::Restarting);

        // Restarting -> Ready (via daemon_ready)
        w.on_daemon_ready();
        assert_eq!(w.state(), WatchdogState::Ready);

        // Degraded -> Starting (via manual restart)
        w.on_spawn_failure("test");
        assert_eq!(w.state(), WatchdogState::Degraded);
        w.manual_restart();
        assert_eq!(w.state(), WatchdogState::Starting);

        // Incompatible is intentionally terminal
        w.on_version_incompatible();
        assert_eq!(w.state(), WatchdogState::Incompatible);
        let t = w.manual_restart();
        assert_eq!(t, Transition::Unchanged); // terminal by design
    }
}
