// ─── Signaling Provider Interface ──────────────────────────────────────────
// Abstraction layer for signaling transport.
// Implementations: WebSocketSignaling (custom Rust WS server), future: Nostr relays.

export interface SignalMessage {
  type: 'offer' | 'answer' | 'ice-candidate' | 'connection_request' | 'connection_accepted' | 'connection_declined';
  data: any;
  from: string;
  to: string;
}

export interface DiscoveredDevice {
  peerCode: string;
  deviceName: string;
  deviceType: 'phone' | 'tablet' | 'laptop' | 'desktop';
}

export interface SignalingProvider {
  /** Connect to the signaling server and register this peer */
  connect(localPeerCode: string, deviceName: string, deviceType: DiscoveredDevice['deviceType']): Promise<void>;

  /** Subscribe to incoming signaling messages */
  onSignal(callback: (signal: SignalMessage) => void): void;

  /** Subscribe to peer discovery events */
  onPeerDiscovered(callback: (peer: DiscoveredDevice) => void): void;
  onPeerLost(callback: (peerCode: string) => void): void;

  /** Send a signaling message to a specific peer */
  sendSignal(type: SignalMessage['type'], data: any, to: string): Promise<void>;

  /** Get currently discovered peers */
  getPeers(): DiscoveredDevice[];

  /** Disconnect and clean up */
  disconnect(): void;

  /** Provider name for logging */
  readonly name: string;
}
