//! Tauri command surface for daemon management.
//!
//! Thin Tauri glue over bolt-app-core runtime. Exposes watchdog state,
//! manual restart, support bundle, and decision relay to the frontend.

use std::sync::Arc;

use serde::Serialize;

use crate::daemon::DaemonManager;
use crate::ipc_types::{IpcMessage, PairingDecisionPayload, TransferIncomingDecisionPayload};
use crate::platform;
use crate::signal_monitor::{self, SignalStatus};
use crate::watchdog::WatchdogState;

/// Watchdog state response for the frontend.
#[derive(Serialize)]
pub struct WatchdogStateResponse {
    pub state: WatchdogState,
    pub retry_count: u32,
}

/// Get current watchdog state.
#[tauri::command]
pub fn get_watchdog_state(manager: tauri::State<'_, Arc<DaemonManager>>) -> WatchdogStateResponse {
    WatchdogStateResponse {
        state: manager.watchdog_state(),
        retry_count: manager.watchdog_retry_count(),
    }
}

/// Request manual daemon restart (only works from degraded state).
#[tauri::command]
pub fn restart_daemon(manager: tauri::State<'_, Arc<DaemonManager>>) -> Result<String, String> {
    if manager.manual_restart() {
        Ok("restart initiated".to_string())
    } else {
        Err("restart not available in current state".to_string())
    }
}

/// Signal status response for the frontend.
#[derive(Serialize)]
pub struct SignalStatusResponse {
    pub status: SignalStatus,
}

/// Get current signal server health status via a synchronous probe.
#[tauri::command]
pub fn get_signal_status() -> SignalStatusResponse {
    let healthy = signal_monitor::probe_signal_health();
    SignalStatusResponse {
        status: if healthy {
            SignalStatus::Active
        } else {
            SignalStatus::Offline
        },
    }
}

/// Send a pairing decision to the daemon via IPC bridge.
#[tauri::command]
pub fn send_pairing_decision(
    manager: tauri::State<'_, Arc<DaemonManager>>,
    payload: PairingDecisionPayload,
) -> Result<String, String> {
    let msg = IpcMessage::new_decision(
        "pairing.decision",
        serde_json::to_value(&payload).map_err(|e| format!("serialize: {e}"))?,
    );
    manager.bridge().send_decision(msg)?;
    tracing::info!(
        "[IPC_BRIDGE] sent pairing.decision for {} -> {:?}",
        payload.request_id,
        payload.decision
    );
    Ok("decision sent".to_string())
}

/// Send a transfer incoming decision to the daemon via IPC bridge.
#[tauri::command]
pub fn send_transfer_decision(
    manager: tauri::State<'_, Arc<DaemonManager>>,
    payload: TransferIncomingDecisionPayload,
) -> Result<String, String> {
    let msg = IpcMessage::new_decision(
        "transfer.incoming.decision",
        serde_json::to_value(&payload).map_err(|e| format!("serialize: {e}"))?,
    );
    manager.bridge().send_decision(msg)?;
    tracing::info!(
        "[IPC_BRIDGE] sent transfer.incoming.decision for {} -> {:?}",
        payload.request_id,
        payload.decision
    );
    Ok("decision sent".to_string())
}

// ── Support Bundle ─────────────────────────────────────────

#[derive(Serialize)]
pub struct SupportBundle {
    pub bundle_version: &'static str,
    pub generated_at: String,
    pub output_path: String,
    pub app_version: String,
    pub daemon_version: Option<String>,
    pub signal_status: SignalStatus,
    pub platform: PlatformMeta,
    pub ipc_config: IpcConfig,
    pub watchdog: WatchdogSnapshot,
    pub daemon_stderr: Vec<String>,
    pub crash_snapshots: Vec<CrashSnapshotInfo>,
    pub manifest: Vec<ManifestEntry>,
}

#[derive(Serialize)]
pub struct PlatformMeta {
    pub os: &'static str,
    pub arch: &'static str,
    pub target_triple: &'static str,
}

#[derive(Serialize)]
pub struct IpcConfig {
    pub socket_path: String,
    pub data_dir: String,
    pub pid_path: String,
}

#[derive(Serialize)]
pub struct WatchdogSnapshot {
    pub state: WatchdogState,
    pub retry_count: u32,
    pub spawn_count: u32,
}

#[derive(Serialize)]
pub struct CrashSnapshotInfo {
    pub filename: String,
    pub present: bool,
    pub size_bytes: Option<u64>,
}

#[derive(Serialize)]
pub struct ManifestEntry {
    pub section: String,
    pub present: bool,
    pub count: Option<usize>,
    pub note: Option<String>,
}

