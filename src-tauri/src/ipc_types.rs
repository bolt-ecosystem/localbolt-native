//! IPC message types for daemon communication (app-side).
//!
//! Mirrors the subset of bolt-daemon's IPC contract needed for readiness probing.
//! Wire format: NDJSON (one JSON object per `\n`-terminated line).

use serde::{Deserialize, Serialize};

/// Top-level IPC message envelope.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IpcMessage {
    pub id: String,
    pub kind: IpcKind,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub ts_ms: u64,
    pub payload: serde_json::Value,
}

/// Message direction.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IpcKind {
    Event,
    Decision,
}

// ── Payloads needed for readiness path ─────────────────────

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VersionHandshakePayload {
    pub app_version: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct VersionStatusPayload {
    pub daemon_version: String,
    pub compatible: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DaemonStatusPayload {
    pub connected_peers: u32,
    pub ui_connected: bool,
    pub version: String,
}

// ── Helpers ────────────────────────────────────────────────

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

static ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn generate_id() -> String {
    let n = ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("app-{n}")
}

impl IpcMessage {
    /// Create a decision message (app -> daemon).
    pub fn new_decision(msg_type: &str, payload: serde_json::Value) -> Self {
        Self {
            id: generate_id(),
            kind: IpcKind::Decision,
            msg_type: msg_type.to_string(),
            ts_ms: now_ms(),
            payload,
        }
    }

    /// Serialize to a single NDJSON line (with trailing newline).
    pub fn to_ndjson(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_handshake_roundtrip() {
        let p = VersionHandshakePayload {
            app_version: "1.0.0".to_string(),
        };
        let msg = IpcMessage::new_decision("version.handshake", serde_json::to_value(&p).unwrap());
        assert_eq!(msg.kind, IpcKind::Decision);
        assert_eq!(msg.msg_type, "version.handshake");
        let line = msg.to_ndjson().unwrap();
        assert!(line.ends_with('\n'));
        let decoded: IpcMessage = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(decoded.payload["app_version"], "1.0.0");
    }

    #[test]
    fn version_status_deserialize() {
        let json = r#"{"id":"evt-0","kind":"event","type":"version.status","ts_ms":1000,"payload":{"daemon_version":"0.0.1","compatible":true}}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        let payload: VersionStatusPayload = serde_json::from_value(msg.payload).unwrap();
        assert!(payload.compatible);
        assert_eq!(payload.daemon_version, "0.0.1");
    }

    #[test]
    fn daemon_status_deserialize() {
        let json = r#"{"id":"evt-1","kind":"event","type":"daemon.status","ts_ms":2000,"payload":{"connected_peers":0,"ui_connected":true,"version":"0.0.1"}}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        let payload: DaemonStatusPayload = serde_json::from_value(msg.payload).unwrap();
        assert!(payload.ui_connected);
        assert_eq!(payload.connected_peers, 0);
    }

    #[test]
    fn extra_fields_preserved() {
        let json =
            r#"{"id":"x","kind":"event","type":"t","ts_ms":0,"payload":{"known":"v","extra":42}}"#;
        let msg: IpcMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.payload["extra"], 42);
    }
}
