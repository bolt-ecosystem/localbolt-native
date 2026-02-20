//! WebSocket connection handling for the signaling server.
//!
//! Each incoming TCP connection is upgraded to a WebSocket. The first message
//! must be a `register` command; subsequent messages are either `signal` relays
//! or invalid (producing an error response). Peer cleanup on disconnect is
//! handled automatically.

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

use crate::protocol::{ClientMessage, ServerMessage};
use crate::room::{PeerInfo, RoomManager};

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

    let ws_stream = match tokio_tungstenite::accept_hdr_async(stream, callback).await {
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

    // --- Registration phase ---
    // The first message must be a "register" command.
    let (peer_code, _device_name, _device_type) = loop {
        match ws_stream_rx.next().await {
            Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
                Ok(ClientMessage::Register {
                    peer_code,
                    device_name,
                    device_type,
                }) => {
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
            },
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
                // Binary or other frame types — ignore.
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
                match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(ClientMessage::Signal { to, payload }) => {
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
            Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
                // Control frames handled by tungstenite automatically.
                continue;
            }
            Some(Ok(Message::Close(_))) | None => {
                break;
            }
            Some(Ok(_)) => {
                // Binary or other frame types — ignore silently.
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
fn validate_peer_code(code: &str) -> Result<(), String> {
    if code.is_empty() {
        return Err("Peer code cannot be empty".to_string());
    }
    if code.len() > 16 {
        return Err("Peer code too long (max 16 characters)".to_string());
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
