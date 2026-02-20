# Bolt Rendezvous

Canonical Rust rendezvous server for the Bolt Protocol ecosystem.

## What This Is

A lightweight WebSocket server that handles peer discovery and signal routing for Bolt-based applications. The rendezvous server is **infrastructure**, not a product.

## Role in the Ecosystem

- Routes opaque signaling payloads between peers.
- Groups peers into rooms based on IP heuristics (RFC 1918, CGNAT, link-local).
- Provides presence notifications (join, leave, peer list).

The rendezvous server is **untrusted**. It cannot observe file contents, encryption keys, or transfer metadata. All security guarantees come from Bolt-layer encryption.

## Dependencies

None. Standalone Rust binary.

## Bundled By

- [localbolt](https://github.com/the9ines/localbolt) — via git subtree (offline mode)
- [localbolt-app](https://github.com/the9ines/localbolt-app) — via git subtree (offline mode)

## Not Bundled By

- [localbolt-v3](https://github.com/the9ines/localbolt-v3) — connects to hosted endpoint only

## Deployment

- **Local**: `ws://<ip>:3001` for LAN-only operation
- **Hosted**: Fly.io or similar for cloud rendezvous (`wss://`)

## License

MIT
