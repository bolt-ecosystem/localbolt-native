//! LocalBolt — GTK4/libadwaita shell for Linux and Steam Deck.
//!
//! A thin native UI over `bolt-app-core` (the shared Rust app runtime used by the
//! CLI and the SwiftUI shell). This shell owns no protocol logic: it starts the
//! daemon via `DaemonLifecycle`, receives IPC/bridge events, and sends decisions.
//!
//! v2 scope: the **acceptor flow** — when another device connects to this one,
//! confirm the SAS, accept an incoming file, and watch it transfer. (Initiating a
//! connection from this device — discovery + Connect — lands next.)

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use adw::prelude::*;
use bolt_app_core::daemon_lifecycle::DaemonLifecycle;
use bolt_app_core::ipc_bridge_core::IpcBridgeCore;
use bolt_app_core::ipc_types::{
    Decision, IpcMessage, PairingDecisionPayload, PairingRequestPayload,
    TransferIncomingDecisionPayload, TransferIncomingRequestPayload,
};
use gtk::glib;

const APP_ID: &str = "com.the9ines.LocalBolt";
const APP_VERSION: &str = "0.1.0";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    // The daemon's bridge callback runs on a background thread; forward its events
    // to the GTK main loop where widgets can be touched safely.
    let (tx, rx) = async_channel::unbounded::<(String, serde_json::Value)>();

    // ── Widgets ──────────────────────────────────────────────
    let status = adw::StatusPage::builder()
        .icon_name("network-transmit-receive-symbolic")
        .title("Starting…")
        .description("Launching the LocalBolt engine")
        .vexpand(true)
        .build();

    let progress = gtk::ProgressBar::builder()
        .show_text(true)
        .text("")
        .visible(false)
        .margin_start(24)
        .margin_end(24)
        .build();

    let credit = gtk::Label::builder()
        .label("by the9ines")
        .css_classes(["dim-label", "caption"])
        .halign(gtk::Align::Center)
        .margin_top(4)
        .margin_bottom(8)
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.append(&status);
    content.append(&progress);
    content.append(&credit);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&content));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("LocalBolt")
        .default_width(460)
        .default_height(560)
        .content(&toolbar)
        .build();

    // ── Daemon lifecycle ─────────────────────────────────────
    let mut lifecycle = DaemonLifecycle::new(APP_VERSION);
    lifecycle.add_binary_search_paths(dev_daemon_paths());
    let lifecycle = Arc::new(lifecycle);
    let bridge = Arc::clone(&lifecycle.bridge);

    let tx_bridge = tx.clone();
    lifecycle.set_bridge_event_callback(Box::new(move |name, payload| {
        let _ = tx_bridge.send_blocking((name.to_string(), payload));
    }));
    let tx_watchdog = tx.clone();
    lifecycle.set_watchdog_callback(Box::new(move |ev| {
        let _ = tx_watchdog.send_blocking((
            "watchdog".to_string(),
            serde_json::json!({ "state": format!("{:?}", ev.state) }),
        ));
    }));
    lifecycle.start();

    let lc = Arc::clone(&lifecycle);
    window.connect_close_request(move |_| {
        lc.shutdown_flag().store(true, Ordering::Relaxed);
        glib::Propagation::Proceed
    });

    // ── Event loop (main thread) ─────────────────────────────
    let win = window.clone();
    glib::spawn_future_local(async move {
        while let Ok((name, payload)) = rx.recv().await {
            match name.as_str() {
                "watchdog" => {
                    if payload["state"] == "Ready" {
                        idle(&status, "Ready", "Waiting for a device to connect");
                    }
                }
                "daemon://pairing-request" => {
                    if let Ok(req) = serde_json::from_value::<PairingRequestPayload>(payload) {
                        show_pairing_dialog(&win, &bridge, req);
                    }
                }
                "daemon://transfer-request" => {
                    if let Ok(req) =
                        serde_json::from_value::<TransferIncomingRequestPayload>(payload)
                    {
                        show_transfer_dialog(&win, &bridge, req);
                    }
                }
                "daemon://session-connected" => {
                    idle(&status, "Connected", "Secure channel established");
                }
                "daemon://transfer-started" => {
                    let file = payload["file_name"].as_str().unwrap_or("file").to_string();
                    status.set_title("Receiving…");
                    status.set_description(Some(&file));
                    status.set_icon_name(Some("folder-download-symbolic"));
                    progress.set_fraction(0.0);
                    progress.set_text(Some(&file));
                    progress.set_visible(true);
                }
                "daemon://transfer-progress" => {
                    if let Some(f) = payload["progress"].as_f64() {
                        progress.set_fraction(f.clamp(0.0, 1.0));
                    }
                }
                "daemon://transfer-complete" => {
                    let file = payload["file_name"].as_str().unwrap_or("file");
                    progress.set_visible(false);
                    idle(
                        &status,
                        "Received",
                        &format!("Saved {file} to your Downloads"),
                    );
                    status.set_icon_name(Some("emblem-ok-symbolic"));
                }
                "daemon://transfer-error" | "daemon://session-error" => {
                    progress.set_visible(false);
                    idle(&status, "Something went wrong", "The transfer didn't finish");
                }
                _ => {}
            }
        }
    });

    window.present();
}

