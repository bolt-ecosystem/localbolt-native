// ─── Dual Signaling ─────────────────────────────────────────────────────────
// Connects to both a local signal server (LAN discovery) and a cloud signal
// server (internet discovery). Merges peer lists and routes signals through
// whichever server knows the target peer.
//
// Graceful degradation: if either connection fails, the other still works.

import type { SignalingProvider, SignalMessage, DiscoveredDevice } from './SignalingProvider';
import { WebSocketSignaling } from './WebSocketSignaling';

export class DualSignaling implements SignalingProvider {
  readonly name = 'DualSignaling';

  private local: WebSocketSignaling | null = null;
  private cloud: WebSocketSignaling | null = null;

  private localUrl: string;
  private cloudUrl: string;

  // Track which server knows which peer
  private peerSource: Map<string, 'local' | 'cloud'> = new Map();

  // Callbacks
  private signalCallbacks: Array<(signal: SignalMessage) => void> = [];
  private peerDiscoveredCallback: ((peer: DiscoveredDevice) => void) | null = null;
  private peerLostCallback: ((peerCode: string) => void) | null = null;

  // Merged peer list
  private allPeers: Map<string, DiscoveredDevice> = new Map();

  // Connection state
  private localConnected = false;
  private cloudConnected = false;
  private onConnectionStateChange: (() => void) | null = null;

  constructor(localUrl: string, cloudUrl: string) {
    this.localUrl = localUrl;
    this.cloudUrl = cloudUrl;
  }

  async connect(
    localPeerCode: string,
    deviceName: string,
    deviceType: DiscoveredDevice['deviceType']
  ): Promise<void> {
    // Create both instances
    this.local = new WebSocketSignaling(this.localUrl);
    this.cloud = new WebSocketSignaling(this.cloudUrl);

    // Wire up local
    this.local.onPeerDiscovered((peer) => this.handlePeerDiscovered(peer, 'local'));
    this.local.onPeerLost((code) => this.handlePeerLost(code, 'local'));
    this.local.onSignal((signal) => this.handleSignal(signal));

    // Wire up cloud
    this.cloud.onPeerDiscovered((peer) => this.handlePeerDiscovered(peer, 'cloud'));
    this.cloud.onPeerLost((code) => this.handlePeerLost(code, 'cloud'));
    this.cloud.onSignal((signal) => this.handleSignal(signal));

    // Connect both — don't fail if one fails
    const results = await Promise.allSettled([
      this.local.connect(localPeerCode, deviceName, deviceType).then(() => {
        this.localConnected = true;
        console.log('[DUAL] Local signal server connected');
        this.onConnectionStateChange?.();
      }),
      this.cloud.connect(localPeerCode, deviceName, deviceType).then(() => {
        this.cloudConnected = true;
        console.log('[DUAL] Cloud signal server connected');
        this.onConnectionStateChange?.();
      }),
    ]);

    // At least one must succeed
    const anyConnected = results.some((r) => r.status === 'fulfilled');
    if (!anyConnected) {
      throw new Error('Failed to connect to any signal server');
    }

    // Log what connected
    if (results[0].status === 'rejected') {
      console.warn('[DUAL] Local signal server failed:', (results[0] as PromiseRejectedResult).reason);
    }
    if (results[1].status === 'rejected') {
      console.warn('[DUAL] Cloud signal server failed:', (results[1] as PromiseRejectedResult).reason);
    }
  }

  onSignal(callback: (signal: SignalMessage) => void): void {
    this.signalCallbacks.push(callback);
  }

  onPeerDiscovered(callback: (peer: DiscoveredDevice) => void): void {
    this.peerDiscoveredCallback = callback;
  }

  onPeerLost(callback: (peerCode: string) => void): void {
    this.peerLostCallback = callback;
  }

  async sendSignal(type: SignalMessage['type'], data: any, to: string): Promise<void> {
    const source = this.peerSource.get(to);

    if (source === 'local' && this.local) {
      return this.local.sendSignal(type, data, to);
    }
    if (source === 'cloud' && this.cloud) {
      return this.cloud.sendSignal(type, data, to);
    }

    // Unknown peer — try both (one will fail, that's fine)
    const errors: Error[] = [];
    if (this.local && this.localConnected) {
      try {
        await this.local.sendSignal(type, data, to);
        return;
      } catch (e) {
        errors.push(e as Error);
      }
    }
    if (this.cloud && this.cloudConnected) {
      try {
        await this.cloud.sendSignal(type, data, to);
        return;
      } catch (e) {
        errors.push(e as Error);
      }
    }

    throw new Error(`Could not send signal to ${to}: no connected server knows this peer`);
  }

  getPeers(): DiscoveredDevice[] {
    return Array.from(this.allPeers.values());
  }

  disconnect(): void {
    this.local?.disconnect();
    this.cloud?.disconnect();
    this.local = null;
    this.cloud = null;
    this.localConnected = false;
    this.cloudConnected = false;
    this.allPeers.clear();
    this.peerSource.clear();
    this.signalCallbacks = [];
    this.peerDiscoveredCallback = null;
    this.peerLostCallback = null;
    this.onConnectionStateChange = null;
  }

  // ─── State Accessors ────────────────────────────────────────────────────

  /** Whether at least one signal server is connected */
  isConnected(): boolean {
    return this.localConnected || this.cloudConnected;
  }

  /** Subscribe to connection state changes (for header indicator) */
  setConnectionStateHandler(handler: () => void): void {
    this.onConnectionStateChange = handler;
  }

  // ─── Internal ───────────────────────────────────────────────────────────

  private handlePeerDiscovered(peer: DiscoveredDevice, source: 'local' | 'cloud') {
    // Don't duplicate — if already known from the other source, skip
    if (this.allPeers.has(peer.peerCode)) return;

    this.allPeers.set(peer.peerCode, peer);
    this.peerSource.set(peer.peerCode, source);
    console.log(`[DUAL] Peer discovered via ${source}: ${peer.peerCode} (${peer.deviceName})`);
    this.peerDiscoveredCallback?.(peer);
  }

  private handlePeerLost(peerCode: string, source: 'local' | 'cloud') {
    // Only remove if this was the source that discovered the peer
    if (this.peerSource.get(peerCode) !== source) return;

    this.allPeers.delete(peerCode);
    this.peerSource.delete(peerCode);
    console.log(`[DUAL] Peer lost via ${source}: ${peerCode}`);
    this.peerLostCallback?.(peerCode);
  }

  private handleSignal(signal: SignalMessage) {
    for (const cb of this.signalCallbacks) {
      cb(signal);
    }
  }
}
