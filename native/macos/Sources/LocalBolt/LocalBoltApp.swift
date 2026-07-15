import AppKit
import SwiftUI
import UniformTypeIdentifiers

// Brand color: #A4E200
private let neon = Color(red: 0.64, green: 0.88, blue: 0)
private let darkBg = Color(red: 0.08, green: 0.08, blue: 0.08)
private let darkAccent = Color(red: 0.12, green: 0.12, blue: 0.12)

/// Get the local LAN IP address for wsUrl in signaling payloads.
private func localIPAddress() -> String {
    var address = "127.0.0.1"
    var ifaddr: UnsafeMutablePointer<ifaddrs>?
    guard getifaddrs(&ifaddr) == 0, let firstAddr = ifaddr else { return address }
    defer { freeifaddrs(ifaddr) }
    for ptr in sequence(first: firstAddr, next: { $0.pointee.ifa_next }) {
        let sa = ptr.pointee.ifa_addr.pointee
        guard sa.sa_family == UInt8(AF_INET) else { continue }
        let name = String(cString: ptr.pointee.ifa_name)
        guard name == "en0" || name == "en1" else { continue }
        var hostname = [CChar](repeating: 0, count: Int(NI_MAXHOST))
        getnameinfo(ptr.pointee.ifa_addr, socklen_t(sa.sa_len),
                    &hostname, socklen_t(hostname.count), nil, 0, NI_NUMERICHOST)
        address = String(cString: hostname)
        break
    }
    return address
}

/// UI-only connecting animation phase (product-specific, not canonical).
enum ConnectingPhase {
    case requesting   // "Waiting for [device] to accept..."
    case establishing // "Establishing secure connection..."
    case slow         // "Still connecting... taking longer than usual"
}

@main
struct LocalBoltApp: App {
    @State private var daemon = DaemonManager()
    @State private var signaling: SignalingManager
    @State private var ipc = IpcManager()
    @State private var showDiagnostics = false

    init() {
        let code = BoltBridge.generatePeerCode()
        _signaling = State(initialValue: SignalingManager(peerCode: code))
    }

    var body: some Scene {
        WindowGroup {
            ContentView(daemon: daemon, signaling: signaling, ipc: ipc, showDiagnostics: $showDiagnostics)
                .onAppear {
                    autoStart()
                    wireSignalingHandler()
                }
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 420, height: 600)
        .commands {
            CommandGroup(after: .appInfo) {
                Button("Diagnostics") { showDiagnostics.toggle() }
                    .keyboardShortcut("d", modifiers: [.command, .option])
            }
        }
    }

    /// Wire incoming signal handler so connection_request signals surface as pairing requests.
    private func wireSignalingHandler() {
        signaling.onIncomingSignal = { [self] signal in
            switch signal.signalType {
            case "connection_request":
                guard ipc.receiveRequest() else {
                    print("[CONNECT] ignoring connection_request while sessionPhase=\(String(describing: ipc.sessionPhase))")
                    return
                }
                let deviceName = signal.data["deviceName"] as? String ?? "Unknown Device"
                let deviceType = signal.data["deviceType"] as? String ?? "desktop"
                var requestData = signal.data
                requestData["deviceName"] = deviceName
                requestData["deviceType"] = deviceType
                signaling.incomingConnectionRequest = IncomingSignal(
                    from: signal.from,
                    signalType: signal.signalType,
                    data: requestData
                )
            case "connection_accepted":
                guard ipc.beginConnecting() else {
                    print("[CONNECT] ignoring connection_accepted while sessionPhase=\(String(describing: ipc.sessionPhase))")
                    return
                }
                if let wsUrl = signal.data["wsUrl"] as? String, !wsUrl.isEmpty {
                    daemon.connectToRemote(
                        wsUrl: wsUrl,
                        quicAddr: signal.data["quicAddr"] as? String,
                        quicCertHash: signal.data["quicCertHash"] as? String
                    )
                } else {
                    // Browser peer accepted — no wsUrl (browsers can't host a WS server).
                    // Stay in .connecting and wait for the browser to connect back to our
                    // daemon's WebTransport endpoint. The WT session detection in
                    // onChange(of: daemon.wtSessionActive) will advance to .connected.
                    print("[CONNECT] connection_accepted without wsUrl — waiting for inbound WT session from browser")
                }
            case "connection_declined":
                // Remote peer declined — M2 onChange handler shows notice + resets
                signaling.connectionDeclined = true
            default:
                break
            }
        }
    }

    private func autoStart() {
        guard daemon.daemonBinaryPath != nil, !daemon.isRunning else { return }
        daemon.start()
        let wtInfoPath = "\(daemon.dataDir)/wt_info.json"
        let quicInfoPath = "\(daemon.dataDir)/quic_info.json"
        let pollInterval: TimeInterval = 0.25
        let maxAttempts = 20 // 5s total timeout
        DispatchQueue.global(qos: .utility).async {
            var attempts = 0
            while attempts < maxAttempts {
                if FileManager.default.fileExists(atPath: wtInfoPath) {
                    break
                }
                attempts += 1
                Thread.sleep(forTimeInterval: pollInterval)
            }
            DispatchQueue.main.async {
                // Read WT metadata (best-effort — nil values are safe)
                if let data = try? Data(contentsOf: URL(fileURLWithPath: wtInfoPath)),
                   let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                    if let port = json["wt_port"] as? Int {
                        self.daemon.wtUrl = "https://\(localIPAddress()):\(port)"
                    }
                    self.daemon.wtCertHash = json["wt_cert_hash"] as? String
                }
                // Read QUIC metadata for native app-to-app routing; WS remains fallback.
                if let data = try? Data(contentsOf: URL(fileURLWithPath: quicInfoPath)),
                   let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                    if let port = json["quic_port"] as? Int {
                        self.daemon.quicAddr = "\(localIPAddress()):\(port)"
                    }
                    self.daemon.quicCertHash = json["quic_cert_hash"] as? String
                }
                self.signaling.start(
                    localUrl: "ws://127.0.0.1:3001",
                    cloudUrl: "wss://bolt-rendezvous.fly.dev",
                    wtUrl: self.daemon.wtUrl,
                    wtCertHash: self.daemon.wtCertHash
                )
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                    self.ipc.start(socketPath: self.daemon.socketPath, dataDir: self.daemon.dataDir)
                }
            }
        }
    }
}

