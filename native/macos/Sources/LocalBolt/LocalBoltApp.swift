import AppKit
import SwiftUI

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
                .onAppear { autoStart() }
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

    /// Auto-start daemon + signaling + IPC on app launch.
    private func autoStart() {
        guard daemon.daemonBinaryPath != nil, !daemon.isRunning else { return }
        daemon.start()
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            signaling.start(
                localUrl: "ws://127.0.0.1:3001",
                cloudUrl: "wss://bolt-rendezvous.fly.dev"
            )
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
            ipc.start(socketPath: daemon.socketPath)
        }
    }
}

// MARK: - Main Content View

struct ContentView: View {
    @Bindable var daemon: DaemonManager
    @Bindable var signaling: SignalingManager
    @Bindable var ipc: IpcManager
    @Binding var showDiagnostics: Bool

    var body: some View {
        VStack(spacing: 0) {
            // ── Header ──────────────────────────────────────
            HStack {
                Image(systemName: "bolt.fill")
                    .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))
                Text("LocalBolt")
                    .font(.system(.headline, design: .monospaced))
                    .fontWeight(.bold)
                Spacer()
                Circle()
                    .fill(signaling.isConnected ? Color(red: 0.64, green: 0.88, blue: 0) : Color.red)
                    .frame(width: 8, height: 8)
                Text(signaling.isConnected ? "ONLINE" : "OFFLINE")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.secondary)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .background(.ultraThinMaterial)

            Divider()

