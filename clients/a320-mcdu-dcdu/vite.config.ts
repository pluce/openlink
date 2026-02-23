import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'node:path'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@openlink/sdk-ts': path.resolve(__dirname, '../openlink-sdk-ts/src/index.ts'),
    },
  },
  server: {
    // Reverse proxy to avoid CORS issues during development.
    // The React app calls "/api/auth/exchange" which Vite forwards
    // to the real auth service (e.g. http://localhost:3001/exchange).
    proxy: {
      '/api/auth': {
        target: 'http://localhost:3001',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api\/auth/, ''),
      },
    },
  },
})
