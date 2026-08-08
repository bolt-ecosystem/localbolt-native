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
    /// QUIC host:port (set after reading quic_info.json from daemon data_dir).
    var quicAddr: String?
    /// QUIC cert hash hex (set after reading quic_info.json from daemon data_dir).
    var quicCertHash: String?
    /// True when daemon has an active WT session (detected from stderr).
    /// Latches on "[WT_SESSION]...ACTIVE_SESSION registered", clears on "cleared".
    private(set) var wtSessionActive: Bool = false
    /// SAS code extracted from daemon stderr "[SAS] XXXXXX" for WT sessions.
    private(set) var wtSessionSAS: String? = nil

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
        wtUrl = nil
        wtCertHash = nil
        quicAddr = nil
        quicCertHash = nil
    }

    /// Send a file to the connected peer via daemon.
    @discardableResult
    func sendFile(path: String) -> Bool {
        guard let h = handle else { return false }
        return path.withCString { cPath in
            bolt_daemon_send_file(h, cPath) == 1
        }
    }

    /// Connect to a remote daemon with WS fallback plus optional QUIC metadata.
    @discardableResult
    func connectToRemote(wsUrl: String, quicAddr: String? = nil, quicCertHash: String? = nil) -> Bool {
        guard let h = handle else { return false }
        return wsUrl.withCString { cUrl in
            if let quicAddr, let quicCertHash {
                return quicAddr.withCString { cQuicAddr in
                    quicCertHash.withCString { cQuicHash in
                        bolt_daemon_connect_remote_v2(h, cUrl, cQuicAddr, cQuicHash) == 1
                    }
                }
            }
            return bolt_daemon_connect_remote_v2(h, cUrl, nil, nil) == 1
        }
    }

    /// Allow the requester QUIC cert hash before accepting an inbound request.
    @discardableResult
    func allowQuicPeerCertHash(_ quicCertHash: String?) -> Bool {
        guard let h = handle, let quicCertHash, !quicCertHash.isEmpty else { return false }
        return quicCertHash.withCString { cHash in
            bolt_daemon_allow_quic_peer_cert_hash(h, cHash) == 1
        }
    }

    /// Request the daemon to disconnect the active WS session (NATIVE-SESSION-UX-2).
    @discardableResult
    func requestDisconnect() -> Bool {
        guard let h = handle else { return false }
        return bolt_daemon_disconnect_session(h) == 1
    }

    /// Pause the active outbound transfer (DAEMON-TRANSFER-CONTROL-1).
    @discardableResult
    func pauseTransfer() -> Bool {
        guard let h = handle else { return false }
        return bolt_daemon_pause_transfer(h) == 1
    }

    /// Resume a paused outbound transfer (DAEMON-TRANSFER-CONTROL-1).
    @discardableResult
    func resumeTransfer() -> Bool {
        guard let h = handle else { return false }
        return bolt_daemon_resume_transfer(h) == 1
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

        // Detect WT session lifecycle from daemon stderr.
        // The bridge exposes a rolling tail, so parse in order and honor only
        // the latest lifecycle token. Otherwise stale "cleared" or "[SAS]"
        // lines from a previous session can poison reconnect state.
        var latestWtLifecycle: Bool?
        var pendingSas: String?
        var latestRegisteredSas: String?
        for line in recentStderr.split(separator: "\n") {
            if let range = line.range(of: "[SAS] ") {
                let candidate = String(line[range.upperBound...]).prefix(6)
                if candidate.count == 6 {
                    pendingSas = String(candidate)
                }
            }
            if line.contains("ACTIVE_SESSION registered") {
                latestWtLifecycle = true
                latestRegisteredSas = pendingSas
            } else if line.contains("ACTIVE_SESSION cleared") {
                latestWtLifecycle = false
                latestRegisteredSas = nil
            }
        }

        if let latestWtLifecycle {
            if latestWtLifecycle {
                if !wtSessionActive {
                    wtSessionActive = true
                    wtSessionSAS = nil
                    print("[DAEMON-POLL] WT session detected from stderr")
                }
                if let sas = latestRegisteredSas, wtSessionSAS != sas {
                    wtSessionSAS = sas
                    print("[DAEMON-POLL] WT SAS extracted: \(sas)")
                }
            } else if wtSessionActive {
                wtSessionActive = false
                wtSessionSAS = nil
                print("[DAEMON-POLL] WT session ended (stderr)")
            }
        }

        // Auto-cleanup if daemon died
        if !running {
            wtSessionActive = false
            wtSessionSAS = nil
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

/// A user's answer to a pairing request.
///
/// `wireValue` must match the daemon's `Decision` serde representation
/// (snake_case) exactly, or the daemon cannot parse the decision and falls back to
/// its fail-closed deny. The `_always` cases are the only ones the daemon persists
/// as a pin; the `_once` cases authorize or refuse this session and persist nothing.
enum PairingDecision {
    case allowOnce
    case allowAlways
    case denyOnce
    case denyAlways

    var wireValue: String {
        switch self {
        case .allowOnce: return "allow_once"
        case .allowAlways: return "allow_always"
        case .denyOnce: return "deny_once"
        case .denyAlways: return "deny_always"
        }
    }

    var isAllow: Bool {
        switch self {
        case .allowOnce, .allowAlways: return true
        case .denyOnce, .denyAlways: return false
        }
    }
}

/// Session trust state (canonical v1: unverified, verified, legacy + M7 mismatch).
enum TrustState: Equatable {
    case unverified(sas: String)
    case verified
    case legacy // peer lacks identity support
    /// M7: TOFU identity mismatch — device name seen before with different key.
    case mismatch(deviceName: String, oldKeyPrefix: String)
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

// ── TOFU Pin Store (key-continuity only, pre-EA1) ─────

/// Persists peer identity keys to disk (JSON) for TOFU key-continuity / mismatch
/// detection. Item-6 / pre-EA1: it does NOT persist a "verified" flag and never asserts
/// a session is verified — real device verification requires EA1/PAKE, which does not
/// exist yet. The `PinEntry.verified` field is vestigial (see below).
final class PinStore {
    private let fileURL: URL
    private var pins: [String: PinEntry] = [:]

    struct PinEntry: Codable {
        /// Item-6 / pre-EA1: legacy field, no longer trusted. Nothing writes `true`
        /// anymore and `isVerified` ignores it; kept only for on-disk shape compatibility.
        var verified: Bool
        var firstSeen: Date
        var deviceName: String?
    }

    init(dataDir: String) {
        let dir = URL(fileURLWithPath: dataDir).appendingPathComponent("pins")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        fileURL = dir.appendingPathComponent("identity_pins.json")
        load()
    }

    /// Item-6 / pre-EA1: always false. No persisted `verified` flag may be treated as
    /// proof of device identity (no EA1/PAKE verification exists), so a stored value —
    /// including one written before this change — is never surfaced as verified.
    func isVerified(identityKeyB64: String) -> Bool {
        return false
    }

    /// Pin an identity key for key-continuity. No-op if already pinned.
    func pin(identityKeyB64: String, deviceName: String? = nil) {
        guard pins[identityKeyB64] == nil else { return }
        pins[identityKeyB64] = PinEntry(verified: false, firstSeen: Date(), deviceName: deviceName)
        save()
    }

    /// Item-6 / pre-EA1: no-op. Session approval is NOT persisted as a verified pin —
    /// there is no cryptographic device verification yet, so nothing may write
    /// `verified: true`. Kept for call-site compatibility; deliberately persists nothing.
    func markVerified(identityKeyB64: String, deviceName: String? = nil) {
        // intentionally no persistence pre-EA1
    }

    /// M7: Check if a device name was previously pinned with a different identity key.
    /// Returns the old identity key (truncated) if mismatch detected, nil otherwise.
    /// Item-6: keys on ANY previously-pinned device name (TOFU key-continuity), not just
    /// "verified" ones — pins are no longer marked verified, so mismatch detection must
    /// not depend on that flag.
    func checkMismatch(identityKeyB64: String, deviceName: String) -> String? {
        for (key, entry) in pins where entry.deviceName == deviceName && key != identityKeyB64 {
            return String(key.prefix(12)) + "..."
        }
        return nil
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

    var canStartFreshConnection: Bool {
        switch self {
        case .idle, .disconnected:
            return true
        case .requesting, .incomingRequest, .connecting, .connected:
            return false
        }
    }
}

/// Transfer lifecycle phase (canonical v1 contract).
enum TransferPhase: Equatable {
    case idle
    case sending(fileName: String, transferId: String, progress: Float)
    case receiving(fileName: String, transferId: String, progress: Float)
    case complete(fileName: String, savePath: String?)
    case failed(fileName: String, reason: String)

    /// S6: Whether an active transfer is in progress (for speed/ETA tracking).
    var isActive: Bool {
        switch self {
        case .sending, .receiving: return true
        default: return false
        }
    }
}

// ── Transfer Gating Policy (canonical P1) ──────────────────

/// Returns true if file transfer is allowed (canonical policy P1).
/// Transfer is gated by: connected AND (verified OR legacy).
func isTransferAllowed(sessionPhase: SessionPhase, trust: TrustState) -> Bool {
    guard sessionPhase == .connected else { return false }
    switch trust {
    case .verified, .legacy: return true
    case .unverified, .mismatch: return false
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
    /// S6: File size from transfer.started event (for speed/ETA display).
    var transferFileSizeBytes: UInt64 = 0
    /// S5: Whether the current outbound transfer is paused (DAEMON-TRANSFER-CONTROL-1).
    var transferPaused: Bool = false

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

    private func resetDisconnectedPresentationIfNeeded() {
        guard case .disconnected = sessionPhase else { return }
        resetSession()
    }

    /// Transition: idle → requesting (user selects peer).
    func beginRequest(peer: DiscoveredPeer) -> Bool {
        resetDisconnectedPresentationIfNeeded()
        guard sessionPhase == .idle else { return false }
        sessionPhase = .requesting
        pendingInitiatorPeer = peer
        return true
    }

    /// Transition: idle → incomingRequest (signal received from remote).
    func receiveRequest() -> Bool {
        resetDisconnectedPresentationIfNeeded()
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
        transferFileSizeBytes = 0
        transferPaused = false
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
        connectedPeerCount = 0
        transferPhase = .idle
        transferFileSizeBytes = 0
        transferPaused = false
        pendingRequest = nil
        pendingAcceptPeer = nil
        pendingInitiatorPeer = nil
    }

    /// Check if a generation is still current (INV-5).
    func isCurrentGeneration(_ gen: UInt64) -> Bool {
        return gen == generation
    }

    // ── Decisions ────────────────────────────────────────────

    /// Send a pairing decision back to the daemon.
    ///
    /// The `_always` cases are what allow the daemon to persist a pin (Stage B), so a
    /// device is remembered rather than re-prompted on every reconnect. The `_once`
    /// cases authorize this session only and persist nothing.
    func sendPairingDecision(requestId: String, decision: PairingDecision) {
        guard let h = handle else { return }

        let payload = """
        {"request_id":"\(requestId)","decision":"\(decision.wireValue)"}
        """

        payload.withCString { payloadCStr in
            "pairing.decision".withCString { typeCStr in
                let _ = bolt_ipc_send_decision(h, typeCStr, payloadCStr)
            }
        }

        if decision.isAllow, let req = pendingRequest {
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

    /// Approve the current session (user confirmed the SAS match).
    /// Item-6 / pre-EA1: session-scoped ONLY. This is NOT cryptographic device
    /// verification and is NOT persisted, so it does not survive a reconnect — the SAS
    /// must be reviewed again next session. The internal `.verified` case name is kept.
    func markVerified() {
        connectedPeer?.trust = .verified
        print("[TOFU] session approved by user (session-scoped, not persisted)")
    }

    /// Clear transfer state back to idle.
    func clearTransfer() {
        transferPhase = .idle
        transferFileSizeBytes = 0
        transferPaused = false
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
                    let peerName = connectedPeer?.deviceName
                    // M7: Check for TOFU identity mismatch before anything else
                    if let key = identityKey, let name = peerName,
                       let oldKey = pinStore?.checkMismatch(identityKeyB64: key, deviceName: name) {
                        connectedPeer?.trust = .mismatch(deviceName: name, oldKeyPrefix: oldKey)
                        connectedPeer?.identityKeyB64 = key
                        print("[TOFU] IDENTITY MISMATCH for \(name) — old key: \(oldKey)")
                    }
                    // Item-6 / pre-EA1: NO reconnect auto-trust. A known identity key is
                    // never silently promoted to verified from a stored pin — the user
                    // reviews the SAS every session. Pin (unverified) for key-continuity.
                    else {
                        connectedPeer?.trust = .unverified(sas: sas)
                        if let key = identityKey {
                            connectedPeer?.identityKeyB64 = key
                            pinStore?.pin(identityKeyB64: key, deviceName: peerName)
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
                transferFileSizeBytes = (payload["file_size_bytes"] as? NSNumber)?.uint64Value ?? 0
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

            case "daemon://transfer-paused":
                transferPaused = true

            case "daemon://transfer-resumed":
                transferPaused = false

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
