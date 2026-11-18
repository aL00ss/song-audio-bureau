// Phase C : annuler et rétablir toutes les modifications du projet.
import { expect, test } from '@playwright/test'

const erreurs: string[] = []
const etat = (page: import('@playwright/test').Page) => page.evaluate(() => {
  const e = (window as any).__studio.etat()
  return { tempo: e.projet.tempo, canaux: e.projet.canaux.length, peutAnnuler: e.peutAnnuler, peutRetablir: e.peutRetablir }
})

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

test('Ctrl+Z annule et Ctrl+Y rétablit, étape par étape', async ({ page }) => {
  expect(await etat(page)).toMatchObject({ peutAnnuler: false, peutRetablir: false })
  const depart = await etat(page)
  await page.evaluate(() => (window as any).__studio.etat().ajouterInstrument(0))
  await page.waitForTimeout(500) // deux gestes distincts
  await page.getByTestId('tempo').fill('133')
  await page.getByTestId('tempo').blur()
  await expect.poll(() => etat(page)).toMatchObject({ tempo: 133, canaux: depart.canaux + 1, peutAnnuler: true })

  await page.keyboard.press('Control+z')
  await expect.poll(() => etat(page)).toMatchObject({ tempo: depart.tempo, canaux: depart.canaux + 1, peutRetablir: true })
  await page.keyboard.press('Control+z')
  await expect.poll(() => etat(page)).toMatchObject({ tempo: depart.tempo, canaux: depart.canaux, peutAnnuler: false })
  await page.keyboard.press('Control+y')
  await expect.poll(() => etat(page)).toMatchObject({ canaux: depart.canaux + 1 })
  await page.keyboard.press('Control+Shift+z')
  await expect.poll(() => etat(page)).toMatchObject({ tempo: 133, peutRetablir: false })
  await expect(page.getByTestId('tempo')).toHaveValue('133')
})

test('le menu Fichier propose Annuler et Rétablir quand c’est possible', async ({ page }) => {
  await page.getByTestId('menu-fichier').click()
  await expect(page.getByRole('menuitem', { name: /Annuler/ })).toBeDisabled()
  await page.keyboard.press('Escape')
  await page.getByTestId('menu-fichier').click() // referme
  await page.evaluate(() => (window as any).__studio.etat().changerTempo(90))
  await page.getByTestId('menu-fichier').click()
  await page.getByRole('menuitem', { name: /Annuler/ }).click()
  await expect(page.getByTestId('tempo')).not.toHaveValue('90')
  await page.getByTestId('menu-fichier').click()
  await page.getByRole('menuitem', { name: /Rétablir/ }).click()
  await expect(page.getByTestId('tempo')).toHaveValue('90')
})

test('un nouveau projet repart sans historique', async ({ page }) => {
  await page.evaluate(() => (window as any).__studio.etat().changerTempo(90))
  await page.getByTestId('nouveau-projet').click()
  expect(await etat(page)).toMatchObject({ peutAnnuler: false, peutRetablir: false })
})

test('nouveau depuis un modèle : le projet du modèle, qui joue', async ({ page }) => {
  await page.getByTestId('menu-fichier').click()
  await page.getByTestId('menu-modeles').click()
  await page.getByTestId('modele-electro').click()
  await expect(page.getByTestId('tempo')).toHaveValue('128')
  await page.getByTestId('lecture').click()
  await expect.poll(() => page.evaluate(() => (window as any).__studio.voixParCanal().filter((v: number) => v > 0).length), { timeout: 10_000 }).toBeGreaterThan(1)
  await page.getByTestId('stop').click()
})

test('le titre de la fenêtre suit le projet et signale les modifications', async ({ page }) => {
  await page.getByTestId('menu-fichier').click()
  await page.getByTestId('menu-modeles').click()
  await page.getByTestId('modele-hip-hop').click()
  await expect(page).toHaveTitle('Hip-hop — song-audio')
  await page.evaluate(() => (window as any).__studio.etat().changerTempo(95))
  await expect(page).toHaveTitle('• Hip-hop — song-audio')
})
