// Keyboard access, ARIA states and narrow windows (design audit fixes).
import { test, expect } from '@playwright/test';
import { openApp, state } from './helpers.js';

const focused = (page) => page.evaluate(() => document.activeElement && (document.activeElement.dataset.testid || document.activeElement.className));

test('menu bar is usable with the keyboard', async ({ page }) => {
  await openApp(page);
  await page.focus('[data-testid=menu-file]');
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-testid=popup-file]')).toBeVisible();
  await expect(page.locator('[data-testid=menu-file]')).toHaveAttribute('aria-expanded', 'true');
  expect(await focused(page)).toBe('mi-new');
  await page.keyboard.press('ArrowUp');
  expect(await focused(page)).toBe('mi-quit');
  await page.keyboard.press('Home');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  expect(await focused(page)).toBe('mi-save-as');
  // Right opens the submenu, Left goes back to its item.
  await page.keyboard.press('ArrowRight');
  expect(await focused(page)).toBe('mi-save-png');
  await page.keyboard.press('ArrowLeft');
  expect(await focused(page)).toBe('mi-save-as');
  // Right on a plain item moves to the next menu of the bar.
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('ArrowRight');
  await expect(page.locator('[data-testid=popup-edit]')).toBeVisible();
  // Escape closes and gives the focus back to the menu button.
  await page.keyboard.press('Escape');
  await expect(page.locator('.popup')).toHaveCount(0);
  expect(await focused(page)).toBe('menu-edit');
  await expect(page.locator('[data-testid=menu-edit]')).toHaveAttribute('aria-expanded', 'false');
  // Enter on an item runs it (View > Gridlines).
  await page.focus('[data-testid=menu-view]');
  await page.keyboard.press('ArrowDown');
  for (let i = 0; i < 5; i++) await page.keyboard.press('ArrowDown');
  expect(await focused(page)).toBe('mi-grid');
  await page.keyboard.press('Enter');
  expect(await page.evaluate(() => window.paint.view.showGrid)).toBe(true);
  expect(await focused(page)).toBe('menu-view');
});

test('galleries open with Down and are navigated with the arrows', async ({ page }) => {
  await openApp(page);
  await page.focus('[data-testid=btn-shapes]');
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('[data-testid=popup-shapes]')).toHaveAttribute('role', 'dialog');
  // Focus starts on the current shape (rectangle, 4th cell); Right then Down (6 columns).
  expect(await focused(page)).toBe('shape-rectangle');
  await page.keyboard.press('ArrowRight');
  expect(await focused(page)).toBe('shape-rounded-rectangle');
  await page.keyboard.press('ArrowDown');
  expect(await focused(page)).toBe('shape-hexagon');
  await page.keyboard.press('Enter');
  const st = await state(page);
  expect([st.tool, st.shape]).toEqual(['shape', 'hexagon']);
  // The brushes gallery too.
  await page.focus('[data-testid=btn-brushes]');
  await page.keyboard.press('ArrowDown');
  expect(await focused(page)).toBe('brush-brush');
  // Three columns: Down goes from the first to the fourth brush.
  await page.keyboard.press('ArrowDown');
  expect(await focused(page)).toBe('brush-airbrush');
  await page.keyboard.press(' ');
  expect((await state(page)).brush).toBe('airbrush');
});

test('tools expose aria-pressed and a selected look different from hover', async ({ page }) => {
  await openApp(page);
  await page.click('[data-testid=tool-fill]');
  await expect(page.locator('[data-testid=tool-fill]')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('[data-testid=tool-pencil]')).toHaveAttribute('aria-pressed', 'false');
  await page.hover('[data-testid=tool-pencil]');
  const look = (id) => page.locator(`[data-testid=${id}]`).evaluate((el) => getComputedStyle(el).boxShadow);
  expect(await look('tool-fill')).not.toBe('none');
  expect(await look('tool-pencil')).toBe('none');
  await expect(page.locator('[data-testid=color1]')).toHaveAttribute('aria-pressed', 'true');
  // Palette chips have names.
  await expect(page.locator('[data-testid=palette-3]')).toHaveAttribute('aria-label', 'Rouge');
});

