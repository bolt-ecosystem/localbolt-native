# State — localbolt-app

> Current project state. Maintained by docs-keeper agent.
> Last refreshed: 2026-05-15 (APP-TO-APP-QUIC-MIGRATION-1 Q2D1)

---

## Latest Release

- **Tag (pushed):** localbolt-app-v2.0.1-multiarch-build
- **HEAD:** 7482aa7
- **Date:** 2026-04-12
- **Tests:** 82 Rust (native/shared) + contract parity tests

## Distribution (LIVE)

- **Release:** [localbolt-app-v2.0.0](https://github.com/the9ines/localbolt-native/releases/tag/localbolt-app-v2.0.0)
- **Artifacts:** `LocalBolt-2.0.0-arm64.dmg` (Apple Silicon) + `LocalBolt-2.0.0-x86_64.dmg` (Intel)
- **Checksums:** SHA256SUMS.txt on release page
- **Artifact host:** GitHub Releases (`the9ines/localbolt-native`)
- **Download links:** `localbolt.app/download/macos`, `/download/macos/apple-silicon`, `/download/macos/intel`
- **Signing:** Ad-hoc (not notarized). First launch requires right-click → Open.
- **Requirements:** macOS 14 (Sonoma)+
- **Auto-update:** Not implemented
- **Repo note:** `localbolt-native` is interim name. Rename to `localbolt-app` pending GitHub support (name retired).

## Forward Path

- **Product:** Platform-native shells over shared Rust core/daemon authority
- **macOS:** SwiftUI shell consuming bolt-app-core via C-ABI FFI (`native/macos/` + `native/shared/`)
- **Linux Phase 1:** Rust CLI helper for Steam Deck validation (`native/linux/cli/`)
- **Linux Phase 2:** GTK4/libadwaita shell (TBD, requires governance decision)
- **Windows:** TBD platform-native shell
- **iOS:** SwiftUI shell under `native/ios/` (future)
- **Android:** Kotlin/Compose shell under `native/android/` (future)
- **Retired:** `src-tauri/` — Tauri implementation frozen, not receiving forward investment
- **Retired:** bolt-ui (egui) — historical desktop shell in bolt-core-sdk, not a fallback
- **Architecture:** `docs/MULTIPLATFORM_ARCH.md` (LOCALBOLT-APP-MULTIPLATFORM-ARCH-1)

## Product Role

`localbolt-app` is the canonical home for native and mobile LocalBolt shells.
`localbolt-v3` owns the production web app. `localbolt` owns the lightweight
self-hosted web app. Native/mobile platform work should land under
`localbolt-app/native/` unless governance explicitly approves a separate repo.

## CI / Shared-Code Discipline

The three app repos share core ecosystem behavior through `bolt-app-core`,
`bolt-daemon`, `bolt-rendezvous`, and the LocalBolt session/transfer contracts.
Changes to signaling metadata, app-to-app transport behavior, session states, or
security gates must update the shared authority first, then update affected app
repos with tests or explicit non-impact evidence.

## App↔App QUIC Migration

`localbolt-app` has Q2B/Q2D1 metadata plumbing for
APP-TO-APP-QUIC-MIGRATION-1. The macOS SwiftUI shell reads daemon
`quic_info.json` when present and includes `quicAddr` / `quicCertHash` in
`connection_request` and `connection_accepted` payloads. Incoming request
metadata is preserved for future acceptor-side pinning.

Q2D1 added the structured native connect bridge. `connectToRemote` now passes
the peer `wsUrl` plus optional `quicAddr` / `quicCertHash` to the Rust bridge,
which writes JSON to `connect_remote.signal`. This does not make QUIC the
active app↔app path: bolt-daemon currently parses the structured signal and
falls back to WS until the QUIC app-session accept/routing bridge is wired.

Validation for Q2D1:
- `swift build` in `native/macos`
- `cargo test` in `native/shared`
- `cargo build --release` in `native/shared` for the static bridge archive

## Follow-ups (not blocking initial release)

| Item | Priority | Blocker |
|------|----------|---------|
| Apple Developer ID signing + notarization | HIGH | $99/yr Apple Developer Program enrollment |
| Auto-update mechanism | MEDIUM | Needs at least one shipped release (done) + signing |
| Repo rename `localbolt-native` → `localbolt-app` | LOW | GitHub support response (name retirement) |

## Dependencies (native/shared/Cargo.toml)

| Crate | Source |
|-------|--------|
| bolt-app-core | path (bolt-core-sdk workspace) |
| libc | crates.io |
| serde_json | crates.io |

## Dependencies (web/package.json — historical, Tauri path)

| Package | Version | Registry | Note |
|---------|---------|----------|------|
| @tauri-apps/api | ^2.0.0 | npmjs.org | Tauri path (retired) |
| @the9ines/bolt-core | 0.5.2 | npmjs.org | |
| @the9ines/bolt-transport-web | 0.6.8 | npmjs.org | |
| @the9ines/localbolt-core | 0.1.2 | npmjs.org | |
| tweetnacl | ^1.0.3 | | |
| tweetnacl-util | ^0.15.1 | | |

## C6 Guards — DONE

| Script | Purpose |
|--------|---------|
| scripts/check-core-version-pin.sh | Verify localbolt-core version pin |
| scripts/check-core-single-install.sh | Verify single install in tree |
| scripts/check-core-drift.sh | Detect declared vs resolved drift |
| scripts/upgrade-localbolt-core.sh | Upgrade localbolt-core (check + upgrade modes) |

## Q4 Coverage — DONE-VERIFIED

- **Thresholds:** 90/90/80/90 (lines/functions/branches/statements)
- **Baseline:** 100% on tested files
- **CI:** `test:coverage` wired in `.github/workflows/ci.yml`
- **Dev dep:** `@vitest/coverage-v8`

## Dev Dependencies

| Package | Version |
|---------|---------|
| @types/node | ^22.5.5 |
| autoprefixer | ^10.4.20 |
| postcss | ^8.4.47 |
| tailwindcss | ^3.4.11 |
| tailwindcss-animate | ^1.0.7 |
| typescript | ^5.5.3 |
| vite | ^7.3.1 |

## Branch

- **Branch:** main
- **Status:** Active
- **Build:** Passing (swift build + cargo build --release + codesign)
- **Tests:** 82+ Rust (native/shared, including 5 FFI crash-fix tests + contract parity)
- **src-tauri:** **Retired.** Tauri desktop path superseded by native SwiftUI shell. Thin glue remains, not receiving forward investment.
- **native/shared:** Rust C-ABI FFI bridge to bolt-app-core. Produces libbolt_native_bridge.a (static). Always rebuilt by build-app.sh (ABI mismatch prevention).
- **native/macos:** macOS SwiftUI shell — forward native product path. Full transfer vertical: discovery, connect, pair, verify (with SAS reject), send/receive with progress, .app bundle with daemon sidecar. Safety controls M1-M3 implemented. UX parity M4-M7 implemented (file queue, multi-file, cancel, TOFU mismatch alert). All 7 MUST-MATCH items complete. NATIVE-SHELL-1, NATIVE-SHELL-UX-1, NATIVE-UX-SAFETY-CONTROLS-1 (partial), NATIVE-UX-PARITY-IMPL-2 CLOSED.
- **bolt-ui (egui):** Historical desktop shell in bolt-core-sdk. Superseded by native shells for forward development.

## TOFU Pin Persistence (v1.2.27)

- **PinStore** class in `BoltBridge.swift` — persists verified identity keys to `<dataDir>/pins/identity_pins.json`
- JSON format, atomic writes, ISO 8601 date encoding, sorted keys
- Pin lifecycle: new identity pinned as unverified on first SAS encounter, promoted to verified on user confirmation
- On reconnect: known verified identities skip SAS automatically (PROTOCOL.md §2)
- `IpcManager.start()` accepts `dataDir` to initialize pin store; `PeerSession.identityKeyB64` carries remote identity key
- Log tokens: `[TOFU] identity pinned as verified`, `[TOFU] known verified identity — SAS skipped`

## Recent Streams (since v1.2.26)

| Stream | Status | Commits |
|--------|--------|---------|
| APP-TO-APP-QUIC-MIGRATION-1 Q2D1 (structured connect signal bridge) | DONE | `7f2a6bd` |
| APP-TO-APP-QUIC-MIGRATION-1 Q2B (metadata signaling) | DONE | prior state update |
| NATIVE-UX-PARITY-IMPL-2 (M4-M7 UX parity) | DONE | `a0f9d91` |
| RECONNECT-INTEGRITY-1 (TOFU pin store) | DONE | `e93a7cc` |
| NATIVE-SHELL-1 closure (Tauri retirement) | DONE | `14094ed` |
| SIGNALING-FFI-CRASH-FIX-1 | DONE | `219aedd` |
| NATIVE-UX-SAFETY-CONTROLS-1 (M1-M3) | DONE | `e0437c9` |

## Linux Phase 1 (LOCALBOLT-LINUX-CLI-IMPL-1)

- **Artifact:** `localbolt-cli` — Rust CLI helper for bolt-daemon
- **Location:** `native/linux/cli/`
- **Target:** Linux x86_64, Steam Deck (SteamOS Desktop Mode)
- **Status:** Scaffolded, builds locally
- **Validates:** daemon-on-Linux, IPC contract, browser-to-native transfer
- **Not:** the final Linux GUI shell (that is Phase 2, TBD)

## Signaling

- **Canonical cloud endpoint:** `wss://bolt-rendezvous.fly.dev`
- **Drift guard:** `scripts/check-signaling-endpoint-drift.sh` — asserts native and web share the same endpoint
- Both native macOS and web (via `VITE_SIGNAL_URL` env var) MUST point to the same rendezvous server

## Known Open Issues

- **Native↔native LAN transfers:** May fail if macOS firewall blocks daemon WS port (3001). Requires manual firewall allow for bolt-daemon. Separate from the browser↔native fix.
