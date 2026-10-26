// Browser Back/Forward buttons undo and redo instead of leaving the page.
import { test, expect } from '@playwright/test';
import { openApp, pixel, drag } from './helpers.js';

test('Back undoes, Forward redoes, and the page stays', async ({ page }) => {
  await openApp(page);
  await page.click('[data-testid=tool-pencil]');
  await drag(page, [[10, 10], [60, 10]]);
  await drag(page, [[10, 30], [60, 30]]);
  const url = page.url();
  await page.goBack();
  await expect.poll(() => pixel(page, 30, 30)).toBe('#ffffff');
  expect(await pixel(page, 30, 10)).toBe('#000000');
  await page.goBack();
  await expect.poll(() => pixel(page, 30, 10)).toBe('#ffffff');
  await page.goForward();
  await expect.poll(() => pixel(page, 30, 10)).toBe('#000000');
  expect(page.url()).toBe(url);
  // Still the same document (not reloaded).
  expect(await page.evaluate(() => window.paint.state.can_redo)).toBe(true);
});
