// Outils des tests navigateur : rendu hors ligne d'une note et analyse du son obtenu.
import { expect, type Page } from '@playwright/test'

/**
 * Rendu hors ligne d'une note tenue 4 s (tempo 60, une mesure) sur un instrument seul ; renvoie
 * la hauteur mesurée par autocorrélation et le niveau au début et à la fin de la tenue.
 */
export async function rendreNoteTenue(page: Page, programme: number, hauteur: number, velocite = 100) {
  await page.evaluate(
    ([prog, h, v]) => {
      const e = (window as any).__studio.etat()
      e.nouveauProjet()
      for (const c of e.projet.canaux) (window as any).__studio.etat().modifierCanal(c.id, { muet: true })
      e.changerTempo(60)
      e.changerMode('pattern')
      e.ajouterInstrument(prog)
      const s = (window as any).__studio.etat()
      s.remplacerNotes(s.canalSelectionne, [{ tic: 0, duree: 384, hauteur: h, velocite: v }])
    },
    [programme, hauteur, velocite] as const,
  )
  // toutes les banques demandées (le chargement commence dès l'ajout de l'instrument) sont prêtes
  await expect
    .poll(() => page.evaluate(() => Object.values((window as any).__studio.etat().banques).every((b: any) => b.phase === 'prete')), { timeout: 60_000 })
    .toBe(true)
  return page.evaluate(async () => {
    const s = (window as any).__studio
    const wav: Blob = await s.etat().exporterWav()
    const tampon = await new AudioContext().decodeAudioData(await wav.arrayBuffer())
    const x = tampon.getChannelData(0)
    const fe = tampon.sampleRate
    const rms = (a: number, b: number) => {
      let somme = 0
      for (let i = Math.floor(a * fe); i < b * fe; i++) somme += x[i] * x[i]
      return Math.sqrt(somme / ((b - a) * fe))
    }
    // autocorrélation normalisée sur 0,2 s à partir de 1 s : premier pic au-dessus de 0,8 × max
    const debut = Math.floor(1.0 * fe)
    const n = Math.floor(0.2 * fe)
    const r: number[] = []
    const lagMin = Math.floor(fe / 2000)
    const lagMax = Math.floor(fe / 25)
    let e0 = 0
    for (let i = 0; i < n; i++) e0 += x[debut + i] * x[debut + i]
    for (let lag = lagMin; lag < lagMax; lag++) {
      let somme = 0
      for (let i = 0; i < n; i++) somme += x[debut + i] * x[debut + i + lag]
      r.push(somme / e0)
    }
    const max = Math.max(...r)
    const pic = r.findIndex((v, i) => i > 0 && i < r.length - 1 && v >= 0.8 * max && v >= r[i - 1] && v >= r[i + 1])
    const [a, b, c] = [r[pic - 1], r[pic], r[pic + 1]]
    const fin = pic + (a - 2 * b + c ? (0.5 * (a - c)) / (a - 2 * b + c) : 0)
    const frequence = fe / (lagMin + fin)
    return { note: 69 + 12 * Math.log2(frequence / 440), debut: rms(0.5, 1.0), fin: rms(3.0, 3.5) }
  })
}
