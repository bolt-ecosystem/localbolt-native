import SwiftUI

@main
struct LocalBoltApp: App {
    @State private var daemon = DaemonManager()
    @State private var signaling: SignalingManager

    init() {
        let code = BoltBridge.generatePeerCode()
        _signaling = State(initialValue: SignalingManager(peerCode: code))
    }

    var body: some Scene {
        WindowGroup {
            ContentView(daemon: daemon, signaling: signaling)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 420, height: 600)
    }
}

struct ContentView: View {
    @Bindable var daemon: DaemonManager
    @Bindable var signaling: SignalingManager

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
                                daemon.stop()
                                signaling.stop()
                            } else {
                                daemon.start()
                                // Start signaling once daemon is up
                                DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                                    signaling.start(
                                        localUrl: "ws://127.0.0.1:3001",
                                        cloudUrl: "wss://bolt-rendezvous.fly.dev"
                                    )
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
                                        // Future: connection flow
                                    }
                                    .buttonStyle(.bordered)
                                    .controlSize(.small)
                                    .disabled(true) // Not wired yet
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
