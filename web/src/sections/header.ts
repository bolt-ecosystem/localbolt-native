import { store } from '@the9ines/bolt-transport-web';

export function createHeader(): HTMLElement {
  const header = document.createElement('header');
  header.className = 'border-b border-white/[0.06] bg-dark/80 backdrop-blur-sm relative z-20';
  header.innerHTML = `
    <div class="max-w-2xl mx-auto px-4 flex h-12 items-center justify-between">
      <a href="/" class="flex items-center gap-2 group" onclick="event.preventDefault()">
        <svg class="w-4 h-4 text-neon transition-all duration-300 group-hover:fill-neon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10"/></svg>
        <span style="font-family:'JetBrains Mono',monospace" class="text-[13px] font-bold tracking-tight text-white/90">LocalBolt</span>
      </a>
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
