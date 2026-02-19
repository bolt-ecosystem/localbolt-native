import { icons } from '@/ui/icons';
import { store } from '@/state/store';

export function createConnectionStatus(): HTMLElement {
  const wrap = document.createElement('div');
  wrap.className = 'flex items-center justify-center space-x-2 text-neon mb-4';

  const iconSpan = document.createElement('span');
  const label = document.createElement('span');
  label.className = 'text-sm';
  label.textContent = 'End-to-End Encrypted';

  wrap.append(iconSpan, label);

  function render() {
    const { isConnected } = store.getState();
    iconSpan.innerHTML = isConnected
      ? icons.shieldFilled('w-5 h-5 text-neon')
      : icons.shield('w-5 h-5 text-neon');
  }

  store.subscribe(render);
  render();
  return wrap;
}
