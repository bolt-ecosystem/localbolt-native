# LocalBolt — Multi-Platform Release Plan

Roadmap, not status. Current build state lives in the ecosystem journal.

## Architecture (decided 2026-07-03)

**Rust core + a thin native shell per platform** — no web/Tauri wrapper. Each shell talks
to the shared Rust core (`bolt-daemon` + `bolt_native_bridge`), the same pattern the macOS
SwiftUI app already uses. Priority is native quality over speed ("go slow").

| Platform | Shell | Core link |
|---|---|---|
| macOS | SwiftUI (exists) | FFI bridge (Swift → Rust C ABI) |
| Steam Deck / Linux | native GUI — see §2 | direct (Rust GUI) or C ABI (GTK/Qt) |
| Windows | native GUI (later) | C ABI (C#/WinUI) or direct (Rust GUI) |

## Order (set 2026-07-03)

**1. Finish macOS → 2. Steam Deck → 3. Windows (later).**

## 1. macOS (arm64 + Intel) — finish first

Done: universal build (both arches, cross-compile verified), universal DMG,
`create-dmg.sh` universal labeling, and `native/macos/RELEASE.md` (sign/notarize runbook).

Remaining:
- **Developer ID signing + notarization** — the hard blocker; needs an Apple Developer
  account ($99/yr). This unblocks both the Intel and Apple-Silicon releases at once.
- **Sparkle** auto-updater (so fixes ship without a hand-reinstall).
- Smoke test on a **real Intel Mac** (the x86_64 slice builds but hasn't run on Intel).

## 2. Steam Deck / Linux — next

The Deck needs an **app, not an installable program**: SteamOS has an immutable root
filesystem, so the format is **Flatpak** (installed from Discover/Flathub in Desktop Mode,
then "Add as non-Steam Game" for Game Mode). AppImage is a portable fallback.

- **Shell (decide at kickoff):** lean toward a **Rust-native GUI (iced or Slint)** — links the
  Rust core directly with *no* FFI bridge, renders on the GPU (good for Game Mode + controller),
  and Flatpaks cleanly. Alternative: **GTK4 / Qt** (more desktop-idiomatic, but needs the C-ABI
  bridge and is mouse-oriented).
- **Daemon:** verify `bolt-daemon` builds for `x86_64-unknown-linux-gnu` (the Deck's arch) —
  a cheap de-risk we can run anytime.
- **Package:** Flatpak manifest → Flathub submission (free; Flathub handles trust + auto-update).
  Grant `--share=network` for LAN discovery.
- **Deck UX:** large touch targets, controller navigation, on-screen keyboard; works in both
  Desktop and Game Mode.

## 3. Windows (x64) — later

- Shell: WinUI 3 (native) via a C ABI, or the same Rust-native GUI.
- Daemon: verify Windows build; Windows Firewall handling.
- Package: MSI (WiX) / NSIS. Sign: **Authenticode** (~$100–400/yr, else SmartScreen warns).
  Update: WinSparkle / Squirrel.

## Cross-cutting

- **FFI bridge:** `bolt_native_bridge` is Swift-tuned today. A Rust-GUI shell skips it entirely
  (Rust-to-Rust, in-process); a GTK/WinUI shell needs a generalized C ABI.
- **Signing accounts:** Apple ($99/yr, unblocks Mac now); Windows Authenticode (later);
  Linux/Flathub is free.
- **Test hardware:** an Intel Mac, a Windows PC, a Steam Deck.
- **CI (later):** GitHub Actions matrix — macOS (signing secrets), Windows, Linux/Flatpak runners.

## Not in scope

ByteBolt (the global relay). The web products (localbolt-v3, etc.) ship on their own tracks.
