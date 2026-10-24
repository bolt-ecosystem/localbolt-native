import Foundation
import SystemConfiguration

/// Computer name advertised to peers.
///
/// `Host.localizedName` is NSHost, which on some Macs returns the wildcard bind
/// address `0.0.0.0` instead of the name in System Settings. Peers then list the
/// machine as "0.0.0.0". Prefer the System Configuration computer name, and never
/// advertise a bind address.
enum LocalDeviceName {
    static func current() -> String {
        for candidate in candidates() {
            let preferred = BoltBridge.preferDeviceName(current: "", candidate: candidate)
            if !preferred.isEmpty && !BoltBridge.deviceNameIsUnusable(preferred) {
                return preferred
            }
        }
        return "Mac"
    }

    private static func candidates() -> [String] {
        var names: [String] = []
        if let computer = SCDynamicStoreCopyComputerName(nil, nil) as String? {
            names.append(computer)
        }
        if let localized = Host.current().localizedName {
            names.append(localized)
        }
        var host = ProcessInfo.processInfo.hostName
        if host.hasSuffix(".local") {
            host.removeLast(".local".count)
        }
        names.append(host)
        return names
    }
}
