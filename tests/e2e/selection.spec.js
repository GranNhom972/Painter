// Selection through the UI: rectangle, free-form, move, copy-move, handles,
// select all, invert, delete, transparent mode, crop, nudge, context menu.
import { test, expect } from '@playwright/test';
import { openApp, state, pixel, drag, clickAt, openRibbonMenu, rect, toScreen } from './helpers.js';

test.beforeEach(async ({ page }) => {
  await openApp(page);
  await rect(page, 20, 20, 40, 40, '#ed1c24');
});

async function selectRect(page, a, b) {
  await page.click('[data-testid=btn-selection] .big-ico');
  await drag(page, [a, b]);
}

test('rectangle selection shows its size and handles', async ({ page }) => {
  await selectRect(page, [10, 10], [70, 50]);
  const st = await state(page);
  expect(st.tool).toBe('select-rect');
  expect(st.selection).toMatchObject({ x: 10, y: 10, w: 60, h: 40, floating: false });
  await expect(page.locator('[data-testid=stat-sel]')).toHaveText('60 × 40px');
  await expect(page.locator('[data-testid=btn-crop]')).toBeEnabled();
  // Hovering a corner shows a resize cursor, inside a move cursor.
  const corner = await toScreen(page, 70, 50);
  await page.mouse.move(corner.x - 0.5, corner.y - 0.5);
  await expect(page.locator('#viewport')).toHaveAttribute('data-cursor', 'nwse-resize');
  const inside = await toScreen(page, 40, 30);
  await page.mouse.move(inside.x, inside.y);
  await expect(page.locator('#viewport')).toHaveAttribute('data-cursor', 'move');
  // Clicking outside deselects.
  await clickAt(page, 300, 300);
  expect((await state(page)).selection).toBeNull();
  await expect(page.locator('[data-testid=stat-sel]')).toHaveText('');
});

test('drag moves the selected pixels and leaves colour 2', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 60]);
  await drag(page, [[40, 40], [140, 90]]);
  let st = await state(page);
  expect(st.selection).toMatchObject({ x: 120, y: 70, floating: true });
  expect(await pixel(page, 30, 30)).toBe('#ffffff');
  expect(await pixel(page, 125, 75)).toBe('#ed1c24');
  await page.keyboard.press('Escape');
  st = await state(page);
  expect(st.selection).toBeNull();
  expect(await pixel(page, 125, 75)).toBe('#ed1c24');
});

test('ctrl+drag moves a copy', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 60]);
  await drag(page, [[40, 40], [140, 40]], { modifiers: ['Control'] });
  await page.keyboard.press('Enter');
  expect(await pixel(page, 30, 30)).toBe('#ed1c24');
  expect(await pixel(page, 130, 30)).toBe('#ed1c24');
});

test('resize handle scales the selection', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 60]);
  await drag(page, [[60, 60], [100, 140]]);
  const st = await state(page);
  expect(st.selection).toMatchObject({ x: 20, y: 20, w: 80, h: 120 });
  await page.keyboard.press('Escape');
  expect(await pixel(page, 95, 135)).toBe('#ed1c24');
});

test('free-form selection moves only the lasso area', async ({ page }) => {
  await openRibbonMenu(page, 'btn-selection');
  await page.click('[data-testid=mi-select-free]');
  expect((await state(page)).tool).toBe('select-free');
  await drag(page, [[20, 20], [60, 20], [20, 60], [20, 20]]);
  const st = await state(page);
  expect(st.selection.freeform).toBe(true);
  await drag(page, [[25, 25], [225, 25]]);
  await page.keyboard.press('Escape');
  expect(await pixel(page, 24, 24)).toBe('#ffffff');
  expect(await pixel(page, 224, 24)).toBe('#ed1c24');
  expect(await pixel(page, 55, 55)).toBe('#ed1c24');
  expect(await pixel(page, 255, 55)).toBe('#ffffff');
});

test('select all, invert and delete', async ({ page }) => {
  await page.keyboard.press('Control+a');
  let st = await state(page);
  expect(st.selection).toMatchObject({ x: 0, y: 0, w: 1152, h: 648 });
  await page.keyboard.press('Escape');
  await selectRect(page, [20, 20], [60, 60]);
  await openRibbonMenu(page, 'btn-selection');
  await page.click('[data-testid=mi-sel-invert]');
  await page.click('[data-testid=color2]');
  await page.click('[data-testid=palette-6]'); // colour 2 = green
  await page.keyboard.press('Delete');
  expect(await pixel(page, 5, 5)).toBe('#22b14c');
  expect(await pixel(page, 40, 40)).toBe('#ed1c24');
  st = await state(page);
  expect(st.selection).toBeNull();
});

