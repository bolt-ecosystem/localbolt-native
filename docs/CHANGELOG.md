# Changelog — localbolt-app

All notable changes to this project are documented here. Newest first.

---

## fix(security): item-6 — honest pre-EA1 verification (no false "Verified", no persisted/reconnect trust) — 2026-07-15

The macOS shell persisted a `verified` pin from the user tapping "I Verified" after eyeballing the
SAS, showed a green "Verified" shield, and auto-asserted `.verified` on reconnect (silently skipping
the SAS). None of that is cryptographic device verification (no EA1/PAKE exists yet), so it overstated
assurance and contradicted the daemon's own stated invariant. Made the shell honest **without**
renaming the internal `.verified` state:
- `PinStore` is now key-continuity only: `markVerified` persists nothing; `isVerified` always returns
  false (a stored `verified: true` — including old on-disk pins — is never trusted, so no reconnect can
  skip the SAS); mismatch detection keys on any pinned device name rather than only "verified" ones.
- Removed the reconnect auto-verify branches in `BoltBridge` (`session-sas`) and `LocalBoltApp`
  (`applyWtVerificationCode`): every session starts unverified and the user re-reviews the SAS.
  `IpcManager.markVerified()` records session-scoped approval only (no persist).
- Wording: "Verified" badge → "Approved" (approval checkmark, not a security shield); "I Verified"
  button → "Approve this session"; SAS prompt "Verify this code matches" → "Compare this code with";
  mismatch alert "last verified" → "last seen".

Approval/authorization and TOFU mismatch handling are unchanged. Added a `LocalBoltTests` target with
`PinStore` tests (approval not persisted; legacy `verified:true` ignored; mismatch on any pin). The app
compiles + links (`swift build`); XCTest execution requires Xcode (dev/CI) and was not runnable in the
Command-Line-Tools-only audit environment. Audit: item-6 (no verified/pin semantics pre-EA1).

---

## fix(security): EA8 (native keydir) — persist identity/trust to the platform data dir — 2026-07-14

The daemon's identity key + TOFU trust store lived in a predictable
`/tmp/bolt-native-<pid>` directory created fresh on every launch, so identity
regenerated each run and TOFU pins never persisted (degrading MITM protection), and a
world-adjacent `/tmp` path is a weak home for key material. The native bridge now points
`--data-dir` at the platform default (`bolt_app_core::platform::default_data_dir()`, e.g.
`~/Library/Application Support/LocalBolt/daemon` on macOS), a stable per-user location.
Because that directory is now persistent, `bolt_daemon_stop` no longer deletes it (which
would have wiped identity + pins on every stop); only the ephemeral IPC socket is
removed. A unit test asserts the data dir is the platform default and never
`/tmp/bolt-native-<pid>` (mutation-verified). The IPC socket stays under `/tmp` (it is
ephemeral, and a socket path in the data dir can exceed the AF_UNIX sun_path limit on
macOS). Bridge tests 23/23. Audit: EA8 (NATIVE-KEYDIR-1), native-app portion; the
`bolt-daemon` identity_store hardening (symlink/uid/O_EXCL) is tracked separately.

---

## fix(security): EA4 (near-term) — launch bolt-daemon with `ask`, not `allow` — 2026-07-14

The native app spawned bolt-daemon with `--pairing-policy allow` on a `0.0.0.0` WS
listener, so any LAN host could write files into `~/Downloads` with no prompt and no
SAS. Now that the daemon trust path fails closed (EA2 legacy closure, EA3 WebTransport
gate, item-2 fail-closed `trust_config`), the app launches with `--pairing-policy ask`,
which denies unpinned inbound by default. Authorization hardening only: this does NOT
add an interactive prompt or any "verified"/pin behavior (that is the full EA4 +
EA1 workstream, still design-locked). The spawn argv is extracted into a testable
`daemon_spawn_args` helper; a unit test asserts the args carry `ask` and never `allow`
(mutation-verified). Bridge tests 22/22. Audit: EA4 (NATIVE-PAIRING-ASK-1), near-term step.

---

## fix(security): EA9 — contain interior-NUL peer metadata at the FFI boundary — 2026-07-14

