import { store } from '@/state/store';

export function createHeader(): HTMLElement {
  const header = document.createElement('header');
  header.className = 'border-b border-white/[0.06] bg-dark/80 backdrop-blur-sm relative z-20';
  header.innerHTML = `
    <div class="max-w-2xl mx-auto px-4 flex h-12 items-center justify-between">
      <div class="flex items-center">
        <img src="/logo.svg" alt="LocalBolt" class="h-5" />
      </div>
      <div class="flex items-center gap-1.5">
        <div class="status-dot w-1.5 h-1.5 rounded-full bg-red-500/70"></div>
        <span style="font-family:'JetBrains Mono',monospace" class="status-label text-[10px] text-white/30 tracking-widest">OFFLINE</span>
      </div>
    </div>
  `;

  const dot = header.querySelector('.status-dot') as HTMLElement;
  const label = header.querySelector('.status-label') as HTMLElement;

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

  return header;
}
