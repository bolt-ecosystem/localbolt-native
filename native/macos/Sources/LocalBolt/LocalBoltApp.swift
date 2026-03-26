import SwiftUI

@main
struct LocalBoltApp: App {
    @State private var daemon = DaemonManager()

    var body: some Scene {
        WindowGroup {
            ContentView(daemon: daemon)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 420, height: 600)
    }
}

struct ContentView: View {
    @Bindable var daemon: DaemonManager
    @State private var peerCode: String = "..."

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
                    .fill(daemon.isRunning ? Color.green : Color.red)
                    .frame(width: 8, height: 8)
                Text(daemon.isRunning ? "DAEMON RUNNING" : "DAEMON STOPPED")
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
                        Text(peerCode)
                            .font(.system(size: 28, weight: .bold, design: .monospaced))
                            .foregroundColor(.green)
                            .tracking(4)
                    }
                    .padding(.top, 20)

                    // Daemon controls
                    VStack(spacing: 12) {
                        if let binPath = daemon.daemonBinaryPath {
                            Label {
                                Text(binPath)
                                    .font(.system(size: 9, design: .monospaced))
                                    .lineLimit(1)
                                    .truncationMode(.middle)
                            } icon: {
                                Image(systemName: "terminal")
                                    .foregroundColor(.secondary)
                            }
                        } else {
                            Label("bolt-daemon not found", systemImage: "exclamationmark.triangle")
                                .foregroundColor(.red)
                                .font(.system(size: 11))
                        }

                        HStack(spacing: 12) {
                            Button(action: {
                                daemon.start()
                            }) {
                                Label("Start Daemon", systemImage: "play.fill")
                            }
                            .disabled(daemon.isRunning || daemon.daemonBinaryPath == nil)

                            Button(action: {
                                daemon.stop()
                            }) {
                                Label("Stop", systemImage: "stop.fill")
                            }
                            .disabled(!daemon.isRunning)
                            .tint(.red)
                        }
                        .buttonStyle(.bordered)

                        if daemon.isRunning {
                            HStack(spacing: 16) {
                                Label("PID \(daemon.pid)", systemImage: "number")
                                Label("WS :\(daemon.wsPort)", systemImage: "network")
                            }
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundColor(.green.opacity(0.8))
                        }
                    }
                    .padding()
                    .background(.quaternary)
                    .cornerRadius(8)

                    // Daemon stderr log
                    if !daemon.recentStderr.isEmpty {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Daemon Log")
                                .font(.system(size: 10, weight: .bold, design: .monospaced))
                                .foregroundColor(.secondary)
                            Text(daemon.recentStderr)
                                .font(.system(size: 9, design: .monospaced))
                                .foregroundColor(.green.opacity(0.7))
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .textSelection(.enabled)
                        }
                        .padding(8)
                        .background(Color.black.opacity(0.3))
                        .cornerRadius(6)
                    }

                    // Bridge status
                    Text("Rust FFI bridge: active")
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundColor(.green.opacity(0.5))
                }
                .padding()
            }
        }
        .frame(minWidth: 380, minHeight: 500)
        .background(Color(nsColor: .windowBackgroundColor))
        .onAppear {
            peerCode = BoltBridge.generatePeerCode()
        }
    }
}
