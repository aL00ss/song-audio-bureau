// Édition des notes d'un canal (piano roll). Fonctions pures sur des listes de notes ; l'ordre
// des notes est conservé pendant une édition, pour que la sélection (des indices) reste valable.

import { TICS_PAR_PAS } from '../musique/temps'
import type { NoteMusicale } from './modele'

/** Pas de l'aimantation, en tics (0 = aucune). */
export const GRILLES: readonly { nom: string; tics: number }[] = [
  { nom: 'Mesure', tics: TICS_PAR_PAS * 16 },
  { nom: 'Temps', tics: TICS_PAR_PAS * 4 },
  { nom: '1/2 temps', tics: TICS_PAR_PAS * 2 },
  { nom: 'Pas (1/4 temps)', tics: TICS_PAR_PAS },
  { nom: '1/2 pas', tics: TICS_PAR_PAS / 2 },
  { nom: 'Aucune', tics: 0 },
]

export function aimanter(tic: number, grille: number): number {
  return grille > 0 ? Math.round(tic / grille) * grille : Math.round(tic)
}

/** Aimante vers le bas (pour placer une note là où l'on clique). */
export function aimanterBas(tic: number, grille: number): number {
  return grille > 0 ? Math.floor(tic / grille) * grille : Math.floor(tic)
}

const borner = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v))

/** Indice de la note sous un point (la plus récente au-dessus), ou -1. */
export function noteSous(notes: readonly NoteMusicale[], tic: number, hauteur: number): number {
  for (let i = notes.length - 1; i >= 0; i--) {
    const n = notes[i]
    if (n.hauteur === hauteur && tic >= n.tic && tic < n.tic + n.duree) return i
  }
  return -1
}

export function ajouterNote(notes: readonly NoteMusicale[], note: NoteMusicale): NoteMusicale[] {
  return [...notes, { ...note, hauteur: borner(note.hauteur, 0, 127), tic: Math.max(0, note.tic), duree: Math.max(1, note.duree), velocite: borner(note.velocite, 1, 127) }]
}

export function supprimerNotes(notes: readonly NoteMusicale[], indices: ReadonlySet<number>): NoteMusicale[] {
  return notes.filter((_, i) => !indices.has(i))
}

/** Déplace les notes choisies (le groupe ne sort pas du piano : 0 à 127, temps ≥ 0). */
export function deplacerNotes(notes: readonly NoteMusicale[], indices: ReadonlySet<number>, dTic: number, dHauteur: number): NoteMusicale[] {
  const choisies = notes.filter((_, i) => indices.has(i))
  if (!choisies.length) return [...notes]
  const ticMin = Math.min(...choisies.map((n) => n.tic))
  const basse = Math.min(...choisies.map((n) => n.hauteur))
  const haute = Math.max(...choisies.map((n) => n.hauteur))
  const dt = Math.max(-ticMin, dTic)
  const dh = borner(dHauteur, -basse, 127 - haute)
  return notes.map((n, i) => (indices.has(i) ? { ...n, tic: n.tic + dt, hauteur: n.hauteur + dh } : n))
}

/** Allonge ou raccourcit les notes choisies (au moins `minimum` tics). */
export function redimensionnerNotes(notes: readonly NoteMusicale[], indices: ReadonlySet<number>, dDuree: number, minimum = 1): NoteMusicale[] {
  return notes.map((n, i) => (indices.has(i) ? { ...n, duree: Math.max(minimum, n.duree + dDuree) } : n))
}

/** Ramène le début des notes choisies sur la grille (toutes si aucune n'est choisie). */
export function quantifier(notes: readonly NoteMusicale[], indices: ReadonlySet<number>, grille: number): NoteMusicale[] {
  if (grille <= 0) return [...notes]
  const toutes = indices.size === 0
  return notes.map((n, i) => (toutes || indices.has(i) ? { ...n, tic: aimanter(n.tic, grille) } : n))
}

export function definirVelocite(notes: readonly NoteMusicale[], indices: ReadonlySet<number>, velocite: number): NoteMusicale[] {
  const v = borner(Math.round(velocite), 1, 127)
  return notes.map((n, i) => (indices.has(i) ? { ...n, velocite: v } : n))
}

/** Indices des notes dont une partie est dans le rectangle [tic1, tic2[ × [hauteur1, hauteur2]. */
export function notesDansRectangle(notes: readonly NoteMusicale[], tic1: number, tic2: number, h1: number, h2: number): Set<number> {
  const [ta, tb] = tic1 <= tic2 ? [tic1, tic2] : [tic2, tic1]
  const [ha, hb] = h1 <= h2 ? [h1, h2] : [h2, h1]
  const resultat = new Set<number>()
  notes.forEach((n, i) => {
    if (n.hauteur >= ha && n.hauteur <= hb && n.tic < tb && n.tic + n.duree > ta) resultat.add(i)
  })
  return resultat
}

/** Hauteurs utilisées (pour centrer la vue), ou null s'il n'y a aucune note. */
export function etendue(notes: readonly NoteMusicale[]): { basse: number; haute: number } | null {
  if (!notes.length) return null
  return { basse: Math.min(...notes.map((n) => n.hauteur)), haute: Math.max(...notes.map((n) => n.hauteur)) }
}
