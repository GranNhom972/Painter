// Entry point: loads the WebAssembly engine, then the application.

import init from '../pkg/paint_wasm.js';
import { App } from './app.js';

const wasm = await init();
window.paint = new App(wasm);
document.body.dataset.ready = 'true';
