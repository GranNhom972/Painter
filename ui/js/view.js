// Canvas view: blits the engine frame, zoom/scroll, pointer input, overlay
// (marching ants, handles, eraser cursor, grid), rulers and canvas resize handles.

const ZOOM_MIN = 0.1;
const ZOOM_MAX = 8;
export const ZOOM_PRESETS = [0.125, 0.25, 0.5, 1, 2, 3, 4, 5, 6, 7, 8];

const cursorSvg = (body, hx, hy) =>
  `url("data:image/svg+xml,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'>${body}</svg>`)}") ${hx} ${hy}, crosshair`;
const outline = (d) => `<path d='${d}' fill='none' stroke='white' stroke-width='3' stroke-linejoin='round'/><path d='${d}' fill='none' stroke='black' stroke-width='1.3' stroke-linejoin='round'/>`;
const CURSORS = {
  pencil: cursorSvg(outline('M2 22 4 16 17 3a2 2 0 0 1 3 3L7 19z M15 5 18 8'), 2, 22),
  fill: cursorSvg(outline('M10 4 18 12 11 19a2 2 0 0 1-3 0L3 14a2 2 0 0 1 0-3z M4 12h14 M20 15c1 2 2 3 2 4a2 2 0 0 1-4 0c0-1 1-2 2-4z'), 20, 21),
  picker: cursorSvg(outline('M2 22l2-5 9-9 3 3-9 9z M14 4a2.5 2.5 0 0 1 4 0l2 2a2.5 2.5 0 0 1 0 4l-2 1-5-5z'), 2, 22),
  brush: 'crosshair',
  eraser: 'none',
  text: 'text',
  'zoom-in': 'zoom-in',
};

export class View {
  constructor(app) {
    this.app = app;
    this.viewport = document.getElementById('viewport');
    this.scroller = document.getElementById('scroller');
    this.frameEl = document.getElementById('canvasFrame');
    this.canvas = document.getElementById('canvas');
    this.ctx = this.canvas.getContext('2d');
    this.overlay = document.getElementById('overlay');
    this.octx = this.overlay.getContext('2d');
    this.rulerH = document.getElementById('rulerH');
    this.rulerV = document.getElementById('rulerV');
    this.zoom = 1;
    this.pointer = null; // last pointer position in image coordinates
    this.dragging = false;
    this.canvasDrag = null;
    this.pan = null;
    this.ants = 0;
    this.airbrushTimer = null;
    this.showGrid = false;
    this.showRulers = false;
    this.bind();
  }

  get engine() {
    return this.app.engine;
  }