fn build_support_bundle(manager: &DaemonManager) -> Result<SupportBundle, String> {
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let now = now_ns / 1_000_000_000;
    let generated_at = format!("{now}");

    let watchdog = WatchdogSnapshot {
        state: manager.watchdog_state(),
        retry_count: manager.watchdog_retry_count(),
        spawn_count: manager.spawn_count(),
    };

    let daemon_stderr = manager.stderr_buffer().last_n(200);
    let stderr_count = daemon_stderr.len();

    let crash_log_dir = platform::crash_log_dir();
    let mut crash_snapshots = Vec::new();
    let mut snapshot_count = 0usize;
    if crash_log_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&crash_log_dir) {
            for entry in entries.flatten() {
                let filename = entry.file_name().to_string_lossy().to_string();
                if filename.starts_with("daemon-crash-") && filename.ends_with(".log") {
                    let meta = entry.metadata().ok();
                    let size = meta.map(|m| m.len());
                    crash_snapshots.push(CrashSnapshotInfo {
                        filename,
                        present: true,
                        size_bytes: size,
                    });
                    snapshot_count += 1;
                }
            }
        }
    }

    let daemon_version = manager.daemon_version();
    let signal_status = if signal_monitor::probe_signal_health() {
        SignalStatus::Active
    } else {
        SignalStatus::Offline
    };

    let plat = PlatformMeta {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        target_triple: env!("TAURI_ENV_TARGET_TRIPLE", "unknown-unknown-unknown"),
    };

    let ipc_config = IpcConfig {
        socket_path: manager.socket_path().to_string(),
        data_dir: manager.data_dir().to_string(),
        pid_path: manager.pid_path().to_string(),
    };

    let app_version = env!("CARGO_PKG_VERSION").to_string();

    let manifest = vec![
        ManifestEntry { section: "daemon_stderr".into(), present: stderr_count > 0, count: Some(stderr_count), note: if stderr_count == 0 { Some("no daemon stderr lines captured".into()) } else { None } },
        ManifestEntry { section: "crash_snapshots".into(), present: snapshot_count > 0, count: Some(snapshot_count), note: if snapshot_count == 0 { Some("no crash snapshot files found".into()) } else { None } },
        ManifestEntry { section: "watchdog_state".into(), present: true, count: None, note: None },
        ManifestEntry { section: "app_version".into(), present: true, count: None, note: None },
        ManifestEntry { section: "daemon_version".into(), present: daemon_version.is_some(), count: None, note: if daemon_version.is_none() { Some("daemon not connected or version unknown".into()) } else { None } },
        ManifestEntry { section: "platform_metadata".into(), present: true, count: None, note: None },
        ManifestEntry { section: "spawn_counters".into(), present: true, count: Some(manager.spawn_count() as usize), note: None },
        ManifestEntry { section: "ipc_config".into(), present: true, count: None, note: None },
        ManifestEntry { section: "signal_status".into(), present: true, count: None, note: Some(format!("signal server: {signal_status}")) },
    ];

    let bundle_dir = platform::support_bundle_dir();
    std::fs::create_dir_all(&bundle_dir).map_err(|e| format!("create bundle dir: {e}"))?;
    let filename = format!("localbolt-support-{now_ns}.json");
    let output_path = bundle_dir.join(&filename);

    let bundle = SupportBundle {
        bundle_version: "1.0.0",
        generated_at,
        output_path: output_path.to_string_lossy().to_string(),
        app_version,
        daemon_version,
        signal_status,
        platform: plat,
        ipc_config,
        watchdog,
        daemon_stderr,
        crash_snapshots,
        manifest,
    };

    let json = serde_json::to_string_pretty(&bundle).map_err(|e| format!("serialize bundle: {e}"))?;
    std::fs::write(&output_path, &json).map_err(|e| format!("write bundle: {e}"))?;
    tracing::info!("[SUPPORT_BUNDLE] written to {}", output_path.display());

    Ok(bundle)
}

#[tauri::command]
pub fn export_support_bundle(
    manager: tauri::State<'_, Arc<DaemonManager>>,
) -> Result<String, String> {
    let bundle = build_support_bundle(&manager)?;
    Ok(bundle.output_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchdog_state_response_serializes() {
        let resp = WatchdogStateResponse {
            state: WatchdogState::Starting,
            retry_count: 0,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"starting\""));
    }

    #[test]
    fn pairing_decision_payload_serializes() {
        use crate::ipc_types::Decision;
        let p = PairingDecisionPayload {
            request_id: "evt-5".into(),
            decision: Decision::AllowOnce,
            note: None,
        };
        let msg = IpcMessage::new_decision("pairing.decision", serde_json::to_value(&p).unwrap());
        let line = msg.to_ndjson().unwrap();
        assert!(line.contains("allow_once"));
    }

    #[test]
    fn support_bundle_builds_successfully() {
        let mgr = DaemonManager::new();
        let result = build_support_bundle(&mgr);
        assert!(result.is_ok());
        let bundle = result.unwrap();
        let section_names: Vec<&str> = bundle.manifest.iter().map(|m| m.section.as_str()).collect();
        assert!(section_names.contains(&"daemon_stderr"));
        assert!(section_names.contains(&"watchdog_state"));
        let _ = std::fs::remove_file(&bundle.output_path);
    }
}
