import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'


// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      '/health': 'http://localhost:4317',
      '/query': 'http://localhost:4317',
      '/tail': 'http://localhost:4317',
      '/detect': 'http://localhost:4317',
      '/status': 'http://localhost:4317',
      '/resources': 'http://localhost:4317',
    },
  },
})
