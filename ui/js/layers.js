// Layers panel: thumbnails, visibility, selection, rename and drag-to-reorder.
// The list shows the top layer first, like Paint.

import { icon } from './icons.js';
import { layerName, t } from './i18n.js';

export class LayersPanel {
  constructor(app) {
    this.app = app;
    this.panel = document.getElementById('layersPanel');
    this.list = document.getElementById('layerList');
    this.key = '';
    this.urls = [];
    this.thumbTimer = null;
  }

  get visible() {
    return !this.panel.hidden;
  }

  toggle(force) {
    this.panel.hidden = force === undefined ? !this.panel.hidden : !force;
    document.querySelector('[data-cmd="layers.toggle"]').classList.toggle('active', this.visible);
    this.key = '';
    this.update(true);
    this.app.view.sizeOverlay();
  }

  /** Rebuilds the list when the layer structure changed; refreshes thumbnails when asked. */
  update(thumbs = false) {
    if (!this.visible) return;
    const st = this.app.state;
    const key = JSON.stringify([st.layers, st.active_layer]);
    if (key !== this.key) {
      this.key = key;
      this.build(st);
      thumbs = true;
    }
    if (thumbs) this.scheduleThumbs();
    this.panel.querySelector('[data-cmd="layers.delete"]').disabled = st.layers.length <= 1;
    this.panel.querySelector('[data-cmd="layers.merge"]').disabled = st.active_layer === 0;
    this.panel.querySelector('[data-cmd="layers.up"]').disabled = st.active_layer >= st.layers.length - 1;
    this.panel.querySelector('[data-cmd="layers.down"]').disabled = st.active_layer === 0;
  }

  build(st) {
    this.list.textContent = '';
    for (let i = st.layers.length - 1; i >= 0; i--) {
      const l = st.layers[i];
      const li = document.createElement('li');
      li.className = 'layer-item';
      li.dataset.index = i;
      li.dataset.testid = `layer-${i}`;
      li.classList.toggle('active', i === st.active_layer);
      li.classList.toggle('hidden-layer', !l.visible);
      li.innerHTML = `<img class="layer-thumb" alt=""><span class="layer-name"></span><button class="icon-btn small" data-testid="layer-eye-${i}">${icon(l.visible ? 'eye' : 'eyeOff')}</button>`;
      li.querySelector('.layer-name').textContent = layerName(l.name);
      const eye = li.querySelector('button');
      eye.title = t('layers.toggle');
      eye.addEventListener('click', (e) => {
        e.stopPropagation();
        this.app.engine.set_layer_visible(i, !l.visible);
        this.app.changed({ layers: true });
      });
      li.addEventListener('click', () => {
        if (this.app.state.active_layer !== i) {
          this.app.engine.set_active_layer(i);
          this.app.changed({ layers: true });
        }
      });
      li.querySelector('.layer-name').addEventListener('dblclick', (e) => this.rename(e.currentTarget, i, l.name));
      li.addEventListener('pointerdown', (e) => this.dragStart(e, li, i));
      this.list.appendChild(li);
    }
  }

  rename(span, i, current) {
    const input = document.createElement('input');
    input.value = layerName(current);
    input.dataset.testid = 'layer-rename';
    span.textContent = '';
    span.appendChild(input);
    input.focus();
    input.select();
    const done = (ok) => {
      if (!input.isConnected) return;
      if (ok) this.app.engine.rename_layer(i, input.value);
      this.key = '';
      this.app.changed({ layers: true });
    };
    input.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') done(true);
      if (e.key === 'Escape') done(false);
    });
    input.addEventListener('blur', () => done(true));
  }

  dragStart(e, li, from) {
    if (e.button !== 0 || e.target.closest('button,input')) return;
    const startY = e.clientY;
    let moved = false;
    const move = (ev) => {
      if (Math.abs(ev.clientY - startY) > 6) {
        moved = true;
        li.classList.add('dragging');
      }
    };
    const up = (ev) => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      li.classList.remove('dragging');
      if (!moved) return;
      const target = document.elementFromPoint(ev.clientX, ev.clientY);
      const over = target && target.closest('.layer-item');
      if (over) {
        const to = Number(over.dataset.index);
        if (to !== from) {
          this.app.engine.move_layer(from, to);
          this.app.changed({ layers: true });
        }
      }
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }

  scheduleThumbs() {
    clearTimeout(this.thumbTimer);
    this.thumbTimer = setTimeout(() => this.thumbs(), 120);
  }

  thumbs() {
    if (!this.visible) return;
    this.urls.forEach((u) => URL.revokeObjectURL(u));
    this.urls = [];
    this.list.querySelectorAll('.layer-item').forEach((li) => {
      const png = this.app.engine.layer_thumbnail_png(Number(li.dataset.index), 112);
      if (!png) return;
      const url = URL.createObjectURL(new Blob([png], { type: 'image/png' }));
      this.urls.push(url);
      li.querySelector('img').src = url;
    });
  }
}
