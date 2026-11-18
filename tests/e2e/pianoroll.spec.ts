// Étape 4 : piano roll, vue d'instrument, jeu en direct et enregistrement.
import { expect, type Page, test } from '@playwright/test'

const erreurs: string[] = []
const etat = (page: Page) => page.evaluate(() => (window as any).__studio.etat())
const onglet = (page: Page, titre: string) => page.locator('.dv-tab').filter({ hasText: new RegExp(`^${titre}$`) })

/** Notes du canal sélectionné dans le pattern actif. */
async function notes(page: Page): Promise<{ tic: number; duree: number; hauteur: number; velocite: number }[]> {
  return page.evaluate(() => {
    const e = (window as any).__studio.etat()
    const pattern = e.projet.patterns.find((p: any) => p.id === e.projet.patternActif)
    return pattern.notes[e.canalSelectionne] ?? []
  })
}

const ecran = (page: Page, tic: number, hauteur: number) =>
  page.evaluate(([t, h]) => (window as any).__pianoRoll.versEcran(t, h), [tic, hauteur] as const) as Promise<{ x: number; y: number }>

async function voixCanalSelectionne(page: Page, ms: number): Promise<number> {
  let max = 0
  for (const fin = Date.now() + ms; Date.now() < fin; await page.waitForTimeout(15)) {
    max = Math.max(max, await page.evaluate(() => {
      const s = (window as any).__studio
      return s.audio.moteur.voix(s.canalMidi(s.etat().canalSelectionne))
    }))
  }
  return max
}

/** Projet vide + un piano, sélectionné ; piano roll au premier plan. */
async function pianoVide(page: Page) {
  await page.getByTestId('nouveau-projet').click()
  await page.locator('summary', { hasText: /^Pianos$/ }).click()
  await page.getByRole('button', { name: '＋ Piano à queue', exact: true }).click()
  await onglet(page, 'Piano roll').click()
  await expect(page.getByTestId('piano-roll')).toBeVisible()
  await expect(page.getByTestId('nb-notes')).toHaveText('0')
}

test.beforeEach(async ({ page }) => {
  erreurs.length = 0
  page.on('pageerror', (e) => erreurs.push(e.message))
  page.on('console', (m) => m.type() === 'error' && erreurs.push(m.text()))
  await page.goto('./')
  await expect(page.getByTestId('statut')).toHaveText('Prêt', { timeout: 30_000 })
})

test.afterEach(() => {
  expect(erreurs).toEqual([])
})

test('le piano roll montre les notes de l’instrument sélectionné', async ({ page }) => {
  await expect(page.getByTestId('nb-notes')).toHaveText('48') // piano de la démo
  await page.getByTestId('canal').nth(1).getByTestId('nom-canal').click()
  await expect(page.getByTestId('nb-notes')).toHaveText('12') // basse
  await page.screenshot({ path: 'test-results/pianoroll-demo.png' })
})

test('ajouter, déplacer, redimensionner, effacer une note', async ({ page }) => {
  await pianoVide(page)
  // clic dans le vide : une croche (48 tics), aimantée au pas
  let p = await ecran(page, 100, 60)
  await page.mouse.click(p.x, p.y)
  expect(await notes(page)).toEqual([{ tic: 96, duree: 48, hauteur: 60, velocite: 100 }])
  // glisser la note : deux temps plus loin, un ton plus haut
  p = await ecran(page, 110, 60)
  const q = await ecran(page, 110 + 192, 62)
  await page.mouse.move(p.x, p.y)
  await page.mouse.down()
  await page.mouse.move(q.x, q.y, { steps: 10 })
  await page.mouse.up()
  expect(await notes(page)).toEqual([{ tic: 288, duree: 48, hauteur: 62, velocite: 100 }])
  // glisser le bord droit : deux pas de plus
  const bord = await ecran(page, 288 + 48, 62)
  const plusLoin = await ecran(page, 288 + 48 + 48, 62)
  await page.mouse.move(bord.x - 2, bord.y)
  await page.mouse.down()
  await page.mouse.move(plusLoin.x - 2, plusLoin.y, { steps: 6 })
  await page.mouse.up()
  expect((await notes(page))[0].duree).toBe(96)
  // la note suivante reprend cette durée
  p = await ecran(page, 0, 64)
  await page.mouse.click(p.x + 3, p.y)
  expect((await notes(page))[1]).toEqual({ tic: 0, duree: 96, hauteur: 64, velocite: 100 })
  // clic droit : effacer
  await page.mouse.click(p.x + 3, p.y, { button: 'right' })
  expect(await notes(page)).toHaveLength(1)
})