/// Reset the main view to an idle status message.
fn idle(status: &adw::StatusPage, title: &str, description: &str) {
    status.set_title(title);
    status.set_description(Some(description));
    status.set_icon_name(Some("network-wireless-symbolic"));
}

/// Prompt to confirm a pairing request — the user verifies the SAS matches the
/// code shown on the other device, then accepts or declines.
fn show_pairing_dialog(
    window: &adw::ApplicationWindow,
    bridge: &Arc<IpcBridgeCore>,
    req: PairingRequestPayload,
) {
    let body = format!(
        "{} wants to connect.\n\nConfirm this code matches on both screens:\n\n{}",
        req.remote_device_name, req.sas
    );
    let dialog = adw::AlertDialog::new(Some("Connection request"), Some(&body));
    dialog.add_response("decline", "Decline");
    dialog.add_response("accept", "Connect");
    dialog.set_response_appearance("accept", adw::ResponseAppearance::Suggested);
    dialog.set_response_appearance("decline", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("accept"));
    dialog.set_close_response("decline");

    let bridge = Arc::clone(bridge);
    dialog.connect_response(None, move |_, response| {
        let decision = if response == "accept" {
            Decision::AllowOnce
        } else {
            Decision::DenyOnce
        };
        let payload = serde_json::to_value(PairingDecisionPayload {
            request_id: req.request_id.clone(),
            decision,
            note: None,
        })
        .unwrap_or_default();
        send(&bridge, "pairing.decision", payload);
    });
    dialog.present(Some(window));
}

/// Prompt to accept an incoming file.
fn show_transfer_dialog(
    window: &adw::ApplicationWindow,
    bridge: &Arc<IpcBridgeCore>,
    req: TransferIncomingRequestPayload,
) {
    let body = format!(
        "{} wants to send you:\n\n{}  ({})",
        req.from_device_name,
        req.file_name,
        human_size(req.file_size_bytes)
    );
    let dialog = adw::AlertDialog::new(Some("Incoming file"), Some(&body));
    dialog.add_response("decline", "Decline");
    dialog.add_response("accept", "Accept");
    dialog.set_response_appearance("accept", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("accept"));
    dialog.set_close_response("decline");

    let bridge = Arc::clone(bridge);
    dialog.connect_response(None, move |_, response| {
        let decision = if response == "accept" {
            Decision::AllowOnce
        } else {
            Decision::DenyOnce
        };
        let payload = serde_json::to_value(TransferIncomingDecisionPayload {
            request_id: req.request_id.clone(),
            decision,
            note: None,
        })
        .unwrap_or_default();
        send(&bridge, "transfer.incoming.decision", payload);
    });
    dialog.present(Some(window));
}

/// Send a decision to the daemon over the bridge.
fn send(bridge: &Arc<IpcBridgeCore>, msg_type: &str, payload: serde_json::Value) {
    if let Err(e) = bridge.send_decision(IpcMessage::new_decision(msg_type, payload)) {
        eprintln!("[localbolt] failed to send {msg_type}: {e}");
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// Dev-only daemon locations (a Flatpak bundles the daemon on `PATH`).
fn dev_daemon_paths() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../bolt-daemon/target");
    vec![root.join("release/bolt-daemon"), root.join("debug/bolt-daemon")]
}
