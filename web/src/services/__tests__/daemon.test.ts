/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';

// Reset module state before each test (daemon.ts has module-level state)
let daemonModule: typeof import('@/services/daemon');

async function loadFreshModule() {
  vi.resetModules();
  daemonModule = await import('@/services/daemon');
}

describe('daemon service — non-Tauri mode', () => {
  beforeEach(async () => {
    // Ensure no Tauri internals
    delete (window as any).__TAURI_INTERNALS__;
    await loadFreshModule();
  });

  it('getDaemonState returns initial state', () => {
    const state = daemonModule.getDaemonState();
    expect(state.watchdog).toBe('starting');
    expect(state.retryCount).toBe(0);
    expect(state.connectedPeers).toBe(0);
    expect(state.daemonVersion).toBeNull();
    expect(state.pendingPairingRequest).toBeNull();
    expect(state.pendingTransferRequest).toBeNull();
  });

  it('isDaemonReady returns false when starting', () => {
    expect(daemonModule.isDaemonReady()).toBe(false);
  });

  it('subscribeDaemonState emits current state immediately', () => {
    const fn = vi.fn();
    const unsub = daemonModule.subscribeDaemonState(fn);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn.mock.calls[0][0].watchdog).toBe('starting');
    unsub();
  });

  it('subscribeDaemonState unsubscribe stops notifications', () => {
    const fn = vi.fn();
    const unsub = daemonModule.subscribeDaemonState(fn);
    expect(fn).toHaveBeenCalledTimes(1);
    unsub();
    // No way to trigger state change in non-Tauri mode,
    // but ensure unsub returns cleanly
  });

  it('initDaemonService in non-Tauri mode does not throw', async () => {
    await expect(daemonModule.initDaemonService()).resolves.not.toThrow();
  });

  it('restartDaemon returns fallback outside Tauri', async () => {
    const result = await daemonModule.restartDaemon();
    expect(result).toBe('not available outside Tauri');
  });

  it('sendPairingDecision returns fallback outside Tauri', async () => {
    const result = await daemonModule.sendPairingDecision('evt-0', 'allow_once');
    expect(result).toBe('not available outside Tauri');
  });

  it('sendTransferDecision returns fallback outside Tauri', async () => {
    const result = await daemonModule.sendTransferDecision('evt-0', 'deny_once');
    expect(result).toBe('not available outside Tauri');
  });

  it('exportSupportBundle returns fallback outside Tauri', async () => {
    const result = await daemonModule.exportSupportBundle();
    expect(result).toBe('not available outside Tauri');
  });
});

