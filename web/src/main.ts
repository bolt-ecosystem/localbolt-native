import './index.css';
import { initProtocolWasm } from '@the9ines/bolt-transport-web';
import { createApp } from './app';

// BR5: Initialize Rust/WASM protocol authority from embedded artifact.
// Falls back silently to TS tweetnacl/BTR if WASM unavailable (PM-RB-03).
initProtocolWasm().then(() => {
  createApp(document.getElementById('root')!);
});
