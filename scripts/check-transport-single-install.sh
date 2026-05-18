#!/usr/bin/env bash
set -euo pipefail

# Phase 4H — Ensures exactly one instance of @the9ines/bolt-transport-web is installed
# and its version matches .transport-web-version.

VERSION_FILE=".transport-web-version"
PKG="@the9ines/bolt-transport-web"

if [ ! -f "$VERSION_FILE" ]; then
  echo "FAIL: $VERSION_FILE not found"
  exit 1
fi

EXPECTED=$(tr -d '[:space:]' < "$VERSION_FILE")

VERSIONS=$(node -e "
  const fs = require('fs');
  const path = require('path');
  const root = path.join(process.cwd(), 'web', 'node_modules');
  const versions = [];
  function walk(dir) {
    if (!fs.existsSync(dir)) return;
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const full = path.join(dir, entry.name);
      if (entry.name === '.bin') continue;
      if (entry.name === '@the9ines') {
        const pkg = path.join(full, 'bolt-transport-web', 'package.json');
        if (fs.existsSync(pkg)) versions.push(JSON.parse(fs.readFileSync(pkg, 'utf8')).version);
      }
      walk(path.join(full, 'node_modules'));
    }
  }
  walk(root);
  console.log(JSON.stringify(versions));
")

COUNT=$(node -e "console.log(JSON.parse(process.argv[1]).length)" "$VERSIONS")
INSTALLED=$(node -e "const v = JSON.parse(process.argv[1]); if (v.length > 0) console.log(v[0]); else console.log('NONE')" "$VERSIONS")

EXIT=0

if [ "$COUNT" -eq 0 ]; then
  echo "FAIL: $PKG not found in node_modules"
  EXIT=1
elif [ "$COUNT" -gt 1 ]; then
  echo "FAIL: $COUNT instances of $PKG found (expected 1)"
  echo "      Versions: $VERSIONS"
  EXIT=1
else
  echo "PASS: single instance of $PKG installed"
fi

if [ "$INSTALLED" != "$EXPECTED" ]; then
  echo "FAIL: installed version \"$INSTALLED\" != expected \"$EXPECTED\" (from $VERSION_FILE)"
  EXIT=1
else
  echo "PASS: version $INSTALLED matches $VERSION_FILE"
fi

exit "$EXIT"
