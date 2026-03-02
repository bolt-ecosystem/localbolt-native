//! Canonical signaling message types for the Bolt rendezvous protocol.
//!
//! This crate is the single source of truth for the JSON message format
//! exchanged between clients and the bolt-rendezvous signaling server.
//!
//! All messages are serialized/deserialized via serde with `#[serde(tag = "type")]`
//! to produce `{ "type": "...", ... }` JSON objects.
//!
//! # Wire Format
//!
//! Client-to-server messages use `snake_case` type tags:
//! - `register`, `signal`, `ping`
//!
//! Server-to-client messages use `snake_case` type tags:
//! - `peers`, `peer_joined`, `peer_left`, `signal`, `error`

use serde::{Deserialize, Serialize};

/// Device type reported by connecting peers.
///
/// Serializes to lowercase strings: `"phone"`, `"tablet"`, `"laptop"`, `"desktop"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Phone,
    Tablet,
    Laptop,
    Desktop,
}

/// Public peer information broadcast to room members.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerData {
    pub peer_code: String,
    pub device_name: String,
    pub device_type: DeviceType,
}

// ---------------------------------------------------------------------------
// Client -> Server messages
// ---------------------------------------------------------------------------

/// Messages sent from a client to the signaling server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// First message a client must send after connecting.
    Register {
        peer_code: String,
        device_name: String,
        device_type: DeviceType,
    },
    /// Relay a WebRTC signaling payload to another peer.
    Signal {
        to: String,
        payload: serde_json::Value,
    },
    /// Keepalive ping from client (no-op, just prevents idle timeout).
    Ping,
}

// ---------------------------------------------------------------------------
// Server -> Client messages
// ---------------------------------------------------------------------------

/// Messages sent from the signaling server to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Full list of peers currently in the same IP room (sent on registration).
    Peers { peers: Vec<PeerData> },
    /// A new peer joined the IP room.
    PeerJoined { peer: PeerData },
    /// A peer left the IP room.
    PeerLeft { peer_code: String },
    /// Relayed signaling payload from another peer.
    Signal {
        from: String,
        payload: serde_json::Value,
    },
    /// Error response for invalid or malformed messages.
    Error { message: String },
}

