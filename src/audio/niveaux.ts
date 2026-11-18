// Calculs du mixer : décibels, loi du fader, pistes audibles (muet / solo). Fonctions pures.

import type { PisteMixage } from '../projet/modele'

/** Position du fader (0 à 1) qui donne le gain unité (0 dB). */
export const FADER_UNITE = 0.8

export const dbVersGain = (db: number): number => 10 ** (db / 20)
export const gainVersDb = (gain: number): number => (gain <= 0 ? -Infinity : 20 * Math.log10(gain))

/**
 * Loi du fader : quadratique, 0,8 = 0 dB, 1 = +3,9 dB, 0 = silence. La moitié de la course
 * donne environ -8 dB, ce qui laisse de la finesse dans la zone utile.
 */
export function gainDuFader(position: number): number {
  const p = Math.min(1, Math.max(0, position))
  return (p / FADER_UNITE) ** 2
}

export function texteDb(gain: number): string {
  const db = gainVersDb(gain)
  if (!Number.isFinite(db) || db < -60) return '-∞ dB'
  return `${db >= 0 ? '+' : ''}${db.toFixed(1)} dB`
}

/**
 * Pistes qui sonnent : si au moins une piste (hors maître) est en solo, seulement les solos ;
 * sinon toutes les pistes non muettes. Le maître (index 0) n'est jamais concerné par le solo.
 */
export function pistesAudibles(pistes: readonly PisteMixage[]): Set<number> {
  const autres = pistes.filter((p) => p.index !== 0)
  const solos = autres.filter((p) => p.solo)
  const retenues = solos.length ? solos : autres.filter((p) => !p.muet)
  return new Set(retenues.map((p) => p.index))
}

/** Niveau de crête en décibels, et part de la hauteur d'un vumètre allant de -60 à +6 dB. */
export function hauteurVumetre(crete: number): number {
  const db = gainVersDb(crete)
  if (!Number.isFinite(db)) return 0
  return Math.min(1, Math.max(0, (db + 60) / 66))
}
