# State

Current state of the bolt-rendezvous repository.

## Current Version

**Tag:** `rendezvous-v0.1.1-room-lifecycle-tests`
**Commit:** `af764f3`
**Branch:** `main`
**Crate:** `bolt-rendezvous` v0.1.1

## Purpose

WebSocket signaling server for the Bolt protocol. Groups peers by IP
address for local-network device discovery and relays WebRTC signaling
messages between them.

## Contents

| Item | Status |
|------|--------|
| Signaling server (`src/lib.rs`) | Complete |
| Connection handler (`src/server.rs`) | Complete (with trust boundary limits) |
| Protocol types (`src/protocol.rs`) | Complete |
| Room manager (`src/room.rs`) | Complete |
| Trust boundary enforcement | **Complete** (Phase 6A.4) |

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
- Total: 46

## Phase Status

| Phase | Description | Status |
|-------|-------------|--------|
| Initial | Signaling server skeleton | Complete |
| Phase 4B | Signaling-only clarification | Complete |
| Phase 6A.4 | Trust boundary hardening | **Complete** |
| Phase 8B.2 | Room/peer lifecycle test coverage | **Complete** (17 tests: add/remove/find, broadcast, concurrent edge, isolation) |
