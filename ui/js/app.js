// Application controller: wires the ribbon, menus, keyboard, clipboard and
// files to the WebAssembly engine, and keeps the UI in sync with its state.

import { Engine, version } from '../pkg/paint_wasm.js';
import { icon } from './icons.js';
import { applyTranslations, formatBytes, language, setLanguage, t } from './i18n.js';
import { Popups } from './menus.js';
import { View, ZOOM_PRESETS } from './view.js';
import { TextTool } from './text.js';
import { LayersPanel } from './layers.js';
import { colorDialog, confirmSave, message, propertiesDialog, resizeDialog } from './dialogs.js';
import { brushPreview, shapePreview } from './previews.js';
import * as platform from './platform.js';

const PALETTE = [
  '#000000', '#7f7f7f', '#880015', '#ed1c24', '#ff7f27', '#fff200', '#22b14c', '#00a2e8', '#3f48cc', '#a349a4',
  '#ffffff', '#c3c3c3', '#b97a57', '#ffaec9', '#ffc90e', '#efe4b0', '#b5e61d', '#99d9ea', '#7092be', '#c8bfe7',
];
const BRUSHES = ['brush', 'calligraphy-1', 'calligraphy-2', 'airbrush', 'oil', 'crayon', 'marker', 'natural-pencil', 'watercolor'];
const SHAPES = [
  'line', 'curve', 'oval', 'rectangle', 'rounded-rectangle', 'polygon', 'triangle', 'right-triangle', 'diamond', 'pentagon', 'hexagon',
  'arrow-right', 'arrow-left', 'arrow-up', 'arrow-down', 'star-4', 'star-5', 'star-6', 'callout-rect', 'callout-oval', 'callout-cloud', 'heart', 'lightning',
];
const SIZE_PRESETS = [1, 3, 5, 8, 12, 16, 24, 32];
const DEFAULT_W = 1152;
const DEFAULT_H = 648;

function readPref(key, fallback) {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : JSON.parse(v);
  } catch {
    return fallback;
  }
}

function writePref(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* storage unavailable */
  }
}

export class App {
  constructor(wasm) {
    this.wasm = wasm;
    this.platform = platform;
    this.engine = new Engine(DEFAULT_W, DEFAULT_H);
    this._state = JSON.parse(this.engine.state_json());
    this.activeSlot = 1;
    this.customColors = readPref('paint.customColors', []);
    this.file = { name: null, path: null, format: null, size: null };
    this.clipboardFallback = null;
    this.frameRequested = false;
    this.layersDirty = false;
    this.popups = new Popups(document.getElementById('popupLayer'));
    this.text = new TextTool(this);
    this.view = new View(this);
    this.layers = new LayersPanel(this);
    this.commands = this.buildCommands();
    this.theme = readPref('paint.theme', 'system');
    this.applyTheme(this.theme);
    this.injectIcons(document);
    applyTranslations();
    this.buildPalette();
    this.bindRibbon();
    this.bindKeyboard();
    this.bindClipboardEvents();
    this.bindDragDrop();
    this.bindBackForward();
    this.windowActions = platform.initWindowChrome(icon, t);
    platform.onCloseRequested(Object.assign(() => this.confirmDiscard(), { isDirty: () => this.engine.is_modified() || (this.text.active && this.text.box.value.trim() !== '') }));
    document.documentElement.lang = language();
    this.view.layout();
    this.view.sizeOverlay();
    this.view.fitIfLarge();
    this.layersDirty = true;
    this.flush();
    this.updatePointerStatus(null);
    this.updateFileStatus();
    this.updateZoomUi();
    this.loadInitialFile();
  }

  // ----- state & rendering -----------------------------------------------------

  /** Engine state; a pending refresh is applied first so event handlers never see stale state. */
  get state() {
    if (this.frameRequested) this.flush();
    return this._state;
  }

  /** Call after any engine mutation: re-renders and refreshes the UI on the next frame. */
  changed({ layers = false } = {}) {
    if (layers) this.layersDirty = true;
    if (this.frameRequested) return;
    this.frameRequested = true;
    requestAnimationFrame(() => {
      if (this.frameRequested) this.flush();
    });
  }

  /** Synchronous render + UI refresh (also used by tests). */
  flush() {
    this.frameRequested = false;
    this.view.blit();
    this._state = JSON.parse(this.engine.state_json());
    this.updateUi();
    this.view.drawOverlay();
    this.view.updateCursor();
    if (!this.view.dragging) {
      this.layers.update(this.layersDirty);
      this.layersDirty = false;
    }
  }

  updateUi() {
    const st = this.state;
    const selTool = st.tool === 'select-rect' || st.tool === 'select-free';
    document.querySelectorAll('[data-tool]').forEach((b) => {
      const tool = b.dataset.tool;
      const on = tool === st.tool || (tool === 'select' && selTool);
      b.classList.toggle('active', on);
      b.setAttribute('aria-pressed', String(on));
    });
    const selIcon = document.getElementById('selectionIcon');
    const wanted = st.tool === 'select-free' ? 'selectFree' : 'selectRectBig';
    if (selIcon.dataset.icon !== wanted) {
      selIcon.dataset.icon = wanted;
      selIcon.innerHTML = icon(wanted);
    }
    const s = st.selection;
    const onCanvas = s && s.x < st.width && s.y < st.height && s.x + s.w > 0 && s.y + s.h > 0;
    document.querySelector('[data-cmd="image.crop"]').disabled = !s || s.drawing || !onCanvas;
    document.querySelector('[data-cmd="edit.undo"]').disabled = !st.can_undo;
    document.querySelector('[data-cmd="edit.redo"]').disabled = !st.can_redo;
    document.querySelector('[data-color-slot="1"] .swatch').style.background = st.color1;
    document.querySelector('[data-color-slot="2"] .swatch').style.background = st.color2;
    document.querySelectorAll('[data-color-slot]').forEach((b) => {
      const on = Number(b.dataset.colorSlot) === this.activeSlot;
      b.classList.toggle('selected', on);
      b.setAttribute('aria-pressed', String(on));
    });
    this.text.setVisible(st.tool === 'text');
    if (this.text.active) this.text.style();
    const sel = st.selection;
    this.setStat('statSel', sel && sel.w > 0 ? `${sel.w} x ${sel.h}px` : '');
    this.updateCanvasStatus(st.width, st.height);
    this.updateTitle();
  }

