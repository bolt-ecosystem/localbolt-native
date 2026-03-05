# Changelog — localbolt-app

All notable changes to this project are documented here. Newest first.

---

## localbolt-app-v1.2.5-c6-core-guards — 2026-03-05

**Commit:** d1761e9

Add C6 enforcement guards for localbolt-core (version pin, single-install,
drift). Three shell scripts verify that @the9ines/localbolt-core remains at
the pinned version, is installed exactly once in the dependency tree, and that
no drift exists between the declared and resolved version.

- Files changed:
  - `scripts/check-core-version-pin.sh`
  - `scripts/check-core-single-install.sh`
  - `scripts/check-core-drift.sh`

## localbolt-app-v1.2.4-c5-localbolt-core — 2026-03-05

**Commit:** 0d267b8

Migrate web layer to @the9ines/localbolt-core orchestration (C5). Replace
ad-hoc store transitions with session phase guards, generation-guarded
callbacks, canonical resetSession(), and isTransferAllowed() policy. Deps:
bolt-core 0.5.0, bolt-transport-web 0.6.2, localbolt-core 0.1.0. Identity
wiring not connected (legacy mode). src-tauri untouched. 1 test pass.

- Files changed:
  - `web/package.json`
  - `web/package-lock.json`
  - `web/src/components/peer-connection.ts`
  - `web/src/sections/transfer.ts`

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