// MARK: - Content View

struct ContentView: View {
    @Bindable var daemon: DaemonManager
    @Bindable var signaling: SignalingManager
    @Bindable var ipc: IpcManager
    @Binding var showDiagnostics: Bool
    @State private var showDeviceList = false
    @State private var isDragOver = false
    // UI-only animation phase (product-specific, not canonical truth)
    @State private var connectingPhase: ConnectingPhase = .requesting
    @State private var connectingTimer: Timer? = nil
    /// Transient notice shown when remote peer declines connection (M2).
    @State private var declinedNotice: String? = nil
    /// Transient notice shown after SAS rejection (M3).
    @State private var rejectionNotice: String? = nil
    /// M4/M5: File queue — selected files awaiting explicit send.
    @State private var fileQueue: [URL] = []
    /// M7: TOFU identity mismatch alert text.
    @State private var mismatchAlert: String? = nil
    /// S7: Disconnect confirmation dialog.
    @State private var showDisconnectConfirm = false
    /// S6: Transfer start time for speed/ETA calculation.
    @State private var transferStartTime: Date? = nil

    var body: some View {
        VStack(spacing: 0) {
            // ── Header ──
            HStack {
                Image(systemName: "bolt.fill")
                    .font(.system(size: 12))
                    .foregroundColor(neon)
                Text("LocalBolt")
                    .font(.system(size: 13, weight: .bold, design: .monospaced))
                    .foregroundColor(.white.opacity(0.9))
                Spacer()
                Circle()
                    .fill(signaling.isConnected ? neon.opacity(0.7) : Color.red.opacity(0.7))
                    .frame(width: 6, height: 6)
                Text(signaling.isConnected ? "ONLINE" : "OFFLINE")
                    .font(.system(size: 10, design: .monospaced))
                    .tracking(1)
                    .foregroundColor(.white.opacity(0.3))
            }
            .padding(.horizontal, 16)
            .frame(height: 48)
            .background(darkBg.opacity(0.8))

            Rectangle().fill(.white.opacity(0.06)).frame(height: 1)

            // ── Main surface ──
            if !daemon.isRunning {
                Spacer()
                startupView
                Spacer()
            } else {
                Spacer()
                transferCard
                    .padding(.horizontal, 20)
                Spacer()
            }
        }
        .frame(minWidth: 380, minHeight: 500)
        .background(darkBg)
        .sheet(item: $ipc.pendingRequest) { request in
            PairingRequestView(request: request, ipc: ipc)
        }
        .sheet(isPresented: $showDiagnostics) {
            DiagnosticsView(daemon: daemon, ipc: ipc, signaling: signaling)
        }
        .onChange(of: signaling.connectionDeclined) {
            if signaling.connectionDeclined {
                signaling.connectionDeclined = false
                clearConnectingUI()
                // M2: Show visible feedback that remote declined
                ipc.resetSession()
                withAnimation { declinedNotice = "Connection declined by remote device" }
                DispatchQueue.main.asyncAfter(deadline: .now() + 3.0) {
                    withAnimation { declinedNotice = nil }
                }
            }
        }
        // S6: Track transfer start time for incoming receives
        .onChange(of: ipc.transferPhase.isActive) {
            if ipc.transferPhase.isActive && transferStartTime == nil {
                transferStartTime = Date()
            } else if !ipc.transferPhase.isActive {
                transferStartTime = nil
            }
        }
        // WT session detection: daemon stderr indicates session up, but IPC
        // events are not emitted for WebTransport sessions. Advance the
        // state machine here so the UI reflects the connected session.
        .onChange(of: daemon.wtSessionActive) {
            if daemon.wtSessionActive && ipc.sessionPhase == .connecting {
                _ = ipc.markConnected()
                if let pending = ipc.pendingAcceptPeer {
                    ipc.connectedPeer = pending
                    ipc.pendingAcceptPeer = nil
                } else if let target = ipc.pendingInitiatorPeer {
                    ipc.connectedPeer = PeerSession(
                        peerCode: target.peerCode,
                        deviceName: target.deviceName,
                        deviceType: target.deviceType,
                        trust: .legacy,
                        identityKeyB64: nil
                    )
                    ipc.pendingInitiatorPeer = nil
                }
                applyWtVerificationCode(daemon.wtSessionSAS)
                print("[WT-DETECT] Advanced to .connected from daemon stderr")
            }
            if !daemon.wtSessionActive && ipc.sessionPhase == .connected {
                ipc.disconnectSession(reason: "WT session ended")
                print("[WT-DETECT] WT session ended — disconnected")
            }
        }
        // WT SAS: apply verification state from daemon stderr
        .onChange(of: daemon.wtSessionSAS) {
            applyWtVerificationCode(daemon.wtSessionSAS)
        }
    }

