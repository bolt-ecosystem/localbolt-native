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
                                    let socketPath = "/tmp/bolt-native-\(daemon.pid).sock"
                                    ipc.start(socketPath: socketPath)
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
                                        // NS1-P2: outbound connection initiation
                                    }
                                    .buttonStyle(.bordered)
                                    .controlSize(.small)
                                    .disabled(true) // Deferred to NS1-P2
                                }
                                .padding(.vertical, 4)
                            }
                        }
                    }
                    .padding()
                    .background(.quaternary)
                    .cornerRadius(8)

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
