# Changelog — localbolt-app

All notable changes to this project are documented here. Newest first.

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