            // ── Main transfer surface ────────────────────────
            ScrollView {
                VStack(spacing: 16) {
                    Spacer().frame(height: 12)

                    // Startup state
                    if !daemon.isRunning {
                        startupView
                    } else {
                        // Peer code
                        peerCodeView

                        // Transfer card (matches web product layout)
                        transferCard
                    }

                    Spacer()
                }
                .padding(.horizontal, 16)
            }
        }
        .frame(minWidth: 380, minHeight: 500)
        .background(Color(nsColor: .windowBackgroundColor))
        .sheet(item: $ipc.pendingRequest) { request in
            PairingRequestView(request: request, ipc: ipc)
        }
        .sheet(isPresented: $showDiagnostics) {
            DiagnosticsView(daemon: daemon, ipc: ipc, signaling: signaling)
        }
    }

    // MARK: - Startup

    private var startupView: some View {
        VStack(spacing: 16) {
            Spacer().frame(height: 40)
            if daemon.daemonBinaryPath == nil {
                Image(systemName: "exclamationmark.triangle")
                    .font(.system(size: 32))
                    .foregroundColor(.yellow)
                Text("Daemon not found")
                    .font(.system(size: 14, weight: .semibold))
                Text("bolt-daemon binary is missing from the app bundle.")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            } else {
                ProgressView()
                    .scaleEffect(0.8)
                Text("Starting...")
                    .font(.system(size: 12))
                    .foregroundColor(.secondary)
            }
        }
        .frame(maxWidth: .infinity)
    }

    // MARK: - Peer Code

    private var peerCodeView: some View {
        VStack(spacing: 6) {
            Text(signaling.peerCode)
                .font(.system(size: 32, weight: .bold, design: .monospaced))
                .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))
                .tracking(4)
            Text("Your Peer Code")
                .font(.system(size: 11, design: .monospaced))
                .foregroundColor(.secondary)
        }
    }

    // MARK: - Transfer Card

    private var transferCard: some View {
        VStack(spacing: 0) {
            // Encryption badge
            HStack(spacing: 6) {
                Image(systemName: "lock.shield")
                    .font(.system(size: 11))
                    .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0).opacity(0.7))
                Text("End-to-End Encrypted")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0).opacity(0.7))
            }
            .padding(.vertical, 10)

            Divider().opacity(0.3)

            // Active session or peer discovery
            if ipc.sessionPhase == .connected, let peer = ipc.connectedPeer {
                sessionView(peer: peer)
            } else {
                peerDiscoveryView
            }

            // Session ended
            if case .disconnected(let reason) = ipc.sessionPhase {
                Divider().opacity(0.3)
                HStack(spacing: 6) {
                    Text("Disconnected: \(reason)")
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                    Spacer()
                    Button("Dismiss") { ipc.resetSession() }
                        .buttonStyle(.plain)
                        .font(.system(size: 11))
                        .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
            }
        }
        .background(Color.black.opacity(0.15))
        .cornerRadius(10)
        .overlay(
            RoundedRectangle(cornerRadius: 10)
                .stroke(Color(red: 0.64, green: 0.88, blue: 0).opacity(0.15), lineWidth: 1)
        )
    }

    // MARK: - Peer Discovery

    private var peerDiscoveryView: some View {
        VStack(spacing: 0) {
            if signaling.peers.isEmpty {
                VStack(spacing: 8) {
                    if signaling.isConnected {
                        ProgressView()
                            .scaleEffect(0.6)
                        Text("Searching for devices...")
                            .font(.system(size: 11))
                            .foregroundColor(.secondary)
                    } else {
                        Image(systemName: "wifi.slash")
                            .foregroundColor(.secondary)
                        Text("Connecting...")
                            .font(.system(size: 11))
                            .foregroundColor(.secondary)
                    }
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 20)
            } else {
                ForEach(signaling.peers) { peer in
                    if peer.id != signaling.peers.first?.id {
                        Divider().opacity(0.2)
                    }
                    peerRow(peer)
                }
            }
        }
    }

    private func peerRow(_ peer: DiscoveredPeer) -> some View {
        HStack(spacing: 10) {
            Image(systemName: deviceIcon(peer.deviceType))
                .font(.system(size: 16))
                .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0).opacity(0.6))
                .frame(width: 24)
            VStack(alignment: .leading, spacing: 1) {
                Text(peer.deviceName)
                    .font(.system(size: 13))
                Text(peer.peerCode)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.secondary)
            }
            Spacer()
            Button("Connect") {
                signaling.sendSignal(toPeerCode: peer.peerCode, signalType: "connect-request")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            .tint(Color(red: 0.64, green: 0.88, blue: 0))
            .disabled(!ipc.isConnected)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    // MARK: - Active Session

    private func sessionView(peer: PeerSession) -> some View {
        VStack(spacing: 0) {
            // Connected peer header
            HStack {
                Image(systemName: "link")
                    .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))
                Text(peer.deviceName)
                    .font(.system(size: 13, weight: .semibold))
                Spacer()
                Button("Disconnect") { ipc.disconnectSession() }
                    .buttonStyle(.plain)
                    .font(.system(size: 11))
                    .foregroundColor(.red.opacity(0.8))
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)

            Divider().opacity(0.2)

            // Trust state
            trustView(peer: peer)

            // Transfer UI (only when transfer is allowed)
            if peer.trust == .verified || peer.trust == .legacy {
                Divider().opacity(0.2)
                transferActionView
            }
        }
    }

    private func trustView(peer: PeerSession) -> some View {
        HStack(spacing: 8) {
            switch peer.trust {
            case .verified:
                Image(systemName: "checkmark.shield.fill")
                    .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))
                Text("Verified")
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))
            case .unverified(let sas):
                Image(systemName: "exclamationmark.shield")
                    .foregroundColor(.yellow)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Verify this code matches:")
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                    Text(sas)
                        .font(.system(size: 20, weight: .bold, design: .monospaced))
                        .foregroundColor(.yellow)
                        .tracking(4)
                }
                Spacer()
                Button("Verify") { ipc.markVerified() }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.small)
                    .tint(Color(red: 0.64, green: 0.88, blue: 0))
            case .legacy:
                Image(systemName: "shield.slash")
                    .foregroundColor(.secondary)
                Text("Legacy Peer")
                    .font(.system(size: 11))
                    .foregroundColor(.secondary)
            }
            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    // MARK: - Transfer Action

    private var transferActionView: some View {
        Group {
            switch ipc.transferPhase {
            case .idle:
                Button(action: { pickAndSendFile() }) {
                    HStack {
                        Image(systemName: "square.and.arrow.up")
                        Text("Send File")
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .tint(Color(red: 0.64, green: 0.88, blue: 0))
                .controlSize(.large)
                .padding(.horizontal, 12)
                .padding(.vertical, 10)

            case .sending(let fileName, _, let progress):
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Image(systemName: "arrow.up.circle")
                            .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))
                        Text("Sending \(fileName)")
                            .font(.system(size: 11))
                            .foregroundColor(.secondary)
                        Spacer()
                        Text("\(Int(progress * 100))%")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))
                    }
                    ProgressView(value: Double(progress))
                        .tint(Color(red: 0.64, green: 0.88, blue: 0))
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)

            case .receiving(let fileName, _, let progress):
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Image(systemName: "arrow.down.circle")
                            .foregroundColor(.blue)
                        Text("Receiving \(fileName)")
                            .font(.system(size: 11))
                            .foregroundColor(.secondary)
                        Spacer()
                        Text("\(Int(progress * 100))%")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundColor(.blue)
                    }
                    ProgressView(value: Double(progress))
                        .tint(.blue)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)

            case .complete(let fileName, let savePath):
                HStack(spacing: 8) {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))
                    Text("\(fileName)")
                        .font(.system(size: 11))
                    Spacer()
                    if let path = savePath {
                        Button("Reveal") {
                            NSWorkspace.shared.selectFile(path, inFileViewerRootedAtPath: "")
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.mini)
                    }
                    Button("Done") { ipc.clearTransfer() }
                        .buttonStyle(.bordered)
                        .controlSize(.mini)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)

            case .failed(let fileName, let reason):
                HStack(spacing: 8) {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundColor(.red)
                    Text("\(fileName): \(reason)")
                        .font(.system(size: 11))
                        .foregroundColor(.secondary)
                    Spacer()
                    Button("Dismiss") { ipc.clearTransfer() }
                        .buttonStyle(.bordered)
                        .controlSize(.mini)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
            }
        }
    }

    // MARK: - Helpers

    func pickAndSendFile() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.title = "Select a file to send"
        let ipcRef = ipc
        let daemonRef = daemon
        panel.begin { response in
            guard response == .OK, let url = panel.url else { return }
            ipcRef.transferPhase = .sending(fileName: url.lastPathComponent, transferId: "pending", progress: 0)
            daemonRef.sendFile(path: url.path)
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
                .font(.system(size: 40))
                .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))

            Text("Connection Request")
                .font(.system(.title2, design: .monospaced))
                .fontWeight(.bold)

            VStack(spacing: 4) {
                Text(request.deviceName)
                    .font(.system(size: 16, weight: .semibold))
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
                        .font(.system(size: 24, weight: .bold, design: .monospaced))
                        .foregroundColor(Color(red: 0.64, green: 0.88, blue: 0))
                        .tracking(4)
                }
                .padding()
                .background(Color(red: 0.64, green: 0.88, blue: 0).opacity(0.08))
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
                .tint(Color(red: 0.64, green: 0.88, blue: 0))
            }
        }
        .padding(30)
        .frame(width: 320)
    }
}

// MARK: - Diagnostics Sheet (Cmd+Option+D)

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
