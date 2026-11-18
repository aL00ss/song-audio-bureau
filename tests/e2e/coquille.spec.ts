// Étape 1 : la coquille du studio (panneaux, raccourcis, disposition, transport, rack).
import { expect, type Page, test } from '@playwright/test'

const erreurs: string[] = []
const onglet = (page: Page, titre: string) => page.locator('.dv-tab').filter({ hasText: new RegExp(`^${titre}$`) })
const etat = (page: Page) => page.evaluate(() => (window as any).__studio.etat())

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

test('les six panneaux sont ouverts dans la disposition par défaut', async ({ page }) => {
  for (const titre of ['Navigateur', 'Playlist', 'Channel Rack', 'Piano roll', 'Instrument', 'Mixer']) {
    await expect(onglet(page, titre)).toHaveCount(1)
  }
  for (const id of ['navigateur', 'playlist', 'rack', 'pianoroll', 'mixer', 'instrument']) {
    await expect(page.getByTestId(`ouvrir-${id}`)).toHaveAttribute('aria-pressed', 'true')
  }
  await expect(page.getByTestId('canal')).toHaveCount(4)
  await page.screenshot({ path: 'test-results/coquille-bureau.png' })
})

test('F6 ferme le Channel Rack au premier plan, puis le rouvre ; F5 ne recharge pas la page', async ({ page }) => {
  await page.evaluate(() => ((window as any).__marqueur = 'toujours là'))
  await onglet(page, 'Channel Rack').click()
  await page.keyboard.press('F6')
  await expect(onglet(page, 'Channel Rack')).toHaveCount(0)
  await expect(page.getByTestId('ouvrir-rack')).toHaveAttribute('aria-pressed', 'false')
  await page.keyboard.press('F6')
  await expect(onglet(page, 'Channel Rack')).toHaveCount(1)
  await expect(page.getByTestId('canal')).toHaveCount(4)
  await page.keyboard.press('F5')
  await page.waitForTimeout(300)
  expect(await page.evaluate(() => (window as any).__marqueur)).toBe('toujours là')
})

test('la disposition est gardée après rechargement, et peut être réinitialisée', async ({ page }) => {
  await onglet(page, 'Mixer').click()
  await page.keyboard.press('F9') // le Mixer est au premier plan : F9 le ferme
  await expect(onglet(page, 'Mixer')).toHaveCount(0)
  await page.reload()
  await expect(page.getByTestId('statut')).toHaveText('Prêt', { timeout: 30_000 })
  await expect(onglet(page, 'Channel Rack')).toHaveCount(1)
  await expect(onglet(page, 'Mixer')).toHaveCount(0)
  await page.getByTestId('reinitialiser-disposition').click()
  await expect(onglet(page, 'Mixer')).toHaveCount(1)
  await page.getByTestId('ouvrir-mixer').click() // déjà ouvert : reste ouvert
  await expect(onglet(page, 'Mixer')).toHaveCount(1)
})

test('Espace lance et met en pause, Maj+Espace arrête, mais pas pendant une saisie', async ({ page }) => {
  await page.locator('body').click({ position: { x: 5, y: 5 } })
  await page.keyboard.press('Space')
  await expect(page.getByTestId('lecture')).toHaveText('Pause')
  await page.waitForTimeout(600)
  await page.keyboard.press('Space')
  await expect(page.getByTestId('lecture')).toHaveText('Lecture')
  await expect(page.getByTestId('position')).not.toHaveText('1:1:00')
  await page.keyboard.press('Shift+Space')
  await expect(page.getByTestId('position')).toHaveText('1:1:00')
  // dans le champ du tempo, Espace ne lance rien
  await page.getByTestId('tempo').focus()
  await page.keyboard.press('Space')
  await page.waitForTimeout(200)
  expect((await etat(page)).transport).toBe('arret')
  // le bouton Lecture cliqué puis Espace : une seule bascule, pas deux
  await page.getByTestId('lecture').click()
  await expect(page.getByTestId('lecture')).toHaveText('Pause')
  await page.keyboard.press('Space')
  await expect(page.getByTestId('lecture')).toHaveText('Lecture')
})

test('les afficheurs suivent la lecture', async ({ page }) => {
  await page.getByTestId('lecture').click()
  await page.waitForTimeout(1300)
  await expect(page.getByTestId('position')).toHaveText(/^1:[3-4]:\d\d$/) // 1,3 s à 100 : 2,1 noires
  await expect(page.getByTestId('temps')).toHaveText(/^0:01\.\d$/)
})

