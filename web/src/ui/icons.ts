// Inline SVG icons from Lucide. Each returns an SVG string.
// Usage: el.innerHTML = icons.shield('w-5 h-5 text-neon');

function svg(paths: string, cls = '', viewBox = '0 0 24 24'): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="${cls}">${paths}</svg>`;
}

function filled(paths: string, cls = '', viewBox = '0 0 24 24'): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}" fill="currentColor" class="${cls}">${paths}</svg>`;
}

export const icons = {
  zap: (cls = '') => svg('<polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>', cls),
  shield: (cls = '') => svg('<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/>', cls),
  shieldFilled: (cls = '') => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="${cls}"><path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/></svg>`,
  wifi: (cls = '') => svg('<path d="M12 20h.01"/><path d="M2 8.82a15 15 0 0 1 20 0"/><path d="M5 12.859a10 10 0 0 1 14 0"/><path d="M8.5 16.429a5 5 0 0 1 7 0"/>', cls),
  laptop: (cls = '') => svg('<path d="M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45L4 16"/>', cls),
  server: (cls = '') => svg('<rect width="20" height="8" x="2" y="2" rx="2" ry="2"/><rect width="20" height="8" x="2" y="14" rx="2" ry="2"/><line x1="6" x2="6.01" y1="6" y2="6"/><line x1="6" x2="6.01" y1="18" y2="18"/>', cls),
  lock: (cls = '') => svg('<rect width="18" height="11" x="3" y="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>', cls),
  globe: (cls = '') => svg('<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>', cls),
  clock: (cls = '') => svg('<circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>', cls),
  arrowDown: (cls = '') => svg('<path d="M12 5v14"/><path d="m19 12-7 7-7-7"/>', cls),
  share2: (cls = '') => svg('<circle cx="18" cy="5" r="3"/><circle cx="6" cy="12" r="3"/><circle cx="18" cy="19" r="3"/><line x1="8.59" x2="15.42" y1="13.51" y2="17.49"/><line x1="15.41" x2="8.59" y1="6.51" y2="10.49"/>', cls),
  smartphone: (cls = '') => svg('<rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/>', cls),
  tablet: (cls = '') => svg('<rect width="16" height="20" x="4" y="2" rx="2" ry="2"/><line x1="12" x2="12.01" y1="18" y2="18"/>', cls),
  monitor: (cls = '') => svg('<rect width="20" height="14" x="2" y="3" rx="2"/><line x1="8" x2="16" y1="21" y2="21"/><line x1="12" x2="12" y1="17" y2="21"/>', cls),
  upload: (cls = '') => svg('<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" x2="12" y1="3" y2="15"/>', cls),
  file: (cls = '') => svg('<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/>', cls),
  pause: (cls = '') => svg('<rect width="4" height="16" x="6" y="4"/><rect width="4" height="16" x="14" y="4"/>', cls),
  play: (cls = '') => svg('<polygon points="6 3 20 12 6 21 6 3"/>', cls),
  x: (cls = '') => svg('<path d="M18 6 6 18"/><path d="m6 6 12 12"/>', cls),
  copy: (cls = '') => svg('<rect width="14" height="14" x="8" y="8" rx="2" ry="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>', cls),
  check: (cls = '') => svg('<path d="M20 6 9 17l-5-5"/>', cls),
  eye: (cls = '') => svg('<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0"/><circle cx="12" cy="12" r="3"/>', cls),
  eyeOff: (cls = '') => svg('<path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49"/><path d="M14.084 14.158a3 3 0 0 1-4.242-4.242"/><path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143"/><path d="m2 2 20 20"/>', cls),
  radio: (cls = '') => svg('<circle cx="12" cy="12" r="2"/><path d="M4.93 19.07A10 10 0 0 1 2 12C2 6.48 6.48 2 12 2s10 4.48 10 10a10 10 0 0 1-2.93 7.07"/><path d="M7.76 16.24A6 6 0 0 1 6 12c0-3.31 2.69-6 6-6s6 2.69 6 6a6 6 0 0 1-1.76 4.24"/>', cls),
  messageCircle: (cls = '') => svg('<path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z"/>', cls),
  userX: (cls = '') => svg('<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><line x1="17" x2="22" y1="8" y2="13"/><line x1="22" x2="17" y1="8" y2="13"/>', cls),
};
