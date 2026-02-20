/**
 * Platform utilities for LocalBolt
 * Includes device detection and local-only network configuration
 */

interface DeviceInfo {
  isWindows: boolean;
  isMac: boolean;
  isLinux: boolean;
  isAndroid: boolean;
  isIOS: boolean;
  isSteamDeck: boolean;
  isMobile: boolean;
  platform: string;
  deviceName: string;  // Friendly device name for display
}

export const detectDevice = (): DeviceInfo => {
  const userAgent = window.navigator.userAgent;
  const ua = userAgent.toLowerCase();
  const platform = window.navigator.platform?.toLowerCase() || '';

  const isWindows = platform.includes('win');
  const isLinux = platform.includes('linux') || /linux/i.test(userAgent);
  const isAndroid = /android/i.test(userAgent);
  const isIOS = /iphone|ipad|ipod/i.test(userAgent);
  // Steam Deck uses Linux but has a specific user agent
  const isSteamDeck = isLinux && ua.includes('steam');
  // iPad detection (iPadOS reports as Mac)
  const isIPad = /ipad/i.test(userAgent) || (platform.includes('mac') && 'ontouchend' in document);
  const isMac = (platform.includes('mac') || /macintosh/i.test(userAgent)) && !isIPad;
  const isMobile = isAndroid || isIOS || isIPad;

  // Generate friendly device name
  let deviceName = 'Device';
  if (/iphone/i.test(userAgent)) {
    deviceName = 'iPhone';
  } else if (isIPad) {
    deviceName = 'iPad';
  } else if (isAndroid) {
    // Try to extract device model
    if (/samsung|sm-/i.test(userAgent)) {
      deviceName = 'Samsung Galaxy';
    } else if (/pixel/i.test(userAgent)) {
      deviceName = 'Google Pixel';
    } else if (/oneplus/i.test(userAgent)) {
      deviceName = 'OnePlus';
    } else {
      deviceName = 'Android';
    }
  } else if (isSteamDeck) {
    deviceName = 'Steam Deck';
  } else if (isMac) {
    deviceName = 'Mac';
  } else if (isWindows) {
    deviceName = 'Windows PC';
  } else if (isLinux) {
    deviceName = 'Linux';
  }

  return {
    isWindows,
    isMac,
    isLinux: isLinux && !isSteamDeck,
    isAndroid,
    isIOS: isIOS || isIPad,
    isSteamDeck,
    isMobile,
    platform: isSteamDeck ? 'steam-deck' : platform,
    deviceName
  };
};

/**
 * Get friendly device name for display
 */
export const getDeviceName = (): string => {
  return detectDevice().deviceName;
};

export const getMaxChunkSize = (): number => {
  const device = detectDevice();

  // Adjust chunk sizes based on platform capabilities
  if (device.isMobile) {
    return 8192; // 8KB for mobile devices
  } else if (device.isSteamDeck) {
    return 32768; // 32KB for Steam Deck
  } else {
    return 16384; // 16KB default for desktop
  }
};

export const getPlatformICEServers = (): RTCIceServer[] => {
  // Same-network policy with robust connectivity:
  // use STUN for candidate discovery, but never allow relay transport.
  return [
    { urls: 'stun:stun.l.google.com:19302' },
    { urls: 'stun:stun1.l.google.com:19302' }
  ];
};

/**
 * Get local-only RTCConfiguration
 */
export const getLocalOnlyRTCConfig = (): RTCConfiguration => {
  return {
    iceServers: getPlatformICEServers(),
    iceCandidatePoolSize: 10,
    iceTransportPolicy: 'all',
    bundlePolicy: 'max-bundle',
    rtcpMuxPolicy: 'require'
  };
};

/**
 * Check if an IP address is a private/local network address
 */
export const isPrivateIP = (ip: string): boolean => {
  if (!ip) return false;

  const patterns = [
    /^10\./,                                    // 10.0.0.0/8
    /^172\.(1[6-9]|2[0-9]|3[0-1])\./,          // 172.16.0.0/12
    /^192\.168\./,                              // 192.168.0.0/16
    /^169\.254\./,                              // 169.254.0.0/16 (link-local)
    /^100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\./, // 100.64.0.0/10 (CGNAT/Tailscale/WireGuard)
    /^fc[0-9a-f]{2}:/i,                         // IPv6 ULA
    /^fd[0-9a-f]{2}:/i,                         // IPv6 ULA
    /^fe80:/i,                                  // IPv6 link-local
  ];

  return patterns.some(pattern => pattern.test(ip));
};

const parseCandidate = (candidateLine: string): { address?: string; type?: string } => {
  // RFC5245 candidate format:
  // candidate:<foundation> <component> <protocol> <priority> <address> <port> typ <type> ...
  const match = candidateLine.match(/^candidate:\S+\s+\d+\s+\S+\s+\d+\s+(\S+)\s+\d+\s+typ\s+(\S+)/);
  if (!match) return {};
  return {
    address: match[1],
    type: match[2]
  };
};

/**
 * Check if an ICE candidate is local-only
 */
export const isLocalCandidate = (candidate: RTCIceCandidate): boolean => {
  const parsed = parseCandidate(candidate.candidate || '');
  const candidateType = candidate.type || parsed.type;

  // Allow direct candidates only. Never allow TURN relay.
  return candidateType === 'host' || candidateType === 'srflx';
};
