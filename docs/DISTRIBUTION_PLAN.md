# LocalBolt Native App — Distribution Plan

**Status:** PARTIAL — macOS live, Linux Phase 1 scaffolded
**Date:** 2026-04-12
**Scope:** Multi-platform native distribution

---

## 1. Current State Audit

### Build Pipeline

| Item | Status | Detail |
|------|--------|--------|
| Build script | Working | `native/macos/build-app.sh` — Swift + Rust + daemon sidecar |
| DMG packaging | Working | `native/macos/create-dmg.sh` — UDZO compressed, Applications symlink |
| Architecture | arm64 only | No universal binary; macOS 14+ (Sonoma) minimum |
| Code signing | Ad-hoc | `codesign --force --sign -` — Gatekeeper blocks by default |
| Notarization | None | Manual steps documented in build scripts but not automated |
| Bundle ID | `com.the9ines.localbolt` | |
| App version | 2.0.0 (Info.plist) | |

### Existing Artifacts

| Artifact | Date | Size | Arch |
|----------|------|------|------|
| `build/LocalBolt-2.0.0-arm64.dmg` | 2026-03-29 | 2.2 MB | arm64 |
| `build/LocalBolt.app` | 2026-04-07 | ~11 MB | arm64 |

### GitHub Releases

| Release | Status | Tag | Date |
|---------|--------|-----|------|
| LocalBolt v1.0.0 | **Draft** | v1.0.0 | 2026-02-19 |

**Stale.** Draft only. Current native releases are tracked by the
`localbolt-app-v2.*` tag series and the SwiftUI native app release artifacts.

### Site References (localbolt.app)

The web frontend mentions "desktop app" in **5 places** (hero tagline, features
card, how-it-works step, 2 FAQ answers) but provides **zero download links**.
No `/download` route exists in Netlify config. Footer links to the self-hosted
repo (`bolt-ecosystem/localbolt`), not the app repo (`bolt-ecosystem/localbolt-native`).

---

## 2. Recommended Distribution Model

### Artifact Hosting: GitHub Releases

GitHub Releases is the correct initial host for native app binaries.

**Why not Netlify?** Netlify is a static site host. Binary downloads should not
be served from it — it's not designed for large asset distribution, and mixing
app binaries with web deployments creates confusion.

**Why not a CDN/S3?** Overkill for initial distribution. GitHub Releases is free,
version-tracked, has checksums, and is where developers already look.

### Download Discovery: localbolt.app redirect

Users should find the download through `localbolt.app`, not by navigating GitHub.

```
localbolt.app/download/macos → redirect to latest GitHub Release asset
```

This gives a stable URL that doesn't change per release. The redirect target
gets updated when a new version ships.

---

## 3. Minimum Viable Release Artifact (macOS)

### Format: `.dmg`

- Standard macOS distribution format for non-App Store apps
- Users understand drag-to-Applications
- `create-dmg.sh` already produces this correctly (UDZO, Applications symlink)

### Architecture: arm64

- arm64-only is acceptable for initial release
- macOS 14 (Sonoma) minimum already excludes pre-2017 hardware
- Intel share of active Macs is ~15% and declining
- Universal binary is a follow-on, not a gate

### Naming Convention

```
LocalBolt-<version>-arm64.dmg
```

Example: `LocalBolt-2.0.0-arm64.dmg`

### Signing & Notarization

| Level | UX Impact | Requirement |
|-------|-----------|-------------|
| Ad-hoc (current) | Right-click → Open → Open | No Apple Developer account |
| Developer ID | Gatekeeper warning, click "Open Anyway" | $99/year Apple Developer Program |
| Developer ID + Notarized | Opens cleanly, no warning | $99/year + `xcrun notarytool` |
| App Store | Standard install UX | $99/year + App Store review + sandboxing |

**Recommendation:** Ship ad-hoc initially with documented Gatekeeper bypass.
Pursue Developer ID + notarization as a fast follow once the Apple Developer
Program enrollment is done.

**Gatekeeper bypass instructions (for ad-hoc):**
> Right-click the app, click Open, then click Open again in the dialog.
> This only needs to be done once — macOS remembers the exception.

---

## 4. Site Integration Strategy

### Where download link should appear

1. **Features section** — the "Browser & Desktop App" card already mentions
   "download the native desktop app for macOS". Add a download link here.

2. **Hero area or nav** — optional CTA button (e.g. "Download for Mac").
   Lower priority than making the features card actionable.

3. **Footer** — add link to `bolt-ecosystem/localbolt-native` GitHub repo alongside
   existing GitHub link.

### Link target

**Option A — Direct GitHub Release (simplest):**
```
https://github.com/bolt-ecosystem/localbolt-native/releases/latest
```
Sends users to the release page where they download the DMG.

