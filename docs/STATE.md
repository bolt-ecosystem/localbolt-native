# State — localbolt-app

> Current project state. Maintained by docs-keeper agent.

---

## Latest Release

- **Tag:** localbolt-app-v1.2.12-n6a2-ipc-ui-gating
- **Commit:** 8f4aea9
- **Date:** 2026-03-07
- **Tests:** 52 web (4 files) + 48 Rust (7 modules) = 100 total

## Dependencies (web/package.json)

| Package | Version | Registry |
|---------|---------|----------|
| @tauri-apps/api | ^2.0.0 | npmjs.org |
| @the9ines/bolt-core | 0.5.1 | npmjs.org |
| @the9ines/bolt-transport-web | 0.6.4 | npmjs.org |
| @the9ines/localbolt-core | 0.1.2 | npmjs.org |
| tweetnacl | ^1.0.3 |
| tweetnacl-util | ^0.15.1 |

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
- **Build:** Passing (vite)
- **Tests:** 100 pass (52 web + 48 Rust)
- **src-tauri:** N6-A2 IPC bridge + event forwarding + decision relay
- **web:** daemon service, readiness gating, degraded/incompatible UX
