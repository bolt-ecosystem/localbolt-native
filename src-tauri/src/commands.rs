//! Tauri command surface for daemon management.
//!
//! Exposes watchdog state, manual restart, support bundle export,
//! and decision relay to the frontend via Tauri's IPC bridge.

use std::sync::Arc;

use serde::Serialize;

use crate::daemon::DaemonManager;
use crate::ipc_types::{IpcMessage, PairingDecisionPayload, TransferIncomingDecisionPayload};
use crate::platform;
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
    let watchdog = manager.watchdog.lock().unwrap();
    WatchdogStateResponse {
        state: watchdog.state(),
        retry_count: watchdog.retry_count(),
    }
}

/// Request manual daemon restart (only works from degraded state).
#[tauri::command]
pub fn restart_daemon(manager: tauri::State<'_, Arc<DaemonManager>>) -> Result<String, String> {
    let success = manager.inner().clone().manual_restart();
    if success {
        Ok("restart initiated".to_string())
    } else {
        Err("restart not available in current state".to_string())
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
    manager.bridge.send_decision(msg)?;
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
    manager.bridge.send_decision(msg)?;
    tracing::info!(
        "[IPC_BRIDGE] sent transfer.incoming.decision for {} -> {:?}",
        payload.request_id,
        payload.decision
    );
    Ok("decision sent".to_string())
}

// ── Support Bundle ─────────────────────────────────────────

/// Support bundle output structure.
#[derive(Serialize)]
pub struct SupportBundle {
    pub bundle_version: &'static str,
    pub generated_at: String,
    pub output_path: String,
    pub app_version: String,
    pub daemon_version: Option<String>,
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

/// Collect, serialize, and write the support bundle to disk.
fn build_support_bundle(manager: &DaemonManager) -> Result<SupportBundle, String> {
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let now = now_ns / 1_000_000_000; // epoch seconds for display

    let generated_at = format!("{now}");

    // Watchdog state
    let watchdog = {
        let w = manager.watchdog.lock().unwrap();
        WatchdogSnapshot {
            state: w.state(),
            retry_count: w.retry_count(),
            spawn_count: manager.spawn_count(),
        }
    };

    // Daemon stderr (last 200 lines)
    let daemon_stderr = manager.stderr_buffer.last_n(200);
    let stderr_count = daemon_stderr.len();

    // Crash snapshots
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

    // Daemon version
    let daemon_version = manager.daemon_version();

    // Platform metadata
    let plat = PlatformMeta {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        target_triple: env!("TAURI_ENV_TARGET_TRIPLE", "unknown-unknown-unknown"),
    };

    // IPC config
    let ipc_config = IpcConfig {
        socket_path: manager.socket_path().to_string(),
        data_dir: manager.data_dir().to_string(),
        pid_path: manager.pid_path().to_string(),
    };

    // App version
    let app_version = env!("CARGO_PKG_VERSION").to_string();

    // Build manifest with explicit presence tracking
    let mut manifest = Vec::new();

    manifest.push(ManifestEntry {
        section: "daemon_stderr".to_string(),
        present: stderr_count > 0,
        count: Some(stderr_count),
        note: if stderr_count == 0 {
            Some("no daemon stderr lines captured".to_string())
        } else {
            None
        },
    });

    manifest.push(ManifestEntry {
        section: "crash_snapshots".to_string(),
        present: snapshot_count > 0,
        count: Some(snapshot_count),
        note: if snapshot_count == 0 {
            Some("no crash snapshot files found".to_string())
        } else {
            None
        },
    });

    manifest.push(ManifestEntry {
        section: "watchdog_state".to_string(),
        present: true,
        count: None,
        note: None,
    });

    manifest.push(ManifestEntry {
        section: "app_version".to_string(),
        present: true,
        count: None,
        note: None,
    });

    manifest.push(ManifestEntry {
        section: "daemon_version".to_string(),
        present: daemon_version.is_some(),
        count: None,
        note: if daemon_version.is_none() {
            Some("daemon not connected or version unknown".to_string())
        } else {
            None
        },
    });

    manifest.push(ManifestEntry {
        section: "platform_metadata".to_string(),
        present: true,
        count: None,
        note: None,
    });

    manifest.push(ManifestEntry {
        section: "spawn_counters".to_string(),
        present: true,
        count: Some(manager.spawn_count() as usize),
        note: None,
    });

    manifest.push(ManifestEntry {
        section: "ipc_config".to_string(),
        present: true,
        count: None,
        note: None,
    });

    // Write to disk
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
        platform: plat,
        ipc_config,
        watchdog,
        daemon_stderr,
        crash_snapshots,
        manifest,
    };

    let json =
        serde_json::to_string_pretty(&bundle).map_err(|e| format!("serialize bundle: {e}"))?;
    std::fs::write(&output_path, &json).map_err(|e| format!("write bundle: {e}"))?;

    tracing::info!("[SUPPORT_BUNDLE] written to {}", output_path.display());

    Ok(bundle)
}

/// Export support bundle — collects diagnostics and writes to disk.
///
/// Returns the output path on success.
#[tauri::command]
pub fn export_support_bundle(
    manager: tauri::State<'_, Arc<DaemonManager>>,
) -> Result<String, String> {
    let bundle = build_support_bundle(manager.inner())?;
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
        assert!(json.contains("\"retry_count\":0"));
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
        assert_eq!(msg.msg_type, "pairing.decision");
        let line = msg.to_ndjson().unwrap();
        assert!(line.contains("allow_once"));
    }

    #[test]
    fn transfer_decision_payload_serializes() {
        use crate::ipc_types::Decision;
        let p = TransferIncomingDecisionPayload {
            request_id: "evt-7".into(),
            decision: Decision::DenyOnce,
            note: Some("test".into()),
        };
        let msg = IpcMessage::new_decision(
            "transfer.incoming.decision",
            serde_json::to_value(&p).unwrap(),
        );
        assert_eq!(msg.msg_type, "transfer.incoming.decision");
        let line = msg.to_ndjson().unwrap();
        assert!(line.contains("deny_once"));
        assert!(line.contains("test"));
    }

    #[test]
    fn support_bundle_builds_successfully() {
        let mgr = DaemonManager::new();
        let result = build_support_bundle(&mgr);
        assert!(result.is_ok());
        let bundle = result.unwrap();

        // Verify all required sections present in manifest
        let section_names: Vec<&str> = bundle.manifest.iter().map(|m| m.section.as_str()).collect();
        assert!(section_names.contains(&"daemon_stderr"));
        assert!(section_names.contains(&"crash_snapshots"));
        assert!(section_names.contains(&"watchdog_state"));
        assert!(section_names.contains(&"app_version"));
        assert!(section_names.contains(&"daemon_version"));
        assert!(section_names.contains(&"platform_metadata"));
        assert!(section_names.contains(&"spawn_counters"));
        assert!(section_names.contains(&"ipc_config"));

        // Verify file was written
        assert!(!bundle.output_path.is_empty());
        assert!(std::path::Path::new(&bundle.output_path).exists());

        // Verify file is valid JSON
        let content = std::fs::read_to_string(&bundle.output_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["bundle_version"], "1.0.0");
        assert!(parsed["manifest"].is_array());

        // Verify non-empty platform metadata
        assert!(!bundle.platform.os.is_empty());
        assert!(!bundle.platform.arch.is_empty());

        // Verify IPC config populated
        assert!(!bundle.ipc_config.socket_path.is_empty());
        assert!(!bundle.ipc_config.data_dir.is_empty());

        // Cleanup
        let _ = std::fs::remove_file(&bundle.output_path);
    }

    #[test]
    fn support_bundle_marks_missing_artifacts() {
        let mgr = DaemonManager::new();
        let bundle = build_support_bundle(&mgr).unwrap();

        // With a fresh manager, daemon is not connected
        let daemon_ver_entry = bundle
            .manifest
            .iter()
            .find(|m| m.section == "daemon_version")
            .unwrap();
        assert!(!daemon_ver_entry.present);
        assert!(daemon_ver_entry.note.is_some());
        assert!(daemon_ver_entry
            .note
            .as_ref()
            .unwrap()
            .contains("not connected"));

        // Cleanup
        let _ = std::fs::remove_file(&bundle.output_path);
    }

    #[test]
    fn support_bundle_includes_stderr_when_present() {
        let mgr = DaemonManager::new();
        mgr.stderr_buffer.push("test error line 1".to_string());
        mgr.stderr_buffer.push("test error line 2".to_string());

        let bundle = build_support_bundle(&mgr).unwrap();
        assert_eq!(bundle.daemon_stderr.len(), 2);
        assert_eq!(bundle.daemon_stderr[0], "test error line 1");

        let stderr_entry = bundle
            .manifest
            .iter()
            .find(|m| m.section == "daemon_stderr")
            .unwrap();
        assert!(stderr_entry.present);
        assert_eq!(stderr_entry.count, Some(2));

        // Cleanup
        let _ = std::fs::remove_file(&bundle.output_path);
    }
}
