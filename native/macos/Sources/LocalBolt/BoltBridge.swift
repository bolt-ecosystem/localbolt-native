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
    /// WebTransport URL (set after reading wt_info.json from daemon data_dir).
    var wtUrl: String?
    /// WebTransport cert hash hex (set after reading wt_info.json from daemon data_dir).
    var wtCertHash: String?

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

    /// Connect to a remote daemon's WS endpoint (NATIVE-CONNECT-1).
    @discardableResult
    func connectToRemote(wsUrl: String) -> Bool {
        guard let h = handle else { return false }
        return wsUrl.withCString { cUrl in
            bolt_daemon_connect_remote(h, cUrl) == 1
        }
    }

    /// Request the daemon to disconnect the active WS session (NATIVE-SESSION-UX-2).
    @discardableResult
    func requestDisconnect() -> Bool {
        guard let h = handle else { return false }
        return bolt_daemon_disconnect_session(h) == 1
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
    /// WebTransport URL (nil if peer doesn't support WT).
    let wtUrl: String?
    /// WebTransport TLS cert SHA-256 hash hex (nil if not available).
    let wtCertHash: String?
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
    /// Set true when remote peer declines connection (consumed by ContentView to clear connecting state).
    var connectionDeclined: Bool = false

    private var handle: OpaquePointer?
    private var pollTimer: Timer?
    /// Callback for incoming signals that need UI handling.
    var onIncomingSignal: ((IncomingSignal) -> Void)? = nil

    init(peerCode: String) {
        self.peerCode = peerCode
    }

    /// Start signaling with local + optional cloud server.
    /// `wtUrl` and `wtCertHash` are optional WebTransport metadata from the daemon.
    func start(localUrl: String, cloudUrl: String?, wtUrl: String? = nil, wtCertHash: String? = nil) {
        guard handle == nil else { return }

        // Helper: call FFI with properly-scoped C string pointers.
        // All withCString closures must be nested so pointers remain valid.
        func callStart(
            _ localCStr: UnsafePointer<CChar>,
            _ cloudCStr: UnsafePointer<CChar>?,
            _ codeCStr: UnsafePointer<CChar>,
            _ nameCStr: UnsafePointer<CChar>,
            _ wtUrlCStr: UnsafePointer<CChar>?,
            _ wtHashCStr: UnsafePointer<CChar>?
        ) -> OpaquePointer? {
            bolt_signaling_start(localCStr, cloudCStr, codeCStr, nameCStr, wtUrlCStr, wtHashCStr)
        }

        handle = peerCode.withCString { codeCStr in
            localUrl.withCString { localCStr in
                let deviceName = Host.current().localizedName ?? "Mac"
                return deviceName.withCString { nameCStr in
                    // Nest optional withCString calls to keep pointers alive
                    if let cloud = cloudUrl, let wtu = wtUrl, let wth = wtCertHash {
                        return cloud.withCString { cCloud in
                            wtu.withCString { cWt in
                                wth.withCString { cHash in
                                    callStart(localCStr, cCloud, codeCStr, nameCStr, cWt, cHash)
                                }
                            }
                        }
                    } else if let cloud = cloudUrl, let wtu = wtUrl {
                        return cloud.withCString { cCloud in
                            wtu.withCString { cWt in
                                callStart(localCStr, cCloud, codeCStr, nameCStr, cWt, nil)
                            }
                        }
                    } else if let cloud = cloudUrl {
                        return cloud.withCString { cCloud in
                            callStart(localCStr, cCloud, codeCStr, nameCStr, nil, nil)
                        }
                    } else {
                        return callStart(localCStr, nil, codeCStr, nameCStr, nil, nil)
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
                let rawWtUrl = peer.wt_url
                let rawWtHash = peer.wt_cert_hash
                let wtUrl: String? = (rawWtUrl != nil) ? String(cString: rawWtUrl!) : nil
                let wtHash: String? = (rawWtHash != nil) ? String(cString: rawWtHash!) : nil
                updated.append(DiscoveredPeer(
                    id: code, peerCode: code, deviceName: name, deviceType: type,
                    wtUrl: wtUrl, wtCertHash: wtHash
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

/// Session trust state (canonical v1: unverified, verified, legacy).
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
    /// Remote identity public key (base64). Used for TOFU pin persistence.
    var identityKeyB64: String?
}

// ── TOFU Pin Store (persistent identity verification) ─────

/// Persists verified peer identity keys to disk (JSON).
/// Implements PROTOCOL.md §2: "Known key match: proceed without user interaction."
final class PinStore {
    private let fileURL: URL
    private var pins: [String: PinEntry] = [:]

    struct PinEntry: Codable {
        var verified: Bool
        var firstSeen: Date
    }

    init(dataDir: String) {
        let dir = URL(fileURLWithPath: dataDir).appendingPathComponent("pins")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        fileURL = dir.appendingPathComponent("identity_pins.json")
        load()
    }

    /// Check if an identity key is already pinned and verified.
    func isVerified(identityKeyB64: String) -> Bool {
        pins[identityKeyB64]?.verified == true
    }

    /// Pin an identity key (unverified). No-op if already pinned.
    func pin(identityKeyB64: String) {
        guard pins[identityKeyB64] == nil else { return }
        pins[identityKeyB64] = PinEntry(verified: false, firstSeen: Date())
        save()
    }

    /// Mark an already-pinned identity key as verified.
    func markVerified(identityKeyB64: String) {
        guard pins[identityKeyB64] != nil else {
            pins[identityKeyB64] = PinEntry(verified: true, firstSeen: Date())
            save()
            return
        }
        pins[identityKeyB64]?.verified = true
        save()
    }

    private func load() {
        guard let data = try? Data(contentsOf: fileURL),
              let decoded = try? JSONDecoder().decode([String: PinEntry].self, from: data)
        else { return }
        pins = decoded
    }

    private func save() {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        encoder.dateEncodingStrategy = .iso8601
        guard let data = try? encoder.encode(pins) else { return }
        try? data.write(to: fileURL, options: .atomic)
    }
}

/// Canonical session phases (v1 contract: SESSION_CONTRACT.md).
/// `disconnected` is native presentation-only — not part of canonical contract.
enum SessionPhase: Equatable {
    // Canonical v1 phases
    case idle
    case requesting
    case incomingRequest
    case connecting
    case connected
    // Presentation-only (native): session ended, reason available for UI display.
    // Dismissed back to idle via resetSession().
    case disconnected(reason: String)
}

/// Transfer lifecycle phase (canonical v1 contract).
enum TransferPhase: Equatable {
    case idle
    case sending(fileName: String, transferId: String, progress: Float)
    case receiving(fileName: String, transferId: String, progress: Float)
    case complete(fileName: String, savePath: String?)
    case failed(fileName: String, reason: String)
}

// ── Transfer Gating Policy (canonical P1) ──────────────────

/// Returns true if file transfer is allowed (canonical policy P1).
/// Transfer is gated by: connected AND (verified OR legacy).
func isTransferAllowed(sessionPhase: SessionPhase, trust: TrustState) -> Bool {
    guard sessionPhase == .connected else { return false }
    switch trust {
    case .verified, .legacy: return true
    case .unverified: return false
    }
}

// ── Session Manager (canonical state owner) ────────────────

/// Manages IPC connection to the daemon for event forwarding and decisions.
/// Owns the canonical session/transfer/verification state (v1 contract).
@Observable
final class IpcManager {
    private(set) var isConnected = false
    var pendingRequest: PairingRequest?
    private(set) var sessionPhase: SessionPhase = .idle
    var connectedPeer: PeerSession?
    private(set) var connectedPeerCount: UInt32 = 0
    var transferPhase: TransferPhase = .idle

    /// Generation counter (INV-4). Increments on every canonical reset.
    /// Used to reject stale async callbacks (INV-5).
    private(set) var generation: UInt64 = 0

    /// Peer info stashed on accept, applied when daemon confirms session.
    var pendingAcceptPeer: PeerSession?
    /// Peer info stashed by initiator for name resolution on session.connected.
    var pendingInitiatorPeer: DiscoveredPeer?

    /// TOFU pin store for persistent identity verification.
    private(set) var pinStore: PinStore?

    private var handle: OpaquePointer?
    private var pollTimer: Timer?

    /// Connect to daemon IPC socket with retry.
    /// - Parameters:
    ///   - dataDir: Daemon data directory for pin store persistence.
    func start(socketPath: String, appVersion: String = "0.0.1", dataDir: String? = nil) {
        if let dir = dataDir, pinStore == nil {
            pinStore = PinStore(dataDir: dir)
        }
        guard handle == nil else { return }
        startWithRetry(socketPath: socketPath, appVersion: appVersion, attempt: 1)
    }

    private func startWithRetry(socketPath: String, appVersion: String, attempt: Int) {
        let maxAttempts = 5

        let result = socketPath.withCString { pathCStr in
            appVersion.withCString { verCStr in
                bolt_ipc_start(pathCStr, verCStr)
            }
        }

        if let h = result {
            handle = h
            isConnected = true
            print("[IPC] connected on attempt \(attempt)")
            pollTimer = Timer.scheduledTimer(withTimeInterval: 0.3, repeats: true) { [weak self] _ in
                self?.poll()
            }
        } else if attempt < maxAttempts {
            print("[IPC] attempt \(attempt)/\(maxAttempts) failed — retrying in 500ms")
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
                self?.startWithRetry(socketPath: socketPath, appVersion: appVersion, attempt: attempt + 1)
            }
        } else {
            print("[IPC] all \(maxAttempts) attempts failed — IPC unavailable")
            isConnected = false
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
        resetSession()
    }

    // ── Canonical session phase transitions ──────────────────

    /// Transition: idle → requesting (user selects peer).
    func beginRequest(peer: DiscoveredPeer) -> Bool {
        guard sessionPhase == .idle else { return false }
        sessionPhase = .requesting
        pendingInitiatorPeer = peer
        return true
    }

    /// Transition: idle → incomingRequest (signal received from remote).
    func receiveRequest() -> Bool {
        guard sessionPhase == .idle else { return false }
        sessionPhase = .incomingRequest
        return true
    }

    /// Transition: requesting|incomingRequest → connecting (accepted).
    func beginConnecting() -> Bool {
        guard sessionPhase == .requesting || sessionPhase == .incomingRequest else { return false }
        sessionPhase = .connecting
        return true
    }

    /// Transition: connecting → connected (handshake complete).
    /// Called by daemon session.connected IPC event.
    func markConnected() -> Bool {
        guard sessionPhase == .connecting else { return false }
        sessionPhase = .connected
        return true
    }

    /// Canonical reset: any → idle (v1 contract P3).
    /// Clears all session, transfer, and verification state atomically.
    /// Increments generation counter (INV-4).
    @discardableResult
    func resetSession() -> UInt64 {
        generation += 1
        sessionPhase = .idle
        connectedPeer = nil
        connectedPeerCount = 0
        transferPhase = .idle
        pendingRequest = nil
        pendingAcceptPeer = nil
        pendingInitiatorPeer = nil
        return generation
    }

    /// Presentation-only disconnect: connected → disconnected(reason).
    /// Then user dismisses to idle via resetSession().
    /// Caller must also call daemon.requestDisconnect() to close WS.
    func disconnectSession(reason: String) {
        sessionPhase = .disconnected(reason: reason)
        connectedPeer = nil
        transferPhase = .idle
    }

    /// Check if a generation is still current (INV-5).
    func isCurrentGeneration(_ gen: UInt64) -> Bool {
        return gen == generation
    }

    // ── Decisions ────────────────────────────────────────────

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
            pendingAcceptPeer = PeerSession(
                peerCode: req.requestId,
                deviceName: req.deviceName,
                deviceType: req.deviceType,
                trust: req.sas.isEmpty ? .legacy : .unverified(sas: req.sas),
                identityKeyB64: nil // resolved on session-connected
            )
        }

        pendingRequest = nil
    }

    /// Mark the current session as verified (user confirmed SAS match).
    /// Persists to TOFU pin store so future reconnects skip SAS.
    func markVerified() {
        connectedPeer?.trust = .verified
        if let key = connectedPeer?.identityKeyB64 {
            pinStore?.markVerified(identityKeyB64: key)
            print("[TOFU] identity pinned as verified: \(key.prefix(8))...")
        }
    }

    /// Clear transfer state back to idle.
    func clearTransfer() {
        transferPhase = .idle
    }

    /// Dismiss disconnected notice (presentation-only → canonical idle).
    func dismissDisconnect() {
        guard case .disconnected = sessionPhase else { return }
        resetSession()
    }

    // ── IPC Event Polling ────────────────────────────────────

    private func poll() {
        guard let h = handle else { return }

        let connected = bolt_ipc_is_connected(h) == 1
        if connected != isConnected {
            isConnected = connected
        }

        guard let ptr = bolt_ipc_drain_events(h) else { return }
        let raw = String(cString: ptr)
        bolt_free_string(ptr)

        let gen = generation // capture for stale check

        for line in raw.split(separator: "\n") {
            // Reject events from a stale generation
            guard isCurrentGeneration(gen) else { break }

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

            case "daemon://status-update":
                connectedPeerCount = UInt32(payload["connected_peers"] as? Int ?? 0)
                if connectedPeerCount == 0 && sessionPhase == .connected {
                    disconnectSession(reason: "peer disconnected")
                }

            case "daemon://session-connected":
                // Transition through connecting → connected.
                // If we're in requesting or incomingRequest, advance to connecting first.
                if sessionPhase == .requesting || sessionPhase == .incomingRequest {
                    _ = beginConnecting()
                }
                let remoteIdentityKey = payload["remote_peer_id"] as? String
                if markConnected() {
                    // Resolve peer info: acceptor has pendingAcceptPeer,
                    // initiator has pendingInitiatorPeer.
                    if let pending = pendingAcceptPeer {
                        var peer = pending
                        peer.identityKeyB64 = remoteIdentityKey
                        connectedPeer = peer
                        pendingAcceptPeer = nil
                    } else if let target = pendingInitiatorPeer {
                        let remotePk = remoteIdentityKey ?? target.peerCode
                        connectedPeer = PeerSession(
                            peerCode: remotePk,
                            deviceName: target.deviceName,
                            deviceType: target.deviceType,
                            trust: .legacy,
                            identityKeyB64: remoteIdentityKey
                        )
                        pendingInitiatorPeer = nil
                    } else if let peerCode = remoteIdentityKey {
                        connectedPeer = PeerSession(
                            peerCode: peerCode,
                            deviceName: "Peer",
                            deviceType: "desktop",
                            trust: .legacy,
                            identityKeyB64: remoteIdentityKey
                        )
                    }
                }

            case "daemon://session-sas":
                if let sas = payload["sas"] as? String {
                    let identityKey = payload["remote_identity_pk_b64"] as? String
                        ?? connectedPeer?.identityKeyB64
                    // TOFU: check pin store for previously verified identity
                    if let key = identityKey, pinStore?.isVerified(identityKeyB64: key) == true {
                        // Known verified peer — skip SAS (PROTOCOL.md §2)
                        connectedPeer?.trust = .verified
                        print("[TOFU] known verified identity — SAS skipped")
                    } else {
                        connectedPeer?.trust = .unverified(sas: sas)
                        // Pin the identity key (unverified) for future reference
                        if let key = identityKey {
                            connectedPeer?.identityKeyB64 = key
                            pinStore?.pin(identityKeyB64: key)
                        }
                    }
                }

            case "daemon://session-ended":
                let reason = payload["reason"] as? String ?? "unknown"
                disconnectSession(reason: reason)

            case "daemon://session-error":
                let reason = payload["reason"] as? String ?? "unknown"
                disconnectSession(reason: reason)

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
                break

            case "daemon://bridge-disconnected":
                isConnected = false
                disconnectSession(reason: "bridge lost")
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
