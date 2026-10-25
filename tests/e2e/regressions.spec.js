// Regression tests for the bugs found by the exploratory QA and the code review.
import { test, expect } from '@playwright/test';
import { openApp, state, pixel, drag, clickAt, rect, toScreen, makePng, menu, expectPixel } from './helpers.js';

test.beforeEach(async ({ page }) => openApp(page));

test('Cancel closes dialogs even with an invalid field', async ({ page }) => {
  await page.click('[data-testid=btn-resize]');
  await page.locator('[data-testid=resize-w]').fill('0');
  await page.click('[data-testid=dlg-resize] [data-testid=dlg-cancel]');
  await expect(page.locator('[data-testid=dlg-resize]')).toBeHidden();
  await page.click('[data-testid=btn-edit-colors]');
  await page.locator('[data-testid=color-r]').fill('300');
  await expect(page.locator('[data-testid=color-hex]')).toHaveValue('#ff0000');
  await page.click('[data-testid=dlg-colors] [data-testid=dlg-cancel]');
  await expect(page.locator('[data-testid=dlg-colors]')).toBeHidden();
});

test('a too large resize is refused with a message instead of crashing', async ({ page }) => {
  await page.click('[data-testid=btn-resize]');
  await page.click('[data-testid=resize-pixels]');
  await page.locator('[data-testid=resize-w]').fill('16384');
  await page.click('[data-testid=dlg-resize] [data-testid=dlg-ok]');
  await expect(page.locator('#toast')).toContainText('Image trop grande');
  const st = await state(page);
  expect([st.width, st.height]).toEqual([1152, 648]);
  // The engine still works.
  await page.click('[data-testid=tool-pencil]');
  await drag(page, [[10, 10], [40, 10]]);
  expect(await pixel(page, 20, 10)).toBe('#000000');
});

test('Enter after clicking a toolbar button does not click it again', async ({ page }) => {
  await page.click('[data-testid=btn-layers]');
  await page.click('[data-testid=layer-add]');
  await page.click('[data-testid=layer-add]');
  await page.click('[data-testid=layer-delete]');
  await expect(page.locator('.layer-item')).toHaveCount(2);
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Control+c');
  await page.keyboard.press('Enter');
  await expect(page.locator('.layer-item')).toHaveCount(2);
  await page.click('[data-testid=qa-undo]');
  await page.keyboard.press('Enter');
  expect((await state(page)).layers.length).toBe(3); // one undo only
});

test('a selection can start on the first canvas pixel (handles are outside)', async ({ page }) => {
  await page.click('[data-testid=btn-selection] .big-ico');
  await drag(page, [[0, 0], [200, 150]]);
  const st = await state(page);
  expect([st.width, st.height]).toEqual([1152, 648]);
  expect(st.selection).toMatchObject({ x: 0, y: 0, w: 200, h: 150 });
});

test('layer thumbnails: dragging works and leaves nothing behind for the next click', async ({ page }) => {
  await page.click('[data-testid=btn-layers]');
  await page.click('[data-testid=layer-add]');
  await rect(page, 0, 0, 50, 50, '#ed1c24');
  const thumb = await page.locator('[data-testid=layer-1] img').boundingBox();
  const target = await page.locator('[data-testid=layer-0]').boundingBox();
  await page.mouse.move(thumb.x + 10, thumb.y + 10);
  await page.mouse.down();
  await page.mouse.move(target.x + 20, target.y + 20, { steps: 6 });
  await page.mouse.up();
  const order = async () => (await state(page)).layers.map((l) => l.id);
  // Dragging the thumbnail moved the red layer under the white one.
  expect(await order()).toEqual([2, 1]);
  expect(await pixel(page, 10, 10)).toBe('#ffffff');
  // A later plain click only selects; it does not reorder again.
  await page.click('[data-testid=layer-1] .layer-name');
  expect(await order()).toEqual([2, 1]);
});

