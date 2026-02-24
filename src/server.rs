//! WebSocket connection handling for the signaling server.
//!
//! Each incoming TCP connection is upgraded to a WebSocket. The first message
//! must be a `register` command; subsequent messages are either `signal` relays
//! or invalid (producing an error response). Peer cleanup on disconnect is
//! handled automatically.
//!
//! ## Trust Boundary Limits (Phase 6A)
//!
//! All incoming data is untrusted. The following limits are enforced:
//!
//! | Limit | Value | Scope |
//! |-------|-------|-------|
//! | `MAX_MESSAGE_BYTES` | 1 MiB | Per WebSocket message (text + binary) |
//! | `MAX_DEVICE_NAME_BYTES` | 256 | `Register.device_name` field |
//! | `MAX_PEER_CODE_BYTES` | 16 | `Register.peer_code` and `Signal.to` fields |
//! | `RATE_LIMIT_PER_SECOND` | 50 | Per-connection message rate |
//! | `RATE_LIMIT_CLOSE_THRESHOLD` | 3 | Consecutive violations before socket close |
//!
//! Protocol-level enforcement via `WebSocketConfig.max_message_size` provides
//! first-line defense. Application-level `validate_message_size()` provides
//! defense-in-depth.

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

use crate::protocol::{ClientMessage, ServerMessage};
use crate::room::{PeerInfo, RoomManager};

// ── Trust Boundary Constants ────────────────────────────────────────────

/// Maximum WebSocket message size (text or binary). 1 MiB.
/// Enforced at both protocol level (WebSocketConfig) and application level.
pub(crate) const MAX_MESSAGE_BYTES: usize = 1_048_576;

/// Maximum length of `Register.device_name` in bytes.
pub(crate) const MAX_DEVICE_NAME_BYTES: usize = 256;

/// Maximum length of peer code fields (`Register.peer_code`, `Signal.to`).
pub(crate) const MAX_PEER_CODE_BYTES: usize = 16;

/// Maximum messages per second per connection.
pub(crate) const RATE_LIMIT_PER_SECOND: u32 = 50;

/// Consecutive rate-limit violations before forcibly closing the socket.
pub(crate) const RATE_LIMIT_CLOSE_THRESHOLD: u32 = 3;

// ── Validation Helpers (pure, testable) ─────────────────────────────────

/// Reject messages exceeding `MAX_MESSAGE_BYTES`.
pub(crate) fn validate_message_size(len: usize) -> Result<(), String> {
    if len > MAX_MESSAGE_BYTES {
        return Err(format!(
            "message too large ({len} bytes, max {MAX_MESSAGE_BYTES})"
        ));
    }
    Ok(())
}

/// Validate device name length.
pub(crate) fn validate_device_name(name: &str) -> Result<(), String> {
    if name.len() > MAX_DEVICE_NAME_BYTES {
        return Err(format!(
            "device_name too long ({} bytes, max {MAX_DEVICE_NAME_BYTES})",
            name.len()
        ));
    }
    Ok(())
}

/// Validate a peer code used as a signal target (`Signal.to`).
/// Same rules as `validate_peer_code`: non-empty, max 16 chars, alphanumeric.
pub(crate) fn validate_signal_target(to: &str) -> Result<(), String> {
    if to.is_empty() {
        return Err("target peer code cannot be empty".to_string());
    }
    if to.len() > MAX_PEER_CODE_BYTES {
        return Err(format!(
            "target peer code too long ({} bytes, max {MAX_PEER_CODE_BYTES})",
            to.len()
        ));
    }
    if !to.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("target peer code must be alphanumeric".to_string());
    }
    Ok(())
}

// ── Per-Connection Rate Limiter ─────────────────────────────────────────

/// Sliding-window rate limiter. Per-connection, not global.
///
/// Resets the counter when the 1-second window elapses.
/// After `RATE_LIMIT_CLOSE_THRESHOLD` consecutive violations, signals
/// that the connection should be closed (fail-closed).
pub(crate) struct RateLimit {
    remaining: u32,
    window_start: tokio::time::Instant,
    consecutive_violations: u32,
}

