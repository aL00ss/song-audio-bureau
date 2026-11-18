import { describe, expect, it } from 'vitest'
import { familleInstrument, LISTE_INSTRUMENTS, nomInstrument, nomNote, nomSonBatterie, vueInstrument } from './gm'
import {
  arrondirALaMesure, formaterPosition, formaterSecondes, secondesEnTics, TICS_PAR_NOIRE, TICS_PAR_PAS,
  ticsEnSecondes, ticsParMesure,
} from './temps'

describe('temps musical', () => {
  it('convertit tics et secondes selon le tempo', () => {
    expect(ticsEnSecondes(TICS_PAR_NOIRE, 120)).toBe(0.5)
    expect(ticsEnSecondes(TICS_PAR_NOIRE * 4, 60)).toBe(4)
    expect(secondesEnTics(0.5, 120)).toBe(TICS_PAR_NOIRE)
    expect(secondesEnTics(ticsEnSecondes(1234, 97), 97)).toBeCloseTo(1234, 9)
  })

  it('un pas vaut une double croche, une mesure dépend de la signature', () => {
    expect(TICS_PAR_PAS).toBe(24)
    expect(ticsParMesure()).toBe(384)
    expect(ticsParMesure({ numerateur: 3, denominateur: 4 })).toBe(288)
    expect(ticsParMesure({ numerateur: 6, denominateur: 8 })).toBe(288)
  })

  it('arrondit à la mesure supérieure, au moins une mesure', () => {
    expect(arrondirALaMesure(0)).toBe(384)
    expect(arrondirALaMesure(384)).toBe(384)
    expect(arrondirALaMesure(385)).toBe(768)
  })

  it('affiche la position en mesure:temps:tic et la durée en m:ss.d', () => {
    expect(formaterPosition(0)).toBe('1:1:00')
    expect(formaterPosition(384 + 96 * 2 + 5)).toBe('2:3:05')
    expect(formaterPosition(-10)).toBe('1:1:00')
    expect(formaterSecondes(72.34)).toBe('1:12.3')
    expect(formaterSecondes(0)).toBe('0:00.0')
  })
})

describe('General MIDI', () => {
  it('nomme les 128 instruments et leurs 16 familles', () => {
    expect(LISTE_INSTRUMENTS).toHaveLength(128)
    expect(nomInstrument(0)).toBe('Piano à queue')
    expect(nomInstrument(40)).toBe('Violon')
    expect(nomInstrument(127)).toBe('Coup de feu')
    expect(familleInstrument(24)).toBe('Guitares')
    expect(familleInstrument(127)).toBe('Bruitages')
    expect(new Set(LISTE_INSTRUMENTS.map((i) => i.famille)).size).toBe(16)
  })

  it('choisit la vue de chaque famille', () => {
    expect(vueInstrument(0)).toBe('clavier')
    expect(vueInstrument(12)).toBe('lames')
    expect(vueInstrument(25)).toBe('manche')
    expect(vueInstrument(33)).toBe('manche')
    expect(vueInstrument(42)).toBe('cordes-frottees')
    expect(vueInstrument(46)).toBe('harpe')
    expect(vueInstrument(56)).toBe('vents')
    expect(vueInstrument(81)).toBe('clavier')
    expect(vueInstrument(0, true)).toBe('batterie')
  })

  it('nomme les sons de batterie et les notes', () => {
    expect(nomSonBatterie(36)).toBe('Grosse caisse')
    expect(nomSonBatterie(42)).toBe('Charleston fermé')
    expect(nomSonBatterie(20)).toBe('Percussion 20')
    expect(nomNote(69)).toBe('La3')
    expect(nomNote(60)).toBe('Do3')
    expect(nomNote(61)).toBe('Do♯3')
  })
})
