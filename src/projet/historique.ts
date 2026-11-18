// Historique des modifications du projet (annuler / rétablir). Fonctions pures : les projets
// sont immuables, l'historique ne garde que des références (le partage de structure d'immer
// rend chaque étape peu coûteuse).

import type { Projet } from './modele'

/** Étapes gardées au plus. */
export const HISTORIQUE_MAX = 300
/** Des modifications plus rapprochées que cela forment une seule étape (glisser un bouton…). */
export const REGROUPEMENT_MS = 400

export interface Historique {
  /** États précédents, du plus ancien au plus récent. */
  passe: readonly Projet[]
  /** États annulés, du plus proche au plus lointain. */
  futur: readonly Projet[]
  /** Instant de la dernière modification enregistrée (ms). */
  derniere: number
}

export const historiqueVide = (): Historique => ({ passe: [], futur: [], derniere: -Infinity })

/**
 * Enregistre qu'on vient de quitter l'état `avant`. Une modification qui suit de près la
 * précédente s'y fond (l'état d'avant le geste reste la seule étape) ; le futur est oublié.
 */
export function enregistrer(h: Historique, avant: Projet, instant: number): Historique {
  if (instant - h.derniere < REGROUPEMENT_MS && h.passe.length) return { ...h, futur: [], derniere: instant }
  const passe = [...h.passe, avant].slice(-HISTORIQUE_MAX)
  return { passe, futur: [], derniere: instant }
}

/** Revient à l'état précédent ; null s'il n'y en a pas. */
export function annuler(h: Historique, courant: Projet): { historique: Historique; projet: Projet } | null {
  const projet = h.passe.at(-1)
  if (!projet) return null
  return { projet, historique: { passe: h.passe.slice(0, -1), futur: [courant, ...h.futur], derniere: -Infinity } }
}

/** Refait l'état annulé le plus proche ; null s'il n'y en a pas. */
export function retablir(h: Historique, courant: Projet): { historique: Historique; projet: Projet } | null {
  const projet = h.futur[0]
  if (!projet) return null
  return { projet, historique: { passe: [...h.passe, courant], futur: h.futur.slice(1), derniere: -Infinity } }
}