impl RateLimit {
    pub(crate) fn new() -> Self {
        Self {
            remaining: RATE_LIMIT_PER_SECOND,
            window_start: tokio::time::Instant::now(),
            consecutive_violations: 0,
        }
    }

    /// Check if the current message is within the rate limit.
    ///
    /// Returns `Ok(())` if allowed, `Err(true)` if the socket should be closed
    /// (threshold exceeded), `Err(false)` if rate-limited but not yet at threshold.
    pub(crate) fn check(&mut self) -> Result<(), bool> {
        let now = tokio::time::Instant::now();
        if now.duration_since(self.window_start) >= std::time::Duration::from_secs(1) {
            self.remaining = RATE_LIMIT_PER_SECOND;
            self.window_start = now;
        }
        if self.remaining > 0 {
            self.remaining -= 1;
            self.consecutive_violations = 0;
            Ok(())
        } else {
            self.consecutive_violations += 1;
            if self.consecutive_violations >= RATE_LIMIT_CLOSE_THRESHOLD {
                Err(true) // close socket
            } else {
                Err(false) // send error, keep open
            }
        }
    }
}

// ── WebSocket Config ────────────────────────────────────────────────────

/// Build the WebSocket protocol config with message size limits.
fn ws_config() -> Option<WebSocketConfig> {
    Some(WebSocketConfig {
        max_message_size: Some(MAX_MESSAGE_BYTES),
        max_frame_size: Some(MAX_MESSAGE_BYTES),
        ..WebSocketConfig::default()
    })
}

// ── Connection Handler ──────────────────────────────────────────────────

