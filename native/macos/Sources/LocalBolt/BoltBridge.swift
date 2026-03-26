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
    private(set) var socketPath: String = ""
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
        if let ptr = bolt_daemon_socket_path(handle) {
            socketPath = String(cString: ptr)
            bolt_free_string(ptr)
        }
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
        socketPath = ""
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

// ── Signaling / Peer Discovery ──────────────────────────────

/// A discovered peer device.
struct DiscoveredPeer: Identifiable {
    let id: String // peer_code
    let peerCode: String
    let deviceName: String
    let deviceType: String
}

/// Manages signaling client and peer discovery via FFI.
@Observable
final class SignalingManager {
    private(set) var isConnected = false
    private(set) var peers: [DiscoveredPeer] = []
    private(set) var peerCode: String

    private var handle: OpaquePointer?
    private var pollTimer: Timer?

    init(peerCode: String) {
        self.peerCode = peerCode
    }

    /// Start signaling with local + optional cloud server.
    func start(localUrl: String, cloudUrl: String?) {
        guard handle == nil else { return }

        handle = peerCode.withCString { codeCStr in
            localUrl.withCString { localCStr in
                let deviceName = Host.current().localizedName ?? "Mac"
                return deviceName.withCString { nameCStr in
                    if let cloud = cloudUrl {
                        return cloud.withCString { cloudCStr in
                            bolt_signaling_start(localCStr, cloudCStr, codeCStr, nameCStr)
                        }
                    } else {
                        return bolt_signaling_start(localCStr, nil, codeCStr, nameCStr)
                    }
                }
            }
        }

        // Poll for peer updates every 500ms
        pollTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            self?.poll()
        }
    }

    /// Send a signal to a peer (connection initiation).
    func sendSignal(toPeerCode: String, signalType: String, dataJson: String? = nil) {
        guard let h = handle else { return }

        toPeerCode.withCString { toCStr in
            signalType.withCString { typeCStr in
                if let json = dataJson {
                    json.withCString { dataCStr in
                        let _ = bolt_signaling_send_signal(h, toCStr, typeCStr, dataCStr)
                    }
                } else {
                    let _ = bolt_signaling_send_signal(h, toCStr, typeCStr, nil)
                }
            }
        }
    }

    /// Stop signaling.
    func stop() {
        pollTimer?.invalidate()
        pollTimer = nil
        if let h = handle {
            bolt_signaling_stop(h)
            handle = nil
        }
        isConnected = false
        peers = []
    }

    private func poll() {
        guard let h = handle else { return }

        isConnected = bolt_signaling_is_connected(h) == 1

        // Drain events to keep the queue clean
        let _ = bolt_signaling_drain_events(h)

        // Read current peer list
        let count = bolt_signaling_peer_count(h)
        var updated: [DiscoveredPeer] = []
        for i in 0..<count {
            if let peerPtr = bolt_signaling_get_peer(h, i) {
                let peer = peerPtr.pointee
                let code = peer.peer_code.map { String(cString: $0) } ?? ""
                let name = peer.device_name.map { String(cString: $0) } ?? ""
                let type = peer.device_type.map { String(cString: $0) } ?? ""
                updated.append(DiscoveredPeer(
                    id: code, peerCode: code, deviceName: name, deviceType: type
                ))
                bolt_peer_free(peerPtr)
            }
        }
        if updated.count != peers.count || !updated.map(\.id).elementsEqual(peers.map(\.id)) {
            peers = updated
        }
    }

    deinit {
        stop()
    }
}

// ── IPC Bridge (daemon event stream) ────────────────────────

/// An incoming pairing request from a remote peer via daemon IPC.
struct PairingRequest: Identifiable {
    let id: String // request_id
    let requestId: String
    let deviceName: String
    let deviceType: String
    let sas: String
}

/// Manages IPC connection to the daemon for event forwarding and decisions.
@Observable
final class IpcManager {
    private(set) var isConnected = false
    var pendingRequest: PairingRequest?

    private var handle: OpaquePointer?
    private var pollTimer: Timer?

    /// Connect to daemon IPC socket.
    func start(socketPath: String, appVersion: String = "0.1.0") {
        guard handle == nil else { return }

        handle = socketPath.withCString { pathCStr in
            appVersion.withCString { verCStr in
                bolt_ipc_start(pathCStr, verCStr)
            }
        }

        guard handle != nil else {
            print("[IPC] start failed — daemon socket not available")
            return
        }

        isConnected = true

        pollTimer = Timer.scheduledTimer(withTimeInterval: 0.3, repeats: true) { [weak self] _ in
            self?.poll()
        }
    }

    /// Stop IPC bridge.
    func stop() {
        pollTimer?.invalidate()
        pollTimer = nil

        if let h = handle {
            bolt_ipc_stop(h)
            handle = nil
        }

        isConnected = false
        pendingRequest = nil
    }

    /// Send a pairing decision back to the daemon.
    func sendPairingDecision(requestId: String, accept: Bool) {
        guard let h = handle else { return }

        let decision = accept ? "allow_once" : "deny_once"
        let payload = """
        {"request_id":"\(requestId)","decision":"\(decision)"}
        """

        payload.withCString { payloadCStr in
            "pairing.decision".withCString { typeCStr in
                let _ = bolt_ipc_send_decision(h, typeCStr, payloadCStr)
            }
        }

        // Clear the pending request after decision
        pendingRequest = nil
    }

    private func poll() {
        guard let h = handle else { return }

        // Check connection
        let connected = bolt_ipc_is_connected(h) == 1
        if connected != isConnected {
            isConnected = connected
        }

        // Drain events
        guard let ptr = bolt_ipc_drain_events(h) else { return }
        let raw = String(cString: ptr)
        bolt_free_string(ptr)

        // Parse newline-separated JSON events
        for line in raw.split(separator: "\n") {
            guard let data = line.data(using: .utf8),
                  let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let event = json["event"] as? String,
                  let payload = json["payload"] as? [String: Any]
            else { continue }

            switch event {
            case "daemon://pairing-request":
                let req = PairingRequest(
                    id: payload["request_id"] as? String ?? UUID().uuidString,
                    requestId: payload["request_id"] as? String ?? "",
                    deviceName: payload["remote_device_name"] as? String ?? "Unknown Device",
                    deviceType: payload["remote_device_type"] as? String ?? "desktop",
                    sas: payload["sas"] as? String ?? ""
                )
                pendingRequest = req

            case "daemon://bridge-disconnected":
                isConnected = false
                stop()

            default:
                break
            }
        }
    }

    deinit {
        stop()
    }
}
