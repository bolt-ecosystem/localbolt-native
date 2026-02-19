import { createHeader } from '@/sections/header';
import { createTransfer } from '@/sections/transfer';
import { createFooter } from '@/sections/footer';

export function createApp(root: HTMLElement) {
  root.innerHTML = '';

  const wrapper = document.createElement('div');
  wrapper.className = 'min-h-screen bg-dark text-white flex flex-col';

  // Subtle centered glow behind the card
  const glow = document.createElement('div');
  glow.className = 'fixed inset-0 pointer-events-none z-0';
  glow.innerHTML = `
    <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[600px] bg-[radial-gradient(ellipse,rgba(164,226,0,0.035)_0%,transparent_65%)]"></div>
  `;

  wrapper.appendChild(glow);
  wrapper.appendChild(createHeader());

  // Main — transfer card vertically + horizontally centered
  const main = document.createElement('main');
  main.className = 'flex-1 flex items-center justify-center px-4 py-8 relative z-10';
  main.appendChild(createTransfer());

  wrapper.appendChild(main);
  wrapper.appendChild(createFooter());

  root.appendChild(wrapper);
}
