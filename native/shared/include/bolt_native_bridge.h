// bolt_native_bridge.h — C-ABI interface for native platform shells.
//
// Generated from bolt-native-bridge Rust crate.
// Import this header in Swift via a bridging header or module map.

#ifndef BOLT_NATIVE_BRIDGE_H
#define BOLT_NATIVE_BRIDGE_H

#ifdef __cplusplus
extern "C" {
#endif

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

#ifdef __cplusplus
}
#endif

#endif // BOLT_NATIVE_BRIDGE_H