test('lasso, Suppr, flèches, quantification, vélocité', async ({ page }) => {
  await pianoVide(page)
  for (const [tic, h] of [[0, 60], [96, 64], [192, 67], [288, 72]] as const) {
    const p = await ecran(page, tic + 2, h)
    await page.mouse.click(p.x, p.y)
  }
  expect(await notes(page)).toHaveLength(4)
  // lasso sur les deux premières
  const a = await ecran(page, 0, 66)
  const b = await ecran(page, 150, 59)
  await page.keyboard.down('Control')
  await page.mouse.move(a.x + 1, a.y)
  await page.mouse.down()
  await page.mouse.move(b.x, b.y, { steps: 8 })
  await page.mouse.up()
  await page.keyboard.up('Control')
  await expect(page.getByTestId('nb-selection')).toHaveText('2')
  // flèche haut : un demi-ton ; Maj+flèche : une octave ; la flèche ne change pas l'octave du clavier
  const base = (await etat(page)).clavier.base
  await page.keyboard.press('ArrowUp')
  await page.keyboard.press('Shift+ArrowDown')
  expect((await notes(page)).map((n) => n.hauteur)).toEqual([49, 53, 67, 72])
  expect((await etat(page)).clavier.base).toBe(base)
  await page.keyboard.press('Delete')
  expect((await notes(page)).map((n) => n.hauteur)).toEqual([67, 72])
  await expect(page.getByTestId('nb-notes')).toHaveText('2')

  // sans aimantation, une note décalée ; Quantifier la ramène sur le pas
  await page.getByTestId('grille').selectOption('0')
  const c = await ecran(page, 30, 74)
  await page.mouse.click(c.x, c.y)
  expect((await notes(page))[2].tic).toBe(30)
  await page.getByTestId('grille').selectOption('24')
  await page.getByTestId('quantifier').click() // la note ajoutée est sélectionnée
  expect((await notes(page))[2].tic).toBe(24)

  // couloir de vélocité : glisser vers le bas
  const v = await page.evaluate(() => (window as any).__pianoRoll.velocite(192, 100))
  await page.mouse.move(v.x, v.y)
  await page.mouse.down()
  await page.mouse.move(v.x, v.y + 25, { steps: 4 })
  await page.mouse.up()
  const apres = (await notes(page)).find((n) => n.tic === 192)!
  expect(apres.velocite).toBeLessThan(60)
})

test('vue d’instrument : clavier qui s’allume, souris, clavier de l’ordinateur, octave', async ({ page }) => {
  await onglet(page, 'Instrument').click()
  await expect(page.getByTestId('clavier')).toBeVisible()
  // les lettres de la rangée du haut commencent au do central
  await expect(page.locator('[data-midi="60"]')).toContainText('A')
  // clavier de l'ordinateur : Q physique (« A » en AZERTY) joue le do central sur le piano
  await page.locator('body').click({ position: { x: 5, y: 5 } })
  await page.keyboard.down('KeyQ')
  await expect(page.locator('[data-midi="60"][data-active]')).toBeVisible()
  expect(await voixCanalSelectionne(page, 200)).toBeGreaterThan(0)
  await page.keyboard.up('KeyQ')
  await expect(page.locator('[data-midi="60"][data-active]')).toHaveCount(0)
  // → : une octave plus haut
  await page.keyboard.press('ArrowRight')
  await expect(page.locator('[data-midi="72"]')).toContainText('A')
  await page.keyboard.press('ArrowLeft')
  // souris sur une touche blanche
  const touche = page.locator('[data-midi="64"]')
  const r = (await touche.boundingBox())!
  await page.mouse.move(r.x + r.width / 2, r.y + r.height * 0.9)
  await page.mouse.down()
  await expect(page.locator('[data-midi="64"][data-active]')).toBeVisible()
  await page.mouse.up()
  await expect(page.locator('[data-midi="64"][data-active]')).toHaveCount(0)
  // pendant la lecture, les touches du piano de la démo s'allument
  await page.getByTestId('lecture').click()
  await expect(page.locator('[data-testid="clavier"] [data-active]').first()).toBeVisible({ timeout: 3000 })
})

test('pads de batterie : s’allument pendant la lecture et se jouent', async ({ page }) => {
  await page.getByTestId('canal').nth(3).getByTestId('nom-canal').click() // batterie de la démo
  await onglet(page, 'Instrument').click()
  await expect(page.getByRole('button', { name: 'Grosse caisse', exact: true })).toBeVisible()
  await page.getByTestId('lecture').click()
  await expect(page.locator('[data-pad="42"][data-active]')).toBeVisible({ timeout: 3000 }) // charleston
  await page.getByTestId('stop').click()
  const pad = page.getByRole('button', { name: 'Caisse claire', exact: true })
  await pad.scrollIntoViewIfNeeded() // le panneau est bas : le pad peut être sous le bord
  const r = (await pad.boundingBox())!
  await page.mouse.move(r.x + 10, r.y + 10)
  await page.mouse.down()
  await expect(page.locator('[data-pad="38"][data-active]')).toBeVisible()
  expect(await voixCanalSelectionne(page, 200)).toBeGreaterThan(0)
  await page.mouse.up()
})

test('enregistrement : les notes jouées pendant la lecture s’écrivent dans le pattern', async ({ page }) => {
  await pianoVide(page)
  await page.getByTestId('enregistrer').click()
  await expect(page.getByTestId('enregistrer')).toHaveAttribute('aria-pressed', 'true')
  await page.locator('body').click({ position: { x: 5, y: 5 } })
  await page.keyboard.press('Space')
  await page.waitForTimeout(400)
  await page.keyboard.down('KeyQ')
  await page.waitForTimeout(250)
  await page.keyboard.up('KeyQ')
  await page.keyboard.down('KeyE')
  await page.waitForTimeout(150)
  await page.keyboard.up('KeyE')
  const enregistrees = await notes(page)
  expect(enregistrees.map((n) => n.hauteur)).toEqual([60, 64])
  expect(enregistrees[0].tic).toBeGreaterThanOrEqual(0)
  expect(enregistrees[0].tic).toBeLessThan(384)
  expect(enregistrees[0].duree).toBeGreaterThan(20) // ~0,25 s à 120 = 48 tics
  expect(enregistrees[1].tic).toBeGreaterThan(enregistrees[0].tic)
  // sans enregistrement, jouer n'écrit rien
  await page.getByTestId('enregistrer').click()
  await page.keyboard.press('KeyT')
  expect(await notes(page)).toHaveLength(2)
})