describe('daemon service — Tauri mode with mocked API', () => {
  const mockListen = vi.fn();
  const mockInvoke = vi.fn();
  const mockUnlisten = vi.fn();

  beforeEach(async () => {
    // Set up Tauri internals flag
    (window as any).__TAURI_INTERNALS__ = {};

    // Mock Tauri API modules
    mockListen.mockReset();
    mockInvoke.mockReset();
    mockUnlisten.mockReset();

    mockListen.mockResolvedValue(mockUnlisten);
    mockInvoke.mockResolvedValue({ state: 'ready', retry_count: 0 });

    vi.doMock('@tauri-apps/api/event', () => ({
      listen: mockListen,
    }));
    vi.doMock('@tauri-apps/api/core', () => ({
      invoke: mockInvoke,
    }));

    await loadFreshModule();
  });

  it('initDaemonService fetches initial state and subscribes to events', async () => {
    await daemonModule.initDaemonService();

    // Should have called invoke for initial state
    expect(mockInvoke).toHaveBeenCalledWith('get_watchdog_state');

    // Should have subscribed to 5 events
    expect(mockListen).toHaveBeenCalledTimes(5);
    const eventNames = mockListen.mock.calls.map((c: any[]) => c[0]);
    expect(eventNames).toContain('daemon://watchdog-state');
    expect(eventNames).toContain('daemon://status-update');
    expect(eventNames).toContain('daemon://pairing-request');
    expect(eventNames).toContain('daemon://transfer-request');
    expect(eventNames).toContain('daemon://bridge-disconnected');
  });

  it('initial state is set from invoke result', async () => {
    await daemonModule.initDaemonService();
    const state = daemonModule.getDaemonState();
    expect(state.watchdog).toBe('ready');
    expect(daemonModule.isDaemonReady()).toBe(true);
  });

  it('watchdog event updates state', async () => {
    await daemonModule.initDaemonService();

    // Find the watchdog listener
    const watchdogCall = mockListen.mock.calls.find((c: any[]) => c[0] === 'daemon://watchdog-state');
    const handler = watchdogCall[1];

    handler({ payload: { state: 'degraded', retry_count: 3 } });
    const state = daemonModule.getDaemonState();
    expect(state.watchdog).toBe('degraded');
    expect(state.retryCount).toBe(3);
    expect(daemonModule.isDaemonReady()).toBe(false);
  });

  it('daemon.status event updates connected peers', async () => {
    await daemonModule.initDaemonService();

    const statusCall = mockListen.mock.calls.find((c: any[]) => c[0] === 'daemon://status-update');
    const handler = statusCall[1];

    handler({ payload: { connected_peers: 2, ui_connected: true, version: '0.0.1' } });
    const state = daemonModule.getDaemonState();
    expect(state.connectedPeers).toBe(2);
    expect(state.daemonVersion).toBe('0.0.1');
  });

  it('pairing.request event stored as pending', async () => {
    await daemonModule.initDaemonService();

    const pairingCall = mockListen.mock.calls.find((c: any[]) => c[0] === 'daemon://pairing-request');
    const handler = pairingCall[1];

    const req = {
      request_id: 'evt-5',
      remote_device_name: 'Alice',
      remote_device_type: 'desktop',
      remote_identity_pk_b64: 'dGVzdA==',
      sas: '123456',
      capabilities_requested: ['file_transfer'],
    };
    handler({ payload: req });
    expect(daemonModule.getDaemonState().pendingPairingRequest).toEqual(req);
  });

  it('transfer.incoming.request event stored as pending', async () => {
    await daemonModule.initDaemonService();

    const transferCall = mockListen.mock.calls.find((c: any[]) => c[0] === 'daemon://transfer-request');
    const handler = transferCall[1];

    const req = {
      request_id: 'evt-7',
      from_device_name: 'Bob',
      from_identity_pk_b64: 'a2V5',
      file_name: 'doc.pdf',
      file_size_bytes: 1024,
      sha256_hex: null,
      mime: 'application/pdf',
    };
    handler({ payload: req });
    expect(daemonModule.getDaemonState().pendingTransferRequest).toEqual(req);
  });

  it('sendPairingDecision invokes correct command and clears pending', async () => {
    mockInvoke.mockResolvedValue('decision sent');
    await daemonModule.initDaemonService();

    // Set a pending request first
    const pairingCall = mockListen.mock.calls.find((c: any[]) => c[0] === 'daemon://pairing-request');
    pairingCall[1]({
      payload: {
        request_id: 'evt-5',
        remote_device_name: 'Alice',
        remote_device_type: 'desktop',
        remote_identity_pk_b64: 'dGVzdA==',
        sas: '123456',
        capabilities_requested: [],
      },
    });
    expect(daemonModule.getDaemonState().pendingPairingRequest).not.toBeNull();

    const result = await daemonModule.sendPairingDecision('evt-5', 'allow_once');
    expect(result).toBe('decision sent');
    expect(mockInvoke).toHaveBeenCalledWith('send_pairing_decision', {
      payload: { request_id: 'evt-5', decision: 'allow_once', note: null },
    });
    expect(daemonModule.getDaemonState().pendingPairingRequest).toBeNull();
  });

  it('sendTransferDecision invokes correct command and clears pending', async () => {
    mockInvoke.mockResolvedValue('decision sent');
    await daemonModule.initDaemonService();

    // Set a pending request
    const transferCall = mockListen.mock.calls.find((c: any[]) => c[0] === 'daemon://transfer-request');
    transferCall[1]({
      payload: {
        request_id: 'evt-7',
        from_device_name: 'Bob',
        from_identity_pk_b64: 'a2V5',
        file_name: 'doc.pdf',
        file_size_bytes: 1024,
        sha256_hex: null,
        mime: null,
      },
    });

    const result = await daemonModule.sendTransferDecision('evt-7', 'deny_always', 'untrusted');
    expect(result).toBe('decision sent');
    expect(mockInvoke).toHaveBeenCalledWith('send_transfer_decision', {
      payload: { request_id: 'evt-7', decision: 'deny_always', note: 'untrusted' },
    });
    expect(daemonModule.getDaemonState().pendingTransferRequest).toBeNull();
  });

  it('subscriber receives updates from events', async () => {
    await daemonModule.initDaemonService();

    const fn = vi.fn();
    daemonModule.subscribeDaemonState(fn);
    fn.mockClear(); // ignore initial emission

    // Trigger watchdog event
    const watchdogCall = mockListen.mock.calls.find((c: any[]) => c[0] === 'daemon://watchdog-state');
    watchdogCall[1]({ payload: { state: 'incompatible', retry_count: 0 } });

    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn.mock.calls[0][0].watchdog).toBe('incompatible');
  });

  it('initDaemonService is idempotent', async () => {
    await daemonModule.initDaemonService();
    const firstCallCount = mockInvoke.mock.calls.length;

    await daemonModule.initDaemonService();
    // Should not have made additional calls
    expect(mockInvoke.mock.calls.length).toBe(firstCallCount);
  });

  it('handles invoke failure gracefully', async () => {
    mockInvoke.mockRejectedValue(new Error('command not found'));
    // Should not throw
    await expect(daemonModule.initDaemonService()).resolves.not.toThrow();
  });
});
