// Étape 9 : 16 instruments jouent ensemble, tous les panneaux ouverts, sans que l'interface rame.
import { expect, test } from '@playwright/test'

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

test('16 instruments : chacun sonne, et l’interface garde un affichage fluide', async ({ page }) => {
  test.setTimeout(120_000)
  // 15 instruments mélodiques variés + la batterie du projet vide, une note par pas chacun
  await page.evaluate(() => {
    const s = (window as any).__studio
    s.etat().nouveauProjet()
    const programmes = [0, 4, 11, 19, 24, 33, 40, 42, 48, 56, 60, 68, 71, 73, 80]
    for (const p of programmes) s.etat().ajouterInstrument(p)
    const e = s.etat()
    e.projet.canaux.forEach((c: any, i: number) => {
      if (c.batterie) return
      const base = 36 + (i % 4) * 12
      e.remplacerNotes(c.id, Array.from({ length: 16 }, (_, k) => ({ tic: k * 24, duree: 24, hauteur: base + ((k * 7) % 12), velocite: 90 })))
    })
    for (const c of e.projet.canaux.filter((c: any) => c.batterie)) for (let k = 0; k < 16; k += 2) s.etat().definirPas(c.id, k, true)
  })
  expect(await page.evaluate(() => (window as any).__studio.etat().projet.canaux.length)).toBe(19)
  // les banques demandées sont prêtes avant de mesurer
  await expect
    .poll(() => page.evaluate(() => Object.values((window as any).__studio.etat().banques).every((b: any) => b.phase === 'prete')), { timeout: 60_000 })
    .toBe(true)

  await page.getByTestId('lecture').click()
  const mesure = await page.evaluate(async () => {
    const s = (window as any).__studio
    const images: number[] = []
    let precedente = performance.now()
    const fin = precedente + 4000
    const sonnent = new Set<number>()
    await new Promise<void>((fini) => {
      const image = (t: number) => {
        images.push(t - precedente)
        precedente = t
        s.voixParCanal().forEach((v: number, i: number) => v > 0 && sonnent.add(i))
        if (t < fin) requestAnimationFrame(image)
        else fini()
      }
      requestAnimationFrame(image)
    })
    images.shift()
    images.sort((a, b) => a - b)
    return {
      ips: 1000 / (images.reduce((a, b) => a + b, 0) / images.length),
      pire: images.at(-1)!,
      p95: images[Math.floor(images.length * 0.95)],
      canauxSonores: sonnent.size,
      canaux: s.etat().projet.canaux.length,
    }
  })
  console.log(`16 instruments : ${mesure.ips.toFixed(0)} images/s, 95 % sous ${mesure.p95.toFixed(1)} ms, pire ${mesure.pire.toFixed(1)} ms, ${mesure.canauxSonores}/${mesure.canaux} canaux entendus`)
  expect(mesure.canauxSonores).toBe(mesure.canaux)
  expect(mesure.ips).toBeGreaterThan(30)
  expect(mesure.p95).toBeLessThan(50)
})
