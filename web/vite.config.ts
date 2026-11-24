import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { mockNodePlugin } from './vite/mock-node-plugin.ts'

export default defineConfig({
  plugins: [react(), mockNodePlugin()],
})
