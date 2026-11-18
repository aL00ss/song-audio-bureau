import { defineConfig } from '@playwright/test'

// Tests navigateur : Chromium, son autorisé sans geste (le contexte audio démarre seul).
// URL_STUDIO=https://al00ss.github.io/song-audio/ : lance les tests sur un site déjà publié.
const urlStudio = process.env.URL_STUDIO

export default defineConfig({
  testDir: 'tests/e2e',
  timeout: 60_000,
  fullyParallel: false,
  reporter: 'list',
  use: {
    baseURL: urlStudio ?? 'http://localhost:4173',
    browserName: 'chromium',
    launchOptions: { args: ['--autoplay-policy=no-user-gesture-required'] },
    locale: 'fr-FR',
  },
  webServer: urlStudio
    ? undefined
    : {
        command: 'npm run build && npm run preview -- --port 4173 --strictPort',
        url: 'http://localhost:4173',
        reuseExistingServer: true,
        timeout: 180_000,
      },
})
