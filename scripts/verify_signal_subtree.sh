#!/usr/bin/env bash
# verify_signal_subtree.sh — Deterministic drift guard for vendored signal/ subtree.
#
# Compares every tracked file under localbolt/signal/ against canonical
# bolt-rendezvous at the pinned upstream tag (SIGNAL_SUBTREE_PIN).
#
# One-directional: detects local modifications/deletions and local extra
# files. Does NOT fail on upstream-only additions (that is staleness
# detection, not drift prevention).
#
# Exit 0: all tracked files match canonical.
# Exit 1: drift detected or precondition failure.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PIN_FILE="$REPO_ROOT/SIGNAL_SUBTREE_PIN"
CANONICAL_DIR="${CANONICAL_DIR:-$REPO_ROOT/../canonical-rendezvous}"

# ── 1. Read pin ──────────────────────────────────────────────────
if [[ ! -f "$PIN_FILE" ]]; then
    echo "FAIL: SIGNAL_SUBTREE_PIN not found at $PIN_FILE"
    exit 1
fi

# shellcheck source=/dev/null
source "$PIN_FILE"

if [[ -z "${UPSTREAM_REPO:-}" || -z "${UPSTREAM_TAG:-}" ]]; then
    echo "FAIL: SIGNAL_SUBTREE_PIN must define UPSTREAM_REPO and UPSTREAM_TAG"
    exit 1
fi

echo "Pin: $UPSTREAM_REPO @ $UPSTREAM_TAG"

# ── 2. Verify canonical checkout ─────────────────────────────────
if [[ ! -d "$CANONICAL_DIR" ]]; then
    echo "FAIL: canonical checkout not found at $CANONICAL_DIR"
    echo "  Local: git -C ../bolt-rendezvous worktree add ../canonical-rendezvous $UPSTREAM_TAG"
    echo "  CI:    actions/checkout with repository=$UPSTREAM_REPO ref=$UPSTREAM_TAG path=canonical-rendezvous and CANONICAL_DIR set"
    exit 1
fi

if [[ ! -f "$CANONICAL_DIR/Cargo.toml" ]]; then
    echo "FAIL: canonical checkout at $CANONICAL_DIR does not look like bolt-rendezvous (missing Cargo.toml)"
    exit 1
fi

# ── 3. Build manifests from tracked files ────────────────────────
LOCAL_MANIFEST="$(mktemp)"
CANONICAL_MANIFEST="$(mktemp)"
trap 'rm -f "$LOCAL_MANIFEST" "$CANONICAL_MANIFEST"' EXIT

cd "$REPO_ROOT"

TRACKED_FILES="$(git ls-files signal)"
if [[ -z "$TRACKED_FILES" ]]; then
    echo "FAIL: no tracked files under signal/"
    exit 1
fi

ERRORS=0

while IFS= read -r f; do
    rel="${f#signal/}"

    canonical_file="$CANONICAL_DIR/$rel"
    if [[ ! -f "$canonical_file" ]]; then
        echo "DRIFT: $rel exists in localbolt/signal/ but not in canonical at $UPSTREAM_TAG"
        ERRORS=$((ERRORS + 1))
        continue
    fi

    local_hash="$(shasum -a 256 "$f" | awk '{print $1}')"
    canonical_hash="$(shasum -a 256 "$canonical_file" | awk '{print $1}')"

    echo "$local_hash  $rel" >> "$LOCAL_MANIFEST"
    echo "$canonical_hash  $rel" >> "$CANONICAL_MANIFEST"
done <<< "$TRACKED_FILES"

# ── 4. Compare manifests ─────────────────────────────────────────
if [[ $ERRORS -gt 0 ]]; then
    echo ""
    echo "FAIL: $ERRORS file(s) exist locally but not in canonical"
    exit 1
fi

if ! diff -u "$CANONICAL_MANIFEST" "$LOCAL_MANIFEST" > /dev/null 2>&1; then
    echo ""
    echo "DRIFT DETECTED — content hash mismatches:"
    diff -u "$CANONICAL_MANIFEST" "$LOCAL_MANIFEST" || true
    echo ""
    echo "FAIL: signal/ has drifted from $UPSTREAM_REPO @ $UPSTREAM_TAG"
    exit 1
fi

FILE_COUNT="$(wc -l < "$LOCAL_MANIFEST" | tr -d ' ')"
echo "PASS: $FILE_COUNT tracked files match canonical ($UPSTREAM_TAG)"
exit 0
