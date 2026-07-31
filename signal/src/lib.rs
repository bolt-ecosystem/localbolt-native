//! LocalBolt Signaling Server library.
//!
//! Provides a reusable [`SignalingServer`] that can be embedded inside a native
//! application or run as a standalone binary. The server groups peers by IP
//! address for local-network device discovery and relays WebRTC signaling
//! messages between them.
//!
//! # Example
//!
//! ```rust,no_run
//! use bolt_rendezvous::SignalingServer;
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

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::{error, info, warn};

use room::RoomManager;
use server::handle_connection;

/// Default maximum concurrent WebSocket connections.
/// Fail-closed: once this limit is reached, new connections receive HTTP 503.
pub const DEFAULT_MAX_WS_CONNECTIONS: usize = 256;

/// RAII guard that decrements the active connection counter on drop.
///
/// Created by [`SignalingServer`] when a connection slot is acquired.
/// The counter is decremented exactly once when the guard is dropped,
/// regardless of whether the connection handler completes normally or panics.
pub struct ConnectionGuard {
    active: Arc<AtomicUsize>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::Release);
    }
}

/// A WebSocket signaling server for LocalBolt P2P file transfer.
///
/// The server listens for incoming WebSocket connections, groups peers by their
/// originating IP address, and relays WebRTC signaling messages between peers
/// in the same IP room.
///
/// ## Connection Limit (AC-22)
///
/// The server enforces a hard concurrent WebSocket connection limit
/// (default: [`DEFAULT_MAX_WS_CONNECTIONS`]). When the limit is reached,
/// new TCP connections are accepted but immediately closed — the connection
/// slot is never consumed. Configure via [`with_max_connections`](Self::with_max_connections)
/// or the `MAX_WS_CONNECTIONS` environment variable at startup.
pub struct SignalingServer {
    addr: SocketAddr,
    room_manager: Arc<RoomManager>,
    trusted_proxies: Arc<Vec<IpAddr>>,
    max_connections: usize,
    active_connections: Arc<AtomicUsize>,
}

impl SignalingServer {
    /// Create a new signaling server bound to the given address.
    ///
    /// By default, `X-Forwarded-For` is ignored (empty trusted proxy list)
    /// and the connection limit is [`DEFAULT_MAX_WS_CONNECTIONS`].
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            room_manager: Arc::new(RoomManager::new()),
            trusted_proxies: Arc::new(Vec::new()),
            max_connections: DEFAULT_MAX_WS_CONNECTIONS,
            active_connections: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Configure trusted proxy addresses whose `X-Forwarded-For` headers
    /// will be honored for room assignment. Addresses not in this list
    /// always use the socket address (fail-closed).
    pub fn with_trusted_proxies(mut self, proxies: Vec<IpAddr>) -> Self {
        self.trusted_proxies = Arc::new(proxies);
        self
    }

    /// Set the maximum number of concurrent WebSocket connections.
    ///
    /// When this limit is reached, new connections are rejected (TCP stream
    /// dropped without WebSocket upgrade). A value of 0 rejects all connections.
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    /// Try to acquire a connection slot. Returns a [`ConnectionGuard`] on
    /// success, or `None` if the limit has been reached.
    ///
    /// Uses `compare_exchange` in a loop to atomically check-and-increment,
    /// ensuring no over-subscription.
    fn try_acquire_slot(&self) -> Option<ConnectionGuard> {
        loop {
            let current = self.active_connections.load(Ordering::Acquire);
            if current >= self.max_connections {
                return None;
            }
            match self.active_connections.compare_exchange(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Some(ConnectionGuard {
                        active: self.active_connections.clone(),
                    });
                }
                Err(_) => continue, // CAS retry
            }
        }
    }

    /// Run the signaling server, accepting connections until the process is terminated.
    ///
    /// This method binds a TCP listener and spawns a task for each incoming
    /// connection. It runs indefinitely and only returns on a fatal bind/accept error.
    ///
    /// Connections beyond [`max_connections`](Self::with_max_connections) are
    /// rejected by dropping the TCP stream immediately (no WebSocket upgrade).
    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(self.addr).await?;

        info!(
            addr = %self.addr,
            max_connections = self.max_connections,
            "LocalBolt signaling server listening on {}",
            self.addr
        );

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    // Acquire a connection slot before spawning the handler.
                    // If the limit is reached, drop the TCP stream immediately.
                    let guard = match self.try_acquire_slot() {
                        Some(g) => g,
                        None => {
                            warn!(
                                addr = %addr,
                                active = self.active_connections.load(Ordering::Relaxed),
                                max = self.max_connections,
                                "connection rejected — limit reached"
                            );
                            // Drop `stream` — TCP RST to client.
                            drop(stream);
                            continue;
                        }
                    };

                    let room_manager = self.room_manager.clone();
                    let trusted_proxies = self.trusted_proxies.clone();
                    tokio::spawn(async move {
                        handle_connection(stream, addr, room_manager, trusted_proxies).await;
                        // `guard` is moved into this future and dropped here,
                        // releasing the connection slot.
                        drop(guard);
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

    /// Current number of active connections.
    pub fn active_connections(&self) -> usize {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Configured maximum connections.
    pub fn max_connections(&self) -> usize {
        self.max_connections
    }
}

// ── Connection Limit Tests (AC-22) ──────────────────────────────────────

#[cfg(test)]
mod connection_limit_tests {
    use super::*;

    fn test_server(max: usize) -> SignalingServer {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        SignalingServer::new(addr).with_max_connections(max)
    }

    #[test]
    fn limit_zero_rejects_all() {
        let server = test_server(0);
        assert!(server.try_acquire_slot().is_none());
        assert_eq!(server.active_connections(), 0);
    }

    #[test]
    fn limit_one_allows_first_rejects_second() {
        let server = test_server(1);

        let guard = server.try_acquire_slot();
        assert!(guard.is_some());
        assert_eq!(server.active_connections(), 1);

        // Second acquisition must fail while first is held.
        assert!(server.try_acquire_slot().is_none());
        assert_eq!(server.active_connections(), 1);

        // Hold guard to prevent drop before assertions.
        drop(guard);
    }

    #[test]
    fn disconnect_releases_slot() {
        let server = test_server(1);

        // Acquire and release.
        let guard = server.try_acquire_slot().unwrap();
        assert_eq!(server.active_connections(), 1);
        drop(guard);
        assert_eq!(server.active_connections(), 0);

        // Slot should be available again.
        let guard2 = server.try_acquire_slot();
        assert!(guard2.is_some());
        assert_eq!(server.active_connections(), 1);
        drop(guard2);
    }

    #[test]
    fn default_limit_nonzero() {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let server = SignalingServer::new(addr);
        assert_eq!(server.max_connections(), DEFAULT_MAX_WS_CONNECTIONS);
        assert!(DEFAULT_MAX_WS_CONNECTIONS > 0);
    }
}
