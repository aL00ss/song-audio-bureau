// Étape 5 : le mixer, vérifié par des mesures du son de chaque piste.
import { expect, type Page, test } from '@playwright/test'

const erreurs: string[] = []
const onglet = (page: Page, titre: string) => page.locator('.dv-tab').filter({ hasText: new RegExp(`^${titre}$`) })

/** Crête maximale [gauche, droite] d'une piste (0 = maître) pendant `ms`. */
async function niveau(page: Page, piste: number, ms: number): Promise<[number, number]> {
  return page.evaluate(async ([p, duree]) => {
    const m = (window as any).__studio.audio.moteur
    let g = 0
    let d = 0
    const fin = performance.now() + duree
    while (performance.now() < fin) {
      const n = m.niveauPiste(p)
      g = Math.max(g, n.gauche)
      d = Math.max(d, n.droite)
      await new Promise((r) => setTimeout(r, 15))
    }
    return [g, d] as [number, number]
  }, [piste, ms] as const)
}
const max = (n: [number, number]) => Math.max(...n)
/** Silence : sous -66 dB (le gain d'une piste coupée tend vers 0 sans l'atteindre exactement). */
const SILENCE = 0.0005
const regler = (page: Page, index: number, modifications: object) =>
  page.evaluate(([i, m]) => (window as any).__studio.etat().modifierPiste(i, m), [index, modifications] as const)

test.beforeEach(async ({ page }) => {
  erreurs.length = 0
  page.on('pageerror', (e) => erreurs.push(e.message))
  page.on('console', (m) => m.type() === 'error' && erreurs.push(m.text()))
  await page.goto('./')
  await expect(page.getByTestId('statut')).toHaveText('Prêt', { timeout: 30_000 })
  await onglet(page, 'Mixer').click()
})

test.afterEach(() => {
  expect(erreurs).toEqual([])
})

test('le maître et une tranche par instrument ; chaque piste sonne pendant la lecture', async ({ page }) => {
  for (const i of [0, 1, 2, 3, 4]) await expect(page.getByTestId(`tranche-${i}`)).toBeVisible()
  // le nom de chaque piste est lisible (pas écrasé par le manque de hauteur)
  for (const [i, nom] of [[0, 'Maître'], [1, 'Piano'], [4, 'Batterie']] as const) {
    const texte = page.getByTestId(`tranche-${i}`).getByText(nom, { exact: true })
    expect((await texte.boundingBox())!.height, nom).toBeGreaterThan(8)
  }
  await page.getByTestId('lecture').click()
  for (const i of [1, 2, 3, 4, 0]) expect(max(await niveau(page, i, 900)), `piste ${i}`).toBeGreaterThan(0.01)
  // les vumètres affichés montent
  await expect.poll(() => page.getByTestId('vumetre-0').locator('[data-cote="gauche"]').getAttribute('data-niveau').then(Number)).toBeGreaterThan(0.3)
  await page.screenshot({ path: 'test-results/mixer.png' })
})

test('muet, solo, faders de piste et du maître', async ({ page }) => {
  await page.getByTestId('lecture').click()
  // muet : la batterie (piste 4) se tait, les autres continuent
  await page.getByRole('button', { name: 'Activer Batterie' }).click()
  await page.waitForTimeout(150)
  expect(max(await niveau(page, 4, 700))).toBeLessThan(SILENCE)
  expect(max(await niveau(page, 1, 700))).toBeGreaterThan(0.01)
  await page.getByRole('button', { name: 'Activer Batterie' }).click()
  // solo sur le piano : seules sa piste et le maître sonnent
  await page.getByRole('button', { name: 'Solo Piano' }).click()
  await page.waitForTimeout(150)
  expect(max(await niveau(page, 1, 700))).toBeGreaterThan(0.01)
  for (const i of [2, 3, 4]) expect(max(await niveau(page, i, 300)), `piste ${i}`).toBeLessThan(SILENCE)
  await page.getByRole('button', { name: 'Solo Piano' }).click()
  // fader de la basse au minimum, au clavier
  const fader = page.getByRole('slider', { name: 'Volume piste Basse' })
  await fader.focus()
  await fader.press('Home')
  await expect(fader).toHaveAttribute('aria-valuetext', '-∞ dB')
  await page.waitForTimeout(150)
  expect(max(await niveau(page, 2, 700))).toBeLessThan(SILENCE)
  // maître à zéro : silence en sortie
  await regler(page, 0, { volume: 0 })
  await page.waitForTimeout(200)
  expect(max(await niveau(page, 0, 500))).toBeLessThan(0.001)
})

test('panoramique : une piste à fond à gauche ne sort presque plus à droite', async ({ page }) => {
  await regler(page, 3, { pan: -1 }) // cordes
  await page.getByTestId('lecture').click()
  const [g, d] = await niveau(page, 3, 1200)
  expect(g).toBeGreaterThan(0.01)
  expect(d).toBeLessThan(g * 0.05)
})

test('routage : la basse envoyée sur la piste 1 quitte la piste 2', async ({ page }) => {
  await onglet(page, 'Channel Rack').click()
  await page.getByLabel('Piste de mixage de Basse').selectOption('1')
  await page.getByTestId('lecture').click()
  expect(max(await niveau(page, 2, 900))).toBeLessThan(SILENCE)
  expect(max(await niveau(page, 1, 900))).toBeGreaterThan(0.01)
  // et elle revient sur sa piste
  await page.getByLabel('Piste de mixage de Basse').selectOption('2')
  expect(max(await niveau(page, 2, 900))).toBeGreaterThan(0.01)
})

test('égaliseur : les graves coupés baissent une note grave, renforcés ils la montent', async ({ page }) => {
  const mesurer = async () => {
    await page.evaluate(() => (window as any).__studio.audio.moteur.noteOn(1, 36, 110)) // basse, canal MIDI 1
    const n = max(await niveau(page, 2, 500))
    await page.evaluate(() => (window as any).__studio.audio.moteur.noteOff(1, 36))
    await page.waitForTimeout(900)
    return n
  }
  await page.evaluate(() => (window as any).__studio.audio.moteur.demarrer())
  const avant = await mesurer()
  await regler(page, 2, { eq: { grave: -12, medium: 0, aigu: 0 } })
  const coupes = await mesurer()
  await regler(page, 2, { eq: { grave: 12, medium: 0, aigu: 0 } })
  const renforces = await mesurer()
  expect(avant).toBeGreaterThan(0.01)
  // la crête mêle la fondamentale (65 Hz) et des harmoniques au-dessus de 200 Hz : l'effet est
  // net mais inférieur aux 12 dB de la bande
  expect(coupes).toBeLessThan(avant * 0.8)
  expect(renforces).toBeGreaterThan(avant * 1.25)
})

test('réverbération : l’envoi laisse une queue après la fin de la note', async ({ page }) => {
  const queue = async () => {
    await page.evaluate(async () => {
      const m = (window as any).__studio.audio.moteur
      await m.demarrer()
      m.noteOn(9, 38, 120) // caisse claire de la démo (piste 4)
      await new Promise((r) => setTimeout(r, 60))
      m.synth.stopAll(true)
    })
    await page.waitForTimeout(250) // la caisse claire est éteinte : reste la réverbération
    const n = max(await niveau(page, 0, 300))
    await page.waitForTimeout(2500)
    return n
  }
  await regler(page, 4, { reverb: 0 })
  const sec = await queue()
  await regler(page, 4, { reverb: 1 })
  const reverbere = await queue()
  expect(reverbere).toBeGreaterThan(sec * 3)
  expect(reverbere).toBeGreaterThan(0.001)
})
