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
                .unsafeFlags([
                    "-L../../native/shared/target/release",
                    "-lbolt_native_bridge",
                ]),
                // System libs required by the Rust static lib
                .linkedLibrary("System"),
                .linkedFramework("Security"),
            ]
        ),
    ]
)
