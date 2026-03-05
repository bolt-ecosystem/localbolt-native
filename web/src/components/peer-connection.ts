import { generateSecurePeerCode } from '@the9ines/bolt-core';
import {
  store, showToast, DualSignaling, detectDeviceType, getDeviceName,
  WebRTCService, WebRTCError, SignalingError, detectDevice,
  createConnectionStatus, createDeviceDiscovery, setWebrtcRef,
} from '@the9ines/bolt-transport-web';
import type { TransferProgress, SignalMessage } from '@the9ines/bolt-transport-web';
import {
  getPhase, getGeneration, isCurrentGeneration,
  beginRequest, receiveRequest, beginConnecting,
  markConnected, resetSession,
} from '@the9ines/localbolt-core';

let signalingRef: DualSignaling | null = null;
let rtcServiceRef: WebRTCService | null = null;

function handleConnectionError(error: WebRTCError) {
  console.error(`[${error.name}]`, error.message, error.details);

  // Use canonical reset path
  resetSession();

  let title = 'Connection Error';
  let description = 'Failed to establish connection';
  const device = detectDevice();

  switch (error.name) {
    case 'ConnectionError':
      title = 'Connection Failed';
      description = device.isLinux
        ? 'Connection failed. Please check your firewall settings.'
        : 'Unable to connect to peer. Please try again.';
      break;
    case 'SignalingError':
      title = 'Signaling Error';
      description = 'Failed to establish initial connection. Please check your peer code.';
      break;
    case 'TransferError':
      title = 'Transfer Failed';
      description = device.isWindows
        ? 'File transfer failed. Please check your Windows Defender settings.'
        : 'File transfer failed. Please try again.';
      break;
    case 'EncryptionError':
      title = 'Security Error';
      description = 'Failed to encrypt/decrypt data. Please reconnect.';
      break;
  }

  showToast(title, description, 'destructive');
}

function handleConnectionStateChange(state: RTCPeerConnectionState) {
  console.log('[UI] Connection state changed:', state);
  const connected = state === 'connected';
  const { peers } = store.getState();

  if (connected && rtcServiceRef) {
    const remotePeerCode = rtcServiceRef.getRemotePeerCode();
    const device = peers.find((p) => p.peerCode === remotePeerCode) || null;

    // Transition session to connected
    markConnected();

    store.setState({
      isConnected: true,
      connectedDevice: device,
      connectingTo: null,
      incomingRequest: null,
      showDeviceList: false,
    });
    setWebrtcRef(rtcServiceRef);
  } else {
    // Non-connected state (disconnected, failed, etc.) — canonical reset
    resetSession();
  }
}

