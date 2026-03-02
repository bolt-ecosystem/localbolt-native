# Bolt Rendezvous

Canonical Rust rendezvous server for the Bolt Protocol ecosystem.

## What This Is

A lightweight WebSocket server that handles peer discovery and signal routing for Bolt-based applications. The rendezvous server is **infrastructure**, not a product.

## Role in the Ecosystem

- Routes opaque signaling payloads between peers.
- Groups peers into rooms based on IP heuristics (RFC 1918, CGNAT, link-local).
- Provides presence notifications (join, leave, peer list).

**Signaling only.** The rendezvous server handles coordination metadata (SDP
offers/answers, ICE candidates, hello/ack). File payload bytes never transit
the rendezvous server — all payload data flows directly between peers over
WebRTC DataChannel (P2P).

The rendezvous server is **untrusted**. It cannot observe file contents, encryption keys, or transfer metadata. All security guarantees come from Bolt-layer encryption.

## Run Locally

```bash
# From source
cargo run --release

# With explicit host/port
cargo run --release -- --host 127.0.0.1 --port 4000

# With environment variables
BOLT_SIGNAL_HOST=127.0.0.1 BOLT_SIGNAL_PORT=4000 cargo run --release
```

The server listens on `0.0.0.0:3001` by default.

## Run with Docker

```bash
# Build
docker build -t bolt-rendezvous .

# Run (default: 0.0.0.0:3001)
docker run -p 3001:3001 bolt-rendezvous

# Run with internet profile
docker run -p 3001:3001 -e BOLT_SIGNAL_PROFILE=internet bolt-rendezvous

# Override host/port
docker run -p 4000:4000 \
  -e BOLT_SIGNAL_HOST=0.0.0.0 \
  -e BOLT_SIGNAL_PORT=4000 \
  bolt-rendezvous
```

## Deploy (Fly.io / VPS)

For public deployment, use the `internet` profile:

```bash
# Fly.io
fly launch --image bolt-rendezvous:latest
fly secrets set BOLT_SIGNAL_PROFILE=internet

# VPS (systemd, Docker, etc.)
docker run -d --restart=unless-stopped \
  -p 3001:3001 \
  -e BOLT_SIGNAL_PROFILE=internet \
  --name bolt-rendezvous \
  bolt-rendezvous
```

TLS termination should be handled by a reverse proxy (Caddy, nginx, Fly.io proxy) — the server itself speaks plain WebSocket (`ws://`).

## Configuration

### Resolution Order

CLI arguments take highest priority, then environment variables, then profile defaults, then hardcoded defaults.

| Source | Host | Port | Log Level |
|--------|------|------|-----------|
| CLI (`--host`, `--port`) | Highest | Highest | — |
| Env (`BOLT_SIGNAL_HOST/PORT`) | High | High | — |
| `RUST_LOG` | — | — | Highest |
| Profile defaults | Fallback | Fallback | Fallback |
| Hardcoded | `0.0.0.0` | `3001` | `info` |

### Profiles (`BOLT_SIGNAL_PROFILE`)

| Variable | Value | Log Default | Notes |
|----------|-------|-------------|-------|
| *(unset)* | — | `info` | Same as `local` |
| `BOLT_SIGNAL_PROFILE` | `local` | `info` | LAN/dev — verbose logging |
| `BOLT_SIGNAL_PROFILE` | `internet` | `warn` | Public deployment — quieter |

`RUST_LOG` always overrides the profile log level when explicitly set.

### Trust Boundary Limits (all profiles)

These limits are enforced regardless of profile and cannot be overridden:

| Limit | Value |
|-------|-------|
| Max WebSocket message | 1 MiB |
| Max device name | 256 bytes |
| Max peer code | 16 bytes |
| Rate limit | 50 msg/sec per connection |
| Rate limit close threshold | 3 consecutive violations |

## Dependencies

None. Standalone Rust binary.

## Bundled By

- [localbolt](https://github.com/the9ines/localbolt) — via git subtree (offline mode)
- [localbolt-app](https://github.com/the9ines/localbolt-app) — via git subtree (offline mode)

## Not Bundled By

- [localbolt-v3](https://github.com/the9ines/localbolt-v3) — connects to hosted endpoint only

## License

MIT
