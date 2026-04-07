//! Tauri-specific daemon manager adapter.
//!
//! Thin wrapper over bolt_app_core::daemon_lifecycle::DaemonLifecycle.
//! Adds only Tauri-specific concerns: AppHandle for event emission,
//! TAURI_ENV_TARGET_TRIPLE for binary resolution.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::Emitter;

use bolt_app_core::daemon_lifecycle::{DaemonLifecycle, WatchdogStateEvent};
use bolt_app_core::watchdog::WatchdogState;

/// App version used for IPC handshake.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tauri-specific daemon manager. Wraps the shell-agnostic DaemonLifecycle.
pub struct DaemonManager {
    inner: Arc<DaemonLifecycle>,
}

impl DaemonManager {
    pub fn new() -> Self {
        let mut lifecycle = DaemonLifecycle::new(APP_VERSION);

        // Add Tauri-specific binary search paths (target triple variant)
        let target_triple = env!("TAURI_ENV_TARGET_TRIPLE", "unknown-unknown-unknown");
        lifecycle.add_binary_search_paths(vec![
            PathBuf::from(format!("bin/bolt-daemon-{target_triple}")),
        ]);

        Self {
            inner: Arc::new(lifecycle),
        }
    }

    /// Wire Tauri event emission to the shared lifecycle callbacks.
    pub fn set_app_handle(&mut self, handle: tauri::AppHandle) {
        let handle_wd = handle.clone();
        self.inner.set_watchdog_callback(Box::new(move |event: WatchdogStateEvent| {
            let _ = handle_wd.emit("daemon://watchdog-state", &event);
        }));

        let handle_bridge = handle.clone();
        self.inner.set_bridge_event_callback(Box::new(move |event_name, payload| {
            let _ = handle_bridge.emit(event_name, &payload);
        }));
    }

    pub fn shutdown_flag(&self) -> Arc<std::sync::atomic::AtomicBool> {
        self.inner.shutdown_flag()
    }

    pub fn start(&self) {
        self.inner.start();
    }

    pub fn shutdown(&self) {
        self.inner.shutdown();
    }

    pub fn manual_restart(&self) -> bool {
        self.inner.manual_restart()
    }

    // ── Delegated accessors for commands.rs ────────────────────

    pub fn watchdog_state(&self) -> WatchdogState {
        self.inner.watchdog.lock().unwrap().state()
    }

    pub fn watchdog_retry_count(&self) -> u32 {
        self.inner.watchdog.lock().unwrap().retry_count()
    }

    pub fn socket_path(&self) -> &str {
        self.inner.socket_path()
    }

    pub fn pid_path(&self) -> &str {
        self.inner.pid_path()
    }

    pub fn data_dir(&self) -> &str {
        self.inner.data_dir()
    }

    pub fn daemon_version(&self) -> Option<String> {
        self.inner.daemon_version()
    }

    pub fn spawn_count(&self) -> u32 {
        self.inner.spawn_count()
    }

    pub fn stderr_buffer(&self) -> &bolt_app_core::daemon_log::StderrBuffer {
        &self.inner.stderr_buffer
    }

    pub fn bridge(&self) -> &Arc<bolt_app_core::ipc_bridge_core::IpcBridgeCore> {
        &self.inner.bridge
    }
}
