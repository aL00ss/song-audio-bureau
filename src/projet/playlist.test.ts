import { produce } from 'immer'
import { describe, expect, it } from 'vitest'
import { evenementsDuMorceau, longueurDuMorceau, type Minuterie, Transport } from '../audio/transport'
import { ajouterCanal, creerProjet, patternActif, type Projet } from './modele'
import { definirPas } from './pas'
import {
  ajouterClip, choisirPattern, clipSous, deplacerClip, dupliquerPattern, nombreDePistes, nomPatternSuivant, nouveauPattern,
  renommerPattern, supprimerClip, supprimerPattern,
} from './playlist'

/** Deux patterns d'une mesure : A (grosse caisse au 1er pas), B (caisse claire au 1er pas). */
function deuxPatterns(): { projet: Projet; a: string; b: string } {
  let projet = creerProjet()
  const kick = ajouterCanal(projet, { batterie: true, nom: 'Grosse caisse', noteDefaut: 36 })
  projet = kick.projet
  const caisse = ajouterCanal(projet, { batterie: true, nom: 'Caisse claire', noteDefaut: 38 })
  projet = caisse.projet
  const a = projet.patternActif
  projet = definirPas(projet, kick.id, 0)
  const nb = nouveauPattern(projet)
  projet = definirPas(nb.projet, caisse.id, 0)
  return { projet, a, b: nb.id }
}

describe('patterns', () => {
  it('nouveau pattern : nom suivant, une mesure, devient actif', () => {
    const p0 = creerProjet()
    expect(nomPatternSuivant(p0)).toBe('Pattern 2')
    const { projet, id } = nouveauPattern(p0)
    expect(projet.patterns.map((p) => p.nom)).toEqual(['Pattern 1', 'Pattern 2'])
    expect(projet.patternActif).toBe(id)
    expect(patternActif(projet).longueur).toBe(384)
  })

  it('dupliquer copie les notes, renommer, choisir, supprimer', () => {
    const { projet: p0, a, b } = deuxPatterns()
    const { projet: p1, id: copie } = dupliquerPattern(p0, a)
    expect(p1.patterns.map((p) => p.nom)).toEqual(['Pattern 1', 'Pattern 1 (copie)', 'Pattern 2'])
    expect(patternActif(p1).notes).toEqual(p0.patterns[0].notes)
    expect(patternActif(p1).notes).not.toBe(p0.patterns[0].notes) // copie indépendante
    const p2 = renommerPattern(p1, copie, '  Refrain ')
    expect(p2.patterns[1].nom).toBe('Refrain')
    expect(renommerPattern(p2, copie, '   ')).toBe(p2)
    expect(choisirPattern(p2, b).patternActif).toBe(b)
    expect(choisirPattern(p2, 'inconnu')).toBe(p2)
    // supprimer le pattern actif : le suivant devient actif ; ses clips disparaissent
    const avecClip = ajouterClip(p2, copie, 0, 0).projet
    const p3 = supprimerPattern(avecClip, copie)
    expect(p3.patterns.map((p) => p.nom)).toEqual(['Pattern 1', 'Pattern 2'])
    expect(p3.patternActif).toBe(b)
    expect(p3.playlist).toEqual([])
    expect(supprimerPattern(supprimerPattern(p3, a), b).patterns).toHaveLength(1) // jamais zéro
  })
})

describe('playlist', () => {
  it('poser, déplacer, retrouver et supprimer des clips', () => {
    const { projet: p0, a, b } = deuxPatterns()
    const c1 = ajouterClip(p0, a, 0, 0)
    const c2 = ajouterClip(c1.projet, b, 1, 384)
    let p = c2.projet
    expect(clipSous(p, 0, 100)?.id).toBe(c1.id)
    expect(clipSous(p, 0, 400)).toBeNull()
    expect(clipSous(p, 1, 384)?.id).toBe(c2.id)
    p = deplacerClip(p, c2.id, 3, 768)
    expect(p.playlist[1]).toMatchObject({ piste: 3, debut: 768 })
    expect(nombreDePistes(p)).toBe(8)
    p = deplacerClip(p, c2.id, 9, -50)
    expect(p.playlist[1]).toMatchObject({ piste: 9, debut: 0 })
    expect(nombreDePistes(p)).toBe(11)
    p = supprimerClip(p, c1.id)
    expect(p.playlist.map((c) => c.id)).toEqual([c2.id])
    expect(ajouterClip(p, 'inconnu', 0, 0).projet).toBe(p)
  })

  it('le morceau enchaîne les clips : longueur et événements décalés', () => {
    const { projet: p0, a, b } = deuxPatterns()
    let p = ajouterClip(p0, a, 0, 0).projet
    p = ajouterClip(p, b, 1, 384).projet
    p = ajouterClip(p, a, 0, 768).projet
    expect(longueurDuMorceau(p)).toBe(3 * 384)
    const attaques = evenementsDuMorceau(p).filter((e) => e.allume)
    expect(attaques.map((e) => [e.tic, e.hauteur])).toEqual([[0, 36], [384, 38], [768, 36]])
    // sans clip : une mesure de silence
    expect(longueurDuMorceau(creerProjet())).toBe(384)
    expect(evenementsDuMorceau(creerProjet())).toEqual([])
  })

  it('en mode morceau, le transport joue l’arrangement et boucle à sa fin', () => {
    const { projet: p0, a, b } = deuxPatterns()
    let p = ajouterClip(p0, a, 0, 0).projet
    p = ajouterClip(p, b, 0, 384).projet
    p = produce(p, (x) => {
      x.tempo = 120
    })
    const horloge = { t: 10, maintenant() { return this.t } }
    const attaques: number[] = []
    const minuterie: Minuterie = { demarrer: () => 1, arreter: () => {} }
    const transport = new Transport(horloge, { noteOn: (_c, h) => attaques.push(h), noteOff: () => {}, toutCouper: () => {} }, minuterie)
    transport.definirProjet(p, 'morceau')
    expect(transport.mode).toBe('morceau')
    expect(transport.longueur).toBe(768)
    transport.jouer()
    for (let i = 0; i < 360; i++) {
      horloge.t += 0.025 // 9 s : un morceau de deux mesures dure 4 s à 120, d'où 2 tours et un quart
      transport.programmer()
    }
    expect(attaques).toEqual([36, 38, 36, 38, 36])
    // repasser en mode pattern arrête et revient au début
    transport.definirProjet(p, 'pattern')
    expect(transport.etat).toBe('arret')
    expect(transport.longueur).toBe(384)
  })
})
