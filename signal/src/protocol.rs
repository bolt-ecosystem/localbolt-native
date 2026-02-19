//! Protocol message types for LocalBolt WebSocket signaling.
//!
//! Defines the JSON message format exchanged between clients and the signaling server.
//! All messages are serialized/deserialized via serde with `#[serde(tag = "type")]`
//! to produce `{ "type": "...", ... }` JSON objects.

use serde::{Deserialize, Serialize};

/// Device type reported by connecting peers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Serialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_register() {
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
    fn deserialize_signal() {
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
    fn serialize_peers() {
        let msg = ServerMessage::Peers {
            peers: vec![PeerData {
                peer_code: "ABC123".into(),
                device_name: "MacBook".into(),
                device_type: DeviceType::Laptop,
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"peers""#));
        assert!(json.contains(r#""peer_code":"ABC123""#));
    }

    #[test]
    fn serialize_peer_joined() {
        let msg = ServerMessage::PeerJoined {
            peer: PeerData {
                peer_code: "DEF456".into(),
                device_name: "iPad".into(),
                device_type: DeviceType::Tablet,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"peer_joined""#));
    }

    #[test]
    fn serialize_peer_left() {
        let msg = ServerMessage::PeerLeft {
            peer_code: "ABC123".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"peer_left""#));
        assert!(json.contains(r#""peer_code":"ABC123""#));
    }

    #[test]
    fn serialize_signal_relay() {
        let msg = ServerMessage::Signal {
            from: "ABC123".into(),
            payload: serde_json::json!({"sdp": "offer-data"}),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"signal""#));
        assert!(json.contains(r#""from":"ABC123""#));
    }

    #[test]
    fn serialize_error() {
        let msg = ServerMessage::Error {
            message: "bad request".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"error""#));
        assert!(json.contains(r#""message":"bad request""#));
    }
}
