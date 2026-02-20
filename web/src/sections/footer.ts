export function createFooter(): HTMLElement {
  const footer = document.createElement('footer');
  footer.className = 'py-4 relative z-20';

  footer.innerHTML = `
    <div class="max-w-2xl mx-auto px-4 flex items-center justify-center text-white/20" style="font-family:'JetBrains Mono',monospace; font-size:10px; letter-spacing:0.05em">
      <a href="https://the9ines.com" target="_blank" rel="noopener noreferrer"
         class="hover:text-[rgb(255,141,197)] transition-colors">the9ines</a>
    </div>
  `;

  return footer;
}
