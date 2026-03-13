# State — localbolt-app

> Current project state. Maintained by docs-keeper agent.

---

## Latest Release

- **Tag:** localbolt-app-v1.2.24-consumer-btr1-p3
- **Commit:** ff33747
- **Date:** 2026-03-12
- **Tests:** 74 web (6 files) + 82 Rust (10 modules) = 156 total

## Dependencies (web/package.json)

| Package | Version | Registry |
|---------|---------|----------|
| @tauri-apps/api | ^2.0.0 | npmjs.org |
| @the9ines/bolt-core | 0.5.2 | npmjs.org |
| @the9ines/bolt-transport-web | 0.6.8 | npmjs.org |
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
- **Tests:** 156 pass (74 web + 82 Rust)
- **BTR:** Enabled (`btrEnabled: true`) — CBTR-3 rollout, burn-in active
- **src-tauri:** N8 signal observability + N6-B3 GA wiring + support bundle + cross-platform IPC transport
- **web:** unified health indicator, signal status subscription, daemon service, readiness gating