/// Handle a single incoming TCP connection: upgrade to WebSocket and process messages.
pub async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    room_manager: Arc<RoomManager>,
) {
    // We'll capture headers during the handshake callback to extract X-Forwarded-For.
    let forwarded_for = Arc::new(std::sync::Mutex::new(None::<String>));
    let forwarded_for_cb = forwarded_for.clone();

    let callback = move |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
        // Extract X-Forwarded-For if present (reverse proxy scenario).
        if let Some(xff) = req.headers().get("x-forwarded-for") {
            if let Ok(value) = xff.to_str() {
                // Take the first IP in a comma-separated list.
                let client_ip = value.split(',').next().unwrap_or(value).trim().to_string();
                if let Ok(mut lock) = forwarded_for_cb.lock() {
                    *lock = Some(client_ip);
                }
            }
        }
        Ok(resp)
    };

    // Protocol-level message size enforcement via WebSocketConfig.
    let ws_stream = match tokio_tungstenite::accept_hdr_async_with_config(
        stream,
        callback,
        ws_config(),
    )
    .await
    {
        Ok(ws) => ws,
        Err(e) => {
            error!(addr = %addr, error = %e, "WebSocket handshake failed");
            return;
        }
    };

    // Determine the effective client IP.
    let raw_ip = forwarded_for
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .unwrap_or_else(|| addr.ip().to_string());

    // For self-hosted mode: all private/loopback IPs share one room ("local").
    // This lets devices on the same LAN discover each other even when the host
    // machine connects via 127.0.0.1 and others via 192.168.x.x.
    let client_ip = if is_private_ip(&raw_ip) {
        "local".to_string()
    } else {
        raw_ip
    };

    debug!(addr = %addr, client_ip = %client_ip, "WebSocket connection established");

    let (mut ws_sink, mut ws_stream_rx) = ws_stream.split();

    // Channel for sending server messages to this peer's WebSocket.
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    // Spawn a task that forwards messages from the channel to the WebSocket sink.
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match serde_json::to_string(&msg) {
                Ok(json) => {
                    if ws_sink.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!(error = %e, "failed to serialize ServerMessage");
                }
            }
        }
    });

    // Per-connection rate limiter (applies to both registration and message phases).
    let mut rate_limit = RateLimit::new();

    // --- Registration phase ---
    // The first message must be a "register" command.
    let (peer_code, _device_name, _device_type) = loop {
        match ws_stream_rx.next().await {
            Some(Ok(Message::Text(text))) => {
                // Rate limit check (pre-registration).
                match rate_limit.check() {
                    Err(true) => {
                        warn!(addr = %addr, "rate limit exceeded — closing connection");
                        write_task.abort();
                        return;
                    }
                    Err(false) => {
                        warn!(addr = %addr, "rate limited during registration");
                        let _ = tx.send(ServerMessage::Error {
                            message: "rate limited".into(),
                        });
                        continue;
                    }
                    Ok(()) => {}
                }

                // Application-level message size check (defense-in-depth).
                if let Err(e) = validate_message_size(text.len()) {
                    warn!(addr = %addr, error = %e, "oversized message during registration");
                    let _ = tx.send(ServerMessage::Error { message: e });
                    continue;
                }

                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Register {
                        peer_code,
                        device_name,
                        device_type,
                    }) => {
                        // Validate device_name length.
                        if let Err(e) = validate_device_name(&device_name) {
                            warn!(addr = %addr, error = %e, "invalid device_name");
                            let _ = tx.send(ServerMessage::Error { message: e });
                            continue;
                        }
                        break (peer_code, device_name, device_type);
                    }
                    Ok(_) => {
                        warn!(addr = %addr, "received non-register message before registration");
                        let err = ServerMessage::Error {
                            message: "must send 'register' as first message".into(),
                        };
                        let _ = tx.send(err);
                    }
                    Err(e) => {
                        warn!(addr = %addr, error = %e, "malformed message during registration");
                        let err = ServerMessage::Error {
                            message: format!("malformed message: {e}"),
                        };
                        let _ = tx.send(err);
                    }
                }
            }
            Some(Ok(Message::Binary(_))) => {
                // Binary frames rejected — signaling is text-only.
                warn!(addr = %addr, "binary frame rejected during registration");
                let _ = tx.send(ServerMessage::Error {
                    message: "binary frames not accepted".into(),
                });
                continue;
            }
            Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                // Ignore control frames during registration.
                continue;
            }
            Some(Ok(Message::Close(_))) | None => {
                debug!(addr = %addr, "connection closed before registration");
                write_task.abort();
                return;
            }
            Some(Ok(_)) => {
                continue;
            }
            Some(Err(e)) => {
                debug!(addr = %addr, error = %e, "WebSocket error before registration");
                write_task.abort();
                return;
            }
        }
    };

    // Validate peer code format.
    if let Err(e) = validate_peer_code(&peer_code) {
        warn!(addr = %addr, error = %e, "invalid peer code");
        let _ = tx.send(ServerMessage::Error { message: e });
        write_task.abort();
        return;
    }

    // Build peer info and add to room.
    let peer_info = PeerInfo {
        peer_code: peer_code.clone(),
        device_name: _device_name,
        device_type: _device_type,
        sender: tx.clone(),
    };

    let existing_peers = match room_manager.add_peer(&client_ip, peer_info) {
        Ok(peers) => peers,
        Err(e) => {
            warn!(addr = %addr, error = %e, "peer code collision");
            let _ = tx.send(ServerMessage::Error { message: e });
            write_task.abort();
            return;
        }
    };

    // Send the current peer list to the newly registered peer.
    let peers_msg = ServerMessage::Peers {
        peers: existing_peers,
    };
    let _ = tx.send(peers_msg);

    info!(
        peer_code = %peer_code,
        client_ip = %client_ip,
        "peer registered"
    );

    // --- Message loop ---
    loop {
        match ws_stream_rx.next().await {
            Some(Ok(Message::Text(text))) => {
                // Rate limit check (post-registration).
                match rate_limit.check() {
                    Err(true) => {
                        warn!(peer_code = %peer_code, "rate limit exceeded — closing connection");
                        break;
                    }
                    Err(false) => {
                        warn!(peer_code = %peer_code, "rate limited");
                        let _ = tx.send(ServerMessage::Error {
                            message: "rate limited".into(),
                        });
                        continue;
                    }
                    Ok(()) => {}
                }

                // Application-level message size check (defense-in-depth).
                if let Err(e) = validate_message_size(text.len()) {
                    warn!(peer_code = %peer_code, error = %e, "oversized message");
                    let _ = tx.send(ServerMessage::Error { message: e });
                    continue;
                }

                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Signal { to, payload }) => {
                        // Validate Signal.to field.
                        if let Err(e) = validate_signal_target(&to) {
                            warn!(from = %peer_code, error = %e, "invalid signal target");
                            let _ = tx.send(ServerMessage::Error { message: e });
                            continue;
                        }

                        info!(from = %peer_code, to = %to, "signal relay");
                        if let Some(target_sender) = room_manager.find_peer(&to) {
                            let relay_msg = ServerMessage::Signal {
                                from: peer_code.clone(),
                                payload,
                            };
                            if target_sender.send(relay_msg).is_err() {
                                warn!(
                                    from = %peer_code,
                                    to = %to,
                                    "target peer channel closed"
                                );
                                let err = ServerMessage::Error {
                                    message: format!("peer '{to}' is no longer connected"),
                                };
                                let _ = tx.send(err);
                            }
                        } else {
                            debug!(from = %peer_code, to = %to, "target peer not found");
                            let err = ServerMessage::Error {
                                message: format!("peer '{to}' not found"),
                            };
                            let _ = tx.send(err);
                        }
                    }
                    Ok(ClientMessage::Ping) => {
                        // Keepalive — no-op, just prevents idle timeout.
                        continue;
                    }
                    Ok(ClientMessage::Register { .. }) => {
                        warn!(peer_code = %peer_code, "duplicate register message");
                        let err = ServerMessage::Error {
                            message: "already registered".into(),
                        };
                        let _ = tx.send(err);
                    }
                    Err(e) => {
                        warn!(peer_code = %peer_code, error = %e, "malformed message");
                        let err = ServerMessage::Error {
                            message: format!("malformed message: {e}"),
                        };
                        let _ = tx.send(err);
                    }
                }
            }
            Some(Ok(Message::Binary(_))) => {
                // Binary frames rejected — signaling is text-only.
                warn!(peer_code = %peer_code, "binary frame rejected");
                let _ = tx.send(ServerMessage::Error {
                    message: "binary frames not accepted".into(),
                });
                continue;
            }
            Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                // Control frames handled by tungstenite automatically.
                continue;
            }
            Some(Ok(Message::Close(_))) | None => {
                break;
            }
            Some(Ok(_)) => {
                continue;
            }
            Some(Err(e)) => {
                warn!(peer_code = %peer_code, error = %e, "WebSocket error");
                break;
            }
        }
    }

    // --- Cleanup ---
    info!(peer_code = %peer_code, client_ip = %client_ip, "peer disconnected");
    room_manager.remove_peer(&client_ip, &peer_code);
    write_task.abort();
}

