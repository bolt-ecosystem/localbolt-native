import { icons } from '@/ui/icons';
import type { TransferProgress } from '@/services/webrtc/WebRTCService';

function formatSpeed(bytesPerSecond: number): string {
  if (bytesPerSecond === 0) return '0 B/s';
  const units = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
  const exp = Math.min(Math.floor(Math.log(bytesPerSecond) / Math.log(1024)), units.length - 1);
  return `${(bytesPerSecond / Math.pow(1024, exp)).toFixed(2)} ${units[exp]}`;
}

function formatTime(seconds: number): string {
  if (!isFinite(seconds) || seconds < 0) return 'calculating...';
  if (seconds === 0) return '0s';
  const hrs = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);
  const parts: string[] = [];
  if (hrs > 0) parts.push(`${hrs}h`);
  if (mins > 0) parts.push(`${mins}m`);
  if (secs > 0 || parts.length === 0) parts.push(`${secs}s`);
  return parts.join(' ');
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const exp = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / Math.pow(1024, exp)).toFixed(2)} ${units[exp]}`;
}

export function createTransferProgress(
  progress: TransferProgress,
  onCancel: () => void,
  onPause: () => void,
  onResume: () => void,
): HTMLElement {
  const isPaused = progress.status === 'paused';
  const isActive = progress.status === 'transferring' || progress.status === 'paused';
  const pct = progress.total > 0 ? Math.round((progress.loaded / progress.total) * 100) : 0;

  const wrap = document.createElement('div');
  wrap.className = 'space-y-2 w-full';

  wrap.innerHTML = `
    <div class="flex items-center gap-2 w-full bg-dark-accent rounded-lg p-3">
      ${icons.file('w-5 h-5 shrink-0 text-white/50')}
      <div class="flex flex-col flex-1 min-w-0">
        <span class="truncate text-sm">${progress.filename}</span>
        <span class="text-xs text-white/50">${formatSize(progress.loaded)} of ${formatSize(progress.total)} (${pct}%)</span>
      </div>
    </div>

    <div class="flex items-center gap-2 w-full">
      <div class="h-2 flex-1 bg-neon/20 rounded-full overflow-hidden">
        <div class="h-full bg-neon rounded-full transition-all duration-300" style="width: ${pct}%"></div>
      </div>
      ${isActive ? `
        <div class="flex items-center gap-1">
          <button class="pause-resume-btn h-8 w-8 inline-flex items-center justify-center rounded-md hover:text-neon" title="${isPaused ? 'Resume transfer' : 'Pause transfer'}">
            ${isPaused ? icons.play('h-4 w-4') : icons.pause('h-4 w-4')}
          </button>
          <button class="cancel-btn h-8 w-8 inline-flex items-center justify-center rounded-md hover:text-neon" title="Cancel transfer">
            ${icons.x('h-4 w-4')}
          </button>
        </div>
      ` : ''}
    </div>

    ${progress.stats ? `
      <div class="grid grid-cols-2 gap-2 text-xs text-gray-400">
        <div>Speed: ${formatSpeed(progress.stats.speed)}</div>
        <div>Avg: ${formatSpeed(progress.stats.averageSpeed)}</div>
        <div>${formatSize(progress.loaded)} / ${formatSize(progress.total)}</div>
        <div>${progress.stats.estimatedTimeRemaining > 0 ? `~${formatTime(progress.stats.estimatedTimeRemaining)} remaining` : 'Calculating...'}</div>
        ${progress.stats.retryCount > 0 ? `<div class="col-span-2 text-yellow-500">Retries: ${progress.stats.retryCount}/${progress.stats.maxRetries}</div>` : ''}
      </div>
    ` : ''}
  `;

  const pauseBtn = wrap.querySelector('.pause-resume-btn');
  if (pauseBtn) pauseBtn.addEventListener('click', isPaused ? onResume : onPause);
  const cancelBtn = wrap.querySelector('.cancel-btn');
  if (cancelBtn) cancelBtn.addEventListener('click', onCancel);

  return wrap;
}
