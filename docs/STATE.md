# State — localbolt-app

> Current project state. Maintained by docs-keeper agent.
> Last refreshed: 2026-04-07 (GOVERNANCE-CODIFICATION-1)

---

## Latest Release

- **Tag (pushed):** localbolt-app-v1.2.26-br5-wasm-init
- **HEAD:** e0437c9 (3 commits ahead of last pushed tag — governance reconciliation)
- **Date:** 2026-04-07
- **Tests:** 82 Rust (native/shared) + contract parity tests

## Forward Path

- **Product:** macOS SwiftUI shell consuming bolt-app-core via C-ABI FFI
- **Location:** `native/macos/` (SwiftUI) + `native/shared/` (Rust FFI bridge)
- **Retired:** `src-tauri/` — Tauri implementation reduced to thin glue, not receiving forward investment
- **Superseded:** bolt-ui (egui) — historical desktop shell in bolt-core-sdk

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
- **native/macos:** macOS SwiftUI shell — forward native product path. Full transfer vertical: discovery, connect, pair, verify (with SAS reject), send/receive with progress, .app bundle with daemon sidecar. Safety controls M1-M3 implemented. NATIVE-SHELL-1, NATIVE-SHELL-UX-1, NATIVE-UX-SAFETY-CONTROLS-1 (partial) CLOSED.
- **bolt-ui (egui):** Historical desktop shell in bolt-core-sdk. Superseded by native shells for forward development.

## Recent Streams (since v1.2.26)

| Stream | Status | Commits |
|--------|--------|---------|
| NATIVE-SHELL-1 closure (Tauri retirement) | DONE | `14094ed` |
| SIGNALING-FFI-CRASH-FIX-1 | DONE | `219aedd` |
| NATIVE-UX-SAFETY-CONTROLS-1 (M1-M3) | DONE | `e0437c9` |

## Known Open Issues

- **RECONNECT-INTEGRITY-1** — Trust state leakage across sessions. SAS verification asymmetric on reconnect. Safety-critical.
- **NATIVE-UX-PARITY-IMPL-2** — 4 remaining MUST-MATCH items (M4-M7).
