//! Bolt Rendezvous Server binary entry point.
//!
//! Starts the WebSocket signaling server with configurable host and port.
//!
//! ## Configuration Resolution Order
//!
//! CLI arguments (`--host`, `--port`) take highest priority, followed by
//! environment variables (`BOLT_SIGNAL_HOST`, `BOLT_SIGNAL_PORT`), then
//! profile defaults (`BOLT_SIGNAL_PROFILE`), then hardcoded defaults.
//!
//! ## Profiles
//!
//! | Profile | Log Level | Notes |
//! |---------|-----------|-------|
//! | `local` | `info` | LAN/dev — verbose logging |
//! | `internet` | `warn` | Public deployment — quieter |
//! | *(unset)* | `info` | Same as `local` |
//!
//! `RUST_LOG` always overrides the profile log level when set.

use std::net::SocketAddr;

use bolt_rendezvous::SignalingServer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Resolve profile (local | internet | unset).
    let profile = std::env::var("BOLT_SIGNAL_PROFILE").ok();
    let default_log = match profile.as_deref() {
        Some("internet") => "warn",
        _ => "info", // local, unset, or unrecognized → info
    };

    // Initialize tracing: RUST_LOG overrides profile default.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_log)),
        )
        .init();

    if let Some(ref p) = profile {
        match p.as_str() {
            "local" | "internet" => {
                tracing::info!(profile = %p, "BOLT_SIGNAL_PROFILE active");
            }
            other => {
                tracing::warn!(
                    profile = %other,
                    "unknown BOLT_SIGNAL_PROFILE value — using defaults"
                );
            }
        }
    }

    // Parse CLI arguments (simple manual parsing — no clap dependency needed).
    let args: Vec<String> = std::env::args().collect();

    // Resolution: CLI > env var > hardcoded default.
    let host = get_arg(&args, "--host")
        .or_else(|| std::env::var("BOLT_SIGNAL_HOST").ok())
        .unwrap_or_else(|| "0.0.0.0".to_string());
    let port = get_arg(&args, "--port")
        .and_then(|p| p.parse::<u16>().ok())
        .or_else(|| {
            std::env::var("BOLT_SIGNAL_PORT")
                .ok()
                .and_then(|p| p.parse::<u16>().ok())
        })
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
