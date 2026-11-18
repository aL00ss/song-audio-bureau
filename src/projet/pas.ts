// Séquenceur pas à pas. Un pas est une note du pattern : une double croche, à la hauteur par
// défaut du canal. Le piano roll (étape 4) édite donc les mêmes données.

import { produce } from 'immer'
import { TICS_PAR_PAS } from '../musique/temps'
import type { Canal, NoteMusicale, Pattern, Projet } from './modele'

export const PAS_MAX = 512
export const VELOCITE_PAS = 100
/** Durée d'une note de pas : un peu moins qu'un pas, pour que deux pas voisins se détachent. */
export const DUREE_PAS = TICS_PAR_PAS - 2

export const nombreDePas = (pattern: Pattern): number => Math.max(1, Math.ceil(pattern.longueur / TICS_PAR_PAS))

/**
 * Vrai si les notes du canal peuvent s'afficher en pas : toutes sur un début de pas et à la
 * hauteur par défaut, au plus une par pas. Un canal vide est en mode pas.
 */
export function estEnModePas(notes: readonly NoteMusicale[], canal: Pick<Canal, 'noteDefaut'>): boolean {
  const vus = new Set<number>()
  for (const n of notes) {
    if (n.hauteur !== canal.noteDefaut || n.tic % TICS_PAR_PAS !== 0 || vus.has(n.tic)) return false
    vus.add(n.tic)
  }
  return true
}

/** Vélocité de chaque pas (0 = pas inactif). */
export function velocitesDesPas(pattern: Pattern, canal: Canal): number[] {
  const pas = new Array<number>(nombreDePas(pattern)).fill(0)
  for (const n of pattern.notes[canal.id] ?? []) {
    const i = n.tic / TICS_PAR_PAS
    if (n.hauteur === canal.noteDefaut && Number.isInteger(i) && i < pas.length) pas[i] = n.velocite
  }
  return pas
}

function modifierPattern(projet: Projet, recette: (pattern: Pattern, canal: Canal) => void, canalId: string): Projet {
  return produce(projet, (p) => {
    const pattern = p.patterns.find((x) => x.id === p.patternActif)
    const canal = p.canaux.find((c) => c.id === canalId)
    if (pattern && canal) recette(pattern, canal)
  })
}

/** Active ou désactive un pas (sans `actif` : bascule). */
export function definirPas(projet: Projet, canalId: string, index: number, actif?: boolean): Projet {
  return modifierPattern(projet, (pattern, canal) => {
    if (index < 0 || index >= nombreDePas(pattern)) return
    const notes = (pattern.notes[canal.id] ??= [])
    const tic = index * TICS_PAR_PAS
    const position = notes.findIndex((n) => n.tic === tic && n.hauteur === canal.noteDefaut)
    const voulu = actif ?? position < 0
    if (voulu && position < 0) {
      notes.push({ tic, duree: DUREE_PAS, hauteur: canal.noteDefaut, velocite: VELOCITE_PAS })
      notes.sort((a, b) => a.tic - b.tic || a.hauteur - b.hauteur)
    } else if (!voulu && position >= 0) {
      notes.splice(position, 1)
    }
  }, canalId)
}

/** Vélocité d'un pas actif (1 à 127). */
export function definirVelocitePas(projet: Projet, canalId: string, index: number, velocite: number): Projet {
  const v = Math.min(127, Math.max(1, Math.round(velocite)))
  return modifierPattern(projet, (pattern, canal) => {
    const note = pattern.notes[canal.id]?.find((n) => n.tic === index * TICS_PAR_PAS && n.hauteur === canal.noteDefaut)
    if (note) note.velocite = v
  }, canalId)
}

/** Change la longueur du pattern actif, en pas (1 à 512). Les notes au-delà sont gardées mais ne jouent pas. */
export function definirNombreDePas(projet: Projet, pas: number): Projet {
  const n = Math.min(PAS_MAX, Math.max(1, Math.round(pas)))
  return produce(projet, (p) => {
    const pattern = p.patterns.find((x) => x.id === p.patternActif)
    if (pattern) pattern.longueur = n * TICS_PAR_PAS
  })
}

/**
 * Change la hauteur par défaut d'un canal (le son de batterie, par exemple). Si le canal est en
 * mode pas, ses pas suivent la nouvelle hauteur.
 */
export function definirNoteDefaut(projet: Projet, canalId: string, hauteur: number): Projet {
  const h = Math.min(127, Math.max(0, Math.round(hauteur)))
  return produce(projet, (p) => {
    const canal = p.canaux.find((c) => c.id === canalId)
    if (!canal) return
    for (const pattern of p.patterns) {
      const notes = pattern.notes[canalId]
      if (notes && estEnModePas(notes, canal)) for (const n of notes) n.hauteur = h
    }
    canal.noteDefaut = h
  })
}

/**
 * Swing : les notes posées sur un pas impair (la 2e double croche de chaque croche) sont
 * retardées. À 100 %, de la moitié d'un pas ; vers 67 %, on obtient le ternaire.
 */
export function decalageSwing(tic: number, swing: number): number {
  if (swing <= 0 || tic % (2 * TICS_PAR_PAS) !== TICS_PAR_PAS) return 0
  return Math.min(1, swing) * (TICS_PAR_PAS / 2)
}