  updateTitle() {
    const name = this.file.name || t('app.untitled');
    platform.setTitle(`${this.state.modified ? '● ' : ''}${t('app.name')} - ${name}`);
  }

  /** Status bar field; an empty field also hides its icon. */
  setStat(id, text) {
    const el = document.getElementById(id);
    if (el.textContent !== text) el.textContent = text;
    el.parentElement.classList.toggle('empty', !text);
  }

  updatePointerStatus(p) {
    if (!p) {
      this.setStat('statPos', '');
      return;
    }
    const x = Math.floor(p.x);
    const y = Math.floor(p.y);
    this.setStat('statPos', x >= 0 && y >= 0 && x < this.state.width && y < this.state.height ? `${x},${y}px` : '');
  }

  updateCanvasStatus(w, h) {
    this.setStat('statCanvas', `${w} x ${h}px`);
  }

  updateFileStatus() {
    this.setStat('statFile', this.file.size != null ? t('status.size', { size: formatBytes(this.file.size) }) : '');
  }

  updateZoomUi() {
    const z = this.view.zoom;
    const label = `${Math.round(z * 100)}%`;
    document.getElementById('zoomValue').textContent = label;
    document.querySelector('[data-testid=zoom-value]').setAttribute('aria-label', `${t('status.zoom')} ${label}`);
    const slider = document.getElementById('zoomSlider');
    slider.value = Math.round((1000 * Math.log(z / 0.1)) / Math.log(80));
    // Blue part of the track, up to the thumb.
    slider.style.setProperty('--fill', `${slider.value / 10}%`);
  }

  /** Applies the text box being edited (shapes and selections stay editable). */
  commitTransient() {
    if (this.text.active) this.text.commit();
  }

  /** Tells the user when the engine refused a size change (memory limits). */
  sizeChecked(ok) {
    if (!ok) this.toast(t('msg.tooLarge'));
    return ok;
  }