test('text tool: size field, palette clicks and layer changes keep the text right', async ({ page }) => {
  await page.click('[data-testid=btn-layers]');
  await page.click('[data-testid=layer-add]');
  await page.click('[data-testid=tool-text]');
  await clickAt(page, 30, 30);
  await page.keyboard.type('Hi');
  // Typing a size: the digits go to the field only, and the size is applied.
  await page.locator('[data-testid=text-size]').fill('72');
  await page.locator('[data-testid=text-size]').press('Enter');
  await expect(page.locator('[data-testid=text-box]')).toHaveValue('Hi');
  expect(await page.evaluate(() => window.paint.text.opts.size)).toBe(72);
  // A palette click keeps the caret in the text box.
  await page.click('[data-testid=palette-3]');
  await page.keyboard.type(' you');
  await expect(page.locator('[data-testid=text-box]')).toHaveValue('Hi you');
  // Out of range sizes are clamped and shown as used.
  await page.locator('[data-testid=text-size]').fill('1000');
  await page.locator('[data-testid=text-size]').press('Enter');
  await expect(page.locator('[data-testid=text-size]')).toHaveValue('400');
  await page.locator('[data-testid=text-size]').fill('72');
  await page.locator('[data-testid=text-size]').press('Enter');
  // Selecting another layer applies the text on the layer it was typed on.
  await page.click('[data-testid=layer-0]');
  const onLayer1 = await page.evaluate(() => {
    const e = window.paint.engine;
    e.set_active_layer(1);
    let n = 0;
    for (let y = 30; y < 110; y++) for (let x = 30; x < 250; x++) if (e.layer_pixel(x, y).length === 7) n++;
    return n;
  });
  expect(onLayer1).toBeGreaterThan(50);
});

test('Undo while typing discards only the text box', async ({ page }) => {
  await page.click('[data-testid=tool-pencil]');
  await drag(page, [[10, 10], [40, 10]]);
  await page.click('[data-testid=tool-text]');
  await clickAt(page, 100, 100);
  await page.keyboard.type('abc');
  await page.click('[data-testid=qa-undo]');
  await expect(page.locator('[data-testid=text-box]')).toBeHidden();
  expect(await pixel(page, 20, 10)).toBe('#000000');
});

test('Ctrl+S works while typing in the text box', async ({ page }) => {
  await page.click('[data-testid=tool-text]');
  await clickAt(page, 50, 50);
  await page.keyboard.type('Save me');
  const dl = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  await dl;
  await expect(page.locator('[data-testid=text-box]')).toBeHidden();
  expect((await state(page)).modified).toBe(false);
});

test('layer rename: Escape and unchanged names do nothing', async ({ page }) => {
  await page.click('[data-testid=btn-layers]');
  await page.dblclick('[data-testid=layer-0] .layer-name');
  await page.locator('[data-testid=layer-rename]').fill('Ciel');
  await page.locator('[data-testid=layer-rename]').press('Escape');
  await expect(page.locator('[data-testid=layer-0] .layer-name')).toHaveText('Calque 1');
  await page.dblclick('[data-testid=layer-0] .layer-name');
  await page.locator('[data-testid=layer-rename]').press('Enter');
  const st = await state(page);
  expect(st.layers[0].name).toBe('Layer 1');
  expect(st.can_undo).toBe(false);
  expect(st.modified).toBe(false);
});

test('unapplied text counts as an unsaved change when leaving the page', async ({ page }) => {
  await page.click('[data-testid=tool-text]');
  await clickAt(page, 50, 50);
  await page.keyboard.type('draft');
  const dirty = await page.evaluate(() => {
    const ev = new Event('beforeunload', { cancelable: true });
    window.dispatchEvent(ev);
    return ev.defaultPrevented;
  });
  expect(dirty).toBe(true);
});

test('AZERTY: Ctrl+à fits the window and Ctrl+& is 100%', async ({ page }) => {
  await page.click('[data-testid=zoom-in]');
  const key = (key, code) => page.evaluate(([key, code]) => document.body.dispatchEvent(new KeyboardEvent('keydown', { key, code, ctrlKey: true, bubbles: true, cancelable: true })), [key, code]);
  await key('&', 'Digit1');
  expect(await page.evaluate(() => window.paint.view.zoom)).toBe(1);
  await page.click('[data-testid=zoom-in]');
  await key('à', 'Digit0');
  expect(await page.evaluate(() => window.paint.view.zoom)).toBeLessThan(2);
});

test('shortcuts close an open menu', async ({ page }) => {
  await page.click('[data-testid=menu-edit]');
  await page.keyboard.press('Control+a');
  await expect(page.locator('.popup')).toHaveCount(0);
  expect((await state(page)).selection.w).toBe(1152);
});

test('Ctrl+V with a dialog open does not paste behind it', async ({ page }) => {
  const png = await makePng(page, 10, 10, '#22b14c');
  await page.evaluate(async (bytes) => {
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': new Blob([new Uint8Array(bytes)], { type: 'image/png' }) })]);
  }, [...png]);
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+e');
  await page.click('[data-testid=dlg-props] h2');
  await page.keyboard.press('Control+v');
  await page.waitForTimeout(200);
  expect((await state(page)).selection).toBeNull();
});

