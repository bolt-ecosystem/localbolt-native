# Changelog — localbolt-app

All notable changes to this project are documented here. Newest first.

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
