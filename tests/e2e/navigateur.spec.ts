// Étape 2 : le navigateur (morceaux de démonstration, recherche en ligne BitMidi).
import { expect, type Page, type Route, test } from '@playwright/test'
import tonejsMidi from '@tonejs/midi'

const { Midi } = tonejsMidi
const erreurs: string[] = []
const etat = (page: Page) => page.evaluate(() => (window as any).__studio.etat())
const voixParCanal = (page: Page): Promise<number[]> => page.evaluate(() => (window as any).__studio.voixParCanal())

async function voixMax(page: Page, ms: number): Promise<number[]> {
  let max: number[] = []
  for (const fin = Date.now() + ms; Date.now() < fin; await page.waitForTimeout(25)) {
    const v = await voixParCanal(page)
    max = v.map((x, i) => Math.max(x, max[i] ?? 0))
  }
  return max
}

/** Fichier MIDI à trois instruments : piano, flûte (73), batterie. */
function fichierTroisInstruments(): Buffer {
  const midi = new Midi()
  midi.header.setTempo(120)
  const pistes: [string, number, number, number][] = [['Piano', 0, 0, 60], ['Flûte', 73, 1, 79], ['Batterie', 0, 9, 42]]
  for (const [nom, programme, canal, hauteur] of pistes) {
    const piste = midi.addTrack()
    piste.name = nom
    piste.instrument.number = programme
    piste.channel = canal
    for (let i = 0; i < 16; i++) piste.addNote({ midi: hauteur, ticks: i * 240, durationTicks: 200, velocity: 0.9 })
  }
  return Buffer.from(midi.toArray())
}

const cors = { 'access-control-allow-origin': '*' }
const reponseJson = (results: unknown[]) => ({ status: 200, contentType: 'application/json', headers: cors, body: JSON.stringify({ result: { results } }) })

test.beforeEach(async ({ page }) => {
  erreurs.length = 0
  page.on('pageerror', (e) => erreurs.push(e.message))
  page.on('console', (m) => m.type() === 'error' && !m.text().includes('Failed to load resource') && erreurs.push(m.text()))
  await page.goto('./')
  await expect(page.getByTestId('statut')).toHaveText('Prêt', { timeout: 30_000 })
})

test.afterEach(() => {
  expect(erreurs).toEqual([])
})

test('les morceaux de démonstration se chargent et se jouent', async ({ page }) => {
  await expect(page.getByTestId('morceau')).toHaveCount(5)
  await page.getByTestId('morceau').filter({ hasText: 'Prélude en do majeur' }).click()
  await expect(page.getByTestId('lecture')).toHaveText('Pause', { timeout: 10_000 })
  const e = await etat(page)
  expect(e.projet.nom).toBe('Prélude en do majeur')
  expect(e.projet.canaux).toHaveLength(2)
  const max = await voixMax(page, 1500)
  expect(max.every((v) => v > 0), `voix par canal : ${max}`).toBe(true)
  await page.getByTestId('charger-demo').click()
  await expect(page.getByTestId('canal')).toHaveCount(4)
  expect((await etat(page)).transport).toBe('arret')
})

test('recherche en ligne : un résultat se charge avec ses instruments et se joue', async ({ page }) => {
  const requetes: string[] = []
  await page.route('https://bitmidi.com/api/midi/search**', (route: Route) => {
    requetes.push(route.request().url())
    return route.fulfill(reponseJson([
      { id: 1, name: 'trio-de-test.mid', downloadUrl: '/uploads/1.mid', plays: 1234 },
      { id: 2, name: 'sans-fichier.mid' },
    ]))
  })
  await page.route('https://bitmidi.com/uploads/1.mid', (route) =>
    route.fulfill({ status: 200, contentType: 'audio/midi', headers: cors, body: fichierTroisInstruments() }))

  const champ = page.getByTestId('recherche')
  await champ.fill('t')
  await page.waitForTimeout(600)
  expect(requetes, 'une seule lettre : pas de recherche').toEqual([])
  await champ.fill('trio')
  await expect(page.getByTestId('resultat')).toHaveCount(1)
  expect(requetes).toHaveLength(1)
  expect(requetes[0]).toContain('q=trio')
  await expect(page.getByTestId('resultat')).toContainText('trio de test')
  await expect(page.getByTestId('resultat')).toContainText('1 234 écoutes')

  // écrire dans la recherche ne lance pas la lecture
  await champ.press('Space')
  expect((await etat(page)).transport).toBe('arret')

  await page.getByTestId('resultat').click()
  await expect(page.getByTestId('canal')).toHaveCount(3)
  await expect(page.getByTestId('lecture')).toHaveText('Pause')
  const projet = (await etat(page)).projet
  expect(projet.nom).toBe('trio de test')
  expect(projet.canaux.map((c: any) => [c.nom, c.programme, c.batterie])).toEqual([['Piano', 0, false], ['Flûte', 73, false], ['Batterie', 0, true]])
  const max = await voixMax(page, 1500)
  expect(max.every((v) => v > 0), `voix par canal : ${max}`).toBe(true)
})

test('recherche : aucun résultat, service en panne, fichier introuvable', async ({ page }) => {
  let reponse = reponseJson([])
  await page.route('https://bitmidi.com/api/midi/search**', (route) => route.fulfill(reponse))
  await page.getByTestId('recherche').fill('zzzzqqq')
  await expect(page.getByText('Aucun fichier trouvé pour « zzzzqqq ».')).toBeVisible()

  reponse = { status: 503, contentType: 'text/plain', headers: cors, body: 'indisponible' }
  await page.getByTestId('recherche').fill('chopin')
  await expect(page.getByText('Recherche indisponible : vérifiez la connexion.')).toBeVisible()

  reponse = reponseJson([{ id: 9, name: 'perdu.mid', downloadUrl: '/uploads/9.mid', plays: 1 }])
  await page.route('https://bitmidi.com/uploads/9.mid', (route) => route.fulfill({ status: 404, headers: cors, body: '' }))
  await page.getByTestId('recherche').fill('perdu')
  await page.getByTestId('resultat').click()
  await expect(page.getByTestId('message')).toHaveText('Impossible de charger « perdu » (fichier introuvable ou illisible).')
  await expect(page.getByTestId('canal')).toHaveCount(4) // le projet en cours est conservé
  erreurs.length = 0 // l'échec est aussi écrit dans la console, volontairement
})

test('recherche réelle sur BitMidi (réseau)', async ({ page }) => {
  test.skip(process.env.RESEAU !== '1', 'test réseau : RESEAU=1 pour le lancer')
  await page.getByTestId('recherche').fill('chopin nocturne')
  await expect(page.getByTestId('resultat').first()).toBeVisible({ timeout: 20_000 })
  await page.getByTestId('resultat').first().click()
  await expect(page.getByTestId('lecture')).toHaveText('Pause', { timeout: 30_000 })
  expect((await etat(page)).projet.canaux.length).toBeGreaterThan(0)
  const max = await voixMax(page, 2500)
  expect(max.some((v) => v > 0)).toBe(true)
})
