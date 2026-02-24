//! IP-based room management for peer discovery.
//!
//! Peers connecting from the same IP address are grouped into the same "room",
//! enabling local-network device discovery without any manual pairing. The
//! [`RoomManager`] uses a [`DashMap`] for lock-free concurrent access.

use dashmap::DashMap;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::protocol::{DeviceType, PeerData, ServerMessage};

/// Channel sender type used to push messages to a connected peer's WebSocket.
pub type PeerSender = mpsc::UnboundedSender<ServerMessage>;

/// Information about a connected peer stored in a room.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Unique peer code chosen by the client.
    pub peer_code: String,
    /// Human-readable device name.
    pub device_name: String,
    /// Device category (phone, tablet, laptop, desktop).
    pub device_type: DeviceType,
    /// Channel for sending messages to this peer's WebSocket write task.
    pub sender: PeerSender,
}

impl PeerInfo {
    /// Convert to the public [`PeerData`] representation (without the sender).
    pub fn to_peer_data(&self) -> PeerData {
        PeerData {
            peer_code: self.peer_code.clone(),
            device_name: self.device_name.clone(),
            device_type: self.device_type.clone(),
        }
    }
}

/// Manages rooms keyed by client IP address.
///
/// Each room contains a list of [`PeerInfo`] entries representing the peers
/// currently connected from that IP. All methods are safe to call concurrently
/// from multiple tasks.
pub struct RoomManager {
    rooms: DashMap<String, Vec<PeerInfo>>,
}

impl RoomManager {
    /// Create a new, empty room manager.
    pub fn new() -> Self {
        Self {
            rooms: DashMap::new(),
        }
    }

    /// Add a peer to the room for the given IP address.
    ///
    /// Returns the list of peers that were **already** in the room (before this
    /// peer was added), so the caller can send the initial peer list to the
    /// newly registered client.
    ///
    /// Also broadcasts a `peer_joined` message to every existing peer in the room.
    pub fn add_peer(&self, ip: &str, peer: PeerInfo) -> Result<Vec<PeerData>, String> {
        let peer_data = peer.to_peer_data();
        let mut existing_peers = Vec::new();

        let mut room = self.rooms.entry(ip.to_string()).or_default();

        // Reject duplicate peer codes within the same room.
        if room.iter().any(|p| p.peer_code == peer.peer_code) {
            return Err(format!("Peer code '{}' already in use", peer.peer_code));
        }

        // Snapshot existing peers for the "peers" response.
        for p in room.iter() {
            existing_peers.push(p.to_peer_data());
        }

        // Broadcast peer_joined to existing room members.
        let join_msg = ServerMessage::PeerJoined {
            peer: peer_data.clone(),
        };
        for p in room.iter() {
            if p.sender.send(join_msg.clone()).is_err() {
                debug!(peer_code = %p.peer_code, "failed to send peer_joined (receiver dropped)");
            }
        }

        info!(
            ip = %ip,
            peer_code = %peer.peer_code,
            device_name = %peer.device_name,
            room_size = room.len() + 1,
            "peer joined room"
        );

        room.push(peer);
        Ok(existing_peers)
    }

    /// Remove a peer from the room for the given IP address.
    ///
    /// Broadcasts a `peer_left` message to all remaining peers in the room.
    /// Cleans up the room entry if it becomes empty.
    pub fn remove_peer(&self, ip: &str, peer_code: &str) {
        let should_remove_room = {
            if let Some(mut room) = self.rooms.get_mut(ip) {
                room.retain(|p| p.peer_code != peer_code);

                // Broadcast peer_left to remaining peers.
                let leave_msg = ServerMessage::PeerLeft {
                    peer_code: peer_code.to_string(),
                };
                for p in room.iter() {
                    if p.sender.send(leave_msg.clone()).is_err() {
                        debug!(peer_code = %p.peer_code, "failed to send peer_left (receiver dropped)");
                    }
                }

                info!(
                    ip = %ip,
                    peer_code = %peer_code,
                    room_size = room.len(),
                    "peer left room"
                );

                room.is_empty()
            } else {
                false
            }
        };

        if should_remove_room {
            self.rooms.remove(ip);
            debug!(ip = %ip, "room cleaned up (empty)");
        }
    }

    /// Get the public peer data for all peers in the room at the given IP.
    pub fn get_room_peers(&self, ip: &str) -> Vec<PeerData> {
        self.rooms
            .get(ip)
            .map(|room| room.iter().map(|p| p.to_peer_data()).collect())
            .unwrap_or_default()
    }

