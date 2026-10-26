// Whole-image commands: rotate, flip, resize & skew, properties, canvas handles,
// remove background, crop.
import { test, expect } from '@playwright/test';
import { openApp, state, pixel, rect, menu } from './helpers.js';

const size = async (page) => {
  const st = await state(page);
  return [st.width, st.height];
};

test.beforeEach(async ({ page }) => {
  await openApp(page);
  await page.evaluate(() => {
    window.paint.engine.new_document(200, 100);
    window.paint.changed();
  });
  await rect(page, 0, 0, 20, 10, '#ed1c24');
});

test('rotate right, left and 180 degrees', async ({ page }) => {
  await page.click('[data-testid=btn-rotate]');
  await page.click('[data-testid=mi-rotate-right]');
  let st = await state(page);
  expect([st.width, st.height]).toEqual([100, 200]);
  expect(await pixel(page, 95, 5)).toBe('#ed1c24');
  await page.click('[data-testid=btn-rotate]');
  await page.click('[data-testid=mi-rotate-left]');
  st = await state(page);
  expect([st.width, st.height]).toEqual([200, 100]);
  await page.click('[data-testid=btn-rotate]');
  await page.click('[data-testid=mi-rotate-180]');
  expect(await pixel(page, 195, 95)).toBe('#ed1c24');
  await expect(page.locator('[data-testid=stat-canvas]')).toHaveText('200 x 100px');
});

test('flip vertical and horizontal', async ({ page }) => {
  await page.click('[data-testid=btn-flip]');
  await page.click('[data-testid=mi-flip-h]');
  expect(await pixel(page, 195, 5)).toBe('#ed1c24');
  await page.click('[data-testid=btn-flip]');
  await page.click('[data-testid=mi-flip-v]');
  expect(await pixel(page, 195, 95)).toBe('#ed1c24');
});

test('resize and skew dialog (percentage, pixels, skew, cancel)', async ({ page }) => {
  await page.click('[data-testid=btn-resize]');
  const dlg = page.locator('[data-testid=dlg-resize]');
  await expect(dlg).toBeVisible();
  await page.locator('[data-testid=resize-w]').fill('50');
  await expect(page.locator('[data-testid=resize-h]')).toHaveValue('50');
  await dlg.locator('[data-testid=dlg-ok]').click();
  await expect.poll(() => size(page)).toEqual([100, 50]);
  // Pixels without keeping the ratio (Ctrl+W opens the dialog).
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+w');
  await page.click('[data-testid=resize-pixels]');
  await expect(page.locator('[data-testid=resize-w]')).toHaveValue('100');
  await page.click('[data-testid=resize-keep]');
  await page.locator('[data-testid=resize-w]').fill('300');
  await page.locator('[data-testid=resize-h]').fill('40');
  await dlg.locator('[data-testid=dlg-ok]').click();
  await expect.poll(() => size(page)).toEqual([300, 40]);
  // Skew only.
  await page.click('[data-testid=btn-resize]');
  await page.locator('[data-testid=skew-x]').fill('45');
  await dlg.locator('[data-testid=dlg-ok]').click();
  await expect.poll(() => size(page)).toEqual([340, 40]);
  // Cancel changes nothing.
  await page.click('[data-testid=btn-resize]');
  await page.locator('[data-testid=resize-w]').fill('10');
  await dlg.locator('[data-testid=dlg-cancel]').click();
  await expect(dlg).toBeHidden();
  expect((await state(page)).width).toBe(340);
});

test('image properties change the canvas size', async ({ page }) => {
  await menu(page, 'file', 'mi-properties');
  const dlg = page.locator('[data-testid=dlg-props]');
  await expect(page.locator('[data-testid=props-w]')).toHaveValue('200');
  await page.locator('[data-testid=props-w]').fill('320');
  await page.locator('[data-testid=props-h]').fill('240');
  await dlg.locator('[data-testid=dlg-ok]').click();
  await expect.poll(() => size(page)).toEqual([320, 240]);
  expect(await pixel(page, 5, 5)).toBe('#ed1c24');
  expect(await pixel(page, 300, 200)).toBe('#ffffff');
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+e');
  await expect(dlg).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(dlg).toBeHidden();
});

test('canvas handles resize the canvas from every side', async ({ page }) => {
  const handle = async (h) => (await page.locator(`.canvas-handle[data-h=${h}]`).boundingBox());
  const dragHandle = async (h, dx, dy) => {
    const b = await handle(h);
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
    await page.mouse.down();
    await page.mouse.move(b.x + b.width / 2 + dx, b.y + b.height / 2 + dy, { steps: 5 });
    await page.mouse.up();
  };
  await dragHandle('se', 50, 30);
  let st = await state(page);
  expect([st.width, st.height]).toEqual([250, 130]);
  await dragHandle('e', -100, 0);
  st = await state(page);
  expect([st.width, st.height]).toEqual([150, 130]);
  await dragHandle('s', 0, 20);
  expect((await state(page)).height).toBe(150);
  // Left/top handles grow the canvas on that side: the picture moves right/down.
  await dragHandle('w', -30, 0);
  st = await state(page);
  expect(st.width).toBe(180);
  expect(await pixel(page, 32, 2)).toBe('#ed1c24');
  expect(await pixel(page, 2, 2)).toBe('#ffffff');
  await dragHandle('n', 0, -10);
  expect(await pixel(page, 32, 12)).toBe('#ed1c24');
  await page.keyboard.press('Control+z');
  expect((await state(page)).height).toBe(150);
});

test('remove background', async ({ page }) => {
  await rect(page, 60, 30, 40, 40, '#3f48cc');
  await page.click('[data-testid=btn-remove-bg]');
  // The white background became fully transparent; the shapes are kept.
  const corner = await page.evaluate(() => window.paint.engine.layer_pixel(150, 90));
  expect(corner).toMatch(/^#[0-9a-f]{6}00$/);
  expect(await pixel(page, 70, 40)).toBe('#3f48cc');
  expect(await pixel(page, 5, 5)).toBe('#ed1c24');
  // Nothing left: a message is shown.
  await page.click('[data-testid=btn-remove-bg]');
  await expect(page.locator('#toast')).toHaveText('Aucun arrière-plan uni détecté.');
});
