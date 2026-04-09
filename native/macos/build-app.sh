#!/usr/bin/env bash
set -euo pipefail

# Build LocalBolt.app — macOS native shell
#
# Prerequisites:
#   1. Rust toolchain installed
#   2. Then run this script from native/macos/
#
# Usage:
#   bash build-app.sh           # release build (default)
#   bash build-app.sh debug     # debug build
#   bash build-app.sh release   # explicit release build

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

APP_NAME="LocalBolt"
BUILD_MODE="${1:-release}"
BUNDLE_DIR="$SCRIPT_DIR/build/${APP_NAME}.app"
ENTITLEMENTS="$SCRIPT_DIR/Resources/LocalBolt.entitlements"

echo "=== Building ${APP_NAME}.app (${BUILD_MODE}) ==="

# Step 1: Always rebuild Rust native bridge (ensures ABI sync with bolt-app-core)
RUST_LIB="../shared/target/release/libbolt_native_bridge.a"
echo "[BUILD] Building Rust native bridge..."
(cd ../shared && cargo build --release)
echo "[BUILD] Rust bridge: $RUST_LIB (static)"

# Step 2: Build Swift executable
echo "[BUILD] Building Swift executable (${BUILD_MODE})..."
if [ "$BUILD_MODE" = "release" ]; then
    swift build -c release 2>&1 | grep -v "^ld: warning:" || true
    SWIFT_BIN=".build/release/${APP_NAME}"
else
    swift build 2>&1 | grep -v "^ld: warning:" || true
    SWIFT_BIN=".build/debug/${APP_NAME}"
fi

if [ ! -f "$SWIFT_BIN" ]; then
    echo "[FAIL] Swift binary not found: $SWIFT_BIN"
    exit 1
fi
echo "[BUILD] Swift binary: $SWIFT_BIN"

# Step 3: Build bolt-daemon with WebTransport support
DAEMON_SRC="$SCRIPT_DIR/../../../bolt-daemon"
DAEMON_BIN="$DAEMON_SRC/target/release/bolt-daemon"
echo "[BUILD] Building bolt-daemon (transport-ws + transport-webtransport)..."
(cd "$DAEMON_SRC" && cargo build --release --features transport-ws,transport-webtransport)
echo "[BUILD] Daemon binary: $DAEMON_BIN"

# Step 4: Assemble .app bundle
echo "[BUILD] Assembling ${APP_NAME}.app..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"

# Copy executable
cp "$SWIFT_BIN" "$BUNDLE_DIR/Contents/MacOS/${APP_NAME}"

# Copy Info.plist
cp "Resources/Info.plist" "$BUNDLE_DIR/Contents/Info.plist"

# Copy bolt-daemon binary into bundle (sidecar)
if [ -f "$DAEMON_BIN" ]; then
    cp "$DAEMON_BIN" "$BUNDLE_DIR/Contents/MacOS/bolt-daemon"
    echo "[BUILD] Daemon sidecar: bundled (WT-enabled)"
else
    echo "[FAIL] bolt-daemon not found at $DAEMON_BIN"
    exit 1
fi

# Create PkgInfo
echo -n "APPL????" > "$BUNDLE_DIR/Contents/PkgInfo"

# Step 5: Ad-hoc code sign (required for valid bundle on macOS)
# For distribution, replace with: codesign --sign "Developer ID Application: ..."
echo "[BUILD] Code signing (ad-hoc)..."
# Sign the daemon sidecar first (nested code)
if [ -f "$BUNDLE_DIR/Contents/MacOS/bolt-daemon" ]; then
    codesign --force --sign - \
        --entitlements "$ENTITLEMENTS" \
        "$BUNDLE_DIR/Contents/MacOS/bolt-daemon"
fi
# Sign the main app bundle
codesign --force --sign - \
    --entitlements "$ENTITLEMENTS" \
    "$BUNDLE_DIR"

# Step 6: Verify
echo "[BUILD] Verifying..."
codesign --verify --verbose "$BUNDLE_DIR" 2>&1 | head -3
LINK_CHECK=$(otool -L "$BUNDLE_DIR/Contents/MacOS/${APP_NAME}" 2>&1 | grep "bolt_native_bridge" || true)
if [ -n "$LINK_CHECK" ]; then
    echo "[FAIL] Dynamic link to bolt_native_bridge detected — must be static"
    echo "  $LINK_CHECK"
    exit 1
fi
echo "[BUILD] Static link verified: no bolt_native_bridge dylib dependency"

echo ""
echo "=== Build complete ==="
echo "  App:    $BUNDLE_DIR"
echo "  Size:   $(du -sh "$BUNDLE_DIR" | cut -f1)"
echo "  Signed: ad-hoc (replace with Developer ID for distribution)"
echo ""
echo "  Run:    open $BUNDLE_DIR"
echo "  Or:     $BUNDLE_DIR/Contents/MacOS/${APP_NAME}"
echo ""
echo "  Distribution checklist:"
echo "    [ ] Replace ad-hoc sign with: codesign --sign 'Developer ID Application: ...'"
echo "    [ ] Notarize with: xcrun notarytool submit LocalBolt.dmg --apple-id ... --team-id ..."
echo "    [ ] Staple with: xcrun stapler staple LocalBolt.app"