test('canvas resize: Escape and cancelled pointers do not apply it', async ({ page }) => {
  const h = await page.locator('.canvas-handle[data-h=se]').boundingBox();
  await page.mouse.move(h.x + 4, h.y + 4);
  await page.mouse.down();
  await page.mouse.move(h.x + 80, h.y + 60, { steps: 4 });
  await page.keyboard.press('Escape');
  await page.mouse.up();
  let st = await state(page);
  expect([st.width, st.height]).toEqual([1152, 648]);
  // pointercancel during the drag
  await page.mouse.move(h.x + 4, h.y + 4);
  await page.mouse.down();
  await page.mouse.move(h.x + 80, h.y + 60, { steps: 4 });
  await page.locator('.canvas-handle[data-h=se]').dispatchEvent('pointercancel');
  await page.mouse.up();
  st = await state(page);
  expect([st.width, st.height]).toEqual([1152, 648]);
});

test('Delete removes a shape being edited; arrows move it', async ({ page }) => {
  await page.click('[data-testid=btn-shapes] [data-icon=chevron]');
  await page.click('[data-testid=shape-rectangle]');
  await drag(page, [[20, 20], [60, 60]]);
  // The 3 px outline covers x = 19..21; one step right makes x = 19 white.
  expect(await pixel(page, 19, 40)).toBe('#000000');
  await page.keyboard.press('ArrowRight');
  expect(await pixel(page, 19, 40)).toBe('#ffffff');
  expect(await pixel(page, 22, 40)).toBe('#000000');
  await page.keyboard.press('Delete');
  expect(await pixel(page, 22, 40)).toBe('#ffffff');
  expect((await state(page)).shape_edit).toBeNull();
});

test('Select All then rotate turns a non-square picture', async ({ page }) => {
  await rect(page, 0, 0, 10, 10, '#ed1c24');
  await page.keyboard.press('Control+a');
  await page.click('[data-testid=btn-rotate]');
  await page.click('[data-testid=mi-rotate-right]');
  const st = await state(page);
  expect([st.width, st.height]).toEqual([648, 1152]);
  expect(await pixel(page, 645, 3)).toBe('#ed1c24');
});

test('dragging a selection past the view edge scrolls the view', async ({ page }) => {
  await page.click('[data-testid=zoom-in]');
  await page.click('[data-testid=zoom-in]');
  await page.evaluate(() => {
    const v = document.getElementById('viewport');
    v.scrollLeft = 0;
    v.scrollTop = 0;
  });
  await page.click('[data-testid=btn-selection] .big-ico');
  const vp = await page.locator('#viewport').boundingBox();
  const start = await toScreen(page, 10, 10);
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.mouse.move(vp.x + vp.width + 30, start.y + 40, { steps: 5 });
  await expect.poll(() => page.evaluate(() => document.getElementById('viewport').scrollLeft)).toBeGreaterThan(50);
  await page.mouse.up();
  const sel = (await state(page)).selection;
  expect(sel.w).toBeGreaterThan(200);
});

test('touch drawing keeps drawing (no browser panning)', async ({ browser }) => {
  const context = await browser.newContext({ hasTouch: true, viewport: { width: 1280, height: 900 }, locale: 'fr-FR' });
  const page = await context.newPage();
  await openApp(page);
  await page.click('[data-testid=tool-pencil]');
  const a = await toScreen(page, 100, 100);
  const cdp = await context.newCDPSession(page);
  const touch = (type, x, y) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: type === 'touchEnd' ? [] : [{ x, y }] });
  await touch('touchStart', a.x, a.y);
  for (let i = 1; i <= 20; i++) await touch('touchMove', a.x + i * 15, a.y + i * 5);
  await touch('touchEnd');
  await expectPixel(page, 370, 190, '#000000');
  await context.close();
});

test('saving with a pending shape, then editing it, keeps the unsaved state', async ({ page }) => {
  await page.click('[data-testid=btn-shapes] [data-icon=chevron]');
  await page.click('[data-testid=shape-oval]');
  await drag(page, [[20, 20], [80, 80]]);
  const dl = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  await dl;
  expect((await state(page)).modified).toBe(false);
  await drag(page, [[50, 50], [150, 150]]);
  expect((await state(page)).modified).toBe(true);
  await menu(page, 'file', 'mi-new');
  await expect(page.locator('[data-testid=dlg-confirm]')).toBeVisible();
});
