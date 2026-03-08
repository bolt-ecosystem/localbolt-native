mod commands;
mod daemon;
mod daemon_log;
mod ipc_bridge;
mod ipc_client;
mod ipc_transport;
mod ipc_types;
mod platform;
mod watchdog;

use bolt_rendezvous::SignalingServer;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

use daemon::DaemonManager;
use tauri::Manager;

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

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize daemon manager with AppHandle for event emission
            let mut manager = DaemonManager::new();
            manager.set_app_handle(app.handle().clone());
            let manager = Arc::new(manager);
            manager.start();
            app.manage(manager);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_watchdog_state,
            commands::restart_daemon,
            commands::send_pairing_decision,
            commands::send_transfer_decision,
            commands::export_support_bundle,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(mgr) = window.try_state::<Arc<DaemonManager>>() {
                    let mgr: &Arc<DaemonManager> = &mgr;
                    mgr.shutdown();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
