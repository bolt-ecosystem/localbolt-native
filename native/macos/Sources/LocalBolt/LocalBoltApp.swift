import AppKit
import SwiftUI

@main
struct LocalBoltApp: App {
    @State private var daemon = DaemonManager()
    @State private var signaling: SignalingManager
    @State private var ipc = IpcManager()

    init() {
        let code = BoltBridge.generatePeerCode()
        _signaling = State(initialValue: SignalingManager(peerCode: code))
    }

    var body: some Scene {
        WindowGroup {
            ContentView(daemon: daemon, signaling: signaling, ipc: ipc)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 420, height: 600)
    }
}

struct ContentView: View {
    @Bindable var daemon: DaemonManager
    @Bindable var signaling: SignalingManager
    @Bindable var ipc: IpcManager

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Image(systemName: "bolt.fill")
                    .foregroundColor(.green)
                Text("LocalBolt")
                    .font(.system(.headline, design: .monospaced))
                    .fontWeight(.bold)
                Spacer()
                Circle()
                    .fill(signaling.isConnected ? Color.green : Color.red)
                    .frame(width: 8, height: 8)
                Text(signaling.isConnected ? "ONLINE" : "OFFLINE")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.secondary)
            }
            .padding()
            .background(.ultraThinMaterial)

            Divider()

            ScrollView {
                VStack(spacing: 20) {
                    // Peer code
                    VStack(spacing: 8) {
                        Text("Your Peer Code")
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundColor(.secondary)
                        Text(signaling.peerCode)
                            .font(.system(size: 28, weight: .bold, design: .monospaced))
                            .foregroundColor(.green)
                            .tracking(4)
                    }
                    .padding(.top, 16)

                    // Daemon controls
                    HStack(spacing: 12) {
                        Button(action: {
                            if daemon.isRunning {
                                ipc.stop()
                                signaling.stop()
                                daemon.stop()
                            } else {
                                daemon.start()
                                // Start signaling + IPC once daemon is up
                                DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                                    signaling.start(
                                        localUrl: "ws://127.0.0.1:3001",
                                        cloudUrl: "wss://bolt-rendezvous.fly.dev"
                                    )
                                }
                                // Connect IPC after daemon socket is ready
                                DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                                    ipc.start(socketPath: daemon.socketPath)
                                }
                            }
                        }) {
                            Label(
                                daemon.isRunning ? "Stop" : "Start",
                                systemImage: daemon.isRunning ? "stop.fill" : "play.fill"
                            )
                        }
                        .disabled(daemon.daemonBinaryPath == nil)
                        .tint(daemon.isRunning ? .red : .green)
                        .buttonStyle(.bordered)

                        if daemon.isRunning {
                            HStack(spacing: 8) {
                                Text("PID \(daemon.pid)")
                                Text("WS :\(daemon.wsPort)")
                                if ipc.isConnected {
                                    Text("IPC")
                                        .foregroundColor(.green)
                                }
                            }
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundColor(.secondary)
                        }
                    }

                    // Peer list
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("Nearby Devices")
                                .font(.system(size: 12, weight: .semibold, design: .monospaced))
                                .foregroundColor(.secondary)
                            Spacer()
                            Text("\(signaling.peers.count)")
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundColor(.green)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(.green.opacity(0.15))
                                .cornerRadius(4)
                        }

                        if signaling.peers.isEmpty {
                            HStack {
                                if signaling.isConnected {
                                    ProgressView()
                                        .scaleEffect(0.6)
                                    Text("Searching for devices...")
                                        .font(.system(size: 11))
                                        .foregroundColor(.secondary)
                                } else if !daemon.isRunning {
                                    Image(systemName: "bolt.slash")
                                        .foregroundColor(.secondary)
                                    Text("Start daemon to discover devices")
                                        .font(.system(size: 11))
                                        .foregroundColor(.secondary)
                                } else {
                                    Image(systemName: "wifi.slash")
                                        .foregroundColor(.secondary)
                                    Text("Connecting to signaling...")
                                        .font(.system(size: 11))
                                        .foregroundColor(.secondary)
                                }
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.vertical, 8)
                        } else {
                            ForEach(signaling.peers) { peer in
                                HStack {
                                    Image(systemName: deviceIcon(peer.deviceType))
                                        .foregroundColor(.green.opacity(0.7))
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(peer.deviceName)
                                            .font(.system(size: 13))
                                        Text(peer.peerCode)
                                            .font(.system(size: 10, design: .monospaced))
                                            .foregroundColor(.secondary)
                                    }
                                    Spacer()
                                    Button("Connect") {
                                        signaling.sendSignal(
                                            toPeerCode: peer.peerCode,
                                            signalType: "connect-request"
                                        )
                                    }
                                    .buttonStyle(.bordered)
                                    .controlSize(.small)
                                    .disabled(!ipc.isConnected)
                                }
                                .padding(.vertical, 4)
                            }
                        }
                    }
                    .padding()
                    .background(.quaternary)
                    .cornerRadius(8)

                    // Active session
                    if ipc.sessionPhase == .connected, let peer = ipc.connectedPeer {
                        VStack(spacing: 12) {
                            HStack {
                                Image(systemName: "link")
                                    .foregroundColor(.green)
                                Text("Connected to \(peer.deviceName)")
                                    .font(.system(size: 13, weight: .semibold))
                                Spacer()
                                Button("Send File") {
                                    pickAndSendFile()
                                }
                                .buttonStyle(.borderedProminent)
                                .controlSize(.small)
                                .tint(.green)
                                .disabled(ipc.transferPhase != .idle)
                                Button("Disconnect") {
                                    ipc.disconnectSession()
                                }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                                .tint(.red)
                            }

                            // Trust state
                            HStack(spacing: 8) {
                                switch peer.trust {
                                case .verified:
                                    Image(systemName: "checkmark.shield.fill")
                                        .foregroundColor(.green)
                                    Text("Verified")
                                        .font(.system(size: 12, weight: .semibold, design: .monospaced))
                                        .foregroundColor(.green)
                                case .unverified(let sas):
                                    Image(systemName: "exclamationmark.shield")
                                        .foregroundColor(.yellow)
                                    VStack(alignment: .leading, spacing: 4) {
                                        Text("Unverified — confirm code matches:")
                                            .font(.system(size: 11))
                                            .foregroundColor(.secondary)
                                        Text(sas)
                                            .font(.system(size: 20, weight: .bold, design: .monospaced))
                                            .foregroundColor(.yellow)
                                            .tracking(4)
                                    }
                                    Spacer()
                                    Button("Verify") {
                                        ipc.markVerified()
                                    }
                                    .buttonStyle(.borderedProminent)
                                    .controlSize(.small)
                                    .tint(.green)
                                case .legacy:
                                    Image(systemName: "shield.slash")
                                        .foregroundColor(.secondary)
                                    Text("Legacy peer (no identity)")
                                        .font(.system(size: 11))
                                        .foregroundColor(.secondary)
                                }
                            }

                            // Transfer status
                            Group {
                            switch ipc.transferPhase {
                            case .sending(let fileName, _):
                                HStack(spacing: 8) {
                                    ProgressView()
                                        .scaleEffect(0.6)
                                    Text("Sending \(fileName)...")
                                        .font(.system(size: 11))
                                        .foregroundColor(.secondary)
                                }
                            case .receiving(let fileName, _):
                                HStack(spacing: 8) {
                                    ProgressView()
                                        .scaleEffect(0.6)
                                    Text("Receiving \(fileName)...")
                                        .font(.system(size: 11))
                                        .foregroundColor(.secondary)
                                }
                            case .complete(let fileName, let savePath):
                                HStack(spacing: 8) {
                                    Image(systemName: "checkmark.circle.fill")
                                        .foregroundColor(.green)
                                    Text("\(fileName) complete")
                                        .font(.system(size: 11))
                                    Spacer()
                                    if let path = savePath {
                                        Button("Reveal") {
                                            NSWorkspace.shared.selectFile(path, inFileViewerRootedAtPath: "")
                                        }
                                        .buttonStyle(.bordered)
                                        .controlSize(.mini)
                                    }
                                    Button("Dismiss") {
                                        ipc.clearTransfer()
                                    }
                                    .buttonStyle(.bordered)
                                    .controlSize(.mini)
                                }
                            case .failed(let fileName, let reason):
                                HStack(spacing: 8) {
                                    Image(systemName: "xmark.circle.fill")
                                        .foregroundColor(.red)
                                    Text("\(fileName) failed: \(reason)")
                                        .font(.system(size: 11))
                                        .foregroundColor(.secondary)
                                    Spacer()
                                    Button("Dismiss") {
                                        ipc.clearTransfer()
                                    }
                                    .buttonStyle(.bordered)
                                    .controlSize(.mini)
                                }
                            case .idle:
                                EmptyView()
                            }
                            } // Group
                        }
                        .padding()
                        .background(.green.opacity(0.06))
                        .cornerRadius(8)
                        .overlay(
                            RoundedRectangle(cornerRadius: 8)
                                .stroke(.green.opacity(0.2), lineWidth: 1)
                        )
                    }

                    // Session ended notice
                    if case .disconnected(let reason) = ipc.sessionPhase {
                        HStack(spacing: 8) {
                            Image(systemName: "xmark.circle")
                                .foregroundColor(.secondary)
                            Text("Disconnected: \(reason)")
                                .font(.system(size: 11))
                                .foregroundColor(.secondary)
                            Spacer()
                            Button("Dismiss") {
                                ipc.resetSession()
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.mini)
                        }
                        .padding(10)
                        .background(.quaternary)
                        .cornerRadius(6)
                    }

                    // Daemon log (collapsed by default)
                    if daemon.isRunning && !daemon.recentStderr.isEmpty {
                        DisclosureGroup("Daemon Log") {
                            Text(daemon.recentStderr)
                                .font(.system(size: 9, design: .monospaced))
                                .foregroundColor(.green.opacity(0.6))
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .textSelection(.enabled)
                        }
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundColor(.secondary)
                        .padding(8)
                        .background(Color.black.opacity(0.2))
                        .cornerRadius(6)
                    }
                }
                .padding()
            }
        }
        .frame(minWidth: 380, minHeight: 500)
        .background(Color(nsColor: .windowBackgroundColor))
        // Incoming pairing request sheet
        .sheet(item: $ipc.pendingRequest) { request in
            PairingRequestView(request: request, ipc: ipc)
        }
    }

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
            let path = url.path
            ipcRef.transferPhase = .sending(fileName: url.lastPathComponent, transferId: "pending")
            daemonRef.sendFile(path: path)
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

/// Sheet shown when a remote peer requests to connect.
struct PairingRequestView: View {
    let request: PairingRequest
    let ipc: IpcManager

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "person.crop.circle.badge.questionmark")
                .font(.system(size: 40))
                .foregroundColor(.green)

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
                        .foregroundColor(.green)
                        .tracking(4)
                }
                .padding()
                .background(.green.opacity(0.08))
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
                .tint(.green)
            }
        }
        .padding(30)
        .frame(width: 320)
    }
}
