// Icons of Windows 11 Paint, as SVG.
// Generic glyphs are the Fluent UI System Icons Paint uses (path data in
// fluent-paths.js, MIT); Paint's own tool pictures (rotate, flip, remove
// background, brush, shapes, size, text...) are drawn here on the same grid.
// Colours come from CSS variables so both themes work.

import { FLUENT as F } from './fluent-paths.js';

let uid = 0;
const svg = (size, body, cls = 'ico') => `<svg viewBox="0 0 ${size} ${size}" class="${cls}" aria-hidden="true">${body}</svg>`;
const path = (d, fill = 'currentColor') => `<path d="${d}" fill="${fill}"/>`;
const stroke = (d, w = 1, color = 'currentColor') => `<path d="${d}" fill="none" stroke="${color}" stroke-width="${w}" stroke-linecap="round" stroke-linejoin="round"/>`;
/** Plain Fluent glyph. */
const fluent = (name) => {
  const size = Number(name.match(/_(\d+)_/)[1]);
  return svg(size, path(F[name]));
};
/**
 * Two-tone Paint icon: the filled Fluent shape in a colour, clipped to `clip`
 * (a polygon), under the regular outline.
 */
const twoTone = (base, size, colour, clip) => {
  const id = `ic${++uid}`;
  const clipDef = clip ? `<clipPath id="${id}"><polygon points="${clip}"/></clipPath>` : '';
  return svg(size, `${clipDef}<path d="${F[`${base}_${size}_filled`]}" fill="${colour}"${clip ? ` clip-path="url(#${id})"` : ''}/>${path(F[`${base}_${size}_regular`])}`);
};

const ACCENT = 'var(--icon-accent)';
const YELLOW = 'var(--icon-yellow)';
const PINK = 'var(--icon-pink)';
const RED = 'var(--icon-red)';
const GRAY = 'var(--icon-gray)';
const SURFACE = 'var(--bg-ribbon)';

