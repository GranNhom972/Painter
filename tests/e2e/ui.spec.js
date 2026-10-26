// Layout, menus, zoom, view options, language and theme.
import { test, expect } from '@playwright/test';
import { openApp, state, toScreen, menu, screenPixel } from './helpers.js';

test.beforeEach(async ({ page }) => openApp(page));

test('ribbon groups match Paint, without Image Creator', async ({ page }) => {
  const labels = await page.locator('.ribbon .group-label').allTextContents();
  expect(labels).toEqual(['Sélection', 'Image', 'Outils', 'Pinceaux', 'Formes', 'Taille', 'Couleurs', 'Calques']);
  await expect(page.locator('text=Image Creator')).toHaveCount(0);
  await expect(page.locator('[data-testid=palette] .chip')).toHaveCount(30);
  await expect(page.locator('[data-testid=stat-canvas]')).toHaveText('1152 x 648px');
  await expect(page).toHaveTitle('Paint - Sans titre');
});

test('canvas starts white and is displayed from WebAssembly memory', async ({ page }) => {
  expect(await screenPixel(page, 10, 10)).toBe('#ffffff');
  const st = await state(page);
  expect([st.width, st.height, st.tool, st.color1, st.color2]).toEqual([1152, 648, 'pencil', '#000000', '#ffffff']);
});

test('menus open, close with Escape and switch on hover', async ({ page }) => {
  await page.click('[data-testid=menu-file]');
  await expect(page.locator('[data-testid=popup-file]')).toBeVisible();
  await page.hover('[data-testid=menu-edit]');
  await expect(page.locator('[data-testid=popup-edit]')).toBeVisible();
  await expect(page.locator('[data-testid=popup-file]')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(page.locator('.popup')).toHaveCount(0);
  // Clicking the same menu twice closes it.
  await page.click('[data-testid=menu-view]');
  await page.click('[data-testid=menu-view]');
  await expect(page.locator('.popup')).toHaveCount(0);
  // Clicking elsewhere closes it.
  await page.click('[data-testid=menu-view]');
  await page.mouse.click(600, 500);
  await expect(page.locator('.popup')).toHaveCount(0);
});

test('pointer position and title update', async ({ page }) => {
  const s = await toScreen(page, 120, 45);
  await page.mouse.move(s.x, s.y);
  await expect(page.locator('[data-testid=stat-pos]')).toHaveText('120,45px');
  await page.mouse.down();
  await page.mouse.move(s.x + 20, s.y + 5);
  await page.mouse.up();
  await expect(page).toHaveTitle('● Paint - Sans titre');
});

test('zoom with buttons, menu, keyboard, slider, ctrl+wheel and fit', async ({ page }) => {
  const zoom = () => page.evaluate(() => window.paint.view.zoom);
  await page.click('[data-testid=zoom-in]');
  expect(await zoom()).toBe(2);
  await expect(page.locator('[data-testid=zoom-value]')).toHaveText('200%');
  await page.click('[data-testid=zoom-out]');
  await page.click('[data-testid=zoom-out]');
  expect(await zoom()).toBe(0.5);
  await page.click('[data-testid=zoom-value]');
  await page.click('[data-testid=mi-zoom-400]');
  expect(await zoom()).toBe(4);
  await page.keyboard.press('Control+1');
  expect(await zoom()).toBe(1);
  await page.keyboard.press('Control+=');
  expect(await zoom()).toBe(2);
  await page.keyboard.press('Control+-');
  expect(await zoom()).toBe(1);
  await menu(page, 'view', 'mi-zoom-in');
  expect(await zoom()).toBe(2);
  await page.click('[data-testid=zoom-fit]');
  const fit = await zoom();
  expect(fit).toBeLessThan(1.2);
  const vp = await page.locator('#viewport').boundingBox();
  const cv = await page.locator('#canvas').boundingBox();
  expect(cv.width).toBeLessThanOrEqual(vp.width);
  // Slider is logarithmic: the middle-ish value is close to 100%.
  await page.locator('[data-testid=zoom-slider]').fill('525');
  expect(Math.abs((await zoom()) - 1)).toBeLessThan(0.02);
  // Ctrl + wheel.
  await page.mouse.move(vp.x + 200, vp.y + 200);
  await page.keyboard.down('Control');
  await page.mouse.wheel(0, -400);
  await page.keyboard.up('Control');
  await expect.poll(zoom).toBeGreaterThan(1.5);
  // The engine knows the zoom (handle tolerance).
  expect(await page.evaluate(() => window.paint.view.zoom)).toBeGreaterThan(1);
});

test('rulers, gridlines and status bar toggles', async ({ page }) => {
  await page.keyboard.press('Control+r');
  await expect(page.locator('#rulerH')).toBeVisible();
  await expect(page.locator('#rulerV')).toBeVisible();
  await menu(page, 'view', 'mi-rulers');
  await expect(page.locator('#rulerH')).toBeHidden();
  await page.keyboard.press('Control+g');
  expect(await page.evaluate(() => window.paint.view.showGrid)).toBe(true);
  await page.click('[data-testid=menu-view]');
  await expect(page.locator('[data-testid=mi-grid]')).toHaveAttribute('aria-checked', 'true');
  await page.click('[data-testid=mi-grid]');
  expect(await page.evaluate(() => window.paint.view.showGrid)).toBe(false);
  await menu(page, 'view', 'mi-statusbar');
  await expect(page.locator('[data-testid=statusbar]')).toBeHidden();
  await menu(page, 'view', 'mi-statusbar');
  await expect(page.locator('[data-testid=statusbar]')).toBeVisible();
});

test('language switch to English and back', async ({ page }) => {
  await page.click('[data-testid=menu-settings]');
  await page.hover('[data-testid=mi-language]');
  await page.click('[data-testid=mi-lang-en]');
  await expect(page.locator('[data-testid=menu-file]')).toHaveText('File');
  await expect(page.locator('.ribbon .group-label').first()).toHaveText('Selection');
  await expect(page).toHaveTitle('Paint - Untitled');
  await page.click('[data-testid=menu-settings]');
  await page.hover('[data-testid=mi-language]');
  await page.click('[data-testid=mi-lang-fr]');
  await expect(page.locator('[data-testid=menu-file]')).toHaveText('Fichier');
});

test('dark and light themes', async ({ page }) => {
  await page.click('[data-testid=menu-settings]');
  await page.hover('[data-testid=mi-theme]');
  await page.click('[data-testid=mi-theme-dark]');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  const bg = await page.evaluate(() => getComputedStyle(document.querySelector('.ribbon')).backgroundColor);
  expect(bg).toBe('rgb(38, 38, 43)');
  await page.click('[data-testid=menu-settings]');
  await page.hover('[data-testid=mi-theme]');
  await page.click('[data-testid=mi-theme-system]');
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /.+/);
});

