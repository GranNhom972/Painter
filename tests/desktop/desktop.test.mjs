// End-to-end tests of the real desktop application (Tauri + WebKitGTK),
// driven through WebDriver (tauri-driver + WebKitWebDriver).
// Run with scripts/test-desktop.sh (needs a display, e.g. Xvfb).
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const APP = resolve(process.env.PAINT_APP || 'target/debug/paint');
const DRIVER = process.env.TAURI_DRIVER || 'tauri-driver';
const NATIVE = process.env.WEBKIT_WEBDRIVER || 'WebKitWebDriver';
const PORT = 4444;
const BASE = `http://127.0.0.1:${PORT}`;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let driver;
let dir;

async function wd(method, path, body) {
  const res = await fetch(BASE + path, { method, headers: { 'content-type': 'application/json' }, body: body ? JSON.stringify(body) : undefined });
  const json = await res.json();
  if (!res.ok || (json.value && json.value.error)) throw new Error(`${method} ${path}: ${JSON.stringify(json.value)}`);
  return json.value;
}

async function session(args = []) {
  const v = await wd('POST', '/session', { capabilities: { alwaysMatch: { browserName: 'wry', 'tauri:options': { application: APP, args } } } });
  const id = v.sessionId;
  const s = {
    id,
    /** Runs `body` (an async function body) in the app and returns its result. */
    run: (body, ...args) =>
      wd('POST', `/session/${id}/execute/async`, {
        script: `const done = arguments[arguments.length - 1]; (async (...args) => { ${body} })(...Array.from(arguments).slice(0, -1)).then(done, (e) => done({ __error: String(e && e.stack || e) }));`,
        args,
      }).then((r) => {
        if (r && r.__error) throw new Error(r.__error);
        return r;
      }),
    actions: (actions) => wd('POST', `/session/${id}/actions`, { actions }),
    close: () => wd('DELETE', `/session/${id}`),
  };
  for (let i = 0; i < 100; i++) {
    if (await s.run('return document.body.dataset.ready === "true" && !!window.paint;').catch(() => false)) return s;
    await sleep(100);
  }
  throw new Error('application did not become ready');
}

before(async () => {
  dir = await mkdtemp(join(tmpdir(), 'paint-desktop-'));
  driver = spawn(DRIVER, ['--port', String(PORT), '--native-driver', NATIVE], { stdio: 'inherit' });
  for (let i = 0; i < 100; i++) {
    try {
      await fetch(`${BASE}/status`);
      return;
    } catch {
      await sleep(100);
    }
  }
  throw new Error('tauri-driver did not start');
});

after(() => driver && driver.kill());

test('desktop app boots with the WebAssembly engine and Tauri APIs', async () => {
  const s = await session();
  try {
    const info = await s.run(`return {
      tauri: !!window.__TAURI__,
      desktop: window.paint.platform.isDesktop,
      size: [window.paint.engine.width(), window.paint.engine.height()],
      title: document.title,
      wasm: typeof WebAssembly,
    };`);
    assert.deepEqual(info.size, [1152, 648]);
    assert.equal(info.tauri, true);
    assert.equal(info.desktop, true);
    assert.equal(info.wasm, 'object');
    assert.match(info.title, /Paint - /);
    const shot = await wd('GET', `/session/${s.id}/screenshot`);
    await writeFile(join(dir, 'desktop.png'), Buffer.from(shot, 'base64'));
    assert.ok(Buffer.from(shot, 'base64').length > 1000);
  } finally {
    await s.close();
  }
});

test('real mouse input draws in the WebKitGTK window', async () => {
  const s = await session();
  try {
    const at = await s.run(`
      window.paint.setTool('pencil');
      window.paint.flush();
      const r = document.getElementById('canvas').getBoundingClientRect();
      return { x: Math.round(r.left), y: Math.round(r.top) };`);
    await s.actions([
      {
        type: 'pointer',
        id: 'mouse',
        parameters: { pointerType: 'mouse' },
        actions: [
          { type: 'pointerMove', x: at.x + 20, y: at.y + 20, origin: 'viewport' },
          { type: 'pointerDown', button: 0 },
          { type: 'pointerMove', x: at.x + 120, y: at.y + 20, origin: 'viewport', duration: 100 },
          { type: 'pointerUp', button: 0 },
        ],
      },
    ]);
    const px = await s.run('return [window.paint.engine.pixel(70, 20), window.paint.engine.pixel(70, 40)];');
    assert.deepEqual(px, ['#000000', '#ffffff']);
  } finally {
    await s.close();
  }
});

test('copy and paste through the native (GTK) clipboard', async () => {
  const s = await session();
  try {
    const r = await s.run(`
      const p = window.paint, e = p.engine;
      e.set_shape('rectangle'); e.set_outline('none'); e.set_fill('solid'); e.set_color2('#ed1c24');
      e.pointer_down(10, 10, 0, false, false, false); e.pointer_move(49, 39, false, false, false); e.pointer_up(49, 39); e.enter();
      e.set_color2('#ffffff');
      e.select_rect(10, 10, 40, 30);
      const copied = await p.copy();
      const fromClipboard = await p.platform.clipboardReadImage();
      e.new_document(200, 100);
      p.clipboardFallback = null; // make sure the image really comes from the system clipboard
      const pasted = await p.paste();
      p.flush();
      return { copied, clip: fromClipboard ? fromClipboard.length : 0, pasted, sel: p.state.selection, px: e.pixel(5, 5) };`);
    assert.equal(r.copied, true);
    assert.ok(r.clip > 50, 'PNG read back from the system clipboard');
    assert.equal(r.pasted, true);
    assert.equal(r.sel.floating, true);
    assert.deepEqual([r.sel.w, r.sel.h], [40, 30]);
    assert.equal(r.px, '#ed1c24');
  } finally {
    await s.close();
  }
});