    private func applyWtVerificationCode(_ sas: String?) {
        guard let sas, ipc.sessionPhase == .connected else { return }
        // Item-6 / pre-EA1: no reconnect auto-trust — always surface the SAS as
        // unverified for user review; a stored pin never skips it.
        ipc.connectedPeer?.trust = .unverified(sas: sas)
        print("[WT-DETECT] SAS from daemon stderr: \(sas)")
    }

    // MARK: - Startup

    private var startupView: some View {
        VStack(spacing: 12) {
            if daemon.daemonBinaryPath == nil {
                Image(systemName: "exclamationmark.triangle")
                    .font(.system(size: 28))
                    .foregroundColor(.yellow)
                Text("Unable to start")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundColor(.white.opacity(0.8))
            } else {
                ProgressView()
                    .scaleEffect(0.7)
                    .tint(neon)
                Text("Starting...")
                    .font(.system(size: 12))
                    .foregroundColor(.white.opacity(0.4))
            }
        }
    }

    // MARK: - Transfer Card

    private var transferCard: some View {
        VStack(spacing: 0) {
            // S4: E2E badge — prominent when connected, subtle otherwise
            if ipc.sessionPhase == .connected {
                HStack(spacing: 6) {
                    Image(systemName: "lock.shield.fill")
                        .font(.system(size: 13))
                        .foregroundColor(neon.opacity(0.8))
                    Text("End-to-End Encrypted")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundColor(neon.opacity(0.8))
                }
                .padding(.vertical, 14)
                .frame(maxWidth: .infinity)
                .background(neon.opacity(0.04))
            } else {
                HStack(spacing: 6) {
                    Image(systemName: "lock.shield")
                        .font(.system(size: 13))
                        .foregroundColor(.white.opacity(0.2))
                    Text("End-to-End Encrypted")
                        .font(.system(size: 13))
                        .foregroundColor(.white.opacity(0.2))
                }
                .padding(.vertical, 14)
            }

            Rectangle().fill(.white.opacity(0.06)).frame(height: 1)

            // Card content: canonical phase drives view selection
            switch ipc.sessionPhase {
            case .connected:
                if let peer = ipc.connectedPeer {
                    sessionContent(peer: peer)
                        .onAppear { clearConnectingUI() }
                }
            case .incomingRequest:
                if let req = signaling.incomingConnectionRequest {
                    incomingRequestContent(request: req)
                }
            case .requesting, .connecting:
                if let target = ipc.pendingInitiatorPeer {
                    connectingContent(target: target)
                } else if let peer = ipc.pendingAcceptPeer {
                    // Acceptor-side: show connecting UI using stashed accept peer info
                    connectingContent(deviceName: peer.deviceName)
                }
            case .idle, .disconnected:
                discoveryContent
            }

            // M2: Declined-by-remote inline notice
            if let notice = declinedNotice {
                Rectangle().fill(.white.opacity(0.06)).frame(height: 1)
                HStack(spacing: 8) {
                    Image(systemName: "hand.raised")
                        .font(.system(size: 12))
                        .foregroundColor(.red.opacity(0.7))
                    Text(notice)
                        .font(.system(size: 12))
                        .foregroundColor(.white.opacity(0.5))
                    Spacer()
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .transition(.opacity)
            }

            // M3: SAS rejection inline notice
            if let notice = rejectionNotice {
                Rectangle().fill(.white.opacity(0.06)).frame(height: 1)
                HStack(spacing: 8) {
                    Image(systemName: "shield.slash")
                        .font(.system(size: 12))
                        .foregroundColor(.red.opacity(0.7))
                    Text(notice)
                        .font(.system(size: 12))
                        .foregroundColor(.white.opacity(0.5))
                    Spacer()
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .transition(.opacity)
            }

            // Presentation-only disconnect notice
            if case .disconnected(let reason) = ipc.sessionPhase {
                Rectangle().fill(.white.opacity(0.06)).frame(height: 1)
                HStack {
                    Text("Disconnected: \(reason)")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.3))
                    Spacer()
                    Button("Dismiss") { ipc.dismissDisconnect() }
                        .buttonStyle(.plain)
                        .font(.system(size: 11))
                        .foregroundColor(neon)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
            }
        }
        .background(darkAccent.opacity(0.6))
        .cornerRadius(12)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(neon.opacity(0.15), lineWidth: 1)
        )
        .shadow(color: neon.opacity(0.08), radius: 30)
    }

    // MARK: - Discovery

    private var discoveryContent: some View {
        VStack(spacing: 0) {
            if !showDeviceList {
                Button(action: { withAnimation(.easeInOut(duration: 0.15)) { showDeviceList = true } }) {
                    HStack(spacing: 8) {
                        Image(systemName: "iphone")
                            .font(.system(size: 13))
                            .foregroundColor(.white.opacity(0.4))
                        Text("Devices")
                            .font(.system(size: 13))
                            .foregroundColor(.white.opacity(0.7))
                        if signaling.peers.count > 0 {
                            Text("\(signaling.peers.count)")
                                .font(.system(size: 11, weight: .bold))
                                .foregroundColor(neon)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(neon.opacity(0.12))
                                .cornerRadius(10)
                        }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                    .background(.white.opacity(0.03))
                    .cornerRadius(8)
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .stroke(.white.opacity(0.08), lineWidth: 1)
                    )
                }
                .buttonStyle(.plain)
                .padding(.vertical, 16)

                Text(signaling.peerCode)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundColor(.white.opacity(0.2))
                    .padding(.bottom, 12)
            } else {
                VStack(spacing: 0) {
                    HStack {
                        Text("Select a device")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundColor(.white.opacity(0.8))
                        Spacer()
                        Button(action: { withAnimation { showDeviceList = false } }) {
                            Image(systemName: "xmark")
                                .font(.system(size: 11))
                                .foregroundColor(.white.opacity(0.4))
                        }
                        .buttonStyle(.plain)
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 12)

                    if signaling.peers.isEmpty {
                        HStack(spacing: 8) {
                            ProgressView().scaleEffect(0.5).tint(.white.opacity(0.3))
                            Text("Searching for nearby devices...")
                                .font(.system(size: 12))
                                .foregroundColor(.white.opacity(0.4))
                        }
                        .padding(.vertical, 16)
                        .padding(.horizontal, 16)
                    } else {
                        ForEach(signaling.peers) { peer in
                            Rectangle().fill(.white.opacity(0.04)).frame(height: 1)
                            peerRow(peer)
                        }
                    }
                }
                .padding(.bottom, 8)
            }
        }
    }

