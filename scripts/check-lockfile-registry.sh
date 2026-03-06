#!/usr/bin/env bash
set -euo pipefail

# D5 — Ensures package-lock.json resolves @the9ines packages from registry.npmjs.org.
# Prevents lockfile drift back to npm.pkg.github.com after regeneration.

LOCKFILE="web/package-lock.json"

if [ ! -f "$LOCKFILE" ]; then
  echo "FAIL: $LOCKFILE not found"
  exit 1
fi

FAIL=0

# Check that no @the9ines package resolves from GitHub Packages
GH_RESOLVED=$(node -e "
  const lock = require('./$LOCKFILE');
  const packages = lock.packages || {};
  const bad = [];
  for (const [key, val] of Object.entries(packages)) {
    if (key.includes('@the9ines/') && val.resolved && val.resolved.includes('npm.pkg.github.com')) {
      bad.push(key + ' -> ' + val.resolved);
    }
  }
  if (bad.length) { console.log(bad.join('\n')); process.exit(1); }
" 2>&1) || {
  echo "FAIL: @the9ines packages resolved from npm.pkg.github.com in lockfile:"
  echo "$GH_RESOLVED"
  FAIL=1
}

# Check that @the9ines packages resolve from npmjs.org
NPMJS_COUNT=$(node -e "
  const lock = require('./$LOCKFILE');
  const packages = lock.packages || {};
  let count = 0;
  for (const [key, val] of Object.entries(packages)) {
    if (key.includes('@the9ines/') && val.resolved && val.resolved.includes('registry.npmjs.org')) {
      count++;
    }
  }
  console.log(count);
")

if [ "$FAIL" -eq 0 ]; then
  echo "PASS: $NPMJS_COUNT @the9ines package(s) resolved from registry.npmjs.org"
  echo "PASS: No @the9ines packages resolved from npm.pkg.github.com"
else
  echo ""
  echo "Lockfile registry guard failed. Regenerate lockfile with"
  echo "@the9ines:registry=https://registry.npmjs.org in web/.npmrc."
  exit 1
fi