  bind() {
    const vp = this.viewport;
    vp.addEventListener('pointerdown', (e) => this.onPointerDown(e));
    vp.addEventListener('pointermove', (e) => this.onPointerMove(e));
    vp.addEventListener('pointerup', (e) => this.onPointerUp(e));
    // A cancelled pointer (touch scroll, lost capture) ends strokes but never
    // applies a canvas resize.
    vp.addEventListener('pointercancel', (e) => {
      if (!this.cancelCanvasResize()) this.onPointerUp(e);
    });
    vp.addEventListener('pointerleave', () => {
      if (!this.dragging && !this.canvasDrag) {
        this.pointer = null;
        this.app.updatePointerStatus(null);
        this.drawOverlay();
      }
    });
    vp.addEventListener('dblclick', (e) => {
      if (this.app.state.tool === 'shape') {
        const p = this.toImage(e.clientX, e.clientY);
        this.engine.double_click(p.x, p.y);
        this.app.changed();
      }
    });
    vp.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const tool = this.app.state.tool;
      if ((tool === 'select-rect' || tool === 'select-free') && !this.suppressContext) {
        this.app.showContextMenu(e.clientX, e.clientY);
      }
      this.suppressContext = false;
    });
    vp.addEventListener('wheel', (e) => {
      if (e.ctrlKey || e.metaKey) {
        e.preventDefault();
        const factor = Math.exp(-e.deltaY * 0.0025);
        this.setZoom(this.zoom * factor, e.clientX, e.clientY);
      }
    }, { passive: false });
    vp.addEventListener('scroll', () => this.drawOverlay());
    new ResizeObserver(() => this.sizeOverlay()).observe(vp);
    this.frameEl.querySelectorAll('.canvas-handle').forEach((h) => {
      h.addEventListener('pointerdown', (e) => this.startCanvasResize(e, h.dataset.h));
    });
    setInterval(() => {
      const st = this.app.state;
      if (st && (st.selection || (st.shape_edit && st.shape_edit.stage === 'editing'))) {
        this.ants = (this.ants + 1) % 8;
        this.drawOverlay();
      }
    }, 140);
  }

  // ----- coordinates ---------------------------------------------------------

  toImage(clientX, clientY) {
    const r = this.canvas.getBoundingClientRect();
    return { x: (clientX - r.left) / this.zoom, y: (clientY - r.top) / this.zoom };
  }

  /** Image-space point to overlay (viewport) pixels. */
  toOverlay(x, y) {
    const c = this.canvas.getBoundingClientRect();
    const v = this.viewport.getBoundingClientRect();
    return { x: c.left - v.left + x * this.zoom, y: c.top - v.top + y * this.zoom };
  }

  /** Top-left image pixel currently visible (where Paint pastes). */
  visibleOrigin() {
    const c = this.canvas.getBoundingClientRect();
    const v = this.viewport.getBoundingClientRect();
    return {
      x: Math.max(0, Math.floor((v.left - c.left) / this.zoom)),
      y: Math.max(0, Math.floor((v.top - c.top) / this.zoom)),
    };
  }

  // ----- rendering -----------------------------------------------------------

  /** Copies the engine's dirty rectangle from WebAssembly memory to the canvas. */
  blit() {
    const e = this.engine;
    const w = e.width();
    const h = e.height();
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
      e.invalidate();
      this.layout();
    }
    const r = e.render();
    if (r.length === 4) {
      const data = new Uint8ClampedArray(this.app.wasm.memory.buffer, e.frame_ptr(), e.frame_len());
      this.ctx.putImageData(new ImageData(data, w, h), 0, 0, r[0], r[1], r[2], r[3]);
    }
  }

  layout() {
    const w = this.engine.width() * this.zoom;
    const h = this.engine.height() * this.zoom;
    this.frameEl.style.width = `${w}px`;
    this.frameEl.style.height = `${h}px`;
    // Crisp pixels whenever one image pixel covers two device pixels or more (zoom, HiDPI).
    this.canvas.classList.toggle('pixelated', this.zoom * (window.devicePixelRatio || 1) >= 2);
    this.app.text.layout();
    this.drawOverlay();
  }

  sizeOverlay() {
    const dpr = window.devicePixelRatio || 1;
    const w = this.viewport.clientWidth;
    const h = this.viewport.clientHeight;
    this.overlay.width = Math.round(w * dpr);
    this.overlay.height = Math.round(h * dpr);
    this.overlay.style.width = `${w}px`;
    this.overlay.style.height = `${h}px`;
    this.drawOverlay();
  }

  drawOverlay() {
    const o = this.overlay;
    const ctx = this.octx;
    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, o.width, o.height);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const st = this.app.state;
    if (!st) return;
    if (this.showGrid) this.drawGrid(ctx);
    if (st.selection) this.drawSelection(ctx, st.selection);
    if (st.shape_edit) this.drawShapeEdit(ctx, st.shape_edit);
    if (this.canvasDrag) this.drawCanvasPreview(ctx);
    if (this.pointer && !this.canvasDrag && (st.tool === 'eraser' || st.tool === 'brush')) this.drawToolCursor(ctx, st);
    if (this.showRulers) this.drawRulers();
  }

  /** Theme accent colour, for handles and previews drawn on the overlay. */
  accent() {
    return getComputedStyle(document.documentElement).getPropertyValue('--accent').trim() || '#005fb8';
  }

  drawGrid(ctx) {
    const w = this.engine.width();
    const h = this.engine.height();
    // At least 8 screen pixels between lines; every 10th line is stronger.
    const step = [1, 2, 5, 10, 20, 50, 100].find((s) => s * this.zoom >= 8) || 100;
    const a = this.toOverlay(0, 0);
    const b = this.toOverlay(w, h);
    const vw = this.viewport.clientWidth;
    const vh = this.viewport.clientHeight;
    ctx.save();
    ctx.lineWidth = 1;
    for (const major of [false, true]) {
      ctx.strokeStyle = major ? 'rgba(128,128,140,.5)' : 'rgba(128,128,140,.28)';
      ctx.beginPath();
      const x0 = Math.max(0, Math.floor((0 - a.x) / this.zoom / step) * step);
      for (let x = x0; x <= w; x += step) {
        if ((x % (step * 10) === 0) !== major) continue;
        const sx = Math.round(a.x + x * this.zoom) + 0.5;
        if (sx > vw) break;
        ctx.moveTo(sx, Math.max(0, a.y));
        ctx.lineTo(sx, Math.min(vh, b.y));
      }
      const y0 = Math.max(0, Math.floor((0 - a.y) / this.zoom / step) * step);
      for (let y = y0; y <= h; y += step) {
        if ((y % (step * 10) === 0) !== major) continue;
        const sy = Math.round(a.y + y * this.zoom) + 0.5;
        if (sy > vh) break;
        ctx.moveTo(Math.max(0, a.x), sy);
        ctx.lineTo(Math.min(vw, b.x), sy);
      }
      ctx.stroke();
    }
    ctx.restore();
  }

  antsPath(ctx, pts, closed) {
    ctx.beginPath();
    pts.forEach(([x, y], i) => {
      const p = this.toOverlay(x, y);
      const px = Math.round(p.x) + 0.5;
      const py = Math.round(p.y) + 0.5;
      if (i === 0) ctx.moveTo(px, py);
      else ctx.lineTo(px, py);
    });
    if (closed) ctx.closePath();
    ctx.lineWidth = 1;
    ctx.setLineDash([]);
    ctx.strokeStyle = '#ffffff';
    ctx.stroke();
    ctx.setLineDash([4, 4]);
    ctx.lineDashOffset = -this.ants;
    ctx.strokeStyle = '#1b1b1f';
    ctx.stroke();
    ctx.setLineDash([]);
  }

  handle(ctx, x, y, round = false) {
    const p = this.toOverlay(x, y);
    ctx.beginPath();
    if (round) ctx.arc(p.x, p.y, 4, 0, Math.PI * 2);
    else ctx.rect(Math.round(p.x) - 3.5, Math.round(p.y) - 3.5, 7, 7);
    ctx.fillStyle = '#ffffff';
    ctx.fill();
    ctx.strokeStyle = this.accent();
    ctx.lineWidth = 1;
    ctx.stroke();
  }

  drawSelection(ctx, s) {
    ctx.save();
    if (s.freeform && !s.drawing) {
      // Bounding box, dimmed, then the lasso outline.
      const r = [[s.x, s.y], [s.x + s.w, s.y], [s.x + s.w, s.y + s.h], [s.x, s.y + s.h]];
      ctx.globalAlpha = 0.45;
      this.antsPath(ctx, r, true);
      ctx.globalAlpha = 1;
    }
    for (const path of s.outline) this.antsPath(ctx, path, !(s.drawing && s.freeform));
    if (!s.drawing) {
      const { x, y, w, h } = s;
      for (const [hx, hy] of [[x, y], [x + w / 2, y], [x + w, y], [x + w, y + h / 2], [x + w, y + h], [x + w / 2, y + h], [x, y + h], [x, y + h / 2]]) {
        this.handle(ctx, hx, hy);
      }
    }
    ctx.restore();
  }

  drawShapeEdit(ctx, s) {
    ctx.save();
    if (s.stage === 'editing' && (s.kind !== 'line' && s.kind !== 'curve')) {
      this.antsPath(ctx, [[s.x, s.y], [s.x + s.w, s.y], [s.x + s.w, s.y + s.h], [s.x, s.y + s.h]], true);
    }
    if (s.kind === 'curve' && s.stage === 'editing') {
      const [a, b, c, d] = s.points;
      ctx.setLineDash([3, 3]);
      ctx.strokeStyle = 'rgba(0,95,184,.7)';
      ctx.beginPath();
      const pa = this.toOverlay(...a);
      const pb = this.toOverlay(...b);
      const pc = this.toOverlay(...c);
      const pd = this.toOverlay(...d);
      ctx.moveTo(pa.x, pa.y);
      ctx.lineTo(pb.x, pb.y);
      ctx.moveTo(pd.x, pd.y);
      ctx.lineTo(pc.x, pc.y);
      ctx.stroke();
      ctx.setLineDash([]);
    }
    const round = s.kind === 'line' || s.kind === 'curve';
    for (const h of s.handles) this.handle(ctx, h.x, h.y, round);
    ctx.restore();
  }

  drawToolCursor(ctx, st) {
    const size = st.size;
    const p = this.pointer;
    ctx.save();
    if (st.tool === 'eraser') {
      const half = Math.max(size, 1) / 2;
      const x0 = Math.round(p.x - half);
      const y0 = Math.round(p.y - half);
      const n = Math.max(1, Math.round(size));
      const a = this.toOverlay(x0, y0);
      ctx.strokeStyle = '#000';
      ctx.lineWidth = 1;
      ctx.strokeRect(Math.round(a.x) + 0.5, Math.round(a.y) + 0.5, n * this.zoom, n * this.zoom);
      ctx.strokeStyle = '#fff';
      ctx.strokeRect(Math.round(a.x) + 1.5, Math.round(a.y) + 1.5, n * this.zoom - 2, n * this.zoom - 2);
    } else if (size * this.zoom >= 6) {
      const c = this.toOverlay(p.x, p.y);
      ctx.beginPath();
      ctx.arc(c.x, c.y, (size * this.zoom) / 2, 0, Math.PI * 2);
      ctx.strokeStyle = 'rgba(0,0,0,.6)';
      ctx.stroke();
      ctx.beginPath();
      ctx.arc(c.x, c.y, (size * this.zoom) / 2 + 1, 0, Math.PI * 2);
      ctx.strokeStyle = 'rgba(255,255,255,.6)';
      ctx.stroke();
    }
    ctx.restore();
  }

  drawCanvasPreview(ctx) {
    const d = this.canvasDrag;
    const a = this.toOverlay(-d.ox, -d.oy);
    ctx.save();
    ctx.setLineDash([4, 3]);
    ctx.strokeStyle = this.accent();
    ctx.lineWidth = 1;
    ctx.strokeRect(Math.round(a.x) + 0.5, Math.round(a.y) + 0.5, d.w * this.zoom, d.h * this.zoom);
    ctx.restore();
  }

  drawRulers() {
    const dpr = window.devicePixelRatio || 1;
    const draw = (cv, horizontal) => {
      const len = horizontal ? cv.clientWidth : cv.clientHeight;
      cv.width = Math.round((horizontal ? len : 20) * dpr);
      cv.height = Math.round((horizontal ? 20 : len) * dpr);
      const ctx = cv.getContext('2d');
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      const css = getComputedStyle(document.documentElement);
      ctx.fillStyle = css.getPropertyValue('--ruler-bg');
      ctx.fillRect(0, 0, horizontal ? len : 20, horizontal ? 20 : len);
      ctx.strokeStyle = ctx.fillStyle = css.getPropertyValue('--ruler-text');
      ctx.font = `10px ${css.getPropertyValue('--font')}`;
      const origin = this.toOverlay(0, 0);
      const start = horizontal ? origin.x : origin.y;
      const steps = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000];
      const major = steps.find((s) => s * this.zoom >= 50) || 5000;
      const minor = major / (major % 10 === 0 ? 10 : 5);
      const first = Math.floor(-start / this.zoom / minor) * minor;
      ctx.beginPath();
      for (let v = first; ; v += minor) {
        const pos = start + v * this.zoom;
        if (pos > len) break;
        if (pos < 0) continue;
        const isMajor = Math.abs(v % major) < 1e-6;
        const tick = isMajor ? 10 : 4;
        const s = Math.round(pos) + 0.5;
        if (horizontal) {
          ctx.moveTo(s, 20);
          ctx.lineTo(s, 20 - tick);
          if (isMajor) ctx.fillText(String(v), s + 2, 9);
        } else {
          ctx.moveTo(20, s);
          ctx.lineTo(20 - tick, s);
          if (isMajor) {
            ctx.save();
            ctx.translate(9, s + 2);
            ctx.rotate(-Math.PI / 2);
            ctx.fillText(String(v), -ctx.measureText(String(v)).width - 2, 0);
            ctx.restore();
          }
        }
      }
      ctx.stroke();
      if (this.pointer) {
        const p = start + (horizontal ? this.pointer.x : this.pointer.y) * this.zoom;
        ctx.strokeStyle = this.accent();
        ctx.beginPath();
        if (horizontal) {
          ctx.moveTo(p, 0);
          ctx.lineTo(p, 20);
        } else {
          ctx.moveTo(0, p);
          ctx.lineTo(20, p);
        }
        ctx.stroke();
      }
    };
    draw(this.rulerH, true);
    draw(this.rulerV, false);
  }

  setRulers(on) {
    this.showRulers = on;
    this.rulerH.hidden = this.rulerV.hidden = document.getElementById('rulerCorner').hidden = !on;
    document.querySelector('.workspace').classList.toggle('with-rulers', on);
    this.sizeOverlay();
  }

  setGrid(on) {
    this.showGrid = on;
    this.drawOverlay();
  }

  updateCursor() {
    if (this.canvasDrag || this.pan) return;
    const st = this.app.state;
    if (!this.pointer || !st) return;
    const name = this.engine.cursor_at(this.pointer.x, this.pointer.y);
    const css = CURSORS[name] || name;
    if (this.viewport.style.cursor !== css) this.viewport.style.cursor = css;
    this.viewport.dataset.cursor = name;
  }

  // ----- zoom ------------------------------------------------------------------

  setZoom(z, clientX, clientY) {
    z = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z));
    if (Math.abs(z - 1) < 0.005) z = 1;
    const vr = this.viewport.getBoundingClientRect();
    const cx = clientX ?? vr.left + vr.width / 2;
    const cy = clientY ?? vr.top + vr.height / 2;
    const before = this.toImage(cx, cy);
    this.zoom = z;
    this.engine.set_zoom(z);
    this.layout();
    // Keep the image point under the anchor where it was.
    const after = this.toImage(cx, cy);
    this.viewport.scrollLeft += (before.x - after.x) * z;
    this.viewport.scrollTop += (before.y - after.y) * z;
    this.app.updateZoomUi();
    this.drawOverlay();
  }

  zoomStep(dir, clientX, clientY) {
    const z = this.zoom;
    const next = dir > 0 ? ZOOM_PRESETS.find((p) => p > z + 1e-6) : [...ZOOM_PRESETS].reverse().find((p) => p < z - 1e-6);
    if (next) this.setZoom(next, clientX, clientY);
  }

  fit() {
    const w = this.engine.width();
    const h = this.engine.height();
    const z = Math.min((this.viewport.clientWidth - 48) / w, (this.viewport.clientHeight - 48) / h);
    this.setZoom(Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z)));
    this.viewport.scrollLeft = 0;
    this.viewport.scrollTop = 0;
  }

  /** 100% when the picture fits the window, otherwise fit to window. */
  fitIfLarge() {
    const w = this.engine.width();
    const h = this.engine.height();
    if (w <= this.viewport.clientWidth - 48 && h <= this.viewport.clientHeight - 48) {
      this.setZoom(1);
      this.viewport.scrollLeft = 0;
      this.viewport.scrollTop = 0;
    } else {
      this.fit();
    }
  }

  // ----- pointer ---------------------------------------------------------------

  onPointerDown(e) {
    if (e.target.classList.contains('canvas-handle') || e.target === this.app.text.box) return;
    this.app.popups.close();
    if (this.app.state.tool !== 'text') this.viewport.focus({ preventScroll: true });
    if (e.button === 1) {
      e.preventDefault();
      this.pan = { x: e.clientX, y: e.clientY, sl: this.viewport.scrollLeft, st: this.viewport.scrollTop };
      this.viewport.setPointerCapture(e.pointerId);
      this.viewport.style.cursor = 'grabbing';
      return;
    }
    if (e.button !== 0 && e.button !== 2) return;
    // Ignore clicks on the scrollbars.
    const vr = this.viewport.getBoundingClientRect();
    if (e.clientX - vr.left > this.viewport.clientWidth || e.clientY - vr.top > this.viewport.clientHeight) return;
    const p = this.toImage(e.clientX, e.clientY);
    const st = this.app.state;
    if (st.tool === 'magnifier') {
      this.zoomStep(e.button === 2 ? -1 : 1, e.clientX, e.clientY);
      this.suppressContext = true;
      return;
    }
    if (st.tool === 'text') {
      // Keep the focus on the text box instead of the viewport.
      e.preventDefault();
      if (e.button === 0) this.app.text.pointerDown(p);
      return;
    }
    if (e.button === 2 && (st.tool === 'select-rect' || st.tool === 'select-free')) return; // context menu
    if (e.button === 2) this.suppressContext = true;
    e.preventDefault();
    this.viewport.setPointerCapture(e.pointerId);
    this.dragging = true;
    this.pointer = p;
    this.engine.pointer_down(p.x, p.y, e.button, e.shiftKey, e.ctrlKey || e.metaKey, e.altKey);
    if (st.tool === 'brush' && st.brush === 'airbrush') {
      clearInterval(this.airbrushTimer);
      this.airbrushTimer = setInterval(() => {
        this.engine.tick();
        this.app.changed();
      }, 40);
    }
    this.app.changed();
  }

  onPointerMove(e) {
    if (this.pan) {
      this.viewport.scrollLeft = this.pan.sl - (e.clientX - this.pan.x);
      this.viewport.scrollTop = this.pan.st - (e.clientY - this.pan.y);
      return;
    }
    if (this.canvasDrag) {
      this.moveCanvasResize(e);
      return;
    }
    const p = this.toImage(e.clientX, e.clientY);
    this.pointer = p;
    this.app.updatePointerStatus(p);
    if (this.dragging) {
      const events = e.getCoalescedEvents ? e.getCoalescedEvents() : [];
      for (const ev of events.length ? events : [e]) {
        const q = this.toImage(ev.clientX, ev.clientY);
        this.engine.pointer_move(q.x, q.y, e.shiftKey, e.ctrlKey || e.metaKey, e.altKey);
      }
      this.app.changed();
      this.autoScroll(e);
    } else {
      this.updateCursor();
      const t = this.app.state && this.app.state.tool;
      if (t === 'eraser' || t === 'brush' || this.showRulers) this.drawOverlay();
    }
  }

  /**
   * While dragging past the edge of the view, scroll towards the pointer and keep
   * feeding the engine the pointer position (selections and shapes follow).
   */
  autoScroll(e) {
    this.scrollPointer = { x: e.clientX, y: e.clientY, shift: e.shiftKey, ctrl: e.ctrlKey || e.metaKey, alt: e.altKey };
    if (this.scrollTimer) return;
    const step = () => {
      const p = this.scrollPointer;
      const r = this.viewport.getBoundingClientRect();
      const over = (v, lo, hi) => (v < lo ? v - lo : v > hi ? v - hi : 0);
      const dx = over(p.x, r.left, r.left + this.viewport.clientWidth);
      const dy = over(p.y, r.top, r.top + this.viewport.clientHeight);
      if ((!this.dragging && !this.canvasDrag) || (!dx && !dy)) {
        this.scrollTimer = null;
        return;
      }
      const speed = (d) => Math.sign(d) * Math.min(40, Math.max(2, Math.abs(d) / 2));
      this.viewport.scrollLeft += dx ? speed(dx) : 0;
      this.viewport.scrollTop += dy ? speed(dy) : 0;
      if (this.dragging) {
        const q = this.toImage(p.x, p.y);
        this.engine.pointer_move(q.x, q.y, p.shift, p.ctrl, p.alt);
        this.app.changed();
      }
      this.scrollTimer = requestAnimationFrame(step);
    };
    this.scrollTimer = requestAnimationFrame(step);
  }

  onPointerUp(e) {
    if (this.pan) {
      this.pan = null;
      this.viewport.style.cursor = '';
      this.updateCursor();
      return;
    }
    if (this.canvasDrag) {
      this.endCanvasResize();
      return;
    }
    if (!this.dragging) return;
    this.dragging = false;
    clearInterval(this.airbrushTimer);
    this.airbrushTimer = null;
    const p = this.toImage(e.clientX, e.clientY);
    this.engine.pointer_up(p.x, p.y);
    this.app.changed({ layers: true });
    this.updateCursor();
  }

  // ----- canvas resize handles ------------------------------------------------

  startCanvasResize(e, h) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    this.app.commitTransient();
    e.target.setPointerCapture(e.pointerId);
    const w0 = this.engine.width();
    const h0 = this.engine.height();
    this.canvasDrag = { handle: h, x: e.clientX, y: e.clientY, w0, h0, w: w0, h: h0, ox: 0, oy: 0 };
    const handle = e.target;
    const move = (ev) => this.moveCanvasResize(ev);
    const done = (apply) => () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', onUp);
      handle.removeEventListener('pointercancel', onCancel);
      if (apply) this.endCanvasResize();
      else this.cancelCanvasResize();
    };
    const onUp = done(true);
    const onCancel = done(false);
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', onUp);
    handle.addEventListener('pointercancel', onCancel);
  }

  moveCanvasResize(e) {
    const d = this.canvasDrag;
    if (!d) return;
    const dx = Math.round((e.clientX - d.x) / this.zoom);
    const dy = Math.round((e.clientY - d.y) / this.zoom);
    d.w = d.w0;
    d.h = d.h0;
    d.ox = 0;
    d.oy = 0;
    if (d.handle.includes('e')) d.w = Math.max(1, d.w0 + dx);
    if (d.handle.includes('s')) d.h = Math.max(1, d.h0 + dy);
    if (d.handle.includes('w')) {
      d.w = Math.max(1, d.w0 - dx);
      d.ox = d.w - d.w0;
    }
    if (d.handle.includes('n')) {
      d.h = Math.max(1, d.h0 - dy);
      d.oy = d.h - d.h0;
    }
    this.app.updateCanvasStatus(d.w, d.h);
    this.drawOverlay();
  }

  /** Drops a canvas-resize drag without applying it (Escape, cancelled pointer). */
  cancelCanvasResize() {
    if (!this.canvasDrag) return false;
    this.canvasDrag = null;
    this.drawOverlay();
    this.app.updateCanvasStatus(this.engine.width(), this.engine.height());
    return true;
  }

  endCanvasResize() {
    const d = this.canvasDrag;
    this.canvasDrag = null;
    if (d && (d.w !== d.w0 || d.h !== d.h0)) {
      this.app.sizeChecked(this.engine.resize_canvas_at(d.w, d.h, d.ox, d.oy));
      this.app.changed({ layers: true });
    } else {
      this.drawOverlay();
      this.app.updateCanvasStatus(this.engine.width(), this.engine.height());
    }
  }
}
