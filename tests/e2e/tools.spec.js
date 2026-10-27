// Drawing tools through the real UI: pencil, brushes, size, eraser, fill,
// colour picker, magnifier, text and colours.
import { test, expect } from '@playwright/test';
import { openApp, state, pixel, screenPixel, drag, clickAt, openRibbonMenu, rect, expectPixel } from './helpers.js';

test.beforeEach(async ({ page }) => openApp(page));

test('pencil: left click uses the selected colour slot, right click the other one', async ({ page }) => {
  await page.click('[data-testid=tool-pencil]');
  await expect(page.locator('[data-testid=tool-pencil]')).toHaveClass(/active/);
  await drag(page, [[10, 10], [60, 10]]);
  expect(await pixel(page, 30, 10)).toBe('#000000');
  expect(await screenPixel(page, 30, 10)).toBe('#000000');
  await page.click('[data-testid=color2]');
  await page.click('[data-testid=palette-3]'); // red into colour 2
  await drag(page, [[10, 30], [60, 30]]);
  expect(await pixel(page, 30, 30)).toBe('#ed1c24');
  // Colour 2 selected: the right button now uses colour 1.
  await drag(page, [[10, 50], [60, 50]], { button: 'right' });
  expect(await pixel(page, 30, 50)).toBe('#000000');
  await page.click('[data-testid=color1]');
  await drag(page, [[10, 70], [60, 70]], { button: 'right' });
  expect(await pixel(page, 30, 70)).toBe('#ed1c24');
  // No context menu after drawing with the right button.
  await expect(page.locator('.popup')).toHaveCount(0);
});

test('size menu: presets and slider', async ({ page }) => {
  await page.click('[data-testid=btn-size]');
  await page.click('[data-testid=size-8]');
  expect((await state(page)).size).toBe(8);
  await page.click('[data-testid=btn-size]');
  await page.locator('[data-testid=size-slider]').fill('20');
  expect((await state(page)).size).toBe(20);
  await page.keyboard.press('Escape');
  await drag(page, [[100, 100], [140, 100]]);
  expect(await pixel(page, 120, 108)).toBe('#000000');
  expect(await pixel(page, 120, 112)).toBe('#ffffff');
});

test('every brush of the gallery paints', async ({ page }) => {
  const brushes = ['brush', 'calligraphy-1', 'calligraphy-2', 'airbrush', 'oil', 'crayon', 'marker', 'natural-pencil', 'watercolor'];
  let y = 20;
  for (const b of brushes) {
    await openRibbonMenu(page, 'btn-brushes');
    await expect(page.locator('[data-testid=popup-brushes] canvas')).toHaveCount(9);
    await page.click(`[data-testid=brush-${b}]`);
    const st = await state(page);
    expect([st.tool, st.brush]).toEqual(['brush', b]);
    await drag(page, [[20, y], [200, y]], { steps: 12 });
    const painted = await page.evaluate((y) => {
      let n = 0;
      for (let x = 20; x < 200; x++) for (let dy = -6; dy <= 6; dy++) if (window.paint.engine.pixel(x, y + dy) !== '#ffffff') n++;
      return n;
    }, y);
    expect(painted, b).toBeGreaterThan(30);
    y += 40;
  }
  // The big button (icon part) re-activates the current brush.
  await page.click('[data-testid=tool-pencil]');
  await page.click('[data-testid=btn-brushes] .big-ico');
  expect((await state(page)).tool).toBe('brush');
});

test('eraser paints colour 2 on the background', async ({ page }) => {
  await rect(page, 10, 10, 100, 60, '#3f48cc');
  await page.click('[data-testid=tool-eraser]');
  await drag(page, [[20, 40], [100, 40]]);
  expect(await pixel(page, 60, 40)).toBe('#ffffff');
  expect(await pixel(page, 60, 15)).toBe('#3f48cc');
  // Eraser cursor is drawn on the overlay, the CSS cursor is hidden.
  await expect(page.locator('#viewport')).toHaveAttribute('data-cursor', 'eraser');
});

test('fill bucket and colour picker', async ({ page }) => {
  await rect(page, 50, 50, 40, 40, '#22b14c');
  await page.click('[data-testid=tool-fill]');
  await page.click('[data-testid=palette-8]'); // indigo
  await clickAt(page, 60, 60);
  expect(await pixel(page, 60, 60)).toBe('#3f48cc');
  expect(await pixel(page, 10, 10)).toBe('#ffffff');
  // With colour 2 selected, a left click fills with colour 2.
  await page.click('[data-testid=color2]');
  await page.click('[data-testid=palette-5]'); // yellow
  await clickAt(page, 10, 10);
  expect(await pixel(page, 10, 10)).toBe('#fff200');
  // The picker fills the selected slot (colour 2 here) and returns to the fill tool.
  await page.click('[data-testid=tool-picker]');
  await clickAt(page, 60, 60);
  let st = await state(page);
  expect([st.color1, st.color2]).toEqual(['#3f48cc', '#3f48cc']);
  expect(st.tool).toBe('fill');
  await page.click('[data-testid=tool-picker]');
  await clickAt(page, 10, 10, { button: 'right' }); // right click: the other slot
  st = await state(page);
  expect([st.color1, st.color2]).toEqual(['#fff200', '#3f48cc']);
  // Colour 1 selected: left click picks into colour 1, right click into colour 2.
  await page.click('[data-testid=color1]');
  await page.click('[data-testid=tool-picker]');
  await clickAt(page, 60, 60);
  await page.click('[data-testid=tool-picker]');
  await clickAt(page, 10, 10, { button: 'right' });
  st = await state(page);
  expect([st.color1, st.color2]).toEqual(['#3f48cc', '#fff200']);
});

