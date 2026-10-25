// Modal dialogs: resize & skew, image properties, colour editor, confirmations.

import { t } from './i18n.js';

function run(dialog, onOpen) {
  return new Promise((resolve) => {
    const form = dialog.querySelector('form');
    if (onOpen) onOpen(form);
    const done = () => {
      dialog.removeEventListener('close', done);
      resolve(dialog.returnValue);
    };
    dialog.returnValue = 'cancel';
    dialog.addEventListener('close', done);
    dialog.showModal();
  });
}

/** Resize & skew. `base` is the size of the selection or of the picture. */
export async function resizeDialog(base) {
  const dlg = document.getElementById('dlgResize');
  let result = null;
  const value = await run(dlg, (form) => {
    const f = form.elements;
    f.unit.value = 'percent';
    f.rw.value = 100;
    f.rh.value = 100;
    f.keep.checked = true;
    f.sx.value = 0;
    f.sy.value = 0;
    const sync = (from) => {
      if (!f.keep.checked) return;
      const pct = f.unit.value === 'percent';
      if (from === 'w') f.rh.value = pct ? f.rw.value : Math.max(1, Math.round((f.rw.value * base.h) / base.w));
      else f.rw.value = pct ? f.rh.value : Math.max(1, Math.round((f.rh.value * base.w) / base.h));
    };
    setTimeout(() => {
      f.rw.focus();
      f.rw.select();
    });
    f.rw.oninput = () => sync('w');
    f.rh.oninput = () => sync('h');
    for (const r of form.querySelectorAll('input[name=unit]')) {
      r.onchange = () => {
        const pct = f.unit.value === 'percent';
        f.rw.value = pct ? 100 : base.w;
        f.rh.value = pct ? 100 : base.h;
      };
    }
    form.onsubmit = () => {
      const pct = f.unit.value === 'percent';
      const w = Math.max(1, Math.round(pct ? (base.w * Number(f.rw.value)) / 100 : Number(f.rw.value)));
      const h = Math.max(1, Math.round(pct ? (base.h * Number(f.rh.value)) / 100 : Number(f.rh.value)));
      result = { w, h, sx: Math.max(-89, Math.min(89, Number(f.sx.value) || 0)), sy: Math.max(-89, Math.min(89, Number(f.sy.value) || 0)) };
    };
  });
  return value === 'ok' ? result : null;
}

export async function propertiesDialog(w, h) {
  const dlg = document.getElementById('dlgProps');
  let result = null;
  const value = await run(dlg, (form) => {
    form.elements.w.value = w;
    form.elements.h.value = h;
    form.onsubmit = () => {
      result = { w: Math.max(1, Math.min(16384, Math.round(Number(form.elements.w.value) || w))), h: Math.max(1, Math.min(16384, Math.round(Number(form.elements.h.value) || h))) };
    };
  });
  return value === 'ok' ? result : null;
}

// ----- colour editor ---------------------------------------------------------

function hexToRgb(hex) {
  const m = /^#?([0-9a-f]{6})/i.exec(hex);
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

const toHex = (r, g, b) => '#' + [r, g, b].map((v) => Math.round(v).toString(16).padStart(2, '0')).join('');

function rgbToHsv(r, g, b) {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  let h = 0;
  if (d) {
    if (max === r) h = 60 * (((g - b) / d) % 6);
    else if (max === g) h = 60 * ((b - r) / d + 2);
    else h = 60 * ((r - g) / d + 4);
  }
  return [(h + 360) % 360, max ? d / max : 0, max];
}

function hsvToRgb(h, s, v) {
  const c = v * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = v - c;
  const [r, g, b] = h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x] : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
  return [(r + m) * 255, (g + m) * 255, (b + m) * 255];
}

/** Colour editor; resolves to `#rrggbb` or null. */
export async function colorDialog(initial) {
  const dlg = document.getElementById('dlgColors');
  const sv = document.getElementById('svArea');
  const thumb = document.getElementById('svThumb');
  const hue = document.getElementById('hueSlider');
  const preview = document.getElementById('colorPreview');
  const hexIn = document.getElementById('hexInput');
  const rIn = document.getElementById('rInput');
  const gIn = document.getElementById('gInput');
  const bIn = document.getElementById('bInput');
  let [h, s, v] = rgbToHsv(...(hexToRgb(initial) || [0, 0, 0]));
  let current = initial;

  const paint = (skip) => {
    const [r, g, b] = hsvToRgb(h, s, v);
    current = toHex(r, g, b);
    sv.style.background = `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, transparent), hsl(${h}, 100%, 50%)`;
    thumb.style.left = `${s * 100}%`;
    thumb.style.top = `${(1 - v) * 100}%`;
    preview.style.background = current;
    if (skip !== 'hue') hue.value = Math.round(h);
    if (skip !== 'hex') hexIn.value = current;
    if (skip !== 'rgb') {
      rIn.value = Math.round(r);
      gIn.value = Math.round(g);
      bIn.value = Math.round(b);
    }
  };
  const fromRgb = (r, g, b, skip) => {
    [h, s, v] = rgbToHsv(r, g, b);
    paint(skip);
  };
  const pickSv = (e) => {
    const r = sv.getBoundingClientRect();
    s = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    v = 1 - Math.min(1, Math.max(0, (e.clientY - r.top) / r.height));
    paint();
  };
  sv.onpointerdown = (e) => {
    sv.setPointerCapture(e.pointerId);
    pickSv(e);
    sv.onpointermove = (ev) => pickSv(ev);
    sv.onpointerup = () => {
      sv.onpointermove = null;
    };
  };
  hue.oninput = () => {
    h = Number(hue.value);
    paint('hue');
  };
  hexIn.oninput = () => {
    const rgb = hexToRgb(hexIn.value);
    if (rgb && hexIn.value.replace('#', '').length === 6) fromRgb(...rgb, 'hex');
  };
  const channel = (el) => Math.min(255, Math.max(0, Math.round(+el.value || 0)));
  for (const el of [rIn, gIn, bIn]) {
    el.oninput = () => fromRgb(channel(rIn), channel(gIn), channel(bIn), 'rgb');
    // Out-of-range values snap back when the field is left.
    el.onchange = () => {
      el.value = channel(el);
    };
  }
  const value = await run(dlg, () => paint());
  return value === 'ok' ? current : null;
}

// ----- confirmations -----------------------------------------------------------

/** "Save changes?" → 'save' | 'discard' | 'cancel'. */
export async function confirmSave(name) {
  const dlg = document.getElementById('dlgConfirm');
  return run(dlg, () => {
    document.getElementById('confirmTitle').textContent = t('dlg.save.title');
    document.getElementById('confirmBody').textContent = t('dlg.save.body', { name });
  });
}

export async function message(title, body) {
  const dlg = document.getElementById('dlgMessage');
  await run(dlg, () => {
    document.getElementById('messageTitle').textContent = title;
    document.getElementById('messageBody').textContent = body;
  });
}
