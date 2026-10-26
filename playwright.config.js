import { defineConfig } from '@playwright/test';

// PAINT_URL runs the suite against an already running server (the Docker image).
const external = process.env.PAINT_URL;

export default defineConfig({
  testDir: 'tests/e2e',
  timeout: 60000,
  expect: { timeout: 10000 },
  fullyParallel: true,
  workers: 4,
  reporter: [['list']],
  use: {
    baseURL: external || 'http://127.0.0.1:4173/',
    viewport: { width: 1280, height: 900 },
    locale: 'fr-FR',
    permissions: ['clipboard-read', 'clipboard-write'],
  },
  webServer: external ? undefined : {
    command: 'node scripts/serve.mjs',
    url: 'http://127.0.0.1:4173/',
    reuseExistingServer: true,
  },
});
