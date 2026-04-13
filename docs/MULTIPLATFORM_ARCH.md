# LocalBolt Native — Multi-Platform Architecture

> Codified: 2026-04-12 (LOCALBOLT-APP-MULTIPLATFORM-ARCH-1)
> Governance: GOVERNANCE-NATIVE-SHELL-ALIGNMENT-1 (`37a40bf`)

---

## Canonical Architecture

LocalBolt Native uses platform-native shells over shared Rust core/daemon authority.
The current macOS app is a SwiftUI shell with a Rust FFI bridge and bundled bolt-daemon
sidecar. Tauri and egui/bolt-ui are retired historical paths, not the forward product
architecture.

**Anti-pattern:** Do not introduce a cross-platform UI shell as the default product
path when platform-native shells are the architectural target.

---

## Repository Layout

```
localbolt-app/
├── native/
│   ├── shared/             # Rust C FFI bridge (for non-Rust shells: Swift, Kotlin)
│   ├── macos/              # SwiftUI shell (ACTIVE — forward macOS product)
│   ├── linux/
│   │   ├── cli/            # Phase 1: Rust CLI helper (Steam Deck validation)
│   │   └── gtk/            # Phase 2: GTK4/libadwaita shell (future, TBD)
│   ├── windows/            # Future: platform-native shell (TBD)
│   └── ios/                # Future: SwiftUI iOS shell
├── src-tauri/              # FROZEN — Tauri v2 historical code
├── web/                    # FROZEN — Tauri-era web frontend
├── signal/                 # Vendored rendezvous subtree
├── scripts/
├── docs/
└── .github/workflows/
```

---

## Shell Architecture

```
                    bolt-core-sdk
    ┌─────────────────────┴─────────────────────┐
    │           bolt-app-core                    │
    │  (IPC, daemon lifecycle, platform paths)   │
    └───────────┬───────────────────┬────────────┘
                │                   │
        ┌───────┴──────┐    ┌──────┴───────────┐
        │  C FFI       │    │ Direct Rust      │
        │  Bridge      │    │ consumption      │
        │ (native/     │    │                  │
        │  shared/)    │    │                  │
        └───────┬──────┘    └──────┬───────────┘
                │                  │
        ┌───────┴──────┐    ┌──────┴───────────┐
        │ Non-Rust     │    │ Rust shells      │
        │ shells       │    │                  │
        ├──────────────┤    ├──────────────────┤
        │ SwiftUI mac  │    │ localbolt-cli    │
        │ Kotlin droid │    │ GTK4 (future)    │
        └──────────────┘    └──────────────────┘
```

**Rule:** Non-Rust shells (Swift, Kotlin) consume the C FFI bridge in `native/shared/`.
Rust-native shells (CLI, GTK4-rs) depend on `bolt-app-core` directly — no FFI overhead.

---

## Platform Status

| Platform | Shell | Location | Status |
|----------|-------|----------|--------|
| macOS (arm64 + x86_64) | SwiftUI + Rust FFI | `native/macos/` | **Production** (v2.0.0) |
| Linux x86_64 / Steam Deck | CLI helper | `native/linux/cli/` | **Phase 1** |
| Linux x86_64 (GUI) | GTK4/libadwaita (TBD) | `native/linux/gtk/` | Future |
| Windows | TBD | `native/windows/` | Future |
| iOS | SwiftUI | `native/ios/` | Future |
| Android | Kotlin/Compose | N/A | Future |

---

## Build Paths

| Platform | Entry | Output | Distribution |
|----------|-------|--------|-------------|
| macOS arm64 | `native/macos/build-app.sh` | DMG | GitHub Releases (localbolt-native) |
| macOS x86_64 | `native/macos/build-app.sh` | DMG | GitHub Releases (localbolt-native) |
| Linux x86_64 (CLI) | `scripts/build-linux-cli.sh` | Tarball | GitHub Releases (localbolt-native) |
| Linux x86_64 (GUI) | TBD | Flatpak / AppImage | TBD |
| Windows | TBD | TBD | TBD |

---

## Ownership Boundaries

| Component | Owns | Does NOT own |
|-----------|------|-------------|
| **localbolt-app** | Platform shells, FFI bridge, build/packaging | Protocol, daemon runtime, crypto |
| **bolt-daemon** | Daemon binary, transports, IPC contract | UI, packaging, distribution |
| **bolt-core-sdk** | Protocol/crypto crates, bolt-app-core | Platform shells, daemon binary |
| **localbolt-v3** | Web product, download pages | Native apps |
| **localbolt-native** (GitHub) | Release artifact hosting | Source code |
| **bolt-ecosystem** | Governance, architecture rules | Implementation |

---

## Constraints

- No Tauri revival
- No egui/bolt-ui as default product shell
- No new repo for Linux CLI (lives in localbolt-app)
- Platform-native shells are the forward direction
- Linux/Windows GUI shells require governance decision before starting
- `localbolt-native` is interim release host until GitHub releases `localbolt-app` name
