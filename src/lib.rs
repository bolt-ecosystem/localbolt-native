//! LocalBolt Signaling Server library.
//!
//! Provides a reusable [`SignalingServer`] that can be embedded inside a Tauri
//! application or run as a standalone binary. The server groups peers by IP
//! address for local-network device discovery and relays WebRTC signaling
//! messages between them.
//!
//! # Example
//!
//! ```rust,no_run
//! use localbolt_signal::SignalingServer;
//! use std::net::SocketAddr;
//!
//! #[tokio::main]
//! async fn main() {
//!     let addr: SocketAddr = "0.0.0.0:3001".parse().unwrap();
//!     let server = SignalingServer::new(addr);
//!     server.run().await.unwrap();
//! }
//! ```

pub mod protocol;
pub mod room;
pub mod server;

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::{error, info};

use room::RoomManager;
use server::handle_connection;

/// A WebSocket signaling server for LocalBolt P2P file transfer.
///
/// The server listens for incoming WebSocket connections, groups peers by their
/// originating IP address, and relays WebRTC signaling messages between peers
/// in the same IP room.
pub struct SignalingServer {
    addr: SocketAddr,
    room_manager: Arc<RoomManager>,
}

impl SignalingServer {
    /// Create a new signaling server bound to the given address.
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            room_manager: Arc::new(RoomManager::new()),
        }
    }

    /// Run the signaling server, accepting connections until the process is terminated.
    ///
    /// This method binds a TCP listener and spawns a task for each incoming
    /// connection. It runs indefinitely and only returns on a fatal bind/accept error.
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.addr).await?;

        info!(
            addr = %self.addr,
            "LocalBolt signaling server listening on {}",
            self.addr
        );

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let room_manager = self.room_manager.clone();
                    tokio::spawn(async move {
                        handle_connection(stream, addr, room_manager).await;
                    });
                }
                Err(e) => {
                    error!(error = %e, "failed to accept connection");
                }
            }
        }
    }

    /// Get a reference to the room manager.
    ///
    /// Useful for inspecting server state (e.g., active rooms, peer counts)
    /// when embedding the server inside a larger application.
    pub fn room_manager(&self) -> Arc<RoomManager> {
        self.room_manager.clone()
    }
}
