# State

Current state of the bolt-rendezvous repository.

## Current Version

**Tag:** `rendezvous-v0.2.0-sig-2a-packaging`
**Commit:** `a6a63ae`
**Branch:** `main`
**Crate:** `bolt-rendezvous` v0.1.1
**Subcrate:** `bolt-rendezvous-protocol` v0.1.0

## Purpose

WebSocket signaling server for the Bolt protocol. Groups peers by IP
address for local-network device discovery and relays WebRTC signaling
messages between them.

## Contents

| Item | Status |
|------|--------|
| Signaling server (`src/lib.rs`) | Complete |
| Connection handler (`src/server.rs`) | Complete (with trust boundary limits) |
| Protocol types (`src/protocol.rs`) | Complete (re-exports from `bolt-rendezvous-protocol`) |
| Protocol subcrate (`protocol/`) | **Complete** (Phase A2 — canonical types, 16 tests) |
| Room manager (`src/room.rs`) | Complete |
| Trust boundary enforcement | **Complete** (Phase 6A.4) |
| Docker packaging | **Complete** (SIG-2A — multi-stage Dockerfile) |
| Env-based profiles | **Complete** (SIG-2A — BOLT_SIGNAL_PROFILE local/internet) |

## Configuration

| Variable | Values | Default |
|----------|--------|---------|
| `BOLT_SIGNAL_PROFILE` | `local`, `internet` | *(unset = local behavior)* |
| `BOLT_SIGNAL_HOST` | IP address | `0.0.0.0` |
| `BOLT_SIGNAL_PORT` | port number | `3001` |
| `RUST_LOG` | tracing filter | profile-dependent (`info` or `warn`) |

Resolution: CLI args > env vars > profile defaults > hardcoded defaults.

## Trust Boundary Limits

| Limit | Value | Enforcement |
|-------|-------|-------------|
| Message size | 1 MiB | WebSocketConfig (protocol) + validate_message_size (app) |
| Device name | 256 bytes | validate_device_name() |
| Peer code | 16 chars | validate_peer_code() / validate_signal_target() |
| Rate limit | 50 msg/sec | RateLimit struct (per-connection) |
| Rate close | 3 consecutive | Fail-closed: socket terminated |
| Binary frames | Rejected | Explicit rejection in both loops |

## Test Summary

- 45 unit tests (7 protocol + 21 server trust boundary + 17 room lifecycle)
- 1 doc-test
- Protocol subcrate: 16 tests (8 wire-compat + 5 deser + 1 DeviceType + 2 Clone)
- Total: 62

## Phase Status

| Phase | Description | Status |
|-------|-------------|--------|
| Initial | Signaling server skeleton | Complete |
| Phase 4B | Signaling-only clarification | Complete |
| Phase 6A.4 | Trust boundary hardening | **Complete** |
| Phase 8B.2 | Room/peer lifecycle test coverage | **Complete** (17 tests: add/remove/find, broadcast, concurrent edge, isolation) |
| Phase A2 | Signaling type deduplication | **Complete** (protocol subcrate extracted, consumed by bolt-daemon via git tag dep) |
| Phase SIG-2A | Canonical packaging + profiles | **Complete** (Dockerfile, env-based profiles, subtree compat verified) |
