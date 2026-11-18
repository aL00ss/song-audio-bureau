import { describe, expect, it } from 'vitest'
import { creerDemo } from './demo'
import { ajouterCanal, creerProjet } from './modele'
import { distanceCirculaire, notesAttendues, noteJuste, TOLERANCE } from './partie'
import { definirPas } from './pas'
import { ajouterClip } from './playlist'

describe('jouer avec l’orchestre', () => {
  it('notes attendues : celles du pattern, ou de chaque clip en mode morceau', () => {
    const demo = creerDemo()
    const basse = demo.canaux[1].id
    expect(notesAttendues(demo, basse, 'pattern').slice(0, 3)).toEqual([{ tic: 0, hauteur: 36 }, { tic: 192, hauteur: 43 }, { tic: 288, hauteur: 48 }])
    let p = creerProjet()
    const r = ajouterCanal(p, { batterie: true })
    p = definirPas(r.projet, r.id, 0)
    p = ajouterClip(p, p.patternActif, 0, 0).projet
    p = ajouterClip(p, p.patternActif, 1, 768).projet
    expect(notesAttendues(p, r.id, 'morceau')).toEqual([{ tic: 0, hauteur: 36 }, { tic: 768, hauteur: 36 }])
  })

  it('distance dans une boucle : la fin rejoint le début', () => {
    expect(distanceCirculaire(10, 30, 384)).toBe(20)
    expect(distanceCirculaire(380, 4, 384)).toBe(8)
    expect(distanceCirculaire(0, 384, 384)).toBe(0)
  })

  it('une note est juste à la bonne hauteur, à un pas près', () => {
    const attendues = [{ tic: 96, hauteur: 60 }, { tic: 0, hauteur: 64 }]
    expect(noteJuste(attendues, 96 + TOLERANCE, 60, 384)).toBe(true)
    expect(noteJuste(attendues, 96 - TOLERANCE - 1, 60, 384)).toBe(false)
    expect(noteJuste(attendues, 96, 61, 384)).toBe(false)
    expect(noteJuste(attendues, 380, 64, 384)).toBe(true) // juste avant la reprise de la boucle
  })
})
