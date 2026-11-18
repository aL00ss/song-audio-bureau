import { describe, expect, it } from 'vitest'
import {
  BASE_DEFAUT, caractere, decalerBase, detecterDisposition, disposition, estNoire, etiquettes, nombreDeBlanches, noteDuCode,
  toucheSous,
} from '../musique/clavier'
import {
  aimanter, aimanterBas, ajouterNote, definirVelocite, deplacerNotes, etendue, noteSous, notesDansRectangle, quantifier,
  redimensionnerNotes, supprimerNotes,
} from './edition'
import type { NoteMusicale } from './modele'

const n = (tic: number, hauteur: number, duree = 24, velocite = 100): NoteMusicale => ({ tic, duree, hauteur, velocite })
const tous = (k: number) => new Set(Array.from({ length: k }, (_, i) => i))

describe('clavier', () => {
  it('88 touches dont 52 blanches, de La0 à Do8', () => {
    const t = disposition()
    expect(t).toHaveLength(88)
    expect(nombreDeBlanches(t)).toBe(52)
    expect([t[0].midi, t.at(-1)!.midi]).toEqual([21, 108])
    expect(estNoire(61)).toBe(true)
    expect(estNoire(60)).toBe(false)
  })

  it('retrouve la touche sous le pointeur (noire en haut, blanche en bas)', () => {
    const t = disposition(48, 72)
    const doDiese = t.find((x) => x.midi === 49)!
    expect(toucheSous(t, doDiese.x + doDiese.largeur / 2, 0.3)).toBe(49)
    expect(toucheSous(t, doDiese.x + doDiese.largeur / 2, 0.9)).toBe(48)
    expect(toucheSous(t, 0.5, 0.9)).toBe(48)
    expect(toucheSous(t, -1, 0.5)).toBeNull()
    expect(toucheSous(t, 0.5, 1.5)).toBeNull()
  })

  it('clavier de l’ordinateur : rangée du haut au do central, lettres AZERTY ou QWERTY', () => {
    expect(noteDuCode('KeyQ')).toBe(60)
    expect(noteDuCode('KeyZ')).toBe(48)
    expect(noteDuCode('Digit2')).toBe(61)
    expect(noteDuCode('Space')).toBeNull()
    expect(caractere('KeyQ', 'azerty')).toBe('A')
    expect(caractere('KeyQ', 'qwerty')).toBe('Q')
    expect(etiquettes(BASE_DEFAUT, 'azerty').get(60)).toBe('A')
    expect(decalerBase(BASE_DEFAUT, 10)).toBe(84)
    expect(decalerBase(BASE_DEFAUT, -1)).toBe(36)
  })

  it('détecte la disposition, sinon d’après la langue', async () => {
    const avec = (q: string) => ({ keyboard: { getLayoutMap: async () => new Map([['KeyQ', q]]) }, language: 'en-US' }) as unknown as Navigator
    expect(await detecterDisposition(avec('a'))).toBe('azerty')
    expect(await detecterDisposition(avec('q'))).toBe('qwerty')
    expect(await detecterDisposition({ language: 'fr-FR' } as Navigator)).toBe('azerty')
    expect(await detecterDisposition({ language: 'en-GB' } as Navigator)).toBe('qwerty')
  })
})

describe('édition des notes', () => {
  it('aimantation à la grille', () => {
    expect(aimanter(37, 24)).toBe(48)
    expect(aimanter(35, 24)).toBe(24)
    expect(aimanterBas(47, 24)).toBe(24)
    expect(aimanter(37.4, 0)).toBe(37)
  })

  it('note sous le pointeur : la plus récente gagne', () => {
    const notes = [n(0, 60, 96), n(48, 60, 24)]
    expect(noteSous(notes, 50, 60)).toBe(1)
    expect(noteSous(notes, 10, 60)).toBe(0)
    expect(noteSous(notes, 10, 61)).toBe(-1)
    expect(noteSous(notes, 96, 60)).toBe(-1)
  })

  it('ajouter et supprimer', () => {
    const notes = ajouterNote([n(0, 60)], { tic: -5, duree: 0, hauteur: 200, velocite: 0 })
    expect(notes[1]).toEqual({ tic: 0, duree: 1, hauteur: 127, velocite: 1 })
    expect(supprimerNotes(notes, new Set([0]))).toEqual([notes[1]])
  })

  it('déplacer un groupe sans sortir du piano, en gardant l’ordre', () => {
    const notes = [n(24, 60), n(48, 126), n(0, 10)]
    const choix = new Set([0, 1])
    expect(deplacerNotes(notes, choix, 24, 1).map((x) => [x.tic, x.hauteur])).toEqual([[48, 61], [72, 127], [0, 10]])
    // trop haut : le groupe s'arrête quand sa note la plus haute atteint 127
    expect(deplacerNotes(notes, choix, 0, 5).map((x) => x.hauteur)).toEqual([61, 127, 10])
    // trop tôt : le groupe s'arrête au début du pattern
    expect(deplacerNotes(notes, choix, -100, 0).map((x) => x.tic)).toEqual([0, 24, 0])
  })

  it('redimensionner, quantifier, vélocité', () => {
    const notes = [n(5, 60, 30), n(50, 62, 30)]
    expect(redimensionnerNotes(notes, new Set([0]), -100).map((x) => x.duree)).toEqual([1, 30])
    expect(quantifier(notes, new Set(), 24).map((x) => x.tic)).toEqual([0, 48])
    expect(quantifier(notes, new Set([1]), 24).map((x) => x.tic)).toEqual([5, 48])
    expect(definirVelocite(notes, tous(2), 300).map((x) => x.velocite)).toEqual([127, 127])
  })

  it('sélection au lasso et étendue des hauteurs', () => {
    const notes = [n(0, 60, 24), n(30, 64, 24), n(100, 70, 24)]
    expect([...notesDansRectangle(notes, 50, 10, 66, 58)]).toEqual([0, 1])
    expect([...notesDansRectangle(notes, 24, 30, 60, 64)]).toEqual([])
    expect(etendue(notes)).toEqual({ basse: 60, haute: 70 })
    expect(etendue([])).toBeNull()
  })
})