/// Validate peer code format: non-empty, max 16 chars, alphanumeric only.
pub(crate) fn validate_peer_code(code: &str) -> Result<(), String> {
    if code.is_empty() {
        return Err("Peer code cannot be empty".to_string());
    }
    if code.len() > MAX_PEER_CODE_BYTES {
        return Err(format!(
            "Peer code too long (max {MAX_PEER_CODE_BYTES} characters)"
        ));
    }
    if !code.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Peer code must be alphanumeric".to_string());
    }
    Ok(())
}

/// Check if an IP address is private (RFC 1918), loopback, or link-local.
fn is_private_ip(ip: &str) -> bool {
    // IPv4 loopback
    if ip == "127.0.0.1" {
        return true;
    }
    // IPv4 Class A private
    if ip.starts_with("10.") {
        return true;
    }
    // IPv4 Class C private
    if ip.starts_with("192.168.") {
        return true;
    }
    // IPv4 link-local
    if ip.starts_with("169.254.") {
        return true;
    }
    // IPv4 Class B private: 172.16.0.0/12 (172.16.0.0 - 172.31.255.255)
    if ip.starts_with("172.") {
        if let Some(second) = ip.split('.').nth(1) {
            if let Ok(n) = second.parse::<u8>() {
                if (16..=31).contains(&n) {
                    return true;
                }
            }
        }
    }
    // IPv4 CGNAT / shared address space: 100.64.0.0/10 (100.64.0.0 - 100.127.255.255)
    // Used by Tailscale, some WireGuard meshes, and carrier-grade NAT.
    // Devices on the same Tailscale/WireGuard mesh are "local" to each other.
    if ip.starts_with("100.") {
        if let Some(second) = ip.split('.').nth(1) {
            if let Ok(n) = second.parse::<u8>() {
                if (64..=127).contains(&n) {
                    return true;
                }
            }
        }
    }
    // IPv6 loopback
    if ip == "::1" {
        return true;
    }
    // IPv6 unique local (fc00::/7)
    if ip.starts_with("fc") || ip.starts_with("fd") {
        return true;
    }
    // IPv6 link-local (fe80::/10)
    if ip.starts_with("fe80") {
        return true;
    }
    false
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── validate_message_size ───────────────────────────────────

    #[test]
    fn message_size_within_limit() {
        assert!(validate_message_size(0).is_ok());
        assert!(validate_message_size(1024).is_ok());
        assert!(validate_message_size(MAX_MESSAGE_BYTES).is_ok());
    }

    #[test]
    fn message_size_exceeds_limit() {
        let result = validate_message_size(MAX_MESSAGE_BYTES + 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("message too large"));
        assert!(err.contains(&(MAX_MESSAGE_BYTES + 1).to_string()));
    }

    #[test]
    fn message_size_boundary() {
        assert!(validate_message_size(MAX_MESSAGE_BYTES).is_ok());
        assert!(validate_message_size(MAX_MESSAGE_BYTES + 1).is_err());
    }

    // ── validate_device_name ────────────────────────────────────

    #[test]
    fn device_name_within_limit() {
        assert!(validate_device_name("iPhone 15").is_ok());
        assert!(validate_device_name(&"x".repeat(MAX_DEVICE_NAME_BYTES)).is_ok());
    }

    #[test]
    fn device_name_exceeds_limit() {
        let result = validate_device_name(&"x".repeat(MAX_DEVICE_NAME_BYTES + 1));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("device_name too long"));
    }

    #[test]
    fn device_name_empty_is_ok() {
        // Empty device names are allowed (field is optional in practice).
        assert!(validate_device_name("").is_ok());
    }

    // ── validate_signal_target ──────────────────────────────────

    #[test]
    fn signal_target_valid() {
        assert!(validate_signal_target("ABC123").is_ok());
        assert!(validate_signal_target("X").is_ok());
        assert!(validate_signal_target(&"A".repeat(MAX_PEER_CODE_BYTES)).is_ok());
    }

    #[test]
    fn signal_target_empty() {
        let result = validate_signal_target("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn signal_target_too_long() {
        let result = validate_signal_target(&"A".repeat(MAX_PEER_CODE_BYTES + 1));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too long"));
    }

    #[test]
    fn signal_target_non_alphanumeric() {
        assert!(validate_signal_target("ABC-123").is_err());
        assert!(validate_signal_target("ABC 123").is_err());
        assert!(validate_signal_target("ABC\n123").is_err());
    }

    // ── validate_peer_code ──────────────────────────────────────

    #[test]
    fn peer_code_valid() {
        assert!(validate_peer_code("ABC123").is_ok());
        assert!(validate_peer_code(&"Z".repeat(MAX_PEER_CODE_BYTES)).is_ok());
    }

    #[test]
    fn peer_code_empty() {
        assert!(validate_peer_code("").is_err());
    }

    #[test]
    fn peer_code_too_long() {
        assert!(validate_peer_code(&"A".repeat(MAX_PEER_CODE_BYTES + 1)).is_err());
    }

    #[test]
    fn peer_code_non_alphanumeric() {
        assert!(validate_peer_code("AB!C").is_err());
    }

    // ── RateLimit ───────────────────────────────────────────────

    #[tokio::test]
    async fn rate_limit_allows_within_budget() {
        tokio::time::pause();
        let mut rl = RateLimit::new();
        for _ in 0..RATE_LIMIT_PER_SECOND {
            assert!(rl.check().is_ok());
        }
    }

    #[tokio::test]
    async fn rate_limit_rejects_over_budget() {
        tokio::time::pause();
        let mut rl = RateLimit::new();
        // Exhaust budget.
        for _ in 0..RATE_LIMIT_PER_SECOND {
            let _ = rl.check();
        }
        // Next call should be rate-limited but not yet close.
        assert_eq!(rl.check(), Err(false));
    }

    #[tokio::test]
    async fn rate_limit_closes_after_threshold() {
        tokio::time::pause();
        let mut rl = RateLimit::new();
        // Exhaust budget.
        for _ in 0..RATE_LIMIT_PER_SECOND {
            let _ = rl.check();
        }
        // Consecutive violations up to threshold.
        for _ in 0..(RATE_LIMIT_CLOSE_THRESHOLD - 1) {
            assert_eq!(rl.check(), Err(false));
        }
        // Threshold reached → close.
        assert_eq!(rl.check(), Err(true));
    }

    #[tokio::test]
    async fn rate_limit_resets_after_window() {
        tokio::time::pause();
        let mut rl = RateLimit::new();
        // Exhaust budget.
        for _ in 0..RATE_LIMIT_PER_SECOND {
            let _ = rl.check();
        }
        assert!(rl.check().is_err());

        // Advance time past the 1-second window.
        tokio::time::advance(std::time::Duration::from_secs(2)).await;

        // Budget should be refreshed.
        assert!(rl.check().is_ok());
    }

    #[tokio::test]
    async fn rate_limit_violation_count_resets_on_success() {
        tokio::time::pause();
        let mut rl = RateLimit::new();
        // Exhaust budget, accumulate 2 violations.
        for _ in 0..RATE_LIMIT_PER_SECOND {
            let _ = rl.check();
        }
        assert_eq!(rl.check(), Err(false));
        assert_eq!(rl.check(), Err(false));

        // Advance time to reset window.
        tokio::time::advance(std::time::Duration::from_secs(2)).await;
        assert!(rl.check().is_ok());

        // Violations counter should be reset — exhaust and re-violate.
        for _ in 1..RATE_LIMIT_PER_SECOND {
            let _ = rl.check();
        }
        // First violation after reset: should NOT trigger close.
        assert_eq!(rl.check(), Err(false));
    }

    // ── Constants sanity ────────────────────────────────────────

    #[test]
    fn trust_boundary_constants() {
        assert_eq!(MAX_MESSAGE_BYTES, 1_048_576);
        assert_eq!(MAX_DEVICE_NAME_BYTES, 256);
        assert_eq!(MAX_PEER_CODE_BYTES, 16);
        assert_eq!(RATE_LIMIT_PER_SECOND, 50);
        assert_eq!(RATE_LIMIT_CLOSE_THRESHOLD, 3);
    }

    // ── ws_config ───────────────────────────────────────────────

    #[test]
    fn ws_config_sets_message_limits() {
        let config = ws_config().expect("config should be Some");
        assert_eq!(config.max_message_size, Some(MAX_MESSAGE_BYTES));
        assert_eq!(config.max_frame_size, Some(MAX_MESSAGE_BYTES));
    }
}
