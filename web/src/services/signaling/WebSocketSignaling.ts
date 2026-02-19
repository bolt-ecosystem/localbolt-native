// ─── WebSocket Signaling Implementation ────────────────────────────────────
// Connects to the custom Rust WebSocket signaling server.
// Features: auto-reconnect with exponential backoff, heartbeat keepalive,
// peer discovery via IP-based grouping.

import type { SignalingProvider, SignalMessage, DiscoveredDevice } from './SignalingProvider';

// ─── Wire Protocol Types (Client <-> Server) ──────────────────────────────

/** Messages sent from client to server */
interface RegisterMessage {
  type: 'register';
  peer_code: string;
  device_name: string;
  device_type: string;
}

interface SignalOutMessage {
  type: 'signal';
  to: string;
  payload: SignalMessage;
}

type ClientMessage = RegisterMessage | SignalOutMessage;

/** Messages received from server */
interface PeersMessage {
  type: 'peers';
  peers: Array<{ peer_code: string; device_name: string; device_type: string }>;
}

interface PeerJoinedMessage {
  type: 'peer_joined';
  peer: { peer_code: string; device_name: string; device_type: string };
}

interface PeerLeftMessage {
  type: 'peer_left';
  peer_code: string;
}

interface SignalInMessage {
  type: 'signal';
  from: string;
  payload: SignalMessage;
}

type ServerMessage = PeersMessage | PeerJoinedMessage | PeerLeftMessage | SignalInMessage;

// ─── Constants ─────────────────────────────────────────────────────────────

const HEARTBEAT_INTERVAL_MS = 30_000;
const RECONNECT_BASE_MS = 1_000;
const RECONNECT_MAX_MS = 30_000;

// ─── WebSocketSignaling ────────────────────────────────────────────────────

export class WebSocketSignaling implements SignalingProvider {
  readonly name = 'WebSocketSignaling';

  private wsUrl: string;
  private ws: WebSocket | null = null;
  private localPeerCode: string = '';
  private deviceName: string = '';
  private deviceType: DiscoveredDevice['deviceType'] = 'laptop';

  // Callbacks
  private signalCallbacks: Array<(signal: SignalMessage) => void> = [];
  private peerDiscoveredCallback: ((peer: DiscoveredDevice) => void) | null = null;
  private peerLostCallback: ((peerCode: string) => void) | null = null;

  // Peer tracking
  private peers: Map<string, DiscoveredDevice> = new Map();

  // Reconnection
  private reconnectAttempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private intentionalDisconnect = false;

  // Heartbeat
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;

  // Connection promise
  private connectResolve: (() => void) | null = null;
  private connectReject: ((err: Error) => void) | null = null;

  constructor(wsUrl: string) {
    this.wsUrl = wsUrl;
  }

  // ─── SignalingProvider Interface ──────────────────────────────────────

  async connect(
    localPeerCode: string,
    deviceName: string,
    deviceType: DiscoveredDevice['deviceType']
  ): Promise<void> {
    this.localPeerCode = localPeerCode;
    this.deviceName = deviceName;
    this.deviceType = deviceType;
    this.intentionalDisconnect = false;
    this.reconnectAttempt = 0;

    return this.openConnection(true);
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
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket not connected');
    }

    const msg: SignalOutMessage = {
      type: 'signal',
      to,
      payload: {
        type,
        data,
        from: this.localPeerCode,
        to,
      },
    };

