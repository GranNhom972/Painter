// Web version: installable (manifest + icons) and usable offline (service worker).
import { test, expect } from '@playwright/test';
import { openApp, state } from './helpers.js';

test('web app manifest and icons', async ({ page, request }) => {
  await openApp(page);
  const href = await page.locator('link[rel=manifest]').getAttribute('href');
  const res = await request.get(href);
  expect(res.headers()['content-type']).toContain('application/manifest+json');
  const manifest = await res.json();
  expect(manifest).toMatchObject({ name: 'Paint', display: 'standalone', start_url: './' });
  for (const icon of manifest.icons) {
    const img = await request.get(icon.src);
    expect(img.ok(), icon.src).toBe(true);
  }
  expect(manifest.file_handlers[0].accept['image/*']).toContain('.png');
});

test('works offline once loaded', async ({ page, context }) => {
  await openApp(page);
  await page.evaluate(() => navigator.serviceWorker.ready);
  // Reload once so every asset goes through the service worker cache.
  await page.reload();
  await page.waitForSelector('body[data-ready=true]');
  await context.setOffline(true);
  await page.reload();
  await page.waitForSelector('body[data-ready=true]');
  const st = await state(page);
  expect([st.width, st.height]).toEqual([1152, 648]);
  expect(await page.evaluate(() => window.paint.engine.pixel(1, 1))).toBe('#ffffff');
  await context.setOffline(false);
});
