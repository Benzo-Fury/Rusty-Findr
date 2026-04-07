import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'
import { existsSync, readFileSync } from 'fs'
import { lookup } from 'mime-types'
import type { Plugin } from 'vite'

function serveRootAssets(): Plugin {
  const assetsDir = path.resolve(__dirname, '../assets/compiled')
  return {
    name: 'serve-root-assets',
    configureServer(server) {
      server.middlewares.use('/web', (req, res, next) => {
        const filePath = path.join(assetsDir, req.url ?? '')
        if (existsSync(filePath)) {
          const mime = lookup(filePath) || 'application/octet-stream'
          res.setHeader('Content-Type', mime)
          res.end(readFileSync(filePath))
          return
        }
        next()
      })
    },
  }
}

// https://vite.dev/config/
export default defineConfig({
  base: '/web/',
  publicDir: false,
  plugins: [react(), tailwindcss(), serveRootAssets()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    hmr: {
      port: 5173,
    },
    proxy: {
      '/api': 'http://localhost:3030',
    },
  },
})
