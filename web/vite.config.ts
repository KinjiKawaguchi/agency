import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { mockNodePlugin } from './vite/mock-node-plugin.ts'

export default defineConfig({
  plugins: [react(), mockNodePlugin()],
  // The end-to-end tests reach the preview server under a host that is not loopback, as a tunnel does.
  preview: { allowedHosts: ['.test'] },
})
