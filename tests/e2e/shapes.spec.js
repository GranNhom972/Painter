// Shapes through the UI: gallery, outline/fill styles, live editing, curve, polygon.
import { test, expect } from '@playwright/test';
import { openApp, state, pixel, drag, clickAt, openRibbonMenu } from './helpers.js';

test.beforeEach(async ({ page }) => openApp(page));

async function pickShape(page, name) {
  await openRibbonMenu(page, 'btn-shapes');
  await page.click(`[data-testid=shape-${name}]`);
}

async function countNotWhite(page, x0, y0, x1, y1) {
  return page.evaluate(([x0, y0, x1, y1]) => {
    let n = 0;
    for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) if (window.paint.engine.pixel(x, y) !== '#ffffff') n++;
    return n;
  }, [x0, y0, x1, y1]);
}

test('gallery lists 23 shapes and draws each of them', async ({ page }) => {
  await openRibbonMenu(page, 'btn-shapes');
  const names = await page.locator('[data-testid=popup-shapes] .gallery button').evaluateAll((els) => els.map((e) => e.dataset.testid.replace('shape-', '')));
  expect(names).toHaveLength(23);
  await page.keyboard.press('Escape');
  let i = 0;
  for (const name of names) {
    const x = 20 + (i % 8) * 120;
    const y = 20 + Math.floor(i / 8) * 120;
    await pickShape(page, name);
    expect((await state(page)).shape).toBe(name);
    if (name === 'polygon') {
      await drag(page, [[x, y], [x + 90, y]]);
      await clickAt(page, x + 45, y + 90);
      await page.keyboard.press('Enter');
    } else {
      await drag(page, [[x, y], [x + 90, y + 90]]);
    }
    await page.keyboard.press('Enter');
    expect(await countNotWhite(page, x - 5, y - 5, x + 100, y + 100), name).toBeGreaterThan(40);
    i++;
  }
});

test('outline and fill styles use colour 1 and colour 2', async ({ page }) => {
  await pickShape(page, 'rectangle');
  await openRibbonMenu(page, 'btn-shapes');
  await page.click('[data-testid=mi-fill-solid]');
  await page.click('[data-testid=color2]');
  await page.click('[data-testid=palette-6]');
  await drag(page, [[20, 20], [80, 60]]);
  expect(await pixel(page, 20, 20)).toBe('#000000');
  expect(await pixel(page, 50, 40)).toBe('#22b14c');
  await openRibbonMenu(page, 'btn-shapes');
  await page.click('[data-testid=mi-outline-none]');
  // The pending shape is redrawn with the new style.
  expect(await pixel(page, 20, 20)).toBe('#22b14c');
  const st = await state(page);
  expect([st.outline, st.fill]).toEqual(['none', 'solid']);
});

test('pending shape: handles, move, resize, then commit', async ({ page }) => {
  await pickShape(page, 'oval');
  await drag(page, [[100, 100], [200, 160]]);
  let st = await state(page);
  expect(st.shape_edit.stage).toBe('editing');
  expect(st.shape_edit.handles).toHaveLength(8);
  // Move by dragging the inside.
  await drag(page, [[150, 130], [350, 230]]);
  expect(await countNotWhite(page, 95, 95, 205, 165)).toBe(0);
  expect(await countNotWhite(page, 295, 195, 405, 265)).toBeGreaterThan(50);
  // Resize with the south-east handle.
  st = await state(page);
  const se = st.shape_edit.handles.find((h) => h.cursor === 'nwse-resize' && h.x > st.shape_edit.x);
  await drag(page, [[se.x, se.y], [se.x + 100, se.y + 60]]);
  st = await state(page);
  expect(st.shape_edit.w).toBeGreaterThan(190);
  // Changing the colour recolours the pending shape.
  await page.click('[data-testid=palette-3]');
  const reds = await page.evaluate(() => {
    let n = 0;
    for (let y = 190; y < 330; y++) for (let x = 290; x < 510; x++) if (window.paint.engine.pixel(x, y) === '#ed1c24') n++;
    return n;
  });
  expect(reds).toBeGreaterThan(50);
  // Switching tool commits.
  await page.click('[data-testid=tool-pencil]');
  expect((await state(page)).shape_edit).toBeNull();
  expect(reds).toBeGreaterThan(0);
});

test('curve: draw, bend twice, edit', async ({ page }) => {
  await pickShape(page, 'curve');
  await drag(page, [[50, 200], [350, 200]]);
  expect((await state(page)).shape_edit.stage).toBe('bend1');
  await drag(page, [[150, 150], [150, 60]]);
  expect((await state(page)).shape_edit.stage).toBe('bend2');
  await drag(page, [[250, 250], [250, 340]]);
  const st = await state(page);
  expect(st.shape_edit.stage).toBe('editing');
  expect(st.shape_edit.handles).toHaveLength(4);
  expect(await countNotWhite(page, 120, 100, 180, 160)).toBeGreaterThan(0);
  expect(await countNotWhite(page, 220, 240, 280, 300)).toBeGreaterThan(0);
});

test('polygon: clicks then close on the first point; double click also closes', async ({ page }) => {
  await pickShape(page, 'polygon');
  await openRibbonMenu(page, 'btn-shapes');
  await page.click('[data-testid=mi-fill-solid]');
  await drag(page, [[50, 50], [250, 50]]);
  expect((await state(page)).shape_edit.stage).toBe('polygon');
  await clickAt(page, 250, 250);
  await clickAt(page, 50, 250);
  await clickAt(page, 51, 51);
  expect((await state(page)).shape_edit.stage).toBe('editing');
  // Closed outline: the closing edge (left side) is drawn.
  expect(await pixel(page, 50, 150)).toBe('#000000');
  await page.keyboard.press('Enter');
  // Second polygon closed by a double click, filled with green.
  await page.click('[data-testid=color2]');
  await page.click('[data-testid=palette-6]');
  await drag(page, [[400, 50], [600, 50]]);
  await clickAt(page, 500, 250);
  await clickAt(page, 500, 250, { clickCount: 2 });
  const st = await state(page);
  expect(st.shape_edit.stage).toBe('editing');
  expect(st.shape_edit.points).toHaveLength(3);
  expect(await pixel(page, 500, 100)).toBe('#22b14c');
});

test('shift draws a square', async ({ page }) => {
  await pickShape(page, 'rectangle');
  await drag(page, [[20, 20], [120, 60]], { modifiers: ['Shift'] });
  const st = await state(page);
  expect([st.shape_edit.w, st.shape_edit.h]).toEqual([100, 100]);
});

test('a shape is undone in one step', async ({ page }) => {
  await pickShape(page, 'heart');
  await drag(page, [[20, 20], [120, 120]]);
  await drag(page, [[70, 70], [170, 170]]);
  await page.keyboard.press('Control+z');
  expect(await countNotWhite(page, 0, 0, 300, 300)).toBe(0);
  await page.keyboard.press('Control+y');
  expect(await countNotWhite(page, 100, 100, 300, 300)).toBeGreaterThan(50);
});
