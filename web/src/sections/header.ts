import { store } from '@the9ines/bolt-transport-web';
import { subscribeDaemonState } from '@/services/daemon';
import type { WatchdogState } from '@/services/daemon';

const DAEMON_STATUS_MAP: Record<WatchdogState, { dot: string; label: string }> = {
  starting: { dot: 'bg-yellow-500/70 animate-pulse', label: 'STARTING' },
  ready: { dot: 'bg-neon/70 animate-pulse', label: 'ACTIVE' },
  restarting: { dot: 'bg-yellow-500/70 animate-pulse', label: 'RESTARTING' },
  degraded: { dot: 'bg-orange-500/70', label: 'DEGRADED' },
  incompatible: { dot: 'bg-red-500/70', label: 'INCOMPATIBLE' },
};

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
          <div class="daemon-dot w-1.5 h-1.5 rounded-full bg-yellow-500/70 animate-pulse"></div>
          <span style="font-family:'JetBrains Mono',monospace" class="daemon-label text-[10px] text-white/30 tracking-widest">STARTING</span>
        </div>
        <div class="flex items-center gap-1.5">
          <div class="status-dot w-1.5 h-1.5 rounded-full bg-red-500/70"></div>
          <span style="font-family:'JetBrains Mono',monospace" class="status-label text-[10px] text-white/30 tracking-widest">OFFLINE</span>
        </div>
      </div>
    </div>
  `;

  const dot = header.querySelector('.status-dot') as HTMLElement;
  const label = header.querySelector('.status-label') as HTMLElement;
  const daemonDot = header.querySelector('.daemon-dot') as HTMLElement;
  const daemonLabel = header.querySelector('.daemon-label') as HTMLElement;

  // Signaling status
  store.subscribe(() => {
    const { signalingConnected } = store.getState();
    if (signalingConnected) {
      dot.className = 'status-dot w-1.5 h-1.5 rounded-full bg-neon/70 animate-pulse';
      label.textContent = 'ACTIVE';
    } else {
      dot.className = 'status-dot w-1.5 h-1.5 rounded-full bg-red-500/70';
      label.textContent = 'OFFLINE';
    }
  });

  // Daemon/watchdog status
  subscribeDaemonState((state) => {
    const mapping = DAEMON_STATUS_MAP[state.watchdog];
    daemonDot.className = `daemon-dot w-1.5 h-1.5 rounded-full ${mapping.dot}`;
    daemonLabel.textContent = mapping.label;
  });

  return header;
}
