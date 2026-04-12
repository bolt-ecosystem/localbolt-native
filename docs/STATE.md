# State — localbolt-app

> Current project state. Maintained by docs-keeper agent.
> Last refreshed: 2026-04-12 (GOVERNANCE-CODIFICATION-7)

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

- **Product:** macOS SwiftUI shell consuming bolt-app-core via C-ABI FFI
- **Location:** `native/macos/` (SwiftUI) + `native/shared/` (Rust FFI bridge)
- **Retired:** `src-tauri/` — Tauri implementation frozen, not receiving forward investment
- **Superseded:** bolt-ui (egui) — historical desktop shell in bolt-core-sdk

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
| NATIVE-UX-PARITY-IMPL-2 (M4-M7 UX parity) | DONE | `a0f9d91` |
| RECONNECT-INTEGRITY-1 (TOFU pin store) | DONE | `e93a7cc` |
| NATIVE-SHELL-1 closure (Tauri retirement) | DONE | `14094ed` |
| SIGNALING-FFI-CRASH-FIX-1 | DONE | `219aedd` |
| NATIVE-UX-SAFETY-CONTROLS-1 (M1-M3) | DONE | `e0437c9` |

## Known Open Issues

None.
