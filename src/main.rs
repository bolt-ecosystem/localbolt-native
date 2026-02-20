//! LocalBolt Signaling Server binary entry point.
//!
//! Starts the WebSocket signaling server with configurable host and port via
//! command-line arguments.

use std::net::SocketAddr;

use localbolt_signal::SignalingServer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Initialize tracing with RUST_LOG env filter support.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Parse CLI arguments (simple manual parsing — no clap dependency needed).
    let args: Vec<String> = std::env::args().collect();
    let host = get_arg(&args, "--host").unwrap_or_else(|| "0.0.0.0".to_string());
    let port = get_arg(&args, "--port")
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3001);

    let addr: SocketAddr = format!("{host}:{port}").parse().unwrap_or_else(|e| {
        eprintln!("invalid address '{host}:{port}': {e}");
        std::process::exit(1);
    });

    let server = SignalingServer::new(addr);

    if let Err(e) = server.run().await {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}

/// Extract the value following a `--key` argument.
fn get_arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
