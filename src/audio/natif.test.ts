import { describe, expect, it } from 'vitest'
import { creerDemo } from '../projet/demo'
import { ajouterCanal, creerProjet } from '../projet/modele'
import { definirPas } from '../projet/pas'
import { ajouterClip } from '../projet/playlist'
import { configurationNative, evenementsExport, QUEUE_EXPORT, sessionNative } from './natif'

describe('client du moteur Rust', () => {
  it('configuration : chaque canal sur son canal MIDI, routé vers sa piste ; pistes audibles', () => {
    const demo = creerDemo()
    const c = configurationNative(demo)
    expect(c.canaux).toHaveLength(4)
    const batterie = c.canaux.find((x) => x.batterie)!
    expect(batterie.canal).toBe(9)
    expect(c.canaux.map((x) => x.piste)).toEqual([1, 2, 3, 4])
    expect(c.pistes[0]).toMatchObject({ index: 0, audible: true })
    expect(c.pistes.every((p) => p.audible)).toBe(true)
    // solo sur une piste : les autres se taisent, le maître reste
    const solo = { ...demo, mixage: demo.mixage.map((p) => (p.index === 2 ? { ...p, solo: true } : p)) }
    expect(configurationNative(solo).pistes.map((p) => p.audible)).toEqual([true, false, true, false, false])
  })

  it('export : notes en secondes au tempo, relâchements à vélocité nulle, queue de 3 s', () => {
    let p = creerProjet()
    const r = ajouterCanal(p, { batterie: true })
    p = definirPas(r.projet, r.id, 4) // 5e double croche
    p = { ...p, tempo: 120 }
    const { evenements, duree } = evenementsExport(p, 'pattern')
    expect(evenements[0]).toMatchObject({ temps: 0.5, canal: 9, velocite: expect.any(Number) })
    expect(evenements[0].velocite).toBeGreaterThan(0)
    // une note de pas dure un peu moins qu'une double croche (0,125 s à 120)
    expect(evenements[1].velocite).toBe(0)
    expect(evenements[1].temps).toBeGreaterThan(0.6)
    expect(evenements[1].temps).toBeLessThanOrEqual(0.625)
    expect(duree).toBeCloseTo(2 + QUEUE_EXPORT) // une mesure à 120 = 2 s
  })

  it('session : notes appariées (début, fin, canal, hauteur, vélocité), boucle, mesure', () => {
    let p = creerProjet()
    const r = ajouterCanal(p, { batterie: true })
    p = definirPas(definirPas(r.projet, r.id, 0), r.id, 8)
    const s = sessionNative(p, 'pattern')
    expect(s.longueur).toBe(384)
    expect(s.temps_par_mesure).toBe(4)
    expect(s.notes).toHaveLength(2)
    const [debut, fin, canal, hauteur, velocite] = s.notes[1]
    expect([debut, canal, hauteur]).toEqual([192, 9, 36])
    expect(fin).toBeGreaterThan(debut)
    expect(velocite).toBeGreaterThan(0)
    // mode morceau : chaque clip rejoue le pattern à sa place
    p = ajouterClip(ajouterClip(p, p.patternActif, 0, 0).projet, p.patternActif, 0, 384).projet
    const m = sessionNative(p, 'morceau')
    expect(m.notes.map((n) => n[0])).toEqual([0, 192, 384, 576])
    expect(m.longueur).toBe(768)
  })
})
