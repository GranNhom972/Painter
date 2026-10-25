// Desktop (Tauri) and browser implementations of clipboard, files and window.

const tauri = window.__TAURI__;
export const isDesktop = !!(tauri && tauri.core);

const invoke = (cmd, args, opts) => tauri.core.invoke(cmd, args, opts);

export const FORMATS = [
  { ext: 'png', mime: 'image/png', name: 'PNG' },
  { ext: 'jpg', mime: 'image/jpeg', name: 'JPEG', alias: ['jpeg', 'jpe', 'jfif'] },
  { ext: 'bmp', mime: 'image/bmp', name: 'BMP', alias: ['dib'] },
  { ext: 'gif', mime: 'image/gif', name: 'GIF' },
  { ext: 'tiff', mime: 'image/tiff', name: 'TIFF', alias: ['tif'] },
  { ext: 'webp', mime: 'image/webp', name: 'WEBP' },
  { ext: 'ico', mime: 'image/x-icon', name: 'ICO' },
];

export function formatOf(name) {
  const ext = (name.split('.').pop() || '').toLowerCase();
  return FORMATS.find((f) => f.ext === ext || (f.alias || []).includes(ext)) || null;
}

const allExtensions = FORMATS.flatMap((f) => [f.ext, ...(f.alias || [])]);

function toBytes(data) {
  if (data instanceof Uint8Array) return data;
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (Array.isArray(data)) return new Uint8Array(data);
  return null;
}

// ----- clipboard ---------------------------------------------------------------

/** Puts a PNG image on the system clipboard. */
export async function clipboardWritePng(png) {
  if (isDesktop) {
    await invoke('clipboard_write_png', png);
    return;
  }
  const blob = new Blob([png], { type: 'image/png' });
  await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
}

/** Encoded image bytes from the system clipboard, or null. */
export async function clipboardReadImage() {
  if (isDesktop) {
    const data = await invoke('clipboard_read_png');
    const bytes = toBytes(data);
    return bytes && bytes.length ? bytes : null;
  }
  if (!navigator.clipboard || !navigator.clipboard.read) return null;
  const items = await navigator.clipboard.read();
  for (const item of items) {
    const type = item.types.find((t) => t.startsWith('image/'));
    if (type) {
      const blob = await item.getType(type);
      return new Uint8Array(await blob.arrayBuffer());
    }
  }
  return null;
}

// ----- files -----------------------------------------------------------------

function pickFileInBrowser(accept) {
  return new Promise((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.style.display = 'none';
    input.addEventListener('change', async () => {
      const file = input.files && input.files[0];
      input.remove();
      if (!file) return resolve(null);
      resolve({ name: file.name, path: null, bytes: new Uint8Array(await file.arrayBuffer()) });
    });
    input.addEventListener('cancel', () => {
      input.remove();
      resolve(null);
    });
    document.body.appendChild(input);
    input.click();
  });
}

/** Asks for an image file and reads it: { name, path, bytes } or null. */
export async function openImageFile(filterName) {
  if (isDesktop) {
    const path = await tauri.dialog.open({ multiple: false, directory: false, filters: [{ name: filterName, extensions: allExtensions }] });
    if (!path) return null;
    return readPath(path);
  }
  return pickFileInBrowser(FORMATS.map((f) => f.mime).join(',') + ',.' + allExtensions.join(',.'));
}

export async function readPath(path) {
  const data = await invoke('read_file', { path });
  return { name: baseName(path), path, bytes: toBytes(data) };
}

export function baseName(path) {
  return String(path).split(/[\\/]/).pop();
}

/**
 * Saves bytes. With `path` (desktop) it writes directly; otherwise it asks for
 * a destination. Returns { name, path } or null when cancelled.
 */
export async function saveBytes(bytes, suggestedName, format, path = null) {
  if (isDesktop) {
    let target = path;
    if (!target) {
      target = await tauri.dialog.save({ defaultPath: suggestedName, filters: [{ name: format.name, extensions: [format.ext, ...(format.alias || [])] }] });
      if (!target) return null;
      if (!formatOf(target)) target += '.' + format.ext;
    }
    await invoke('write_file', bytes, { headers: { 'x-path': encodeURIComponent(target) } });
    return { name: baseName(target), path: target };
  }
  const blob = new Blob([bytes], { type: format.mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = suggestedName;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10000);
  return { name: suggestedName, path: null };
}

/** File given on the command line ("Open with Paint"), desktop only. */
export async function initialFile() {
  if (!isDesktop) return null;
  const path = await invoke('initial_file');
  return path ? readPath(path) : null;
}

// ----- window ----------------------------------------------------------------

export function setTitle(title) {
  document.title = title;
  if (isDesktop) tauri.window.getCurrentWindow().setTitle(title).catch(() => {});
}

/** `handler()` returns true when the window may close. */
export function onCloseRequested(handler) {
  if (isDesktop) {
    const win = tauri.window.getCurrentWindow();
    win.onCloseRequested(async (event) => {
      event.preventDefault();
      if (await handler()) await win.destroy();
    });
  } else {
    window.addEventListener('beforeunload', (e) => {
      if (handler.isDirty && handler.isDirty()) {
        e.preventDefault();
        e.returnValue = '';
      }
    });
  }
}

export async function quit(handler) {
  if (isDesktop) {
    if (await handler()) await tauri.window.getCurrentWindow().destroy();
  } else if (await handler()) {
    window.close();
  }
}

export async function toggleFullscreen() {
  if (isDesktop) {
    const win = tauri.window.getCurrentWindow();
    await win.setFullscreen(!(await win.isFullscreen()));
  } else if (document.fullscreenElement) {
    await document.exitFullscreen();
  } else {
    await document.documentElement.requestFullscreen();
  }
}
