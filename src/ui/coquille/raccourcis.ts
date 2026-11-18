// Raccourcis clavier du studio. Fonction pure : elle reçoit la touche et sa cible, et dit
// quelle action déclencher (ou aucune).

import { DEMI_TONS } from '../../musique/clavier'
import type { IdPanneau } from './panneaux'

export type Action =
  | { type: 'panneau'; id: IdPanneau }
  | { type: 'lecture' }
  | { type: 'stop' }
  /** Touche du clavier de l'ordinateur qui joue une note (code physique). */
  | { type: 'note'; code: string }
  | { type: 'octave'; sens: -1 | 1 }
  | { type: 'annuler' }
  | { type: 'retablir' }
  | { type: 'enregistrer' }
  | { type: 'enregistrerSous' }
  | { type: 'ouvrir' }

export interface Touche {
  key: string
  code: string
  ctrlKey: boolean
  metaKey: boolean
  altKey: boolean
  shiftKey: boolean
  repeat: boolean
  /** Nom de la balise qui a le focus (INPUT, TEXTAREA, BUTTON…), son type et son rôle éventuels. */
  cible: { balise: string; type?: string; editable?: boolean; role?: string }
}

const PANNEAUX_PAR_TOUCHE: Record<string, IdPanneau> = {
  F5: 'playlist',
  F6: 'rack',
  F7: 'pianoroll',
  F8: 'navigateur',
  F9: 'mixer',
}

const TYPES_SANS_SAISIE = new Set(['checkbox', 'radio', 'range', 'button', 'submit', 'color', 'file'])

/** Vrai si la touche sert à écrire dans un champ (le studio ne doit pas l'intercepter). */
export function saisieEnCours(cible: Touche['cible']): boolean {
  if (cible.editable) return true
  if (cible.balise === 'TEXTAREA' || cible.balise === 'SELECT') return true
  return cible.balise === 'INPUT' && !TYPES_SANS_SAISIE.has((cible.type ?? 'text').toLowerCase())
}

export function actionPourTouche(touche: Touche): Action | null {
  // Ctrl+Z, Ctrl+Y, Ctrl+Maj+Z (Cmd sur Mac) ; dans un champ de saisie, c'est le champ qui annule
  if ((touche.ctrlKey || touche.metaKey) && !touche.altKey && !saisieEnCours(touche.cible)) {
    if (touche.code === 'KeyZ') return { type: touche.shiftKey ? 'retablir' : 'annuler' }
    if (touche.code === 'KeyY' && !touche.shiftKey) return { type: 'retablir' }
  }
  // Ctrl+S, Ctrl+Maj+S, Ctrl+O : même dans un champ (comme partout ailleurs)
  if ((touche.ctrlKey || touche.metaKey) && !touche.altKey) {
    if (touche.code === 'KeyS') return { type: touche.shiftKey ? 'enregistrerSous' : 'enregistrer' }
    if (touche.code === 'KeyO' && !touche.shiftKey) return { type: 'ouvrir' }
  }
  if (touche.ctrlKey || touche.metaKey || touche.altKey) return null
  const panneau = PANNEAUX_PAR_TOUCHE[touche.code]
  if (panneau) return touche.repeat ? null : { type: 'panneau', id: panneau }
  if (saisieEnCours(touche.cible)) return null
  if (touche.code === 'Space') return touche.repeat ? null : touche.shiftKey ? { type: 'stop' } : { type: 'lecture' }
  // les flèches règlent un curseur (bouton rotatif…) qui a le focus : on ne les prend pas
  const surCurseur = touche.cible.type === 'range' || touche.cible.role === 'slider'
  if ((touche.code === 'ArrowLeft' || touche.code === 'ArrowRight') && !surCurseur) {
    return touche.repeat ? null : { type: 'octave', sens: touche.code === 'ArrowLeft' ? -1 : 1 }
  }
  if (DEMI_TONS.has(touche.code) && !touche.shiftKey) return { type: 'note', code: touche.code }
  return null
}

export function toucheDepuisEvenement(e: KeyboardEvent): Touche {
  const element = e.target instanceof HTMLElement ? e.target : null
  return {
    key: e.key,
    code: e.code,
    ctrlKey: e.ctrlKey,
    metaKey: e.metaKey,
    altKey: e.altKey,
    shiftKey: e.shiftKey,
    repeat: e.repeat,
    cible: {
      balise: element?.tagName ?? '',
      type: element instanceof HTMLInputElement ? element.type : undefined,
      editable: element?.isContentEditable ?? false,
      role: element?.getAttribute('role') ?? undefined,
    },
  }
}
