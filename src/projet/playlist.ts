// Patterns et playlist : créer, dupliquer, renommer, supprimer, choisir un pattern ; poser,
// déplacer, supprimer des clips. Fonctions pures (immer).

import { produce } from 'immer'
import { ticsParMesure } from '../musique/temps'
import { type ClipPlaylist, creerPattern, nouvelId, type Pattern, type Projet } from './modele'

export const PISTES_PLAYLIST_MIN = 8

/** « Pattern 2 », « Pattern 3 »… : le premier numéro libre. */
export function nomPatternSuivant(projet: Projet): string {
  const pris = new Set(projet.patterns.map((p) => p.nom))
  for (let n = projet.patterns.length + 1; ; n++) if (!pris.has(`Pattern ${n}`)) return `Pattern ${n}`
}

/** Nouveau pattern vide (une mesure), qui devient le pattern actif. */
export function nouveauPattern(projet: Projet): { projet: Projet; id: string } {
  const pattern = creerPattern(nomPatternSuivant(projet), ticsParMesure(projet.signature))
  return {
    projet: produce(projet, (p) => {
      p.patterns.push(pattern)
      p.patternActif = pattern.id
    }),
    id: pattern.id,
  }
}

/** Copie d'un pattern (notes comprises), qui devient le pattern actif. */
export function dupliquerPattern(projet: Projet, id: string): { projet: Projet; id: string } {
  const source = projet.patterns.find((p) => p.id === id)
  if (!source) return { projet, id }
  const copie: Pattern = { ...structuredClone(source), id: nouvelId('pattern'), nom: `${source.nom} (copie)` }
  return {
    projet: produce(projet, (p) => {
      p.patterns.splice(p.patterns.findIndex((x) => x.id === id) + 1, 0, copie)
      p.patternActif = copie.id
    }),
    id: copie.id,
  }
}

export function renommerPattern(projet: Projet, id: string, nom: string): Projet {
  const propre = nom.trim()
  if (!propre) return projet
  return produce(projet, (p) => {
    const pattern = p.patterns.find((x) => x.id === id)
    if (pattern) pattern.nom = propre
  })
}

export function choisirPattern(projet: Projet, id: string): Projet {
  if (!projet.patterns.some((p) => p.id === id) || projet.patternActif === id) return projet
  return { ...projet, patternActif: id }
}

/** Supprime un pattern et ses clips ; le dernier pattern ne peut pas être supprimé. */
export function supprimerPattern(projet: Projet, id: string): Projet {
  if (projet.patterns.length <= 1 || !projet.patterns.some((p) => p.id === id)) return projet
  return produce(projet, (p) => {
    const index = p.patterns.findIndex((x) => x.id === id)
    p.patterns.splice(index, 1)
    p.playlist = p.playlist.filter((c) => c.pattern !== id)
    if (p.patternActif === id) p.patternActif = p.patterns[Math.min(index, p.patterns.length - 1)].id
  })
}

export function ajouterClip(projet: Projet, pattern: string, piste: number, debut: number): { projet: Projet; id: string } {
  const id = nouvelId('clip')
  if (!projet.patterns.some((p) => p.id === pattern)) return { projet, id }
  const clip: ClipPlaylist = { id, pattern, piste: Math.max(0, Math.round(piste)), debut: Math.max(0, Math.round(debut)) }
  return { projet: produce(projet, (p) => void p.playlist.push(clip)), id }
}

export function deplacerClip(projet: Projet, id: string, piste: number, debut: number): Projet {
  return produce(projet, (p) => {
    const clip = p.playlist.find((c) => c.id === id)
    if (!clip) return
    clip.piste = Math.max(0, Math.round(piste))
    clip.debut = Math.max(0, Math.round(debut))
  })
}

export function supprimerClip(projet: Projet, id: string): Projet {
  if (!projet.playlist.some((c) => c.id === id)) return projet
  return produce(projet, (p) => {
    p.playlist = p.playlist.filter((c) => c.id !== id)
  })
}

/** Clip posé sur une piste à un instant (le plus récent au-dessus), ou null. */
export function clipSous(projet: Projet, piste: number, tic: number): ClipPlaylist | null {
  for (let i = projet.playlist.length - 1; i >= 0; i--) {
    const clip = projet.playlist[i]
    const pattern = projet.patterns.find((p) => p.id === clip.pattern)
    if (pattern && clip.piste === piste && tic >= clip.debut && tic < clip.debut + pattern.longueur) return clip
  }
  return null
}

/** Nombre de pistes à afficher : au moins 8, et une de plus que la dernière utilisée. */
export function nombreDePistes(projet: Projet): number {
  return Math.max(PISTES_PLAYLIST_MIN, ...projet.playlist.map((c) => c.piste + 2))
}
