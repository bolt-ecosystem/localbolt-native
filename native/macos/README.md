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

## How the app finds bolt-daemon

`bolt_daemon_find_binary()` (in `../shared/src/daemon.rs`) checks these in order and
uses the first that exists. When none exist it logs every path it checked under
`[bolt-daemon-lookup]` rather than failing silently.

1. **`BOLT_DAEMON_PATH`** — explicit override. Point it at any local build:
   ```
   BOLT_DAEMON_PATH=/path/to/bolt-daemon ./.build/release/LocalBolt
   ```
2. **Bundle sidecar** — `LocalBolt.app/Contents/MacOS/bolt-daemon`, installed by
   `build-app.sh`. This is the only path a shipped install depends on.
3. **Sibling `bolt-daemon` checkout** — found by walking up from the executable, so
   a plain `swift build` run inside an ecosystem checkout resolves
   `<ecosystem>/bolt-daemon/target/release/bolt-daemon` (then `target/debug`). This is
   relative to the executable and encodes no absolute home directory.

A bare `swift build -c release` produces only the executable, with no sidecar — it
relies on 1 or 3. Use `bash build-app.sh` to get a bundle with the daemon inside.

## Status

**Scaffold only.** The Xcode project and SwiftUI views have not been created yet.
This directory establishes the location and build relationship.

## Next Steps

1. Create Xcode project with bridging header pointing to `../shared/include/bolt_native_bridge.h`
2. Link `libbolt_native_bridge.a` from `../shared/target/release/`
3. Build minimal SwiftUI app that calls `bolt_generate_peer_code()` to verify FFI works
4. Implement connection flow consuming bolt-app-core via bridge
