#!/usr/bin/env bash
set -euo pipefail

# C6 — Ensures @the9ines/localbolt-core uses exact version pin (no ^, ~, *, ranges).

PKG_JSON="web/package.json"

SPEC=$(node -e "
  const pkg = require('./$PKG_JSON');
  const v = (pkg.dependencies || {})['@the9ines/localbolt-core'];
  if (!v) { console.error('FAIL: @the9ines/localbolt-core not found in $PKG_JSON'); process.exit(1); }
  process.stdout.write(v);
")

if echo "$SPEC" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "PASS: @the9ines/localbolt-core pinned to exact version \"$SPEC\""
else
  echo "FAIL: @the9ines/localbolt-core has non-exact spec \"$SPEC\""
  echo "      Required format: X.Y.Z (no ^, ~, >=, *, latest, file:, git:, workspace:)"
  exit 1
fi
