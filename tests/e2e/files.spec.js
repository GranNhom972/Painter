// Files (browser build): new, open, save, save as, unsaved-changes prompt,
// drag and drop, print; and undo/redo from the quick access bar.
import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { openApp, state, pixel, drag, rect, menu, makePng, pngSize, expectPixel } from './helpers.js';

test.beforeEach(async ({ page }) => openApp(page));

test('save downloads a PNG of the picture and shows its size', async ({ page }) => {
  await rect(page, 0, 0, 10, 10, '#ed1c24');
  const dl = page.waitForEvent('download');
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+s');
  const download = await dl;
  expect(download.suggestedFilename()).toBe('Sans titre.png');
  const buf = await readFile(await download.path());
  expect(pngSize(buf)).toEqual({ w: 1152, h: 648 });
  await expect(page.locator('[data-testid=stat-file]')).toHaveText(/^Taille : \d+(,\d)? (octets|Ko)$/);
  await expect(page).toHaveTitle('Paint - Sans titre.png');
  expect((await state(page)).modified).toBe(false);
});

test('save as every format', async ({ page }) => {
  const magic = { png: [0x89, 0x50], jpg: [0xff, 0xd8], bmp: [0x42, 0x4d], gif: [0x47, 0x49], tiff: [0x49, 0x49], webp: [0x52, 0x49], ico: [0x00, 0x00] };
  for (const [ext, head] of Object.entries(magic)) {
    const dl = page.waitForEvent('download');
    await menu(page, 'file', 'mi-save-as', `mi-save-${ext}`);
    const download = await dl;
    expect(download.suggestedFilename()).toMatch(new RegExp(`\\.${ext}$`));
    const buf = await readFile(await download.path());
    expect([...buf.subarray(0, 2)], ext).toEqual(head);
  }
});

test('open an image with the file chooser', async ({ page }) => {
  const png = await makePng(page, 64, 48, '#3f48cc');
  const chooser = page.waitForEvent('filechooser');
  await menu(page, 'file', 'mi-open');
  await (await chooser).setFiles({ name: 'bleu.png', mimeType: 'image/png', buffer: png });
  await expect.poll(async () => (await state(page)).width).toBe(64);
  expect(await pixel(page, 10, 10)).toBe('#3f48cc');
  await expect(page).toHaveTitle('Paint - bleu.png');
  await expect(page.locator('[data-testid=stat-canvas]')).toHaveText('64 x 48px');
  expect((await state(page)).can_undo).toBe(false);
});

test('opening a broken file shows an error and keeps the picture', async ({ page }) => {
  const chooser = page.waitForEvent('filechooser');
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+o');
  await (await chooser).setFiles({ name: 'broken.png', mimeType: 'image/png', buffer: Buffer.from('nope') });
  await expect(page.locator('[data-testid=dlg-message]')).toBeVisible();
  await page.click('[data-testid=dlg-message] [data-testid=dlg-ok]');
  expect((await state(page)).width).toBe(1152);
});

test('new asks to save unsaved changes', async ({ page }) => {
  await page.click('[data-testid=tool-pencil]');
  await drag(page, [[10, 10], [50, 10]]);
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+n');
  const dlg = page.locator('[data-testid=dlg-confirm]');
  await expect(dlg).toBeVisible();
  await expect(dlg).toContainText('Sans titre');
  await page.click('[data-testid=confirm-cancel]');
  await expect(dlg).toBeHidden();
  expect(await pixel(page, 30, 10)).toBe('#000000');
  await menu(page, 'file', 'mi-new');
  await page.click('[data-testid=confirm-discard]');
  await expectPixel(page, 30, 10, '#ffffff');
  expect((await state(page)).can_undo).toBe(false);
  // "Save" in the prompt saves before clearing.
  await drag(page, [[10, 10], [50, 10]]);
  await menu(page, 'file', 'mi-new');
  const dl = page.waitForEvent('download');
  await page.click('[data-testid=confirm-save]');
  await dl;
  await expectPixel(page, 30, 10, '#ffffff');
});

test('drop an image file onto the canvas', async ({ page }) => {
  const png = await makePng(page, 30, 20, '#22b14c');
  const dt = await page.evaluateHandle((bytes) => {
    const d = new DataTransfer();
    d.items.add(new File([new Uint8Array(bytes)], 'vert.png', { type: 'image/png' }));
    return d;
  }, [...png]);
  await page.dispatchEvent('#viewport', 'dragover', { dataTransfer: dt });
  await page.dispatchEvent('#viewport', 'drop', { dataTransfer: dt });
  await expect.poll(async () => (await state(page)).width).toBe(30);
  expect(await pixel(page, 5, 5)).toBe('#22b14c');
});

test('print uses the browser print dialog', async ({ page }) => {
  await page.evaluate(() => {
    window.__printed = 0;
    window.print = () => window.__printed++;
  });
  await menu(page, 'file', 'mi-print');
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+p');
  expect(await page.evaluate(() => window.__printed)).toBe(2);
});

test('undo and redo from the quick access bar, menu and keyboard', async ({ page }) => {
  await expect(page.locator('[data-testid=qa-undo]')).toBeDisabled();
  await page.click('[data-testid=tool-pencil]');
  await drag(page, [[10, 10], [50, 10]]);
  await expect(page.locator('[data-testid=qa-undo]')).toBeEnabled();
  await page.click('[data-testid=qa-undo]');
  expect(await pixel(page, 30, 10)).toBe('#ffffff');
  await expect(page.locator('[data-testid=qa-redo]')).toBeEnabled();
  await page.click('[data-testid=qa-redo]');
  expect(await pixel(page, 30, 10)).toBe('#000000');
  await menu(page, 'edit', 'mi-undo');
  expect(await pixel(page, 30, 10)).toBe('#ffffff');
  await page.keyboard.press('Control+Shift+Z');
  expect(await pixel(page, 30, 10)).toBe('#000000');
  await page.keyboard.press('Control+z');
  await page.keyboard.press('Control+y');
  expect(await pixel(page, 30, 10)).toBe('#000000');
});