A malformed or malicious rendezvous peer whose `peer_code`, `device_name`, or
`device_type` carried an interior NUL byte aborted the whole app: the FFI boundary
in `native/shared/src/signaling.rs` called `CString::new(...).unwrap()` on those
untrusted fields (rendezvous is untrusted by design), a zero-interaction remote
DoS. Each field now mirrors the existing `wt_url` handling and maps a NUL-bearing
value to a null pointer (the Swift host already null-coalesces it to ""), so a
malformed peer is skipped instead of crashing. Two adversarial tests cover NUL in
`device_name` and `peer_code`. Audit: EA9 (FFI-PANIC-SAFETY-1).

---

## fix(mac): order-aware WT lifecycle parsing + initiator session handling — 2026-07-03

Recovered May-era working-tree progress (found uncommitted during the
Governance OS sweep, kept per PM direction). BoltBridge now parses the daemon
stderr tail in order and honors only the latest lifecycle token, so stale
"cleared"/"[SAS]" lines from a prior session cannot poison reconnect state;
the SAS is associated with its registered session. LocalBoltApp populates
connectedPeer from pendingInitiatorPeer on initiator-side WT connects and
applies SAS verification through a shared applyWtVerificationCode() helper on
connect and on SAS change.

Validation: `swift build` clean (compile-validated per
`os/rules/validation-protocol.md`); runtime validation tracked in the
App↔Browser manual checklist.

---

## (pending) — 2026-04-13

fix: browser↔native connectivity — stop resetting session on browserless connection_accepted

**Root cause:** When a browser peer accepted a connection, the native app's
`connection_accepted` handler called `ipc.resetSession()` because the browser
sends no `wsUrl` (browsers cannot host a WS server). This killed the session
before the browser could connect back via WebTransport to the native daemon.

**Fix:** The else branch now stays in `.connecting` state and logs a message,
allowing the WT session detection (`onChange(of: daemon.wtSessionActive)`) to
advance the connection to `.connected` when the browser connects inbound.

**New:** `scripts/check-signaling-endpoint-drift.sh` — CI-ready guard that
asserts native and web apps share the same canonical signaling endpoint.

**Files changed:**
- native/macos/Sources/LocalBolt/LocalBoltApp.swift
- scripts/check-signaling-endpoint-drift.sh
- docs/STATE.md
- docs/CHANGELOG.md

---

## localbolt-app-v2.0.1-multiarch-build — 2026-04-12

**Commit:** 7482aa7

feat: architecture-aware build pipeline for multi-arch releases.

Build pipeline hardened for deterministic Apple Silicon and Intel builds.
`build-app.sh` accepts target architecture parameter (`arm64` / `x86_64`),
handles Rust cross-compilation and library staging automatically.
`create-dmg.sh` detects architecture from the built binary via `lipo`.

**Files changed:** native/macos/build-app.sh, native/macos/create-dmg.sh

---

## localbolt-app-v2.0.0-tauri-audit — 2026-04-12

**Commit:** b57f19c

docs: mark PRD and ROADMAP as superseded (Tauri retired).

Add SUPERSEDED headers to PRD.md and ROADMAP.md. Strike through
Tauri-specific items. Part of ecosystem-wide Tauri reference audit.

**Files changed:** PRD.md, ROADMAP.md

---

## localbolt-app-v2.0.0-docs — 2026-04-12

**Commit:** 64060b3

docs: rewrite README for current SwiftUI/Rust native architecture.

Remove stale Tauri-era download table. Replace with v2.0.0 download
link, architecture table, Gatekeeper instructions, and historical note.

**Files changed:** README.md

---

## localbolt-app-v2.0.0 — 2026-04-11

**Commit:** 24f50f9

First published release of the native SwiftUI macOS desktop app.

- SwiftUI UI with drag-and-drop, real-time progress, transfer controls
- Rust FFI bridge (bolt-native-bridge static library)
- bolt-daemon sidecar (WS default + WebTransport)
- BTR-secured transfers (per-transfer DH ratchet + ChaCha20-Poly1305)
- Profile Envelope v1 (NaCl-box outer encryption)
- Apple Silicon + Intel DMGs
- Ad-hoc signed, macOS 14+, Gatekeeper bypass on first launch

**Release:** https://github.com/the9ines/localbolt-native/releases/tag/localbolt-app-v2.0.0

---

## localbolt-app-v1.2.30-wt-session-state — 2026-04-11

**Commit:** 24f50f9

