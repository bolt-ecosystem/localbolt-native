#!/usr/bin/env bash
set -euo pipefail

# Build on this machine, copy LocalBolt.app to the MacBook, and register the
# daemon sidecar with macOS Firewall so repeated smoke tests do not require a
# manual "Allow incoming connections" click.
#
# Usage:
#   bash deploy-macbook.sh [host] [arch]
#
# Defaults:
#   host: EOs-MacBook-Pro.local
#   arch: x86_64

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

HOST="${1:-EOs-MacBook-Pro.local}"
TARGET_ARCH="${2:-x86_64}"
REMOTE_APP_DIR="Applications/LocalBolt.app"
REMOTE_DAEMON="\$HOME/${REMOTE_APP_DIR}/Contents/MacOS/bolt-daemon"

echo "[DEPLOY] Building LocalBolt.app for ${TARGET_ARCH} on this Mac..."
CODESIGN_IDENTITY="${CODESIGN_IDENTITY:--}" bash build-app.sh release "$TARGET_ARCH"

echo "[DEPLOY] Preparing ${HOST}:~/Applications..."
ssh -o BatchMode=yes -o ConnectTimeout=5 "$HOST" 'mkdir -p "$HOME/Applications"'

echo "[DEPLOY] Copying app bundle to ${HOST}:~/${REMOTE_APP_DIR}..."
rsync -av --delete "$SCRIPT_DIR/build/LocalBolt.app/" "$HOST:${REMOTE_APP_DIR}/"

echo "[DEPLOY] Verifying copied bundle..."
ssh -o BatchMode=yes -o ConnectTimeout=5 "$HOST" \
    "codesign --verify --verbose \"\$HOME/${REMOTE_APP_DIR}\" && lipo -archs \"\$HOME/${REMOTE_APP_DIR}/Contents/MacOS/LocalBolt\" && lipo -archs \"$REMOTE_DAEMON\""

echo "[DEPLOY] Registering daemon sidecar with macOS Firewall..."
ssh -o BatchMode=yes -o ConnectTimeout=5 "$HOST" \
    "/usr/libexec/ApplicationFirewall/socketfilterfw --add \"$REMOTE_DAEMON\" && /usr/libexec/ApplicationFirewall/socketfilterfw --unblockapp \"$REMOTE_DAEMON\""

echo "[DEPLOY] Firewall entry:"
ssh -o BatchMode=yes -o ConnectTimeout=5 "$HOST" \
    "/usr/libexec/ApplicationFirewall/socketfilterfw --listapps | grep -A1 -B1 \"$REMOTE_DAEMON\" || true"

echo "[DEPLOY] Done: ${HOST}:~/${REMOTE_APP_DIR}"
