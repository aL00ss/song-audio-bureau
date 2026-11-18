import { Midi } from '@tonejs/midi'
import { describe, expect, it } from 'vitest'
import { creerDemo } from './demo'
import { longueurExport, nomDeFichier, versMidi, versMidiOctets } from './exportMidi'
import { depuisMidi } from './importMidi'
import { ajouterCanal, creerProjet, modifierCanal, patternActif, type Projet } from './modele'
import { definirPas } from './pas'
import { ajouterClip, nouveauPattern } from './playlist'

const relire = (octets: Uint8Array) => new Midi(octets)

describe('export MIDI', () => {
  it('aller-retour : la démo réimportée a les mêmes instruments et les mêmes notes', () => {
    const demo = creerDemo()
    const fichier = relire(versMidiOctets(demo, { mode: 'pattern' }))
    expect(fichier.header.tempos[0].bpm).toBeCloseTo(100, 6)
    expect(fichier.tracks.map((t) => [t.name, t.instrument.number, t.channel])).toEqual([
      ['Piano', 0, 0], ['Basse', 33, 1], ['Cordes', 48, 2], ['Batterie', 0, 9],
    ])
    const relu = depuisMidi(fichier, 'relu')
    expect(relu.tempo).toBe(100)
    const avant = patternActif(demo)
    const apres = patternActif(relu)
    demo.canaux.forEach((canal, i) => {
      const a = avant.notes[canal.id].map((n) => [n.tic, n.duree, n.hauteur, n.velocite])
      const b = apres.notes[relu.canaux[i].id].map((n) => [n.tic, n.duree, n.hauteur, n.velocite])
      expect(b, canal.nom).toEqual(a)
    })
  })

  it('les canaux coupés ne sont pas exportés ; le swing est appliqué', () => {
    let p: Projet = creerProjet()
    const a = ajouterCanal(p, { batterie: true, nom: 'Grosse caisse' })
    p = a.projet
    const b = ajouterCanal(p, { batterie: true, nom: 'Caisse claire', noteDefaut: 38 })
    p = definirPas(definirPas(b.projet, a.id, 0), a.id, 1)
    p = definirPas(p, b.id, 4)
    p = { ...modifierCanal(p, b.id, { muet: true }), swing: 1 }
    const fichier = versMidi(p, { mode: 'pattern' })
    expect(fichier.tracks).toHaveLength(1)
    expect(fichier.tracks[0].notes.map((n) => n.ticks)).toEqual([0, (24 + 12) * 5]) // 2e pas retardé d'un demi-pas
  })

  it('batteries : canal 10 dans un fichier standard, canaux internes pour le rendu du studio', () => {
    let p = creerProjet()
    const a = ajouterCanal(p, { batterie: true, nom: 'Grosse caisse' })
    p = a.projet
    const b = ajouterCanal(p, { batterie: true, nom: 'Caisse claire', noteDefaut: 38 })
    p = definirPas(definirPas(b.projet, a.id, 0), b.id, 4)
    expect(versMidi(p, { mode: 'pattern' }).tracks.map((t) => t.channel)).toEqual([9, 9])
    expect(versMidi(p, { mode: 'pattern', canauxInternes: true }).tracks.map((t) => t.channel)).toEqual([9, 0])
  })

  it('mode morceau : les clips de la playlist, à leur place', () => {
    let p = creerProjet()
    const a = ajouterCanal(p, { batterie: true, nom: 'Grosse caisse' })
    p = definirPas(a.projet, a.id, 0)
    const premier = p.patternActif
    const deuxieme = nouveauPattern(p)
    p = definirPas(deuxieme.projet, a.id, 2)
    p = ajouterClip(p, premier, 0, 0).projet
    p = ajouterClip(p, deuxieme.id, 0, 384).projet
    p = ajouterClip(p, premier, 0, 768).projet
    expect(longueurExport(p, 'morceau')).toBe(3 * 384)
    expect(versMidi(p, { mode: 'morceau' }).tracks[0].notes.map((n) => n.ticks / 5)).toEqual([0, 384 + 48, 768])
    expect(longueurExport(p, 'pattern')).toBe(384)
  })

  it('nom de fichier sûr', () => {
    expect(nomDeFichier('Démo', 'mid')).toBe('Démo.mid')
    expect(nomDeFichier('a/b:c?', 'wav')).toBe('a b c.wav')
    expect(nomDeFichier('   ', 'mid')).toBe('projet.mid')
  })
})
