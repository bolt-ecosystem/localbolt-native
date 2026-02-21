# LocalBolt App

Native desktop app for [LocalBolt](https://localbolt.site). Encrypted peer-to-peer file transfer with an embedded signaling server.

Built with [Tauri v2](https://tauri.app). No browser needed. Open the app and start transferring.

## Download

| Platform | Download |
|----------|----------|
| macOS (Apple Silicon) | [LocalBolt_1.0.0_aarch64.dmg](https://github.com/the9ines/localbolt-app/releases/latest) |
| macOS (Intel) | [LocalBolt_1.0.0_x64.dmg](https://github.com/the9ines/localbolt-app/releases/latest) |
| Windows | [LocalBolt_1.0.0_x64-setup.exe](https://github.com/the9ines/localbolt-app/releases/latest) |
| Linux | [LocalBolt_1.0.0_amd64.AppImage](https://github.com/the9ines/localbolt-app/releases/latest) |

> **macOS:** Right-click the app, click Open, then click Open again to bypass Gatekeeper.
> **Windows:** Click "More info" then "Run anyway" on the SmartScreen prompt.

## Features

- **Embedded signal server** - starts automatically with the app, no setup required
- **Dual signaling** - discovers devices on your LAN and across the internet simultaneously
- **NaCl/Curve25519 encryption** - same crypto as Signal and WireGuard
- **WebRTC P2P transfer** - files go directly between devices, never stored on any server
- **Cross-discovery** - finds devices running the website, the desktop app, or the self-hosted version
- **Works offline** - LAN discovery works with no internet connection

## Build from Source

```bash
cd web && npm install && npx vite build
cd ../src-tauri && cargo tauri dev
```

### Production build

```bash
cd web && npm install && npx vite build
cd ../src-tauri && cargo tauri build
```

Produces platform-specific installers: `.app`/`.dmg` (macOS), `.msi`/`.exe` (Windows), `.deb`/`.AppImage` (Linux).

## Structure

```
localbolt-app/
├── web/           # Frontend (Vanilla TypeScript, Tailwind, Vite)
├── signal/        # Rust signal server crate
├── src-tauri/     # Tauri v2 app shell
│   ├── src/
│   │   ├── lib.rs     # Embedded signal server + Tauri setup
│   │   └── main.rs    # Entry point
│   └── tauri.conf.json
└── README.md
```

## How It Works

When you launch the app:

1. The embedded Rust signal server starts on port 3001
2. The web frontend loads in a native webview
3. `DualSignaling` connects to both `ws://localhost:3001` (local) and `wss://localbolt-signal.fly.dev` (cloud)
4. Devices from both sources appear in the device list
5. Select a device, approve the connection, and transfer files

## Ecosystem

LocalBolt App is part of the [Bolt Protocol](https://github.com/the9ines/bolt-protocol) ecosystem. See the [bolt-ecosystem](https://github.com/the9ines/bolt-ecosystem) repo for governance documents, PRD, and roadmap.

| Relationship | Repository |
|-------------|-----------|
| Ecosystem governance | [bolt-ecosystem](https://github.com/the9ines/bolt-ecosystem) |
| Protocol spec | [bolt-protocol](https://github.com/the9ines/bolt-protocol) |
| SDK dependency | [bolt-core-sdk](https://github.com/the9ines/bolt-core-sdk) |
| Bundles (subtree) | [bolt-rendezvous](https://github.com/the9ines/bolt-rendezvous) |
| Bundles | [bolt-daemon](https://github.com/the9ines/bolt-daemon) |
| Lite web version | [localbolt](https://github.com/the9ines/localbolt) |
| Web app | [localbolt-v3](https://github.com/the9ines/localbolt-v3) |

This is an **open-source** project. Free to use, build, and modify.

## Related

- **[localbolt.site](https://localbolt.site)** — use it in the browser, no install
- **[LocalBolt (self-hosted)](https://github.com/the9ines/localbolt)** — download and run on your own network

## License

MIT — built by [the9ines](https://the9ines.com)
