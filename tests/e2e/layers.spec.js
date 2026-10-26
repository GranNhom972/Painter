// Layers panel: add, draw on a layer, hide, reorder (buttons and drag),
// duplicate, merge, delete, rename.
import { test, expect } from '@playwright/test';
import { openApp, state, pixel, drag, fillCanvas } from './helpers.js';

test.beforeEach(async ({ page }) => {
  await openApp(page);
  await page.click('[data-testid=btn-layers]');
  await expect(page.locator('[data-testid=layers-panel]')).toBeVisible();
});

test('add a layer and paint on it without touching the background', async ({ page }) => {
  await expect(page.locator('.layer-item')).toHaveCount(1);
  await expect(page.locator('.layer-item .layer-name')).toHaveText('Calque 1');
  await page.click('[data-testid=layer-add]');
  await expect(page.locator('.layer-item')).toHaveCount(2);
  await expect(page.locator('[data-testid=layer-1]')).toHaveClass(/active/);
  await page.click('[data-testid=tool-pencil]');
  await page.click('[data-testid=btn-size]');
  await page.click('[data-testid=size-8]');
  await drag(page, [[50, 50], [150, 50]]);
  expect(await pixel(page, 100, 50)).toBe('#000000');
  const bg = await page.evaluate(() => {
    window.paint.engine.set_active_layer(0);
    return window.paint.engine.layer_pixel(100, 50);
  });
  expect(bg).toBe('#ffffff');
  // Thumbnails are rendered.
  await expect(page.locator('[data-testid=layer-1] img')).toHaveAttribute('src', /^blob:/);
});

test('hide and show a layer', async ({ page }) => {
  await page.click('[data-testid=layer-add]');
  await fillCanvas(page, '#ed1c24');
  expect(await pixel(page, 10, 10)).toBe('#ed1c24');
  await page.click('[data-testid=layer-eye-1]');
  expect(await pixel(page, 10, 10)).toBe('#ffffff');
  await expect(page.locator('[data-testid=layer-1]')).toHaveClass(/hidden-layer/);
  await page.click('[data-testid=layer-eye-1]');
  expect(await pixel(page, 10, 10)).toBe('#ed1c24');
});

test('reorder with buttons and by dragging', async ({ page }) => {
  await page.click('[data-testid=layer-add]');
  await fillCanvas(page, '#ed1c24');
  await page.click('[data-testid=layer-down]');
  expect((await state(page)).active_layer).toBe(0);
  expect(await pixel(page, 10, 10)).toBe('#ffffff'); // red layer is now under the white one
  await page.click('[data-testid=layer-up]');
  expect(await pixel(page, 10, 10)).toBe('#ed1c24');
  // The list is rebuilt on the next frame: measure the new rows, not the old ones.
  await expect(page.locator('[data-testid=layer-1]')).toHaveClass(/active/);
  // Drag the background (bottom of the list) above the red layer.
  const from = await page.locator('[data-testid=layer-0]').boundingBox();
  const to = await page.locator('[data-testid=layer-1]').boundingBox();
  await page.mouse.move(from.x + 100, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(to.x + 100, to.y + to.height / 2, { steps: 8 });
  await page.mouse.up();
  expect(await pixel(page, 10, 10)).toBe('#ffffff');
});

test('duplicate, merge down and delete', async ({ page }) => {
  await page.click('[data-testid=layer-add]');
  await fillCanvas(page, '#22b14c');
  await page.click('[data-testid=layer-duplicate]');
  await expect(page.locator('.layer-item')).toHaveCount(3);
  await expect(page.locator('[data-testid=layer-2] .layer-name')).toHaveText('Calque 2 (copie)');
  await page.click('[data-testid=layer-merge]');
  await expect(page.locator('.layer-item')).toHaveCount(2);
  await page.click('[data-testid=layer-delete]');
  await expect(page.locator('.layer-item')).toHaveCount(1);
  expect(await pixel(page, 10, 10)).toBe('#ffffff');
  await expect(page.locator('[data-testid=layer-delete]')).toBeDisabled();
  await expect(page.locator('[data-testid=layer-merge]')).toBeDisabled();
  await page.keyboard.press('Control+z');
  await expect(page.locator('.layer-item')).toHaveCount(2);
});

test('rename by double click and select by click', async ({ page }) => {
  await page.click('[data-testid=layer-add]');
  await page.dblclick('[data-testid=layer-1] .layer-name');
  const input = page.locator('[data-testid=layer-rename]');
  await input.fill('Ciel');
  await input.press('Enter');
  await expect(page.locator('[data-testid=layer-1] .layer-name')).toHaveText('Ciel');
  await page.click('[data-testid=layer-0]');
  await expect(page.locator('[data-testid=layer-0]')).toHaveClass(/active/);
  expect((await state(page)).active_layer).toBe(0);
  // Ctrl+Shift+N adds a layer; the panel toggles off.
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+Shift+N');
  await expect(page.locator('.layer-item')).toHaveCount(3);
  await page.click('[data-testid=btn-layers]');
  await expect(page.locator('[data-testid=layers-panel]')).toBeHidden();
});
