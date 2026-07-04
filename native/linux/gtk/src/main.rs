//! LocalBolt — GTK4/libadwaita shell for Linux and Steam Deck.
//!
//! A thin native UI over `bolt-app-core` (the shared Rust app runtime used by the
//! CLI and the SwiftUI shell). This shell owns no protocol logic: it starts the
//! daemon via `DaemonLifecycle`, receives IPC/bridge events, and renders them.
//!
//! v1 scope: launch the engine, show its state, and surface live events (peers,
//! pairing, SAS, transfers). Interactive connect / SAS-confirm / send land next.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use adw::prelude::*;
use bolt_app_core::daemon_lifecycle::DaemonLifecycle;
use gtk::glib;

const APP_ID: &str = "com.the9ines.LocalBolt";
const APP_VERSION: &str = "0.1.0";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    // Channel: the daemon's bridge callback runs on a background thread; forward
    // its events to the GTK main loop where we can safely touch widgets.
    let (tx, rx) = async_channel::unbounded::<(String, serde_json::Value)>();

    // ── Content ──────────────────────────────────────────────
    let status = adw::StatusPage::builder()
        .icon_name("network-transmit-receive-symbolic")
        .title("Starting…")
        .description("Launching the LocalBolt engine")
        .build();

    let events = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .valign(gtk::Align::Start)
        .build();

    let events_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&events)
        .build();

    let credit = gtk::Label::builder()
        .label("by the9ines")
        .css_classes(["dim-label", "caption"])
        .halign(gtk::Align::Center)
        .margin_top(4)
        .margin_bottom(8)
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    content.append(&status);
    content.append(&events_scroll);
    content.append(&credit);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&content));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("LocalBolt")
        .default_width(460)
        .default_height(640)
        .content(&toolbar)
        .build();

    // ── Receive events on the main thread ────────────────────
    let status_for_events = status.clone();
    glib::spawn_future_local(async move {
        while let Ok((name, payload)) = rx.recv().await {
            match name.as_str() {
                "watchdog" => {
                    if payload.get("state").and_then(|s| s.as_str()) == Some("Ready") {
                        status_for_events.set_title("Ready");
                        status_for_events
                            .set_description(Some("Searching for devices on your network"));
                        status_for_events.set_icon_name(Some("network-wireless-symbolic"));
                    }
                }
                _ => {
                    let row = adw::ActionRow::builder()
                        .title(&name)
                        .subtitle(&compact(&payload))
                        .build();
                    events.prepend(&row);
                }
            }
        }
    });

    // ── Wire + start the daemon lifecycle ────────────────────
    let mut lifecycle = DaemonLifecycle::new(APP_VERSION);
    lifecycle.add_binary_search_paths(dev_daemon_paths());
    let lifecycle = Arc::new(lifecycle);

    let tx_bridge = tx.clone();
    lifecycle.set_bridge_event_callback(Box::new(move |name, payload| {
        let _ = tx_bridge.send_blocking((name.to_string(), payload));
    }));
    let tx_watchdog = tx.clone();
    lifecycle.set_watchdog_callback(Box::new(move |ev| {
        let _ = tx_watchdog.send_blocking((
            "watchdog".to_string(),
            serde_json::json!({ "state": format!("{:?}", ev.state), "retry": ev.retry_count }),
        ));
    }));
    lifecycle.start(); // spawns the supervision thread (holds its own Arc)

    // Ask the daemon to shut down cleanly when the window closes.
    let lc = Arc::clone(&lifecycle);
    window.connect_close_request(move |_| {
        lc.shutdown_flag().store(true, Ordering::Relaxed);
        glib::Propagation::Proceed
    });

    window.present();
}

/// Dev-only daemon locations (a Flatpak bundles the daemon at a known path, which
/// `resolve_daemon_binary` finds via `bin/bolt-daemon` / PATH).
fn dev_daemon_paths() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../bolt-daemon/target");
    vec![
        root.join("release/bolt-daemon"),
        root.join("debug/bolt-daemon"),
    ]
}

/// Render a payload as a short one-line summary for the event feed.
fn compact(payload: &serde_json::Value) -> String {
    let s = payload.to_string();
    if s.len() > 120 {
        format!("{}…", &s[..119])
    } else {
        s
    }
}
