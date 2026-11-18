// Ce que le studio attend d'un moteur sonore. Deux réalisations : MoteurSon (SpessaSynth et
// Web Audio, dans un navigateur) et MoteurNatif (le moteur Rust, dans l'application de bureau).

import type { Projet } from '../projet/modele'
import type { BanqueSupplementaire } from './banques'
import type { NiveauStereo } from './mixer'
import type { Destination, Horloge, ModeLecture } from './transport'

export type Progression = (recus: number, total: number) => void

export interface MoteurAudio extends Destination, Horloge {
  /** À appeler depuis un geste de l'utilisateur (les navigateurs bloquent le son avant). */
  demarrer(): Promise<void>
  noteOn(canal: number, hauteur: number, velocite: number, temps?: number): void
  noteOff(canal: number, hauteur: number, temps?: number): void
  /** Instruments, volumes, panoramiques, pistes et routage du mixer. */
  appliquerProjet(projet: Projet): void
  /** Crête stéréo d'une piste du mixer (0 = maître). */
  niveauPiste(index: number): NiveauStereo
  /** Niveau général (0 = silence). */
  niveau(): number
  /** Voix qui sonnent sur un canal MIDI. */
  voix(canal: number): number
  /** Ce moteur sait-il charger les banques supplémentaires (SF3) de la version web ? */
  readonly banquesSupplementaires: boolean
  chargerBanque(banque: BanqueSupplementaire, url: string, progression?: Progression): Promise<void>
  /** Rendu hors ligne du projet en WAV. */
  exporterWav(projet: Projet, mode: ModeLecture): Promise<Blob>
}
