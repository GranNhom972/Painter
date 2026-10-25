// Shared helpers for the end-to-end tests (browser build of the UI).
import { expect } from '@playwright/test';

export async function openApp(page) {
  await page.goto('/');
  await page.waitForSelector('body[data-ready=true]');
}

/** Engine state after flushing pending UI work. */
export async function state(page) {
  return page.evaluate(() => {
    window.paint.flush();
    return window.paint.state;
  });
}

/** Visible colour of an image pixel, from the engine. */
export async function pixel(page, x, y) {
  return page.evaluate(([x, y]) => window.paint.engine.pixel(x, y), [x, y]);
}

/** Colour actually displayed by the <canvas> element (checks the WebAssembly → canvas blit). */
export async function screenPixel(page, x, y) {
  return page.evaluate(([x, y]) => {
    window.paint.flush();
    const d = document.getElementById('canvas').getContext('2d').getImageData(x, y, 1, 1).data;
    return '#' + [d[0], d[1], d[2]].map((v) => v.toString(16).padStart(2, '0')).join('');
  }, [x, y]);
}

/** Screen position of an image coordinate (pixel centre). */
export async function toScreen(page, x, y) {
  const box = await page.locator('#canvas').boundingBox();
  const zoom = await page.evaluate(() => window.paint.view.zoom);
  return { x: box.x + (x + 0.5) * zoom, y: box.y + (y + 0.5) * zoom };
}

/** Drags in image coordinates through all `points`. */
export async function drag(page, points, { button = 'left', modifiers = [], steps = 6 } = {}) {
  for (const m of modifiers) await page.keyboard.down(m);
  const first = await toScreen(page, ...points[0]);
  await page.mouse.move(first.x, first.y);
  await page.mouse.down({ button });
  for (const p of points.slice(1)) {
    const s = await toScreen(page, ...p);
    await page.mouse.move(s.x, s.y, { steps });
  }
  await page.mouse.up({ button });
  for (const m of modifiers) await page.keyboard.up(m);
}

export async function clickAt(page, x, y, { button = 'left', clickCount = 1 } = {}) {
  const s = await toScreen(page, x, y);
  await page.mouse.click(s.x, s.y, { button, clickCount });
}

/** Clicks a ribbon split button's arrow (opens its menu). */
export async function openRibbonMenu(page, testid) {
  await page.click(`[data-testid=${testid}] [data-icon=chevron]`);
}

/** Opens a menu-bar menu and clicks an item (optionally inside a submenu). */
export async function menu(page, top, item, sub) {
  await page.click(`[data-testid=menu-${top}]`);
  if (sub) {
    await page.hover(`[data-testid=${item}]`);
    await page.click(`[data-testid=${sub}]`);
  } else {
    await page.click(`[data-testid=${item}]`);
  }
}

/** Fills the canvas (or the active layer) with a colour using the bucket. */
export async function fillCanvas(page, hex, at = [20, 20]) {
  await page.evaluate((hex) => {
    window.paint.engine.set_color1(hex);
  }, hex);
  await page.click('[data-testid=tool-fill]');
  await clickAt(page, ...at);
}

/** Draws a filled rectangle with the engine API (fast fixture). */
export async function rect(page, x, y, w, h, hex) {
  await page.evaluate(([x, y, w, h, hex]) => {
    const e = window.paint.engine;
    const st = JSON.parse(e.state_json());
    e.set_shape('rectangle');
    e.set_outline('none');
    e.set_fill('solid');
    const c2 = st.color2;
    e.set_color2(hex);
    e.pointer_down(x, y, 0, false, false, false);
    e.pointer_move(x + w - 1, y + h - 1, false, false, false);
    e.pointer_up(x + w - 1, y + h - 1);
    e.enter();
    e.set_color2(c2);
    e.set_outline(st.outline);
    e.set_fill(st.fill);
    e.set_tool(st.tool);
    window.paint.changed({ layers: true });
  }, [x, y, w, h, hex]);
}

export async function expectPixel(page, x, y, hex) {
  await expect.poll(() => pixel(page, x, y)).toBe(hex);
}

/** A PNG made in the page (simulates an image coming from another program). */
export async function makePng(page, w, h, hex) {
  const bytes = await page.evaluate(async ([w, h, hex]) => {
    const c = document.createElement('canvas');
    c.width = w;
    c.height = h;
    const ctx = c.getContext('2d');
    ctx.fillStyle = hex;
    ctx.fillRect(0, 0, w, h);
    const blob = await new Promise((r) => c.toBlob(r, 'image/png'));
    return [...new Uint8Array(await blob.arrayBuffer())];
  }, [w, h, hex]);
  return Buffer.from(bytes);
}

export function pngSize(buf) {
  expect(buf.subarray(1, 4).toString()).toBe('PNG');
  return { w: buf.readUInt32BE(16), h: buf.readUInt32BE(20) };
}
