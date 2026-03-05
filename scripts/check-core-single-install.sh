#!/usr/bin/env bash
set -euo pipefail

# C6 — Ensures exactly one instance of @the9ines/localbolt-core is installed.

PKG="@the9ines/localbolt-core"

JSON=$(npm ls "$PKG" --json --prefix web 2>/dev/null || true)

VERSIONS=$(node -e "
  const data = JSON.parse(process.argv[1]);
  const versions = [];
  function walk(obj) {
    if (!obj || typeof obj !== 'object') return;
    if (obj.dependencies) {
      for (const [name, dep] of Object.entries(obj.dependencies)) {
        if (name === '$PKG' && dep.version) versions.push(dep.version);
        walk(dep);
      }
    }
  }
  walk(data);
  console.log(JSON.stringify(versions));
" "$JSON")

COUNT=$(node -e "console.log(JSON.parse(process.argv[1]).length)" "$VERSIONS")

if [ "$COUNT" -eq 0 ]; then
  echo "FAIL: $PKG not found in node_modules"
  exit 1
elif [ "$COUNT" -gt 1 ]; then
  echo "FAIL: $COUNT instances of $PKG found (expected 1)"
  echo "      Versions: $VERSIONS"
  exit 1
else
  echo "PASS: single instance of $PKG installed"
fi