    private func peerRow(_ peer: DiscoveredPeer) -> some View {
        Button(action: {
            initiateConnection(to: peer)
        }) {
            HStack(spacing: 10) {
                Image(systemName: deviceIcon(peer.deviceType))
                    .font(.system(size: 14))
                    .foregroundColor(.white.opacity(0.4))
                    .frame(width: 20)
                // S1: Device name only — peer code removed for cleaner discovery
                Text(peer.deviceName)
                    .font(.system(size: 13))
                    .foregroundColor(.white.opacity(0.8))
                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .background(.white.opacity(0.001))
        .disabled(!ipc.isConnected || !ipc.sessionPhase.canStartFreshConnection)
    }

    // MARK: - Connecting (UI animation, product-specific)

    private func connectingContent(target: DiscoveredPeer) -> some View {
        VStack(spacing: 12) {
            ZStack {
                Circle()
                    .fill(neon.opacity(0.05))
                    .frame(width: 40, height: 40)
                    .scaleEffect(connectingPhase == .slow ? 1.2 : 1.0)
                    .animation(.easeInOut(duration: 1.0).repeatForever(autoreverses: true), value: connectingPhase)
                Circle()
                    .fill(neon.opacity(0.1))
                    .frame(width: 24, height: 24)
                Circle()
                    .fill(neon.opacity(0.4))
                    .frame(width: 12, height: 12)
            }
            .padding(.top, 8)

            VStack(spacing: 4) {
                switch connectingPhase {
                case .requesting:
                    Text("Waiting for **\(target.deviceName)** to accept...")
                        .font(.system(size: 13))
                        .foregroundColor(.white.opacity(0.8))
                    Text("The other device will be asked to confirm")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.3))
                case .establishing:
                    Text("Establishing secure connection with **\(target.deviceName)**...")
                        .font(.system(size: 13))
                        .foregroundColor(.white.opacity(0.8))
                    Text("Setting up encrypted channel")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.3))
                case .slow:
                    // S2: Actionable slow-connection copy
                    Text("Still connecting to **\(target.deviceName)**...")
                        .font(.system(size: 13))
                        .foregroundColor(.white.opacity(0.8))
                    Text("Check that both devices are on the same network")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.3))
                }
            }
            .multilineTextAlignment(.center)

            Button("Cancel") {
                // M1: Send decline signal to remote before resetting
                if let target = ipc.pendingInitiatorPeer {
                    signaling.sendSignal(toPeerCode: target.peerCode, signalType: "connection_declined")
                }
                ipc.resetSession()
                clearConnectingUI()
            }
            .buttonStyle(.plain)
            .font(.system(size: 12))
            .foregroundColor(.white.opacity(0.3))
            .padding(.bottom, 8)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
    }

    /// Acceptor-side connecting UI (peer accepted, waiting for WT/WS session).
    private func connectingContent(deviceName: String) -> some View {
        VStack(spacing: 12) {
            ZStack {
                Circle()
                    .fill(neon.opacity(0.05))
                    .frame(width: 40, height: 40)
                Circle()
                    .fill(neon.opacity(0.1))
                    .frame(width: 24, height: 24)
                Circle()
                    .fill(neon.opacity(0.4))
                    .frame(width: 12, height: 12)
            }
            .padding(.top, 8)

            Text("Establishing secure connection with **\(deviceName)**...")
                .font(.system(size: 13))
                .foregroundColor(.white.opacity(0.8))
                .multilineTextAlignment(.center)
            Text("Waiting for encrypted channel")
                .font(.system(size: 11))
                .foregroundColor(.white.opacity(0.3))

            Button("Cancel") {
                ipc.resetSession()
                clearConnectingUI()
            }
            .buttonStyle(.plain)
            .font(.system(size: 12))
            .foregroundColor(.white.opacity(0.3))
            .padding(.bottom, 8)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
    }

    // MARK: - Incoming Request (inline)

    private func incomingRequestContent(request: IncomingSignal) -> some View {
        let deviceName = request.data["deviceName"] as? String ?? "Unknown Device"
        let deviceType = request.data["deviceType"] as? String ?? "desktop"

        return VStack(spacing: 12) {
            HStack(spacing: 10) {
                Image(systemName: deviceIcon(deviceType))
                    .font(.system(size: 20))
                    .foregroundColor(neon.opacity(0.6))
                VStack(alignment: .leading, spacing: 2) {
                    Text(deviceName)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundColor(.white.opacity(0.9))
                    // S8: Subtext clarifies what accepting means
                    Text("wants to connect")
                        .font(.system(size: 12))
                        .foregroundColor(.white.opacity(0.4))
                    Text("Accept to start sharing files")
                        .font(.system(size: 10))
                        .foregroundColor(.white.opacity(0.25))
                }
                Spacer()
            }

            HStack(spacing: 12) {
                Button(action: {
                    signaling.sendSignal(toPeerCode: request.from, signalType: "connection_declined")
                    signaling.incomingConnectionRequest = nil
                    ipc.resetSession()
                }) {
                    Text("Decline")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .tint(.red.opacity(0.8))

                Button(action: {
                    let wsPort = daemon.wsPort
                    let wsUrl = "ws://\(localIPAddress()):\(wsPort)"
                    // Stash peer info before accepting so a fast inbound session
                    // can resolve the connected peer deterministically.
                    ipc.pendingAcceptPeer = PeerSession(
                        peerCode: request.from,
                        deviceName: deviceName,
                        deviceType: deviceType,
                        trust: .legacy,
                        identityKeyB64: nil // resolved on session-connected
                    )
                    guard ipc.beginConnecting() else {
                        ipc.pendingAcceptPeer = nil
                        signaling.incomingConnectionRequest = nil
                        print("[CONNECT] refusing accept while sessionPhase=\(String(describing: ipc.sessionPhase))")
                        return
                    }
                    daemon.allowQuicPeerCertHash(request.data["quicCertHash"] as? String)
                    var acceptDict: [String: String] = ["wsUrl": wsUrl]
                    if let wtUrl = daemon.wtUrl { acceptDict["wtUrl"] = wtUrl }
                    if let wtHash = daemon.wtCertHash { acceptDict["certHash"] = wtHash }
                    if let quicAddr = daemon.quicAddr { acceptDict["quicAddr"] = quicAddr }
                    if let quicHash = daemon.quicCertHash { acceptDict["quicCertHash"] = quicHash }
                    let acceptJson = (try? JSONSerialization.data(withJSONObject: acceptDict))
                        .flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
                    signaling.sendSignal(
                        toPeerCode: request.from,
                        signalType: "connection_accepted",
                        dataJson: acceptJson
                    )
                    signaling.incomingConnectionRequest = nil
                }) {
                    Text("Accept")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .tint(neon)
            }
        }
        .padding(16)
    }

    // MARK: - Session

    private func sessionContent(peer: PeerSession) -> some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "link.circle.fill")
                    .font(.system(size: 20))
                    .foregroundColor(neon)
                VStack(alignment: .leading, spacing: 1) {
                    Text(peer.deviceName)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundColor(.white.opacity(0.9))
                    trustLabel(peer: peer)
                }
                Spacer()
                // S7: Disconnect with confirmation dialog
                Button(action: { showDisconnectConfirm = true }) {
                    Image(systemName: "xmark")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.3))
                }
                .buttonStyle(.plain)
                .help("Disconnect")
                .alert("Disconnect?", isPresented: $showDisconnectConfirm) {
                    Button("Cancel", role: .cancel) {}
                    Button("Disconnect", role: .destructive) {
                        daemon.requestDisconnect()
                        ipc.disconnectSession(reason: "user initiated")
                    }
                } message: {
                    Text("This will end the secure session with \(peer.deviceName).")
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)

            // M7: TOFU identity mismatch — security alert (blocks transfer)
            if case .mismatch(let name, let oldKey) = peer.trust {
                Rectangle().fill(.white.opacity(0.06)).frame(height: 1)
                VStack(spacing: 10) {
                    HStack(spacing: 6) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(.system(size: 16))
                            .foregroundColor(.red)
                        Text("Security Warning")
                            .font(.system(size: 13, weight: .bold))
                            .foregroundColor(.red)
                    }
                    Text("The identity key for \"\(name)\" has changed since it was last seen.")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.6))
                        .multilineTextAlignment(.center)
                    Text("Previous key: \(oldKey)")
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundColor(.white.opacity(0.3))
                    Text("This may indicate a security threat. Do not proceed unless you expect this change.")
                        .font(.system(size: 10))
                        .foregroundColor(.red.opacity(0.6))
                        .multilineTextAlignment(.center)
                    Button(action: {
                        daemon.requestDisconnect()
                        ipc.disconnectSession(reason: "identity mismatch")
                    }) {
                        Text("Disconnect")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(.red)
                    .controlSize(.regular)
                }
                .padding(16)
            }

            // SAS verification prompt (blocks transfer per P1)
            if case .unverified(let sas) = peer.trust {
                Rectangle().fill(.white.opacity(0.06)).frame(height: 1)
                VStack(spacing: 10) {
                    Text("Compare this code with the other device:")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.4))
                    Text(sas)
                        .font(.system(size: 22, weight: .bold, design: .monospaced))
                        .foregroundColor(.yellow)
                        .tracking(4)
                    HStack(spacing: 12) {
                        // M3: SAS reject button — disconnect session, show notice
                        Button(action: {
                            daemon.requestDisconnect()
                            ipc.disconnectSession(reason: "identity not verified")
                            withAnimation { rejectionNotice = "Connection closed — peer identity was not verified" }
                            DispatchQueue.main.asyncAfter(deadline: .now() + 4.0) {
                                withAnimation { rejectionNotice = nil }
                            }
                        }) {
                            Text("Reject")
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.bordered)
                        .tint(.red.opacity(0.8))
                        .controlSize(.regular)

                        // Item-6: session-scoped approval, not a verification claim.
                        Button(action: { ipc.markVerified() }) {
                            Text("Approve this session")
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(neon)
                        .controlSize(.regular)
                    }
                }
                .padding(16)
            }

            // Transfer area — gated by canonical policy P1
            if isTransferAllowed(sessionPhase: ipc.sessionPhase, trust: peer.trust) {
                Rectangle().fill(.white.opacity(0.06)).frame(height: 1)
                transferArea
            }
        }
    }

    @ViewBuilder
    private func trustLabel(peer: PeerSession) -> some View {
        switch peer.trust {
        case .verified:
            // Item-6 / pre-EA1: user approved this session after comparing the SAS. Not
            // cryptographic device verification, so the badge says "Approved", not
            // "Verified", and uses an approval check rather than a security shield.
            HStack(spacing: 4) {
                Image(systemName: "checkmark.circle")
                    .font(.system(size: 10))
                    .foregroundColor(neon.opacity(0.7))
                Text("Approved")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(neon.opacity(0.7))
            }
        case .unverified:
            HStack(spacing: 4) {
                Image(systemName: "exclamationmark.shield")
                    .font(.system(size: 10))
                    .foregroundColor(.yellow.opacity(0.7))
                Text("Unverified")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.yellow.opacity(0.7))
            }
        case .legacy:
            HStack(spacing: 4) {
                Image(systemName: "shield.slash")
                    .font(.system(size: 10))
                    .foregroundColor(.white.opacity(0.25))
                Text("Legacy")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.white.opacity(0.25))
            }
        case .mismatch:
            HStack(spacing: 4) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 10))
                    .foregroundColor(.red)
                Text("Identity Mismatch")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.red)
            }
        }
    }

    // MARK: - Transfer Area

    private var transferArea: some View {
        Group {
            switch ipc.transferPhase {
            case .idle:
                if fileQueue.isEmpty {
                    // M4: Drop zone — files go to queue, not auto-sent
                    VStack(spacing: 12) {
                        Image(systemName: "square.and.arrow.up")
                            .font(.system(size: 24))
                            .foregroundColor(.white.opacity(isDragOver ? 0.6 : 0.3))
                        Text("Drop files here")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundColor(.white.opacity(0.7))
                        Text("or click to select files")
                            .font(.system(size: 12))
                            .foregroundColor(.white.opacity(0.3))
                        Button("Select Files") { pickFiles() }
                            .buttonStyle(.bordered)
                            .tint(.white.opacity(0.5))
                            .controlSize(.small)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 20)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .strokeBorder(style: StrokeStyle(lineWidth: 2, dash: [6]))
                            .foregroundColor(.white.opacity(isDragOver ? 0.2 : 0.08))
                    )
                    .padding(16)
                    .onDrop(of: [.fileURL], isTargeted: $isDragOver) { providers in
                        handleDrop(providers)
                    }
                } else {
                    // M4/M5: File queue — review before sending
                    fileQueueView
                }

            case .sending(let fileName, _, let progress):
                transferProgress(icon: "arrow.up.circle", label: "Sending", fileName: fileName, progress: progress, color: neon)

            case .receiving(let fileName, _, let progress):
                transferProgress(icon: "arrow.down.circle", label: "Receiving", fileName: fileName, progress: progress, color: .blue)

            case .complete(let fileName, let savePath):
                VStack(spacing: 8) {
                    HStack(spacing: 8) {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundColor(neon)
                        Text(fileName)
                            .font(.system(size: 12))
                            .foregroundColor(.white.opacity(0.7))
                        Spacer()
                        if let path = savePath {
                            Button("Reveal") { NSWorkspace.shared.selectFile(path, inFileViewerRootedAtPath: "") }
                                .buttonStyle(.bordered)
                                .controlSize(.mini)
                        }
                        Button("Done") { sendNextOrClear() }
                            .buttonStyle(.bordered)
                            .controlSize(.mini)
                    }
                    // M5: Show remaining queue count
                    if !fileQueue.isEmpty {
                        Text("\(fileQueue.count) file\(fileQueue.count == 1 ? "" : "s") remaining")
                            .font(.system(size: 10))
                            .foregroundColor(.white.opacity(0.3))
                    }
                }
                .padding(16)

            case .failed(let fileName, let reason):
                HStack(spacing: 8) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.red)
                    Text("\(fileName): \(reason)")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.4))
                    Spacer()
                    Button("Dismiss") {
                        fileQueue.removeAll()
                        ipc.clearTransfer()
                    }
                        .buttonStyle(.bordered)
                        .controlSize(.mini)
                }
                .padding(16)
            }
        }
    }

    // M4/M5: File queue view — shows selected files with remove + explicit send
    private var fileQueueView: some View {
        VStack(spacing: 8) {
            HStack {
                Text("\(fileQueue.count) file\(fileQueue.count == 1 ? "" : "s") selected")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundColor(.white.opacity(0.7))
                Spacer()
                Button(action: { pickFiles() }) {
                    Image(systemName: "plus")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.4))
                }
                .buttonStyle(.plain)
                .help("Add more files")
            }

            // File list with individual remove
            ScrollView {
                VStack(spacing: 2) {
                    ForEach(Array(fileQueue.enumerated()), id: \.offset) { index, url in
                        HStack(spacing: 6) {
                            Image(systemName: "doc")
                                .font(.system(size: 10))
                                .foregroundColor(.white.opacity(0.3))
                            Text(url.lastPathComponent)
                                .font(.system(size: 11))
                                .foregroundColor(.white.opacity(0.6))
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Spacer()
                            // M5: Individual file remove
                            Button(action: { fileQueue.remove(at: index) }) {
                                Image(systemName: "xmark")
                                    .font(.system(size: 9))
                                    .foregroundColor(.white.opacity(0.3))
                            }
                            .buttonStyle(.plain)
                        }
                        .padding(.vertical, 3)
                    }
                }
            }
            .frame(maxHeight: 100)

            // M4: Explicit send + clear buttons
            HStack(spacing: 8) {
                Button("Clear") { fileQueue.removeAll() }
                    .buttonStyle(.bordered)
                    .tint(.white.opacity(0.4))
                    .controlSize(.small)
                Button("Send \(fileQueue.count) File\(fileQueue.count == 1 ? "" : "s")") {
                    sendFirstFile()
                }
                .buttonStyle(.borderedProminent)
                .tint(neon)
                .controlSize(.small)
            }
        }
        .padding(16)
        .onDrop(of: [.fileURL], isTargeted: $isDragOver) { providers in
            handleDrop(providers)
        }
    }

    private func transferProgress(icon: String, label: String, fileName: String, progress: Float, color: Color) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Image(systemName: icon).foregroundColor(color)
                Text("\(label) \(fileName)")
                    .font(.system(size: 12))
                    .foregroundColor(.white.opacity(0.6))
                Spacer()
                Text("\(Int(progress * 100))%")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundColor(color)
            }
            ProgressView(value: Double(progress))
                .tint(color)
            // S6: Speed + ETA stats
            HStack {
                if let stats = transferStats(progress: progress) {
                    Text(stats)
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundColor(.white.opacity(0.3))
                }
                Spacer()
                // S5: Pause/Resume toggle (send only, DAEMON-TRANSFER-CONTROL-1)
                if case .sending = ipc.transferPhase {
                    Button(ipc.transferPaused ? "Resume" : "Pause") {
                        if ipc.transferPaused {
                            daemon.resumeTransfer()
                        } else {
                            daemon.pauseTransfer()
                        }
                    }
                    .buttonStyle(.plain)
                    .font(.system(size: 11))
                    .foregroundColor(.white.opacity(0.5))
                }
                // M6: Cancel transfer button
                Button("Cancel Transfer") {
                    fileQueue.removeAll()
                    daemon.requestDisconnect()
                    ipc.disconnectSession(reason: "transfer cancelled")
                }
                .buttonStyle(.plain)
                .font(.system(size: 11))
                .foregroundColor(.red.opacity(0.6))
            }
        }
        .padding(16)
    }

    /// S6: Calculate speed and ETA from transfer progress + elapsed time.
    private func transferStats(progress: Float) -> String? {
        guard let startTime = transferStartTime, progress > 0.01 else { return nil }
        let elapsed = Date().timeIntervalSince(startTime)
        guard elapsed > 0.5 else { return nil } // avoid jitter in first half-second

        let totalBytes = ipc.transferFileSizeBytes
        guard totalBytes > 0 else { return nil }

        let bytesTransferred = Double(totalBytes) * Double(progress)
        let bytesPerSecond = bytesTransferred / elapsed

        let speedStr: String
        if bytesPerSecond >= 1_048_576 {
            speedStr = String(format: "%.1f MB/s", bytesPerSecond / 1_048_576)
        } else if bytesPerSecond >= 1024 {
            speedStr = String(format: "%.0f KB/s", bytesPerSecond / 1024)
        } else {
            speedStr = String(format: "%.0f B/s", bytesPerSecond)
        }

        let remaining = Double(1.0 - progress)
        let etaSeconds = (remaining / Double(progress)) * elapsed
        let etaStr: String
        if etaSeconds < 60 {
            etaStr = String(format: "%.0fs left", etaSeconds)
        } else if etaSeconds < 3600 {
            etaStr = String(format: "%.0fm %02.0fs left", floor(etaSeconds / 60), etaSeconds.truncatingRemainder(dividingBy: 60))
        } else {
            etaStr = String(format: "%.0fh %02.0fm left", floor(etaSeconds / 3600), floor(etaSeconds.truncatingRemainder(dividingBy: 3600) / 60))
        }

        return "\(speedStr)  ·  \(etaStr)"
    }

    // MARK: - Connection Management

    func initiateConnection(to peer: DiscoveredPeer) {
        // Canonical transition: idle → requesting
        guard ipc.beginRequest(peer: peer) else { return }
        withAnimation(.easeInOut(duration: 0.15)) {
            showDeviceList = false
        }

        // Send connection_request with wsUrl plus optional WT/QUIC metadata.
        let wsPort = daemon.wsPort
        let wsUrl = "ws://\(localIPAddress()):\(wsPort)"
        let deviceName = Host.current().localizedName ?? "Mac"
        var payloadDict: [String: String] = [
            "deviceName": deviceName,
            "deviceType": "desktop",
            "wsUrl": wsUrl,
        ]
        if let wtUrl = daemon.wtUrl { payloadDict["wtUrl"] = wtUrl }
        if let wtHash = daemon.wtCertHash { payloadDict["certHash"] = wtHash }
        if let quicAddr = daemon.quicAddr { payloadDict["quicAddr"] = quicAddr }
        if let quicHash = daemon.quicCertHash { payloadDict["quicCertHash"] = quicHash }
        let payload = (try? JSONSerialization.data(withJSONObject: payloadDict))
            .flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
        signaling.sendSignal(toPeerCode: peer.peerCode, signalType: "connection_request", dataJson: payload)

        // UI-only animation phase timers (product-specific)
        startConnectingTimers()
    }

    /// Clear UI-only connecting animation state.
    func clearConnectingUI() {
        connectingTimer?.invalidate()
        connectingTimer = nil
        connectingPhase = .requesting
    }

    private func startConnectingTimers() {
        connectingTimer?.invalidate()
        connectingPhase = .requesting
        connectingTimer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: false) { [self] _ in
            if ipc.sessionPhase == .requesting || ipc.sessionPhase == .connecting {
                withAnimation { connectingPhase = .establishing }
                connectingTimer = Timer.scheduledTimer(withTimeInterval: 5.0, repeats: false) { _ in
                    if ipc.sessionPhase == .requesting || ipc.sessionPhase == .connecting {
                        withAnimation { connectingPhase = .slow }
                    }
                }
            }
        }
    }

    // MARK: - Helpers

    // M4: File selection adds to queue — no auto-send.
    // M5: Multi-file selection enabled.
    func pickFiles() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.begin { response in
            guard response == .OK else { return }
            let newUrls = panel.urls.filter { url in !self.fileQueue.contains(url) }
            self.fileQueue.append(contentsOf: newUrls)
        }
    }

    // M4/M5: Drop adds to queue, not auto-send.
    func handleDrop(_ providers: [NSItemProvider]) -> Bool {
        for provider in providers {
            provider.loadItem(forTypeIdentifier: UTType.fileURL.identifier, options: nil) { item, _ in
                guard let data = item as? Data, let url = URL(dataRepresentation: data, relativeTo: nil) else { return }
                DispatchQueue.main.async {
                    if !self.fileQueue.contains(url) {
                        self.fileQueue.append(url)
                    }
                }
            }
        }
        return true
    }

    // M4: Explicit send — sends first file from queue.
    func sendFirstFile() {
        guard let url = fileQueue.first else { return }
        fileQueue.removeFirst()
        transferStartTime = Date() // S6: track start for speed/ETA
        ipc.transferPhase = .sending(fileName: url.lastPathComponent, transferId: "pending", progress: 0)
        daemon.sendFile(path: url.path)
    }

    // M5: After transfer complete, send next in queue or clear.
    func sendNextOrClear() {
        if !fileQueue.isEmpty {
            sendFirstFile()
        } else {
            ipc.clearTransfer()
        }
    }

    func deviceIcon(_ type: String) -> String {
        switch type {
        case "desktop": return "desktopcomputer"
        case "laptop": return "laptopcomputer"
        case "phone": return "iphone"
        case "tablet": return "ipad"
        default: return "display"
        }
    }
}

