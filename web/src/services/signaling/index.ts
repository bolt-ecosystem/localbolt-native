// ─── Signaling Module ──────────────────────────────────────────────────────
// Barrel export for the signaling abstraction layer.

export type { SignalMessage, DiscoveredDevice, SignalingProvider } from './SignalingProvider';
export { WebSocketSignaling } from './WebSocketSignaling';
export { DualSignaling } from './DualSignaling';
export { detectDeviceType, getDeviceName } from './device-detect';
