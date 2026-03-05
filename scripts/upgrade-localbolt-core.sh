#!/usr/bin/env bash
set -euo pipefail

# C6 — Upgrade @the9ines/localbolt-core to a new version.
#
# Modes:
#   check:  bash scripts/upgrade-localbolt-core.sh --check
#           Validates version pin, single install, lockfile consistency. No writes.
#
#   upgrade: bash scripts/upgrade-localbolt-core.sh <version>
#           Updates package.json, reinstalls, runs build+test gates.
#           Does NOT auto-commit or auto-tag.

REPO="localbolt-app"
PKG_JSON="web/package.json"
PKG="@the9ines/localbolt-core"

# ── Check mode ──────────────────────────────────────────────────────
if [ "${1:-}" = "--check" ]; then
  echo "=== $PKG check mode ==="
  EXIT=0

  # 1) Version pin (exact semver, no ranges)
  SPEC=$(node -e "
    const pkg = require('./$PKG_JSON');
    const v = (pkg.dependencies || {})['$PKG'];
    if (!v) { console.error('FAIL: $PKG not found in $PKG_JSON'); process.exit(1); }
    process.stdout.write(v);
  ")
  if echo "$SPEC" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "PASS: version pin exact ($SPEC)"
  else
    echo "FAIL: non-exact version spec \"$SPEC\""
    EXIT=1
  fi

  # 2) Lockfile consistency
  LOCK_VER=$(node -e "
    const lock = require('./web/package-lock.json');
    const entry = (lock.packages || {})['node_modules/$PKG'];
    if (!entry) { console.error('FAIL: $PKG not in lockfile'); process.exit(1); }
    process.stdout.write(entry.version);
  ")
  if [ "$SPEC" = "$LOCK_VER" ]; then
    echo "PASS: lockfile version matches ($LOCK_VER)"
  else
    echo "FAIL: package.json=$SPEC lockfile=$LOCK_VER"
    EXIT=1
  fi

  # 3) Single install (reuse guard script if available)
  if [ -f scripts/check-core-single-install.sh ]; then
    if bash scripts/check-core-single-install.sh; then
      true  # already prints PASS
    else
      EXIT=1
    fi
  fi

  exit "$EXIT"
fi

# ── Upgrade mode ────────────────────────────────────────────────────
if [ $# -ne 1 ]; then
  echo "Usage:"
  echo "  $0 --check          Validate current state (no writes)"
  echo "  $0 <version>        Upgrade to specified version"
  echo "Example: $0 0.2.0"
  exit 1
fi

VERSION="$1"

if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "FAIL: \"$VERSION\" is not strict semver (required: X.Y.Z)"
  exit 1
fi

echo "=== Upgrading $PKG to $VERSION in $REPO ==="

# Update package.json
node -e "
  const fs = require('fs');
  const pkg = JSON.parse(fs.readFileSync('$PKG_JSON', 'utf8'));
  pkg.dependencies['$PKG'] = '$VERSION';
  fs.writeFileSync('$PKG_JSON', JSON.stringify(pkg, null, 2) + '\n');
  console.log('Updated $PKG_JSON');
"

# Clean install
echo "--- Clean install ---"
rm -rf web/node_modules
(cd web && npm install)
INSTALL_RC=$?
if [ "$INSTALL_RC" -ne 0 ]; then
  echo "FAIL: npm install failed (exit $INSTALL_RC). Do not commit."
  exit 1
fi

# Build
echo "--- Build ---"
(cd web && npm run build)
BUILD_RC=$?

# Tests
echo "--- Tests ---"
(cd web && npx vitest run)
TEST_RC=$?

echo ""
echo "==============================="
echo "  Upgrade Report"
echo "==============================="
echo "  Repo:      $REPO"
echo "  Package:   $PKG"
echo "  Version:   $VERSION"
echo "  Install:   npm install → DONE"
if [ "$BUILD_RC" -eq 0 ]; then
  echo "  Build:     npm run build → PASS"
else
  echo "  Build:     npm run build → FAIL"
fi
if [ "$TEST_RC" -eq 0 ]; then
  echo "  Tests:     vitest run → PASS"
else
  echo "  Tests:     vitest run → FAIL"
fi
echo "==============================="

if [ "$BUILD_RC" -ne 0 ] || [ "$TEST_RC" -ne 0 ]; then
  echo "FAIL: gates did not pass. Do not commit."
  exit 1
fi

echo "PASS: ready to commit. Review changes with 'git diff' before committing."
