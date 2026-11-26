import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// Kept apart from vite.config.ts so tests do not load the dev-server mock node plugin.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    css: false,
    include: ['src/**/*.test.{ts,tsx}'],
    // Several agents share this machine and its load can be far above its cores; a test that is only slow
    // should not fail. A test that is wrong still fails, only later.
    testTimeout: 20_000,
  },
})
