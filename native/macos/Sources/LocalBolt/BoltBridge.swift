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
    private(set) var dataDir: String = ""
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
        if let ptr = bolt_daemon_data_dir(handle) {
            dataDir = String(cString: ptr)
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
        dataDir = ""
        recentStderr = ""
    }

    /// Send a file to the connected peer via daemon.
    @discardableResult
    func sendFile(path: String) -> Bool {
        guard let h = handle else { return false }
        return path.withCString { cPath in
            bolt_daemon_send_file(h, cPath) == 1
        }
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

/// An incoming signaling signal from a remote peer.
struct IncomingSignal {
    let from: String
    let signalType: String
    let data: [String: Any]
}

/// Manages signaling client and peer discovery via FFI.
@Observable
final class SignalingManager {
    private(set) var isConnected = false
    private(set) var peers: [DiscoveredPeer] = []
    private(set) var peerCode: String
    /// Incoming connection request from a remote peer (set by poll, consumed by UI).
    var incomingConnectionRequest: IncomingSignal? = nil

    private var handle: OpaquePointer?
    private var pollTimer: Timer?
    /// Callback for incoming signals that need UI handling.
    var onIncomingSignal: ((IncomingSignal) -> Void)? = nil

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

        // Drain discovery events to keep the queue clean
        let _ = bolt_signaling_drain_events(h)

        // Drain incoming signals (connection_request, connection_accepted, etc.)
        if let sigPtr = bolt_signaling_drain_signals(h) {
            let raw = String(cString: sigPtr)
            bolt_free_string(sigPtr)
            for line in raw.split(separator: "\n") {
                guard let data = line.data(using: .utf8),
                      let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                      let from = json["from"] as? String,
                      let sigType = json["signal_type"] as? String
                else { continue }
                let sigData = json["data"] as? [String: Any] ?? [:]
                let signal = IncomingSignal(from: from, signalType: sigType, data: sigData)
                onIncomingSignal?(signal)
            }
        }

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

/// Session trust state.
enum TrustState: Equatable {
    case unverified(sas: String)
    case verified
    case legacy // peer lacks identity support
}

/// Connected peer session info.
struct PeerSession {
    let peerCode: String
    let deviceName: String
    let deviceType: String
    var trust: TrustState
}

/// Session lifecycle phase.
enum SessionPhase: Equatable {
    case idle
    case pairingPending
    case connected
    case disconnected(reason: String)
}

/// Transfer lifecycle phase.
enum TransferPhase: Equatable {
    case idle
    case sending(fileName: String, transferId: String, progress: Float)
    case receiving(fileName: String, transferId: String, progress: Float)
    case complete(fileName: String, savePath: String?)
    case failed(fileName: String, reason: String)
}

/// Manages IPC connection to the daemon for event forwarding and decisions.
@Observable
final class IpcManager {
    private(set) var isConnected = false
    var pendingRequest: PairingRequest?
    private(set) var sessionPhase: SessionPhase = .idle
    private(set) var connectedPeer: PeerSession?
    private(set) var connectedPeerCount: UInt32 = 0
    var transferPhase: TransferPhase = .idle

    private var handle: OpaquePointer?
    private var pollTimer: Timer?

    /// Connect to daemon IPC socket.
    func start(socketPath: String, appVersion: String = "0.0.1") {
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
        sessionPhase = .idle
        connectedPeer = nil
        connectedPeerCount = 0
        transferPhase = .idle
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

        if accept, let req = pendingRequest {
            // Transition to connected. If SAS was present, start as unverified.
            let trust: TrustState = req.sas.isEmpty ? .legacy : .unverified(sas: req.sas)
            connectedPeer = PeerSession(
                peerCode: req.requestId,
                deviceName: req.deviceName,
                deviceType: req.deviceType,
                trust: trust
            )
            sessionPhase = .connected
        }

        pendingRequest = nil
    }

    /// Mark the current session as verified (user confirmed SAS match).
    func markVerified() {
        connectedPeer?.trust = .verified
    }

    /// Clear transfer state back to idle.
    func clearTransfer() {
        transferPhase = .idle
    }

    /// Reset session phase to idle (dismiss disconnected notice).
    func resetSession() {
        sessionPhase = .idle
        connectedPeer = nil
    }

    /// Disconnect the current session.
    func disconnectSession() {
        guard let h = handle else { return }

        let payload = """
        {"reason":"user_initiated"}
        """
        payload.withCString { payloadCStr in
            "session.disconnect".withCString { typeCStr in
                let _ = bolt_ipc_send_decision(h, typeCStr, payloadCStr)
            }
        }

        sessionPhase = .disconnected(reason: "user initiated")
        connectedPeer = nil
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
                  let event = json["event"] as? String
            else { continue }

            let payload = json["payload"] as? [String: Any] ?? [:]

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
                sessionPhase = .pairingPending

            case "daemon://status-update":
                connectedPeerCount = UInt32(payload["connected_peers"] as? Int ?? 0)
                // If daemon reports 0 peers and we think we're connected, session ended
                if connectedPeerCount == 0 && sessionPhase == .connected {
                    sessionPhase = .disconnected(reason: "peer disconnected")
                    connectedPeer = nil
                }

            case "daemon://session-connected":
                // Future: daemon emits when WebRTC session established
                sessionPhase = .connected
                // Update peer info if available in payload
                if let peerCode = payload["remote_peer_id"] as? String {
                    if connectedPeer == nil {
                        connectedPeer = PeerSession(
                            peerCode: peerCode,
                            deviceName: "Peer",
                            deviceType: "desktop",
                            trust: .legacy
                        )
                    }
                }

            case "daemon://session-sas":
                // Future: daemon emits SAS for verification
                if let sas = payload["sas"] as? String {
                    connectedPeer?.trust = .unverified(sas: sas)
                }

            case "daemon://session-ended":
                let reason = payload["reason"] as? String ?? "unknown"
                sessionPhase = .disconnected(reason: reason)
                connectedPeer = nil

            case "daemon://session-error":
                let reason = payload["reason"] as? String ?? "unknown"
                sessionPhase = .disconnected(reason: reason)
                connectedPeer = nil

            case "daemon://transfer-started":
                let fileName = payload["file_name"] as? String ?? "file"
                let transferId = payload["transfer_id"] as? String ?? ""
                let direction = payload["direction"] as? String ?? "send"
                if direction == "receive" {
                    transferPhase = .receiving(fileName: fileName, transferId: transferId, progress: 0)
                } else {
                    transferPhase = .sending(fileName: fileName, transferId: transferId, progress: 0)
                }

            case "daemon://transfer-progress":
                let progress = (payload["progress"] as? NSNumber)?.floatValue ?? 0
                let transferId = payload["transfer_id"] as? String ?? ""
                // Update progress on the current phase without changing the phase type
                switch transferPhase {
                case .sending(let fn, _, _):
                    transferPhase = .sending(fileName: fn, transferId: transferId, progress: progress)
                case .receiving(let fn, _, _):
                    transferPhase = .receiving(fileName: fn, transferId: transferId, progress: progress)
                default:
                    break
                }

            case "daemon://transfer-complete":
                let fileName = payload["file_name"] as? String ?? "file"
                let savePath = payload["save_path"] as? String
                transferPhase = .complete(fileName: fileName, savePath: savePath)

            case "daemon://transfer-error":
                let fileName = payload["file_name"] as? String ?? "file"
                let reason = payload["reason"] as? String ?? "unknown"
                transferPhase = .failed(fileName: fileName, reason: reason)

            case "daemon://transfer-request":
                // Incoming file transfer request — handled through pairing flow for now
                break

            case "daemon://bridge-disconnected":
                isConnected = false
                sessionPhase = .disconnected(reason: "bridge lost")
                connectedPeer = nil
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
