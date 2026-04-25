# Manual Test Matrix — Connectivity Fix (LOCALBOLT-MACOS-NATIVE-CONNECTIVITY-FIX-1)

## What changed

- `LocalBoltApp.swift` line 84-90: `connection_accepted` handler no longer resets
  session when browser peer accepts without a `wsUrl`. Instead stays in `.connecting`
  and waits for inbound WebTransport session from the browser.

## Prerequisite

- Both devices on same LAN (same public IP for rendezvous room grouping)
- macOS native app built from this branch (`native/macos/build/LocalBolt.app`)
- Web app at `localbolt.app` (production) or local dev server

## Test Cases

### TC-1: Browser → Native (initiator: browser)

| Step | Action | Expected |
|------|--------|----------|
| 1 | Open native app on Mac. Note peer code. | Peer code shown, daemon starts |
| 2 | Open localbolt.app in browser on same LAN. | Browser discovers native peer |
| 3 | Browser initiates connection to native peer. | Native shows incoming request |
| 4 | Accept on native side. | Both show "Connected" (WT path) |
| 5 | Send a file from browser → native. | Transfer completes, file saved |

### TC-2: Native → Browser (initiator: native)

| Step | Action | Expected |
|------|--------|----------|
| 1 | Open native app and browser on same LAN. | Both discover each other |
| 2 | Native initiates connection to browser peer. | Browser shows incoming request |
| 3 | Accept on browser side. | Native stays in `.connecting`, then WT session detected → "Connected" |
| 4 | Send a file from native → browser. | Transfer completes, browser downloads file |

### TC-3: Native → Native (same LAN, direct WS)

| Step | Action | Expected |
|------|--------|----------|
| 1 | Two Macs on same LAN, both running native app. | Discover each other |
| 2 | Mac A initiates connection to Mac B. | Mac B shows incoming request |
| 3 | Accept on Mac B. | Both connected via direct WS (wsUrl present) |
| 4 | Send file both directions. | Transfers complete |
| **Note** | If this fails, check macOS firewall allows bolt-daemon on port 3001 | Separate issue from this fix |

### TC-4: Browser → Browser (WebRTC, regression check)

| Step | Action | Expected |
|------|--------|----------|
| 1 | Two browsers on same LAN at localbolt.app. | Discover each other |
| 2 | Initiate and accept connection. | Connected via WebRTC |
| 3 | Transfer file. | Completes normally |
| **Note** | This path is unaffected by the fix (no native code involved) | Regression check only |

### TC-5: Timeout behavior (no WT session)

| Step | Action | Expected |
|------|--------|----------|
| 1 | Native app accepts connection from browser. | Enters `.connecting` |
| 2 | Kill the browser before it connects via WT. | Native stays in `.connecting` |
| 3 | Observe UI after ~5 seconds. | "Slow connection" indicator shown |
| **Note** | No auto-reset currently. User can manually cancel. | Known behavior — not a regression |

## Architecture Matrix

| Pair | Transport | Status |
|------|-----------|--------|
| arm64 native ↔ browser | WebTransport | **PRIMARY target of this fix** |
| x86_64 native ↔ browser | WebTransport | Same code path — needs build |
| arm64 native ↔ arm64 native | Direct WS | Unaffected by fix |
| arm64 native ↔ x86_64 native | Direct WS | Unaffected by fix |
| browser ↔ browser | WebRTC | Unaffected by fix |
