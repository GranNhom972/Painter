// Web version: installable (manifest + icons) and usable offline (service worker).
import { test, expect } from '@playwright/test';
import { openApp, state, drag, rect } from './helpers.js';

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

test('serves the engine as application/wasm', async ({ request }) => {
  // Streaming compilation refuses any other type.
  const res = await request.get('pkg/paint_wasm_bg.wasm');
  expect(res.ok()).toBe(true);
  expect(res.headers()['content-type']).toBe('application/wasm');
});

test('no Content Security Policy violation while working', async ({ page }) => {
  // Only the Docker image sends a CSP (PAINT_URL); with the dev server the list stays empty.
  await page.addInitScript(() => {
    window.violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.violations.push(`${e.violatedDirective} ${e.blockedURI}`));
  });
  await openApp(page);
  // Tool cursors are data: URLs, the size menu uses inline styles, layer
  // thumbnails are blob: URLs and saving downloads a blob: URL.
  await page.click('[data-testid=tool-pencil]');
  await drag(page, [[30, 30], [90, 60]]);
  await page.click('[data-testid=btn-size]');
  await page.click('[data-testid=size-8]');
  await page.click('[data-testid=btn-layers]');
  await expect(page.locator('[data-testid=layer-0] img')).toHaveAttribute('src', /^blob:/);
  await rect(page, 0, 0, 10, 10, '#ed1c24');
  const dl = page.waitForEvent('download');
  await page.locator('#viewport').focus();
  await page.keyboard.press('Control+s');
  await dl;
  expect(await page.evaluate(() => window.violations)).toEqual([]);
});