test('keyboard shortcuts copy to the system clipboard', async () => {
  const s = await session();
  try {
    await s.run(`window.paint.engine.set_color1('#22b14c'); window.paint.setTool('fill'); window.paint.flush();
      const r = document.getElementById('canvas').getBoundingClientRect();
      window.paint.engine.pointer_down(5, 5, 0, false, false, false); window.paint.engine.pointer_up(5, 5);
      window.paint.changed(); document.getElementById('viewport').focus(); return true;`);
    const key = (value) => [{ type: 'keyDown', value: '' }, { type: 'keyDown', value }, { type: 'keyUp', value }, { type: 'keyUp', value: '' }];
    await s.actions([{ type: 'key', id: 'kb', actions: [...key('a'), ...key('c')] }]);
    await sleep(300);
    const r = await s.run(`
      const bytes = await window.paint.platform.clipboardReadImage();
      window.paint.flush();
      return { sel: window.paint.state.selection, clip: bytes ? bytes.length : 0 };`);
    assert.deepEqual([r.sel.w, r.sel.h], [1152, 648]);
    assert.ok(r.clip > 100);
  } finally {
    await s.close();
  }
});

test('save writes a real file, open reads it back', async () => {
  const s = await session();
  const path = join(dir, 'dessin été.png');
  try {
    const r = await s.run(`
      const p = window.paint, e = p.engine;
      e.new_document(64, 32);
      e.set_color1('#3f48cc'); e.set_tool('fill'); e.pointer_down(1, 1, 0, false, false, false); e.pointer_up(1, 1);
      const bytes = e.export('png', 100);
      const fmt = p.platform.FORMATS[0];
      const saved = await p.platform.saveBytes(bytes, 'x.png', fmt, arguments[0]);
      e.new_document(10, 10);
      const f = await p.platform.readPath(arguments[0]);
      p.loadFile(f);
      p.flush();
      return { saved, size: [e.width(), e.height()], px: e.pixel(3, 3), title: document.title };`, path);
    assert.equal(r.saved.name, 'dessin été.png');
    assert.deepEqual(r.size, [64, 32]);
    assert.equal(r.px, '#3f48cc');
    assert.match(r.title, /dessin été\.png/);
    const disk = await readFile(path);
    assert.equal(disk.subarray(1, 4).toString(), 'PNG');
    assert.equal(disk.readUInt32BE(16), 64);
  } finally {
    await s.close();
  }
});

test('the extension of the chosen file decides the saved format', async () => {
  const s = await session();
  const path = join(dir, 'photo.jpg');
  try {
    // PNG was the format picked, but the file name says .jpg: JPEG bytes are written.
    const r = await s.run(`
      const p = window.paint;
      const ok = await p.write(p.platform.FORMATS[0], arguments[0], 'photo.jpg');
      return { ok, format: p.file.format.ext };`, path);
    assert.equal(r.ok, true);
    assert.equal(r.format, 'jpg');
    const disk = await readFile(path);
    assert.deepEqual([...disk.subarray(0, 2)], [0xff, 0xd8]);
  } finally {
    await s.close();
  }
});

test('a file given on the command line is opened at start', async () => {
  const path = join(dir, 'start.png');
  // 3x2 PNG made by the save test's engine is not available here: write one with the app first.
  const s1 = await session();
  await s1.run(`const e = window.paint.engine; e.new_document(33, 22);
    await window.paint.platform.saveBytes(e.export('png', 100), 'start.png', window.paint.platform.FORMATS[0], arguments[0]); return true;`, path);
  await s1.close();
  const s = await session([path]);
  try {
    let r;
    for (let i = 0; i < 50; i++) {
      r = await s.run('window.paint.flush(); return [window.paint.engine.width(), window.paint.engine.height(), document.title];');
      if (r[0] === 33) break;
      await sleep(100);
    }
    assert.deepEqual(r.slice(0, 2), [33, 22]);
    assert.match(r[2], /start\.png/);
  } finally {
    await s.close();
  }
});

test('closing with unsaved changes asks for confirmation', async () => {
  const s = await session();
  try {
    const r = await s.run(`
      const e = window.paint.engine;
      e.set_tool('pencil'); e.pointer_down(3, 3, 0, false, false, false); e.pointer_up(3, 3);
      window.__TAURI__.window.getCurrentWindow().close();
      for (let i = 0; i < 50 && !document.getElementById('dlgConfirm').open; i++) await new Promise((r) => setTimeout(r, 50));
      const open = document.getElementById('dlgConfirm').open;
      document.querySelector('[data-testid=confirm-cancel]').click();
      await new Promise((r) => setTimeout(r, 100));
      return { open, stillHere: !!window.paint, closed: document.getElementById('dlgConfirm').open };`);
    assert.equal(r.open, true);
    assert.equal(r.stillHere, true);
    assert.equal(r.closed, false);
  } finally {
    await s.close();
  }
});
