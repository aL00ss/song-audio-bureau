// Étape 8 : fichiers (projet, MIDI, WAV), sauvegarde automatique, jouer avec l'orchestre.
import { readFile } from 'node:fs/promises'
import { expect, type Page, test } from '@playwright/test'

const erreurs: string[] = []

const etat = <T,>(page: Page, f: (e: any) => T) => page.evaluate(f as any, null) as Promise<T>

async function menu(page: Page, entree: string) {
  await page.getByTestId('menu-fichier').click()
  await page.getByRole('menuitem', { name: entree }).click()
}

test.beforeEach(async ({ page }) => {
  erreurs.length = 0
  page.on('pageerror', (e) => erreurs.push(e.message))
  page.on('console', (m) => m.type() === 'error' && erreurs.push(m.text()))
  await page.goto('./')
  await expect(page.getByTestId('statut')).toHaveText('Prêt', { timeout: 30_000 })
  await page.getByTestId('nouveau-projet').click()
})

test.afterEach(() => {
  expect(erreurs).toEqual([])
})

test('enregistrer puis rouvrir un projet', async ({ page }) => {
  await page.getByTestId('tempo').fill('97')
  const telechargement = page.waitForEvent('download')
  await menu(page, 'Enregistrer le projet')
  const fichier = await telechargement
  expect(fichier.suggestedFilename()).toBe('Nouveau projet.song-audio.json')
  const chemin = await fichier.path()
  expect(JSON.parse(await readFile(chemin, 'utf-8')).tempo).toBe(97)

  await page.getByTestId('tempo').fill('140')
  await page.getByTestId('ouvrir-projet').setInputFiles(chemin)
  await expect(page.getByTestId('tempo')).toHaveValue('97')

  // un fichier qui n'est pas un projet est refusé, avec un message
  await page.getByTestId('ouvrir-projet').setInputFiles({ name: 'faux.json', mimeType: 'application/json', buffer: Buffer.from('{"a":1}') })
  await expect(page.getByTestId('message')).toContainText('Projet illisible')
  await expect(page.getByTestId('tempo')).toHaveValue('97')
})

test('exporter en MIDI', async ({ page }) => {
  await page.getByTestId('charger-demo').click()
  const telechargement = page.waitForEvent('download')
  await menu(page, 'Exporter en MIDI')
  const fichier = await telechargement
  expect(fichier.suggestedFilename()).toMatch(/\.mid$/)
  const octets = await readFile(await fichier.path())
  expect(octets.subarray(0, 4).toString('latin1')).toBe('MThd')
})

test('exporter en WAV : un rendu non silencieux de la bonne durée', async ({ page }) => {
  await page.getByTestId('charger-demo').click()
  await page.getByTestId('mode-pattern').click()
  const resultat = await page.evaluate(async () => {
    const s = (window as any).__studio
    const { projet } = s.etat()
    const wav: Blob = await s.etat().exporterWav()
    const tampon = await new AudioContext().decodeAudioData(await wav.arrayBuffer())
    let crete = 0
    for (let c = 0; c < tampon.numberOfChannels; c++) for (const v of tampon.getChannelData(c)) crete = Math.max(crete, Math.abs(v))
    const longueur = Math.max(...projet.patterns.filter((p: any) => p.id === projet.patternActif).map((p: any) => p.longueur))
    return { duree: tampon.duration, canaux: tampon.numberOfChannels, crete, attendue: (longueur / 96) * (60 / projet.tempo) + 3 }
  })
  expect(resultat.canaux).toBe(2)
  expect(resultat.duree).toBeCloseTo(resultat.attendue, 1)
  expect(resultat.crete).toBeGreaterThan(0.05)
  expect(resultat.crete).toBeLessThanOrEqual(1)
})

test('le projet est gardé dans le navigateur et revient au rechargement', async ({ page }) => {
  await page.getByTestId('tempo').fill('111')
  await page.waitForTimeout(1500)
  await page.reload()
  await expect(page.getByTestId('statut')).toHaveText('Prêt', { timeout: 30_000 })
  await expect(page.getByTestId('tempo')).toHaveValue('111')
})

test('jouer avec l’orchestre : la partie se tait et les notes justes sont comptées', async ({ page }) => {
  // grosse caisse sur chaque pas : toute note 36 jouée pendant la lecture est juste
  await etat(page, () => {
    const e = (window as any).__studio.etat()
    const id = e.projet.canaux[0].id
    e.selectionnerCanal(id)
    e.remplacerNotes(id, Array.from({ length: 16 }, (_, i) => ({ tic: i * 24, duree: 24, hauteur: 36, velocite: 100 })))
  })
  await page.locator('.dv-tab').filter({ hasText: /^Instrument$/ }).click()
  await page.getByTestId('jouer-partie').click()
  await expect(page.getByTestId('jouer-partie')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByTestId('lecture')).toHaveText('Pause')
  expect(await etat(page, () => (window as any).__studio.etat().projet.canaux[0].muet)).toBe(true)

  await etat(page, () => {
    const e = (window as any).__studio.etat()
    const id = e.projet.canaux[0].id
    e.jouerNote(36, 100, id)
    e.relacherNote(36, id)
    e.jouerNote(37, 100, id)
    e.relacherNote(37, id)
  })
  await expect(page.getByTestId('score-partie')).toContainText('1 justes')
  await expect(page.getByTestId('score-partie')).toContainText('1 fausses')

  await page.getByTestId('jouer-partie').click()
  await expect(page.getByTestId('score-partie')).toHaveCount(0)
  expect(await etat(page, () => (window as any).__studio.etat().projet.canaux[0].muet)).toBe(false)
})
