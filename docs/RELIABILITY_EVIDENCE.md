# localbolt-app Reliability Verification Evidence

Captured: 2026-03-15
Context: Supervisor + Click-to-Connect reliability verification.

---

## 1. Existing Architecture (Verified Working)

| Component | Status | Tests |
|-----------|--------|-------|
| Embedded rendezvous server | ✅ Running (0.0.0.0:3001) | — |
| Daemon supervisor (DaemonManager) | ✅ Spawn/monitor/restart | 4 daemon tests |
| Watchdog state machine (5 states) | ✅ Full lifecycle | 18 tests (was 14 + 4 new) |
| Bounded backoff (1s/3s/10s, 3 retries) | ✅ Deterministic | 4 backoff tests |
| Terminal failure (Degraded) | ✅ After 3 retries | 2 tests |
| Manual restart from Degraded | ✅ UI button | 1 test |
| IPC bridge (NDJSON persistent) | ✅ Handshake + events | — |
| Click-to-connect nearby peers | ✅ selectPeer() | — |
| Peer discovery (DualSignaling) | ✅ LAN auto | 11 DM3 tests |

---

## 2. Hardening Applied

### Rendezvous panic guard (lib.rs)
Added `std::panic::catch_unwind()` around signaling server thread. Panic is caught and logged, does not crash the app.

---

## 3. Reconnect Stress Drill (4 New Tests)

| Test | Cycles | Result |
|------|--------|--------|
| `reconnect_stress_10_cycles` | 10 connect/disconnect/reconnect | PASS — no stuck state, retries reset each cycle |
| `rapid_crash_no_stable_window_degrades` | 4 rapid crashes | PASS — enters Degraded deterministically |
| `degraded_manual_restart_full_recovery` | Degraded → restart → ready | PASS |
| `no_stuck_state` | All state transitions | PASS — every state has valid exit |

---

## 4. Failure Mode Coverage

| Failure Mode | Behavior | Tested |
|-------------|----------|--------|
| Daemon crash | Auto-restart with 1s/3s/10s backoff | ✅ |
| Daemon spawn failure | Degraded immediately (no retry) | ✅ |
| 3 rapid crashes | Degraded + Retry button | ✅ |
| Version mismatch | Incompatible (terminal) | ✅ |
| Startup timeout (10s) | Triggers restart sequence | ✅ |
| Manual restart from Degraded | Resets to Starting | ✅ |
| 10 reconnect cycles | No stuck state | ✅ |
| Rendezvous thread panic | Caught, logged, app survives | ✅ (code) |

---

## 5. 30-Second Timeout Issue (Cross-Device bolt-ui)

The 30-second disconnect observed during cross-device bolt-ui testing is **NOT a localbolt-app issue**. It affects daemon-to-daemon rendezvous connections where the phase timeout covers the entire session lifecycle. localbolt-app uses a different architecture:
- Web UI manages WebRTC connections through the SDK
- Daemon acts as local answerer for IPC events only
- No phase timeout applies to web-initiated connections

This is a daemon architecture limitation that requires a focused N-STREAM timeout patch, not a localbolt-app fix.

---

## 6. Two-Device Operator Runbook

### Prerequisites
- Two devices on same LAN
- localbolt-app running on both (or localbolt-app + browser at localbolt.app)
- Both see "NEARBY" indicator in header

### Steps
1. Device A: observe peer list — Device B should appear automatically
2. Device A: click Device B in peer list
3. Device B: accept incoming connection request
4. Both: verify SAS code match
5. Device A: send file → Device B receives
6. Either: click Disconnect
7. Repeat from step 2 to test reconnect

### Expected Results
- Peer discovery: automatic, within 2-3 seconds
- Click-to-connect: immediate request visible on other device
- SAS verification: codes match
- Transfer: completes with integrity
- Disconnect/reconnect: clean, no stuck state

---

## 7. Test Results

```
Rust (src-tauri): 86 passed, 0 failed
Web (cbtr3):     10 passed, 0 failed
Total:           96 passed, 0 failed
```
