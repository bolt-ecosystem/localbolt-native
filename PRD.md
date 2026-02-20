# LocalBolt App — Product Requirements Document

**Version:** 1.0.0
**Date:** 2026-02-20

---

## 1. Current State Summary

**Version:** v1.0.0 (production, released 2026-02-18)
**Stack:** Tauri v2, Vanilla TypeScript, Tailwind CSS, Vite, TweetNaCl, embedded Rust signal server
**Test coverage:** Signal protocol tests (Rust); no web frontend tests
**Deployment:** GitHub Releases (macOS aarch64/x64, Windows, Linux deb/rpm)

### Implemented and Working

- Tauri v2 native shell with embedded signal server on port 3001
- Same encryption and transfer engine as localbolt (NaCl box, 16KB chunks, WebRTC)
- Dual signaling (local embedded + cloud at localbolt-signal.fly.dev)
- Multi-platform automated release builds (GitHub Actions)
- Connection approval workflow (request, accept/decline, connect)
- Security hardening (XSS, CSP, peer validation, relay candidate blocking, SAS verification)
- Cross-platform: macOS (Apple Silicon + Intel), Windows (MSI + setup.exe), Linux (deb, rpm)

### Partially Implemented

- Progress tracking: per-file, per-chunk, but ETA is linear calculation
- Device detection: works for major platforms, heuristic-based tablet/desktop distinction

### Missing

- Batch transfer (files queue sequentially, no parallel)
- Download resume (full restart on connection loss)
- Directory transfer (files only)
- Auto-update mechanism (manual download from GitHub Releases)
- Native OS notifications (web toasts only, no system tray alerts)
- bolt-daemon integration (transfer runs in webview process, not background)
- Persistent identity (keys are session-only)
- Accessibility (no ARIA, limited keyboard navigation)
- Localization (English only)

### Legacy Debt

- None. Codebase is fresh post-v1.0.0.

### Production-Ready

- Core transfer pipeline
- Multi-platform builds
- Automated release workflow
- Embedded signal server

---

## 2. Target State (12-Month Horizon)

LocalBolt App becomes the primary native Bolt client:

1. Integrates bolt-daemon for background transfers and identity persistence
2. Consumes bolt-core-sdk instead of inline TweetNaCl
3. Auto-update via Tauri updater
4. Mobile builds (iOS, Android) via Tauri v2
5. System tray integration with native notifications
6. Directory transfer support

---

## 3. Gap Analysis

| Capability | Current | Target | Gap |
|-----------|---------|--------|-----|
| Encryption source | Inline TweetNaCl | bolt-core-sdk | SDK not yet published |
| Background transfers | In webview process | bolt-daemon IPC | Daemon not implemented |
| Identity persistence | Session-only | Daemon-managed key store | Daemon not implemented |
| Auto-update | None | Tauri updater plugin | Plugin integration |
| Mobile | Entry point defined, not built | iOS + Android builds | Full mobile pipeline |
| Notifications | Web toasts | Native OS notifications | Tauri notification plugin |
| Accessibility | None | WCAG 2.1 AA | Significant work |

---

## 4. Non-Goals

1. **Not a web app.** Web versions are localbolt and localbolt-v3.
2. **No relay support.** Local/LAN only. Global relay is ByteBolt.
3. **No accounts.** Zero-knowledge design.
4. **No cloud storage.** Files go peer-to-peer only.
5. **No browser extension.** Native only.

---

## 5. Technical Constraints

- Must bundle embedded signal server (offline-capable)
- Must support macOS 10.15+, Windows 10+, Ubuntu 20.04+
- Mobile: iOS 15+, Android 10+ (when implemented)
- Frontend must remain vanilla TypeScript (shared with localbolt)
- Binary size targets: <20MB macOS, <15MB Windows, <20MB Linux
- Release builds must be automated (no manual signing except Apple notarization)

---

## 6. Dependency Requirements

| Dependency | Status | Required For |
|-----------|--------|-------------|
| bolt-core-sdk (TypeScript) | Not published | SDK migration |
| bolt-core-sdk (Rust) | Not published | Daemon integration |
| bolt-rendezvous | Subtree formalized | Signal server updates |
| bolt-daemon | Not implemented | Background transfers, identity |
| bytebolt-relay | Not applicable | — |

---

## 7. Release Milestones

| Milestone | Version | Description |
|-----------|---------|-------------|
| SDK migration | localbolt-app-v1.1.0 | Replace inline TweetNaCl with bolt-core-sdk |
| Auto-update | localbolt-app-v1.2.0 | Tauri updater plugin integration |
| Native notifications | localbolt-app-v1.3.0 | System tray + OS notification support |
| Daemon integration | localbolt-app-v2.0.0 | Background transfers via bolt-daemon IPC |
| Mobile builds | localbolt-app-v2.1.0 | iOS and Android via Tauri v2 |
| Directory transfer | localbolt-app-v2.2.0 | Recursive directory send/receive |

---

## 8. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|:---:|:---:|-----------|
| Daemon not ready in time | Medium | High | Ship v1.x without daemon; add in v2.0.0 |
| Apple notarization issues | Medium | Medium | Apple Developer account + CI signing |
| Tauri v2 mobile stability | Medium | Medium | Test early, track upstream issues |
| SDK migration breaks crypto | Low | Critical | Conformance test vectors validate equivalence |
| Binary size bloat | Low | Medium | LTO + strip symbols (already configured) |

---

## 9. Success Metrics

- App launches in under 3 seconds on supported platforms
- LAN peer discovery within 5 seconds
- Transfer speed reaches 80%+ of theoretical LAN bandwidth
- Auto-update succeeds silently on 95%+ of installations
- Zero critical security vulnerabilities
- App store rating 4.5+ (when published)