fix: detect WT session state from daemon stderr for native UI.

---

## localbolt-app-v1.2.28-ux-parity-m4-m7 — 2026-04-07

**Commit:** a0f9d91

feat: UX parity M4-M7 — file queue, cancel, TOFU mismatch alert.

Implements the remaining four MUST-MATCH items (M4-M7) from the
NATIVE-UX-PARITY-IMPL-2 workstream, bringing the macOS native shell to
full UX parity with the web client.

**M4 — Explicit transfer initiation:** File selection (NSOpenPanel) and
drag-and-drop now add files to a queue instead of auto-sending. A "Send N
Files" button triggers transfer explicitly, giving users a chance to review
before committing.

**M5 — Multi-file support:** NSOpenPanel `allowsMultipleSelection` enabled.
Queue UI shows all selected files with individual remove (x) buttons.
Duplicate detection prevents the same URL from being queued twice.
Sequential send: after each transfer completes, the next file in the queue
is sent automatically via `sendNextOrClear()`.

**M6 — Cancel transfer:** A "Cancel Transfer" button appears during active
send/receive progress. Cancellation clears the file queue and disconnects
the session (`disconnectSession(reason: "transfer cancelled")`).

**M7 — TOFU identity mismatch alert:** `PinStore` now tracks `deviceName`
alongside identity keys. On SAS negotiation, `checkMismatch()` detects if
a device name was previously verified with a different identity key. When a
mismatch is found, the UI shows a red security warning with the old key
prefix and a Disconnect button. The `.mismatch` trust state blocks transfers
via `isTransferAllowed()`.

Workstream: NATIVE-UX-PARITY-IMPL-2.

**Files changed:**
- native/macos/Sources/LocalBolt/BoltBridge.swift
- native/macos/Sources/LocalBolt/LocalBoltApp.swift

---

## localbolt-app-v1.2.27-tofu-pin-store — 2026-04-07

**Commit:** e93a7cc

fix: TOFU pin store for native reconnect identity persistence.

Adds persistent TOFU (Trust On First Use) pin store to the macOS native shell.
Previously, SAS verification state was lost across reconnects — the acceptor
saw SAS while the initiator auto-skipped, creating an asymmetric trust UX.
The new `PinStore` class persists verified peer identity keys to
`<dataDir>/pins/identity_pins.json` (JSON, atomic writes, ISO 8601 dates).

On `daemon://session-sas`, the IPC manager checks the pin store: if the
remote identity key is already verified, SAS is skipped (PROTOCOL.md §2).
New identities are pinned as unverified on first encounter, then promoted
to verified when the user confirms the SAS match via `markVerified()`.

`PeerSession` now carries `identityKeyB64` (optional), populated from
`remote_peer_id` in the `session.connected` payload. `IpcManager.start()`
accepts an optional `dataDir` parameter to initialize the pin store.

Workstream: RECONNECT-INTEGRITY-1.

**Files changed:**
- native/macos/Sources/LocalBolt/BoltBridge.swift
- native/macos/Sources/LocalBolt/LocalBoltApp.swift

---

## localbolt-app-v1.2.24-consumer-btr1-p3 — 2026-03-12

**Commit:** ff33747

CBTR-3: Enable Bolt Transfer Ratchet (BTR) in localbolt-app.

**Changes:**
- Bump `@the9ines/bolt-core` 0.5.1 → 0.5.2 (BTR negotiation exports)
- Bump `@the9ines/bolt-transport-web` 0.6.7 → 0.6.8 (BTR wire integration + CBTR-F1 fix)
- Enable `btrEnabled: true` in WebRTCService options (peer-connection.ts)
- Rollback: single-line change `btrEnabled: false`

**Tests:** 74 web (6 files) + 82 Rust (10 modules) = 156 total
- 10 new CBTR-3 tests: dependency gate, config verification, rollback, compatibility, Tauri isolation
- All existing tests pass (no regression)
- Vite build green, cargo check green

**Acceptance criteria:**
- [x] AC-CBTR-14: SDK dependency updated to BTR-4-capable version
- [x] AC-CBTR-15: btrEnabled: true in WebRTCService configuration
- [x] AC-CBTR-16: BTR↔BTR transfer succeeds (config-level proof)
- [x] AC-CBTR-17: BTR↔non-BTR downgrade-with-warning (SDK handles internally)
- [x] AC-CBTR-18: Kill switch rollback verified
- [x] AC-CBTR-19: All existing tests pass
- [x] AC-CBTR-20: Tauri native transport path unaffected

