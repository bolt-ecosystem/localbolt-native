import SwiftUI

@main
struct LocalBoltApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 420, height: 600)
    }
}

struct ContentView: View {
    @State private var peerCode: String = "..."
    @State private var dataDir: String = "..."
    @State private var signalHealthy: Bool = false

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
                    .fill(signalHealthy ? Color.green : Color.red)
                    .frame(width: 8, height: 8)
                Text(signalHealthy ? "ONLINE" : "OFFLINE")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.secondary)
            }
            .padding()
            .background(.ultraThinMaterial)

            Divider()

            // Main content
            VStack(spacing: 24) {
                Spacer()

                // Peer code from Rust bridge
                VStack(spacing: 8) {
                    Text("Your Peer Code")
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundColor(.secondary)
                    Text(peerCode)
                        .font(.system(size: 28, weight: .bold, design: .monospaced))
                        .foregroundColor(.green)
                        .tracking(4)
                }

                // Platform info from Rust bridge
                VStack(alignment: .leading, spacing: 4) {
                    Label {
                        Text(dataDir)
                            .font(.system(size: 10, design: .monospaced))
                    } icon: {
                        Image(systemName: "folder")
                            .foregroundColor(.secondary)
                    }
                }
                .padding()
                .background(.quaternary)
                .cornerRadius(8)

                // Bridge status
                Text("Rust FFI bridge: active")
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundColor(.green.opacity(0.7))

                Spacer()
            }
            .padding()
        }
        .frame(minWidth: 380, minHeight: 500)
        .background(Color(nsColor: .windowBackgroundColor))
        .onAppear {
            // Call into Rust via FFI bridge
            peerCode = BoltBridge.generatePeerCode()
            dataDir = BoltBridge.platformDataDir()
            signalHealthy = BoltBridge.probeSignalHealth()
        }
    }
}
