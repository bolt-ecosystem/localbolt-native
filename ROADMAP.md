# LocalBolt App — Roadmap

> **SUPERSEDED (2026-04-12).** This roadmap was written for the Tauri v2 implementation (2026-02-20).
> The Tauri path is retired. The current native app is **SwiftUI + Rust FFI + bolt-daemon sidecar** (macOS-only, v2.0.0).
> Items referencing Tauri updater, Tauri notifications, or Tauri v2 mobile builds are no longer applicable.
> This document is preserved for historical context.

**Date:** 2026-02-20 (historical — Tauri era)

---

## Stability Work

### S1. Add web frontend tests
- Port test suite from localbolt (shared codebase)
- Add Vitest configuration to web/
- Target 80% coverage on shared components

### S2. SDK migration
- Replace inline TweetNaCl with @the9ines/bolt-core
- Validate with conformance test vectors
- **Depends on:** bolt-core-sdk npm + crate publish

### S3. Subtree maintenance
- Document subtree pull procedure
- Verify signal/ stays in sync with bolt-rendezvous

---

## Infrastructure Work

### ~~I1. Auto-update~~ (Tauri retired)
- ~~Integrate Tauri updater plugin~~
- ~~Configure update endpoint (GitHub Releases or custom)~~
- ~~Silent background check, user-prompted install~~

### I2. Code signing
- Apple Developer ID signing for macOS
- Windows Authenticode signing
- Notarization pipeline in CI

### I3. Daemon integration
- Replace in-process signaling with bolt-daemon IPC
- Enable background transfers (app can close, daemon continues)
- Identity persistence via daemon key store
- **Depends on:** bolt-daemon v1.0.0

---

## Feature Work

### ~~F1. Native notifications~~ (Tauri retired)
- ~~Tauri notification plugin~~
- System tray integration
- Transfer complete / incoming request alerts

### ~~F2. Mobile builds~~ (Tauri retired)
- ~~iOS build via Tauri v2~~
- ~~Android build via Tauri v2~~
- ~~Platform-specific UI adjustments~~
- ~~App store submission preparation~~
- ~~**Depends on:** I2 (code signing), I3 (daemon)~~

### F3. Directory transfer
- Native file dialog for directory selection
- Recursive directory send/receive
- Preserve directory structure
- **Depends on:** S2 (SDK migration)

### F4. Bandwidth optimization
- Adaptive chunk sizes based on connection quality
- Optional compression before encryption
- **Depends on:** S2 (SDK migration)

---

## Execution Order

```
S1 (tests) ────────────────────────────────────────►
  │
  ▼
S2 (SDK migration) ──► F3 (directory transfer)
  │                         │
  ▼                         ▼
S3 (subtree)           F4 (bandwidth)

I1 (auto-update) ──► I2 (code signing) ──► F2 (mobile)
                                               │
I3 (daemon) ──────────────────────────────────►│
  │
  ▼
F1 (notifications)
```

---

## Critical Path

S2 (SDK) → I3 (daemon) → F2 (mobile)

Mobile builds are the long pole.
Daemon integration is the highest-value infrastructure milestone.
SDK migration unblocks all feature work.