test('delete from the selection menu', async ({ page }) => {
  await selectRect(page, [20, 20], [40, 40]);
  await openRibbonMenu(page, 'btn-selection');
  await page.click('[data-testid=mi-sel-delete]');
  expect(await pixel(page, 30, 30)).toBe('#ffffff');
  expect(await pixel(page, 50, 50)).toBe('#ed1c24');
});

test('transparent selection ignores colour 2', async ({ page }) => {
  await openRibbonMenu(page, 'btn-selection');
  await page.click('[data-testid=mi-sel-transparent]');
  expect((await state(page)).transparent_selection).toBe(true);
  await rect(page, 200, 0, 100, 100, '#3f48cc');
  // Select red square plus white margin, drop it on the blue area.
  await selectRect(page, [10, 10], [70, 70]);
  await drag(page, [[40, 40], [240, 40]]);
  expect(await pixel(page, 215, 15)).toBe('#3f48cc');
  expect(await pixel(page, 235, 35)).toBe('#ed1c24');
  await openRibbonMenu(page, 'btn-selection');
  await expect(page.locator('[data-testid=mi-sel-transparent]')).toHaveAttribute('aria-checked', 'true');
});

test('crop to selection', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 50]);
  await page.click('[data-testid=btn-crop]');
  const st = await state(page);
  expect([st.width, st.height]).toEqual([40, 30]);
  await expect(page.locator('[data-testid=stat-canvas]')).toHaveText('40 × 30px');
  expect(await pixel(page, 0, 0)).toBe('#ed1c24');
  await expect(page.locator('[data-testid=btn-crop]')).toBeDisabled();
  await page.keyboard.press('Control+z');
  expect((await state(page)).width).toBe(1152);
  // Ctrl+Shift+X also crops.
  await selectRect(page, [5, 5], [15, 15]);
  await page.keyboard.press('Control+Shift+X');
  expect((await state(page)).width).toBe(10);
});

test('arrow keys nudge the selection', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 60]);
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Shift+ArrowDown');
  const st = await state(page);
  expect(st.selection).toMatchObject({ x: 22, y: 30, floating: true });
  await page.keyboard.press('Enter');
  expect(await pixel(page, 61, 69)).toBe('#ed1c24');
  expect(await pixel(page, 21, 25)).toBe('#ffffff');
});

test('right click opens the selection context menu', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 60]);
  await clickAt(page, 40, 40, { button: 'right' });
  const ctx = page.locator('[data-testid=popup-context]');
  await expect(ctx).toBeVisible();
  await page.hover('[data-testid=ctx-rotate]');
  await page.click('[data-testid=mi-rotate-right]');
  expect((await state(page)).selection).toMatchObject({ floating: true, w: 40, h: 40 });
  await clickAt(page, 40, 40, { button: 'right' });
  await page.click('[data-testid=ctx-delete]');
  expect(await pixel(page, 40, 40)).toBe('#ffffff');
  await clickAt(page, 40, 40, { button: 'right' });
  await expect(page.locator('[data-testid=ctx-cut]')).toBeDisabled();
  await page.click('[data-testid=ctx-select-all]');
  expect((await state(page)).selection.w).toBe(1152);
});

test('rotate and flip a selection from the ribbon', async ({ page }) => {
  await rect(page, 20, 20, 10, 10, '#3f48cc'); // blue corner in the red square
  await selectRect(page, [20, 20], [60, 60]);
  await page.click('[data-testid=btn-flip]');
  await page.click('[data-testid=mi-flip-h]');
  expect(await pixel(page, 55, 25)).toBe('#3f48cc');
  await page.click('[data-testid=btn-flip]');
  await page.click('[data-testid=mi-flip-v]');
  expect(await pixel(page, 55, 55)).toBe('#3f48cc');
  await page.click('[data-testid=btn-rotate]');
  await page.click('[data-testid=mi-rotate-180]');
  expect(await pixel(page, 25, 25)).toBe('#3f48cc');
  await page.click('[data-testid=btn-rotate]');
  await page.click('[data-testid=mi-rotate-left]');
  expect(await pixel(page, 25, 55)).toBe('#3f48cc');
  // The canvas did not change.
  expect((await state(page)).width).toBe(1152);
});

test('selection survives switching between rectangle and free-form tools', async ({ page }) => {
  await selectRect(page, [20, 20], [60, 60]);
  await openRibbonMenu(page, 'btn-selection');
  await page.click('[data-testid=mi-select-free]');
  expect((await state(page)).selection).not.toBeNull();
  await page.click('[data-testid=tool-pencil]');
  expect((await state(page)).selection).toBeNull();
});
