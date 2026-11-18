// Banques de sons supplémentaires, plus fidèles que GeneralUser GS pour quelques instruments.
// Elles sont téléchargées à la demande, seulement quand le projet utilise un de leurs
// instruments ; en attendant, GeneralUser GS joue à leur place. Construites par outils/banques.

import type { Projet } from '../projet/modele'

export interface BanqueSupplementaire {
  id: string
  fichier: string
  nom: string
  /** Programmes General MIDI (banque 0) que cette banque remplace. */
  programmes: readonly number[]
}

/** Dans l'ordre de priorité : la première l'emporte, GeneralUser GS vient en dernier. */
export const BANQUES: readonly BanqueSupplementaire[] = [
  { id: 'piano', fichier: 'piano-salamander.sf3', nom: 'Piano à queue', programmes: [0] },
  { id: 'cordes', fichier: 'orchestre-cordes.sf3', nom: 'Cordes', programmes: [40, 41, 42, 43, 44, 45, 46, 48, 49] },
  { id: 'cuivres', fichier: 'orchestre-cuivres.sf3', nom: 'Cuivres', programmes: [56, 57, 58, 59, 60] },
  { id: 'bois', fichier: 'orchestre-bois.sf3', nom: 'Bois', programmes: [68, 70, 71, 72, 73] },
]

/** Identifiant de GeneralUser GS dans le synthétiseur. */
export const BANQUE_GM = 'gm'

/** Banques dont le projet a besoin (un canal mélodique joue un de leurs programmes). */
export function banquesNecessaires(projet: Projet, catalogue: readonly BanqueSupplementaire[] = BANQUES): BanqueSupplementaire[] {
  const programmes = new Set(projet.canaux.filter((c) => !c.batterie).map((c) => c.programme))
  return catalogue.filter((b) => b.programmes.some((p) => programmes.has(p)))
}

/** Ordre de priorité des banques chargées (les supplémentaires d'abord, dans l'ordre du catalogue). */
export function ordreDePriorite(chargees: Iterable<string>, catalogue: readonly BanqueSupplementaire[] = BANQUES): string[] {
  const ids = new Set(chargees)
  return [...catalogue.filter((b) => ids.has(b.id)).map((b) => b.id), BANQUE_GM]
}
