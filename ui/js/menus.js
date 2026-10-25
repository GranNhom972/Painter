// Popup menus, galleries and context menus.

import { icon } from './icons.js';

/**
 * Item spec:
 *   { sep: true }
 *   { title: 'text' }
 *   { label, key?, icon?, run?, checked?, disabled?, testid?, items?: [...] }
 *   { element: HTMLElement }   custom content (galleries, sliders)
 */
export class Popups {
  constructor(layer) {
    this.layer = layer;
    this.stack = [];
    this.anchor = null;
    document.addEventListener('pointerdown', (e) => {
      if (this.stack.length && !this.stack.some((p) => p.contains(e.target)) && !(this.anchor && this.anchor.contains(e.target))) {
        this.close();
      }
    }, true);
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && this.stack.length) {
        e.stopPropagation();
        e.preventDefault();
        this.close();
      }
    }, true);
    window.addEventListener('blur', () => this.close());
    window.addEventListener('resize', () => this.close());
  }

  get isOpen() {
    return this.stack.length > 0;
  }

  close() {
    this.stack.forEach((p) => p.remove());
    this.stack = [];
    if (this.anchor) this.anchor.classList.remove('open');
    this.anchor = null;
  }

  /** Opens a menu under `anchor` (an element) or at a point {x, y}. */
  open(anchor, items, { testid } = {}) {
    const same = anchor instanceof Element && this.anchor === anchor;
    this.close();
    if (same) return null; // second click on the same button closes it
    const el = this.build(items, 0);
    if (testid) el.dataset.testid = testid;
    this.layer.appendChild(el);
    this.stack.push(el);
    if (anchor instanceof Element) {
      this.anchor = anchor;
      anchor.classList.add('open');
      const r = anchor.getBoundingClientRect();
      this.place(el, r.left, r.bottom + 2, r.top);
    } else {
      this.place(el, anchor.x, anchor.y, anchor.y);
    }
    return el;
  }

  place(el, x, y, altBottom) {
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let left = Math.min(x, vw - w - 4);
    let top = y;
    if (top + h > vh - 4) top = Math.max(4, altBottom - h - 2);
    el.style.left = `${Math.max(4, left)}px`;
    el.style.top = `${Math.max(4, top)}px`;
  }

  build(items, depth) {
    const el = document.createElement('div');
    el.className = 'popup';
    el.setAttribute('role', 'menu');
    for (const it of items) {
      if (!it) continue;
      if (it.sep) {
        const s = document.createElement('div');
        s.className = 'menu-sep';
        el.appendChild(s);
        continue;
      }
      if (it.title) {
        const s = document.createElement('div');
        s.className = 'menu-title';
        s.textContent = it.title;
        el.appendChild(s);
        continue;
      }
      if (it.element) {
        el.appendChild(it.element);
        continue;
      }
      const b = document.createElement('button');
      b.className = 'mi';
      b.setAttribute('role', it.checked === undefined ? 'menuitem' : 'menuitemcheckbox');
      if (it.testid) b.dataset.testid = it.testid;
      if (it.disabled) b.disabled = true;
      const ico = it.checked !== undefined ? (it.checked ? icon('check') : '') : it.icon ? icon(it.icon) : '';
      if (it.checked !== undefined) b.setAttribute('aria-checked', String(!!it.checked));
      b.innerHTML = `<span class="mi-ico">${ico}</span><span class="mi-label"></span>${it.key ? `<span class="mi-key">${it.key}</span>` : ''}${it.items ? '<span class="mi-sub">›</span>' : ''}`;
      b.querySelector('.mi-label').textContent = it.label;
      if (it.items) {
        const openSub = () => {
          while (this.stack.length > depth + 1) this.stack.pop().remove();
          el.querySelectorAll('.mi.open').forEach((m) => m.classList.remove('open'));
          b.classList.add('open');
          const sub = this.build(it.items, depth + 1);
          this.layer.appendChild(sub);
          this.stack.push(sub);
          const r = b.getBoundingClientRect();
          let left = r.right + 2;
          if (left + sub.offsetWidth > window.innerWidth - 4) left = r.left - sub.offsetWidth - 2;
          this.place(sub, left, r.top - 4, r.bottom);
          sub.style.left = `${Math.max(4, left)}px`;
        };
        b.addEventListener('pointerenter', openSub);
        b.addEventListener('click', openSub);
      } else {
        b.addEventListener('pointerenter', () => {
          while (this.stack.length > depth + 1) this.stack.pop().remove();
          el.querySelectorAll('.mi.open').forEach((m) => m.classList.remove('open'));
        });
        b.addEventListener('click', () => {
          this.close();
          if (it.run) it.run();
        });
      }
      el.appendChild(b);
    }
    return el;
  }
}