    this.ws.send(JSON.stringify(msg));
    console.log(`[WS-SIGNAL] Sent ${type} to ${to}`);
  }

  getPeers(): DiscoveredDevice[] {
    return Array.from(this.peers.values());
  }

  disconnect(): void {
    console.log('[WS-SIGNAL] Disconnecting');
    this.intentionalDisconnect = true;
    this.stopHeartbeat();
    this.stopReconnect();

    if (this.ws) {
      // Remove handlers before closing to avoid reconnect trigger
      this.ws.onclose = null;
      this.ws.onerror = null;
      this.ws.onmessage = null;
      this.ws.onopen = null;
      try { this.ws.close(); } catch { /* ignore */ }
      this.ws = null;
    }

    this.peers.clear();
    this.signalCallbacks = [];
    this.peerDiscoveredCallback = null;
    this.peerLostCallback = null;
    this.connectResolve = null;
    this.connectReject = null;
  }

  // ─── Internal ────────────────────────────────────────────────────────

  private openConnection(isInitial: boolean): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      if (isInitial) {
        this.connectResolve = resolve;
        this.connectReject = reject;
      }

      console.log(`[WS-SIGNAL] Connecting to ${this.wsUrl}...`);
      this.ws = new WebSocket(this.wsUrl);

      this.ws.onopen = () => {
        console.log('[WS-SIGNAL] Connected');
        this.reconnectAttempt = 0;
        this.sendRegister();
        this.startHeartbeat();

        // Resolve only the initial connect() call
        if (this.connectResolve) {
          this.connectResolve();
          this.connectResolve = null;
          this.connectReject = null;
        }
      };

      this.ws.onmessage = (event) => {
        this.handleMessage(event);
      };

      this.ws.onclose = (event) => {
        console.log(`[WS-SIGNAL] Connection closed (code=${event.code}, reason=${event.reason})`);
        this.stopHeartbeat();

        if (!this.intentionalDisconnect) {
          this.scheduleReconnect();
        }
      };

      this.ws.onerror = (event) => {
        console.error('[WS-SIGNAL] WebSocket error:', event);

        // Reject the initial connect() call on error
        if (this.connectReject) {
          this.connectReject(new Error('WebSocket connection failed'));
          this.connectResolve = null;
          this.connectReject = null;
        }
      };

      // If not the initial connection, resolve immediately
      // (reconnect attempts don't return promises to callers)
      if (!isInitial) {
        resolve();
      }
    });
  }

  private sendRegister(): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;

    const msg: RegisterMessage = {
      type: 'register',
      peer_code: this.localPeerCode,
      device_name: this.deviceName,
      device_type: this.deviceType,
    };

    this.ws.send(JSON.stringify(msg));
    console.log(`[WS-SIGNAL] Registered as ${this.localPeerCode} (${this.deviceName})`);
  }

  private handleMessage(event: MessageEvent): void {
    let msg: ServerMessage;
    try {
      msg = JSON.parse(event.data);
    } catch {
      console.warn('[WS-SIGNAL] Failed to parse message:', event.data);
      return;
    }

    switch (msg.type) {
      case 'peers':
        this.handlePeersList(msg);
        break;
      case 'peer_joined':
        this.handlePeerJoined(msg);
        break;
      case 'peer_left':
        this.handlePeerLeft(msg);
        break;
      case 'signal':
        this.handleSignal(msg);
        break;
      default:
        console.warn('[WS-SIGNAL] Unknown message type:', (msg as any).type);
    }
  }

  private handlePeersList(msg: PeersMessage): void {
    this.peers.clear();
    for (const p of msg.peers) {
      const device = this.toDiscoveredDevice(p);
      this.peers.set(device.peerCode, device);
      if (this.peerDiscoveredCallback) {
        this.peerDiscoveredCallback(device);
      }
    }
    console.log(`[WS-SIGNAL] Received peer list: ${msg.peers.length} peers`);
  }

  private handlePeerJoined(msg: PeerJoinedMessage): void {
    const device = this.toDiscoveredDevice(msg.peer);
    this.peers.set(device.peerCode, device);
    console.log(`[WS-SIGNAL] Peer joined: ${device.peerCode} (${device.deviceName})`);
    if (this.peerDiscoveredCallback) {
      this.peerDiscoveredCallback(device);
    }
  }

  private handlePeerLeft(msg: PeerLeftMessage): void {
    this.peers.delete(msg.peer_code);
    console.log(`[WS-SIGNAL] Peer left: ${msg.peer_code}`);
    if (this.peerLostCallback) {
      this.peerLostCallback(msg.peer_code);
    }
  }

  private handleSignal(msg: SignalInMessage): void {
    console.log(`[WS-SIGNAL] Received ${msg.payload.type} from ${msg.from}`);
    for (const cb of this.signalCallbacks) {
      cb(msg.payload);
    }
  }

  private toDiscoveredDevice(raw: { peer_code: string; device_name: string; device_type: string }): DiscoveredDevice {
    const validTypes: DiscoveredDevice['deviceType'][] = ['phone', 'tablet', 'laptop', 'desktop'];
    const deviceType = validTypes.includes(raw.device_type as any)
      ? (raw.device_type as DiscoveredDevice['deviceType'])
      : 'laptop';

    return {
      peerCode: raw.peer_code,
      deviceName: raw.device_name,
      deviceType,
    };
  }

  // ─── Heartbeat ───────────────────────────────────────────────────────

  private startHeartbeat(): void {
    this.stopHeartbeat();
    this.heartbeatTimer = setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: 'ping' }));
      }
    }, HEARTBEAT_INTERVAL_MS);
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  // ─── Reconnection ───────────────────────────────────────────────────

  private scheduleReconnect(): void {
    if (this.intentionalDisconnect) return;

    const delay = Math.min(
      RECONNECT_BASE_MS * Math.pow(2, this.reconnectAttempt),
      RECONNECT_MAX_MS
    );
    this.reconnectAttempt++;

    console.log(`[WS-SIGNAL] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempt})`);
    this.reconnectTimer = setTimeout(() => {
      this.openConnection(false).catch((err) => {
        console.error('[WS-SIGNAL] Reconnect failed:', err);
      });
    }, delay);
  }

  private stopReconnect(): void {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    this.reconnectAttempt = 0;
  }
}
