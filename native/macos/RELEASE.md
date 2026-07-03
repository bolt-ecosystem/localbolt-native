# LocalBolt — macOS Release Runbook

Timeless *procedure*, not status. What is built and where it stands lives in the
ecosystem journal, never here.

## Why an ad-hoc build cannot ship

LocalBolt is a peer-to-peer LAN file-transfer app. A public release means a Mac that
has never seen it, with default Gatekeeper, can download and open it with a
double-click. That requires Apple **code signing (Developer ID) + notarization**. An
ad-hoc-signed build is **rejected** by `spctl` on every machine except the one that
built it — verify with `spctl -a -vv build/LocalBolt.app`.

## One-time prerequisites (human — needs your Apple ID + payment)

1. **Apple Developer Program** ($99/yr): <https://developer.apple.com/programs/>
2. **Developer ID Application** certificate — Xcode → Settings → Accounts → Manage
   Certificates → **+** → *Developer ID Application*. Confirm it landed:
   `security find-identity -v -p codesigning` must list
   `Developer ID Application: <name> (<TEAMID>)`.
3. **notarytool credentials**, stored once (app-specific password from
   appleid.apple.com → Sign-In and Security → App-Specific Passwords):
   ```bash
   xcrun notarytool store-credentials localbolt-notary \
     --apple-id <you@apple.id> --team-id <TEAMID> --password <app-specific-pw>
   ```

## Build → sign → notarize → staple (repeatable)

```bash
cd localbolt-app/native/macos
export CODESIGN_IDENTITY="Developer ID Application: <name> (<TEAMID>)"

# 1. Universal build signed with your Developer ID (see "Universal build" below).
# 2. Package the DMG.
bash create-dmg.sh                                   # -> build/LocalBolt-<ver>-universal.dmg

# 3. Notarize and wait for Apple's verdict.
xcrun notarytool submit build/LocalBolt-<ver>-universal.dmg \
  --keychain-profile localbolt-notary --wait

# 4. Staple the ticket to the app, re-package so the stapled app ships, staple the DMG.
xcrun stapler staple build/LocalBolt.app
bash create-dmg.sh
xcrun stapler staple build/LocalBolt-<ver>-universal.dmg

# 5. Prove a clean machine would accept it.
spctl -a -vv build/LocalBolt.app        # expect: accepted, source=Notarized Developer ID
```

## Universal build (Intel + Apple Silicon)

`build-app.sh <mode> <arch>` builds one arch. For a universal bundle, build both,
`lipo` the two Mach-O binaries (`LocalBolt`, `bolt-daemon`), then re-sign. Requires
`rustup target add x86_64-apple-darwin` (the daemon's native-full transports have been
verified to cross-compile).

```bash
bash build-app.sh release arm64
mkdir -p /tmp/arm64 && cp build/LocalBolt.app/Contents/MacOS/{LocalBolt,bolt-daemon} /tmp/arm64/
bash build-app.sh release x86_64
for bin in LocalBolt bolt-daemon; do
  lipo -create /tmp/arm64/$bin build/LocalBolt.app/Contents/MacOS/$bin \
    -output build/LocalBolt.app/Contents/MacOS/$bin
done
xattr -cr build/LocalBolt.app            # strip detritus codesign rejects
codesign --force --sign "$CODESIGN_IDENTITY" --entitlements Resources/LocalBolt.entitlements \
  build/LocalBolt.app/Contents/MacOS/bolt-daemon
codesign --force --sign "$CODESIGN_IDENTITY" --entitlements Resources/LocalBolt.entitlements \
  build/LocalBolt.app
```

`create-dmg.sh` labels a two-arch bundle `universal` automatically.

## Before a real "mass" release (beyond signing)

- **Run on an actual Intel Mac** — the x86_64 slice builds but has not executed on Intel hardware.
- **Beta** across more machines and macOS versions. LocalBolt is LAN-only by design; test
  real device pairs on a shared network, not just localhost.
- **Distribution + auto-update** — a download page and an updater (e.g. Sparkle) so users get
  fixes without a hand-reinstall. Without this, the "stale build" failure class is permanent.
- **Crash/error reporting** — none today.

## Scope

This runbook is the LocalBolt macOS app only. ByteBolt (global relay) is not built. The web
products (localbolt-v3, etc.) ship on their own tracks.
