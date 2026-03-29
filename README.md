# LocalBolt App

Native desktop app for [LocalBolt](https://localbolt.app). Encrypted peer-to-peer file transfer.

## Current State

The forward native product path uses **SwiftUI on macOS** consuming `bolt-app-core` (Rust) via C-ABI FFI with a bundled daemon sidecar. Full transfer vertical: discovery, connection, pairing, verification, send/receive with progress, and `.app` bundle packaging.

The Tauri v2 implementation is frozen — the last published Tauri release was v1.2.24.

See `docs/STATE.md` for detailed current state.

## Download (Last Tauri Release)

| Platform | Download |
|----------|----------|
| macOS (Apple Silicon) | [LocalBolt_1.0.0_aarch64.dmg](https://github.com/the9ines/localbolt-app/releases/latest) |
| macOS (Intel) | [LocalBolt_1.0.0_x64.dmg](https://github.com/the9ines/localbolt-app/releases/latest) |
| Windows | [LocalBolt_1.0.0_x64-setup.exe](https://github.com/the9ines/localbolt-app/releases/latest) |
| Linux | [LocalBolt_1.0.0_amd64.AppImage](https://github.com/the9ines/localbolt-app/releases/latest) |

> **macOS:** Right-click the app, click Open, then click Open again to bypass Gatekeeper.
> **Windows:** Click "More info" then "Run anyway" on the SmartScreen prompt.

## Structure

```
localbolt-app/
├── web/           # Frontend (Vanilla TypeScript, Tailwind, Vite) — Tauri era, frozen
├── signal/        # Rust signal server crate (vendored bolt-rendezvous)
├── src-tauri/     # Tauri v2 app shell — frozen, last release v1.2.24
├── native/
│   ├── shared/    # Rust C-ABI FFI bridge (libbolt_native_bridge.a)
│   └── macos/     # SwiftUI shell scaffold (daemon lifecycle, signaling)
└── README.md
```

## Ecosystem

LocalBolt App is part of the [Bolt Protocol](https://github.com/the9ines/bolt-protocol) ecosystem.

| Relationship | Repository |
|-------------|-----------|
| Ecosystem governance | [bolt-ecosystem](https://github.com/the9ines/bolt-ecosystem) |
| Protocol spec | [bolt-protocol](https://github.com/the9ines/bolt-protocol) |
| SDK (Rust) | [bolt-core-sdk](https://github.com/the9ines/bolt-core-sdk) |
| Daemon | [bolt-daemon](https://github.com/the9ines/bolt-daemon) |
| Signal server (subtree) | [bolt-rendezvous](https://github.com/the9ines/bolt-rendezvous) |
| Web app | [localbolt-v3](https://github.com/the9ines/localbolt-v3) |
| Self-hosted | [localbolt](https://github.com/the9ines/localbolt) |

This is an **open-source** project. Free to use, build, and modify.

## Related

- **[localbolt.app](https://localbolt.app)** — use it in the browser, no install
- **[LocalBolt (self-hosted)](https://github.com/the9ines/localbolt)** — download and run on your own network

## License

MIT — built by [the9ines](https://the9ines.com)
