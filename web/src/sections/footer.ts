import { icons } from '@/ui/icons';

function createPrivacyDialog(): HTMLDialogElement {
  const dialog = document.createElement('dialog');
  dialog.className = 'max-w-2xl w-full max-h-[85vh] rounded-lg bg-dark/95 backdrop-blur-xl border border-neon/15 shadow-[0_0_40px_rgba(164,226,0,0.08)] text-white p-0';

  dialog.innerHTML = `
    <div class="p-6 space-y-5">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2.5">
          <div class="w-8 h-8 rounded-lg bg-neon/10 flex items-center justify-center">
            ${icons.shield('w-4 h-4 text-neon')}
          </div>
          <h2 class="text-lg font-semibold">Privacy</h2>
        </div>
        <button class="close-btn text-gray-500 hover:text-white transition-colors" aria-label="Close">
          ${icons.x('w-5 h-5')}
        </button>
      </div>

      <div class="overflow-y-auto max-h-[60vh] space-y-5 pr-2">
        <p class="text-sm text-gray-300 leading-relaxed">LocalBolt transfers files directly between devices. No data is collected or stored.</p>

        <div class="grid gap-3 sm:grid-cols-2">
          <div class="rounded-lg bg-white/[0.03] border border-white/5 p-4 space-y-3">
            <div class="flex items-center gap-2">
              ${icons.eye('w-3.5 h-3.5 text-neon/70')}
              <h3 class="text-xs font-semibold uppercase tracking-wider">Processed</h3>
            </div>
            <ul class="space-y-2">
              <li class="flex items-start gap-2 text-xs text-gray-400 leading-relaxed">
                <span class="w-1 h-1 rounded-full bg-neon/40 mt-1.5 flex-shrink-0"></span>
                Temporary signaling messages for connection setup
              </li>
            </ul>
          </div>
          <div class="rounded-lg bg-white/[0.03] border border-white/5 p-4 space-y-3">
            <div class="flex items-center gap-2">
              ${icons.eyeOff('w-3.5 h-3.5 text-neon/70')}
              <h3 class="text-xs font-semibold uppercase tracking-wider">Never Stored</h3>
            </div>
            <ul class="space-y-2">
              <li class="flex items-start gap-2 text-xs text-gray-400 leading-relaxed"><span class="w-1 h-1 rounded-full bg-gray-600 mt-1.5 flex-shrink-0"></span>File contents</li>
              <li class="flex items-start gap-2 text-xs text-gray-400 leading-relaxed"><span class="w-1 h-1 rounded-full bg-gray-600 mt-1.5 flex-shrink-0"></span>Transfer history</li>
              <li class="flex items-start gap-2 text-xs text-gray-400 leading-relaxed"><span class="w-1 h-1 rounded-full bg-gray-600 mt-1.5 flex-shrink-0"></span>Personal information</li>
            </ul>
          </div>
        </div>

        <div class="rounded-lg bg-white/[0.03] border border-white/5 p-4 space-y-2.5">
          <div class="flex items-center gap-2">
            ${icons.radio('w-3.5 h-3.5 text-neon/70')}
            <h3 class="text-xs font-semibold uppercase tracking-wider">How It Works</h3>
          </div>
          <p class="text-xs text-gray-400 leading-relaxed">Files travel directly between devices over encrypted WebRTC data channels. The signaling server only helps devices discover each other — it never sees your files.</p>
        </div>
      </div>
    </div>
  `;

  dialog.querySelector('.close-btn')!.addEventListener('click', () => dialog.close());
  dialog.addEventListener('click', (e) => {
    if (e.target === dialog) dialog.close();
  });

  return dialog;
}

export function createFooter(): HTMLElement {
  const footer = document.createElement('footer');
  footer.className = 'py-4 relative z-20';

  const privacyDialog = createPrivacyDialog();

  footer.innerHTML = `
    <div class="max-w-2xl mx-auto px-4 flex items-center justify-center gap-3 text-white/20" style="font-family:'JetBrains Mono',monospace; font-size:10px; letter-spacing:0.05em">
      <a href="https://github.com/the9ines/localbolt" target="_blank" rel="noopener noreferrer"
         class="hover:text-white/50 transition-colors">GitHub</a>
      <span class="text-white/[0.08]">/</span>
      <button class="privacy-btn hover:text-white/50 transition-colors">Privacy</button>
      <span class="text-white/[0.08]">/</span>
      <a href="https://the9ines.com" target="_blank" rel="noopener noreferrer"
         class="hover:text-[rgb(255,141,197)] transition-colors">the9ines</a>
    </div>
  `;

  footer.appendChild(privacyDialog);
  footer.querySelector('.privacy-btn')!.addEventListener('click', () => privacyDialog.showModal());

  return footer;
}