test('boutons rotatifs : volume général et volume d’un canal, au clavier', async ({ page }) => {
  const general = page.getByRole('slider', { name: 'Volume général' })
  await general.focus()
  await general.press('ArrowDown')
  await general.press('ArrowDown')
  expect((await etat(page)).projet.mixage[0].volume).toBeCloseTo(0.78, 5)
  const volumePiano = page.getByRole('slider', { name: 'Volume Piano' })
  await volumePiano.focus()
  await volumePiano.press('Home')
  expect((await etat(page)).projet.canaux[0].volume).toBe(0)
  const pan = page.getByRole('slider', { name: 'Panoramique Basse' })
  await pan.focus()
  await pan.press('End')
  expect((await etat(page)).projet.canaux[1].pan).toBe(1)
  await expect(pan).toHaveAttribute('aria-valuetext', '100 % droite')
})

test('les diodes du rack s’allument pendant la lecture', async ({ page }) => {
  await page.getByTestId('lecture').click()
  let allumees = 0
  for (let i = 0; i < 40 && allumees < 4; i++) {
    const voix = await page.getByTestId('canal').evaluateAll((lignes) => lignes.map((l) => Number(l.querySelector('[data-voix]')?.getAttribute('data-voix'))))
    allumees = Math.max(allumees, voix.filter((v) => v > 0).length)
    await page.waitForTimeout(50)
  }
  expect(allumees).toBe(4)
})

test('le navigateur ajoute un instrument, qui joue et devient sélectionné', async ({ page }) => {
  await page.locator('summary', { hasText: /^Cordes$/ }).click()
  await page.getByRole('button', { name: '＋ Violon', exact: true }).click()
  await expect(page.getByTestId('canal')).toHaveCount(5)
  await expect(page.getByTestId('canal').nth(4)).toHaveAttribute('data-selectionne', 'true')
  const projet = await etat(page)
  expect(projet.projet.canaux[4]).toMatchObject({ nom: 'Violon', programme: 40 })
  // le nouvel instrument est envoyé au synthétiseur, avec son programme, sur son propre canal
  // MIDI (le 3 : la batterie de la démo est sur le canal 10 et ne prend pas de place)
  const canal = await page.evaluate((id) => (window as any).__studio.canalMidi(id), projet.projet.canaux[4].id)
  expect(canal).toBe(3)
  await expect.poll(() => page.evaluate((c) => (window as any).__studio.audio.moteur.synth.midiChannels[c].patch.program, canal)).toBe(40)
  await page.evaluate((c) => (window as any).__studio.audio.moteur.noteOn(c, 76, 110), canal)
  await expect.poll(() => page.evaluate((c) => (window as any).__studio.audio.moteur.voix(c), canal)).toBeGreaterThan(0)
})

test('sur un écran étroit, la barre se replie sans déborder', async ({ page }) => {
  await page.setViewportSize({ width: 820, height: 700 })
  await page.waitForTimeout(300)
  const debord = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)
  expect(debord).toBeLessThanOrEqual(0)
  await page.screenshot({ path: 'test-results/coquille-etroit.png' })
})

test('sur un téléphone : un panneau à la fois, choisi par onglets, et rien ne déborde', async ({ page }) => {
  await page.setViewportSize({ width: 400, height: 800 })
  await expect(page.getByTestId('panneau-unique')).toBeVisible()
  await expect(page.locator('.dv-tab')).toHaveCount(0)
  await expect(page.getByTestId('canal').first()).toBeVisible()
  await page.getByTestId('onglet-mixer').click()
  await expect(page.getByTestId('onglet-mixer')).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByTestId('canal')).toHaveCount(0)
  await expect(page.getByRole('tabpanel', { name: 'Mixer' })).toBeVisible()
  await page.getByTestId('onglet-instrument').click()
  await expect(page.getByTestId('jouer-partie')).toBeVisible()
  // la page elle-même ne défile pas en largeur (seuls les panneaux défilent)
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(400)
  await page.screenshot({ path: 'test-results/studio-telephone.png' })
  // revenu sur grand écran : les panneaux ancrables reviennent
  await page.setViewportSize({ width: 1280, height: 720 })
  await expect(page.locator('.dv-tab')).toHaveCount(6)
})
