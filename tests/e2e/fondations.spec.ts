// Étape 0 : le moteur multi-instruments dans un vrai navigateur.
import { expect, type Page, test } from '@playwright/test'
import tonejsMidi from '@tonejs/midi'

const { Midi } = tonejsMidi

const erreurs: string[] = []

/** Nombre de voix qui sonnent sur chaque canal du projet, dans l'ordre des canaux. */
const voixParCanal = (page: Page): Promise<number[]> => page.evaluate(() => (window as any).__studio.voixParCanal())

/** Maximum de voix par canal observé pendant `ms` millisecondes. */
async function voixMax(page: Page, ms: number): Promise<number[]> {
  let max: number[] = []
  const fin = Date.now() + ms
  while (Date.now() < fin) {
    const v = await voixParCanal(page)
    max = v.map((x, i) => Math.max(x, max[i] ?? 0))
    await page.waitForTimeout(20)
  }
  return max
}

const position = (page: Page) => page.evaluate(() => (window as any).__studio.audio.transport.position as number)
const niveau = (page: Page) => page.evaluate(() => (window as any).__studio.audio.moteur.niveau() as number)

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

test('la démo joue ses quatre instruments en même temps, au tempo', async ({ page }) => {
  await expect(page.getByTestId('canal')).toHaveCount(4)
  await page.getByTestId('lecture').click()
  await expect(page.getByTestId('lecture')).toHaveText('Pause')
  const max = await voixMax(page, 1500)
  expect(max.every((v) => v > 0), `voix par canal : ${max}`).toBe(true)
  expect(await niveau(page)).toBeGreaterThan(0.005)
  // 100 à la noire : 96 tics en 0,6 s
  const p0 = await position(page)
  await page.waitForTimeout(1200)
  const avance = (await position(page)) - p0
  const attendu = (1.2 / 0.6) * 96
  expect(Math.abs(((avance + 1536) % 1536) - attendu)).toBeLessThan(25)
  await expect(page.getByTestId('position')).not.toHaveText('1:1:00')
})

test('Stop : les notes déjà programmées sont annulées, puis silence', async ({ page }) => {
  // Une note programmée 100 ms dans le futur puis « tout couper » aussitôt : elle ne doit
  // jamais sonner (SpessaSynth ne vide pas sa file de messages datés, voir moteur.ts).
  const voix = await page.evaluate(async () => {
    const moteur = (window as any).__studio.audio.moteur
    await moteur.demarrer()
    const t = moteur.maintenant()
    moteur.noteOn(0, 60, 110, t + 0.1) // piano
    moteur.noteOn(9, 38, 110, t + 0.1) // caisse claire (la batterie ignore les relâchements)
    moteur.toutCouper()
    let max = [0, 0]
    const fin = performance.now() + 400
    while (performance.now() < fin) {
      max = [Math.max(max[0], moteur.voix(0)), Math.max(max[1], moteur.voix(9))]
      await new Promise((r) => setTimeout(r, 5))
    }
    return max
  })
  expect(voix, 'voix maximales [piano, batterie] après « tout couper »').toEqual([0, 0])

  // en lecture réelle : Stop ramène au début et le son s'éteint
  await page.getByTestId('lecture').click()
  await page.waitForTimeout(1300)
  await page.getByTestId('stop').click()
  await expect(page.getByTestId('position')).toHaveText('1:1:00')
  await expect.poll(() => voixParCanal(page).then((v) => v.reduce((a, b) => a + b, 0)), { timeout: 4000 }).toBe(0)
})

test('relancer juste après un Stop rejoue bien les notes', async ({ page }) => {
  await page.getByTestId('lecture').click()
  await page.waitForTimeout(800)
  await page.getByTestId('stop').click()
  await page.getByTestId('lecture').click()
  const max = await voixMax(page, 1200)
  expect(max.every((v) => v > 0), `voix par canal : ${max}`).toBe(true)
})

test('pause et reprise au même endroit', async ({ page }) => {
  await page.getByTestId('lecture').click()
  await page.waitForTimeout(900)
  await page.getByTestId('lecture').click()
  await expect(page.getByTestId('lecture')).toHaveText('Lecture')
  const p = await position(page)
  await page.waitForTimeout(600)
  expect(await position(page)).toBe(p)
  await page.getByTestId('lecture').click()
  await page.waitForTimeout(400)
  expect(await position(page)).toBeGreaterThan(p)
})

test('un canal coupé ne joue plus, les autres continuent', async ({ page }) => {
  await page.getByLabel('Couper Batterie').check()
  await page.getByTestId('lecture').click()
  const max = await voixMax(page, 1500)
  expect(max.slice(0, 3).every((v) => v > 0)).toBe(true)
  expect(max[3]).toBe(0)
})

test('le tempo change la vitesse de lecture', async ({ page }) => {
  await page.getByTestId('tempo').fill('200')
  await page.getByTestId('lecture').click()
  await page.waitForTimeout(300)
  const p0 = await position(page)
  await page.waitForTimeout(900)
  const avance = ((await position(page)) - p0 + 1536) % 1536
  expect(Math.abs(avance - (0.9 / 0.3) * 96)).toBeLessThan(25) // 200 à la noire : 96 tics en 0,3 s
})

test('un fichier MIDI importé joue tous ses instruments', async ({ page }) => {
  const midi = new Midi()
  midi.header.setTempo(120)
  const pistes: [string, number, number, number][] = [['Piano', 0, 0, 60], ['Violon', 40, 1, 76], ['Batterie', 0, 9, 36]]
  for (const [nom, programme, canal, hauteur] of pistes) {
    const piste = midi.addTrack()
    piste.name = nom
    piste.instrument.number = programme
    piste.channel = canal
    for (let i = 0; i < 8; i++) piste.addNote({ midi: hauteur, ticks: i * 240, durationTicks: 200, velocity: 0.9 })
  }
  await page.getByTestId('import').setInputFiles({ name: 'essai.mid', mimeType: 'audio/midi', buffer: Buffer.from(midi.toArray()) })
  await expect(page.getByTestId('canal')).toHaveCount(3)
  await expect(page.getByTestId('canal').nth(1)).toContainText('Violon')
  const batterie = page.getByTestId('canal').nth(2).getByTestId('nom-canal')
  await expect(batterie).toHaveText('Batterie')
  await expect(batterie).toHaveAttribute('title', 'Batterie : Grosse caisse')
  await page.getByTestId('lecture').click()
  const max = await voixMax(page, 1500)
  expect(max.every((v) => v > 0), `voix par canal : ${max}`).toBe(true)
})

test('un fichier illisible est refusé avec un message', async ({ page }) => {
  await page.getByTestId('import').setInputFiles({ name: 'faux.mid', mimeType: 'audio/midi', buffer: Buffer.from('pas un midi') })
  await expect(page.getByTestId('message')).toHaveText('Fichier MIDI illisible : faux.mid')
  erreurs.length = 0 // l'erreur est aussi écrite dans la console, volontairement
  await expect(page.getByTestId('canal')).toHaveCount(4)
})
