# LocalBolt Native — Roadmap

**Status:** Current

## Product Direction

LocalBolt Native uses shared Rust core services with thin platform-native
wrappers. The macOS app is the current shipping native shell:

- `native/macos` — SwiftUI app
- `native/shared` — Rust C-ABI bridge
- `signal` — vendored rendezvous subtree
- bundled `bolt-daemon` sidecar for session and transport orchestration

## Near-Term Work

1. Keep the macOS native shell buildable and test-covered.
2. Keep daemon/bridge contracts synced with `bolt-daemon`.
3. Keep public security wording aligned with the current trust model.
4. Prepare EA1 materials for external cryptographer/formal-methods review.

## Later Work

1. Signing and notarization.
2. Auto-update strategy.
3. Linux and Windows shell decisions.
4. Mobile shell decisions after the native architecture is stable.

## Locked Until Review

EA1 PAKE remains review-ready but not implementation-authorized. Do not add
wire changes, app trust badges, or verified-device behavior until the outside
review and wire-freeze gates are complete.
