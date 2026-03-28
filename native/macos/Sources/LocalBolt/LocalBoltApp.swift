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

/// Connection request phases (matches web: requesting → establishing → slow)
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
                let deviceName = signal.data["deviceName"] as? String ?? "Unknown Device"
                let deviceType = signal.data["deviceType"] as? String ?? "desktop"
                // Surface as an incoming connection request (reuse pairing request UI)
                ipc.pendingRequest = PairingRequest(
                    id: signal.from,
                    requestId: signal.from,
                    deviceName: deviceName,
                    deviceType: deviceType,
                    sas: "" // SAS comes later after session established
                )
                // Send connection_accepted back with our wsUrl so the browser can connect
                let wsPort = daemon.wsPort
                let wsUrl = "ws://\(localIPAddress()):\(wsPort)"
                signaling.sendSignal(
                    toPeerCode: signal.from,
                    signalType: "connection_accepted",
                    dataJson: "{\"wsUrl\":\"\(wsUrl)\"}"
                )
            case "connection_accepted":
                // Remote peer accepted our request — they may provide wsUrl for direct connect
                // (browser→native: browser accepted, provides nothing — session via WebRTC)
                // (native→browser: browser accepted, we already have WS)
                break
            case "connection_declined":
                // Remote peer declined — clear connecting state
                break
            default:
                break
            }
        }
    }

    private func autoStart() {
        guard daemon.daemonBinaryPath != nil, !daemon.isRunning else { return }
        daemon.start()
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            signaling.start(localUrl: "ws://127.0.0.1:3001", cloudUrl: "wss://bolt-rendezvous.fly.dev")
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
            ipc.start(socketPath: daemon.socketPath)
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
    @State private var connectingTo: DiscoveredPeer? = nil
    @State private var connectingPhase: ConnectingPhase = .requesting
    @State private var connectingTimer: Timer? = nil

    var body: some View {
        VStack(spacing: 0) {
            // ── Header (matches web: 48px, bolt icon, brand, status) ──
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

    // MARK: - Transfer Card (matches web: centered, bordered, glow)

    private var transferCard: some View {
        VStack(spacing: 0) {
            // Encryption badge (matches web: shield + "End-to-End Encrypted")
            HStack(spacing: 6) {
                Image(systemName: ipc.sessionPhase == .connected ? "lock.shield.fill" : "lock.shield")
                    .font(.system(size: 13))
                    .foregroundColor(neon.opacity(0.7))
                Text("End-to-End Encrypted")
                    .font(.system(size: 13))
                    .foregroundColor(neon.opacity(0.7))
            }
            .padding(.vertical, 14)

            Rectangle().fill(.white.opacity(0.06)).frame(height: 1)

            // Card content: priority order matches web
            // connected > incoming request (sheet) > connecting > device list > button
            if ipc.sessionPhase == .connected, let peer = ipc.connectedPeer {
                sessionContent(peer: peer)
                    .onAppear { cancelConnection() } // clear connecting state
            } else if let target = connectingTo {
                connectingContent(target: target)
            } else {
                discoveryContent
            }

            // Session ended
            if case .disconnected(let reason) = ipc.sessionPhase {
                Rectangle().fill(.white.opacity(0.06)).frame(height: 1)
                HStack {
                    Text("Disconnected: \(reason)")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.3))
                    Spacer()
                    Button("Dismiss") { ipc.resetSession() }
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

    // MARK: - Discovery (matches web: "Devices" button → list)

    private var discoveryContent: some View {
        VStack(spacing: 0) {
            if !showDeviceList {
                // Default: "Devices" button (matches web exactly)
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

                // Peer code (subtle, below devices — matches web "Legacy Peer" area)
                Text(signaling.peerCode)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundColor(.white.opacity(0.2))
                    .padding(.bottom, 12)
            } else {
                // Device list (matches web popup)
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
                VStack(alignment: .leading, spacing: 1) {
                    Text(peer.deviceName)
                        .font(.system(size: 13))
                        .foregroundColor(.white.opacity(0.8))
                    Text(peer.peerCode)
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundColor(.white.opacity(0.25))
                }
                Spacer()
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .background(.white.opacity(0.001)) // hit target
        .disabled(!ipc.isConnected)
    }

    // MARK: - Connecting (matches web: awaiting approval)

    private func connectingContent(target: DiscoveredPeer) -> some View {
        VStack(spacing: 12) {
            // Pulsing indicator (matches web: nested circles)
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

            // Status text (matches web phases)
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
                    Text("Still connecting to **\(target.deviceName)**...")
                        .font(.system(size: 13))
                        .foregroundColor(.white.opacity(0.8))
                    Text("This is taking longer than usual")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.3))
                }
            }
            .multilineTextAlignment(.center)

            // Cancel button (matches web: subtle text button)
            Button("Cancel") {
                cancelConnection()
            }
            .buttonStyle(.plain)
            .font(.system(size: 12))
            .foregroundColor(.white.opacity(0.3))
            .padding(.bottom, 8)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
    }

    // MARK: - Session

    private func sessionContent(peer: PeerSession) -> some View {
        VStack(spacing: 0) {
            // Trust state (inline, matches web verification flow)
            trustRow(peer: peer)

            // Transfer area (gated by trust — matches web policy)
            if peer.trust == .verified || peer.trust == .legacy {
                Rectangle().fill(.white.opacity(0.06)).frame(height: 1)
                transferArea
            }

            // Disconnect
            Rectangle().fill(.white.opacity(0.06)).frame(height: 1)
            HStack {
                HStack(spacing: 6) {
                    Image(systemName: "link")
                        .font(.system(size: 11))
                        .foregroundColor(neon.opacity(0.5))
                    Text(peer.deviceName)
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.4))
                }
                Spacer()
                Button("Disconnect") { ipc.disconnectSession() }
                    .buttonStyle(.plain)
                    .font(.system(size: 11))
                    .foregroundColor(.red.opacity(0.6))
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
    }

    private func trustRow(peer: PeerSession) -> some View {
        HStack(spacing: 8) {
            switch peer.trust {
            case .verified:
                Image(systemName: "checkmark.shield.fill")
                    .foregroundColor(neon)
                Text("Verified")
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundColor(neon)
            case .unverified(let sas):
                Image(systemName: "exclamationmark.shield")
                    .foregroundColor(.yellow)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Verify this code matches:")
                        .font(.system(size: 11))
                        .foregroundColor(.white.opacity(0.4))
                    Text(sas)
                        .font(.system(size: 18, weight: .bold, design: .monospaced))
                        .foregroundColor(.yellow)
                        .tracking(3)
                }
                Spacer()
                Button("Verify") { ipc.markVerified() }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .tint(neon)
            case .legacy:
                Image(systemName: "shield.slash")
                    .foregroundColor(.white.opacity(0.3))
                Text("Legacy Peer")
                    .font(.system(size: 11))
                    .foregroundColor(.white.opacity(0.3))
            }
            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    // MARK: - Transfer Area (matches web: drop zone + progress)

    private var transferArea: some View {
        Group {
            switch ipc.transferPhase {
            case .idle:
                // Drop zone (matches web: dashed border, "Drop files here")
                VStack(spacing: 12) {
                    Image(systemName: "square.and.arrow.up")
                        .font(.system(size: 24))
                        .foregroundColor(.white.opacity(isDragOver ? 0.6 : 0.3))
                    Text("Drop files here")
                        .font(.system(size: 14, weight: .medium))
                        .foregroundColor(.white.opacity(0.7))
                    Text("or click to select")
                        .font(.system(size: 12))
                        .foregroundColor(.white.opacity(0.3))
                    Button("Select Files") { pickAndSendFile() }
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

            case .sending(let fileName, _, let progress):
                transferProgress(icon: "arrow.up.circle", label: "Sending", fileName: fileName, progress: progress, color: neon)

            case .receiving(let fileName, _, let progress):
                transferProgress(icon: "arrow.down.circle", label: "Receiving", fileName: fileName, progress: progress, color: .blue)

            case .complete(let fileName, let savePath):
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
                    Button("Done") { ipc.clearTransfer() }
                        .buttonStyle(.bordered)
                        .controlSize(.mini)
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
                    Button("Dismiss") { ipc.clearTransfer() }
                        .buttonStyle(.bordered)
                        .controlSize(.mini)
                }
                .padding(16)
            }
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
        }
        .padding(16)
    }

    // MARK: - Connection Management

    func initiateConnection(to peer: DiscoveredPeer) {
        withAnimation(.easeInOut(duration: 0.15)) {
            connectingTo = peer
            connectingPhase = .requesting
            showDeviceList = false
        }
        // Send connection_request with wsUrl (matches web protocol)
        let wsPort = daemon.wsPort
        let wsUrl = "ws://\(localIPAddress()):\(wsPort)"
        let deviceName = Host.current().localizedName ?? "Mac"
        let payload = "{\"deviceName\":\"\(deviceName)\",\"deviceType\":\"desktop\",\"wsUrl\":\"\(wsUrl)\"}"
        signaling.sendSignal(toPeerCode: peer.peerCode, signalType: "connection_request", dataJson: payload)

        // Phase transitions: requesting (0s) → establishing (3s) → slow (8s)
        connectingTimer?.invalidate()
        connectingTimer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: false) { [self] _ in
            if connectingTo != nil && ipc.sessionPhase != .connected {
                withAnimation { connectingPhase = .establishing }
                connectingTimer = Timer.scheduledTimer(withTimeInterval: 5.0, repeats: false) { _ in
                    if connectingTo != nil && ipc.sessionPhase != .connected {
                        withAnimation { connectingPhase = .slow }
                    }
                }
            }
        }
    }

    func cancelConnection() {
        connectingTimer?.invalidate()
        connectingTimer = nil
        withAnimation(.easeInOut(duration: 0.15)) {
            connectingTo = nil
            connectingPhase = .requesting
        }
    }

    // MARK: - Helpers

    func pickAndSendFile() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        let ipcRef = ipc
        let daemonRef = daemon
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            ipcRef.transferPhase = .sending(fileName: url.lastPathComponent, transferId: "pending", progress: 0)
            daemonRef.sendFile(path: url.path)
        }
    }

    func handleDrop(_ providers: [NSItemProvider]) -> Bool {
        guard let provider = providers.first else { return false }
        provider.loadItem(forTypeIdentifier: UTType.fileURL.identifier, options: nil) { item, _ in
            guard let data = item as? Data, let url = URL(dataRepresentation: data, relativeTo: nil) else { return }
            DispatchQueue.main.async {
                ipc.transferPhase = .sending(fileName: url.lastPathComponent, transferId: "pending", progress: 0)
                daemon.sendFile(path: url.path)
            }
        }
        return true
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
