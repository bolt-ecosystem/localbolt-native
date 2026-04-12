# LocalBolt Native

Native macOS desktop app for [LocalBolt](https://localbolt.app). Encrypted peer-to-peer file transfer.

## Download

**[LocalBolt 2.0.0 for macOS →](https://github.com/the9ines/localbolt-native/releases/tag/localbolt-app-v2.0.0)**

| | |
|---|---|
| **Platform** | macOS 14 (Sonoma) or later |
| **Architecture** | Apple Silicon (M1 / M2 / M3 / M4) |
| **Format** | DMG — drag to Applications |
| **Signing** | Ad-hoc (not notarized) |

> **First launch:** Right-click the app, click Open, then click Open again in the dialog.
> This only needs to be done once — macOS remembers the exception.

Checksum verification: see [SHA256SUMS.txt](https://github.com/the9ines/localbolt-native/releases/download/localbolt-app-v2.0.0/SHA256SUMS.txt) on the release page.

## Architecture

LocalBolt Native is a **SwiftUI** macOS app with a **Rust** cryptographic core and a bundled **bolt-daemon** sidecar.

| Layer | Technology | Role |
|-------|-----------|------|
| UI shell | SwiftUI | macOS-native drag-and-drop, transfer progress, device discovery |
| FFI bridge | Rust → C-ABI static library (`libbolt_native_bridge.a`) | Cryptographic operations, protocol logic |
| Transport daemon | [bolt-daemon](https://github.com/the9ines/bolt-daemon) sidecar | WebSocket (default) + WebTransport (browser↔native) |
| Encryption | Bolt Transfer Ratchet (BTR) | Per-transfer DH ratchet + ChaCha20-Poly1305 chunk encryption |
| Identity | Profile Envelope v1 | NaCl-box outer encryption with Ed25519 identity keys |

Browser users at [localbolt.app](https://localbolt.app) connect seamlessly with native app users — same protocol, same encryption.

## Structure

```
localbolt-native/
├── native/
│   ├── shared/    # Rust C-ABI FFI bridge (libbolt_native_bridge.a)
│   └── macos/     # SwiftUI app, build scripts, DMG packaging
├── signal/        # Rust signal server crate (vendored bolt-rendezvous)
└── docs/          # State, changelog, distribution plan
```

## Ecosystem

LocalBolt Native is part of the [Bolt Protocol](https://github.com/the9ines/bolt-protocol) ecosystem.

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
- **[localbolt.app/download/macos](https://localbolt.app/download/macos)** — direct DMG download
- **[LocalBolt (self-hosted)](https://github.com/the9ines/localbolt)** — download and run on your own network

## Historical Note

This repository previously contained a Tauri v2 cross-platform implementation (last Tauri release: v1.2.24). That implementation is frozen. The current native app is SwiftUI + Rust, macOS-only. The `web/` and `src-tauri/` directories in this repo are archived Tauri-era code and are not part of the current build.

## License

MIT — built by [the9ines](https://the9ines.com)
