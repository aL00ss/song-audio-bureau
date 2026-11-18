import { describe, expect, it } from 'vitest'
import { ACCORDAGES, accordagePour, doigter, ECART_MAX, positionCase, positionsPossibles } from './manche'
import { CORDES_HARPE, cordeHarpe, CUIVRES_A_PISTONS, pistons } from './vents'

const guitare = ACCORDAGES.guitare

describe('manche', () => {
  it('accordage selon le programme General MIDI', () => {
    expect(accordagePour(24)).toBe('guitare')
    expect(accordagePour(33)).toBe('basse')
    expect(accordagePour(40)).toBe('violon')
    expect(accordagePour(42)).toBe('violoncelle')
    expect(accordagePour(0)).toBeNull()
  })

  it('positions possibles d’une note', () => {
    expect(positionsPossibles(40, guitare)).toEqual([{ corde: 0, case: 0 }]) // mi grave à vide
    expect(positionsPossibles(64, guitare).map((p) => `${p.corde}:${p.case}`)).toEqual(['0:24', '1:19', '2:14', '3:9', '4:5', '5:0'].filter((x) => Number(x.split(':')[1]) <= 20))
    expect(positionsPossibles(30, guitare)).toEqual([]) // trop grave
  })

  it('un accord de do majeur ouvert : une corde par note, main resserrée', () => {
    const [d] = doigter([[48, 52, 55, 60, 64]], guitare) // do mi sol do mi
    const placees = d.positions.filter((p) => p !== null)
    expect(placees).toHaveLength(5)
    expect(new Set(placees.map((p) => p!.corde)).size).toBe(5)
    expect(d.ecart).toBeLessThanOrEqual(ECART_MAX)
    expect(Math.max(...placees.map((p) => p!.case))).toBeLessThanOrEqual(5)
  })

  it('une gamme reste dans une position de main (aucun saut absurde)', () => {
    const gamme = [55, 57, 59, 60, 62, 64, 65, 67].map((n) => [n])
    const d = doigter(gamme, guitare)
    // les cordes à vide ne demandent aucun déplacement : on compare les cases appuyées
    const appuyees = d.map((x) => x.positions[0]!.case).filter((c) => c > 0)
    expect(Math.max(...appuyees) - Math.min(...appuyees), `cases ${appuyees}`).toBeLessThanOrEqual(ECART_MAX + 1)
    expect(Math.max(...appuyees)).toBeLessThanOrEqual(7) // une gamme de sol se joue en bas du manche
  })

  it('les notes en trop ou hors tessiture ne sont pas placées', () => {
    const [d] = doigter([[20, 60]], guitare)
    expect(d.positions[0]).toBeNull()
    expect(d.positions[1]).not.toBeNull()
    const [tropDeNotes] = doigter([[40, 45, 50, 55, 59, 64, 69]], guitare)
    expect(tropDeNotes.positions.filter((p) => p === null)).toHaveLength(1)
    expect(doigter([], guitare)).toEqual([])
  })

  it('les cases se resserrent vers le haut du manche', () => {
    expect(positionCase(0, 20)).toBe(0)
    const largeur1 = positionCase(1, 20) - positionCase(0, 20)
    const largeur12 = positionCase(13, 20) - positionCase(12, 20)
    expect(largeur12).toBeCloseTo(largeur1 / 2, 2)
  })
})

describe('cuivres et harpe', () => {
  it('pistons de la trompette (son réel)', () => {
    expect(pistons(58)).toEqual([]) // si♭3 à vide
    expect(pistons(57)).toEqual([2])
    expect(pistons(56)).toEqual([1])
    expect(pistons(60)).toEqual([1, 3]) // do4 (ré écrit) : 1 et 3
    expect(pistons(65)).toEqual([]) // fa4 à vide
    expect(pistons(64)).toEqual([2])
    expect(pistons(52)).toEqual([1, 2, 3]) // mi3, la note la plus grave : trois pistons
    expect(pistons(51)).toBeNull() // sous la tessiture
    expect(CUIVRES_A_PISTONS[56].nom).toBe('Trompette')
  })

  it('harpe : cordes diatoniques, altérations sur la corde du dessous', () => {
    expect(CORDES_HARPE[0]).toBe(24)
    expect(CORDES_HARPE.at(-1)).toBe(103)
    expect(CORDES_HARPE.every((n) => [0, 2, 4, 5, 7, 9, 11].includes(n % 12))).toBe(true)
    expect(cordeHarpe(60)).toBe(60)
    expect(cordeHarpe(61)).toBe(60)
  })
})
