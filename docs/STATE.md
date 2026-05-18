# State — localbolt-app

> Current project state. Maintained by docs-keeper agent.
> Last refreshed: 2026-05-18 (native reconnect fix + LocalBolt state audit)

---

## Latest Validated Main

- **HEAD:** `7aaf4dc` (`fix(mac): reset stale disconnect state before reconnect`)
- **Date:** 2026-05-18
- **Validation:** `cargo test` in `native/shared`, `swift build -c release` in
  `native/macos`, local x86_64 bundle build, MacBook Pro deploy, GitHub CI, and
  CodeQL all passed.

## Latest Public Release

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

Forward CI gates:
- Main CI now builds/tests `native/shared` against sibling `bolt-core-sdk`.
- Main CI builds the macOS SwiftUI shell after producing the release Rust FFI
  archive expected by `Package.swift`.
- `native/shared` clippy runs as an advisory gate with the FFI raw-pointer lint
  allowed. Existing rustfmt/clippy warning cleanup is deliberately left as a
  separate code-quality task so this CI realignment does not rewrite working
  bridge code.
- Linux/Steam Deck CLI remains covered by `ci-native-linux.yml` and the manual
  Steam Deck package workflow.
- The old Tauri release workflow is no longer tag-triggered; it is retained only
  as an explicit retired-path guard until native release automation is codified.
- Manual Windows CI no longer treats `src-tauri` as a forward gate. Windows
  native shell CI requires a future `native/windows` implementation decision.

## App↔App QUIC Migration

`localbolt-app` has Q2-Q6 metadata, bridge, packaging, and validation plumbing
for APP-TO-APP-QUIC-MIGRATION-1. The macOS SwiftUI shell reads daemon
`quic_info.json` when present and includes `quicAddr` / `quicCertHash` in
`connection_request` and `connection_accepted` payloads. Incoming request
metadata is now used for acceptor-side pinning: before the app sends
`connection_accepted`, it writes the requester's `quicCertHash` to the daemon
allowlist signal.

Q2D1 added the structured native connect bridge. `connectToRemote` now passes
the peer `wsUrl` plus optional `quicAddr` / `quicCertHash` to the Rust bridge,
which writes JSON to `connect_remote.signal`. When bolt-daemon is built with
`transport-quic`, complete QUIC metadata now routes to the QUIC app-session
adapter first; WS remains the fallback when metadata is missing or QUIC connect
fails.

Q4 native packaging now builds the bundled macOS daemon with the bolt-daemon
`native-full` feature, enabling WS + WT + QUIC for the app sidecar while keeping
WS fallback behavior intact.

Q5 validation completed on 2026-05-17: two-device QUIC/BTR smoke passed in both
directions between Mac Studio and MacBook Pro, WS fallback passed through the
legacy WS-only signal path, disconnect propagation cleared active sessions and
zeroized BTR state on both peers, and pairing trust enforcement plus
QUIC-vs-WS throughput comparison are documented in the ecosystem roadmap.

On 2026-05-18, a native session-state bug was fixed after a real-device report:
if the user disconnected a native↔native session and immediately attempted a
browser↔native connection, the daemon could establish while the macOS UI stayed
in the presentation-only disconnected phase. The macOS state machine now clears
that presentation state before a fresh request, rejects incoming/accepted
signals when the canonical transition is illegal, and stashes acceptor peer
state before signaling acceptance. The fixed x86_64 app bundle was built on Mac
Studio, copied to MacBook Pro, verified there, and the bundled daemon sidecar
was registered as allowed in the MacBook firewall.

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
| NATIVE-RECONNECT-STATE-1 (disconnect → fresh connect state reset) | DONE | `7aaf4dc` |
| MACBOOK-FIREWALL-DEPLOY-1 (build on Studio, deploy/register sidecar on MacBook) | DONE | `a9deac0` |
| APP-TO-APP-QUIC-MIGRATION-1 Q6 (QUIC docs graduation) | DONE | state update |
| APP-TO-APP-QUIC-MIGRATION-1 Q4 (native-full daemon packaging) | DONE | `5ea6293` |
| CI-DEPLOY-INHERITANCE-REALIGN-1 (native CI gates + retired Tauri guards) | DONE | this state update |
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

- **Public macOS distribution:** Current public DMGs are still ad-hoc signed and
  not notarized. First launch may require right-click -> Open, and firewall
  trust can be reset by ad-hoc rebuilds. The repo-local MacBook deploy script
  registers the bundled daemon sidecar for development smoke tests, but the
  production fix is Developer ID signing + notarization.
- **Release freshness:** GitHub Releases still points at the older public
  `LocalBolt-2.0.0` DMGs. Current validated main needs fresh release artifacts
  before public download links represent the QUIC-native build.