**Files changed:**
- web/package.json
- web/package-lock.json
- web/src/components/peer-connection.ts
- web/src/components/__tests__/cbtr3-btr-compatibility.test.ts (new)
- docs/STATE.md
- docs/CHANGELOG.md

---

## localbolt-app-v1.2.23-recon-xfer1-phase-b — 2026-03-09

**Commit:** 84a4749

RECON-XFER-1 Phase B verification — no code changes required.

Consumer audit confirmed localbolt-app is already protected against the reconnect-resend
bug (RECON-XFER-1) via both web-layer and Tauri/Rust-layer defenses.

**Evidence (no-change proof):**

Web layer:
- Same `@the9ines/localbolt-core` generation guards as localbolt (shared SDK)
- Phase guards (`beginRequest`, `receiveRequest`, `markConnected`) enforce state preconditions
- 21 security-session-integrity tests + 10 TOFU integration tests covering stale callback rejection

Tauri/Rust layer:
- IPC bridge: Writer guarded by Mutex; old reader thread guaranteed dead before new one spawns
- `send_decision()` fails safely if bridge disconnected — no stale message delivery
- Watchdog 5-state machine (Starting→Ready→Restarting→Degraded→Incompatible); no stale state survives daemon restart
- Multi-layer transfer gate: daemon readiness + verification state + connection state — all three required
- No Tauri command caches service/session refs across reconnect

Build/test results:
- Web build: Vite production build green (WASM bundle present)
- Web tests: 5 files, 64 tests — all pass
- Rust tests: 82 tests — all pass (146 total)
- Type check: not separately configured (Vite handles)

**AC evidence:**
- AC-RX-07 (remaining consumers): localbolt-app verified — no patch needed
- AC-RX-08 (WASM/fallback): WASM policy adapter is orthogonal to reconnect path (transfer scheduling only, not session lifecycle). Build output includes WASM bundle. Forced-fallback uses same session lifecycle. Manual runtime confirmation deferred (not automatable without live peers).

**Files changed:**
- docs/CHANGELOG.md
- docs/STATE.md

---

## localbolt-app-v1.2.22-domain-rename — 2026-03-08

**Commit:** beb8891

Rename localbolt.site references to localbolt.app.

**Files changed:**
- README.md

---

## localbolt-app-v1.2.21-csp-wasm — 2026-03-08

**Commit:** 83a8350

Allow WASM compilation in CSP for policy adapter.

**Files changed:**
- web/index.html

---

## localbolt-app-v1.2.14-n8-signal-observability — 2026-03-07

**Commit:** a7e4f8b

N8 signal health observability — post-closure follow-on from N-STREAM-1/A0.

- Signal health monitor (signal_monitor.rs): app-side TCP probe to
  127.0.0.1:3001 with 4-state machine (unknown/active/degraded/offline),
  5s interval, 3-failure offline threshold, shutdown-aware suppression
- Unified health indicator: aggregates daemon watchdog + signal status
  into combined display (HEALTHY/SIG DEGRADED/SIG OFFLINE/STARTING)
- Individual daemon + signal status dots preserved alongside unified
- get_signal_status Tauri command for point-in-time probe
- signal://status event subscription in frontend daemon service
- Support bundle includes signal_status section
- No transfer gating changes (observability only, per PM approval)
- AC-SE-06 realized: signal health measured by app (runtime owner)
- AC-SE-07 realized: unified indicator reflects daemon + signal state
- Option A topology preserved: app remains signal server owner
- 82 Rust tests (66 existing + 16 new), clippy clean, fmt clean
- 64 web tests (52 existing + 12 new), all pass
- signal/ subtree: zero diff (guardrail verified)

**Files changed:**
- src-tauri/src/signal_monitor.rs (new — 343 lines, 15 tests)
- src-tauri/src/commands.rs (get_signal_status, bundle signal_status)
- src-tauri/src/daemon.rs (shutdown_flag accessor)
- src-tauri/src/lib.rs (module + monitor + command registration)
- web/src/services/daemon.ts (SignalStatus type, event subscription)
- web/src/services/__tests__/daemon.test.ts (4 new signal tests)
- web/src/sections/header.ts (unified + individual indicators)
- web/src/sections/__tests__/header.test.ts (new — 8 unified tests)

