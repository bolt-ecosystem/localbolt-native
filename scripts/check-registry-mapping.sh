#!/usr/bin/env bash
set -euo pipefail

# D5 — Ensures @the9ines scope resolves from registry.npmjs.org (PAT-free path).
# Prevents regression to npm.pkg.github.com which requires PAT for all installs.

NPMRC="web/.npmrc"

if [ ! -f "$NPMRC" ]; then
  echo "FAIL: $NPMRC not found"
  exit 1
fi

FAIL=0

# Must contain the npmjs.org registry mapping
if grep -q '@the9ines:registry=https://registry.npmjs.org' "$NPMRC"; then
  echo "PASS: $NPMRC maps @the9ines to registry.npmjs.org"
else
  echo "FAIL: $NPMRC does not map @the9ines to registry.npmjs.org"
  echo "      Expected: @the9ines:registry=https://registry.npmjs.org"
  FAIL=1
fi

# Must NOT contain GitHub Packages registry
if grep -q 'npm.pkg.github.com' "$NPMRC"; then
  echo "FAIL: $NPMRC still references npm.pkg.github.com (PAT-required)"
  FAIL=1
fi

# Must NOT contain _authToken references (PAT dependency)
if grep -q '_authToken' "$NPMRC"; then
  echo "FAIL: $NPMRC contains _authToken reference (PAT dependency)"
  FAIL=1
fi

if [ "$FAIL" -ne 0 ]; then
  echo ""
  echo "Registry mapping guard failed. @the9ines packages must resolve from"
  echo "registry.npmjs.org without PAT. See D-STREAM-1 D4/D5 governance."
  exit 1
fi