    /// Look up a peer's sender channel by peer code across all rooms.
    ///
    /// This performs a linear scan; acceptable for the expected small number of
    /// peers per deployment.
    pub fn find_peer(&self, peer_code: &str) -> Option<PeerSender> {
        for room in self.rooms.iter() {
            for peer in room.value().iter() {
                if peer.peer_code == peer_code {
                    return Some(peer.sender.clone());
                }
            }
        }
        None
    }

    /// Return the total number of active rooms (unique IPs with at least one peer).
    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    /// Return the total number of connected peers across all rooms.
    pub fn peer_count(&self) -> usize {
        self.rooms.iter().map(|r| r.value().len()).sum()
    }
}

impl Default for RoomManager {
    fn default() -> Self {
        Self::new()
    }
}

// ServerMessage needs Clone for broadcasting.
impl Clone for ServerMessage {
    fn clone(&self) -> Self {
        // We serialize and deserialize to avoid manual field cloning for the
        // serde_json::Value payload. This only happens on broadcast fan-out,
        // which is infrequent and low-volume.
        let json = serde_json::to_string(self).expect("ServerMessage serialization");
        serde_json::from_str(&json).unwrap_or_else(|_| ServerMessage::Error {
            message: "internal clone error".into(),
        })
    }
}

// ServerMessage needs Deserialize only for the Clone impl above.
impl<'de> serde::Deserialize<'de> for ServerMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Use an intermediate representation to avoid infinite recursion.
        let value = serde_json::Value::deserialize(deserializer)?;
        let msg_type = value
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| serde::de::Error::custom("missing type field"))?;

        match msg_type {
            "peers" => {
                let peers: Vec<PeerData> = serde_json::from_value(
                    value
                        .get("peers")
                        .cloned()
                        .unwrap_or(serde_json::Value::Array(vec![])),
                )
                .map_err(serde::de::Error::custom)?;
                Ok(ServerMessage::Peers { peers })
            }
            "peer_joined" => {
                let peer: PeerData =
                    serde_json::from_value(value.get("peer").cloned().unwrap_or_default())
                        .map_err(serde::de::Error::custom)?;
                Ok(ServerMessage::PeerJoined { peer })
            }
            "peer_left" => {
                let peer_code = value
                    .get("peer_code")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                Ok(ServerMessage::PeerLeft { peer_code })
            }
            "signal" => {
                let from = value
                    .get("from")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let payload = value.get("payload").cloned().unwrap_or_default();
                Ok(ServerMessage::Signal { from, payload })
            }
            "error" => {
                let message = value
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                Ok(ServerMessage::Error { message })
            }
            other => Err(serde::de::Error::custom(format!(
                "unknown message type: {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    /// Create a PeerInfo with a channel, returning (PeerInfo, receiver).
    fn make_peer(code: &str, name: &str) -> (PeerInfo, mpsc::UnboundedReceiver<ServerMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let peer = PeerInfo {
            peer_code: code.to_string(),
            device_name: name.to_string(),
            device_type: DeviceType::Desktop,
            sender: tx,
        };
        (peer, rx)
    }

    // ─── add_peer ───────────────────────────────────────────────────────

    #[test]
    fn add_peer_inserts_into_correct_room() {
        let rm = RoomManager::new();
        let (peer, _rx) = make_peer("ALPHA", "Desktop A");

        let result = rm.add_peer("192.168.1.10", peer);
        assert!(result.is_ok());

        // First peer in room → existing list is empty
        assert!(result.unwrap().is_empty());

        assert_eq!(rm.room_count(), 1);
        assert_eq!(rm.peer_count(), 1);

        let peers = rm.get_room_peers("192.168.1.10");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].peer_code, "ALPHA");
    }

    #[test]
    fn add_peer_returns_existing_peers_before_insert() {
        let rm = RoomManager::new();
        let (p1, _r1) = make_peer("AAA", "First");
        let (p2, _r2) = make_peer("BBB", "Second");

        rm.add_peer("10.0.0.1", p1).unwrap();
        let existing = rm.add_peer("10.0.0.1", p2).unwrap();

        // Second peer should see the first peer in the existing list
        assert_eq!(existing.len(), 1);
        assert_eq!(existing[0].peer_code, "AAA");
    }

    #[test]
    fn add_peer_rejects_duplicate_peer_code() {
        let rm = RoomManager::new();
        let (p1, _r1) = make_peer("DUP", "First");
        let (p2, _r2) = make_peer("DUP", "Second");

        assert!(rm.add_peer("10.0.0.1", p1).is_ok());
        let err = rm.add_peer("10.0.0.1", p2);
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("already in use"));

        // Only 1 peer in room
        assert_eq!(rm.peer_count(), 1);
    }

    #[test]
    fn add_peer_same_code_different_rooms_allowed() {
        let rm = RoomManager::new();
        let (p1, _r1) = make_peer("SAME", "Room A");
        let (p2, _r2) = make_peer("SAME", "Room B");

        assert!(rm.add_peer("10.0.0.1", p1).is_ok());
        assert!(rm.add_peer("10.0.0.2", p2).is_ok());

        assert_eq!(rm.room_count(), 2);
        assert_eq!(rm.peer_count(), 2);
    }

    #[test]
    fn add_peer_broadcasts_peer_joined_to_existing() {
        let rm = RoomManager::new();
        let (p1, mut rx1) = make_peer("FIRST", "Device 1");
        let (p2, _r2) = make_peer("SECOND", "Device 2");

        rm.add_peer("10.0.0.1", p1).unwrap();
        rm.add_peer("10.0.0.1", p2).unwrap();

        // FIRST should have received a PeerJoined for SECOND
        let msg = rx1.try_recv().expect("should have received PeerJoined");
        match msg {
            ServerMessage::PeerJoined { peer } => {
                assert_eq!(peer.peer_code, "SECOND");
            }
            other => panic!("expected PeerJoined, got: {:?}", serde_json::to_string(&other)),
        }
    }

    // ─── remove_peer ────────────────────────────────────────────────────

    #[test]
    fn remove_peer_removes_from_room() {
        let rm = RoomManager::new();
        let (p1, _r1) = make_peer("RM1", "Device 1");
        let (p2, _r2) = make_peer("RM2", "Device 2");

        rm.add_peer("10.0.0.1", p1).unwrap();
        rm.add_peer("10.0.0.1", p2).unwrap();
        assert_eq!(rm.peer_count(), 2);

        rm.remove_peer("10.0.0.1", "RM1");

        assert_eq!(rm.peer_count(), 1);
        let peers = rm.get_room_peers("10.0.0.1");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].peer_code, "RM2");
    }

    #[test]
    fn remove_peer_cleans_up_empty_room() {
        let rm = RoomManager::new();
        let (p1, _r1) = make_peer("SOLO", "Only One");

        rm.add_peer("10.0.0.1", p1).unwrap();
        assert_eq!(rm.room_count(), 1);

        rm.remove_peer("10.0.0.1", "SOLO");

        assert_eq!(rm.room_count(), 0);
        assert_eq!(rm.peer_count(), 0);
    }

    #[test]
    fn remove_peer_broadcasts_peer_left() {
        let rm = RoomManager::new();
        let (stay, mut stay_rx) = make_peer("STAY", "Stayer");
        let (leave, _leave_rx) = make_peer("LEAVE", "Leaver");

        rm.add_peer("10.0.0.1", stay).unwrap();
        rm.add_peer("10.0.0.1", leave).unwrap();

        // Drain PeerJoined from STAY's channel
        let _ = stay_rx.try_recv();

        rm.remove_peer("10.0.0.1", "LEAVE");

        let msg = stay_rx.try_recv().expect("should have received PeerLeft");
        match msg {
            ServerMessage::PeerLeft { peer_code } => {
                assert_eq!(peer_code, "LEAVE");
            }
            other => panic!("expected PeerLeft, got: {:?}", serde_json::to_string(&other)),
        }
    }

    #[test]
    fn remove_peer_nonexistent_does_not_panic() {
        let rm = RoomManager::new();
        let (p1, _r1) = make_peer("EXISTS", "Real");

        rm.add_peer("10.0.0.1", p1).unwrap();

        // Remove a peer that doesn't exist in the room
        rm.remove_peer("10.0.0.1", "GHOST");
        assert_eq!(rm.peer_count(), 1);

        // Remove from a room that doesn't exist
        rm.remove_peer("10.0.0.99", "GHOST");
        assert_eq!(rm.peer_count(), 1);
    }

    // ─── find_peer ──────────────────────────────────────────────────────

    #[test]
    fn find_peer_returns_sender_for_existing_peer() {
        let rm = RoomManager::new();
        let (p1, _r1) = make_peer("FINDME", "Device");

        rm.add_peer("10.0.0.1", p1).unwrap();

        let sender = rm.find_peer("FINDME");
        assert!(sender.is_some());
    }

    #[test]
    fn find_peer_returns_none_for_absent_peer() {
        let rm = RoomManager::new();
        assert!(rm.find_peer("NOBODY").is_none());
    }

    #[test]
    fn find_peer_works_across_rooms() {
        let rm = RoomManager::new();
        let (p1, _r1) = make_peer("ROOM1PEER", "Room 1");
        let (p2, _r2) = make_peer("ROOM2PEER", "Room 2");

        rm.add_peer("10.0.0.1", p1).unwrap();
        rm.add_peer("10.0.0.2", p2).unwrap();

        // find_peer scans all rooms
        assert!(rm.find_peer("ROOM1PEER").is_some());
        assert!(rm.find_peer("ROOM2PEER").is_some());
        assert!(rm.find_peer("MISSING").is_none());
    }

    // ─── Concurrent edge simulation ─────────────────────────────────────

    #[test]
    fn peer_a_disconnect_does_not_affect_peer_b() {
        let rm = RoomManager::new();
        let (pa, _ra) = make_peer("PEERA", "Device A");
        let (pb, _rb) = make_peer("PEERB", "Device B");

        rm.add_peer("10.0.0.1", pa).unwrap();
        rm.add_peer("10.0.0.1", pb).unwrap();
        assert_eq!(rm.peer_count(), 2);

        // A disconnects
        rm.remove_peer("10.0.0.1", "PEERA");

        // B is still intact
        assert_eq!(rm.peer_count(), 1);
        assert!(rm.find_peer("PEERB").is_some());
        assert!(rm.find_peer("PEERA").is_none());

        let peers = rm.get_room_peers("10.0.0.1");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].peer_code, "PEERB");

        // Room still exists (not empty)
        assert_eq!(rm.room_count(), 1);
    }

    #[test]
    fn multi_room_isolation() {
        let rm = RoomManager::new();
        let (p1, _r1) = make_peer("R1A", "Room1 A");
        let (p2, _r2) = make_peer("R1B", "Room1 B");
        let (p3, _r3) = make_peer("R2A", "Room2 A");

        rm.add_peer("10.0.0.1", p1).unwrap();
        rm.add_peer("10.0.0.1", p2).unwrap();
        rm.add_peer("10.0.0.2", p3).unwrap();

        assert_eq!(rm.room_count(), 2);
        assert_eq!(rm.peer_count(), 3);

        // Remove all peers from room 1
        rm.remove_peer("10.0.0.1", "R1A");
        rm.remove_peer("10.0.0.1", "R1B");

        // Room 1 cleaned up, room 2 intact
        assert_eq!(rm.room_count(), 1);
        assert_eq!(rm.peer_count(), 1);
        assert!(rm.find_peer("R2A").is_some());
        assert!(rm.get_room_peers("10.0.0.1").is_empty());
    }

    // ─── Invalid room access ────────────────────────────────────────────

    #[test]
    fn get_room_peers_nonexistent_returns_empty() {
        let rm = RoomManager::new();
        let peers = rm.get_room_peers("172.16.0.99");
        assert!(peers.is_empty());
    }

    #[test]
    fn empty_manager_counts_are_zero() {
        let rm = RoomManager::new();
        assert_eq!(rm.room_count(), 0);
        assert_eq!(rm.peer_count(), 0);
    }

    // ─── get_room_peers ─────────────────────────────────────────────────

    #[test]
    fn get_room_peers_returns_public_data() {
        let rm = RoomManager::new();
        let (p1, _r1) = make_peer("PUB1", "Device 1");
        let (p2, _r2) = make_peer("PUB2", "Device 2");

        rm.add_peer("10.0.0.1", p1).unwrap();
        rm.add_peer("10.0.0.1", p2).unwrap();

        let peers = rm.get_room_peers("10.0.0.1");
        assert_eq!(peers.len(), 2);

        let codes: Vec<&str> = peers.iter().map(|p| p.peer_code.as_str()).collect();
        assert!(codes.contains(&"PUB1"));
        assert!(codes.contains(&"PUB2"));
    }
}
