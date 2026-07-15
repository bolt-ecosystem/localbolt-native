// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "LocalBolt",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "LocalBolt", targets: ["LocalBolt"]),
    ],
    targets: [
        // System library target wrapping the Rust FFI bridge
        .systemLibrary(
            name: "CBoltBridge",
            path: "Sources/CBoltBridge"
        ),
        // Main SwiftUI app target
        .executableTarget(
            name: "LocalBolt",
            dependencies: ["CBoltBridge"],
            path: "Sources/LocalBolt",
            linkerSettings: [
                // Link the STATIC archive directly — not -l which prefers dylib.
                // This eliminates the runtime dependency on dev-path dylib.
                .unsafeFlags([
                    "../../native/shared/target/release/libbolt_native_bridge.a",
                ]),
                // System libs required by the Rust static lib
                .linkedLibrary("System"),
                .linkedLibrary("resolv"),
                .linkedFramework("Security"),
                .linkedFramework("CoreFoundation"),
            ]
        ),
        // Unit tests for pure/model logic (e.g. the TOFU PinStore). `@testable import`
        // pulls in the whole app module, which references the Rust FFI, so the test
        // bundle re-declares the same link settings as the app target.
        .testTarget(
            name: "LocalBoltTests",
            dependencies: ["LocalBolt"],
            path: "Tests/LocalBoltTests",
            linkerSettings: [
                .unsafeFlags([
                    "../../native/shared/target/release/libbolt_native_bridge.a",
                ]),
                .linkedLibrary("System"),
                .linkedLibrary("resolv"),
                .linkedFramework("Security"),
                .linkedFramework("CoreFoundation"),
            ]
        ),
    ]
)
