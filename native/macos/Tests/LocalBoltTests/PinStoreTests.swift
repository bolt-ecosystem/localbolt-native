import XCTest
@testable import LocalBolt

/// Item-6 (no verified/persistent-pin semantics pre-EA1) tests for the macOS TOFU
/// `PinStore`. These pin the honest, fail-safe behavior: session approval is never
/// persisted as a verified pin, a stored `verified: true` flag is never trusted (so a
/// reconnect can never silently skip the SAS), and key-continuity mismatch detection
/// still works without relying on the verified flag.
final class PinStoreTests: XCTestCase {
    private func tempDataDir() -> String {
        NSTemporaryDirectory() + "localbolt-pinstore-" + UUID().uuidString
    }

    /// markVerified must NOT persist a verified flag, and isVerified must never report
    /// true — even within the same run and after reloading from disk. This is what makes
    /// a reconnect unable to auto-verify from a stored pin.
    func testApprovalIsNotPersistedAndIsVerifiedIsAlwaysFalse() {
        let dir = tempDataDir()
        let key = "AAAABBBBCCCCDDDDEEEEFFFF"

        let store = PinStore(dataDir: dir)
        store.pin(identityKeyB64: key, deviceName: "Mac")
        // Approving the session must not mark the pin verified.
        store.markVerified(identityKeyB64: key, deviceName: "Mac")
        XCTAssertFalse(store.isVerified(identityKeyB64: key),
                       "pre-EA1: a pin must never read back as verified")

        // Reloaded from disk: nothing verified was persisted, so no reconnect skip.
        let reloaded = PinStore(dataDir: dir)
        XCTAssertFalse(reloaded.isVerified(identityKeyB64: key),
                       "pre-EA1: no persisted verified flag may survive to skip SAS on reconnect")
    }

    /// An OLD on-disk pin with verified=true (written before item-6) must be ignored:
    /// the pin still loads (key-continuity intact) but isVerified stays false, so the
    /// reconnect path cannot use it to skip user SAS review.
    func testLegacyVerifiedTruePinIsIgnored() throws {
        let dir = tempDataDir()
        let pinsDir = URL(fileURLWithPath: dir).appendingPathComponent("pins")
        try FileManager.default.createDirectory(at: pinsDir, withIntermediateDirectories: true)
        let key = "LEGACYVERIFIEDKEYAAAABBBB"
        // Legacy record with verified=true. firstSeen is a numeric date to match the
        // decoder used by PinStore.load (default `.deferredToDate`).
        let json = "{\"\(key)\":{\"verified\":true,\"firstSeen\":0,\"deviceName\":\"Mac\"}}"
        try json.data(using: .utf8)!.write(to: pinsDir.appendingPathComponent("identity_pins.json"))

        let store = PinStore(dataDir: dir)
        // The legacy pin loaded: a different key for the same device name flags a mismatch.
        XCTAssertNotNil(store.checkMismatch(identityKeyB64: "DIFFERENTKEYCCCCDDDD", deviceName: "Mac"),
                        "the legacy pin should have loaded (mismatch detection sees it)")
        // Core assertion: the stored verified:true is never surfaced as verified.
        XCTAssertFalse(store.isVerified(identityKeyB64: key),
                       "pre-EA1: a stored verified:true pin must NOT be used to skip SAS")
    }

    /// Mismatch detection keys on ANY previously-pinned device name (key-continuity),
    /// not just verified ones — pins are no longer marked verified.
    func testMismatchDetectedForUnverifiedPin() {
        let dir = tempDataDir()
        let store = PinStore(dataDir: dir)
        store.pin(identityKeyB64: "OLDKEYAAAABBBBCCCC", deviceName: "Mac")

        // Same device name, new identity key → mismatch, even though nothing is verified.
        XCTAssertNotNil(store.checkMismatch(identityKeyB64: "NEWKEYDDDDEEEEFFFF", deviceName: "Mac"),
                        "a device name reappearing with a new key must flag a mismatch")
        // Same key → no mismatch.
        XCTAssertNil(store.checkMismatch(identityKeyB64: "OLDKEYAAAABBBBCCCC", deviceName: "Mac"))
    }
}
