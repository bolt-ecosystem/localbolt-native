#!/usr/bin/env bash
set -euo pipefail

# Phase 4G drift guard — prevents reintroduction of duplicated transport files
# and deep imports from @the9ines/bolt-transport-web.

SRC_DIR="${1:-web/src}"
EXIT=0

# 1) Check for deep imports
echo "--- Deep import check ---"
if grep -rn '@the9ines/bolt-transport-web/' "$SRC_DIR" --include='*.ts' --include='*.tsx'; then
  echo "FAIL: deep imports detected (use barrel import only)"
  EXIT=1
else
  echo "PASS: no deep imports"
fi

# 2) Check for reintroduced duplicate source files
echo "--- Duplicate source file check ---"
DUPES=(
  services/webrtc/WebRTCService.ts
  services/signaling/DualSignaling.ts
  services/signaling/WebSocketSignaling.ts
  services/signaling/SignalingProvider.ts
  services/signaling/device-detect.ts
  services/signaling/index.ts
  lib/crypto-utils.ts
  lib/platform-utils.ts
  lib/sanitize.ts
  state/store.ts
  types/webrtc-errors.ts
  ui/icons.ts
  ui/toast.ts
  components/connection-status.ts
  components/device-discovery.ts
  components/file-upload.ts
  components/transfer-progress.ts
)

for f in "${DUPES[@]}"; do
  if [ -f "$SRC_DIR/$f" ]; then
    echo "FAIL: duplicate source file exists: $SRC_DIR/$f"
    EXIT=1
  fi
done

if [ "$EXIT" -eq 0 ]; then
  echo "PASS: no duplicate source files"
fi

exit "$EXIT"
