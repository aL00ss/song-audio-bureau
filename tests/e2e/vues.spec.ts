// Étape 7 : vues dédiées (manche, cordes frottées, harpe, pistons des cuivres).
import { expect, type Page, test } from '@playwright/test'

const erreurs: string[] = []
const onglet = (page: Page, titre: string) => page.locator('.dv-tab').filter({ hasText: new RegExp(`^${titre}$`) })

/** Ajoute un instrument depuis le navigateur ; il devient sélectionné. */
async function ajouter(page: Page, famille: string, instrument: string) {
  await page.locator('summary', { hasText: new RegExp(`^${famille}$`) }).click()
  await page.getByRole('button', { name: `＋ ${instrument}`, exact: true }).click()
}

/** Remplace les notes du canal sélectionné. */
const notes = (page: Page, liste: { tic: number; duree: number; hauteur: number; velocite: number }[]) =>
  page.evaluate((l) => {
    const e = (window as any).__studio.etat()
    e.remplacerNotes(e.canalSelectionne, l)
  }, liste)

async function voixCanalSelectionne(page: Page, ms: number): Promise<number> {
  return page.evaluate(async (duree) => {
    const s = (window as any).__studio
    const canal = s.canalMidi(s.etat().canalSelectionne)
    let max = 0
    const fin = performance.now() + duree
    while (performance.now() < fin) {
      max = Math.max(max, s.audio.moteur.voix(canal))
      await new Promise((r) => setTimeout(r, 10))
    }
    return max
  }, ms)
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

test('guitare : un accord de do s’allume sur cinq cordes différentes, en bas du manche', async ({ page }) => {
  await ajouter(page, 'Guitares', 'Guitare classique')
  await notes(page, [48, 52, 55, 60, 64].map((hauteur) => ({ tic: 0, duree: 384, hauteur, velocite: 100 })))
  await onglet(page, 'Instrument').click()
  await expect(page.getByTestId('manche')).toBeVisible()
  await page.getByTestId('lecture').click()
  const points = page.locator('[data-testid="manche"] circle[data-corde]')
  await expect(points).toHaveCount(5)
  const positions = await points.evaluateAll((els) => els.map((e) => [Number(e.getAttribute('data-corde')), Number(e.getAttribute('data-case'))]))
  expect(new Set(positions.map(([corde]) => corde)).size).toBe(5)
  const appuyees = positions.map(([, c]) => c).filter((c) => c > 0)
  expect(Math.max(...appuyees) - Math.min(...appuyees)).toBeLessThanOrEqual(4)
  expect(Math.max(...appuyees)).toBeLessThanOrEqual(5)
  await page.screenshot({ path: 'test-results/vue-guitare.png' })
})

test('guitare : cliquer sur une case joue sa note', async ({ page }) => {
  await ajouter(page, 'Guitares', 'Guitare classique')
  await onglet(page, 'Instrument').click()
  const manche = (await page.getByTestId('manche').boundingBox())!
  // corde de la (2e en partant du bas), vers la 3e case
  const yLa = manche.y + manche.height * ((20 + 4 * 26) / (40 + 6 * 26))
  await page.mouse.move(manche.x + manche.width * 0.17, yLa)
  await page.mouse.down()
  expect(await voixCanalSelectionne(page, 250)).toBeGreaterThan(0)
  await expect(page.locator('[data-testid="manche"] circle[data-corde="1"]')).toHaveCount(1)
  await page.mouse.up()
})

test('basse de la démo : manche à quatre cordes, allumé pendant la lecture', async ({ page }) => {
  await page.getByTestId('charger-demo').click()
  await page.getByTestId('canal').nth(1).getByTestId('nom-canal').click()
  await onglet(page, 'Instrument').click()
  await expect(page.getByTestId('manche')).toBeVisible()
  await expect(page.locator('[data-testid="manche"] line')).toHaveCount(4)
  await page.getByTestId('lecture').click()
  await expect(page.locator('[data-testid="manche"] circle[data-corde]').first()).toBeVisible({ timeout: 3000 })
})

test('violon : touche sans frettes', async ({ page }) => {
  await ajouter(page, 'Cordes', 'Violon')
  await onglet(page, 'Instrument').click()
  await expect(page.getByText('touche sans frettes')).toBeVisible()
  await expect(page.locator('[data-testid="manche"] line')).toHaveCount(4)
})

test('harpe : une corde touchée sonne et s’allume', async ({ page }) => {
  await ajouter(page, 'Cordes', 'Harpe')
  await onglet(page, 'Instrument').click()
  const harpe = (await page.getByTestId('harpe').boundingBox())!
  await page.mouse.move(harpe.x + harpe.width * 0.5, harpe.y + harpe.height * 0.8)
  await page.mouse.down()
  await expect(page.locator('[data-corde-harpe][data-active]')).toHaveCount(1)
  expect(await voixCanalSelectionne(page, 250)).toBeGreaterThan(0)
  await page.mouse.up()
})

test('trompette : les pistons du do (1 et 3), joué au clavier de l’ordinateur', async ({ page }) => {
  await ajouter(page, 'Cuivres', 'Trompette')
  await onglet(page, 'Instrument').click()
  await expect(page.getByTestId('pistons')).toBeVisible()
  await page.locator('body').click({ position: { x: 5, y: 5 } })
  await page.keyboard.down('KeyQ') // do central
  await expect(page.locator('[data-piston][data-enfonce]')).toHaveCount(2)
  await expect(page.locator('[data-piston="1"][data-enfonce]')).toHaveCount(1)
  await expect(page.locator('[data-piston="3"][data-enfonce]')).toHaveCount(1)
  await expect(page.getByTestId('note-cuivre')).toHaveText('Do3')
  await page.keyboard.up('KeyQ')
  await expect(page.locator('[data-piston][data-enfonce]')).toHaveCount(0)
})