// MARK: - Pairing Request Sheet

struct PairingRequestView: View {
    let request: PairingRequest
    let ipc: IpcManager
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "person.crop.circle.badge.questionmark")
                .font(.system(size: 36))
                .foregroundColor(neon)

            Text("Connection Request")
                .font(.system(.title3, design: .monospaced))
                .fontWeight(.bold)

            VStack(spacing: 4) {
                Text(request.deviceName)
                    .font(.system(size: 15, weight: .semibold))
                Text(request.deviceType)
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
            }

            if !request.sas.isEmpty {
                VStack(spacing: 4) {
                    Text("Verification Code")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundColor(.secondary)
                    Text(request.sas)
                        .font(.system(size: 22, weight: .bold, design: .monospaced))
                        .foregroundColor(neon)
                        .tracking(4)
                }
                .padding()
                .background(neon.opacity(0.06))
                .cornerRadius(8)

                Text("Confirm this code matches on the other device")
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }

            HStack(spacing: 16) {
                Button("Decline") {
                    ipc.sendPairingDecision(requestId: request.requestId, accept: false)
                    dismiss()
                }
                .buttonStyle(.bordered)
                .tint(.red)

                Button("Accept") {
                    ipc.sendPairingDecision(requestId: request.requestId, accept: true)
                    dismiss()
                }
                .buttonStyle(.borderedProminent)
                .tint(neon)
            }
        }
        .padding(28)
        .frame(width: 300)
    }
}

