# LocalBolt App

Native desktop app for [LocalBolt](https://localbolt.site). Encrypted peer-to-peer file transfer with an embedded signaling server.

Built with [Tauri v2](https://tauri.app). No browser needed. Open the app and start transferring.

## Features

- **Embedded signal server** - starts automatically with the app, no setup required
- **Dual signaling** - discovers devices on your LAN and across the internet simultaneously
- **NaCl/Curve25519 encryption** - same crypto as Signal and WireGuard
- **WebRTC P2P transfer** - files go directly between devices, never stored on any server
- **Cross-discovery** - finds devices running the website, the desktop app, or the self-hosted version
- **Works offline** - LAN discovery works with no internet connection

## Quick Start

```bash
cd web && npm install && npx vite build
cd ../src-tauri && cargo tauri dev
```

## Build for Production

```bash
npx tauri build
```

Produces platform-specific installers: `.app`/`.dmg` (macOS), `.msi` (Windows), `.deb`/`.AppImage` (Linux).

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

## Related

- **[localbolt.site](https://localbolt.site)** - use it in the browser, no install
- **[LocalBolt (self-hosted)](https://github.com/the9ines/localbolt)** - download and run on your own network

## License

MIT - built by [the9ines](https://the9ines.com)
