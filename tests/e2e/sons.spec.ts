// Étape 9 : banques de sons supplémentaires, chargées à la demande.
import { expect, type Page, test } from '@playwright/test'
import { rendreNoteTenue } from './rendu.js'

const erreurs: string[] = []

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

/** Crête du niveau général pendant `ms` après une note du canal sélectionné. */
async function crete(page: Page, velocite: number, ms = 600): Promise<number> {
  return page.evaluate(
    async ([v, duree]) => {
      const s = (window as any).__studio
      const e = s.etat()
      e.jouerNote(60, v)
      let max = 0
      const fin = performance.now() + duree
      while (performance.now() < fin) {
        max = Math.max(max, s.audio.moteur.niveau())
        await new Promise((r) => setTimeout(r, 10))
      }
      e.relacherNote(60)
      await new Promise((r) => setTimeout(r, 1500))
      return max
    },
    [velocite, ms] as const,
  )
}

test('le piano Salamander se charge quand un piano est utilisé, et passe devant General MIDI', async ({ page }) => {
  // un projet de batteries seulement, repris au rechargement : rien à télécharger
  await page.getByTestId('nouveau-projet').click()
  await page.waitForTimeout(1500)
  await page.reload()
  await expect(page.getByTestId('statut')).toHaveText('Prêt', { timeout: 30_000 })
  await page.waitForTimeout(300)
  expect(await page.evaluate(() => (window as any).__studio.etat().banques)).toEqual({})

  await page.locator('summary', { hasText: /^Pianos$/ }).click()
  await page.getByRole('button', { name: '＋ Piano à queue', exact: true }).click()
  await expect.poll(() => page.evaluate(() => (window as any).__studio.etat().banques.piano?.phase), { timeout: 60_000 }).toBe('prete')
  const banques = await page.evaluate(() => (window as any).__studio.audio.moteur.synth.soundBankManager.priorityOrder)
  expect(banques).toEqual(['piano', 'gm'])

  await page.getByTestId('lecture').click() // le son démarre au premier geste
  await page.getByTestId('lecture').click()
  const doux = await crete(page, 30)
  const fort = await crete(page, 120)
  console.log(`piano : doux ${doux.toFixed(4)}, fort ${fort.toFixed(4)}, rapport ${(fort / doux).toFixed(2)}`)
  expect(doux).toBeGreaterThan(0.002)
  expect(fort / doux).toBeGreaterThan(2)
  expect(fort / doux).toBeLessThan(20)
})

for (const [programme, nom, hauteur, banque] of [
  [40, 'violon', 67, 'cordes'],
  [42, 'violoncelle', 48, 'cordes'],
  [48, 'ensemble de cordes', 60, 'cordes'],
  [56, 'trompette', 65, 'cuivres'],
  [60, 'cor', 53, 'cuivres'],
  [71, 'clarinette', 62, 'bois'],
  [73, 'flûte', 72, 'bois'],
] as const) {
  test(`${nom} (VSCO 2) : juste, et la note tient grâce à la boucle`, async ({ page }) => {
    const r = await rendreNoteTenue(page, programme, hauteur)
    console.log(`${nom} : note ${r.note.toFixed(2)} (attendue ${hauteur}), niveau ${r.debut.toFixed(4)} -> ${r.fin.toFixed(4)}`)
    const ordre = await page.evaluate(() => (window as any).__studio.audio.moteur.synth.soundBankManager.priorityOrder)
    expect(ordre).toContain(banque)
    expect(ordre.at(-1)).toBe('gm')
    // l'autocorrélation peut tomber sur l'octave : on compare la classe de hauteur
    const ecart = ((((r.note - hauteur) % 12) + 18) % 12) - 6
    expect(Math.abs(ecart)).toBeLessThan(0.35)
    expect(r.debut).toBeGreaterThan(0.005)
    expect(r.fin).toBeGreaterThan(r.debut * 0.25)
  })
}
