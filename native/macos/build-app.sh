#!/usr/bin/env bash
set -euo pipefail

# Build LocalBolt.app — macOS native shell
#
# Prerequisites:
#   1. Rust toolchain installed
#   2. cd native/shared && cargo build --release
#   3. Then run this script from native/macos/

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

APP_NAME="LocalBolt"
BUILD_MODE="${1:-release}"
BUNDLE_DIR="$SCRIPT_DIR/build/${APP_NAME}.app"

echo "=== Building ${APP_NAME}.app (${BUILD_MODE}) ==="

# Step 1: Ensure Rust native bridge is built
RUST_LIB="../shared/target/release/libbolt_native_bridge.a"
if [ ! -f "$RUST_LIB" ]; then
    echo "[BUILD] Building Rust native bridge..."
    (cd ../shared && cargo build --release)
fi
echo "[BUILD] Rust bridge: $RUST_LIB"

# Step 2: Build Swift executable
echo "[BUILD] Building Swift executable (${BUILD_MODE})..."
if [ "$BUILD_MODE" = "release" ]; then
    swift build -c release
    SWIFT_BIN=".build/release/${APP_NAME}"
else
    swift build
    SWIFT_BIN=".build/debug/${APP_NAME}"
fi

if [ ! -f "$SWIFT_BIN" ]; then
    echo "[FAIL] Swift binary not found: $SWIFT_BIN"
    exit 1
fi
echo "[BUILD] Swift binary: $SWIFT_BIN"

# Step 3: Assemble .app bundle
echo "[BUILD] Assembling ${APP_NAME}.app..."
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

# Copy executable
cp "$SWIFT_BIN" "$BUNDLE_DIR/Contents/MacOS/${APP_NAME}"

# Copy Info.plist
cp "Resources/Info.plist" "$BUNDLE_DIR/Contents/Info.plist"

# Copy bolt-daemon binary into bundle (sidecar)
# bolt-daemon: SCRIPT_DIR/../../../ = localbolt-app, which is inside bolt-ecosystem
DAEMON_BIN="$SCRIPT_DIR/../../../bolt-daemon/target/release/bolt-daemon"
if [ -f "$DAEMON_BIN" ]; then
    cp "$DAEMON_BIN" "$BUNDLE_DIR/Contents/MacOS/bolt-daemon"
    echo "[BUILD] Daemon sidecar: bundled"
else
    echo "[WARN] bolt-daemon not found at $DAEMON_BIN — daemon must be on PATH"
fi

# Create minimal PkgInfo
echo -n "APPL????" > "$BUNDLE_DIR/Contents/PkgInfo"

echo ""
echo "=== Build complete ==="
echo "  App:  $BUNDLE_DIR"
echo "  Size: $(du -sh "$BUNDLE_DIR" | cut -f1)"
echo ""
echo "  Run:  open $BUNDLE_DIR"
echo "  Or:   $BUNDLE_DIR/Contents/MacOS/${APP_NAME}"