function handleFileReceive(file: Blob, filename: string) {
  const url = URL.createObjectURL(file);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

function handleReceiveProgress(progress: TransferProgress) {
  store.setState({ transferProgress: progress });

  if (progress.status === 'completed') {
    showToast('Transfer Complete', `${progress.filename} has been received successfully`);
    setTimeout(() => store.setState({ transferProgress: null }), 2000);
  } else if (progress.status === 'canceled_by_sender' || progress.status === 'canceled_by_receiver') {
    store.setState({ transferProgress: null });
    showToast('Transfer Canceled', 'The file transfer was cancelled');
  } else if (progress.status === 'error') {
    store.setState({ transferProgress: null });
    showToast('Transfer Error', 'The transfer was terminated due to an error', 'destructive');
  }
}

// ── Connection Approval Protocol ─────────────────────────────────────────

function handleApprovalSignal(signal: SignalMessage) {
  // Only handle connection approval types — ignore WebRTC types
  switch (signal.type) {
    case 'connection_request': {
      console.log('[APPROVAL] Received connection request from', signal.from);
      const currentPhase = getPhase();
      if (currentPhase !== 'idle') {
        // Already busy — auto-decline
        signalingRef?.sendSignal('connection_declined', { reason: 'busy' }, signal.from);
        return;
      }
      // Transition session to incoming_request
      receiveRequest(signal.from);
      store.setState({
        incomingRequest: {
          peerCode: signal.from,
          deviceName: signal.data.deviceName,
          deviceType: signal.data.deviceType,
        },
        showDeviceList: false,
      });
      break;
    }

    case 'connection_accepted': {
      console.log('[APPROVAL] Connection accepted by', signal.from);
      const currentPhase = getPhase();
      const { connectingTo } = store.getState();
      if (currentPhase !== 'requesting' || connectingTo !== signal.from) return; // stale

      // Transition session to connecting
      beginConnecting(signal.from);

      // Now initiate the actual WebRTC connection
      const gen = getGeneration();
      if (rtcServiceRef) {
        rtcServiceRef.connect(signal.from).catch((error) => {
          // Guard against stale callback
          if (!isCurrentGeneration(gen)) return;
          store.setState({ connectingTo: null });
          if (error instanceof WebRTCError) {
            handleConnectionError(error);
          } else {
            showToast('Connection Failed', 'Unable to connect to device. Please try again.', 'destructive');
          }
        });
      }
      break;
    }

    case 'connection_declined': {
      console.log('[APPROVAL] Connection declined by', signal.from);
      const { connectingTo, incomingRequest } = store.getState();
      if (connectingTo === signal.from) {
        // We were waiting for approval — they declined
        resetSession();
        showToast('Connection Declined', 'The other device declined the connection request');
      } else if (incomingRequest?.peerCode === signal.from) {
        // They cancelled their request to us
        resetSession();
      }
      break;
    }
  }
}

function selectPeer(peerCode: string) {
  if (!signalingRef) return;

  // Use session phase guard instead of just isConnected
  if (!beginRequest(peerCode)) return;

  const localDeviceName = getDeviceName();
  const localDeviceType = detectDeviceType();

  store.setState({ connectingTo: peerCode, showDeviceList: false });

  // Send connection request via signaling (not WebRTC yet)
  const gen = getGeneration();
  signalingRef.sendSignal('connection_request', {
    deviceName: localDeviceName,
    deviceType: localDeviceType,
  }, peerCode).catch(() => {
    // Guard against stale callback
    if (!isCurrentGeneration(gen)) return;
    resetSession();
    showToast('Request Failed', 'Could not send connection request', 'destructive');
  });
}

function acceptRequest() {
  const { incomingRequest } = store.getState();
  if (!incomingRequest || !signalingRef) return;

  console.log('[APPROVAL] Accepting request from', incomingRequest.peerCode);

  // Transition session to connecting
  beginConnecting(incomingRequest.peerCode);

  // Send acceptance signal — the other side will initiate WebRTC
  signalingRef.sendSignal('connection_accepted', {}, incomingRequest.peerCode);
  store.setState({ incomingRequest: null, connectingTo: incomingRequest.peerCode });
}

function declineRequest() {
  const { incomingRequest } = store.getState();
  if (!incomingRequest || !signalingRef) return;

  console.log('[APPROVAL] Declining request from', incomingRequest.peerCode);
  signalingRef.sendSignal('connection_declined', { reason: 'user_declined' }, incomingRequest.peerCode);
  resetSession();
}

function cancelRequest() {
  const { connectingTo } = store.getState();
  if (!connectingTo || !signalingRef) return;

  console.log('[APPROVAL] Cancelling request to', connectingTo);
  signalingRef.sendSignal('connection_declined', { reason: 'cancelled' }, connectingTo);
  resetSession();
}

function disconnect() {
  if (rtcServiceRef) {
    rtcServiceRef.disconnect();
  }
  // Canonical reset — clears all state via session-state
  resetSession();
  setWebrtcRef(null);
  showToast('Disconnected', 'Connection closed successfully');
}

// ── Main Component ───────────────────────────────────────────────────────

export function createPeerConnection(): HTMLElement {
  const container = document.createElement('div');
  container.className = 'space-y-4';

  container.appendChild(createConnectionStatus());

  const touchWrap = document.createElement('div');
  touchWrap.className = 'touch-manipulation';
  touchWrap.appendChild(createDeviceDiscovery(
    selectPeer,
    disconnect,
    acceptRequest,
    declineRequest,
    cancelRequest,
  ));
  container.appendChild(touchWrap);

  // Initialize signaling + WebRTC
  const peerCode = generateSecurePeerCode();
  store.setState({ peerCode });
  console.log('[WEBRTC] Peer code:', peerCode);

  // Dual signaling: local (LAN) + cloud (internet)
  const localUrl = import.meta.env.VITE_SIGNAL_URL || `ws://${window.location.hostname}:3001`;
  const cloudUrl = import.meta.env.VITE_CLOUD_SIGNAL_URL as string | undefined;

  if (!cloudUrl) {
    console.warn('[SIGNALING] VITE_CLOUD_SIGNAL_URL not set — cloud signaling disabled, local-only mode');
  }

  const signaling = new DualSignaling(localUrl, cloudUrl ?? '');
  signalingRef = signaling;

  // Update header indicator when connection state changes
  signaling.setConnectionStateHandler(() => {
    store.setState({ signalingConnected: signaling.isConnected() });
  });

  signaling.onPeerDiscovered((peer) => {
    const { peers } = store.getState();
    if (peers.some((p) => p.peerCode === peer.peerCode)) return;
    store.setState({ peers: [...peers, peer] });
  });

  signaling.onPeerLost((lostCode) => {
    const { peers } = store.getState();
    store.setState({ peers: peers.filter((p) => p.peerCode !== lostCode) });

    // Clean up if the lost peer was involved in a pending request
    const currentPhase = getPhase();
    if (currentPhase === 'requesting' || currentPhase === 'incoming_request' || currentPhase === 'connecting') {
      const { connectingTo, incomingRequest } = store.getState();
      if (connectingTo === lostCode || incomingRequest?.peerCode === lostCode) {
        resetSession();
        showToast('Device Left', 'The device you were connecting to has left');
      }
    }
  });

  // Register connection approval signal handler
  signaling.onSignal(handleApprovalSignal);

  signaling.connect(peerCode, getDeviceName(), detectDeviceType()).then(() => {
    store.setState({ signalingConnected: true });
    const rtcService = new WebRTCService(
      signaling,
      peerCode,
      handleFileReceive,
      handleConnectionError,
      handleReceiveProgress,
    );
    rtcService.setConnectionStateHandler(handleConnectionStateChange);
    rtcServiceRef = rtcService;
    setWebrtcRef(rtcService);
  }).catch((err) => {
    console.error('[SIGNALING] Failed to connect:', err);
    store.setState({ signalingConnected: false });
  });

  return container;
}