test('magnifier zooms in with left click and out with right click', async ({ page }) => {
  await page.click('[data-testid=tool-magnifier]');
  await clickAt(page, 100, 100);
  expect(await page.evaluate(() => window.paint.view.zoom)).toBe(2);
  await clickAt(page, 100, 100, { button: 'right' });
  expect(await page.evaluate(() => window.paint.view.zoom)).toBe(1);
  await expect(page.locator('.popup')).toHaveCount(0);
});

test('text tool: type, style and apply', async ({ page }) => {
  await page.click('[data-testid=tool-text]');
  await expect(page.locator('[data-testid=text-bar]')).toBeVisible();
  await page.locator('[data-testid=text-size]').fill('40');
  await page.click('[data-testid=text-bold]');
  await expect(page.locator('[data-testid=text-bold]')).toHaveClass(/on/);
  await clickAt(page, 40, 40);
  const box = page.locator('[data-testid=text-box]');
  await expect(box).toBeVisible();
  await expect(box).toBeFocused();
  await page.keyboard.type('Paint');
  await page.click('[data-testid=text-apply]');
  await expect(box).toBeHidden();
  const dark = await page.evaluate(() => {
    let n = 0;
    for (let y = 40; y < 90; y++) for (let x = 40; x < 200; x++) if (window.paint.engine.pixel(x, y) === '#000000') n++;
    return n;
  });
  expect(dark).toBeGreaterThan(150);
  expect((await state(page)).can_undo).toBe(true);
  // Opaque background + clicking outside commits; Escape cancels.
  await page.click('[data-testid=text-opaque]');
  await page.click('[data-testid=color2]');
  await page.click('[data-testid=palette-5]');
  await page.click('[data-testid=color1]');
  await clickAt(page, 300, 200);
  await page.keyboard.type('Hi');
  await clickAt(page, 600, 400);
  expect(await pixel(page, 301, 201)).toBe('#fff200');
  await clickAt(page, 500, 300);
  await page.keyboard.type('gone');
  await page.keyboard.press('Escape');
  expect(await pixel(page, 501, 301)).toBe('#ffffff');
  // Switching tool commits the text box.
  await clickAt(page, 700, 300);
  await page.keyboard.type('kept');
  await page.click('[data-testid=tool-pencil]');
  await expect(page.locator('[data-testid=text-bar]')).toBeHidden();
  const kept = await page.evaluate(() => {
    let n = 0;
    for (let y = 300; y < 340; y++) for (let x = 700; x < 800; x++) if (window.paint.engine.pixel(x, y) !== '#ffffff') n++;
    return n;
  });
  expect(kept).toBeGreaterThan(20);
});

test('colours: slots, palette, right click and colour editor', async ({ page }) => {
  await page.click('[data-testid=palette-6]');
  expect((await state(page)).color1).toBe('#22b14c');
  await page.click('[data-testid=palette-7]', { button: 'right' });
  expect((await state(page)).color2).toBe('#00a2e8');
  await page.click('[data-testid=color2]');
  await expect(page.locator('[data-testid=color2]')).toHaveClass(/selected/);
  await page.click('[data-testid=palette-0]');
  expect((await state(page)).color2).toBe('#000000');
  await page.click('[data-testid=color1]');
  await page.click('[data-testid=btn-edit-colors]');
  await expect(page.locator('[data-testid=dlg-colors]')).toBeVisible();
  await page.locator('[data-testid=color-hex]').fill('#123456');
  await expect(page.locator('[data-testid=color-r]')).toHaveValue('18');
  await page.click('[data-testid=dlg-colors] [data-testid=dlg-ok]');
  // Dialogs resolve after their close event.
  await expect.poll(async () => (await state(page)).color1).toBe('#123456');
  // The new colour lands in the custom row.
  await expect(page.locator('[data-testid=palette-20]')).toHaveAttribute('data-color', '#123456');
  // Cancel keeps the colour; the SV square and hue slider edit it.
  await page.click('[data-testid=btn-edit-colors]');
  await page.locator('[data-testid=color-hue]').fill('120');
  // Dragging past the top-right corner clamps to full saturation and value.
  const sv = await page.locator('[data-testid=color-sv]').boundingBox();
  await page.mouse.move(sv.x + sv.width / 2, sv.y + sv.height / 2);
  await page.mouse.down();
  await page.mouse.move(sv.x + sv.width + 30, sv.y - 30, { steps: 4 });
  await page.mouse.up();
  await expect(page.locator('[data-testid=color-hex]')).toHaveValue('#00ff00');
  await page.click('[data-testid=dlg-colors] [data-testid=dlg-cancel]');
  await expect(page.locator('[data-testid=dlg-colors]')).toBeHidden();
  expect((await state(page)).color1).toBe('#123456');
});

test('airbrush keeps spraying while the button is held', async ({ page }) => {
  await openRibbonMenu(page, 'btn-brushes');
  await page.click('[data-testid=brush-airbrush]');
  const s = await page.locator('#canvas').boundingBox();
  await page.mouse.move(s.x + 200, s.y + 200);
  await page.mouse.down();
  const count = () => page.evaluate(() => {
    let n = 0;
    for (let y = 170; y < 230; y++) for (let x = 170; x < 230; x++) if (window.paint.engine.pixel(x, y) !== '#ffffff') n++;
    return n;
  });
  const first = await count();
  await page.waitForTimeout(400);
  const later = await count();
  await page.mouse.up();
  expect(later).toBeGreaterThan(first);
  await expectPixel(page, 0, 0, '#ffffff');
});
