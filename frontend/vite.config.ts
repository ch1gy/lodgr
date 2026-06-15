import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

// Vite proxy — the backend uses SameSite=Strict cookies and no CORS headers,
// so in dev we MUST route /auth, /tickets, /admin, /reports through Vite to
// stay on the same origin. See FRONTEND_HANDOFF.md.
//
// In production the built frontend is served by the backend's ServeDir at /,
// so no proxy or CORS config is needed.

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'node',
  },
  server: {
    proxy: {
      '/auth': 'http://localhost:3000',
      '/tickets': 'http://localhost:3000',
      '/admin': 'http://localhost:3000',
      '/reports': 'http://localhost:3000',
      '/uploads': 'http://localhost:3000',
    },
  },
})
