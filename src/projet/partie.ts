// « Jouer avec l'orchestre » : on choisit un canal, sa partie se tait et sert de guide, on la joue
// soi-même. Une note jouée est juste si la même hauteur était attendue à peu près au même moment.

import type { ModeLecture } from '../audio/transport'
import { TICS_PAR_PAS } from '../musique/temps'
import { patternActif, type Projet } from './modele'

export interface NoteAttendue {
  tic: number
  hauteur: number
}

/** Tolérance : un pas (une double croche) avant ou après. */
export const TOLERANCE = TICS_PAR_PAS

/** Notes que le canal joue sur toute la boucle (le pattern, ou l'arrangement en mode morceau). */
export function notesAttendues(projet: Projet, canalId: string, mode: ModeLecture): NoteAttendue[] {
  if (mode === 'pattern') return (patternActif(projet).notes[canalId] ?? []).map((n) => ({ tic: n.tic, hauteur: n.hauteur }))
  const resultat: NoteAttendue[] = []
  for (const clip of projet.playlist) {
    const pattern = projet.patterns.find((p) => p.id === clip.pattern)
    for (const n of pattern?.notes[canalId] ?? []) resultat.push({ tic: clip.debut + n.tic, hauteur: n.hauteur })
  }
  return resultat.sort((a, b) => a.tic - b.tic)
}

/** Distance entre deux instants d'une boucle de longueur donnée (la fin rejoint le début). */
export function distanceCirculaire(a: number, b: number, longueur: number): number {
  const d = Math.abs(a - b) % longueur
  return Math.min(d, longueur - d)
}

/** Vrai si une note jouée à `tic` (position dans la boucle) correspond à une note attendue. */
export function noteJuste(attendues: readonly NoteAttendue[], tic: number, hauteur: number, longueur: number, tolerance = TOLERANCE): boolean {
  return attendues.some((n) => n.hauteur === hauteur && distanceCirculaire(n.tic, tic, longueur) <= tolerance)
}
