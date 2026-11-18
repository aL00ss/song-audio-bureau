// Tests de l'application de bureau : Playwright se branche sur la fenêtre WebView2 de Tauri
// (lancée avec WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222).
import { chromium, expect, test } from '@playwright/test'

const PORT = process.env.PORT_DEBOGAGE ?? '9222'

test('la fenêtre de bureau affiche le studio, le son démarre et le pont Rust répond', async () => {
  const navigateur = await chromium.connectOverCDP(`http://localhost:${PORT}`)
  const page = navigateur.contexts()[0].pages()[0]
  await expect(page).toHaveTitle(/song-audio/)
  await expect(page.getByTestId('statut')).toHaveText('Prêt', { timeout: 60_000 })
  const version = await page.evaluate(() => (window as any).__TAURI_INTERNALS__.invoke('version'))
  expect(version).toMatch(/^\d+\.\d+\.\d+$/)
  await expect(page.locator('.dv-tab')).toHaveCount(6)
  await page.getByTestId('lecture').click()
  await expect.poll(() => page.evaluate(() => (window as any).__studio.voixParCanal().reduce((a: number, b: number) => a + b, 0)), { timeout: 10_000 }).toBeGreaterThan(0)
  await page.getByTestId('stop').click()
  await page.screenshot({ path: 'test-results/bureau-fenetre.png' })
  await navigateur.close()
})
