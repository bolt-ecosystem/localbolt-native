#!/usr/bin/env bash
set -euo pipefail

# Signaling endpoint drift guard
#
# Asserts that the native macOS app and the web app both reference the same
# canonical cloud signaling URL.  Run this in CI or before tagging a release
# to catch accidental endpoint mismatches.
#
# Usage:
#   bash scripts/check-signaling-endpoint-drift.sh
#
# Requires: the bolt-ecosystem workspace layout
#   ../../localbolt-v3/packages/localbolt-web/.env   (or Netlify env)
#   native/macos/Sources/LocalBolt/LocalBoltApp.swift

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CANONICAL="wss://bolt-rendezvous.fly.dev"
EXIT=0

echo "=== Signaling endpoint drift check ==="
echo "Canonical endpoint: $CANONICAL"
echo ""

# --- Native macOS app ---
NATIVE_SWIFT="$REPO_ROOT/native/macos/Sources/LocalBolt/LocalBoltApp.swift"
if [ ! -f "$NATIVE_SWIFT" ]; then
    echo "[SKIP] Native Swift source not found: $NATIVE_SWIFT"
else
    NATIVE_URL=$(grep -oE 'cloudUrl:\s*"[^"]+"' "$NATIVE_SWIFT" | head -1 | grep -oE '"[^"]+"' | tr -d '"')
    if [ -z "$NATIVE_URL" ]; then
        echo "[FAIL] Could not extract cloudUrl from $NATIVE_SWIFT"
        EXIT=1
    elif [ "$NATIVE_URL" != "$CANONICAL" ]; then
        echo "[FAIL] Native macOS endpoint MISMATCH"
        echo "  Expected: $CANONICAL"
        echo "  Found:    $NATIVE_URL"
        EXIT=1
    else
        echo "[OK]   Native macOS: $NATIVE_URL"
    fi
fi

# --- Web app (.env — local dev) ---
WEB_ENV="$REPO_ROOT/../../localbolt-v3/packages/localbolt-web/.env"
if [ -f "$WEB_ENV" ]; then
    WEB_URL=$(grep -E '^VITE_SIGNAL_URL=' "$WEB_ENV" | head -1 | cut -d= -f2- | tr -d '"' | tr -d "'" | xargs)
    if [ -z "$WEB_URL" ]; then
        echo "[SKIP] VITE_SIGNAL_URL not set in $WEB_ENV"
    elif [ "$WEB_URL" != "$CANONICAL" ]; then
        echo "[FAIL] Web .env endpoint MISMATCH"
        echo "  Expected: $CANONICAL"
        echo "  Found:    $WEB_URL"
        EXIT=1
    else
        echo "[OK]   Web .env:     $WEB_URL"
    fi
else
    echo "[SKIP] Web .env not found (OK if checking only native)"
fi

# --- Rust FFI default (informational) ---
SIGNALING_RS="$REPO_ROOT/native/shared/src/signaling.rs"
if [ -f "$SIGNALING_RS" ]; then
    if grep -q "$CANONICAL" "$SIGNALING_RS"; then
        echo "[OK]   Rust FFI doc reference matches"
    else
        # Not a failure — the Rust FFI takes cloudUrl as a parameter, the
        # doc comment may just be an example. Informational only.
        echo "[INFO] Rust FFI doc comment does not contain canonical URL (OK — passed at runtime)"
    fi
fi

echo ""
if [ $EXIT -eq 0 ]; then
    echo "=== All signaling endpoints match canonical: $CANONICAL ==="
else
    echo "=== DRIFT DETECTED — fix endpoints before tagging ==="
fi

exit $EXIT
