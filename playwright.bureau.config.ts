// Tests de l'application de bureau : la fenêtre Tauri doit déjà tourner, avec le port de
// débogage de WebView2 ouvert (voir outils/tests-bureau.mjs).
import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: 'tests/bureau',
  timeout: 120_000,
  workers: 1,
  reporter: [['list']],
})