**Option B — Netlify redirect (stable URL):**
```toml
# netlify.toml
[[redirects]]
  from = "/download/macos"
  to = "https://github.com/bolt-ecosystem/localbolt-native/releases/download/localbolt-app-v2.0.0/LocalBolt-2.0.0-arm64.dmg"
  status = 302
```
Gives `localbolt.app/download/macos` as a stable shareable URL.
Redirect target updated per release.

**Recommendation:** Option B. A branded `/download/macos` URL is more
professional and doesn't expose GitHub infrastructure to end users.

### Required copy/caveats

Any download link MUST include:
- "macOS only" (no Windows/Linux yet)
- "Apple Silicon (M1+)" (no Intel yet)
- "macOS 14 Sonoma or later"
- Gatekeeper bypass instructions (until notarized)

---

## 5. Auto-Update Prerequisites

Auto-update work MUST NOT begin until all of the following are true:

| Prerequisite | Status | Notes |
|-------------|--------|-------|
| Published GitHub Release with DMG | NOT DONE | No published release exists |
| Stable release artifact URL pattern | NOT DONE | Need consistent tag → asset naming |
| Version detection in app | NOT DONE | App must know its own version at runtime |
| SHA-256 checksums in release | NOT DONE | Integrity verification for downloaded update |
| At least one user-facing release shipped | NOT DONE | Can't update from nothing |
| Signing with Developer ID | OPTIONAL | Ad-hoc works but users re-trigger Gatekeeper on update |

### Additional auto-update design decisions (not yet made)

- **Update feed:** GitHub Releases API (`/repos/.../releases/latest`) vs custom manifest
- **Update mechanism:** Download new DMG + prompt relaunch vs in-place binary swap
- **Update UX:** Badge/notification in app vs blocking modal
- **Rollback:** Keep previous version? Automatic downgrade on crash?

These decisions are deferred until the initial release is shipped and stable.

---

## 6. Implementation Sequence

```
Phase 1: Ship initial release (THIS IS NEXT)
  1. Rebuild app fresh from HEAD (build-app.sh + create-dmg.sh)
  2. Create GitHub Release: localbolt-app-v2.0.0-native-macos
  3. Upload DMG + SHA256 checksum
  4. Add Gatekeeper bypass note to release description

Phase 2: Site download link
  1. Add /download/macos redirect in localbolt-v3 netlify.toml
  2. Add download link to features card in localbolt-web
  3. Add app repo link to footer
  4. Deploy to localbolt.app

Phase 3: Signing (when Apple Developer enrollment completes)
  1. Replace ad-hoc with Developer ID signing in build-app.sh
  2. Add notarization step to create-dmg.sh
  3. Rebuild + re-upload to GitHub Release
  4. Remove Gatekeeper caveat from site copy

Phase 4: Auto-update (deferred)
  - Only after Phases 1-3 are stable and at least one release is in the wild

Phase 5: Linux distribution
  1. Build localbolt-cli from native/linux/cli (scripts/build-linux-cli.sh)
  2. Bundle bolt-daemon Linux binary
  3. Create tarball: localbolt-cli-<version>-x86_64-linux.tar.gz
  4. Upload to GitHub Releases (bolt-ecosystem/localbolt-native)
  5. Add /download/linux and /download/linux/steam-deck redirects to localbolt.app
  6. Document: Desktop Mode required, manual install to ~/.local/bin/

Phase 6: Linux GUI shell (future, requires governance decision)
  - GTK4/libadwaita or other platform-native toolkit
  - Flatpak as primary distribution (SteamOS preferred method)
  - AppImage as fallback
```

---

## 7. Next Implementation Prompt

```
Execute workstream: LOCALBOLT-NATIVE-RELEASE-1

Context:
Distribution plan is codified in localbolt-app/docs/DISTRIBUTION_PLAN.md.
No published GitHub Release exists for the SwiftUI native app.
Build pipeline (build-app.sh + create-dmg.sh) is functional.
Current build artifact: LocalBolt-2.0.0-arm64.dmg (2026-03-29, ad-hoc signed).

Your task:
1. Rebuild the app fresh from current HEAD:
   - cd localbolt-app/native/macos
   - bash build-app.sh release
   - bash create-dmg.sh
2. Generate SHA-256 checksum for the DMG
3. Create a GitHub Release:
   - Tag: localbolt-app-v2.0.0-native-macos
   - Title: LocalBolt 2.0.0 — Native macOS App
   - Body: feature summary, Gatekeeper bypass instructions,
     system requirements (macOS 14+, Apple Silicon)
   - Upload: DMG + SHA256SUMS.txt
   - Mark as latest release (not pre-release, not draft)
4. Verify the release page loads and DMG downloads correctly

Constraints:
- Do not modify any app source code
- Do not attempt signing or notarization (ad-hoc is acceptable)
- Do not add download links to localbolt.app yet (Phase 2)
```
