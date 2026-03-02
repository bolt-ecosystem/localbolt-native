//! Protocol message types for LocalBolt WebSocket signaling.
//!
//! This module re-exports the canonical types from `bolt-rendezvous-protocol`.
//! All message types, enums, and structs are defined in the shared crate to
//! eliminate duplication between the server and client implementations.

pub use bolt_rendezvous_protocol::*;

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
