import type { DiscoveredDevice } from '@/services/signaling/SignalingProvider';
import type { TransferProgress } from '@/services/webrtc/WebRTCService';

export interface ConnectionRequest {
  peerCode: string;
  deviceName: string;
  deviceType: string;
}

export interface AppState {
  peerCode: string;
  peers: DiscoveredDevice[];
  isConnected: boolean;
  signalingConnected: boolean;
  connectingTo: string | null;
  connectedDevice: DiscoveredDevice | null;
  transferProgress: TransferProgress | null;
  files: File[];
  isDragging: boolean;
  incomingRequest: ConnectionRequest | null;
  showDeviceList: boolean;
}

type Listener = () => void;

const initialState: AppState = {
  peerCode: '',
  peers: [],
  isConnected: false,
  signalingConnected: false,
  connectingTo: null,
  connectedDevice: null,
  transferProgress: null,
  files: [],
  isDragging: false,
  incomingRequest: null,
  showDeviceList: false,
};

class AppStore {
  private state: AppState = { ...initialState };
  private listeners: Set<Listener> = new Set();

  getState(): Readonly<AppState> {
    return this.state;
  }

  setState(partial: Partial<AppState>) {
    this.state = { ...this.state, ...partial };
    this.listeners.forEach((fn) => fn());
  }

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}

export const store = new AppStore();