test('menus show the current choice', async ({ page }) => {
  await openApp(page);
  await page.click('[data-testid=btn-selection] .big-ico');
  await page.click('[data-testid=btn-selection] [data-icon=chevron]');
  await expect(page.locator('[data-testid=mi-select-rect]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid=mi-select-free]')).toHaveAttribute('aria-checked', 'false');
  await page.keyboard.press('Escape');
  await page.click('[data-testid=zoom-value]');
  await expect(page.locator('[data-testid=mi-zoom-100]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid=popup-zoom]')).toHaveClass(/no-icons/);
  await page.keyboard.press('Escape');
  await page.click('[data-testid=menu-settings]');
  await page.hover('[data-testid=mi-language]');
  await expect(page.locator('[data-testid=mi-lang-fr]')).toHaveAttribute('role', 'menuitemradio');
});

test('dialog fields: hue slider keeps its rainbow, radios stay inline', async ({ page }) => {
  await openApp(page);
  await page.click('[data-testid=btn-edit-colors]');
  const hue = await page.locator('[data-testid=color-hue]').evaluate((el) => {
    const cs = getComputedStyle(el);
    return { bg: cs.backgroundImage, h: el.getBoundingClientRect().height };
  });
  expect(hue.bg).toContain('gradient');
  expect(hue.h).toBeLessThan(20);
  await page.keyboard.press('Escape');
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+w');
  // The first field is focused and selected.
  expect(await focused(page)).toBe('resize-w');
  const radio = await page.locator('[data-testid=resize-percent]').boundingBox();
  const label = await page.locator('[data-testid=resize-percent] + span').boundingBox();
  expect(Math.abs(radio.y + radio.height / 2 - (label.y + label.height / 2))).toBeLessThan(4);
});

test('language switch updates the document language and shortcut labels', async ({ page }) => {
  await openApp(page);
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr');
  await page.click('[data-testid=menu-settings]');
  await page.hover('[data-testid=mi-language]');
  await page.click('[data-testid=mi-lang-en]');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await page.click('[data-testid=menu-edit]');
  await expect(page.locator('[data-testid=mi-delete] .mi-key')).toHaveText('Del');
  await expect(page.locator('[data-testid=mi-crop] .mi-key')).toHaveText('Ctrl+Shift+X');
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid=ribbon]')).toHaveAttribute('aria-label', 'Ribbon');
});

for (const [w, h] of [[800, 600], [600, 500], [460, 500]]) {
  test(`narrow window ${w}x${h}: every ribbon group stays reachable`, async ({ page }) => {
    await page.setViewportSize({ width: w, height: h });
    await openApp(page);
    const ribbon = page.locator('[data-testid=ribbon]');
    await expect(ribbon).toHaveClass(/compact-colors/);
    const overflow = await ribbon.evaluate((r) => r.scrollWidth - r.clientWidth);
    if (w >= 600) expect(overflow).toBe(0);
    // Colours from the compact dropdown.
    await page.click('[data-testid=btn-colors-more]');
    await page.click('[data-testid=menu-palette-6]');
    expect((await state(page)).color1).toBe('#22b14c');
    // Image commands from the compact dropdown.
    if (await page.locator('[data-testid=btn-image-more]').isVisible()) {
      await page.click('[data-testid=btn-image-more]');
      await page.click('[data-testid=mi-image-rotate]');
      await page.click('[data-testid=mi-rotate-right]');
      expect((await state(page)).width).toBe(648);
    }
    // The canvas fits the window at start and the zoom controls are visible.
    const canvas = await page.locator('#canvas').boundingBox();
    const vp = await page.locator('#viewport').boundingBox();
    expect(canvas.x + canvas.width).toBeLessThanOrEqual(vp.x + vp.width);
    await expect(page.locator('[data-testid=zoom-in]')).toBeInViewport();
    // The wheel scrolls an overflowing ribbon horizontally.
    if (overflow > 0) {
      const b = await ribbon.boundingBox();
      await page.mouse.move(b.x + 50, b.y + 40);
      await page.mouse.wheel(0, 200);
      await expect.poll(() => ribbon.evaluate((r) => r.scrollLeft)).toBeGreaterThan(0);
    }
  });
}
