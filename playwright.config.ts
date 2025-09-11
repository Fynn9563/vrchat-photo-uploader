import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  timeout: 30 * 1000,
  expect: {
    timeout: 5000
  },
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  
  use: {
    actionTimeout: 0,
    baseURL: 'http://localhost:1420', // Tauri dev server
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },

  projects: [
    {
      name: 'tauri-app',
      testMatch: /.*\.e2e\.ts/,
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1200, height: 800 },
      },
    },
  ],

  webServer: {
    command: 'pnpm tauri dev',
    port: 1420,
    reuseExistingServer: !process.env.CI,
    timeout: 120 * 1000,
  },
});