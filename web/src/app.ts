import { createHeader } from '@/sections/header';
import { createTransfer } from '@/sections/transfer';
import { createFooter } from '@/sections/footer';

export function createApp(root: HTMLElement) {
  root.innerHTML = '';

  const wrapper = document.createElement('div');
  wrapper.className = 'min-h-screen bg-dark text-white flex flex-col';

  // Pulsating grid background centered on card
  const bgContainer = document.createElement('div');
  bgContainer.className = 'fixed inset-0 pointer-events-none z-0';
  bgContainer.style.maskImage = 'linear-gradient(to bottom, white 60%, transparent 100%)';
  bgContainer.style.webkitMaskImage = 'linear-gradient(to bottom, white 60%, transparent 100%)';
  const radialBg = document.createElement('div');
  radialBg.className = 'absolute inset-0 bg-[radial-gradient(circle_at_50%_50%,rgba(164,226,0,0.07),rgba(0,0,0,0))] animate-pulse';
  const gridBg = document.createElement('div');
  gridBg.className = "absolute inset-0 bg-[url('/grid.svg')] bg-center [mask-image:radial-gradient(white,transparent_80%)]";
  bgContainer.append(radialBg, gridBg);

  wrapper.appendChild(bgContainer);
  wrapper.appendChild(createHeader());

  // Main — transfer card vertically + horizontally centered
  const main = document.createElement('main');
  main.className = 'flex-1 flex items-center justify-center px-4 py-8 relative z-10';
  main.appendChild(createTransfer());

  wrapper.appendChild(main);
  wrapper.appendChild(createFooter());

  root.appendChild(wrapper);
}
