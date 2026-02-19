// ─── Device Detection Utility ──────────────────────────────────────────────
// Detects device type and generates a human-readable device name
// from the browser's user agent string. Used for peer discovery display.

import type { DiscoveredDevice } from './SignalingProvider';

/**
 * Detect whether the current device is a phone, tablet, laptop, or desktop.
 * Uses navigator.userAgent and screen dimensions for classification.
 */
export function detectDeviceType(): DiscoveredDevice['deviceType'] {
  const ua = navigator.userAgent;
  const screenWidth = window.screen?.width || 0;

  // Check for mobile/tablet user agents
  const isMobile = /Mobi|Android|iPhone|iPod/i.test(ua);
  const isTablet =
    /iPad/i.test(ua) ||
    (/Macintosh/i.test(ua) && 'ontouchend' in document) || // iPadOS
    (/Android/i.test(ua) && !isMobile);

  if (isTablet) return 'tablet';
  if (isMobile) return 'phone';

  // Desktop browsers: distinguish laptop vs desktop is unreliable,
  // so default to 'laptop' for web clients (most common form factor).
  // Tauri native apps can override this with actual hardware detection.
  if (screenWidth >= 2560) return 'desktop'; // Large monitor likely a desktop
  return 'laptop';
}

/**
 * Generate a clean device name like "iPhone", "Mac", "Windows PC".
 * Parses navigator.userAgent for the device/OS only — no browser prefix.
 */
export function getDeviceName(): string {
  const ua = navigator.userAgent;

  if (/iPhone/i.test(ua)) return 'iPhone';
  if (/iPad/i.test(ua) || (/Macintosh/i.test(ua) && 'ontouchend' in document)) return 'iPad';
  if (/Android/i.test(ua)) return 'Android';
  if (/CrOS/i.test(ua)) return 'Chromebook';
  if (/Windows/i.test(ua)) return 'Windows PC';
  if (/Mac OS X|Macintosh/i.test(ua)) return 'Mac';
  if (/Linux/i.test(ua)) return 'Linux';

  return 'Device';
}
