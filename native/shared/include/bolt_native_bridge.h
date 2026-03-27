// bolt_native_bridge.h — C-ABI interface for native platform shells.
//
// Generated from bolt-native-bridge Rust crate.
// Import this header in Swift via a bridging header or module map.

#ifndef BOLT_NATIVE_BRIDGE_H
#define BOLT_NATIVE_BRIDGE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ── Platform / Identity ─────────────────────────────────────

/// Generate a secure peer code. Caller must free with bolt_free_string.
char* bolt_generate_peer_code(void);

/// Get the platform-specific data directory. Caller must free with bolt_free_string.
char* bolt_platform_data_dir(void);

/// Get the platform-specific IPC socket path. Caller must free with bolt_free_string.
char* bolt_platform_ipc_path(void);

/// Probe signal server health. Returns 1 if healthy, 0 if not.
int bolt_probe_signal_health(void);

/// Free a string returned by any bolt_ function.
void bolt_free_string(char* ptr);

// ── Daemon Lifecycle ────────────────────────────────────────

/// Opaque daemon handle.
typedef struct BoltDaemon BoltDaemon;

/// Find the bolt-daemon binary. Returns null if not found.
/// Caller must free with bolt_free_string.
char* bolt_daemon_find_binary(void);

/// Start a daemon process. Returns opaque handle, or null on failure.
/// daemon_bin: path to bolt-daemon binary (C string).
/// ws_port: port for WS endpoint (0 = auto-assign).
BoltDaemon* bolt_daemon_start(const char* daemon_bin, uint16_t ws_port);

/// Check if daemon is running. Returns 1 if running, 0 if not.
int bolt_daemon_is_running(BoltDaemon* handle);

/// Get the daemon's WS port.
uint16_t bolt_daemon_ws_port(BoltDaemon* handle);

/// Get the daemon's PID.
uint32_t bolt_daemon_pid(BoltDaemon* handle);

/// Get recent stderr lines (last N, newline-separated).
/// Caller must free with bolt_free_string.
char* bolt_daemon_recent_stderr(BoltDaemon* handle, uint32_t last_n);

/// Get the daemon's IPC socket path. Caller must free with bolt_free_string.
char* bolt_daemon_socket_path(BoltDaemon* handle);

/// Get the daemon's data directory. Caller must free with bolt_free_string.
char* bolt_daemon_data_dir(BoltDaemon* handle);

/// Trigger a file send via the daemon. file_path must be absolute.
/// Returns 1 on success, 0 on failure.
int bolt_daemon_send_file(BoltDaemon* handle, const char* file_path);

/// Stop the daemon and free the handle.
/// After this call, handle is invalid.
void bolt_daemon_stop(BoltDaemon* handle);

// ── Signaling / Peer Discovery ───────────────────────────────

/// Opaque signaling handle.
typedef struct BoltSignaling BoltSignaling;

/// Discovered peer.
typedef struct {
    char* peer_code;
    char* device_name;
    char* device_type;
} BoltPeer;

/// Start signaling client. cloud_url may be null.
BoltSignaling* bolt_signaling_start(
    const char* local_url,
    const char* cloud_url,
    const char* peer_code,
    const char* device_name
);

/// Number of currently discovered peers.
uint32_t bolt_signaling_peer_count(BoltSignaling* handle);

/// Get peer by index. Caller must free with bolt_peer_free.
BoltPeer* bolt_signaling_get_peer(BoltSignaling* handle, uint32_t index);

/// Free a peer returned by bolt_signaling_get_peer.
void bolt_peer_free(BoltPeer* peer);

/// Check if connected to at least one signaling plane.
int bolt_signaling_is_connected(BoltSignaling* handle);

/// Drain pending events (returns count). Read peer list after draining.
uint32_t bolt_signaling_drain_events(BoltSignaling* handle);

/// Send a signal to a peer (connection initiation).
/// data_json may be null for empty payload.
/// Returns 1 on success, 0 on failure.
int bolt_signaling_send_signal(
    BoltSignaling* handle,
    const char* to_peer_code,
    const char* signal_type,
    const char* data_json
);

/// Stop signaling and free the handle.
void bolt_signaling_stop(BoltSignaling* handle);

// ── IPC Bridge (daemon event stream) ────────────────────────

/// Opaque IPC bridge handle.
typedef struct BoltIpc BoltIpc;

/// Start IPC bridge to daemon. Connects, handshakes, starts event loop.
/// socket_path: daemon IPC socket path (C string).
/// app_version: app version string for handshake (C string).
/// Returns opaque handle or null on failure.
BoltIpc* bolt_ipc_start(const char* socket_path, const char* app_version);

/// Check if IPC bridge is connected. Returns 1 if connected, 0 if not.
int bolt_ipc_is_connected(BoltIpc* handle);

/// Drain pending events as newline-separated JSON.
/// Each line: {"event": "...", "payload": {...}}
/// Returns null if no events. Caller must free with bolt_free_string.
char* bolt_ipc_drain_events(BoltIpc* handle);

/// Send a decision to the daemon.
/// msg_type: IPC message type (e.g. "pairing.decision").
/// payload_json: JSON string with decision payload.
/// Returns 1 on success, 0 on failure.
int bolt_ipc_send_decision(BoltIpc* handle, const char* msg_type, const char* payload_json);

/// Stop IPC bridge and free the handle.
void bolt_ipc_stop(BoltIpc* handle);

#ifdef __cplusplus
}
#endif

#endif // BOLT_NATIVE_BRIDGE_H
