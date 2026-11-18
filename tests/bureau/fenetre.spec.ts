// Tests de l'application de bureau : Playwright se branche sur la fenêtre WebView2 de Tauri
// (lancée avec WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222).
import { existsSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
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

test('le transport est tenu par le moteur : la position avance, la pause la fige, la recherche la déplace', async () => {
  await page.evaluate(() => (window as any).__studio.etat().chargerDemo())
  const position = () => page.evaluate(() => (window as any).__studio.audio.transport.positionAbsolue as number)
  await page.getByTestId('lecture').click()
  await page.waitForTimeout(600)
  const a = await position()
  await page.waitForTimeout(500)
  const b = await position()
  // à 100 noires par minute : 0,5 s ≈ 80 tics
  expect(b - a).toBeGreaterThan(55)
  expect(b - a).toBeLessThan(110)
  await page.getByTestId('lecture').click() // pause
  await expect(page.getByTestId('lecture')).toHaveText('Lecture')
  const figee = await position()
  await page.waitForTimeout(400)
  expect(Math.abs((await position()) - figee)).toBeLessThan(2)
  await page.evaluate(() => (window as any).__studio.audio.transport.chercher(96))
  await expect.poll(position).toBeCloseTo(96, 0)
  await page.getByTestId('stop').click()
  await expect.poll(position).toBe(0)
})

test('métronome : le bouton existe sur le bureau et le maître sonne même sans instrument', async () => {
  await page.evaluate(() => {
    const e = (window as any).__studio.etat()
    e.nouveauProjet()
    for (const c of e.projet.canaux) (window as any).__studio.etat().modifierCanal(c.id, { muet: true })
  })
  await page.getByTestId('metronome').click()
  await expect(page.getByTestId('metronome')).toHaveAttribute('aria-pressed', 'true')
  await page.getByTestId('lecture').click()
  // tous les canaux sont muets : ce qui sonne au maître, c'est le métronome
  expect(await crete(0, 1500)).toBeGreaterThan(0.05)
  await page.getByTestId('stop').click()
  await page.getByTestId('metronome').click()
  await page.waitForTimeout(300) // relevés d'avant l'arrêt écoulés
  await crete(0, 100)
  await page.getByTestId('lecture').click()
  expect(await crete(0, 1200)).toBeLessThan(0.0005)
  await page.getByTestId('stop').click()
})

test('fichier de projet : Ctrl+S écrit sur le disque, relecture identique, seuls les projets sont acceptés', async () => {
  const chemin = join(tmpdir(), `essai-${Date.now()}.song-audio.json`)
  await page.evaluate(() => (window as any).__studio.etat().nouveauDepuisModele('hip-hop'))
  await page.evaluate(() => (window as any).__studio.etat().changerTempo(97))
  // comme après « Enregistrer sous » : le projet a désormais un fichier
  await page.evaluate((c) => (window as any).__studio.magasin.setState({ cheminProjet: c }), chemin)
  await expect(page).toHaveTitle(/^• essai-\d+ — song-audio$/)
  await page.keyboard.press('Control+s')
  await expect(page).toHaveTitle(/^essai-\d+ — song-audio$/)
  expect(existsSync(chemin)).toBe(true)
  expect(JSON.parse(readFileSync(chemin, 'utf-8')).tempo).toBe(97)
  const relu = await page.evaluate((c) => (window as any).__TAURI_INTERNALS__.invoke('projet_lire', { chemin: c }), chemin)
  expect(JSON.parse(relu).tempo).toBe(97)
  const refus = await page.evaluate(() => (window as any).__TAURI_INTERNALS__.invoke('projet_ecrire', { chemin: 'C:/Windows/Temp/autre.txt', contenu: 'x' }).then(() => 'écrit', (e: unknown) => String(e)))
  expect(refus).toContain("n'est pas un fichier de projet")
  rmSync(chemin)
})

test('synthé intégré (moteur Rust) : il apparaît dans le navigateur et sonne', async () => {
  await page.evaluate(() => (window as any).__studio.etat().nouveauDepuisModele('vide'))
  await expect(page.getByTestId('synthes-integres')).toBeVisible()
  await page.getByTestId('synthes-integres').getByRole('button', { name: '＋ Lead acide' }).click()
  const r = await page.evaluate(async () => {
    const s = (window as any).__studio
    const e = s.etat()
    const canal = e.projet.canaux.at(-1)
    e.jouerNote(57, 110, canal.id)
    await new Promise((f) => setTimeout(f, 400))
    const voix = s.audio.moteur.voix(s.canalMidi(canal.id))
    e.relacherNote(57, canal.id)
    return { source: canal.source, nom: canal.nom, voix }
  })
  expect(r).toMatchObject({ source: 'soustractif', nom: 'Lead acide' })
  expect(r.voix).toBeGreaterThan(0)
})