// MARK: - Diagnostics (Cmd+Option+D)

struct DiagnosticsView: View {
    let daemon: DaemonManager
    let ipc: IpcManager
    let signaling: SignalingManager
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Diagnostics")
                    .font(.system(.title3, design: .monospaced))
                    .fontWeight(.bold)
                Spacer()
                Button("Close") { dismiss() }
                    .keyboardShortcut(.escape)
            }

            Divider()

            Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 6) {
                GridRow {
                    Text("Daemon").foregroundColor(.secondary)
                    Text(daemon.isRunning ? "Running (PID \(daemon.pid))" : "Stopped")
                }
                GridRow {
                    Text("WS Port").foregroundColor(.secondary)
                    Text(daemon.isRunning ? ":\(daemon.wsPort)" : "-")
                }
                GridRow {
                    Text("IPC").foregroundColor(.secondary)
                    Text(ipc.isConnected ? "Connected" : "Disconnected")
                        .foregroundColor(ipc.isConnected ? .green : .red)
                }
                GridRow {
                    Text("Signaling").foregroundColor(.secondary)
                    Text(signaling.isConnected ? "Connected" : "Disconnected")
                        .foregroundColor(signaling.isConnected ? .green : .red)
                }
                GridRow {
                    Text("Peers").foregroundColor(.secondary)
                    Text("\(signaling.peers.count)")
                }
                GridRow {
                    Text("Session").foregroundColor(.secondary)
                    Text("\(String(describing: ipc.sessionPhase))")
                }
                GridRow {
                    Text("Generation").foregroundColor(.secondary)
                    Text("\(ipc.generation)")
                }
            }
            .font(.system(size: 12, design: .monospaced))

            if daemon.isRunning && !daemon.recentStderr.isEmpty {
                Divider()
                Text("Daemon Log")
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .foregroundColor(.secondary)
                ScrollView {
                    Text(daemon.recentStderr)
                        .font(.system(size: 9, design: .monospaced))
                        .foregroundColor(.green.opacity(0.7))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .textSelection(.enabled)
                }
                .frame(maxHeight: 200)
                .background(Color.black.opacity(0.3))
                .cornerRadius(4)
            }
        }
        .padding(20)
        .frame(minWidth: 500, minHeight: 300)
    }
}
