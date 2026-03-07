/**
 * Daemon IPC service for localbolt-app.
 *
 * Bridges Tauri backend events (watchdog state, daemon.status, pairing/transfer
 * requests) into frontend state and exposes command wrappers for decisions.
 *
 * Degrades gracefully when not running inside Tauri (browser dev mode).
 */

// ── Types ──────────────────────────────────────────────────────────────

export type WatchdogState = 'starting' | 'ready' | 'restarting' | 'degraded' | 'incompatible';

export type Decision = 'allow_once' | 'allow_always' | 'deny_once' | 'deny_always';

export interface WatchdogStateEvent {
  state: WatchdogState;
  retry_count: number;
}

export interface DaemonStatusUpdate {
  connected_peers: number;
  ui_connected: boolean;
  version: string;
}

export interface PairingRequest {
  request_id: string;
  remote_device_name: string;
  remote_device_type: string;
  remote_identity_pk_b64: string;
  sas: string;
  capabilities_requested: string[];
}

export interface TransferIncomingRequest {
  request_id: string;
  from_device_name: string;
  from_identity_pk_b64: string;
  file_name: string;
  file_size_bytes: number;
  sha256_hex: string | null;
  mime: string | null;
}

export interface DaemonState {
  watchdog: WatchdogState;
  retryCount: number;
  connectedPeers: number;
  daemonVersion: string | null;
  pendingPairingRequest: PairingRequest | null;
  pendingTransferRequest: TransferIncomingRequest | null;
}

export type DaemonStateListener = (state: DaemonState) => void;

// ── Tauri detection ────────────────────────────────────────────────────

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

function isTauri(): boolean {
  return typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__;
}

// ── Service ────────────────────────────────────────────────────────────

let currentState: DaemonState = {
  watchdog: 'starting',
  retryCount: 0,
  connectedPeers: 0,
  daemonVersion: null,
  pendingPairingRequest: null,
  pendingTransferRequest: null,
};

const listeners: Set<DaemonStateListener> = new Set();
const unlisten: Array<() => void> = [];

function setState(partial: Partial<DaemonState>) {
  currentState = { ...currentState, ...partial };
  listeners.forEach((fn) => fn(currentState));
}

/** Subscribe to daemon state changes. Returns unsubscribe function. */
export function subscribeDaemonState(listener: DaemonStateListener): () => void {
  listeners.add(listener);
  // Emit current state immediately
  listener(currentState);
  return () => listeners.delete(listener);
}

/** Get current daemon state snapshot. */
export function getDaemonState(): DaemonState {
  return currentState;
}

/** Whether the daemon is ready (transfer actions should be enabled). */
export function isDaemonReady(): boolean {
  return currentState.watchdog === 'ready';
}

// ── Initialization ─────────────────────────────────────────────────────

let initialized = false;

export async function initDaemonService(): Promise<void> {
  if (initialized) return;
  initialized = true;

  if (!isTauri()) {
    console.warn('[DAEMON] Not running in Tauri — daemon service disabled');
    return;
  }

  try {
    const { listen } = await import('@tauri-apps/api/event');
    const { invoke } = await import('@tauri-apps/api/core');

    // Fetch initial watchdog state
    try {
      const initial = await invoke<WatchdogStateEvent>('get_watchdog_state');
      setState({
        watchdog: initial.state,
        retryCount: initial.retry_count,
      });
    } catch (e) {
      console.warn('[DAEMON] failed to get initial state:', e);
    }

    // Subscribe to watchdog state changes
    const u1 = await listen<WatchdogStateEvent>('daemon://watchdog-state', (event) => {
      setState({
        watchdog: event.payload.state,
        retryCount: event.payload.retry_count,
      });
    });
    unlisten.push(u1);

    // Subscribe to daemon.status updates
    const u2 = await listen<DaemonStatusUpdate>('daemon://status-update', (event) => {
      setState({
        connectedPeers: event.payload.connected_peers,
        daemonVersion: event.payload.version,
      });
    });
    unlisten.push(u2);

    // Subscribe to pairing requests
    const u3 = await listen<PairingRequest>('daemon://pairing-request', (event) => {
      setState({ pendingPairingRequest: event.payload });
    });
    unlisten.push(u3);

    // Subscribe to transfer requests
    const u4 = await listen<TransferIncomingRequest>('daemon://transfer-request', (event) => {
      setState({ pendingTransferRequest: event.payload });
    });
    unlisten.push(u4);

    // Subscribe to bridge disconnection
    const u5 = await listen('daemon://bridge-disconnected', () => {
      console.warn('[DAEMON] IPC bridge disconnected');
    });
    unlisten.push(u5);

    console.log('[DAEMON] service initialized');
  } catch (e) {
    console.warn('[DAEMON] failed to initialize:', e);
  }
}

// ── Commands ───────────────────────────────────────────────────────────

export async function restartDaemon(): Promise<string> {
  if (!isTauri()) return 'not available outside Tauri';
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<string>('restart_daemon');
}

export async function sendPairingDecision(
  requestId: string,
  decision: Decision,
  note?: string,
): Promise<string> {
  if (!isTauri()) return 'not available outside Tauri';
  const { invoke } = await import('@tauri-apps/api/core');
  const result = await invoke<string>('send_pairing_decision', {
    payload: { request_id: requestId, decision, note: note ?? null },
  });
  setState({ pendingPairingRequest: null });
  return result;
}

export async function sendTransferDecision(
  requestId: string,
  decision: Decision,
  note?: string,
): Promise<string> {
  if (!isTauri()) return 'not available outside Tauri';
  const { invoke } = await import('@tauri-apps/api/core');
  const result = await invoke<string>('send_transfer_decision', {
    payload: { request_id: requestId, decision, note: note ?? null },
  });
  setState({ pendingTransferRequest: null });
  return result;
}

export async function exportSupportBundle(): Promise<string> {
  if (!isTauri()) return 'not available outside Tauri';
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<string>('export_support_bundle');
}
