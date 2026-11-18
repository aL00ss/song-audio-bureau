// Étape 3 : le séquenceur pas à pas du Channel Rack.
import { expect, type Locator, type Page, test } from '@playwright/test'

const erreurs: string[] = []
const etat = (page: Page) => page.evaluate(() => (window as any).__studio.etat())
const ligne = (page: Page, nom: string) => page.getByTestId('canal').filter({ has: page.getByTestId('nom-canal').getByText(nom, { exact: true }) })
const pas = (page: Page, nom: string, n: number) => page.getByRole('button', { name: `Pas ${n} ${nom}`, exact: true })
const actifs = (page: Page, nom: string) =>
  ligne(page, nom).locator('[data-pas][aria-pressed="true"]').evaluateAll((els) => els.map((e) => Number(e.getAttribute('data-pas')) + 1))

/** Boîte d'un élément après l'avoir fait défiler dans la vue (le rack peut être petit). */
async function boite(loc: Locator) {
  await loc.scrollIntoViewIfNeeded()
  return (await loc.boundingBox())!
}

/** Voix maximales sur le canal MIDI d'un canal du projet pendant `ms`. */
async function voixMax(page: Page, nom: string, ms: number): Promise<number> {
  const canal = await page.evaluate((n) => {
    const s = (window as any).__studio
    return s.canalMidi(s.etat().projet.canaux.find((c: any) => c.nom === n).id)
  }, nom)
  let max = 0
  for (const fin = Date.now() + ms; Date.now() < fin; await page.waitForTimeout(15)) {
    max = Math.max(max, await page.evaluate((c) => (window as any).__studio.audio.moteur.voix(c), canal))
  }
  return max
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

test('un nouveau projet : quatre sons de batterie, seize pas éteints', async ({ page }) => {
  await expect(page.getByTestId('canal')).toHaveCount(4)
  for (const nom of ['Grosse caisse', 'Clap', 'Charleston', 'Caisse claire']) {
    await expect(ligne(page, nom)).toHaveAttribute('data-mode', 'pas')
    await expect(ligne(page, nom).locator('[data-pas]')).toHaveCount(16)
    expect(await actifs(page, nom)).toEqual([])
  }
  await expect(page.getByLabel('Son de Charleston')).toHaveValue('42')
})

test('clic, clavier, peinture et clic droit', async ({ page }) => {
  for (const n of [1, 5, 9, 13]) await pas(page, 'Grosse caisse', n).click()
  expect(await actifs(page, 'Grosse caisse')).toEqual([1, 5, 9, 13])
  await pas(page, 'Grosse caisse', 5).click()
  expect(await actifs(page, 'Grosse caisse')).toEqual([1, 9, 13])
  // clavier : Entrée sur un pas
  await pas(page, 'Grosse caisse', 3).focus()
  await page.keyboard.press('Enter')
  expect(await actifs(page, 'Grosse caisse')).toEqual([1, 3, 9, 13])

  // peinture : glisser du pas 1 au pas 8 du charleston
  const debut = await boite(pas(page, 'Charleston', 1))
  const fin = await boite(pas(page, 'Charleston', 8))
  await page.mouse.move(debut.x + 5, debut.y + 5)
  await page.mouse.down()
  await page.mouse.move(fin.x + 5, fin.y + 5, { steps: 12 })
  await page.mouse.up()
  expect(await actifs(page, 'Charleston')).toEqual([1, 2, 3, 4, 5, 6, 7, 8])
  // clic droit glissé : efface
  const a = await boite(pas(page, 'Charleston', 3))
  const b = await boite(pas(page, 'Charleston', 5))
  await page.mouse.move(a.x + 5, a.y + 5)
  await page.mouse.down({ button: 'right' })
  await page.mouse.move(b.x + 5, b.y + 5, { steps: 8 })
  await page.mouse.up({ button: 'right' })
  expect(await actifs(page, 'Charleston')).toEqual([1, 2, 6, 7, 8])

  const projet = (await etat(page)).projet
  const grosseCaisse = projet.canaux.find((c: any) => c.nom === 'Grosse caisse')
  expect(projet.patterns[0].notes[grosseCaisse.id].map((n: any) => [n.tic, n.hauteur])).toEqual([[0, 36], [48, 36], [192, 36], [288, 36]])
})

test('les pas sonnent, et un pas ajouté pendant la lecture est joué au tour suivant', async ({ page }) => {
  for (const n of [1, 5, 9, 13]) await pas(page, 'Grosse caisse', n).click()
  await page.getByTestId('lecture').click()
  expect(await voixMax(page, 'Grosse caisse', 1200)).toBeGreaterThan(0)
  expect(await voixMax(page, 'Clap', 300)).toBe(0)
  // la tête de lecture parcourt les pas
  await expect(page.locator('[data-pas].outline').first()).toBeVisible()
  // un clap ajouté en pleine lecture : entendu dans les 2 s (une boucle de 16 pas à 120)
  await pas(page, 'Clap', 1).click()
  expect(await voixMax(page, 'Clap', 2500)).toBeGreaterThan(0)
})

test('son de batterie, longueur du pattern, swing', async ({ page }) => {
  await pas(page, 'Clap', 2).click()
  await page.getByLabel('Son de Clap').selectOption('56') // cloche à vache
  let projet = (await etat(page)).projet
  const clap = projet.canaux.find((c: any) => c.nom === 'Clap')
  expect(clap.noteDefaut).toBe(56)
  expect(projet.patterns[0].notes[clap.id].map((n: any) => n.hauteur)).toEqual([56])
  expect(await actifs(page, 'Clap')).toEqual([2])

  await page.getByTestId('nombre-pas').fill('32')
  await expect(ligne(page, 'Clap').locator('[data-pas]')).toHaveCount(32)
  expect((await etat(page)).projet.patterns[0].longueur).toBe(32 * 24)

  const swing = page.getByRole('slider', { name: 'Swing' })
  await swing.focus()
  await swing.press('ArrowUp')
  await swing.press('ArrowUp')
  projet = (await etat(page)).projet
  expect(projet.swing).toBeCloseTo(0.02, 5)
})

test('éditeur de vélocité du canal sélectionné', async ({ page }) => {
  await pas(page, 'Caisse claire', 5).click()
  await ligne(page, 'Caisse claire').getByTestId('nom-canal').click()
  const curseur = page.getByRole('slider', { name: 'Vélocité du pas 5', exact: true })
  await expect(curseur).toHaveAttribute('aria-valuenow', '100')
  // chaque barre de vélocité est exactement sous son pas
  for (const n of [1, 5, 16]) {
    const p = await pas(page, 'Caisse claire', n).boundingBox()
    const v = await page.getByRole('slider', { name: `Vélocité du pas ${n}`, exact: true }).boundingBox()
    expect(Math.abs(p!.x - v!.x), `pas ${n}`).toBeLessThan(1)
  }
  await curseur.focus()
  await curseur.press('ArrowDown')
  await curseur.press('ArrowDown')
  await expect(curseur).toHaveAttribute('aria-valuenow', '92')
  // glisser en haut de la colonne : vélocité maximale
  const cadre = await boite(curseur)
  await page.mouse.move(cadre.x + 5, cadre.y + 1)
  await page.mouse.down()
  await page.mouse.up()
  await expect(curseur).toHaveAttribute('aria-valuenow', /^12[0-7]$/)
  await page.screenshot({ path: 'test-results/sequenceur.png' })
})

test('un morceau importé s’affiche en aperçu, pas en pas', async ({ page }) => {
  await page.getByTestId('charger-demo').click()
  await expect(page.getByTestId('canal')).toHaveCount(4)
  await expect(page.getByTestId('canal').first()).toHaveAttribute('data-mode', 'apercu')
  await expect(page.getByTestId('apercu')).toHaveCount(4)
})

test('en 1280 × 720 : barre du haut sur une ligne, les quatre lignes du kit visibles dans le rack', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 })
  await page.getByTestId('reinitialiser-disposition').click()
  const barre = (await page.locator('header').first().boundingBox())!
  expect(barre.height, 'hauteur de la barre du haut').toBeLessThan(56)
  const rack = (await page.locator('.dv-tab', { hasText: /^Channel Rack$/ }).boundingBox())!
  const mixer = (await page.locator('.dv-tab', { hasText: /^Mixer$/ }).boundingBox())!
  for (const nom of ['Grosse caisse', 'Clap', 'Charleston', 'Caisse claire']) {
    const ligneBoite = (await ligne(page, nom).boundingBox())!
    expect(ligneBoite.y, nom).toBeGreaterThan(rack.y)
    expect(ligneBoite.y + ligneBoite.height, nom).toBeLessThan(mixer.y)
  }
})
