# Changelog

All notable changes to bolt-rendezvous are documented here. Newest first.

## [rendezvous-v0.2.0-sig-2a-packaging] - 2026-02-24

Phase SIG-2A: Canonicalize Signal Server Packaging.

Adds Docker packaging and env-based profiles for local vs internet
deployment. Default behavior preserved when profile is unset.
Subtree consumer compatibility verified (localbolt, localbolt-app).

### Added
- `Dockerfile` — multi-stage build (rust:1.84-slim-bookworm builder,
  debian:bookworm-slim runtime). Non-root user. Builds `--release --locked`.
- `.dockerignore` — excludes target/, .git/, docs/.
- `BOLT_SIGNAL_PROFILE` env var — `local` (info log) or `internet` (warn log).
  Unset preserves pre-SIG-2A behavior.
- `BOLT_SIGNAL_HOST` / `BOLT_SIGNAL_PORT` env vars — override bind address.
  CLI args (`--host`, `--port`) take highest priority.
- README sections: "Run Locally", "Run with Docker", "Deploy (Fly.io / VPS)",
  "Configuration" with resolution order and profile tables.

### Changed
- `src/main.rs` — env var resolution (CLI > env > profile > hardcoded).
  Profile log level default. Warning on unknown profile value.
- `src/room.rs` — cargo fmt whitespace only (pre-existing).

### Verification
- localbolt subtree: fmt PASS, clippy PASS, 46 tests PASS.
- localbolt-app subtree: fmt PASS, clippy PASS, 46 tests PASS.
- No wire format changes. No protocol type changes.

### Tests
- Unchanged: 45 unit + 16 protocol + 1 doc-test = 62 total.

**Commit:** `f7e15f7` (feature), `a6a63ae` (merge to main)

## [rendezvous-protocol-v0.1.0] - 2026-02-24

Phase A2: Signaling Type Deduplication.

Extracts canonical protocol types (`ClientMessage`, `ServerMessage`,
`PeerData`, `DeviceType`) into `bolt-rendezvous-protocol` v0.1.0
subcrate (`protocol/`). Replaces manual `Clone` and `Deserialize`
impls in `room.rs` with derived traits. Both `Serialize` and
`Deserialize` derived on all types, enabling consumption by
both server (bolt-rendezvous) and client (bolt-daemon).

### Added
- `protocol/Cargo.toml` — `bolt-rendezvous-protocol` v0.1.0 crate.
  Dependencies: `serde` + `serde_json` only.
- `protocol/src/lib.rs` — canonical types with `Debug`, `Clone`,
  `Serialize`, `Deserialize` derives. 16 tests (8 wire-compat
  via `serde_json::Value` equality, 5 deser roundtrip, 1 DeviceType
  variants, 2 Clone).
- `protocol/.gitignore` — build artifacts.

### Changed
- `Cargo.toml` — added path dependency on `bolt-rendezvous-protocol`.
- `src/protocol.rs` — replaced type definitions with
  `pub use bolt_rendezvous_protocol::*;`.
- `src/room.rs` — removed manual `Deserialize` and `Clone` impls
  for `ServerMessage` (now derived).

### Consumers
- bolt-rendezvous: path dep (same repo).
- bolt-daemon: git dep pinned to `rendezvous-protocol-v0.1.0` tag.

### Tests
- Protocol subcrate: 16 tests.
- Main crate: 45 unit + 1 doc-test = 46 (unchanged).
- Total: 62.

**Commit:** `68befd7` (feature), `121145c` (merge to main)

## [rendezvous-v0.1.1-room-lifecycle-tests] - 2026-02-23

Phase 8B.2: Room/peer lifecycle unit coverage.

Adds 17 unit tests for RoomManager in `room.rs`. No production code
changes. Tests cover add_peer (insertion, return value, duplicate
rejection, cross-room same code, PeerJoined broadcast), remove_peer
(removal, empty-room cleanup, PeerLeft broadcast, nonexistent peer/room),
find_peer (found, absent, cross-room scan), concurrent edge simulation
(A disconnect doesn't affect B, multi-room isolation), and invalid
room access (nonexistent room returns empty, empty manager counts).

### Added
- 17 unit tests in `room::tests` module:
  - `add_peer_inserts_into_correct_room`
  - `add_peer_returns_existing_peers_before_insert`
  - `add_peer_rejects_duplicate_peer_code`
  - `add_peer_same_code_different_rooms_allowed`
  - `add_peer_broadcasts_peer_joined_to_existing`
  - `remove_peer_removes_from_room`
  - `remove_peer_cleans_up_empty_room`
  - `remove_peer_broadcasts_peer_left`
  - `remove_peer_nonexistent_does_not_panic`
  - `find_peer_returns_sender_for_existing_peer`
  - `find_peer_returns_none_for_absent_peer`
  - `find_peer_works_across_rooms`
  - `peer_a_disconnect_does_not_affect_peer_b`
  - `multi_room_isolation`
  - `get_room_peers_nonexistent_returns_empty`
  - `empty_manager_counts_are_zero`
  - `get_room_peers_returns_public_data`

### Changed
- Version bumped from `0.1.0` to `0.1.1` (tests only, no API change).

### Tests
- 45 unit tests + 1 doc-test = 46 total (was 28 + 1 = 29).

## [rendezvous-v0.0.5-trust-boundary] - 2026-02-23

### Added
- Trust boundary limits for all untrusted input:
  - `MAX_MESSAGE_BYTES` (1 MiB) — WebSocket message size cap
  - `MAX_DEVICE_NAME_BYTES` (256) — `Register.device_name` field cap
  - `MAX_PEER_CODE_BYTES` (16) — `Register.peer_code` and `Signal.to` cap
  - `RATE_LIMIT_PER_SECOND` (50) — per-connection message rate
  - `RATE_LIMIT_CLOSE_THRESHOLD` (3) — consecutive violations before socket close
- Protocol-level enforcement via `WebSocketConfig.max_message_size` and
  `max_frame_size` (first-line defense at tungstenite layer).
- Pure validation helpers: `validate_message_size()`, `validate_device_name()`,
  `validate_signal_target()`.
- `RateLimit` struct with fail-closed behavior (closes socket after 3
  consecutive violations). Applied in both registration and message loops.
- Binary frame rejection in both registration and message loops (signaling
  is text-only).
- 21 new unit tests for validation helpers, rate limiter (with tokio
  `time::pause`/`advance`), constants sanity, and WebSocketConfig verification.
- `tokio` `test-util` dev-dependency for deterministic time control in tests.

## [rendezvous-v0.0.4-docs] - 2026-02-22

### Added
- Documentation sync (Phase 4B signaling-only clarification).

## [rendezvous-v0.0.3-ci] - 2026-02-22

### Added
- Rust CI workflow (fmt, clippy, test).

## [rendezvous-v0.0.2-naming] - 2026-02-22

### Changed
- Renamed crate and binary to `bolt-rendezvous`.

## [rendezvous-v0.0.1] - 2026-02-21

### Added
- Initial WebSocket signaling server.
- IP-based room grouping for local-network device discovery.
- WebRTC signaling relay between peers in the same room.
- Peer registration, signal forwarding, and disconnect cleanup.
- Private IP detection (RFC 1918, CGNAT, IPv6 ULA/link-local).
- 7 protocol tests (serde roundtrip).
