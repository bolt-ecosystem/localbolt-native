/**
 * @vitest-environment jsdom
 */
import { describe, it, expect } from 'vitest';
import { computeUnifiedStatus } from '@/sections/header';
import type { WatchdogState, SignalStatus } from '@/services/daemon';

describe('computeUnifiedStatus', () => {
  // Daemon non-ready states dominate
  it('starting daemon shows STARTING regardless of signal', () => {
    const states: SignalStatus[] = ['unknown', 'active', 'degraded', 'offline'];
    for (const sig of states) {
      const result = computeUnifiedStatus('starting', sig);
      expect(result.label).toBe('STARTING');
    }
  });

  it('restarting daemon shows RESTARTING regardless of signal', () => {
    const result = computeUnifiedStatus('restarting', 'active');
    expect(result.label).toBe('RESTARTING');
  });

  it('degraded daemon shows DEGRADED regardless of signal', () => {
    const result = computeUnifiedStatus('degraded', 'active');
    expect(result.label).toBe('DEGRADED');
  });

  it('incompatible daemon shows INCOMPATIBLE regardless of signal', () => {
    const result = computeUnifiedStatus('incompatible', 'active');
    expect(result.label).toBe('INCOMPATIBLE');
  });

  // Daemon ready — signal determines unified status
  it('ready + active = HEALTHY', () => {
    const result = computeUnifiedStatus('ready', 'active');
    expect(result.label).toBe('HEALTHY');
    expect(result.dot).toContain('bg-neon');
  });

  it('ready + degraded = SIG DEGRADED', () => {
    const result = computeUnifiedStatus('ready', 'degraded');
    expect(result.label).toBe('SIG DEGRADED');
    expect(result.dot).toContain('bg-yellow');
  });

  it('ready + offline = SIG OFFLINE', () => {
    const result = computeUnifiedStatus('ready', 'offline');
    expect(result.label).toBe('SIG OFFLINE');
    expect(result.dot).toContain('bg-orange');
  });

  it('ready + unknown = STARTING', () => {
    const result = computeUnifiedStatus('ready', 'unknown');
    expect(result.label).toBe('STARTING');
    expect(result.dot).toContain('bg-gray');
  });
});
