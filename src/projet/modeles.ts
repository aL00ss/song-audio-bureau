// Modèles de projet : des points de départ prêts à jouer, écrits dans le code (aucun contenu
// extérieur). Chaque modèle donne un projet neuf à chaque appel.

import { produce } from 'immer'
import { creerDemo } from './demo'
import { ajouterCanal, creerProjet, type NoteMusicale, type Projet } from './modele'
import { definirPas } from './pas'
import { ajouterClip } from './playlist'

export interface Modele {
  id: string
  nom: string
  description: string
  creer(): Projet
}

/** Un canal de batterie avec ses pas allumés (indices de doubles croches sur une mesure). */
function batterie(projet: Projet, nom: string, note: number, pas: number[]): Projet {
  const r = ajouterCanal(projet, { batterie: true, nom, noteDefaut: note })
  let p = r.projet
  for (const i of pas) p = definirPas(p, r.id, i, true)
  return p
}

/** Ajoute un instrument mélodique et ses notes dans le pattern actif. */
function instrument(projet: Projet, programme: number, nom: string, notes: NoteMusicale[], volume?: number): Projet {
  const r = ajouterCanal(projet, { programme, nom, volume })
  return produce(r.projet, (p) => {
    const pattern = p.patterns.find((x) => x.id === p.patternActif)!
    pattern.notes[r.id] = notes
  })
}

const n = (tic: number, duree: number, hauteur: number, velocite = 100): NoteMusicale => ({ tic, duree, hauteur, velocite })

/** Le pattern actif placé une fois dans l'arrangement. */
const avecClip = (p: Projet): Projet => ajouterClip(p, p.patternActif, 0, 0).projet

export const MODELES: readonly Modele[] = [
  {
    id: 'vide',
    nom: 'Vide',
    description: 'Quatre sons de batterie, rien de programmé',
    creer: () => {
      let p = creerProjet('Nouveau projet')
      for (const [nom, note] of [['Grosse caisse', 36], ['Clap', 39], ['Charleston', 42], ['Caisse claire', 38]] as const) {
        p = ajouterCanal(p, { batterie: true, nom, noteDefaut: note }).projet
      }
      return avecClip(p)
    },
  },
  {
    id: 'hip-hop',
    nom: 'Hip-hop',
    description: '90 BPM, swing, batterie et basse',
    creer: () => {
      let p = { ...creerProjet('Hip-hop'), tempo: 90, swing: 0.25 }
      p = batterie(p, 'Grosse caisse', 36, [0, 7, 10])
      p = batterie(p, 'Caisse claire', 38, [4, 12])
      p = batterie(p, 'Charleston', 42, [0, 2, 4, 6, 8, 10, 12, 14])
      p = instrument(p, 33, 'Basse', [n(0, 72, 36), n(84, 12, 36, 80), n(192, 96, 39), n(288, 48, 34)])
      return avecClip(p)
    },
  },
  {
    id: 'electro',
    nom: 'Électro',
    description: '128 BPM, grosse caisse sur chaque temps, nappe et mélodie',
    creer: () => {
      let p = { ...creerProjet('Électro'), tempo: 128 }
      p = batterie(p, 'Grosse caisse', 36, [0, 4, 8, 12])
      p = batterie(p, 'Clap', 39, [4, 12])
      p = batterie(p, 'Charleston ouverte', 46, [2, 6, 10, 14])
      p = instrument(p, 89, 'Nappe', [n(0, 384, 57, 70), n(0, 384, 60, 70), n(0, 384, 64, 70)], 0.6)
      p = instrument(p, 81, 'Mélodie', [0, 72, 144, 216, 288].map((t, i) => n(t, 36, [69, 72, 76, 72, 74][i], 95)))
      p = instrument(p, 38, 'Basse synthé', [0, 96, 192, 288].map((t) => n(t + 48, 36, 45, 105)))
      return avecClip(p)
    },
  },
  {
    id: 'orchestre',
    nom: 'Orchestre',
    description: 'Cordes, bois, cuivres, timbales : prêts à écrire',
    creer: () => {
      let p = { ...creerProjet('Orchestre'), tempo: 80 }
      for (const [programme, nom] of [[48, 'Cordes'], [42, 'Violoncelles'], [73, 'Flûte'], [68, 'Hautbois'], [60, 'Cors'], [47, 'Timbales']] as const) {
        p = ajouterCanal(p, { programme, nom }).projet
      }
      return avecClip(p)
    },
  },
  {
    id: 'piano',
    nom: 'Piano seul',
    description: 'Un piano à queue, pour composer',
    creer: () => avecClip(ajouterCanal({ ...creerProjet('Piano'), tempo: 72 }, { programme: 0, nom: 'Piano' }).projet),
  },
  {
    id: 'demo',
    nom: 'Démo',
    description: 'Quatre mesures sur quatre instruments',
    creer: creerDemo,
  },
]

export const modele = (id: string): Modele | undefined => MODELES.find((m) => m.id === id)