---

## localbolt-app-v1.2.13-n6b3-ga-wiring — 2026-03-07

**Commit:** 88954c8

N-STREAM-1 / N6-B3: GA wiring, support bundle completion, cross-platform IPC.

- Platform-aware IPC paths: --socket-path and --data-dir passed to daemon
  at spawn (platform.rs centralized defaults for macOS/Linux/Windows)
- Cross-platform IPC transport abstraction (ipc_transport.rs): IpcStream
  enum supports Unix domain sockets and Windows named pipes. All IPC
  client/bridge code migrated from direct UnixStream usage
- Full support bundle export (commands.rs): 8 manifest sections — daemon
  stderr, crash snapshots, watchdog state, app/daemon versions, platform
  metadata, spawn counters, IPC config. Missing artifacts explicitly marked
- Daemon process management abstracted: platform::process_alive/terminate/
  force_kill helpers (Unix via libc, Windows compile-validated stubs)
- DaemonManager tracks daemon_version and spawn_count for diagnostics
- Windows named pipe path detection and platform-aware binary resolution
  (which → where on Windows)
- Signal server coexistence verified: TCP:3001 vs Unix socket, no conflict
- B-DEP-N1-1 consumed: daemon receives --socket-path and --data-dir
- B-DEP-N2-3 integrated: transport layer supports \\.\pipe\ format
- 66 Rust tests (18 new across platform, ipc_transport, commands, daemon)
- 52 web tests unchanged, all pass
- Quality gates: cargo clippy 0 warnings, cargo fmt clean

No subtree modifications. No daemon protocol changes.

**Files changed:**
- src-tauri/src/platform.rs (new, 202 lines)
- src-tauri/src/ipc_transport.rs (new, 191 lines)
- src-tauri/src/commands.rs (support bundle implementation)
- src-tauri/src/daemon.rs (path wiring, process helpers)
- src-tauri/src/ipc_bridge.rs (IpcStream transport)
- src-tauri/src/ipc_client.rs (IpcStream transport)
- src-tauri/src/lib.rs (module declarations)

---

## localbolt-app-v1.2.12-n6a2-ipc-ui-gating — 2026-03-07

**Commit:** 8f4aea9

N-STREAM-1 / N6-A2: IPC bridge, frontend readiness gating, and daemon UX integration.

- Persistent IPC bridge (ipc_bridge.rs): connects after readiness, forwards
  daemon.status / pairing.request / transfer.incoming.request events to frontend,
  relays pairing.decision / transfer.incoming.decision back to daemon
- IPC types: PairingRequestPayload, TransferIncomingRequestPayload, Decision enum,
  PairingDecisionPayload, TransferIncomingDecisionPayload
- Tauri commands: send_pairing_decision, send_transfer_decision
- DaemonManager emits watchdog state changes via Tauri event bus
- Frontend daemon service (daemon.ts): subscribe to Tauri events, command wrappers,
  graceful non-Tauri degradation, pending request state management
- Header: dual status indicators (signaling + daemon watchdog, 5-state rendering)
- Transfer: readiness gating (fail-closed when daemon not ready), degraded banner
  with restart action, incompatible banner with update message
- Support bundle stub updated: deferred to N6-B, returns NOT_IMPLEMENTED
- @tauri-apps/api added as frontend dependency for Tauri IPC
- 48 Rust tests (11 new in ipc_bridge, ipc_types, commands)
- 52 web tests (20 new daemon service tests with mocked Tauri API)
- Coverage: 91/94/85/90 (above 90/90/80/90 thresholds)

No subtree modifications. No daemon protocol changes.

**Files changed:**
- src-tauri/src/ipc_bridge.rs (new)
- src-tauri/src/ipc_types.rs
- src-tauri/src/commands.rs
- src-tauri/src/daemon.rs
- src-tauri/src/lib.rs
- web/src/services/daemon.ts (new)
- web/src/services/__tests__/daemon.test.ts (new)
- web/src/sections/header.ts
- web/src/sections/transfer.ts
- web/src/app.ts
- web/package.json
- web/package-lock.json

---

## localbolt-app-v1.2.11-n6a-sidecar-lifecycle — 2026-03-07

