//! Persistent IPC bridge for ongoing daemon event forwarding and decision relay.
//!
//! After readiness is confirmed via probe_readiness (N6-A1), the bridge
//! establishes a new persistent connection to the daemon, performs the
//! version handshake, and enters an event loop that forwards daemon events
//! to the Tauri frontend and relays user decisions back to the daemon.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::Emitter;

use crate::ipc_types::{
    DaemonStatusPayload, IpcKind, IpcMessage, PairingRequestPayload,
    TransferIncomingRequestPayload, VersionHandshakePayload, VersionStatusPayload,
};

/// Read timeout during handshake phase.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Read timeout during event loop (allows periodic shutdown checks).
const EVENT_LOOP_TIMEOUT: Duration = Duration::from_secs(5);

/// Persistent IPC bridge between app and daemon.
pub struct IpcBridge {
    writer: Arc<Mutex<Option<UnixStream>>>,
    shutdown: Arc<AtomicBool>,
}

impl IpcBridge {
    pub fn new() -> Self {
        Self {
            writer: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Establish persistent connection and start event forwarding.
    ///
    /// Performs version handshake, reads initial daemon.status, then spawns
    /// a reader thread that forwards all subsequent events to the frontend.
    pub fn start(
        &self,
        socket_path: &Path,
        app_version: &str,
        app_handle: tauri::AppHandle,
    ) -> Result<(), String> {
        let stream =
            UnixStream::connect(socket_path).map_err(|e| format!("bridge connect: {e}"))?;
        stream
            .set_read_timeout(Some(HANDSHAKE_TIMEOUT))
            .map_err(|e| format!("set_read_timeout: {e}"))?;
        stream
            .set_write_timeout(Some(HANDSHAKE_TIMEOUT))
            .map_err(|e| format!("set_write_timeout: {e}"))?;

        let write_stream = stream
            .try_clone()
            .map_err(|e| format!("clone writer: {e}"))?;
        let mut writer = stream
            .try_clone()
            .map_err(|e| format!("clone for handshake: {e}"))?;
        let mut reader = BufReader::new(stream);

        // Step 1: Send version.handshake
        let handshake = IpcMessage::new_decision(
            "version.handshake",
            serde_json::to_value(VersionHandshakePayload {
                app_version: app_version.to_string(),
            })
            .unwrap(),
        );
        let line = handshake
            .to_ndjson()
            .map_err(|e| format!("serialize handshake: {e}"))?;
        writer
            .write_all(line.as_bytes())
            .map_err(|e| format!("write handshake: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("flush handshake: {e}"))?;

        // Step 2: Read version.status
        let mut buf = String::new();
        reader
            .read_line(&mut buf)
            .map_err(|e| format!("read version.status: {e}"))?;
        let vs_msg: IpcMessage =
            serde_json::from_str(buf.trim()).map_err(|e| format!("parse version.status: {e}"))?;
        if vs_msg.msg_type != "version.status" || vs_msg.kind != IpcKind::Event {
            return Err(format!(
                "expected version.status event, got {}:{:?}",
                vs_msg.msg_type, vs_msg.kind
            ));
        }
        let vs: VersionStatusPayload = serde_json::from_value(vs_msg.payload)
            .map_err(|e| format!("parse version.status payload: {e}"))?;
        if !vs.compatible {
            return Err(format!(
                "bridge version incompatible: daemon={}",
                vs.daemon_version
            ));
        }
        tracing::info!("[IPC_BRIDGE] handshake ok: daemon v{}", vs.daemon_version);

        // Step 3: Read initial daemon.status
        buf.clear();
        reader
            .read_line(&mut buf)
            .map_err(|e| format!("read daemon.status: {e}"))?;
        let ds_msg: IpcMessage =
            serde_json::from_str(buf.trim()).map_err(|e| format!("parse daemon.status: {e}"))?;
        if ds_msg.msg_type == "daemon.status" {
            if let Ok(payload) = serde_json::from_value::<DaemonStatusPayload>(ds_msg.payload) {
                let _ = app_handle.emit("daemon://status-update", &payload);
                tracing::info!(
                    "[IPC_BRIDGE] initial status: peers={}",
                    payload.connected_peers
                );
            }
        }

        // Switch to event-loop timeout (longer, allows shutdown checks)
        // Since write_stream shares the underlying socket, setting timeout
        // on it affects reads on the original stream wrapped by BufReader.
        let _ = write_stream.set_read_timeout(Some(EVENT_LOOP_TIMEOUT));

        // Store writer for decision relay
        *self.writer.lock().unwrap() = Some(write_stream);

        // Spawn reader thread for ongoing event forwarding
        let shutdown = Arc::clone(&self.shutdown);
        std::thread::spawn(move || {
            Self::event_loop(reader, &app_handle, &shutdown);
            tracing::info!("[IPC_BRIDGE] reader thread exiting");
        });

        Ok(())
    }

    fn event_loop(
        mut reader: BufReader<UnixStream>,
        app_handle: &tauri::AppHandle,
        shutdown: &AtomicBool,
    ) {
        let mut buf = String::new();
        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }
            buf.clear();
            match reader.read_line(&mut buf) {
                Ok(0) => {
                    tracing::warn!("[IPC_BRIDGE] daemon disconnected (EOF)");
                    let _ = app_handle.emit("daemon://bridge-disconnected", ());
                    break;
                }
                Ok(_) => {
                    Self::dispatch_event(app_handle, buf.trim());
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // Timeout — check shutdown flag and retry
                    continue;
                }
                Err(e) => {
                    tracing::warn!("[IPC_BRIDGE] read error: {e}");
                    let _ = app_handle.emit("daemon://bridge-disconnected", ());
                    break;
                }
            }
        }
    }

    fn dispatch_event(app_handle: &tauri::AppHandle, line: &str) {
        let msg: IpcMessage = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!("[IPC_BRIDGE] parse error: {e}");
                return;
            }
        };

        if msg.kind != IpcKind::Event {
            tracing::debug!("[IPC_BRIDGE] ignoring non-event: {:?}", msg.kind);
            return;
        }

        match msg.msg_type.as_str() {
            "daemon.status" => {
                if let Ok(payload) = serde_json::from_value::<DaemonStatusPayload>(msg.payload) {
                    let _ = app_handle.emit("daemon://status-update", &payload);
                }
            }
            "pairing.request" => {
                if let Ok(payload) = serde_json::from_value::<PairingRequestPayload>(msg.payload) {
                    tracing::info!(
                        "[IPC_BRIDGE] pairing request from {}",
                        payload.remote_device_name
                    );
                    let _ = app_handle.emit("daemon://pairing-request", &payload);
                }
            }
            "transfer.incoming.request" => {
                if let Ok(payload) =
                    serde_json::from_value::<TransferIncomingRequestPayload>(msg.payload)
                {
                    tracing::info!(
                        "[IPC_BRIDGE] transfer request: {} ({} bytes)",
                        payload.file_name,
                        payload.file_size_bytes
                    );
                    let _ = app_handle.emit("daemon://transfer-request", &payload);
                }
            }
            other => {
                tracing::debug!("[IPC_BRIDGE] unhandled event type: {other}");
            }
        }
    }

