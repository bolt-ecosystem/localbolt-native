# LocalBolt Native — Product Requirements

**Status:** Current
**Product path:** macOS SwiftUI shell + Rust native bridge + bundled bolt-daemon sidecar

## Current State

LocalBolt Native is the installable macOS app for the Bolt ecosystem. It uses a
thin platform-native UI shell over shared Rust authority:

- SwiftUI owns macOS UI, drag-and-drop, progress display, and platform packaging.
- `native/shared` exposes the Rust C-ABI bridge consumed by Swift.
- `bolt-daemon` runs as a bundled sidecar and owns session orchestration.
- `bolt-core-sdk` remains the shared Rust authority for protocol, BTR, transfer
  state machines, and app runtime core.
- Browser users at `localbolt.app` interoperate with native users through the
  same Bolt protocol surfaces.

## Requirements

1. Keep native UI shells thin. Protocol, transfer, and daemon authority stay in
   Rust.
2. Keep macOS release packaging reproducible from `native/macos`.
3. Keep the daemon sidecar contract explicit and tested.
4. Keep product security claims honest: no product-facing verified-device claim
   until EA1 is reviewed, wire-frozen, specified, and implemented.
5. Future Linux, Windows, iOS, and Android shells must follow the same native
   wrapper pattern unless governance approves a different architecture.

## Non-Goals

- No browser wrapper as the native product path.
- No protocol fork inside the shell.
- No product-specific crypto implementation.
- No EA1 PAKE implementation until the external cryptographer/formal-methods
  review gate is complete.

## Open Work

- External review of EA1 PAKE v7 before wire-freeze.
- Native distribution polish: signing, notarization, auto-update strategy, and
  platform-specific packaging decisions.
- Future platform shell decisions after macOS remains stable.