  toast(msg) {
    const el = document.getElementById('toast');
    el.textContent = msg;
    el.hidden = false;
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => {
      el.hidden = true;
    }, 2600);
  }

  // ----- theme -----------------------------------------------------------------

  applyTheme(theme) {
    this.theme = theme;
    writePref('paint.theme', theme);
    if (theme === 'system') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = theme;
    this.view.drawOverlay();
  }

  injectIcons(root) {
    root.querySelectorAll('[data-icon]').forEach((el) => {
      if (!el.firstChild) el.innerHTML = icon(el.dataset.icon);
    });
  }

  // ----- commands --------------------------------------------------------------

  buildCommands() {
    const e = () => this.engine;
    const act = (fn, layers = true) => () => {
      this.commitTransient();
      fn();
      this.changed({ layers });
    };
    const cmds = {
      'file.new': () => this.newImage(),
      'file.open': () => this.openImage(),
      'file.save': () => this.save(),
      'file.print': () => {
        this.commitTransient();
        window.print();
      },
      'file.properties': () => this.properties(),
      'file.quit': () => platform.quit(() => this.confirmDiscard()),
      'edit.undo': () => {
        // While typing, Undo throws the text box away instead of undoing the picture.
        if (this.text.active) {
          this.text.cancel();
          return;
        }
        e().undo();
        this.changed({ layers: true });
      },
      'edit.redo': act(() => e().redo()),
      'edit.cut': () => this.cut(),
      'edit.copy': () => this.copy(),
      'edit.paste': () => this.paste(),
      'edit.pasteFrom': () => this.pasteFromFile(),
      'edit.selectAll': act(() => e().select_all(), false),
      'edit.invert': act(() => e().invert_selection(), false),
      'edit.delete': act(() => e().delete_selection()),
      'edit.deselect': act(() => e().deselect()),
      'view.zoomIn': () => this.view.zoomStep(1),
      'view.zoomOut': () => this.view.zoomStep(-1),
      'view.actual': () => this.view.setZoom(1),
      'view.fit': () => this.view.fit(),
      'view.rulers': () => this.view.setRulers(!this.view.showRulers),
      'view.grid': () => this.view.setGrid(!this.view.showGrid),
      'view.statusbar': () => {
        const bar = document.getElementById('statusbar');
        bar.hidden = !bar.hidden;
      },
      'view.fullscreen': () => platform.toggleFullscreen(),
      'image.crop': act(() => e().crop()),
      'image.resize': () => this.resizeSkew(),
      'image.rotateRight': act(() => this.sizeChecked(e().rotate(90))),
      'image.rotateLeft': act(() => this.sizeChecked(e().rotate(-90))),
      'image.rotate180': act(() => e().rotate(180)),
      'image.flipH': act(() => e().flip_horizontal()),
      'image.flipV': act(() => e().flip_vertical()),
      'image.removeBg': act(() => {
        if (!e().remove_background(32)) this.toast(t('msg.noBackground'));
      }),
      'color.edit': () => this.editColor(),
      'color.swap': act(() => e().swap_colors(), false),
      'layers.toggle': () => this.layers.toggle(),
      'layers.add': act(() => e().add_layer()),
      'layers.delete': act(() => e().delete_layer(this.state.active_layer)),
      'layers.duplicate': act(() => e().duplicate_layer(this.state.active_layer)),
      'layers.merge': act(() => e().merge_down(this.state.active_layer)),
      'layers.up': act(() => e().move_layer(this.state.active_layer, this.state.active_layer + 1)),
      'layers.down': act(() => e().move_layer(this.state.active_layer, this.state.active_layer - 1)),
      'settings.about': () => message(t('dlg.about.title'), t('dlg.about.body', { version: version() })),
    };
    for (const f of platform.FORMATS) cmds[`file.saveAs.${f.ext}`] = () => this.saveAs(f);
    return cmds;
  }

  run(id) {
    const fn = this.commands[id];
    if (fn) return fn();
    console.warn('unknown command', id);
  }

  setTool(name) {
    if (name !== 'text') this.commitTransient();
    this.engine.set_tool(name);
    this.changed();
  }

  // ----- ribbon & menus --------------------------------------------------------

  bindRibbon() {
    // A mouse click on a toolbar button must not move the focus (so Enter/Space
    // keep going to the picture, and the text box keeps its caret). Keyboard
    // users still Tab to the buttons.
    document.addEventListener('mousedown', (e) => {
      const b = e.target.closest && e.target.closest('.menubar button, .ribbon button, .statusbar button, .layers-panel button, .text-bar button, .popup button');
      if (b) e.preventDefault();
    });
    document.querySelectorAll('[data-cmd]').forEach((el) => {
      el.addEventListener('click', () => this.run(el.dataset.cmd));
    });
    document.querySelectorAll('.tool-btn[data-tool]').forEach((el) => {
      el.addEventListener('click', () => this.setTool(el.dataset.tool));
    });
    document.querySelectorAll('[data-menu]').forEach((el) => {
      el.setAttribute('aria-haspopup', 'menu');
      el.setAttribute('aria-expanded', 'false');
      el.addEventListener('click', (ev) => {
        // Split buttons: the icon part activates the tool, the arrow opens the menu.
        const arrow = ev.target.closest('.chev') || ev.target.closest('[data-icon="chevron"]');
        const tool = el.dataset.tool;
        if (tool && !arrow) {
          if (tool === 'select') this.setTool(this.state.tool === 'select-free' ? 'select-free' : this.lastSelectTool || 'select-rect');
          else if (tool === 'brush') this.setBrush(this.state.brush);
          else if (tool === 'shape') this.setShape(this.state.shape);
          return;
        }
        this.openMenu(el.dataset.menu, el, ev.detail === 0);
      });
      // Down arrow opens the menu of any menu button (split buttons included).
      el.addEventListener('keydown', (ev) => {
        if (ev.key === 'ArrowDown') {
          ev.preventDefault();
          this.openMenu(el.dataset.menu, el, true);
        }
      });
    });
    const menuButtons = [...document.querySelectorAll('.menu-btn')];
    this.popups.onNavigate = (dir) => {
      const i = menuButtons.indexOf(this.popups.anchor);
      const next = menuButtons[(i + dir + menuButtons.length) % menuButtons.length];
      this.openMenu(next.dataset.menu, next, true);
    };
    // Narrow windows: collapse groups into dropdowns, then let the wheel scroll the ribbon.
    const ribbon = document.getElementById('ribbon');
    const steps = ['compact-colors', 'compact-image', 'compact-tight'];
    const fit = () => {
      ribbon.classList.remove(...steps);
      for (const step of steps) {
        if (ribbon.scrollWidth <= ribbon.clientWidth) break;
        ribbon.classList.add(step);
      }
    };
    new ResizeObserver(fit).observe(ribbon);
    ribbon.addEventListener('wheel', (e) => {
      if (!e.deltaX && ribbon.scrollWidth > ribbon.clientWidth) {
        ribbon.scrollLeft += e.deltaY;
        e.preventDefault();
      }
    }, { passive: false });
    // Menu bar: hovering another menu while one is open switches to it.
    document.querySelectorAll('.menu-btn').forEach((el) => {
      el.addEventListener('pointerenter', () => {
        if (this.popups.isOpen && this.popups.anchor && this.popups.anchor.classList.contains('menu-btn') && this.popups.anchor !== el) {
          this.openMenu(el.dataset.menu, el);
        }
      });
    });
    document.querySelectorAll('[data-color-slot]').forEach((el) => {
      el.addEventListener('click', () => {
        this.activeSlot = Number(el.dataset.colorSlot);
        this.updateUi();
      });
      el.addEventListener('dblclick', () => this.editColor());
    });
    const slider = document.getElementById('zoomSlider');
    slider.addEventListener('input', () => this.view.setZoom(0.1 * Math.pow(80, slider.value / 1000)));
  }

  setBrush(name) {
    this.commitTransient();
    this.engine.set_brush(name);
    this.changed();
  }

  setShape(name) {
    this.commitTransient();
    this.engine.set_shape(name);
    this.changed();
  }

  setColor(hex, slot = this.activeSlot) {
    if (slot === 1) this.engine.set_color1(hex);
    else this.engine.set_color2(hex);
    this.changed();
  }

  /** Fills the palette grid (the ribbon one, or a copy inside the compact Colours menu). */
  buildPalette(pal = document.getElementById('palette'), prefix = 'palette') {
    pal.textContent = '';
    pal.classList.add('palette');
    pal.setAttribute('role', 'group');
    pal.setAttribute('aria-label', t('group.colors'));
    const chips = [...PALETTE, ...Array.from({ length: 10 }, (_, i) => this.customColors[i] || null)];
    chips.forEach((c, i) => {
      const b = document.createElement('button');
      b.className = 'chip';
      b.dataset.testid = `${prefix}-${i}`;
      if (c) {
        b.style.background = c;
        b.dataset.color = c;
        const name = i < PALETTE.length ? t(`color.name.${i}`) : t('color.custom');
        b.title = `${name} (${c.toUpperCase()})`;
        b.setAttribute('aria-label', name);
        b.addEventListener('click', () => {
          this.popups.close();
          this.setColor(c);
        });
        b.addEventListener('contextmenu', (e) => {
          e.preventDefault();
          this.setColor(c, 2);
        });
      } else {
        b.classList.add('empty');
        b.title = t('color.custom');
        b.setAttribute('aria-label', t('color.custom'));
        b.addEventListener('click', () => {
          this.popups.close();
          this.editColor();
        });
      }
      pal.appendChild(b);
    });
    return pal;
  }

  async editColor() {
    const current = this.activeSlot === 1 ? this.state.color1 : this.state.color2;
    const hex = await colorDialog(current.slice(0, 7));
    if (!hex) return;
    this.setColor(hex);
    this.customColors = [hex, ...this.customColors.filter((c) => c !== hex)].slice(0, 10);
    writePref('paint.customColors', this.customColors);
    this.buildPalette();
  }

  menuItems(name) {
    const st = this.state;
    const c = (id) => () => this.run(id);
    const hasSel = !!st.selection;
    switch (name) {
      case 'file':
        return [
          { label: t('file.new'), key: 'Ctrl+N', run: c('file.new'), testid: 'mi-new' },
          { label: t('file.open'), key: 'Ctrl+O', run: c('file.open'), testid: 'mi-open' },
          { label: t('file.save'), key: 'Ctrl+S', run: c('file.save'), testid: 'mi-save' },
          {
            label: t('file.saveAs'),
            testid: 'mi-save-as',
            items: platform.FORMATS.map((f) => ({ label: t('file.saveAsFormat', { fmt: f.name }), key: f.ext === 'png' ? 'F12' : '', run: c(`file.saveAs.${f.ext}`), testid: `mi-save-${f.ext}` })),
          },
          { sep: true },
          { label: t('file.print'), key: 'Ctrl+P', run: c('file.print'), testid: 'mi-print' },
          { sep: true },
          { label: t('file.properties'), key: 'Ctrl+E', run: c('file.properties'), testid: 'mi-properties' },
          { sep: true },
          { label: t('file.quit'), run: c('file.quit'), testid: 'mi-quit' },
        ];
      case 'edit':
        return [
          { label: t('edit.undo'), key: 'Ctrl+Z', icon: 'undo', run: c('edit.undo'), disabled: !st.can_undo, testid: 'mi-undo' },
          { label: t('edit.redo'), key: 'Ctrl+Y', icon: 'redo', run: c('edit.redo'), disabled: !st.can_redo, testid: 'mi-redo' },
          { sep: true },
          { label: t('edit.cut'), key: 'Ctrl+X', icon: 'cut', run: c('edit.cut'), disabled: !hasSel, testid: 'mi-cut' },
          { label: t('edit.copy'), key: 'Ctrl+C', icon: 'copy', run: c('edit.copy'), disabled: !hasSel, testid: 'mi-copy' },
          { label: t('edit.paste'), key: 'Ctrl+V', icon: 'paste', run: c('edit.paste'), testid: 'mi-paste' },
          { label: t('edit.pasteFrom'), run: c('edit.pasteFrom'), testid: 'mi-paste-from' },
          { sep: true },
          { label: t('edit.selectAll'), key: 'Ctrl+A', icon: 'selectAll', run: c('edit.selectAll'), testid: 'mi-select-all' },
          { label: t('edit.invert'), icon: 'invert', run: c('edit.invert'), testid: 'mi-invert' },
          { label: t('edit.delete'), key: t('key.del'), icon: 'trash', run: c('edit.delete'), disabled: !hasSel, testid: 'mi-delete' },
          { label: t('image.crop'), key: t('key.crop'), icon: 'crop', run: c('image.crop'), disabled: !hasSel, testid: 'mi-crop' },
        ];
      case 'view':
        return [
          { label: t('view.zoomIn'), key: 'Ctrl++', icon: 'zoomIn', run: c('view.zoomIn'), testid: 'mi-zoom-in' },
          { label: t('view.zoomOut'), key: 'Ctrl+-', icon: 'zoomOut', run: c('view.zoomOut'), testid: 'mi-zoom-out' },
          { label: t('view.actual'), key: 'Ctrl+1', run: c('view.actual'), testid: 'mi-zoom-100' },
          { label: t('view.fit'), key: 'Ctrl+0', icon: 'fit', run: c('view.fit'), testid: 'mi-zoom-fit' },
          { sep: true },
          { label: t('view.rulers'), key: 'Ctrl+R', checked: this.view.showRulers, run: c('view.rulers'), testid: 'mi-rulers' },
          { label: t('view.grid'), key: 'Ctrl+G', checked: this.view.showGrid, run: c('view.grid'), testid: 'mi-grid' },
          { label: t('view.statusbar'), checked: !document.getElementById('statusbar').hidden, run: c('view.statusbar'), testid: 'mi-statusbar' },
          { sep: true },
          { label: t('view.fullscreen'), key: 'F11', run: c('view.fullscreen'), testid: 'mi-fullscreen' },
        ];
      case 'settings':
        return [
          {
            label: t('settings.theme'),
            testid: 'mi-theme',
            items: ['system', 'light', 'dark'].map((th) => ({ label: t(`settings.theme.${th}`), checked: this.theme === th, radio: true, run: () => this.applyTheme(th), testid: `mi-theme-${th}` })),
          },
          {
            label: t('settings.language'),
            testid: 'mi-language',
            items: [['fr', 'Français'], ['en', 'English']].map(([l, n]) => ({ label: n, checked: language() === l, radio: true, run: () => this.switchLanguage(l), testid: `mi-lang-${l}` })),
          },
          { sep: true },
          { label: t('settings.about'), run: c('settings.about'), testid: 'mi-about' },
        ];
      case 'selection':
        return [
          { label: t('sel.rect'), icon: 'selectRect', checked: st.tool === 'select-rect', radio: true, run: () => this.selectTool('select-rect'), testid: 'mi-select-rect' },
          { label: t('sel.free'), icon: 'selectFree', checked: st.tool === 'select-free', radio: true, run: () => this.selectTool('select-free'), testid: 'mi-select-free' },
          { sep: true },
          { label: t('edit.selectAll'), key: 'Ctrl+A', icon: 'selectAll', run: c('edit.selectAll'), testid: 'mi-sel-all' },
          { label: t('edit.invert'), icon: 'invert', run: c('edit.invert'), testid: 'mi-sel-invert' },
          { label: t('edit.delete'), key: t('key.del'), icon: 'trash', run: c('edit.delete'), disabled: !hasSel, testid: 'mi-sel-delete' },
          { sep: true },
          {
            label: t('sel.transparent'),
            checked: st.transparent_selection,
            run: () => {
              this.engine.set_transparent_selection(!st.transparent_selection);
              this.changed();
            },
            testid: 'mi-sel-transparent',
          },
        ];
      case 'rotate':
        return [
          { label: t('image.rotateRight'), icon: 'rotate', run: c('image.rotateRight'), testid: 'mi-rotate-right' },
          { label: t('image.rotateLeft'), icon: 'rotateLeft', run: c('image.rotateLeft'), testid: 'mi-rotate-left' },
          { label: t('image.rotate180'), icon: 'rotate180', run: c('image.rotate180'), testid: 'mi-rotate-180' },
        ];
      case 'flip':
        return [
          { label: t('image.flipV'), icon: 'flipV', run: c('image.flipV'), testid: 'mi-flip-v' },
          { label: t('image.flipH'), icon: 'flip', run: c('image.flipH'), testid: 'mi-flip-h' },
        ];
      case 'brushes':
        return [{ element: this.gallery('brushes', BRUSHES, (k) => brushPreview(Engine, this.wasm, k), (k) => this.setBrush(k), st.tool === 'brush' ? st.brush : null, st.brush) }];
      case 'shapes':
        return [
          { element: this.gallery('shapes', SHAPES, (k) => shapePreview(Engine, this.wasm, k), (k) => this.setShape(k), st.tool === 'shape' ? st.shape : null, st.shape) },
          { sep: true },
          { title: t('shape.outline') },
          ...['none', 'solid'].map((o) => ({ label: t(`style.${o}`), icon: o === 'solid' ? 'outline' : 'close', checked: st.outline === o, radio: true, run: () => this.setStyle('outline', o), testid: `mi-outline-${o}` })),
          { title: t('shape.fill') },
          ...['none', 'solid'].map((o) => ({ label: t(`style.${o}`), icon: o === 'solid' ? 'fillStyle' : 'close', checked: st.fill === o, radio: true, run: () => this.setStyle('fill', o), testid: `mi-fill-${o}` })),
        ];
      case 'size':
        return [{ element: this.sizePanel() }];
      case 'zoom':
        return [...ZOOM_PRESETS]
          .reverse()
          .map((z) => ({ label: `${z * 100}%`, checked: Math.abs(z - this.view.zoom) < 1e-6, radio: true, run: () => this.view.setZoom(z), testid: `mi-zoom-${z * 100}` }))
          .concat([{ sep: true }, { label: t('view.fit'), run: c('view.fit') }]);
      case 'colors':
        return [{ element: this.buildPalette(document.createElement('div'), 'menu-palette') }, { sep: true }, { label: t('color.edit'), icon: 'plus', run: c('color.edit'), testid: 'mi-edit-colors' }];
      case 'image':
        return [
          { label: t('image.crop'), icon: 'crop', key: t('key.crop'), run: c('image.crop'), disabled: !hasSel, testid: 'mi-image-crop' },
          { label: t('image.resize'), icon: 'resize', key: 'Ctrl+W', run: c('image.resize'), testid: 'mi-image-resize' },
          { label: t('image.rotate'), icon: 'rotate', items: this.menuItems('rotate'), testid: 'mi-image-rotate' },
          { label: t('image.flip'), icon: 'flip', items: this.menuItems('flip'), testid: 'mi-image-flip' },
          { label: t('image.removeBg'), icon: 'removeBg', run: c('image.removeBg'), testid: 'mi-image-remove-bg' },
        ];
      default:
        return [];
    }
  }

  openMenu(name, anchor, keyboard = false) {
    const label = anchor.getAttribute('aria-label') || anchor.textContent.trim();
    this.popups.open(anchor, this.menuItems(name), { testid: `popup-${name}`, label, noIcons: name === 'zoom', keyboard });
  }

  selectTool(name) {
    this.lastSelectTool = name;
    this.setTool(name);
  }

  setStyle(which, value) {
    if (which === 'outline') this.engine.set_outline(value);
    else this.engine.set_fill(value);
    this.changed();
  }

  gallery(kind, names, preview, pick, active, current = active) {
    this.previewCache = this.previewCache || {};
    const el = document.createElement('div');
    el.className = `gallery ${kind}`;
    for (const name of names) {
      const b = document.createElement('button');
      b.title = t(`${kind === 'brushes' ? 'brush' : 'shape'}.${name}`);
      b.dataset.testid = `${kind === 'brushes' ? 'brush' : 'shape'}-${name}`;
      b.classList.toggle('active', name === active);
      if (name === current) b.dataset.current = 'true';
      const key = `${kind}:${name}`;
      if (!this.previewCache[key]) this.previewCache[key] = preview(name);
      const cv = this.previewCache[key];
      const copy = document.createElement('canvas');
      copy.width = cv.width;
      copy.height = cv.height;
      copy.getContext('2d').drawImage(cv, 0, 0);
      b.appendChild(copy);
      b.addEventListener('click', () => {
        this.popups.close();
        pick(name);
      });
      el.appendChild(b);
    }
    return el;
  }

  sizePanel() {
    const el = document.createElement('div');
    el.className = 'size-panel';
    const size = this.state.size;
    el.innerHTML = `<div class="size-row"><input type="range" min="1" max="100" value="${size}" data-testid="size-slider"><output>${size}px</output></div><div class="size-presets"></div>`;
    const range = el.querySelector('input');
    const out = el.querySelector('output');
    range.addEventListener('input', () => {
      out.textContent = `${range.value}px`;
      this.engine.set_size(Number(range.value));
      this.changed();
    });
    const presets = el.querySelector('.size-presets');
    for (const s of SIZE_PRESETS) {
      const b = document.createElement('button');
      b.dataset.testid = `size-${s}`;
      b.classList.toggle('active', s === size);
      b.innerHTML = `<span style="width:34px">${s}px</span><span class="bar" style="height:${Math.min(s, 16)}px"></span>`;
      b.addEventListener('click', () => {
        this.popups.close();
        this.engine.set_size(s);
        this.changed();
      });
      presets.appendChild(b);
    }
    return el;
  }

  showContextMenu(x, y) {
    const st = this.state;
    const hasSel = !!st.selection;
    const c = (id) => () => this.run(id);
    this.popups.open(
      { x, y },
      [
        { label: t('edit.cut'), icon: 'cut', key: 'Ctrl+X', run: c('edit.cut'), disabled: !hasSel, testid: 'ctx-cut' },
        { label: t('edit.copy'), icon: 'copy', key: 'Ctrl+C', run: c('edit.copy'), disabled: !hasSel, testid: 'ctx-copy' },
        { label: t('edit.paste'), icon: 'paste', key: 'Ctrl+V', run: c('edit.paste'), testid: 'ctx-paste' },
        { sep: true },
        { label: t('image.crop'), icon: 'crop', key: t('key.crop'), run: c('image.crop'), disabled: !hasSel, testid: 'ctx-crop' },
        { label: t('edit.selectAll'), icon: 'selectAll', key: 'Ctrl+A', run: c('edit.selectAll'), testid: 'ctx-select-all' },
        { label: t('edit.invert'), icon: 'invert', run: c('edit.invert'), testid: 'ctx-invert' },
        { label: t('edit.delete'), icon: 'trash', key: t('key.del'), run: c('edit.delete'), disabled: !hasSel, testid: 'ctx-delete' },
        { sep: true },
        { label: t('image.rotate'), icon: 'rotate', items: this.menuItems('rotate'), testid: 'ctx-rotate' },
        { label: t('image.flip'), icon: 'flip', items: this.menuItems('flip'), testid: 'ctx-flip' },
        { label: t('image.resize'), icon: 'resize', key: 'Ctrl+W', run: c('image.resize'), testid: 'ctx-resize' },
      ],
      { testid: 'popup-context' },
    );
  }

  switchLanguage(l) {
    setLanguage(l);
    this.previewCache = {};
    this.buildPalette();
    this.layers.key = '';
    this.changed({ layers: true });
    this.updateFileStatus();
  }

  // ----- keyboard ----------------------------------------------------------------

  bindKeyboard() {
    // File and view shortcuts that also work while typing in the text box.
    const global = new Set(['C-s', 'C-S-s', 'F12', 'C-o', 'C-n', 'C-p', 'F11']);
    document.addEventListener('keydown', (e) => {
      const target = e.target;
      // Nothing reaches the picture while a modal dialog is open.
      if (document.querySelector('dialog[open]')) return;
      const ctrl = e.ctrlKey || e.metaKey;
      // Digits and +/- by physical key, so AZERTY keyboards work (Ctrl+0 is Ctrl+à there).
      const byCode = { Digit0: '0', Numpad0: '0', Digit1: '1', Numpad1: '1', Equal: '=', NumpadAdd: '+', Minus: '-', NumpadSubtract: '-' };
      const k = ctrl && byCode[e.code] ? byCode[e.code] : e.key.length === 1 ? e.key.toLowerCase() : e.key;
      const combo = `${ctrl ? 'C-' : ''}${e.shiftKey ? 'S-' : ''}${e.altKey ? 'A-' : ''}${k}`;
      const typing = target.closest && target.closest('input, textarea, select');
      if (typing && !(target === this.text.box && global.has(combo))) return;
      // A focused toolbar button keeps its native Enter/Space activation.
      if ((e.key === 'Enter' || e.key === ' ') && target.closest && target.closest('button')) return;
      const map = {
        'C-n': 'file.new',
        'C-o': 'file.open',
        'C-s': 'file.save',
        'C-S-s': 'file.saveAs.png',
        F12: 'file.saveAs.png',
        'C-p': 'file.print',
        'C-e': 'file.properties',
        'C-w': 'image.resize',
        'C-z': 'edit.undo',
        'C-y': 'edit.redo',
        'C-S-z': 'edit.redo',
        'C-x': 'edit.cut',
        'C-c': 'edit.copy',
        'C-a': 'edit.selectAll',
        'C-S-x': 'image.crop',
        Delete: 'edit.delete',
        'C-+': 'view.zoomIn',
        'C-=': 'view.zoomIn',
        'C-S-+': 'view.zoomIn',
        'C--': 'view.zoomOut',
        'C-PageUp': 'view.zoomIn',
        'C-PageDown': 'view.zoomOut',
        'C-1': 'view.actual',
        'C-0': 'view.fit',
        'C-r': 'view.rulers',
        'C-g': 'view.grid',
        F11: 'view.fullscreen',
        'C-S-n': 'layers.add',
      };
      if (combo === 'C-v') {
        // In browsers the native "paste" event carries the clipboard without a permission prompt.
        if (platform.isDesktop) {
          e.preventDefault();
          this.paste();
        }
        return;
      }
      const id = map[combo];
      if (id) {
        e.preventDefault();
        this.popups.close();
        this.run(id);
        return;
      }
      if (this.popups.isOpen) return;
      const arrows = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
      const st = this.state;
      const movable = (st.selection && !st.selection.drawing) || (st.shape_edit && st.shape_edit.stage === 'editing');
      if (arrows[e.key] && movable && !target.closest('button, [role=toolbar]')) {
        e.preventDefault();
        const [dx, dy] = arrows[e.key];
        const step = e.shiftKey ? 10 : 1;
        this.engine.nudge(dx * step, dy * step);
        this.changed();
        return;
      }
      if (e.key === 'Escape') {
        if (this.view.cancelCanvasResize()) return;
        this.engine.escape();
        this.changed({ layers: true });
      } else if (e.key === 'Enter') {
        this.engine.enter();
        this.changed({ layers: true });
      }
    });
  }

  // ----- back / forward = undo / redo ------------------------------------------------

  /**
   * The Back/Forward buttons of a mouse undo and redo (browser and desktop). In
   * the browser, its own Back/Forward buttons do the same instead of leaving the
   * page: the page keeps itself between two history entries and turns a visit
   * to either of them into undo or redo.
   */
  bindBackForward() {
    const nav = (dir) => this.run(dir < 0 ? 'edit.undo' : 'edit.redo');
    if (platform.isDesktop) {
      // Mouse buttons 3 (back) and 4 (forward); in a browser they go through the history below.
      for (const type of ['mousedown', 'mouseup']) {
        window.addEventListener(type, (e) => {
          if (e.button !== 3 && e.button !== 4) return;
          e.preventDefault();
          if (type === 'mouseup' && !document.querySelector('dialog[open]')) nav(e.button === 3 ? -1 : 1);
        });
      }
      return;
    }
    // History entries must be created after a user gesture, or browsers skip them.
    const arm = () => {
      window.removeEventListener('pointerdown', arm, true);
      window.removeEventListener('keydown', arm, true);
      history.replaceState({ paint: 'back' }, '');
      history.pushState({ paint: 'home' }, '');
      history.pushState({ paint: 'forward' }, '');
      history.back();
    };
    window.addEventListener('pointerdown', arm, true);
    window.addEventListener('keydown', arm, true);
    window.addEventListener('popstate', (e) => {
      const where = e.state && e.state.paint;
      if (where === 'back') {
        history.forward();
        if (!document.querySelector('dialog[open]')) nav(-1);
      } else if (where === 'forward') {
        history.back();
        if (!document.querySelector('dialog[open]')) nav(1);
      }
    });
  }

  // ----- clipboard -------------------------------------------------------------------

  bindClipboardEvents() {
    document.addEventListener('paste', (e) => {
      if (platform.isDesktop) return;
      if (e.target.closest && e.target.closest('input, textarea')) return;
      if (document.querySelector('dialog[open]')) return;
      e.preventDefault();
      const items = [...(e.clipboardData ? e.clipboardData.items : [])];
      const item = items.find((i) => i.kind === 'file' && i.type.startsWith('image/'));
      if (item) {
        item.getAsFile().arrayBuffer().then((buf) => this.pasteBytes(new Uint8Array(buf)));
      } else if (this.clipboardFallback) {
        this.pasteBytes(this.clipboardFallback);
      } else {
        this.toast(t('msg.clipboardEmpty'));
      }
    });
  }

  async copy() {
    this.commitTransient();
    const png = this.engine.copy_png();
    if (!png) return false;
    this.clipboardFallback = png;
    try {
      await platform.clipboardWritePng(png);
    } catch (err) {
      console.warn('system clipboard unavailable, kept internally', err);
    }
    return true;
  }

  async cut() {
    this.commitTransient();
    const png = this.engine.cut_png();
    this.changed({ layers: true });
    if (!png) return false;
    this.clipboardFallback = png;
    try {
      await platform.clipboardWritePng(png);
    } catch (err) {
      console.warn('system clipboard unavailable, kept internally', err);
    }
    return true;
  }

  async paste() {
    this.commitTransient();
    let bytes = null;
    try {
      bytes = await platform.clipboardReadImage();
    } catch (err) {
      console.warn('cannot read the system clipboard', err);
    }
    bytes = bytes || this.clipboardFallback;
    if (!bytes) {
      this.toast(t('msg.clipboardEmpty'));
      return false;
    }
    return this.pasteBytes(bytes);
  }

  pasteBytes(bytes) {
    this.commitTransient();
    const o = this.view.visibleOrigin();
    try {
      this.engine.paste_bytes(bytes, o.x, o.y);
    } catch (err) {
      this.toast(String(err).includes('too large') ? t('msg.tooLarge') : t('msg.openFailed', { error: String(err) }));
      return false;
    }
    this.lastSelectTool = 'select-rect';
    this.changed({ layers: true });
    return true;
  }

  async pasteFromFile() {
    const f = await platform.openImageFile('Images');
    if (f) this.pasteBytes(f.bytes);
  }

  // ----- files -----------------------------------------------------------------------

  /** Resolves true when the current picture may be replaced. */
  async confirmDiscard() {
    this.commitTransient();
    if (!this.engine.is_modified()) return true;
    const answer = await confirmSave(this.file.name || t('app.untitled'));
    if (answer === 'cancel') return false;
    if (answer === 'save') return this.save();
    return true;
  }

  async newImage() {
    if (!(await this.confirmDiscard())) return;
    this.engine.new_document(DEFAULT_W, DEFAULT_H);
    this.file = { name: null, path: null, format: null, size: null };
    this.afterLoad();
  }

  async openImage() {
    if (!(await this.confirmDiscard())) return;
    let f;
    try {
      f = await platform.openImageFile('Images');
    } catch (err) {
      await message(t('dlg.error'), t('msg.openFailed', { error: String(err) }));
      return;
    }
    if (f) this.loadFile(f);
  }

  loadFile(f) {
    try {
      this.engine.open_bytes(f.bytes);
    } catch (err) {
      message(t('dlg.error'), t('msg.openFailed', { error: String(err) }));
      return false;
    }
    this.file = { name: f.name, path: f.path, format: platform.formatOf(f.name), size: f.bytes.length };
    this.afterLoad();
    return true;
  }

  afterLoad() {
    this.view.blit();
    this.view.fitIfLarge();
    this.updateFileStatus();
    this.changed({ layers: true });
  }

  async loadInitialFile() {
    try {
      const f = await platform.initialFile();
      if (f) this.loadFile(f);
    } catch (err) {
      console.warn(err);
    }
    // Installed web app opened from the file manager ("Open with Paint").
    if ('launchQueue' in window) {
      window.launchQueue.setConsumer(async (params) => {
        const handle = params.files && params.files[0];
        if (!handle) return;
        const file = await handle.getFile();
        if (await this.confirmDiscard()) this.loadFile({ name: file.name, path: null, bytes: new Uint8Array(await file.arrayBuffer()) });
      });
    }
  }

  /** Saves to the current file (or asks for one). Resolves true when saved. */
  async save() {
    if (this.file.path || (!platform.isDesktop && this.file.name)) {
      return this.write(this.file.format || platform.FORMATS[0], this.file.path, this.file.name);
    }
    return this.saveAs(this.file.format || platform.FORMATS[0]);
  }

  async saveAs(format) {
    const base = (this.file.name || t('app.untitled')).replace(/\.[^.]+$/, '');
    return this.write(format, null, `${base}.${format.ext}`);
  }

  async write(format, path, name) {
    // Holding Ctrl+S must not start overlapping saves of the same file.
    if (this.saving) return this.saving;
    this.saving = this.writeNow(format, path, name).finally(() => {
      this.saving = null;
    });
    return this.saving;
  }

  async writeNow(format, path, name) {
    this.commitTransient();
    let bytes;
    try {
      let target = path;
      if (platform.isDesktop && !target) {
        target = await platform.chooseSavePath(name, format);
        if (!target) return false;
      }
      // The extension typed in the file dialog decides the format.
      const fmt = (target && platform.formatOf(target)) || format;
      bytes = this.engine.export(fmt.ext, 92);
      const res = await platform.saveBytes(bytes, name, fmt, target);
      if (!res) return false;
      this.file = { name: res.name, path: res.path, format: fmt, size: bytes.length };
    } catch (err) {
      await message(t('dlg.error'), t('msg.saveFailed', { error: String(err) }));
      return false;
    }
    this.engine.mark_saved();
    this.updateFileStatus();
    this.toast(t('msg.saved', { name: this.file.name }));
    this.changed();
    return true;
  }

  bindDragDrop() {
    if (platform.isDesktop) {
      const ev = window.__TAURI__.event;
      ev.listen('tauri://drag-drop', async (e) => {
        const paths = (e.payload && e.payload.paths) || [];
        const path = paths.find((p) => platform.formatOf(p));
        if (path && (await this.confirmDiscard())) this.loadFile(await platform.readPath(path));
      });
      return;
    }
    const vp = document.getElementById('viewport');
    vp.addEventListener('dragover', (e) => {
      if (e.dataTransfer && [...e.dataTransfer.types].includes('Files')) e.preventDefault();
    });
    vp.addEventListener('drop', async (e) => {
      e.preventDefault();
      const file = [...e.dataTransfer.files].find((f) => f.type.startsWith('image/') || platform.formatOf(f.name));
      if (!file || !(await this.confirmDiscard())) return;
      this.loadFile({ name: file.name, path: null, bytes: new Uint8Array(await file.arrayBuffer()) });
    });
  }

  // ----- image dialogs ---------------------------------------------------------------

  async resizeSkew() {
    this.commitTransient();
    const sel = this.state.selection;
    const base = sel && !sel.drawing ? { w: sel.w, h: sel.h } : { w: this.state.width, h: this.state.height };
    const r = await resizeDialog(base);
    if (!r) return;
    if (r.w !== base.w || r.h !== base.h) this.sizeChecked(this.engine.resize_image(r.w, r.h));
    if (r.sx || r.sy) this.sizeChecked(this.engine.skew(r.sx, r.sy));
    this.changed({ layers: true });
  }

  async properties() {
    this.commitTransient();
    const r = await propertiesDialog(this.state.width, this.state.height);
    if (!r) return;
    this.sizeChecked(this.engine.resize_canvas(r.w, r.h));
    this.changed({ layers: true });
  }
}
