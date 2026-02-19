use localbolt_signal::SignalingServer;
use std::net::SocketAddr;
use tracing_subscriber::EnvFilter;

/// Spawn the embedded signaling server on a background thread.
///
/// Runs on 0.0.0.0:3001 so other devices on the LAN can connect.
fn start_embedded_signal_server() {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
        rt.block_on(async {
            let addr: SocketAddr = "0.0.0.0:3001".parse().unwrap();
            let server = SignalingServer::new(addr);
            if let Err(e) = server.run().await {
                eprintln!("[signal] server error: {e}");
            }
        });
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Start signal server before the UI
    start_embedded_signal_server();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
