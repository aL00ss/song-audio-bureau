import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

// Sur GitHub Pages, le site est servi sous /song-audio/ (variable posée par le déploiement).
export default defineConfig({
  base: process.env.BASE_PUBLIC ?? '/',
  plugins: [react(), tailwindcss()],
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
})
