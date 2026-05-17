#!/usr/bin/env bash
set -euo pipefail

# Build LocalBolt.app — macOS native shell
#
# Prerequisites:
#   1. Rust toolchain installed (with target for cross-compile)
#   2. Then run this script from native/macos/
#
# Usage:
#   bash build-app.sh                    # release build, host architecture
#   bash build-app.sh debug              # debug build, host architecture
#   bash build-app.sh release arm64      # release build, Apple Silicon
#   bash build-app.sh release x86_64     # release build, Intel

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

APP_NAME="LocalBolt"
BUILD_MODE="${1:-release}"
TARGET_ARCH="${2:-$(uname -m)}"
BUNDLE_DIR="$SCRIPT_DIR/build/${APP_NAME}.app"
ENTITLEMENTS="$SCRIPT_DIR/Resources/LocalBolt.entitlements"

# Resolve Rust target triple
case "$TARGET_ARCH" in
    arm64)  RUST_TARGET="aarch64-apple-darwin" ;;
    x86_64) RUST_TARGET="x86_64-apple-darwin" ;;
    *) echo "[FAIL] Unsupported architecture: $TARGET_ARCH"; exit 1 ;;
esac

HOST_ARCH=$(uname -m)
CROSS_COMPILE=false
if [ "$TARGET_ARCH" != "$HOST_ARCH" ]; then
    CROSS_COMPILE=true
    echo "=== Building ${APP_NAME}.app (${BUILD_MODE}, cross-compile ${HOST_ARCH} → ${TARGET_ARCH}) ==="
else
    echo "=== Building ${APP_NAME}.app (${BUILD_MODE}, ${TARGET_ARCH}) ==="
fi

# Step 1: Build Rust native bridge
echo "[BUILD] Building Rust native bridge (${RUST_TARGET})..."
SHARED_DIR="../shared"
if $CROSS_COMPILE; then
    (cd "$SHARED_DIR" && cargo build --release --target "$RUST_TARGET")
    RUST_LIB="$SHARED_DIR/target/${RUST_TARGET}/release/libbolt_native_bridge.a"
else
    (cd "$SHARED_DIR" && cargo build --release)
    RUST_LIB="$SHARED_DIR/target/release/libbolt_native_bridge.a"
fi
echo "[BUILD] Rust bridge: $RUST_LIB (static, ${TARGET_ARCH})"

# Stage the .a to the path Package.swift expects (target/release/)
# This is a no-op for native builds but required for cross-compile.
EXPECTED_LIB="$SHARED_DIR/target/release/libbolt_native_bridge.a"
if [ "$RUST_LIB" != "$EXPECTED_LIB" ]; then
    mkdir -p "$(dirname "$EXPECTED_LIB")"
    cp "$RUST_LIB" "$EXPECTED_LIB"
    echo "[BUILD] Staged ${TARGET_ARCH} library → ${EXPECTED_LIB}"
fi

# Step 2: Build Swift executable
echo "[BUILD] Building Swift executable (${BUILD_MODE}, ${TARGET_ARCH})..."
if $CROSS_COMPILE; then
    swift build -c "$BUILD_MODE" --arch "$TARGET_ARCH" 2>&1 | grep -v "^ld: warning:" || true
else
    swift build -c "$BUILD_MODE" 2>&1 | grep -v "^ld: warning:" || true
fi

if [ "$BUILD_MODE" = "release" ]; then
    SWIFT_BIN=".build/release/${APP_NAME}"
else
    SWIFT_BIN=".build/debug/${APP_NAME}"
fi

if [ ! -f "$SWIFT_BIN" ]; then
    echo "[FAIL] Swift binary not found: $SWIFT_BIN"
    exit 1
fi
echo "[BUILD] Swift binary: $SWIFT_BIN"

# Step 3: Build bolt-daemon with native production transports.
DAEMON_SRC="$SCRIPT_DIR/../../../bolt-daemon"
echo "[BUILD] Building bolt-daemon (${RUST_TARGET}, native-full: WS + WT + QUIC)..."
if $CROSS_COMPILE; then
    (cd "$DAEMON_SRC" && cargo build --release --target "$RUST_TARGET" --features native-full)
    DAEMON_BIN="$DAEMON_SRC/target/${RUST_TARGET}/release/bolt-daemon"
else
    (cd "$DAEMON_SRC" && cargo build --release --features native-full)
    DAEMON_BIN="$DAEMON_SRC/target/release/bolt-daemon"
fi
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
    echo "[BUILD] Daemon sidecar: bundled (native-full)"
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
ACTUAL_ARCH=$(lipo -archs "$BUNDLE_DIR/Contents/MacOS/${APP_NAME}" 2>/dev/null || echo "unknown")
if [ "$ACTUAL_ARCH" != "$TARGET_ARCH" ]; then
    echo "[FAIL] Architecture mismatch: expected $TARGET_ARCH, got $ACTUAL_ARCH"
    exit 1
fi
echo "[BUILD] Architecture verified: $ACTUAL_ARCH"

LINK_CHECK=$(otool -L "$BUNDLE_DIR/Contents/MacOS/${APP_NAME}" 2>&1 | grep "bolt_native_bridge" || true)
if [ -n "$LINK_CHECK" ]; then
    echo "[FAIL] Dynamic link to bolt_native_bridge detected — must be static"
    echo "  $LINK_CHECK"
    exit 1
fi
echo "[BUILD] Static link verified: no bolt_native_bridge dylib dependency"

# Verify daemon architecture matches
DAEMON_ARCH=$(lipo -archs "$BUNDLE_DIR/Contents/MacOS/bolt-daemon" 2>/dev/null || echo "unknown")
if [ "$DAEMON_ARCH" != "$TARGET_ARCH" ]; then
    echo "[FAIL] Daemon architecture mismatch: expected $TARGET_ARCH, got $DAEMON_ARCH"
    exit 1
fi
echo "[BUILD] Daemon architecture verified: $DAEMON_ARCH"

echo ""
echo "=== Build complete ==="
echo "  App:    $BUNDLE_DIR"
echo "  Arch:   $TARGET_ARCH"
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
