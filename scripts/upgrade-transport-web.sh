#!/usr/bin/env bash
set -euo pipefail

# Phase 4H — Upgrade @the9ines/bolt-transport-web to a new version.
# Usage: bash scripts/upgrade-transport-web.sh <version>
# Example: bash scripts/upgrade-transport-web.sh 0.2.0
#
# Does NOT auto-commit or auto-tag. Prepares the repo and proves gates locally.

REPO="localbolt-app"
PKG_JSON="web/package.json"
VERSION_FILE=".transport-web-version"
PKG="@the9ines/bolt-transport-web"

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>"
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

# Update version file
echo "$VERSION" > "$VERSION_FILE"
echo "Updated $VERSION_FILE"

# Clean install
echo "--- Clean install ---"
rm -rf web/node_modules
(cd web && npm install)

# Build
echo "--- Build ---"
(cd web && npm run build)
BUILD_RC=$?

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
echo "  Tests:     N/A (no test suite)"
echo "==============================="

if [ "$BUILD_RC" -ne 0 ]; then
  echo "FAIL: gates did not pass. Do not commit."
  exit 1
fi

echo "PASS: ready to commit. Review changes with 'git diff' before committing."
