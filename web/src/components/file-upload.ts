import { icons } from '@/ui/icons';
import { store } from '@/state/store';
import { showToast } from '@/ui/toast';
import { escapeHTML } from '@/lib/sanitize';
import { createTransferProgress } from './transfer-progress';
import type WebRTCService from '@/services/webrtc/WebRTCService';
import type { TransferProgress } from '@/services/webrtc/WebRTCService';

let webrtcRef: WebRTCService | null = null;

export function setWebrtcRef(service: WebRTCService | null) {
  webrtcRef = service;
}

export function createFileUpload(): HTMLElement {
  const container = document.createElement('div');
  container.className = 'space-y-4';

  // Drag-drop area
  const dropZone = document.createElement('div');
  dropZone.className = 'border-2 border-dashed rounded-lg p-8 text-center transition-all duration-200 border-white/10 hover:border-white/20';

  const fileInput = document.createElement('input');
  fileInput.type = 'file';
  fileInput.multiple = true;
  fileInput.className = 'hidden';

  dropZone.innerHTML = `
    <div class="space-y-4">
      ${icons.upload('w-12 h-12 mx-auto text-white/50')}
      <div>
        <p class="text-lg font-medium">Drop files here</p>
        <p class="text-sm text-white/50">or click to select files</p>
      </div>
    </div>
  `;
  dropZone.appendChild(fileInput);

  const selectBtn = document.createElement('button');
  selectBtn.className = 'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium h-10 px-4 py-2 border border-white/10 bg-dark-accent hover:bg-white/5 transition-colors';
  selectBtn.textContent = 'Select Files';
  selectBtn.addEventListener('click', () => fileInput.click());
  dropZone.querySelector('.space-y-4')!.appendChild(selectBtn);

  container.appendChild(dropZone);

  // File list + progress + send button area
  const actionArea = document.createElement('div');
  actionArea.className = 'space-y-4 animate-fade-up';
  actionArea.hidden = true;
  container.appendChild(actionArea);

  const fileListEl = document.createElement('div');
  fileListEl.className = 'space-y-2';
  actionArea.appendChild(fileListEl);

  const progressEl = document.createElement('div');
  actionArea.appendChild(progressEl);

  const sendBtn = document.createElement('button');
  sendBtn.className = 'w-full h-10 rounded-md text-sm font-medium bg-neon text-black hover:bg-neon/90 transition-colors disabled:opacity-50 disabled:pointer-events-none';
  sendBtn.textContent = 'Start Transfer';
  actionArea.appendChild(sendBtn);

  // Local state
  let files: File[] = [];
  let progress: TransferProgress | null = null;

  function addFiles(newFiles: File[]) {
    files = [...files, ...newFiles];
    showToast('Files added', `${newFiles.length} file(s) ready to transfer`);
    renderActions();
  }

  function removeFile(index: number) {
    files = files.filter((_, i) => i !== index);
    renderActions();
  }

  function renderFileList() {
    fileListEl.innerHTML = '';
    files.forEach((file, i) => {
      if (progress && file.name === progress.filename) return;
      const row = document.createElement('div');
      row.className = 'flex items-center justify-between p-3 bg-dark-accent rounded-lg';
      row.innerHTML = `
        <div class="flex items-center space-x-3 min-w-0 flex-1">
          ${icons.file('w-5 h-5 shrink-0 text-white/50')}
          <span class="text-sm truncate pr-2">${escapeHTML(file.name)}</span>
        </div>
      `;
      if (!progress) {
        const removeBtn = document.createElement('button');
        removeBtn.className = 'h-10 w-10 inline-flex items-center justify-center rounded-md text-white/50 hover:text-neon shrink-0';
        removeBtn.innerHTML = icons.x('w-4 h-4');
        removeBtn.addEventListener('click', () => removeFile(i));
        row.appendChild(removeBtn);
      }
      fileListEl.appendChild(row);
    });
  }

  function renderProgress() {
    progressEl.innerHTML = '';
    if (progress && progress.status) {
      const el = createTransferProgress(
        progress,
        () => { if (webrtcRef && progress) webrtcRef.cancelTransfer(progress.filename); },
        () => { if (webrtcRef && progress) webrtcRef.pauseTransfer(progress.filename); },
        () => { if (webrtcRef && progress) webrtcRef.resumeTransfer(progress.filename); },
      );
      progressEl.appendChild(el);
    }
  }

  function renderActions() {
    actionArea.hidden = files.length === 0 && !progress;
    sendBtn.disabled = !!progress;
    renderFileList();
    renderProgress();
  }

  function handleProgress(p: TransferProgress) {
    console.log('[TRANSFER] Progress update:', p);
    progress = p;

    if (p.status === 'canceled_by_sender' || p.status === 'canceled_by_receiver') {
      files = files.filter((f) => f.name !== p.filename);
      progress = null;
      showToast('Transfer cancelled', 'The file transfer was cancelled');
    } else if (p.status === 'error') {
      files = files.filter((f) => f.name !== p.filename);
      progress = null;
      showToast('Transfer error', 'An error occurred during the transfer', 'destructive');
    } else if (p.status === 'completed') {
      files = files.filter((f) => f.name !== p.filename);
      showToast('Transfer complete', `${p.filename} has been sent successfully`);
      setTimeout(() => { progress = null; renderActions(); }, 2000);
    }

    renderActions();
  }

  async function startTransfer() {
    if (!webrtcRef || files.length === 0) return;
    const file = files[0];
    console.log('Starting transfer for:', file.name);
    webrtcRef.setProgressCallback(handleProgress);
    try {
      await webrtcRef.sendFile(file);
      showToast('Transfer complete', `${file.name} has been sent successfully`);
      files = files.slice(1);
      progress = null;
      renderActions();
    } catch (error: any) {
      console.error('Transfer error:', error);
      if (error.message !== 'Transfer cancelled by user') {
        showToast('Transfer failed', 'Failed to send file', 'destructive');
      }
      progress = null;
      renderActions();
    }
  }

  sendBtn.addEventListener('click', startTransfer);

  // Drag events
  dropZone.addEventListener('dragenter', (e) => { e.preventDefault(); e.stopPropagation(); dropZone.classList.add('border-neon', 'bg-neon/5'); dropZone.classList.remove('border-white/10'); });
  dropZone.addEventListener('dragleave', (e) => { e.preventDefault(); e.stopPropagation(); dropZone.classList.remove('border-neon', 'bg-neon/5'); dropZone.classList.add('border-white/10'); });
  dropZone.addEventListener('dragover', (e) => { e.preventDefault(); e.stopPropagation(); });
  dropZone.addEventListener('drop', (e) => {
    e.preventDefault();
    e.stopPropagation();
    dropZone.classList.remove('border-neon', 'bg-neon/5');
    dropZone.classList.add('border-white/10');
    addFiles(Array.from(e.dataTransfer?.files || []));
  });
  fileInput.addEventListener('change', () => {
    addFiles(Array.from(fileInput.files || []));
    fileInput.value = '';
  });

  // Also handle incoming file receives (receiver side progress from store)
  store.subscribe(() => {
    const { transferProgress } = store.getState();
    if (transferProgress && transferProgress !== progress) {
      progress = transferProgress;
      renderActions();
    }
  });

  return container;
}
