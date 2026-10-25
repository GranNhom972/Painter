// Entry point: loads the WebAssembly engine, then the application.

import init from '../pkg/paint_wasm.js';
import { App } from './app.js';

const wasm = await init();
window.paint = new App(wasm);
document.body.dataset.ready = 'true';

// Web version: installable and usable offline (not inside the desktop app).
if (!window.__TAURI__ && 'serviceWorker' in navigator && location.protocol.startsWith('http')) {
  navigator.serviceWorker.register('sw.js').catch((err) => console.warn('service worker', err));
}
