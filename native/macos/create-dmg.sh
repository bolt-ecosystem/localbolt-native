#!/usr/bin/env bash
set -euo pipefail

# Create LocalBolt.dmg — distributable macOS disk image
#
# Prerequisites:
#   bash build-app.sh release   (produces build/LocalBolt.app)
#
# Usage:
#   bash create-dmg.sh                    # uses version from Info.plist
#   bash create-dmg.sh 2.0.1             # explicit version override

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

APP_NAME="LocalBolt"
BUNDLE_DIR="$SCRIPT_DIR/build/${APP_NAME}.app"

if [ ! -d "$BUNDLE_DIR" ]; then
    echo "[FAIL] ${APP_NAME}.app not found. Run 'bash build-app.sh release' first."
    exit 1
fi

# Read version from Info.plist or use override
if [ $# -ge 1 ]; then
    VERSION="$1"
else
    VERSION=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$BUNDLE_DIR/Contents/Info.plist" 2>/dev/null || echo "2.0.0")
fi

# Determine architecture
ARCH=$(uname -m)
case "$ARCH" in
    arm64) ARCH_LABEL="arm64" ;;
    x86_64) ARCH_LABEL="x64" ;;
    *) ARCH_LABEL="$ARCH" ;;
esac

DMG_NAME="${APP_NAME}-${VERSION}-${ARCH_LABEL}.dmg"
DMG_PATH="$SCRIPT_DIR/build/${DMG_NAME}"
VOLUME_NAME="${APP_NAME} ${VERSION}"
STAGING_DIR="$SCRIPT_DIR/build/dmg-staging"

echo "=== Creating ${DMG_NAME} ==="
echo "  App:     ${BUNDLE_DIR}"
echo "  Version: ${VERSION}"
echo "  Arch:    ${ARCH_LABEL}"

# Clean previous
rm -rf "$STAGING_DIR" "$DMG_PATH"

# Stage DMG contents
mkdir -p "$STAGING_DIR"
cp -R "$BUNDLE_DIR" "$STAGING_DIR/"
ln -s /Applications "$STAGING_DIR/Applications"

# Create DMG from staging directory
echo "[DMG] Creating disk image..."
hdiutil create \
    -volname "$VOLUME_NAME" \
    -srcfolder "$STAGING_DIR" \
    -ov \
    -format UDZO \
    -imagekey zlib-level=9 \
    "$DMG_PATH"

# Clean staging
rm -rf "$STAGING_DIR"

# Verify
echo ""
echo "[DMG] Verifying..."
hdiutil verify "$DMG_PATH" 2>&1 | tail -1

# Mount and check contents
MOUNT_POINT=$(hdiutil attach "$DMG_PATH" -nobrowse -noverify -noautoopen 2>/dev/null | grep "/Volumes/" | awk -F'\t' '{print $NF}')
if [ -n "$MOUNT_POINT" ]; then
    echo "[DMG] Mounted at: $MOUNT_POINT"
    echo "[DMG] Contents:"
    ls -la "$MOUNT_POINT/" | grep -v "^total"

    # Verify app bundle exists and is valid
    if [ -d "$MOUNT_POINT/${APP_NAME}.app" ]; then
        echo "[DMG] App bundle: present"
        codesign --verify "$MOUNT_POINT/${APP_NAME}.app" 2>&1 && echo "[DMG] Codesign: valid" || echo "[DMG] Codesign: INVALID"
    else
        echo "[DMG] App bundle: MISSING"
    fi

    # Verify Applications symlink
    if [ -L "$MOUNT_POINT/Applications" ]; then
        echo "[DMG] Applications link: present"
    else
        echo "[DMG] Applications link: MISSING"
    fi

    hdiutil detach "$MOUNT_POINT" -quiet 2>/dev/null
else
    echo "[WARN] Could not mount DMG for verification"
fi

echo ""
echo "=== DMG complete ==="
echo "  Output: ${DMG_PATH}"
echo "  Size:   $(du -sh "$DMG_PATH" | cut -f1)"
echo ""
echo "  Distribution steps remaining:"
echo "    [ ] Sign app with Developer ID: bash build-app.sh release  (modify codesign identity)"
echo "    [ ] Notarize: xcrun notarytool submit '${DMG_PATH}' --apple-id ... --team-id ..."
echo "    [ ] Staple:   xcrun stapler staple '${BUNDLE_DIR}' && recreate DMG"
