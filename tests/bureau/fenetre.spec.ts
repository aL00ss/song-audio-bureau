// Tests de l'application de bureau : Playwright se branche sur la fenêtre WebView2 de Tauri
// (lancée avec WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222).
import { type Browser, chromium, expect, type Page, test } from '@playwright/test'

const PORT = process.env.PORT_DEBOGAGE ?? '9222'
let navigateur: Browser
let page: Page

test.beforeAll(async () => {
  navigateur = await chromium.connectOverCDP(`http://localhost:${PORT}`)
  page = navigateur.contexts()[0].pages()[0]
  await expect(page.getByTestId('statut')).toHaveText('Prêt', { timeout: 60_000 })
})

test.afterAll(async () => {
  await navigateur.close()
})

/** Crête maximale d'une piste du mixer pendant `ms`. */
const crete = (piste: number, ms: number) =>
  page.evaluate(
    async ([p, duree]) => {
      const m = (window as any).__studio.audio.moteur
      let max = 0
      const fin = performance.now() + duree
      while (performance.now() < fin) {
        const n = m.niveauPiste(p)
        max = Math.max(max, n.gauche, n.droite)
        await new Promise((r) => setTimeout(r, 20))
      }
      return max
    },
    [piste, ms] as const,
  )

test('la fenêtre affiche le studio et le pont Rust répond', async () => {
  await expect(page).toHaveTitle(/song-audio/)
  const version = await page.evaluate(() => (window as any).__TAURI_INTERNALS__.invoke('version'))
  expect(version).toMatch(/^\d+\.\d+\.\d+$/)
  await expect(page.locator('.dv-tab')).toHaveCount(6)
})

test('le son passe par le moteur Rust : canaux, pistes et maître sonnent pendant la lecture', async () => {
  await page.evaluate(() => (window as any).__studio.etat().chargerDemo())
  expect(await page.evaluate(() => (window as any).__studio.audio.moteur.banquesSupplementaires)).toBe(false)
  await page.getByTestId('lecture').click()
  await expect.poll(() => page.evaluate(() => (window as any).__studio.voixParCanal().filter((v: number) => v > 0).length), { timeout: 10_000 }).toBeGreaterThan(1)
  for (const piste of [1, 2, 3, 4, 0]) expect(await crete(piste, 1200), `piste ${piste}`).toBeGreaterThan(0.01)
  // solo sur le piano : les autres pistes se taisent
  await page.evaluate(() => (window as any).__studio.etat().modifierPiste(1, { solo: true }))
  await page.waitForTimeout(200)
  expect(await crete(4, 800)).toBeLessThan(0.0005)
  expect(await crete(1, 1200)).toBeGreaterThan(0.01)
  await page.evaluate(() => (window as any).__studio.etat().modifierPiste(1, { solo: false }))
  await page.getByTestId('stop').click()
  await page.screenshot({ path: 'test-results/bureau-fenetre.png' })
})

test('export WAV rendu par le moteur Rust : bonne durée, non silencieux', async () => {
  await page.evaluate(() => (window as any).__studio.etat().changerMode('pattern'))
  const r = await page.evaluate(async () => {
    const e = (window as any).__studio.etat()
    const wav: Blob = await e.exporterWav()
    const tampon = await new AudioContext().decodeAudioData(await wav.arrayBuffer())
    let max = 0
    for (const v of tampon.getChannelData(0)) max = Math.max(max, Math.abs(v))
    const pattern = e.projet.patterns.find((p: any) => p.id === e.projet.patternActif)
    return { duree: tampon.duration, crete: max, attendue: (pattern.longueur / 96) * (60 / e.projet.tempo) + 3 }
  })
  expect(r.duree).toBeCloseTo(r.attendue, 1)
  expect(r.crete).toBeGreaterThan(0.05)
  expect(r.crete).toBeLessThanOrEqual(1)
})
