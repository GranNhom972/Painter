// Text tool: an editable text box over the canvas, rasterised into the
// active layer (engine.stamp_rgba) when it is committed.

const FONTS = [
  'Segoe UI', 'Arial', 'Calibri', 'Cambria', 'Comic Sans MS', 'Courier New', 'DejaVu Sans', 'DejaVu Serif',
  'Georgia', 'Impact', 'Liberation Sans', 'Liberation Serif', 'Noto Sans', 'Noto Serif', 'Times New Roman',
  'Trebuchet MS', 'Ubuntu', 'Verdana', 'sans-serif', 'serif', 'monospace',
];

export class TextTool {
  constructor(app) {
    this.app = app;
    this.box = document.getElementById('textBox');
    this.bar = document.getElementById('textBar');
    this.fontSel = document.getElementById('textFont');
    this.sizeInput = document.getElementById('textSize');
    this.toggles = {
      bold: document.getElementById('textBold'),
      italic: document.getElementById('textItalic'),
      underline: document.getElementById('textUnderline'),
      strike: document.getElementById('textStrike'),
      opaque: document.getElementById('textOpaque'),
    };
    this.opts = { font: 'Segoe UI', size: 24, bold: false, italic: false, underline: false, strike: false, opaque: false };
    this.pos = null; // image coordinates of the box
    this.active = false;
    for (const f of FONTS) {
      const o = document.createElement('option');
      o.value = o.textContent = f;
      this.fontSel.appendChild(o);
    }
    this.fontSel.value = this.opts.font;
    this.fontSel.addEventListener('change', () => this.set('font', this.fontSel.value));
    this.sizeInput.addEventListener('input', () => {
      const v = Number(this.sizeInput.value);
      if (v >= 6 && v <= 400) this.set('size', v);
    });
    for (const [k, el] of Object.entries(this.toggles)) {
      el.addEventListener('click', () => this.set(k, !this.opts[k]));
    }
    document.getElementById('textApply').addEventListener('click', () => this.commit());
    this.box.addEventListener('input', () => this.autosize());
    this.box.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Escape') {
        e.preventDefault();
        this.cancel();
      } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        this.commit();
      } else if ((e.ctrlKey || e.metaKey) && ['b', 'i', 'u'].includes(e.key.toLowerCase())) {
        e.preventDefault();
        const k = { b: 'bold', i: 'italic', u: 'underline' }[e.key.toLowerCase()];
        this.set(k, !this.opts[k]);
      }
    });
    this.box.addEventListener('pointerdown', (e) => e.stopPropagation());
  }

  set(key, value) {
    this.opts[key] = value;
    this.syncControls();
    this.style();
    if (this.active) this.box.focus();
  }

  syncControls() {
    for (const [k, el] of Object.entries(this.toggles)) {
      el.classList.toggle('on', !!this.opts[k]);
      el.setAttribute('aria-pressed', String(!!this.opts[k]));
    }
    this.fontSel.value = this.opts.font;
    this.sizeInput.value = this.opts.size;
  }

  fontString(scale = 1) {
    const o = this.opts;
    return `${o.italic ? 'italic ' : ''}${o.bold ? 'bold ' : ''}${o.size * scale}px "${o.font}"`;
  }

  style() {
    const st = this.app.state;
    const b = this.box;
    const z = this.app.view.zoom;
    b.style.font = this.fontString(z);
    b.style.lineHeight = '1.2';
    b.style.color = st ? st.color1 : '#000';
    b.style.background = this.opts.opaque && st ? st.color2 : 'transparent';
    const deco = [this.opts.underline && 'underline', this.opts.strike && 'line-through'].filter(Boolean).join(' ');
    b.style.textDecoration = deco || 'none';
    this.autosize();
  }

  setVisible(toolIsText) {
    this.bar.hidden = !toolIsText;
    if (!toolIsText && this.active) this.commit();
  }

  pointerDown(p) {
    if (this.active) {
      this.commit();
      return;
    }
    this.start(p);
  }

  start(p) {
    this.app.commitTransient();
    this.pos = { x: Math.floor(p.x), y: Math.floor(p.y) };
    this.active = true;
    this.box.value = '';
    this.box.hidden = false;
    this.style();
    this.layout();
    this.box.focus({ preventScroll: true });
  }

  layout() {
    if (!this.active) return;
    const z = this.app.view.zoom;
    this.box.style.left = `${this.pos.x * z - 1}px`;
    this.box.style.top = `${this.pos.y * z - 1}px`;
    this.style();
  }

  measure(text = this.box.value) {
    const ctx = document.createElement('canvas').getContext('2d');
    ctx.font = this.fontString();
    const lines = text.split('\n');
    const width = Math.max(this.opts.size * 0.6, ...lines.map((l) => ctx.measureText(l).width));
    return { lines, width: Math.ceil(width + this.opts.size * 0.25), height: Math.ceil(lines.length * this.opts.size * 1.2) };
  }

  autosize() {
    if (!this.active) return;
    const z = this.app.view.zoom;
    const m = this.measure();
    this.box.style.width = `${m.width * z + 2}px`;
    this.box.style.height = `${m.height * z + 2}px`;
  }

  cancel() {
    this.active = false;
    this.box.hidden = true;
    this.box.value = '';
  }

  /** Rasterises the text and stamps it into the active layer. */
  commit() {
    if (!this.active) return;
    const text = this.box.value;
    this.cancel();
    if (!text.trim() && !this.opts.opaque) return;
    const st = this.app.state;
    const m = this.measure(text);
    const cv = document.createElement('canvas');
    cv.width = Math.max(1, m.width);
    cv.height = Math.max(1, m.height);
    const ctx = cv.getContext('2d');
    if (this.opts.opaque) {
      ctx.fillStyle = st.color2;
      ctx.fillRect(0, 0, cv.width, cv.height);
    }
    ctx.font = this.fontString();
    ctx.fillStyle = st.color1;
    ctx.textBaseline = 'middle';
    const lh = this.opts.size * 1.2;
    const thick = Math.max(1, Math.round(this.opts.size / 15));
    m.lines.forEach((line, i) => {
      const mid = i * lh + lh / 2;
      ctx.fillText(line, 0, mid);
      const w = ctx.measureText(line).width;
      if (this.opts.underline) ctx.fillRect(0, Math.round(mid + this.opts.size * 0.42), w, thick);
      if (this.opts.strike) ctx.fillRect(0, Math.round(mid), w, thick);
    });
    const data = ctx.getImageData(0, 0, cv.width, cv.height);
    this.app.engine.stamp_rgba(this.pos.x, this.pos.y, cv.width, cv.height, new Uint8Array(data.data.buffer));
    this.app.changed({ layers: true });
  }
}