export const icons = {
  // --- menu bar ------------------------------------------------------------
  save: fluent('save_20_regular'),
  undo: fluent('arrow_undo_20_regular'),
  redo: fluent('arrow_redo_20_regular'),
  gear: fluent('settings_20_regular'),
  chevron: svg(12, path(F.chevron_down_12_regular), 'chev'),
  chevronRight: fluent('chevron_right_12_regular'),

  // --- selection --------------------------------------------------------------
  selectRect: svg(20, '<rect x="3.5" y="3.5" width="13" height="13" rx="1" fill="none" stroke="currentColor" stroke-dasharray="2.5 1.83" stroke-dashoffset="1.25"/>'),
  selectRectBig: svg(24, '<rect x="3.5" y="3.5" width="17" height="17" fill="none" stroke="currentColor" stroke-dasharray="3 2.03" stroke-dashoffset="1.5"/>'),
  selectFree: svg(20, stroke('M5.5 6.2C7 3.6 11.4 2.6 14.3 4.1s3.6 4.8 1.6 7.2c-1.9 2.3-5.8 2.1-7.2 4.4', 1, 'currentColor').replace('stroke-linejoin="round"', 'stroke-linejoin="round" stroke-dasharray="2 1.6"') + stroke('M7.6 16.6 6.2 18', 1)),
  selectAll: fluent('select_all_on_20_regular'),
  invert: svg(20, stroke('M3.5 3.5h13v13h-13z', 1).replace('stroke-linejoin="round"', 'stroke-linejoin="round" stroke-dasharray="2 1.5"') + `<rect x="7" y="7" width="6" height="6" rx="1" fill="${ACCENT}"/>`),
  trash: fluent('delete_20_regular'),
  transparent: svg(20, `<rect x="3.5" y="3.5" width="13" height="13" rx="2" fill="none" stroke="currentColor"/>` + path('M4 4h4v4H4zM12 4h4v4h-4zM8 8h4v4H8zM4 12h4v4H4zM12 12h4v4h-4z', GRAY)),

  // --- image -------------------------------------------------------------------
  crop: fluent('crop_20_regular'),
  rotate: svg(20, stroke('M5 7.5C5.4 4.6 9.5 3.2 12.2 4.6c1.2.6 2 1.5 2.4 2.6', 1) + stroke('M14.9 4.3l-.3 3-2.9-.6', 1) + `<path d="M4.5 16.5h11v-6.5z" fill="${ACCENT}" stroke="${ACCENT}" stroke-linejoin="round"/>`),
  rotateLeft: fluent('arrow_rotate_counterclockwise_20_regular'),
  rotate180: svg(20, stroke('M4 11a6 6 0 0 1 12 0', 1) + stroke('M14 9l2 2 2-2', 1) + `<path d="M6 13.5h8l-4 4z" fill="${ACCENT}"/>`),
  flip: svg(20, `<path d="M4.5 3.5v6h11z" fill="${ACCENT}" stroke="${ACCENT}" stroke-linejoin="round"/>` + stroke('M4.5 10.5v6h11z', 1)),
  flipV: svg(20, `<path d="M3.5 4.5h6v11z" fill="${ACCENT}" stroke="${ACCENT}" stroke-linejoin="round"/>` + stroke('M10.5 4.5h6l-6 11z', 1)),
  removeBg: (() => {
    const id = `ic${++uid}`;
    return svg(
      20,
      `<clipPath id="${id}"><rect x="3" y="3" width="14" height="14" rx="2"/></clipPath>` +
        `<g clip-path="url(#${id})">${stroke('M1 9 9 1M1 14 14 1M1 19 19 1M6 19 19 6M11 19 19 11', 1)}</g>` +
        `<circle cx="10" cy="8" r="3" fill="${SURFACE}" stroke="currentColor"/>` +
        `<path d="M4.6 17.5c.6-2.8 2.8-4.3 5.4-4.3s4.8 1.5 5.4 4.3z" fill="${SURFACE}" stroke="currentColor" stroke-linejoin="round"/>`,
    );
  })(),
  resize: svg(
    24,
    stroke('M3.5 8V5.5a2 2 0 0 1 2-2H8M11 3.5h2.5M20.5 13.5V16M20.5 19v-.5', 1).replace('<path', '<path opacity=".9"') +
      stroke('M16.5 20.5h2a2 2 0 0 0 2-2', 1) +
      `<rect x="3.5" y="10.5" width="10" height="10" rx="1.5" fill="none" stroke="${ACCENT}"/>` +
      stroke('M4 19.5l3.3-3.3 2.4 2.4 1.6-1.6 2.2 2.2', 1, ACCENT) +
      `<circle cx="10.5" cy="13.6" r="1.1" fill="none" stroke="${ACCENT}"/>` +
      stroke('M14.5 9.5l6-6M15.5 3.5h5v5', 1, ACCENT),
  ),

  // --- tools -------------------------------------------------------------------
  pencil: twoTone('edit', 20, YELLOW),
  fill: (() => {
    const a = `ic${++uid}`;
    const b = `ic${++uid}`;
    return svg(
      20,
      `<clipPath id="${a}"><polygon points="0,0 14,0 14,13 0,13"/></clipPath><clipPath id="${b}"><polygon points="14,10 20,10 20,20 14,20"/></clipPath>` +
        `<path d="${F.paint_bucket_20_filled}" fill="${ACCENT}" clip-path="url(#${a})"/>` +
        `<path d="${F.paint_bucket_20_filled}" fill="${RED}" clip-path="url(#${b})"/>` +
        path(F.paint_bucket_20_regular),
    );
  })(),
  text: svg(20, stroke('M4.5 16.5 10 3.5l5.5 13M6.6 11.5h6.8', 1.1)),
  eraser: twoTone('eraser', 20, PINK, '0,0 20,0 20,4 4,20 0,20'),
  picker: twoTone('eyedropper', 20, ACCENT, '7,0 20,0 20,13'),
  magnifier: fluent('zoom_in_20_regular'),

  // --- brushes, shapes, size, colours, layers (24 px, as in Paint) ---------------
  brush: svg(
    24,
    `<path d="M7.6 13.5h8.8l-3.3 8a1.2 1.2 0 0 1-2.2 0z" fill="${GRAY}"/>` +
      stroke('M7.5 2.5h9v6.5h-9zM10.5 2.5v3.5M13.5 2.5v3.5M7.5 9l-.4 2.6c-.1.7.1 1.4.4 2l3.4 7.9a1.2 1.2 0 0 0 2.2 0l3.4-7.9c.3-.6.5-1.3.4-2L16.5 9', 1),
  ),
  shapes: svg(24, `<circle cx="9" cy="9" r="6.5" fill="none" stroke="${ACCENT}"/>` + `<rect x="10.5" y="10.5" width="11" height="11" rx="2.5" fill="${SURFACE}" stroke="currentColor"/>`),
  outline: svg(20, `<rect x="4" y="4" width="12" height="12" rx="2" fill="none" stroke="currentColor" stroke-width="1.6"/>`),
  fillStyle: svg(20, `<rect x="4" y="4" width="12" height="12" rx="2" fill="${ACCENT}" stroke="currentColor"/>`),
  size: svg(24, stroke('M3.5 6.5c3-1.6 5.6 1.6 8.5 0s5.5-1.6 8.5 0', 1) + stroke('M3.5 11.5c3-1.6 5.6 1.6 8.5 0s5.5-1.6 8.5 0', 1.6) + stroke('M3.5 17c3-1.6 5.6 1.6 8.5 0s5.5-1.6 8.5 0', 2.4)),
  layers: fluent('layer_24_regular'),

  // --- status bar --------------------------------------------------------------
  pointer: fluent('cursor_20_regular'),
  selSize: svg(20, '<rect x="3.5" y="3.5" width="13" height="13" rx="1" fill="none" stroke="currentColor" stroke-dasharray="2.5 1.83" stroke-dashoffset="1.25"/>'),
  canvasSize: svg(20, '<rect x="5.5" y="5.5" width="9" height="9" fill="none" stroke="currentColor"/>' + stroke('M5.5 2.5v3M2.5 5.5h3M14.5 2.5v3M17.5 5.5h-3M5.5 17.5v-3M2.5 14.5h3M14.5 17.5v-3M17.5 14.5h-3', 1)),
  disk: fluent('save_20_regular'),
  fit: svg(20, stroke('M3.5 7V5.5a2 2 0 0 1 2-2H7M13 3.5h1.5a2 2 0 0 1 2 2V7M16.5 13v1.5a2 2 0 0 1-2 2H13M7 16.5H5.5a2 2 0 0 1-2-2V13', 1) + '<circle cx="10" cy="10" r="3" fill="currentColor"/>'),
  zoomOut: fluent('zoom_out_20_regular'),
  zoomIn: fluent('zoom_in_20_regular'),

  // --- panels and menus ----------------------------------------------------------
  plus: fluent('add_20_regular'),
  eye: fluent('eye_20_regular'),
  eyeOff: fluent('eye_off_20_regular'),
  duplicate: fluent('copy_20_regular'),
  mergeDown: fluent('arrow_download_20_regular'),
  up: fluent('arrow_up_20_regular'),
  down: fluent('arrow_down_20_regular'),
  close: svg(16, path(F.dismiss_16_regular)),
  check: svg(16, path(F.checkmark_16_regular)),
  copy: fluent('copy_20_regular'),
  cut: fluent('cut_20_regular'),
  paste: fluent('clipboard_paste_20_regular'),
  bold: fluent('text_bold_20_regular'),
  italic: fluent('text_italic_20_regular'),
  underline: fluent('text_underline_20_regular'),
  strike: fluent('text_strikethrough_20_regular'),
  textBg: svg(20, `<rect x="3" y="3.5" width="14" height="13" rx="2" fill="${GRAY}" opacity=".45"/>` + stroke('M6.5 14 10 6l3.5 8M7.6 11.6h4.8', 1)),

  // --- window (desktop title bar, Windows 11 caption buttons) --------------------
  winMinimize: svg(16, '<path d="M3 8.5h10" stroke="currentColor" stroke-width="1"/>', 'cap'),
  winMaximize: svg(16, '<rect x="3.5" y="3.5" width="9" height="9" rx="1" fill="none" stroke="currentColor"/>', 'cap'),
  winRestore: svg(16, '<rect x="3.5" y="5.5" width="7" height="7" rx="1" fill="none" stroke="currentColor"/><path d="M5.5 5.5v-1a1 1 0 0 1 1-1h5a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-1" fill="none" stroke="currentColor"/>', 'cap'),
  winClose: svg(16, '<path d="M3.5 3.5l9 9M12.5 3.5l-9 9" stroke="currentColor" stroke-width="1"/>', 'cap'),
};

export function icon(name) {
  return icons[name] || '';
}
