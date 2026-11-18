import { describe, expect, it } from 'vitest'
import { creerDemo } from '../projet/demo'
import { creerPiste } from '../projet/modele'
import { deserialiser, serialiser } from '../projet/serialisation'
import { dbVersGain, FADER_UNITE, gainDuFader, gainVersDb, hauteurVumetre, pistesAudibles, texteDb } from './niveaux'

describe('niveaux du mixer', () => {
  it('décibels et gain', () => {
    expect(dbVersGain(0)).toBe(1)
    expect(dbVersGain(-6)).toBeCloseTo(0.501, 3)
    expect(gainVersDb(1)).toBe(0)
    expect(gainVersDb(0)).toBe(-Infinity)
    expect(gainVersDb(dbVersGain(-12.5))).toBeCloseTo(-12.5, 9)
  })

  it('loi du fader : 0,8 = 0 dB, quadratique, bornée', () => {
    expect(gainDuFader(FADER_UNITE)).toBe(1)
    expect(gainDuFader(0)).toBe(0)
    expect(gainVersDb(gainDuFader(1))).toBeCloseTo(3.88, 2)
    expect(gainVersDb(gainDuFader(0.4))).toBeCloseTo(-12.04, 2)
    expect(gainDuFader(2)).toBe(gainDuFader(1))
    expect(texteDb(1)).toBe('+0.0 dB')
    expect(texteDb(0.5)).toBe('-6.0 dB')
    expect(texteDb(0)).toBe('-∞ dB')
  })

  it('solo et muet : le maître n’est jamais concerné', () => {
    const pistes = [creerPiste(0, 'Maître'), creerPiste(1, 'A'), creerPiste(2, 'B'), creerPiste(3, 'C')]
    expect(pistesAudibles(pistes)).toEqual(new Set([1, 2, 3]))
    pistes[2].muet = true
    expect(pistesAudibles(pistes)).toEqual(new Set([1, 3]))
    pistes[3].solo = true
    expect(pistesAudibles(pistes)).toEqual(new Set([3]))
    pistes[2].solo = true // une piste muette mise en solo sonne
    expect(pistesAudibles(pistes)).toEqual(new Set([2, 3]))
    pistes[0].solo = true // solo du maître : sans effet
    expect(pistesAudibles(pistes)).toEqual(new Set([2, 3]))
  })

  it('vumètre : -60 dB en bas, +6 dB en haut', () => {
    expect(hauteurVumetre(0)).toBe(0)
    expect(hauteurVumetre(dbVersGain(-60))).toBeCloseTo(0, 9)
    expect(hauteurVumetre(1)).toBeCloseTo(60 / 66, 9)
    expect(hauteurVumetre(4)).toBe(1)
  })

  it('les pistes ont égaliseur, solo et réverbération ; les anciens projets se relisent', () => {
    const demo = creerDemo()
    expect(demo.mixage[0]).toMatchObject({ index: 0, nom: 'Maître', solo: false, eq: { grave: 0, medium: 0, aigu: 0 }, reverb: 0 })
    expect(demo.mixage[1].reverb).toBeGreaterThan(0)
    expect(deserialiser(serialiser(demo))).toEqual(demo)
    const ancien = JSON.parse(serialiser(demo))
    for (const piste of ancien.mixage) {
      delete piste.solo
      delete piste.eq
      delete piste.reverb
    }
    const relu = deserialiser(JSON.stringify(ancien))
    expect(relu.mixage.map((p) => [p.solo, p.reverb, p.eq.grave])).toEqual([[false, 0, 0], [false, 0.18, 0], [false, 0.18, 0], [false, 0.18, 0], [false, 0.18, 0]])
    ancien.mixage[1].eq = { grave: 99, medium: 0, aigu: 0 }
    expect(() => deserialiser(JSON.stringify(ancien))).toThrow(/mixage\[1\]\.eq\.grave/)
  })
})
