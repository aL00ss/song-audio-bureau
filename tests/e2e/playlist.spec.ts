// Étape 6 : patterns, playlist et mode SONG.
import { expect, type Page, test } from '@playwright/test'

const erreurs: string[] = []
const etat = (page: Page) => page.evaluate(() => (window as any).__studio.etat())
const clips = (page: Page) => page.locator('[data-clip]')

/** Voix maximales sur le canal MIDI d'un canal du projet, nommé, pendant `ms`. */
async function voix(page: Page, nom: string, ms: number): Promise<number> {
  return page.evaluate(async ([n, duree]) => {
    const s = (window as any).__studio
    const canal = s.canalMidi(s.etat().projet.canaux.find((c: any) => c.nom === n).id)
    let max = 0
    const fin = performance.now() + duree
    while (performance.now() < fin) {
      max = Math.max(max, s.audio.moteur.voix(canal))
      await new Promise((r) => setTimeout(r, 10))
    }
    return max
  }, [nom, ms] as const)
}

/** Point de la zone des pistes : mesure (à partir de 1), piste (à partir de 1). */
async function point(page: Page, mesure: number, piste: number, dansLaMesure = 0.25) {
  const zone = (await page.getByTestId('pistes-playlist').boundingBox())!
  const pxParMesure = 64
  return { x: zone.x + (mesure - 1 + dansLaMesure) * pxParMesure, y: zone.y + (piste - 0.5) * 34 }
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

test('la démo a un pattern posé en mesure 1, piste 1', async ({ page }) => {
  await expect(page.getByTestId('pattern')).toHaveText(['Démo'])
  await expect(clips(page)).toHaveCount(1)
  await expect(clips(page).first()).toHaveAttribute('data-debut', '0')
  await expect(clips(page).first()).toHaveAttribute('data-piste', '0')
  await expect(clips(page).first()).toContainText('Démo')
})

test('arrangement : la caisse claire n’arrive qu’en mesure 2 en mode SONG', async ({ page }) => {
  await page.getByTestId('nouveau-projet').click()
  await expect(clips(page)).toHaveCount(1) // le pattern 1, posé au début
  // pattern 1 : grosse caisse sur les 4 temps
  await page.evaluate(() => {
    const e = (window as any).__studio.etat()
    const kick = e.projet.canaux.find((c: any) => c.nom === 'Grosse caisse').id
    for (const i of [0, 4, 8, 12]) e.definirPas(kick, i, true)
  })
  // pattern 2 (nouveau, devient actif) : caisse claire sur les 4 temps
  await page.getByTestId('nouveau-pattern').click()
  await expect(page.getByTestId('choix-pattern').locator('option:checked')).toHaveText('Pattern 2')
  await page.evaluate(() => {
    const e = (window as any).__studio.etat()
    const caisse = e.projet.canaux.find((c: any) => c.nom === 'Caisse claire').id
    for (const i of [0, 4, 8, 12]) e.definirPas(caisse, i, true)
  })
  // clic dans la playlist : le pattern actif posé en mesure 2, piste 2
  const p = await point(page, 2, 2)
  await page.mouse.click(p.x, p.y)
  await expect(clips(page)).toHaveCount(2)
  await expect(clips(page).nth(1)).toHaveAttribute('data-debut', '384')
  await expect(clips(page).nth(1)).toHaveAttribute('data-piste', '1')

  await page.getByTestId('mode-morceau').click()
  await expect(page.getByTestId('mode-morceau')).toHaveAttribute('aria-pressed', 'true')
  expect(await page.evaluate(() => (window as any).__studio.audio.transport.longueur)).toBe(768)
  await page.getByTestId('lecture').click()
  // mesure 1 (les 2 premières secondes à 120) : grosse caisse, pas de caisse claire
  const [kick1, caisse1] = await Promise.all([voix(page, 'Grosse caisse', 1600), voix(page, 'Caisse claire', 1600)])
  expect(kick1).toBeGreaterThan(0)
  expect(caisse1).toBe(0)
  // mesure 2 : la caisse claire joue
  await page.waitForFunction(() => (window as any).__studio.audio.transport.position >= 400)
  expect(await voix(page, 'Caisse claire', 1200)).toBeGreaterThan(0)

  // en mode PAT, seul le pattern actif (le 2) tourne en boucle
  await page.getByTestId('mode-pattern').click()
  expect((await etat(page)).transport).toBe('arret')
  await page.getByTestId('lecture').click()
  expect(await voix(page, 'Grosse caisse', 1500)).toBe(0)
})

test('déplacer, supprimer, choisir un clip ; règle des mesures', async ({ page }) => {
  const demo = clips(page).first()
  // glisser le clip de la démo vers la mesure 3, piste 2
  const depart = await point(page, 1, 1, 0.5)
  const arrivee = await point(page, 3, 2, 0.5)
  await page.mouse.move(depart.x, depart.y)
  await page.mouse.down()
  await page.mouse.move(arrivee.x, arrivee.y, { steps: 10 })
  await page.mouse.up()
  await expect(demo).toHaveAttribute('data-debut', String(2 * 384))
  await expect(demo).toHaveAttribute('data-piste', '1')
  // un second pattern et un clip, puis double clic sur le clip de la démo : la démo redevient active
  await page.getByTestId('nouveau-pattern').click()
  const p = await point(page, 1, 1)
  await page.mouse.click(p.x, p.y)
  await expect(clips(page)).toHaveCount(2)
  await demo.dblclick()
  await expect(page.getByTestId('choix-pattern').locator('option:checked')).toHaveText('Démo')
  // clic droit : supprimer
  await clips(page).nth(1).click({ button: 'right' })
  await expect(clips(page)).toHaveCount(1)
  // règle : clic sur la mesure 3 -> mode SONG, position au début de la mesure 3
  const regle = (await page.getByTestId('regle').boundingBox())!
  await page.mouse.click(regle.x + 2.2 * 64, regle.y + 5)
  await expect(page.getByTestId('mode-morceau')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByTestId('position')).toHaveText('3:1:00')
  await page.screenshot({ path: 'test-results/playlist.png' })
})

test('renommer et dupliquer un pattern', async ({ page }) => {
  await page.getByTestId('pattern').first().dblclick()
  const champ = page.getByLabel('Nom du pattern')
  await champ.fill('Couplet')
  await champ.press('Enter')
  await expect(page.getByTestId('pattern')).toHaveText(['Couplet'])
  await page.getByTestId('dupliquer-pattern').click()
  await expect(page.getByTestId('pattern')).toHaveText(['Couplet', 'Couplet (copie)'])
  const projet = (await etat(page)).projet
  expect(projet.patternActif).toBe(projet.patterns[1].id)
  expect(projet.patterns[1].notes).toEqual(projet.patterns[0].notes)
  // le piano roll suit le pattern actif : la copie a les mêmes 48 notes de piano
  await expect(page.getByTestId('nb-notes')).toHaveText('48')
})
