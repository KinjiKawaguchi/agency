import { defineConfig, devices } from '@playwright/test'

// Runs the built client against the mock node served by `vite preview`. In CI the browser comes from
// `playwright install`. On a machine that already has a headless Chromium, AGENCY_CHROMIUM_PATH points at it.
const executablePath = process.env.AGENCY_CHROMIUM_PATH
const port = 4173

export default defineConfig({
  testDir: 'e2e',
  outputDir: 'e2e-results',
  // The mock node keeps its state in memory, so tests run one at a time and reset it first.
  fullyParallel: false,
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never', outputFolder: 'e2e-report' }], ['./e2e/flakyReporter.ts']] : 'list',
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: 'retain-on-failure',
    ...(executablePath === undefined ? {} : { launchOptions: { executablePath } }),
  },
  webServer: {
    command: `npx vite preview --host 127.0.0.1 --port ${port} --strictPort`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: !process.env.CI,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
})