// ---------------------------------------------------------------------------
// Wire compatibility tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Helper: serialize to Value and compare against golden fixture.
    fn assert_wire_eq<T: Serialize>(msg: &T, expected: serde_json::Value) {
        let actual = serde_json::to_value(msg).expect("serialization failed");
        assert_eq!(actual, expected, "wire format mismatch");
    }

    // ── ClientMessage wire compatibility ─────────────────────

    #[test]
    fn wire_client_register() {
        let msg = ClientMessage::Register {
            peer_code: "ABC123".into(),
            device_name: "iPhone 15".into(),
            device_type: DeviceType::Phone,
        };
        assert_wire_eq(
            &msg,
            json!({
                "type": "register",
                "peer_code": "ABC123",
                "device_name": "iPhone 15",
                "device_type": "phone"
            }),
        );
    }

    #[test]
    fn wire_client_signal() {
        let msg = ClientMessage::Signal {
            to: "XYZ789".into(),
            payload: json!({"sdp": "offer-data"}),
        };
        assert_wire_eq(
            &msg,
            json!({
                "type": "signal",
                "to": "XYZ789",
                "payload": {"sdp": "offer-data"}
            }),
        );
    }

    #[test]
    fn wire_client_ping() {
        let msg = ClientMessage::Ping;
        assert_wire_eq(&msg, json!({"type": "ping"}));
    }

    // ── ServerMessage wire compatibility ─────────────────────

    #[test]
    fn wire_server_peers() {
        let msg = ServerMessage::Peers {
            peers: vec![PeerData {
                peer_code: "ABC123".into(),
                device_name: "MacBook".into(),
                device_type: DeviceType::Laptop,
            }],
        };
        assert_wire_eq(
            &msg,
            json!({
                "type": "peers",
                "peers": [{
                    "peer_code": "ABC123",
                    "device_name": "MacBook",
                    "device_type": "laptop"
                }]
            }),
        );
    }

    #[test]
    fn wire_server_peer_joined() {
        let msg = ServerMessage::PeerJoined {
            peer: PeerData {
                peer_code: "DEF456".into(),
                device_name: "iPad".into(),
                device_type: DeviceType::Tablet,
            },
        };
        assert_wire_eq(
            &msg,
            json!({
                "type": "peer_joined",
                "peer": {
                    "peer_code": "DEF456",
                    "device_name": "iPad",
                    "device_type": "tablet"
                }
            }),
        );
    }

    #[test]
    fn wire_server_peer_left() {
        let msg = ServerMessage::PeerLeft {
            peer_code: "ABC123".into(),
        };
        assert_wire_eq(
            &msg,
            json!({
                "type": "peer_left",
                "peer_code": "ABC123"
            }),
        );
    }

    #[test]
    fn wire_server_signal() {
        let msg = ServerMessage::Signal {
            from: "ABC123".into(),
            payload: json!({"sdp": "offer-data"}),
        };
        assert_wire_eq(
            &msg,
            json!({
                "type": "signal",
                "from": "ABC123",
                "payload": {"sdp": "offer-data"}
            }),
        );
    }

    #[test]
    fn wire_server_error() {
        let msg = ServerMessage::Error {
            message: "bad request".into(),
        };
        assert_wire_eq(
            &msg,
            json!({
                "type": "error",
                "message": "bad request"
            }),
        );
    }

    // ── Deserialization roundtrip tests ──────────────────────

    #[test]
    fn deserialize_client_register() {
        let json = r#"{"type":"register","peer_code":"ABC123","device_name":"iPhone 15","device_type":"phone"}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Register {
                peer_code,
                device_name,
                device_type,
            } => {
                assert_eq!(peer_code, "ABC123");
                assert_eq!(device_name, "iPhone 15");
                assert_eq!(device_type, DeviceType::Phone);
            }
            _ => panic!("expected Register"),
        }
    }

    #[test]
    fn deserialize_client_signal() {
        let json = r#"{"type":"signal","to":"XYZ789","payload":{"sdp":"..."}}"#;
        let msg: ClientMessage = serde_json::from_str(json).unwrap();
        match msg {
            ClientMessage::Signal { to, payload } => {
                assert_eq!(to, "XYZ789");
                assert!(payload.get("sdp").is_some());
            }
            _ => panic!("expected Signal"),
        }
    }

    #[test]
    fn deserialize_server_peers() {
        let json = r#"{"type":"peers","peers":[{"peer_code":"ABC123","device_name":"test","device_type":"desktop"}]}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        match msg {
            ServerMessage::Peers { peers } => {
                assert_eq!(peers.len(), 1);
                assert_eq!(peers[0].peer_code, "ABC123");
            }
            _ => panic!("expected Peers"),
        }
    }

    #[test]
    fn deserialize_server_signal() {
        let json = r#"{"type":"signal","from":"alice","payload":{"sdp":"offer-data"}}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        match msg {
            ServerMessage::Signal { from, payload } => {
                assert_eq!(from, "alice");
                assert_eq!(payload, json!({"sdp": "offer-data"}));
            }
            _ => panic!("expected Signal"),
        }
    }

    #[test]
    fn deserialize_server_error() {
        let json = r#"{"type":"error","message":"peer not found"}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();
        match msg {
            ServerMessage::Error { message } => assert_eq!(message, "peer not found"),
            _ => panic!("expected Error"),
        }
    }

    // ── DeviceType serde ─────────────────────────────────────

    #[test]
    fn device_type_roundtrip() {
        for (variant, expected_str) in [
            (DeviceType::Phone, "phone"),
            (DeviceType::Tablet, "tablet"),
            (DeviceType::Laptop, "laptop"),
            (DeviceType::Desktop, "desktop"),
        ] {
            let json = serde_json::to_value(&variant).unwrap();
            assert_eq!(json, json!(expected_str));
            let decoded: DeviceType = serde_json::from_value(json).unwrap();
            assert_eq!(decoded, variant);
        }
    }

    // ── Clone ────────────────────────────────────────────────

    #[test]
    fn server_message_clone() {
        let msg = ServerMessage::Signal {
            from: "peer1".into(),
            payload: json!({"key": "value"}),
        };
        let cloned = msg.clone();
        let orig_val = serde_json::to_value(&msg).unwrap();
        let clone_val = serde_json::to_value(&cloned).unwrap();
        assert_eq!(orig_val, clone_val);
    }

    #[test]
    fn client_message_clone() {
        let msg = ClientMessage::Register {
            peer_code: "ABC".into(),
            device_name: "Test".into(),
            device_type: DeviceType::Desktop,
        };
        let cloned = msg.clone();
        let orig_val = serde_json::to_value(&msg).unwrap();
        let clone_val = serde_json::to_value(&cloned).unwrap();
        assert_eq!(orig_val, clone_val);
    }
}
