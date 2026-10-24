import CBoltBridge
import Foundation

struct AdvertisedEndpoint: Equatable {
    let url: String
    let certHash: String
}

/// Reads daemon `wt_info.json` / `quic_info.json`. Empty and missing files are nil
/// so they compare equal to a "no baseline" snapshot.
enum TransportMetadataFile {
    static func read(dataDir: String, name: String) -> String? {
        guard !dataDir.isEmpty else { return nil }
        let url = URL(fileURLWithPath: dataDir).appendingPathComponent(name)
        guard let data = try? Data(contentsOf: url),
              let text = String(data: data, encoding: .utf8) else { return nil }
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

extension BoltBridge {
    /// Live WebTransport URL + cert hash, or nil when the on-disk record is stale.
    static func selectWt(
        currentJSON: String?,
        baselineJSON: String?,
        wsPort: UInt16,
        host: String
    ) -> AdvertisedEndpoint? {
        withOptionalCString(currentJSON) { current in
            withOptionalCString(baselineJSON) { baseline in
                host.withCString { hostPtr in
                    guard let ptr = bolt_select_wt_advertisement(current, baseline, wsPort, hostPtr) else {
                        return nil
                    }
                    defer { bolt_free_string(ptr) }
                    return parseAdvertisement(String(cString: ptr))
                }
            }
        }
    }

    /// Live QUIC `host:port` + cert hash. Nil leaves the native path on WebSocket.
    static func selectQuic(
        currentJSON: String?,
        baselineJSON: String?,
        wsPort: UInt16,
        host: String
    ) -> AdvertisedEndpoint? {
        withOptionalCString(currentJSON) { current in
            withOptionalCString(baselineJSON) { baseline in
                host.withCString { hostPtr in
                    guard let ptr = bolt_select_quic_advertisement(current, baseline, wsPort, hostPtr) else {
                        return nil
                    }
                    defer { bolt_free_string(ptr) }
                    return parseAdvertisement(String(cString: ptr))
                }
            }
        }
    }

    /// Name shown in the device list. Never a bind address.
    static func displayDeviceName(_ raw: String) -> String {
        raw.withCString { ptr in
            guard let out = bolt_device_display_name(ptr) else { return "Mac" }
            defer { bolt_free_string(out) }
            return String(cString: out)
        }
    }

    /// Keep a real name. Replace `0.0.0.0` (and other bind addresses) when a real one exists.
    static func preferDeviceName(current: String, candidate: String) -> String {
        current.withCString { currentPtr in
            candidate.withCString { candidatePtr in
                guard let out = bolt_prefer_device_name(currentPtr, candidatePtr) else { return current }
                defer { bolt_free_string(out) }
                return String(cString: out)
            }
        }
    }

    static func deviceNameIsUnusable(_ name: String) -> Bool {
        name.withCString { bolt_device_name_is_unusable($0) == 1 }
    }

    private static func parseAdvertisement(_ json: String) -> AdvertisedEndpoint? {
        guard let data = json.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let url = obj["url"] as? String, !url.isEmpty,
              let certHash = obj["certHash"] as? String, !certHash.isEmpty
        else { return nil }
        return AdvertisedEndpoint(url: url, certHash: certHash)
    }
}

private func withOptionalCString<T>(_ value: String?, _ body: (UnsafePointer<CChar>?) -> T) -> T {
    guard let value else { return body(nil) }
    return value.withCString { body($0) }
}
