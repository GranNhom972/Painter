// Clipboard: copy/cut/paste with the system clipboard (Chromium's async
// clipboard API), images coming from other programs, and paste from file.
import { test, expect } from '@playwright/test';
import { openApp, state, pixel, drag, rect, makePng, menu } from './helpers.js';

test.beforeEach(async ({ page }) => {
  await openApp(page);
  await rect(page, 20, 20, 40, 30, '#ed1c24');
});

async function selectRect(page, a, b) {
  await page.click('[data-testid=btn-selection] .big-ico');
  await drag(page, [a, b]);
}

async function clipboardImage(page) {
  return page.evaluate(async () => {
    const items = await navigator.clipboard.read();
    for (const it of items) {
      if (it.types.includes('image/png')) {
        const buf = new Uint8Array(await (await it.getType('image/png')).arrayBuffer());
        const bmp = await createImageBitmap(new Blob([buf], { type: 'image/png' }));
        const c = new OffscreenCanvas(bmp.width, bmp.height);
        const ctx = c.getContext('2d');
        ctx.drawImage(bmp, 0, 0);
        const d = ctx.getImageData(1, 1, 1, 1).data;
        return { w: bmp.width, h: bmp.height, rgb: [d[0], d[1], d[2]] };
      }
    }
    return null;
  });
}

test('Ctrl+C puts the selection on the system clipboard as PNG', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 50]);
  await page.keyboard.press('Control+c');
  await expect.poll(() => clipboardImage(page)).toEqual({ w: 40, h: 30, rgb: [237, 28, 36] });
  // Copy does not modify the picture.
  expect(await pixel(page, 30, 30)).toBe('#ed1c24');
});

test('Ctrl+V pastes at the top-left of the view as a movable selection', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 50]);
  await page.keyboard.press('Control+c');
  await expect.poll(() => clipboardImage(page)).not.toBeNull();
  await page.keyboard.press('Control+v');
  await expect.poll(async () => (await state(page)).selection?.floating).toBe(true);
  let st = await state(page);
  expect(st.selection).toMatchObject({ x: 0, y: 0, w: 40, h: 30 });
  expect(st.tool).toBe('select-rect');
  await drag(page, [[10, 10], [310, 210]]);
  await page.keyboard.press('Enter');
  expect(await pixel(page, 305, 205)).toBe('#ed1c24');
  expect(await pixel(page, 30, 30)).toBe('#ed1c24');
  st = await state(page);
  expect(st.selection).toBeNull();
});

test('Ctrl+X cuts and the edit menu pastes', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 50]);
  await page.keyboard.press('Control+x');
  expect(await pixel(page, 30, 30)).toBe('#ffffff');
  await expect.poll(() => clipboardImage(page)).toEqual({ w: 40, h: 30, rgb: [237, 28, 36] });
  await menu(page, 'edit', 'mi-paste');
  await expect.poll(async () => (await state(page)).selection?.w).toBe(40);
  expect(await pixel(page, 5, 5)).toBe('#ed1c24');
});

test('pastes an image copied by another program', async ({ page }) => {
  const png = await makePng(page, 25, 15, '#22b14c');
  await page.evaluate(async (bytes) => {
    const blob = new Blob([new Uint8Array(bytes)], { type: 'image/png' });
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
  }, [...png]);
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+v');
  await expect.poll(async () => (await state(page)).selection?.w).toBe(25);
  expect(await pixel(page, 3, 3)).toBe('#22b14c');
});

test('pasting a bigger picture enlarges the canvas', async ({ page }) => {
  await page.evaluate(() => {
    window.paint.engine.new_document(100, 80);
    window.paint.changed();
  });
  const png = await makePng(page, 300, 200, '#3f48cc');
  await page.evaluate(async (bytes) => {
    const blob = new Blob([new Uint8Array(bytes)], { type: 'image/png' });
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
  }, [...png]);
  await page.click('[data-testid=menu-edit]');
  await page.click('[data-testid=mi-paste]');
  await expect.poll(async () => (await state(page)).width).toBe(300);
  await expect(page.locator('[data-testid=stat-canvas]')).toHaveText('300 × 200px');
  await page.keyboard.press('Control+z');
  expect((await state(page)).width).toBe(100);
});

test('empty clipboard shows a message', async ({ page }) => {
  await page.evaluate(async () => navigator.clipboard.writeText('just text'));
  await menu(page, 'edit', 'mi-paste');
  await expect(page.locator('#toast')).toHaveText("Le presse-papiers ne contient pas d'image.");
});

test('paste from file', async ({ page }) => {
  const png = await makePng(page, 12, 9, '#a349a4');
  const chooser = page.waitForEvent('filechooser');
  await menu(page, 'edit', 'mi-paste-from');
  await (await chooser).setFiles({ name: 'violet.png', mimeType: 'image/png', buffer: png });
  await expect.poll(async () => (await state(page)).selection?.w).toBe(12);
  expect(await pixel(page, 2, 2)).toBe('#a349a4');
});

test('context menu copy and paste', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 50]);
  const p = await page.locator('#canvas').boundingBox();
  await page.mouse.click(p.x + 40, p.y + 30, { button: 'right' });
  await page.click('[data-testid=ctx-copy]');
  await expect.poll(() => clipboardImage(page)).not.toBeNull();
  await page.mouse.click(p.x + 40, p.y + 30, { button: 'right' });
  await page.click('[data-testid=ctx-paste]');
  await expect.poll(async () => (await state(page)).selection?.x).toBe(0);
});
