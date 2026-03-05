# State — localbolt-app

> Current project state. Maintained by docs-keeper agent.

---

## Latest Release

- **Tag:** localbolt-app-v1.2.5-c6-core-guards
- **Commit:** d1761e9
- **Date:** 2026-03-05

## Dependencies (web/package.json)

| Package | Version |
|---------|---------|
| @the9ines/bolt-core | 0.5.0 |
| @the9ines/bolt-transport-web | 0.6.2 |
| @the9ines/localbolt-core | 0.1.0 |
| tweetnacl | ^1.0.3 |
| tweetnacl-util | ^0.15.1 |

## C6 Guards

| Script | Purpose |
|--------|---------|
| scripts/check-core-version-pin.sh | Verify localbolt-core version pin |
| scripts/check-core-single-install.sh | Verify single install in tree |
| scripts/check-core-drift.sh | Detect declared vs resolved drift |

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
- **Tests:** 1 test pass
- **src-tauri:** Unchanged (not yet consuming localbolt-core)