**Commit:** 0c218bb

N-STREAM-1 / N6-A1: daemon sidecar lifecycle and watchdog core.

- Watchdog state machine (5 states: starting/ready/restarting/degraded/incompatible)
  with 1s/3s/10s backoff, 3 max retries, 60s reset window
- Daemon manager: spawn, stale socket/PID cleanup, SIGTERM/SIGKILL shutdown
- IPC readiness probe: NDJSON version.handshake + daemon.status per N2 contract
- stderr ring buffer (1000 lines) with crash snapshot persistence
- Tauri commands: get_watchdog_state, restart_daemon, export_support_bundle (stub)
- App lifecycle hooks: daemon start on launch, shutdown on window close
- 37 new Rust tests. 32 existing web tests unchanged.

No frontend/UI changes (deferred to N6-A2). No subtree modifications.

**Files changed:**
- .gitignore
- src-tauri/Cargo.lock
- src-tauri/Cargo.toml
- src-tauri/src/lib.rs
- src-tauri/src/commands.rs (new)
- src-tauri/src/daemon.rs (new)
- src-tauri/src/daemon_log.rs (new)
- src-tauri/src/ipc_client.rs (new)
- src-tauri/src/ipc_types.rs (new)
- src-tauri/src/watchdog.rs (new)

---

## localbolt-app-v1.2.10-s-stream-r1-r1.4-security-test-lift — 2026-03-06

**Commit:** 71c3181

R1-4 security-focused product test lift — 21 security-session-integrity tests
covering identity/trust wiring in connect/reconnect paths, stale generation
callback rejection across timing patterns, transfer gating under verification
transitions, and no trust leakage across sessions. Baseline 11 → 32 tests.

**Files changed:**
- web/src/components/__tests__/security-session-integrity.test.ts (new, 416 lines)

---

## localbolt-app-v1.2.9-d5-registry-guards — 2026-03-06

**Commit:** 93afc2c

D5: registry/auth regression guards + CI cleanup removing GitHub Packages auth.
Two new CI guard scripts prevent regression to npm.pkg.github.com for @the9ines
packages. CI workflow cleaned of GitHub Packages auth (registry-url, NODE_AUTH_TOKEN,
packages:read permission).

**Files changed:**
- scripts/check-registry-mapping.sh (new)
- scripts/check-lockfile-registry.sh (new)
- .github/workflows/ci.yml

---

## localbolt-app-v1.2.8-d4-npmjs-cutover — 2026-03-05

**Commit:** 55c3e17

D4: switch consumer resolution from GitHub Packages to npmjs.org.
PAT no longer required for public package installs.
`.npmrc` updated, deps bumped (bolt-core 0.5.1, transport-web 0.6.4,
localbolt-core 0.1.2), lockfile regenerated from registry.npmjs.org.
11 tests pass, build succeeds.

**Files changed:**
- web/.npmrc
- web/package-lock.json
- web/package.json

---

## localbolt-app-v1.2.7-c6-hardening — 2026-03-05

**Commit:** 3ff4625

Add localbolt-core upgrade tooling + coverage threshold enforcement
(C6/Q4). upgrade-localbolt-core.sh with check + upgrade modes.
@vitest/coverage-v8 installed with thresholds (90/90/80/90
lines/functions/branches/statements). CI wired to test:coverage.
Baseline: 100% on tested files. Q4 closed.

**Files changed:**
- scripts/upgrade-localbolt-core.sh (new)
- .gitignore (coverage/)
- .github/workflows/ci.yml (test:coverage)
- web/package.json (test:coverage, coverage-v8)
- web/package-lock.json
- web/vite.config.ts (coverage config)

---

## localbolt-app-v1.2.6-c7-tofu-wiring — 2026-03-05

**Commit:** e902186

Wire identity and TOFU verification flow (Batch 4A) and enforce core
guard scripts in CI (Batch 4B).

**4A — Identity/TOFU wiring (5552f37):**
- Identity keypair persistence via IndexedDBIdentityStore + initIdentity()
- TOFU pinning wired through localbolt-core onVerificationState callback
- Generation-guarded stale callback rejection across disconnect/reconnect
- Mismatch fail-closed with security toast
- Verification states (unverified, verified) now reachable from UI
- 10 new integration tests (tofu-integration.test.ts): identity wiring,
  verification state integration, mismatch fail-closed, generation guard
  race, reject flow
