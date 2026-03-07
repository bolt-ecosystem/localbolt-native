import { createPeerConnection } from '@/components/peer-connection';
import { createFileUpload, store } from '@the9ines/bolt-transport-web';
import { getVerificationState, onVerificationStateChange, isTransferAllowed } from '@the9ines/localbolt-core';
import { subscribeDaemonState, isDaemonReady, restartDaemon } from '@/services/daemon';

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

  // Daemon status banner (shown in non-ready states)
  const daemonBanner = document.createElement('div');
  daemonBanner.className = 'relative rounded-lg p-3 text-center text-sm';
  daemonBanner.hidden = true;

  const bannerText = document.createElement('span');
  const restartBtn = document.createElement('button');
  restartBtn.className =
    'ml-3 px-2 py-0.5 text-xs rounded border border-white/20 ' +
    'text-white/70 hover:bg-white/10 transition-colors';
  restartBtn.textContent = 'Retry';
  restartBtn.hidden = true;
  restartBtn.addEventListener('click', async () => {
    restartBtn.disabled = true;
    restartBtn.textContent = 'Restarting...';
    try {
      await restartDaemon();
    } catch {
      restartBtn.textContent = 'Retry';
      restartBtn.disabled = false;
    }
  });
  daemonBanner.append(bannerText, restartBtn);

  subscribeDaemonState((state) => {
    const { watchdog, retryCount } = state;
    if (watchdog === 'ready') {
      daemonBanner.hidden = true;
      restartBtn.hidden = true;
    } else if (watchdog === 'starting' || watchdog === 'restarting') {
      daemonBanner.hidden = false;
      daemonBanner.className = 'relative rounded-lg p-3 text-center text-sm bg-yellow-500/10 border border-yellow-500/20 text-yellow-300/80';
      bannerText.textContent = watchdog === 'starting' ? 'Daemon starting...' : `Daemon restarting (attempt ${retryCount}/3)...`;
      restartBtn.hidden = true;
    } else if (watchdog === 'degraded') {
      daemonBanner.hidden = false;
      daemonBanner.className = 'relative rounded-lg p-3 text-center text-sm bg-orange-500/10 border border-orange-500/20 text-orange-300/80';
      bannerText.textContent = 'Daemon unavailable — transfers disabled.';
      restartBtn.hidden = false;
      restartBtn.disabled = false;
      restartBtn.textContent = 'Retry';
    } else if (watchdog === 'incompatible') {
      daemonBanner.hidden = false;
      daemonBanner.className = 'relative rounded-lg p-3 text-center text-sm bg-red-500/10 border border-red-500/20 text-red-300/80';
      bannerText.textContent = 'Daemon version incompatible — please update the app.';
      restartBtn.hidden = true;
    }
  });

  // Content area
  const content = document.createElement('div');
  content.className = 'relative';

  const peerConnectionEl = createPeerConnection();
  content.appendChild(peerConnectionEl);

  // File upload (shown when connected AND transfer is allowed AND daemon ready)
  const fileUploadWrap = document.createElement('div');
  fileUploadWrap.className = 'animate-fade-in mt-6';
  fileUploadWrap.hidden = true;

  const fileUploadEl = createFileUpload();
  fileUploadWrap.appendChild(fileUploadEl);
  content.appendChild(fileUploadWrap);

  // Gate: file transfer requires daemon ready + connection + allowed verification state.
  // Policy (N6-A2 + C-pre-2 stabilization):
  //   daemon not ready → transfer BLOCKED (fail-closed)
  //   verified + connected + daemon ready → transfer allowed
  //   legacy + connected + daemon ready → transfer allowed
  //   unverified + connected → transfer BLOCKED (SAS pending)
  //   mismatch → transfer BLOCKED (fail-closed)
  function updateFileUploadVisibility() {
    const { isConnected } = store.getState();
    const vState = getVerificationState().state;
    const daemonOk = isDaemonReady();
    fileUploadWrap.hidden = !daemonOk || !isTransferAllowed(vState, isConnected);
  }

  store.subscribe(updateFileUploadVisibility);
  onVerificationStateChange(updateFileUploadVisibility);
  subscribeDaemonState(updateFileUploadVisibility);

  card.append(gradientBr, gradientT, titleWrap, daemonBanner, content);
  return card;
}
