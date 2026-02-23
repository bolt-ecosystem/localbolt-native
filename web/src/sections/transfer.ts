import { createPeerConnection } from '@/components/peer-connection';
import { createFileUpload, store } from '@the9ines/bolt-transport-web';

export function createTransfer(): HTMLElement {
  const card = document.createElement('div');
  card.className = `
    relative overflow-hidden p-8 md:p-10 max-w-2xl mx-auto space-y-6
    bg-dark-accent/40 backdrop-blur-xl border border-neon/20 rounded-xl
    shadow-[0_0_40px_rgba(164,226,0,0.12)] animate-fade-up
    transition-all duration-500
    hover:shadow-[0_0_60px_rgba(164,226,0,0.2)] hover:border-neon/40
  `;

  // Gradient overlays
  const gradientBr = document.createElement('div');
  gradientBr.className = 'absolute inset-0 bg-gradient-to-br from-neon/5 via-transparent to-transparent opacity-60';
  const gradientT = document.createElement('div');
  gradientT.className = 'absolute inset-0 bg-gradient-to-t from-dark/20 via-transparent to-transparent';

  // Title
  const titleWrap = document.createElement('div');
  titleWrap.className = 'relative space-y-2 text-center';
  titleWrap.innerHTML = `
    <h2 class="text-2xl font-semibold tracking-tight">Fast, Private File Transfer</h2>
    <p class="text-sm text-gray-400">Share files directly between devices on the same network</p>
  `;

  // Content area
  const content = document.createElement('div');
  content.className = 'relative';

  const peerConnectionEl = createPeerConnection();
  content.appendChild(peerConnectionEl);

  // File upload (shown when connected)
  const fileUploadWrap = document.createElement('div');
  fileUploadWrap.className = 'animate-fade-in mt-6';
  fileUploadWrap.hidden = true;

  const fileUploadEl = createFileUpload();
  fileUploadWrap.appendChild(fileUploadEl);
  content.appendChild(fileUploadWrap);

  store.subscribe(() => {
    const { isConnected } = store.getState();
    fileUploadWrap.hidden = !isConnected;
  });

  card.append(gradientBr, gradientT, titleWrap, content);
  return card;
}
