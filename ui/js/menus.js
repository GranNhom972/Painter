// Popup menus, galleries and context menus, with full keyboard support.

import { icon } from './icons.js';

/**
 * Item spec:
 *   { sep: true }
 *   { title: 'text' }
 *   { label, key?, icon?, run?, disabled?, testid?, items?: [...] }
 *   { ..., checked: bool }               check box item
 *   { ..., checked: bool, radio: true }  one choice among several
 *   { element: HTMLElement }             custom content (galleries, sliders, palette)
 */
export class Popups {
  constructor(layer) {
    this.layer = layer;
    this.stack = [];
    this.anchor = null;
    /** Called with -1/+1 when Left/Right is pressed in a top-level menu-bar menu. */
    this.onNavigate = null;
    document.addEventListener('pointerdown', (e) => {
      if (this.stack.length && !this.stack.some((p) => p.contains(e.target)) && !(this.anchor && this.anchor.contains(e.target))) {
        this.close(false);
      }
    }, true);
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && this.stack.length) {
        e.stopPropagation();
        e.preventDefault();
        if (this.stack.length > 1) this.closeSub();
        else this.close(true);
      }
    }, true);
    window.addEventListener('blur', () => this.close(false));
    window.addEventListener('resize', () => this.close(false));
  }

  get isOpen() {
    return this.stack.length > 0;
  }

  /** Closes everything; `refocus` gives the focus back to the button that opened it. */
  close(refocus = false) {
    this.stack.forEach((p) => p.remove());
    this.stack = [];
    const a = this.anchor;
    if (a) {
      a.classList.remove('open');
      a.setAttribute('aria-expanded', 'false');
      if (refocus) a.focus();
    }
    this.anchor = null;
  }

  closeSub() {
    const sub = this.stack.pop();
    if (sub) sub.remove();
    const parent = this.stack[this.stack.length - 1];
    const item = parent && parent.querySelector('.mi.open');
    if (item) {
      item.classList.remove('open');
      item.setAttribute('aria-expanded', 'false');
      item.focus();
    }
  }

  /**
   * Opens a menu under `anchor` (an element) or at a point {x, y}.
   * `keyboard`: focus the first item (opened with the keyboard).
   */
  open(anchor, items, { testid, label, noIcons = false, keyboard = false } = {}) {
    const same = anchor instanceof Element && this.anchor === anchor;
    this.close(false);
    if (same && !keyboard) return null; // second click on the same button closes it
    const el = this.build(items, 0, { label, noIcons });
    if (testid) el.dataset.testid = testid;
    this.layer.appendChild(el);
    this.stack.push(el);
    if (anchor instanceof Element) {
      this.anchor = anchor;
      anchor.classList.add('open');
      anchor.setAttribute('aria-expanded', 'true');
      const r = anchor.getBoundingClientRect();
      this.place(el, r.left, r.bottom + 2, r.top);
    } else {
      this.place(el, anchor.x, anchor.y, anchor.y);
    }
    if (keyboard) this.focusFirst(el);
    return el;
  }

  place(el, x, y, altBottom) {
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const left = Math.min(x, vw - w - 4);
    let top = y;
    if (top + h > vh - 4) top = Math.max(4, altBottom - h - 2);
    el.style.left = `${Math.max(4, left)}px`;
    el.style.top = `${Math.max(4, top)}px`;
  }

  focusables(popup) {
    return [...popup.querySelectorAll('.mi:not(:disabled), .gallery button, .palette button, .size-panel input, .size-presets button')];
  }

  focusFirst(popup) {
    const f = this.focusables(popup);
    // Start on the current choice: the selected gallery cell, else the checked radio item.
    const current =
      f.find((b) => b.classList.contains('active')) ||
      f.find((b) => b.dataset.current === 'true') ||
      f.find((b) => b.getAttribute('role') === 'menuitemradio' && b.getAttribute('aria-checked') === 'true');
    (current || f[0])?.focus();
  }

  build(items, depth, { label, noIcons } = {}) {
    const el = document.createElement('div');
    el.className = 'popup';
    if (noIcons) el.classList.add('no-icons');
    const custom = items.some((it) => it && it.element);
    el.setAttribute('role', custom ? 'dialog' : 'menu');
    if (label) el.setAttribute('aria-label', label);
    for (const it of items) {
      if (!it) continue;
      if (it.sep) {
        const s = document.createElement('div');
        s.className = 'menu-sep';
        s.setAttribute('role', 'separator');
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
      b.tabIndex = -1;
      const checkable = it.checked !== undefined;
      b.setAttribute('role', checkable ? (it.radio ? 'menuitemradio' : 'menuitemcheckbox') : 'menuitem');
      if (checkable) b.setAttribute('aria-checked', String(!!it.checked));
      if (it.testid) b.dataset.testid = it.testid;
      if (it.disabled) b.disabled = true;
      // Radio items keep their icon (the accent pill marks the choice); check boxes show a tick.
      const ico = checkable && !it.radio ? (it.checked ? icon('check') : '') : it.icon ? icon(it.icon) : '';
      b.innerHTML = `<span class="mi-ico">${ico}</span><span class="mi-label"></span>${it.key ? `<span class="mi-key">${it.key}</span>` : ''}${it.items ? `<span class="mi-sub">${icon('chevronRight')}</span>` : ''}`;
      b.querySelector('.mi-label').textContent = it.label;
      if (it.items) {
        b.setAttribute('aria-haspopup', 'menu');
        b.setAttribute('aria-expanded', 'false');
        const openSub = (focus) => {
          while (this.stack.length > depth + 1) this.stack.pop().remove();
          el.querySelectorAll('.mi.open').forEach((m) => {
            m.classList.remove('open');
            m.setAttribute('aria-expanded', 'false');
          });
          b.classList.add('open');
          b.setAttribute('aria-expanded', 'true');
          const sub = this.build(it.items, depth + 1);
          this.layer.appendChild(sub);
          this.stack.push(sub);
          const r = b.getBoundingClientRect();
          const w = sub.offsetWidth;
          let left = r.right + 2;
          let top = r.top - 4;
          if (left + w > window.innerWidth - 4) left = r.left - w - 2;
          if (left < 4) {
            // No room on either side (narrow window): open below the item, never over it.
            left = Math.max(4, Math.min(r.left + 16, window.innerWidth - w - 4));
            top = r.bottom + 2;
          }
          this.place(sub, left, top, r.top);
          if (focus) this.focusFirst(sub);
        };
        b.addEventListener('pointerenter', () => openSub(false));
        b.addEventListener('click', (e) => openSub(e.detail === 0));
        b.openSub = openSub;
      } else {
        b.addEventListener('pointerenter', () => {
          while (this.stack.length > depth + 1) this.stack.pop().remove();
          el.querySelectorAll('.mi.open').forEach((m) => m.classList.remove('open'));
        });
        b.addEventListener('click', (e) => {
          // detail === 0: activated with Enter/Space, so the focus goes back to the menu button.
          this.close(e.detail === 0);
          if (it.run) it.run();
        });
      }
      el.appendChild(b);
    }
    el.addEventListener('keydown', (e) => this.onKey(e, el, depth));
    return el;
  }

  onKey(e, popup, depth) {
    const items = this.focusables(popup);
    const i = items.indexOf(document.activeElement);
    const go = (k) => {
      e.preventDefault();
      items[(k + items.length) % items.length]?.focus();
    };
    const gallery = document.activeElement && document.activeElement.closest('.gallery, .palette');
    if (gallery) {
      // 2-D navigation in galleries and the palette grid.
      const cells = [...gallery.querySelectorAll('button')];
      const j = cells.indexOf(document.activeElement);
      const cols = getComputedStyle(gallery).gridTemplateColumns.split(' ').length || 1;
      const step = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: cols, ArrowUp: -cols }[e.key];
      if (step) {
        e.preventDefault();
        const k = j + step;
        if (k >= 0 && k < cells.length) cells[k].focus();
        else if (step > 0) items[items.indexOf(cells[cells.length - 1]) + 1]?.focus();
        else items[items.indexOf(cells[0]) - 1]?.focus();
        return;
      }
    }
    switch (e.key) {
      case 'ArrowDown':
        if (document.activeElement?.type === 'range') return;
        go(i + 1);
        break;
      case 'ArrowUp':
        if (document.activeElement?.type === 'range') return;
        go(i - 1);
        break;
      case 'Home':
        go(0);
        break;
      case 'End':
        go(items.length - 1);
        break;
      case 'ArrowRight':
        if (document.activeElement?.openSub) {
          e.preventDefault();
          document.activeElement.openSub(true);
        } else if (depth === 0 && this.onNavigate && this.anchor?.classList.contains('menu-btn')) {
          e.preventDefault();
          this.onNavigate(1);
        }
        break;
      case 'ArrowLeft':
        if (depth > 0) {
          e.preventDefault();
          this.closeSub();
        } else if (this.onNavigate && this.anchor?.classList.contains('menu-btn')) {
          e.preventDefault();
          this.onNavigate(-1);
        }
        break;
      case 'Tab':
        this.close(false);
        break;
      default:
        break;
    }
  }
}
