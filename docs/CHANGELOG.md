# LocalBolt Native Changelog

## Current Native Line

- macOS app uses SwiftUI with a Rust C-ABI bridge in `native/shared`.
- Bundled `bolt-daemon` sidecar owns session orchestration and transport.
- Native distribution artifacts are produced from `native/macos`.
- Product trust wording is intentionally honest: current session approval is not
  marketed as verified-device identity.

## EA Trust Gate

- Product-facing verified-device behavior is locked until EA1 completes outside
  cryptographer/formal-methods review, wire-freeze, spec update, and
  implementation authorization.
- The current app may describe encrypted transfers and user-approved sessions.
  It must not claim MITM-proof verified pairing.

## Repository Cleanup

- Removed retired desktop shell source from the active tree.
- Removed stale web-wrapper source from the native app repo.
- Current contributor path is Rust core with native wrappers.