- 11 tests pass. Clean build.

**4B — CI guard wiring (e902186):**
- Core version pin guard (before npm ci)
- Core single-install guard (after npm ci)
- Core drift guard (after build)
- Mirrors transport guard placement in CI workflow

No SDK or subtree edits.

**Files changed:**
- web/src/services/identity.ts (new)
- web/src/components/peer-connection.ts
- web/src/components/__tests__/tofu-integration.test.ts (new)
- web/package.json
- web/package-lock.json
- web/vite.config.ts
- .github/workflows/ci.yml

---

## localbolt-app-v1.2.5-c6-core-guards — 2026-03-05

**Commit:** d1761e9

Add C6 enforcement guards for localbolt-core (version pin, single-install,
drift).

**Files changed:**
- scripts/check-core-version-pin.sh
- scripts/check-core-single-install.sh
- scripts/check-core-drift.sh

---

## localbolt-app-v1.2.4-c5-localbolt-core — 2026-03-05

**Commit:** 0d267b8

Migrate to @the9ines/localbolt-core orchestration (C4). Replace ad-hoc store
transitions with session phase guards, generation-guarded callbacks, canonical
resetSession(), and isTransferAllowed() policy. Deps: bolt-core 0.5.0,
bolt-transport-web 0.6.2, localbolt-core 0.1.0. Identity wiring not connected
(legacy mode). 273 tests pass.

**Files changed:**
- web/package.json
- web/package-lock.json
- web/src/components/peer-connection.ts
- web/src/sections/transfer.ts
- web/src/components/__tests__/peer-connection.test.ts
- web/src/__tests__/app.test.ts

## localbolt-app-v1.2.1 — 2026-02-24

**Commit:** c541b36

Remove hardcoded `wss://localbolt-signal.fly.dev` fallback from
peer-connection.ts (SIG-3). Cloud signaling URL (`VITE_CLOUD_SIGNAL_URL`)
now required via explicit configuration — if unset, cloud signaling is
disabled with console warning and app operates in local-only mode. Local
signaling fallback (`ws://<hostname>:3001`) preserved. Build passes.

- Files changed:
  - `web/src/components/peer-connection.ts`

## localbolt-app-v1.2.0 — 2026-02-24

**Commit:** 90584bf

Bump @the9ines/bolt-core from 0.3.0 to 0.4.0 (A1 adoption). Dead constant
exports removed upstream; no behavior changes. transport-web remains 0.6.0.
Build (vite) passes. No test suite.

- Files changed:
  - `web/package.json`
  - `web/package-lock.json`

## localbolt-app-v1.1.0 — 2026-02-24

**Commit:** c6bb71e

SDK dependency upgrade. Bumped @the9ines/bolt-core from 0.2.0 to 0.3.0 and
@the9ines/bolt-transport-web from 0.2.0 to 0.6.0. Both packages now resolve
from npm.pkg.github.com (transport-web previously used a stale local file:
reference). Zero application code changes; only web/package.json and
web/package-lock.json modified. Build (vite) passes. No test suite exists.

**Files changed:**
- web/package.json
- web/package-lock.json

---

## localbolt-app-v1.0.14 — 2026-02-23

**Commit:** 9bea4ba

Gate release workflow on CI passing (Phase 7C.1). Added `gate-ci` job to
`release.yml` that queries GitHub API to verify CI passed for the commit SHA
before allowing release artifacts to build. Polls up to 10 minutes for CI
completion, blocks release on failure. `workflow_dispatch` bypasses the gate
with a warning (emergency re-release only). CI workflow updated to also trigger
on tag pushes so CI runs exist for tagged commits. Action versions pinned to
SHA digests (`actions/checkout`, `actions/setup-node`, `dtolnay/rust-toolchain`,
`Swatinem/rust-cache`). Added `actions: read` permission to release workflow.

**Files changed:**
- .github/workflows/ci.yml
- .github/workflows/release.yml

---

## localbolt-app-v1.0.13 — 2026-02-23

**Commit:** 561ca1c

Bump bolt-core to 0.2.0 and bolt-transport-web to 0.2.0 (picks up encrypted HELLO + TOFU identity pinning from Phase 7A).

**Files changed:**
- web/package.json
