//! Minimal IPC client for daemon readiness probing.
//!
//! Connects to the daemon's Unix socket, performs version handshake,
//! and reads the daemon.status event. Full message routing (pairing,
//! transfer decisions) is deferred to N6-A2.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use crate::ipc_types::{
    DaemonStatusPayload, IpcKind, IpcMessage, VersionHandshakePayload, VersionStatusPayload,
};

/// Default socket path (matches daemon's DEFAULT_SOCKET_PATH).
pub const DAEMON_SOCKET_PATH: &str = "/tmp/bolt-daemon.sock";

/// Timeout for the full readiness handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Result of a readiness probe.
#[derive(Debug)]
pub enum ReadinessResult {
    /// Daemon is ready and compatible.
    Ready {
        daemon_version: String,
        #[allow(dead_code)]
        connected_peers: u32,
    },
    /// Daemon responded with incompatible version.
    Incompatible { daemon_version: String },
    /// Could not connect or handshake failed.
    Failed(String),
}

/// Probe daemon readiness via IPC.
///
/// 1. Connect to Unix socket
/// 2. Send version.handshake
/// 3. Read version.status
/// 4. Read daemon.status
pub fn probe_readiness(socket_path: &Path, app_version: &str) -> ReadinessResult {
    // Connect
    let stream = match UnixStream::connect(socket_path) {
        Ok(s) => s,
        Err(e) => {
            return ReadinessResult::Failed(format!("socket connect failed: {e}"));
        }
    };

    if let Err(e) = stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT)) {
        return ReadinessResult::Failed(format!("set_read_timeout: {e}"));
    }
    if let Err(e) = stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT)) {
        return ReadinessResult::Failed(format!("set_write_timeout: {e}"));
    }

    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);

    // Step 1: Send version.handshake
    let handshake = IpcMessage::new_decision(
        "version.handshake",
        serde_json::to_value(VersionHandshakePayload {
            app_version: app_version.to_string(),
        })
        .unwrap(),
    );
    let line = match handshake.to_ndjson() {
        Ok(l) => l,
        Err(e) => return ReadinessResult::Failed(format!("serialize handshake: {e}")),
    };
    if let Err(e) = writer.write_all(line.as_bytes()) {
        return ReadinessResult::Failed(format!("write handshake: {e}"));
    }
    if let Err(e) = writer.flush() {
        return ReadinessResult::Failed(format!("flush handshake: {e}"));
    }

    // Step 2: Read version.status
    let mut buf = String::new();
    if let Err(e) = reader.read_line(&mut buf) {
        return ReadinessResult::Failed(format!("read version.status: {e}"));
    }
    let version_msg: IpcMessage = match serde_json::from_str(buf.trim()) {
        Ok(m) => m,
        Err(e) => return ReadinessResult::Failed(format!("parse version.status: {e}")),
    };

    if version_msg.msg_type != "version.status" || version_msg.kind != IpcKind::Event {
        return ReadinessResult::Failed(format!(
            "expected version.status event, got {}:{:?}",
            version_msg.msg_type, version_msg.kind
        ));
    }

    let version_payload: VersionStatusPayload = match serde_json::from_value(version_msg.payload) {
        Ok(p) => p,
        Err(e) => return ReadinessResult::Failed(format!("parse version.status payload: {e}")),
    };

    if !version_payload.compatible {
        tracing::warn!(
            "[WATCHDOG] daemon version incompatible: {}",
            version_payload.daemon_version
        );
        return ReadinessResult::Incompatible {
            daemon_version: version_payload.daemon_version,
        };
    }

    // Step 3: Read daemon.status
    buf.clear();
    if let Err(e) = reader.read_line(&mut buf) {
        return ReadinessResult::Failed(format!("read daemon.status: {e}"));
    }
    let status_msg: IpcMessage = match serde_json::from_str(buf.trim()) {
        Ok(m) => m,
        Err(e) => return ReadinessResult::Failed(format!("parse daemon.status: {e}")),
    };

    if status_msg.msg_type != "daemon.status" || status_msg.kind != IpcKind::Event {
        return ReadinessResult::Failed(format!(
            "expected daemon.status event, got {}:{:?}",
            status_msg.msg_type, status_msg.kind
        ));
    }

    let status_payload: DaemonStatusPayload = match serde_json::from_value(status_msg.payload) {
        Ok(p) => p,
        Err(e) => return ReadinessResult::Failed(format!("parse daemon.status payload: {e}")),
    };

    tracing::info!(
        "[WATCHDOG] daemon ready: version={}, peers={}",
        version_payload.daemon_version,
        status_payload.connected_peers
    );

    ReadinessResult::Ready {
        daemon_version: version_payload.daemon_version,
        connected_peers: status_payload.connected_peers,
    }
}

