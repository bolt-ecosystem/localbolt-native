# LocalBolt macOS — SwiftUI Shell

Native macOS shell for LocalBolt, consuming `bolt-app-core` via the
`bolt-native-bridge` FFI crate.

## Architecture

```
SwiftUI Views  →  Swift Bridge Layer  →  bolt-native-bridge (C ABI)  →  bolt-app-core (Rust)
```

## Prerequisites

- Xcode 16+
- Rust toolchain (`rustup`)
- macOS 14+ target

## Build

1. Build the Rust bridge:
   ```
   cd ../shared
   cargo build --release
   ```

2. Open `LocalBolt.xcodeproj` in Xcode (not yet created — scaffold only)

## Status

**Scaffold only.** The Xcode project and SwiftUI views have not been created yet.
This directory establishes the location and build relationship.

## Next Steps

1. Create Xcode project with bridging header pointing to `../shared/include/bolt_native_bridge.h`
2. Link `libbolt_native_bridge.a` from `../shared/target/release/`
3. Build minimal SwiftUI app that calls `bolt_generate_peer_code()` to verify FFI works
4. Implement connection flow consuming bolt-app-core via bridge
