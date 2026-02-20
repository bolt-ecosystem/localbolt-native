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