/// Check if a socket path has a responsive daemon.
///
/// Non-blocking probe: connect + immediate disconnect. Used for stale socket detection.
pub fn socket_probe(socket_path: &Path) -> bool {
    match UnixStream::connect(socket_path) {
        Ok(stream) => {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            drop(stream);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    fn temp_socket_path() -> std::path::PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::path::PathBuf::from(format!("/tmp/bolt-test-ipc-{id}.sock"))
    }

    #[test]
    fn probe_fails_on_nonexistent_socket() {
        let path = std::path::Path::new("/tmp/bolt-nonexistent-test.sock");
        let result = probe_readiness(path, "1.0.0");
        matches!(result, ReadinessResult::Failed(_));
    }

    #[test]
    fn socket_probe_returns_false_for_nonexistent() {
        let path = std::path::Path::new("/tmp/bolt-nonexistent-probe-test.sock");
        assert!(!socket_probe(path));
    }

    #[test]
    fn socket_probe_returns_true_for_listening() {
        let path = temp_socket_path();
        let _ = std::fs::remove_file(&path);
        let _listener = UnixListener::bind(&path).unwrap();
        assert!(socket_probe(&path));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn probe_readiness_with_compatible_mock() {
        let path = temp_socket_path();
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();

        let path_clone = path.clone();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;

            // Read handshake
            let mut buf = String::new();
            reader.read_line(&mut buf).unwrap();
            let msg: IpcMessage = serde_json::from_str(buf.trim()).unwrap();
            assert_eq!(msg.msg_type, "version.handshake");

            // Send version.status (compatible)
            let vs = IpcMessage {
                id: "evt-0".to_string(),
                kind: IpcKind::Event,
                msg_type: "version.status".to_string(),
                ts_ms: 1000,
                payload: serde_json::to_value(VersionStatusPayload {
                    daemon_version: "0.0.1".to_string(),
                    compatible: true,
                })
                .unwrap(),
            };
            writer
                .write_all(vs.to_ndjson().unwrap().as_bytes())
                .unwrap();

            // Send daemon.status
            let ds = IpcMessage {
                id: "evt-1".to_string(),
                kind: IpcKind::Event,
                msg_type: "daemon.status".to_string(),
                ts_ms: 1001,
                payload: serde_json::to_value(DaemonStatusPayload {
                    connected_peers: 0,
                    ui_connected: true,
                    version: "0.0.1".to_string(),
                })
                .unwrap(),
            };
            writer
                .write_all(ds.to_ndjson().unwrap().as_bytes())
                .unwrap();
            writer.flush().unwrap();
        });

        let result = probe_readiness(&path_clone, "1.0.0");
        handle.join().unwrap();
        let _ = std::fs::remove_file(&path);

        match result {
            ReadinessResult::Ready {
                daemon_version,
                connected_peers,
            } => {
                assert_eq!(daemon_version, "0.0.1");
                assert_eq!(connected_peers, 0);
            }
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    #[test]
    fn probe_readiness_with_incompatible_mock() {
        let path = temp_socket_path();
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();

        let path_clone = path.clone();
        let handle = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;

            // Read handshake
            let mut buf = String::new();
            reader.read_line(&mut buf).unwrap();

            // Send version.status (incompatible)
            let vs = IpcMessage {
                id: "evt-0".to_string(),
                kind: IpcKind::Event,
                msg_type: "version.status".to_string(),
                ts_ms: 1000,
                payload: serde_json::to_value(VersionStatusPayload {
                    daemon_version: "99.0.0".to_string(),
                    compatible: false,
                })
                .unwrap(),
            };
            writer
                .write_all(vs.to_ndjson().unwrap().as_bytes())
                .unwrap();
            writer.flush().unwrap();
        });

        let result = probe_readiness(&path_clone, "1.0.0");
        handle.join().unwrap();
        let _ = std::fs::remove_file(&path);

        match result {
            ReadinessResult::Incompatible { daemon_version } => {
                assert_eq!(daemon_version, "99.0.0");
            }
            other => panic!("expected Incompatible, got {other:?}"),
        }
    }
}
