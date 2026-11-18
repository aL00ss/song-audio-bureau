// Synthétiseurs intégrés au moteur de bureau. Le numéro de programme d'un canal « soustractif »
// choisit le préréglage ; la version navigateur, qui n'a pas ces synthés, joue à la place
// l'instrument General MIDI le plus proche.

import { nomInstrument } from './gm'

export type SourceCanal = 'soundfont' | 'soustractif'

export interface PrereglageSynthe {
  nom: string
  /** Instrument General MIDI le plus proche (version navigateur). */
  equivalentGM: number
}

/** Dans l'ordre des préréglages du moteur Rust (moteur/src/soustractif.rs). */
export const PREREGLAGES_SOUSTRACTIF: readonly PrereglageSynthe[] = [
  { nom: 'Basse ronde', equivalentGM: 38 },
  { nom: 'Lead acide', equivalentGM: 81 },
  { nom: 'Nappe chaude', equivalentGM: 89 },
  { nom: 'Cuivres synthé', equivalentGM: 62 },
  { nom: 'Pluck', equivalentGM: 84 },
  { nom: 'Sinus pur', equivalentGM: 80 },
]

/** Nom du son d'un canal mélodique (instrument General MIDI ou préréglage de synthé). */
export function nomDuSon(canal: { source: SourceCanal; programme: number }): string {
  if (canal.source === 'soustractif') return PREREGLAGES_SOUSTRACTIF[canal.programme % PREREGLAGES_SOUSTRACTIF.length].nom
  return nomInstrument(canal.programme)
}

/** Programme General MIDI à jouer quand le synthé intégré n'est pas disponible. */
export function programmeGM(canal: { source: SourceCanal; programme: number }): number {
  if (canal.source === 'soustractif') return PREREGLAGES_SOUSTRACTIF[canal.programme % PREREGLAGES_SOUSTRACTIF.length].equivalentGM
  return canal.programme
}
