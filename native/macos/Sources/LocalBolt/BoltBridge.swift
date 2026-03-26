import CBoltBridge
import Foundation

/// Swift-friendly wrapper around the Rust FFI bridge.
/// Converts C strings to Swift strings and handles memory management.
enum BoltBridge {

    /// Generate a secure 6-character peer code via Rust bolt-core.
    static func generatePeerCode() -> String {
        guard let ptr = bolt_generate_peer_code() else { return "ERROR" }
        let code = String(cString: ptr)
        bolt_free_string(ptr)
        return code
    }

    /// Get the platform data directory from bolt-app-core.
    static func platformDataDir() -> String {
        guard let ptr = bolt_platform_data_dir() else { return "" }
        let dir = String(cString: ptr)
        bolt_free_string(ptr)
        return dir
    }

    /// Get the platform IPC socket path from bolt-app-core.
    static func platformIpcPath() -> String {
        guard let ptr = bolt_platform_ipc_path() else { return "" }
        let path = String(cString: ptr)
        bolt_free_string(ptr)
        return path
    }

    /// Probe the signal server health.
    static func probeSignalHealth() -> Bool {
        bolt_probe_signal_health() == 1
    }

    /// Find the bolt-daemon binary.
    static func findDaemonBinary() -> String? {
        guard let ptr = bolt_daemon_find_binary() else { return nil }
        let path = String(cString: ptr)
        bolt_free_string(ptr)
        return path
    }
}

// ── Daemon Lifecycle ────────────────────────────────────────

/// Manages a bolt-daemon child process via the Rust FFI bridge.
/// Observable for SwiftUI binding.
@Observable
final class DaemonManager {
    private(set) var isRunning = false
    private(set) var pid: UInt32 = 0
    private(set) var wsPort: UInt16 = 0
    private(set) var recentStderr: String = ""
    private(set) var daemonBinaryPath: String?

    private var handle: OpaquePointer?
    private var pollTimer: Timer?

    init() {
        daemonBinaryPath = BoltBridge.findDaemonBinary()
    }

    /// Start the daemon. Returns true if started successfully.
    @discardableResult
    func start(wsPort: UInt16 = 0) -> Bool {
        guard let binPath = daemonBinaryPath else { return false }
        guard handle == nil else { return false } // already running

        handle = binPath.withCString { cPath in
            bolt_daemon_start(cPath, wsPort)
        }

        guard handle != nil else { return false }

        pid = bolt_daemon_pid(handle)
        self.wsPort = bolt_daemon_ws_port(handle)
        isRunning = true

        // Poll stderr every 500ms
        pollTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            self?.poll()
        }

        return true
    }

    /// Stop the daemon and clean up.
    func stop() {
        pollTimer?.invalidate()
        pollTimer = nil

        if let h = handle {
            bolt_daemon_stop(h)
            handle = nil
        }

        isRunning = false
        pid = 0
        wsPort = 0
        recentStderr = ""
    }

    /// Poll daemon state (called on timer).
    private func poll() {
        guard let h = handle else { return }

        let running = bolt_daemon_is_running(h) == 1
        if running != isRunning {
            isRunning = running
        }

        if let ptr = bolt_daemon_recent_stderr(h, 30) {
            recentStderr = String(cString: ptr)
            bolt_free_string(ptr)
        }

        // Auto-cleanup if daemon died
        if !running {
            pollTimer?.invalidate()
            pollTimer = nil
            handle = nil
            pid = 0
            wsPort = 0
        }
    }

    deinit {
        stop()
    }
}
