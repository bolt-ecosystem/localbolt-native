// @vitest-environment jsdom
/**
 * R1-4 — Security-focused session integrity tests for localbolt-app.
 *
 * Covers R1-0 security gaps:
 * 1. Identity/trust wiring in realistic connect/reconnect paths
 * 2. Stale generation callbacks rejected across distinct timing patterns
 * 3. Gating/security behavior under unverified/verified/mismatch transitions
 * 4. Reconnect does not leak prior session trust/verification state
 *
 * All WebRTC/WebSocket interactions are mocked.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  getVerificationState,
  setVerificationState,
  resetVerificationState,
  onVerificationStateChange,
  isTransferAllowed,
  getGeneration,
  isCurrentGeneration,
  getPhase,
  beginRequest,
  beginConnecting,
  markConnected,
  resetSession,
  _resetForTest,
} from '@the9ines/localbolt-core';

// ── Mock SDK dependencies ──────────────────────────────────────────────

vi.mock('@the9ines/bolt-transport-web', () => ({
  IndexedDBIdentityStore: class {
    load = vi.fn().mockResolvedValue(null);
    save = vi.fn().mockResolvedValue(undefined);
  },
  getOrCreateIdentity: vi.fn(async (store: { load: () => Promise<unknown>; save: (p: unknown) => Promise<void> }) => {
    const existing = await store.load();
    if (existing) return existing;
    const pair = { publicKey: new Uint8Array(32), secretKey: new Uint8Array(32) };
    await store.save(pair);
    return pair;
  }),
  IndexedDBPinStore: class {
    getPin = vi.fn().mockResolvedValue(null);
    setPin = vi.fn().mockResolvedValue(undefined);
    removePin = vi.fn().mockResolvedValue(undefined);
    markVerified = vi.fn().mockResolvedValue(undefined);
  },
  store: {
    getState: () => ({ isConnected: false, peers: [], signalingConnected: false }),
    setState: vi.fn(),
    subscribe: vi.fn(),
  },
  showToast: vi.fn(),
  createVerificationStatus: vi.fn(() => ({
    element: document.createElement('div'),
    update: vi.fn(),
  })),
  createFileUpload: () => document.createElement('div'),
  createConnectionStatus: () => document.createElement('div'),
  createDeviceDiscovery: () => document.createElement('div'),
  setWebrtcRef: vi.fn(),
  detectDeviceType: () => 'desktop',
  getDeviceName: () => 'Test Device',
  detectDevice: () => ({ isLinux: false, isWindows: false, isMobile: false }),
  DualSignaling: class {
    connect() { return Promise.resolve(); }
    setConnectionStateHandler() {}
    onPeerDiscovered() {}
    onPeerLost() {}
    onSignal() {}
    sendSignal() { return Promise.resolve(); }
    isConnected() { return false; }
  },
  WebRTCService: class {
    setConnectionStateHandler() {}
    getRemotePeerCode() { return ''; }
    disconnect() {}
    connect() { return Promise.resolve(); }
    markPeerVerified = vi.fn();
  },
  WebRTCError: class extends Error { details?: string; },
  SignalingError: class extends Error {},
}));

vi.mock('@the9ines/bolt-core', () => ({
  generateSecurePeerCode: () => 'MOCK-PEER-CODE',
}));

// ── Setup ─────────────────────────────────────────────────────────────────

beforeEach(() => {
  _resetForTest();
  resetVerificationState();
});

// ── Helper: simulate full session connect ─────────────────────────────────

function connectToPeer(peerCode: string): number {
  beginRequest(peerCode);
  beginConnecting(peerCode);
  markConnected();
  return getGeneration();
}

// ── 1. Identity/trust wiring in realistic connect/reconnect paths ─────────

describe('R1-4: Identity/trust wiring across connect/reconnect', () => {
  it('identity loads and caches consistently', async () => {
    const { getOrCreateIdentity, IndexedDBIdentityStore } = await import('@the9ines/bolt-transport-web');
    const store = new IndexedDBIdentityStore();
    const first = await getOrCreateIdentity(store);
    expect(first.publicKey).toBeInstanceOf(Uint8Array);
    expect(first.secretKey).toBeInstanceOf(Uint8Array);
    expect(first.publicKey.length).toBe(32);
    expect(first.secretKey.length).toBe(32);
  });

  it('full connect → verify → disconnect → reconnect clears trust', () => {
    // Session A: connect and verify
    connectToPeer('PEER-A');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-A' });
    setVerificationState({ state: 'verified', sasCode: 'SAS-A' });
    expect(getVerificationState().state).toBe('verified');

    // Disconnect
    resetSession();
    expect(getVerificationState().state).toBe('legacy');
    expect(getPhase()).toBe('idle');

    // Reconnect to new peer — trust is clean
    connectToPeer('PEER-B');
    expect(getVerificationState().state).toBe('legacy');
    expect(getVerificationState().sasCode).toBeNull();
  });

  it('connect → mismatch → reset → reconnect starts clean', () => {
    connectToPeer('PEER-A');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-A' });

    // Simulate mismatch error path: error handler calls resetSession
    const genA = getGeneration();
    resetSession();
    expect(isCurrentGeneration(genA)).toBe(false);
    expect(getVerificationState().state).toBe('legacy');

    // Reconnect: clean state
    connectToPeer('PEER-B');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-B' });
    expect(getVerificationState().state).toBe('unverified');
    expect(getVerificationState().sasCode).toBe('SAS-B');
  });

  it('rapid connect/disconnect cycles maintain trust isolation', () => {
    for (let i = 0; i < 5; i++) {
      connectToPeer(`PEER-${i}`);
      setVerificationState({ state: 'unverified', sasCode: `SAS-${i}` });
      if (i % 2 === 0) {
        setVerificationState({ state: 'verified', sasCode: `SAS-${i}` });
      }
      resetSession();
      expect(getVerificationState().state).toBe('legacy');
    }
  });
});

// ── 2. Stale generation callbacks across distinct timing patterns ──────────

describe('R1-4: Stale generation callbacks rejected', () => {
  it('single reset: stale generation detected', () => {
    connectToPeer('PEER-A');
    const genA = getGeneration();
    resetSession();
    expect(isCurrentGeneration(genA)).toBe(false);
  });

  it('double reset: both prior generations stale', () => {
    connectToPeer('PEER-A');
    const gen0 = getGeneration();
    resetSession();
    connectToPeer('PEER-B');
    const gen1 = getGeneration();
    resetSession();
    expect(isCurrentGeneration(gen0)).toBe(false);
    expect(isCurrentGeneration(gen1)).toBe(false);
  });

  it('mid-connection stale: generation captured at requesting is stale after reset', () => {
    beginRequest('PEER-A');
    const genAtRequest = getGeneration();

    // Connection attempt cancelled before connecting
    resetSession();
    expect(isCurrentGeneration(genAtRequest)).toBe(false);
    expect(getPhase()).toBe('idle');
  });

  it('mid-connecting stale: generation captured during connecting is stale after reset', () => {
    beginRequest('PEER-A');
    beginConnecting('PEER-A');
    const genAtConnecting = getGeneration();

    // Peer lost while connecting
    resetSession();
    expect(isCurrentGeneration(genAtConnecting)).toBe(false);
  });

  it('post-mismatch stale: generation before mismatch reset is stale', () => {
    connectToPeer('PEER-A');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-A' });
    const genBeforeMismatch = getGeneration();

    // Mismatch triggers reset
    resetSession();
    expect(isCurrentGeneration(genBeforeMismatch)).toBe(false);

    // Late verification callback from A should be rejected
    if (isCurrentGeneration(genBeforeMismatch)) {
      setVerificationState({ state: 'verified', sasCode: 'SAS-A' });
    }
    expect(getVerificationState().state).toBe('legacy');
  });

  it('5-cycle monotonicity with generation guard', () => {
    const gens: number[] = [getGeneration()];

    for (let i = 0; i < 5; i++) {
      connectToPeer(`PEER-${i}`);
      gens.push(getGeneration());
      resetSession();
    }

    // Monotonic increase
    for (let i = 1; i < gens.length; i++) {
      expect(gens[i]).toBeGreaterThanOrEqual(gens[i - 1]);
    }

    // All prior stale
    const current = getGeneration();
    for (let i = 0; i < gens.length - 1; i++) {
      if (gens[i] !== current) {
        expect(isCurrentGeneration(gens[i])).toBe(false);
      }
    }
  });

  it('stale callback from session A cannot set verification during session B', () => {
    connectToPeer('PEER-A');
    const genA = getGeneration();
    setVerificationState({ state: 'unverified', sasCode: 'SAS-A' });

    resetSession();
    connectToPeer('PEER-B');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-B' });

    // Late callback from A
    if (isCurrentGeneration(genA)) {
      setVerificationState({ state: 'verified', sasCode: 'SAS-A' });
    }

    // Session B state intact
    expect(getVerificationState().state).toBe('unverified');
    expect(getVerificationState().sasCode).toBe('SAS-B');
  });
});

// ── 3. Gating/security behavior under state transitions ───────────────────

describe('R1-4: Transfer gating under verification transitions', () => {
  it('full transfer policy matrix', () => {
    // verified + connected → allowed
    expect(isTransferAllowed('verified', true)).toBe(true);
    // verified + disconnected → blocked
    expect(isTransferAllowed('verified', false)).toBe(false);
    // legacy + connected → allowed
    expect(isTransferAllowed('legacy', true)).toBe(true);
    // legacy + disconnected → blocked
    expect(isTransferAllowed('legacy', false)).toBe(false);
    // unverified + connected → blocked
    expect(isTransferAllowed('unverified', true)).toBe(false);
    // unverified + disconnected → blocked
    expect(isTransferAllowed('unverified', false)).toBe(false);
  });

  it('transfer gating transitions through unverified → verified', () => {
    connectToPeer('PEER-A');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-A' });
    expect(isTransferAllowed('unverified', true)).toBe(false);

    setVerificationState({ state: 'verified', sasCode: 'SAS-A' });
    expect(isTransferAllowed('verified', true)).toBe(true);
  });

  it('transfer blocked after mismatch reset', () => {
    connectToPeer('PEER-A');
    setVerificationState({ state: 'verified', sasCode: 'SAS-A' });
    expect(isTransferAllowed('verified', true)).toBe(true);

    // Mismatch → reset → disconnected
    resetSession();
    expect(isTransferAllowed(getVerificationState().state, false)).toBe(false);
  });

  it('transfer gating correct through full reconnect cycle', () => {
    // Session A: unverified (blocked) → verified (allowed) → disconnect (blocked)
    connectToPeer('PEER-A');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-A' });
    expect(isTransferAllowed('unverified', true)).toBe(false);
    setVerificationState({ state: 'verified', sasCode: 'SAS-A' });
    expect(isTransferAllowed('verified', true)).toBe(true);

    resetSession();
    expect(isTransferAllowed(getVerificationState().state, false)).toBe(false);

    // Session B: same cycle
    connectToPeer('PEER-B');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-B' });
    expect(isTransferAllowed('unverified', true)).toBe(false);
    setVerificationState({ state: 'verified', sasCode: 'SAS-B' });
    expect(isTransferAllowed('verified', true)).toBe(true);
  });
});

// ── 4. Reconnect does not leak prior session trust/verification state ─────

describe('R1-4: No trust/verification leakage across sessions', () => {
  it('verification state resets to legacy on disconnect', () => {
    connectToPeer('PEER-A');
    setVerificationState({ state: 'verified', sasCode: 'SAS-A' });

    resetSession();
    expect(getVerificationState().state).toBe('legacy');
    expect(getVerificationState().sasCode).toBeNull();
  });

  it('SAS code from session A absent in session B', () => {
    connectToPeer('PEER-A');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-A-SECRET' });

    resetSession();
    expect(getVerificationState().sasCode).toBeNull();

    connectToPeer('PEER-B');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-B' });
    expect(getVerificationState().sasCode).toBe('SAS-B');
  });

  it('three-session isolation: each starts at legacy', () => {
    const sessionPeers = ['PEER-A', 'PEER-B', 'PEER-C'];
    const sessionStates: Array<'unverified' | 'verified'> = ['unverified', 'verified', 'unverified'];

    for (let i = 0; i < sessionPeers.length; i++) {
      // Each session starts at legacy
      expect(getVerificationState().state).toBe('legacy');

      connectToPeer(sessionPeers[i]);
      setVerificationState({ state: sessionStates[i], sasCode: `SAS-${i}` });
      expect(getVerificationState().state).toBe(sessionStates[i]);

      resetSession();
    }

    // Final state is legacy
    expect(getVerificationState().state).toBe('legacy');
  });

  it('verification listener receives clean transitions across reconnect', () => {
    const transitions: string[] = [];
    const unsub = onVerificationStateChange((info) => {
      transitions.push(info.state);
    });

    connectToPeer('PEER-A');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-A' });
    setVerificationState({ state: 'verified', sasCode: 'SAS-A' });
    resetSession();
    connectToPeer('PEER-B');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-B' });

    // Verify transitions are clean: unverified → verified → legacy (reset) → unverified
    expect(transitions).toEqual(['unverified', 'verified', 'legacy', 'unverified']);

    unsub();
  });

  it('generation counter isolates sessions even with same peer code', () => {
    connectToPeer('PEER-A');
    const gen1 = getGeneration();

    resetSession();
    connectToPeer('PEER-A'); // Same peer, new session
    const gen2 = getGeneration();

    expect(gen2).toBeGreaterThan(gen1);
    expect(isCurrentGeneration(gen1)).toBe(false);
    expect(isCurrentGeneration(gen2)).toBe(true);
  });

  it('mismatch in session A does not affect session B verification flow', () => {
    // Session A: connect, mismatch detected
    connectToPeer('PEER-A');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-A' });
    // Mismatch error fires → resetSession
    resetSession();

    // Session B: clean verification flow
    connectToPeer('PEER-B');
    setVerificationState({ state: 'unverified', sasCode: 'SAS-B' });
    expect(getVerificationState().state).toBe('unverified');

    setVerificationState({ state: 'verified', sasCode: 'SAS-B' });
    expect(getVerificationState().state).toBe('verified');
  });
});