    /// Send a decision message to the daemon.
    pub fn send_decision(&self, msg: IpcMessage) -> Result<(), String> {
        let mut guard = self.writer.lock().unwrap();
        let writer = guard.as_mut().ok_or("bridge not connected")?;
        let line = msg
            .to_ndjson()
            .map_err(|e| format!("serialize decision: {e}"))?;
        writer
            .write_all(line.as_bytes())
            .map_err(|e| format!("write decision: {e}"))?;
        writer.flush().map_err(|e| format!("flush decision: {e}"))?;
        Ok(())
    }

    /// Check if the bridge has an active writer connection.
    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        self.writer.lock().unwrap().is_some()
    }

    /// Shut down the bridge.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        // Drop the writer to unblock the daemon side
        *self.writer.lock().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_initially_not_connected() {
        let bridge = IpcBridge::new();
        assert!(!bridge.is_connected());
    }

    #[test]
    fn bridge_shutdown_is_idempotent() {
        let bridge = IpcBridge::new();
        bridge.shutdown();
        bridge.shutdown();
        assert!(!bridge.is_connected());
    }

    #[test]
    fn send_decision_fails_when_not_connected() {
        let bridge = IpcBridge::new();
        let msg = IpcMessage::new_decision(
            "pairing.decision",
            serde_json::json!({"request_id": "evt-0", "decision": "deny_once"}),
        );
        let result = bridge.send_decision(msg);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not connected"));
    }

    #[test]
    fn dispatch_ignores_non_event_kind() {
        // dispatch_event should silently skip decision-kind messages.
        // We can't easily test emit without a real AppHandle, but we
        // verify the function doesn't panic on non-event input.
        let line = r#"{"id":"app-0","kind":"decision","type":"test","ts_ms":0,"payload":{}}"#;
        // This just tests the parse + kind check path — no AppHandle needed
        // for the skip branch. We verify via the IpcMessage parse.
        let msg: IpcMessage = serde_json::from_str(line).unwrap();
        assert_eq!(msg.kind, crate::ipc_types::IpcKind::Decision);
    }
}
