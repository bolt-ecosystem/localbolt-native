import { store } from '@the9ines/bolt-transport-web';
import { subscribeDaemonState } from '@/services/daemon';
import type { WatchdogState, SignalStatus } from '@/services/daemon';

const DAEMON_STATUS_MAP: Record<WatchdogState, { dot: string; label: string }> = {
  starting: { dot: 'bg-yellow-500/70 animate-pulse', label: 'STARTING' },
  ready: { dot: 'bg-neon/70 animate-pulse', label: 'ACTIVE' },
  restarting: { dot: 'bg-yellow-500/70 animate-pulse', label: 'RESTARTING' },
  degraded: { dot: 'bg-orange-500/70', label: 'DEGRADED' },
  incompatible: { dot: 'bg-red-500/70', label: 'INCOMPATIBLE' },
};

const SIGNAL_STATUS_MAP: Record<SignalStatus, { dot: string; label: string }> = {
  unknown: { dot: 'bg-gray-500/70', label: 'SIGNAL ...' },
  active: { dot: 'bg-neon/70 animate-pulse', label: 'SIGNAL' },
  degraded: { dot: 'bg-yellow-500/70 animate-pulse', label: 'SIGNAL' },
  offline: { dot: 'bg-red-500/70', label: 'SIGNAL' },
};

/** Compute unified health label from daemon + signal state. */
export function computeUnifiedStatus(
  watchdog: WatchdogState,
  signalStatus: SignalStatus,
): { dot: string; label: string } {
  // Daemon non-ready states always dominate
  if (watchdog !== 'ready') {
    return DAEMON_STATUS_MAP[watchdog];
  }
  // Daemon ready — reflect signal health in unified indicator
  switch (signalStatus) {
    case 'active':
      return { dot: 'bg-neon/70 animate-pulse', label: 'HEALTHY' };
    case 'degraded':
      return { dot: 'bg-yellow-500/70 animate-pulse', label: 'SIG DEGRADED' };
    case 'offline':
      return { dot: 'bg-orange-500/70', label: 'SIG OFFLINE' };
    default:
      return { dot: 'bg-gray-500/70 animate-pulse', label: 'STARTING' };
  }
}

export function createHeader(): HTMLElement {
  const header = document.createElement('header');
  header.className = 'border-b border-white/[0.06] bg-dark/80 backdrop-blur-sm relative z-20';
  header.innerHTML = `
    <div class="max-w-2xl mx-auto px-4 flex h-12 items-center justify-between">
      <a href="/" class="flex items-center gap-2 group" onclick="event.preventDefault()">
        <svg class="w-4 h-4 text-neon transition-all duration-300 group-hover:fill-neon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10"/></svg>
        <span style="font-family:'JetBrains Mono',monospace" class="text-[13px] font-bold tracking-tight text-white/90">LocalBolt</span>
      </a>
      <div class="flex items-center gap-3">
        <div class="flex items-center gap-1.5">
          <div class="unified-dot w-1.5 h-1.5 rounded-full bg-yellow-500/70 animate-pulse"></div>
          <span style="font-family:'JetBrains Mono',monospace" class="unified-label text-[10px] text-white/30 tracking-widest">STARTING</span>
        </div>
        <div class="flex items-center gap-1.5">
          <div class="daemon-dot w-1.5 h-1.5 rounded-full bg-yellow-500/70 animate-pulse"></div>
          <span style="font-family:'JetBrains Mono',monospace" class="daemon-label text-[10px] text-white/30 tracking-widest">DAEMON</span>
        </div>
        <div class="flex items-center gap-1.5">
          <div class="signal-dot w-1.5 h-1.5 rounded-full bg-gray-500/70"></div>
          <span style="font-family:'JetBrains Mono',monospace" class="signal-label text-[10px] text-white/30 tracking-widest">SIGNAL ...</span>
        </div>
      </div>
    </div>
  `;

  const unifiedDot = header.querySelector('.unified-dot') as HTMLElement;
  const unifiedLabel = header.querySelector('.unified-label') as HTMLElement;
  const daemonDot = header.querySelector('.daemon-dot') as HTMLElement;
  const daemonLabel = header.querySelector('.daemon-label') as HTMLElement;
  const signalDot = header.querySelector('.signal-dot') as HTMLElement;
  const signalLabel = header.querySelector('.signal-label') as HTMLElement;

  // Signaling transport status (existing bolt-transport-web store)
  // Preserved: this drives the signal-dot alongside the new N8 signal monitor.
  store.subscribe(() => {
    const { signalingConnected } = store.getState();
    if (signalingConnected) {
      signalDot.className = 'signal-dot w-1.5 h-1.5 rounded-full bg-neon/70 animate-pulse';
      signalLabel.textContent = 'SIGNAL';
    } else {
      signalDot.className = 'signal-dot w-1.5 h-1.5 rounded-full bg-red-500/70';
      signalLabel.textContent = 'SIGNAL';
    }
  });

  // Daemon/watchdog + signal health (N8 unified status)
  subscribeDaemonState((state) => {
    // Individual daemon indicator
    const daemonMapping = DAEMON_STATUS_MAP[state.watchdog];
    daemonDot.className = `daemon-dot w-1.5 h-1.5 rounded-full ${daemonMapping.dot}`;
    daemonLabel.textContent = 'DAEMON';

    // Individual signal indicator (from N8 monitor)
    const signalMapping = SIGNAL_STATUS_MAP[state.signalStatus];
    signalDot.className = `signal-dot w-1.5 h-1.5 rounded-full ${signalMapping.dot}`;
    signalLabel.textContent = signalMapping.label;

    // Unified health indicator
    const unified = computeUnifiedStatus(state.watchdog, state.signalStatus);
    unifiedDot.className = `unified-dot w-1.5 h-1.5 rounded-full ${unified.dot}`;
    unifiedLabel.textContent = unified.label;
  });

  return header;
}
