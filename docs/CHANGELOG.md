# Changelog — localbolt-app

All notable changes to this project are documented here. Newest first.

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