test('about dialog', async ({ page }) => {
  await page.click('[data-testid=menu-settings]');
  await page.click('[data-testid=mi-about]');
  await expect(page.locator('[data-testid=dlg-message]')).toContainText('WebAssembly');
  await page.click('[data-testid=dlg-message] [data-testid=dlg-ok]');
  await expect(page.locator('[data-testid=dlg-message]')).toBeHidden();
});

test('no console errors during a typical session', async ({ page }) => {
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
  await page.reload();
  await page.waitForSelector('body[data-ready=true]');
  for (const id of ['btn-brushes', 'btn-shapes', 'btn-size', 'btn-selection', 'btn-rotate', 'btn-flip']) {
    await page.click(`[data-testid=${id}] [data-icon=chevron]`);
    await page.keyboard.press('Escape');
  }
  expect(errors).toEqual([]);
});

test('middle button pans the view', async ({ page }) => {
  await page.click('[data-testid=zoom-in]');
  await page.click('[data-testid=zoom-in]');
  const vp = page.locator('#viewport');
  const before = await vp.evaluate((el) => [el.scrollLeft, el.scrollTop]);
  const b = await vp.boundingBox();
  await page.mouse.move(b.x + 400, b.y + 300);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(b.x + 200, b.y + 150, { steps: 5 });
  await page.mouse.up({ button: 'middle' });
  const after = await vp.evaluate((el) => [el.scrollLeft, el.scrollTop]);
  expect(after[0] - before[0]).toBe(200);
  expect(after[1] - before[1]).toBe(150);
  // Nothing was drawn.
  expect((await state(page)).can_undo).toBe(false);
});

test('full screen from the view menu and F11', async ({ page }) => {
  await page.evaluate(() => {
    window.__fs = 0;
    document.documentElement.requestFullscreen = async () => {
      window.__fs++;
    };
  });
  await menu(page, 'view', 'mi-fullscreen');
  await page.locator('#viewport').focus();
  await page.keyboard.press('F11');
  expect(await page.evaluate(() => window.__fs)).toBe(2);
});

test('text style toggles update the text box', async ({ page }) => {
  await page.click('[data-testid=tool-text]');
  const s = await toScreen(page, 50, 50);
  await page.mouse.click(s.x, s.y);
  const box = page.locator('[data-testid=text-box]');
  for (const id of ['italic', 'underline', 'strike']) await page.click(`[data-testid=text-${id}]`);
  await page.locator('[data-testid=text-font]').selectOption('monospace');
  const style = await box.evaluate((el) => ({ font: el.style.font, deco: el.style.textDecoration }));
  expect(style.font).toContain('italic');
  expect(style.font).toContain('monospace');
  expect(style.deco).toContain('underline');
  expect(style.deco).toContain('line-through');
  await page.keyboard.type('a');
  await page.keyboard.press('Control+b');
  await expect(page.locator('[data-testid=text-bold]')).toHaveClass(/on/);
  await page.keyboard.press('Control+Enter');
  await expect(box).toBeHidden();
  expect((await state(page)).can_undo).toBe(true);
});
