// Routage du mixer : quel canal MIDI va dans quelle piste. Fonction pure, commune aux deux moteurs.

import { attribuerCanauxMidi, type Projet } from '../projet/modele'

/**
 * Canal MIDI (sortie séparée : `sorties` au plus, au-delà elles se partagent) -> piste qui le
 * reçoit (0 = maître si la piste demandée n'existe pas).
 */
export function routageMixer(projet: Projet, sorties = 16): Map<number, number> {
  const attribution = attribuerCanauxMidi(projet.canaux)
  const pistes = new Set(projet.mixage.map((p) => p.index))
  const routage = new Map<number, number>()
  for (const canal of projet.canaux) {
    const midi = attribution.get(canal.id)
    if (midi !== undefined) routage.set(midi % sorties, pistes.has(canal.pisteMixage) ? canal.pisteMixage : 0)
  }
  return routage
}
