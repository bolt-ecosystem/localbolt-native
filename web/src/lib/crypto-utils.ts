/**
 * Cryptographic utilities for LocalBolt
 * Uses Web Crypto API for secure random generation
 */

// Base32 alphabet without ambiguous characters (no 0/O, 1/I/L)
const ALPHABET = 'ABCDEFGHJKMNPQRSTUVWXYZ23456789';

/**
 * Generate a cryptographically secure peer code
 * Uses crypto.getRandomValues() instead of Math.random()
 * Returns 6-character code for backward compatibility
 */
export function generateSecurePeerCode(): string {
  const array = new Uint8Array(6);
  crypto.getRandomValues(array);

  let code = '';
  for (let i = 0; i < 6; i++) {
    code += ALPHABET[array[i] % ALPHABET.length];
  }

  return code;
}

/**
 * Generate a longer peer code with dash separator
 * Format: XXXX-XXXX (~40 bits of entropy)
 */
export function generateLongPeerCode(): string {
  const array = new Uint8Array(8);
  crypto.getRandomValues(array);

  let code = '';
  for (let i = 0; i < 8; i++) {
    code += ALPHABET[array[i] % ALPHABET.length];
    if (i === 3) code += '-';
  }

  return code;
}

/**
 * Validate peer code format
 */
export function isValidPeerCode(code: string): boolean {
  const normalized = code.replace(/-/g, '').toUpperCase();
  if (normalized.length !== 6 && normalized.length !== 8) {
    return false;
  }
  return normalized.split('').every(char => ALPHABET.includes(char));
}

/**
 * Normalize peer code for comparison
 */
export function normalizePeerCode(code: string): string {
  return code.replace(/-/g, '').toUpperCase();
}

/**
 * Compute SHA-256 hash of data
 */
export async function sha256(data: ArrayBuffer | Uint8Array): Promise<ArrayBuffer> {
  return await crypto.subtle.digest('SHA-256', data);
}

/**
 * Convert ArrayBuffer to hex string
 */
export function bufferToHex(buffer: ArrayBuffer): string {
  return Array.from(new Uint8Array(buffer))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');
}

/**
 * Compute SHA-256 hash of a file
 */
export async function hashFile(file: File): Promise<string> {
  const buffer = await file.arrayBuffer();
  const hash = await sha256(buffer);
  return bufferToHex(hash);
}
