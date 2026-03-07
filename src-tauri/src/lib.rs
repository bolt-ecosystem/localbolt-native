mod commands;
mod daemon;
mod daemon_log;
mod ipc_client;
mod ipc_types;
mod watchdog;

use bolt_rendezvous::SignalingServer;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

use crate::daemon::DaemonManager;

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

    // Start signal server before the UI (unchanged from pre-N6)
    start_embedded_signal_server();

    // Initialize daemon manager and start lifecycle
    let manager = Arc::new(DaemonManager::new());
    manager.start();

    let manager_for_exit = Arc::clone(&manager);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(manager)
        .invoke_handler(tauri::generate_handler![
            commands::get_watchdog_state,
            commands::restart_daemon,
            commands::export_support_bundle,
        ])
        .on_window_event(move |_window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                manager_for_exit.shutdown();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
