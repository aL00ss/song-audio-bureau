// Export du projet en fichier MIDI. Les notes viennent des mêmes événements que le transport :
// le swing, les canaux coupés et le mode (pattern ou morceau) sont donc respectés. Chaque canal
// du projet devient une piste avec son instrument General MIDI.

import { Midi } from '@tonejs/midi'
import { evenementsDuMorceau, evenementsDuPattern, longueurDuMorceau, type ModeLecture } from '../audio/transport'
import { CANAL_BATTERIE } from '../musique/gm'
import { TICS_PAR_NOIRE } from '../musique/temps'
import { attribuerCanauxMidi, patternActif, type Projet } from './modele'

const PPQ = 480
const ECHELLE = PPQ / TICS_PAR_NOIRE

export interface OptionsExport {
  mode: ModeLecture
  /**
   * Fichier standard (par défaut) : toutes les batteries sur le canal 10, comme le veut
   * General MIDI. Sinon, chaque canal garde son canal MIDI interne (rendu WAV du studio).
   */
  canauxInternes?: boolean
}

/** Longueur exportée, en tics du projet. */
export function longueurExport(projet: Projet, mode: ModeLecture): number {
  return mode === 'morceau' ? longueurDuMorceau(projet) : patternActif(projet).longueur
}

export function versMidi(projet: Projet, { mode, canauxInternes = false }: OptionsExport): Midi {
  const midi = new Midi()
  midi.header.setTempo(projet.tempo)
  midi.header.timeSignatures.push({ ticks: 0, timeSignature: [projet.signature.numerateur, projet.signature.denominateur] })
  midi.header.name = projet.nom
  midi.header.update()

  const evenements = mode === 'morceau' ? evenementsDuMorceau(projet) : evenementsDuPattern(projet, patternActif(projet))
  const attribution = attribuerCanauxMidi(projet.canaux)
  const canalParMidi = new Map<number, (typeof projet.canaux)[number]>()
  for (const canal of projet.canaux) {
    const m = attribution.get(canal.id)
    if (m !== undefined) canalParMidi.set(m, canal)
  }
  const pistes = new Map<number, ReturnType<Midi['addTrack']>>()
  const piste = (canalMidi: number) => {
    let p = pistes.get(canalMidi)
    if (!p) {
      const canal = canalParMidi.get(canalMidi)
      p = midi.addTrack()
      p.name = canal?.nom ?? `Canal ${canalMidi + 1}`
      p.channel = canauxInternes || !canal?.batterie ? canalMidi % 16 : CANAL_BATTERIE
      if (canal && !canal.batterie) p.instrument.number = canal.programme
      pistes.set(canalMidi, p)
    }
    return p
  }
  // les attaques et relâchements sont appariés note par note (premier entré, premier sorti)
  const ouvertes = new Map<string, { tic: number; velocite: number }[]>()
  for (const e of evenements) {
    const cle = `${e.canal}:${e.hauteur}`
    if (e.allume) {
      ouvertes.set(cle, [...(ouvertes.get(cle) ?? []), { tic: e.tic, velocite: e.velocite }])
      continue
    }
    const debut = ouvertes.get(cle)?.shift()
    if (!debut) continue
    piste(e.canal).addNote({
      midi: e.hauteur,
      ticks: Math.round(debut.tic * ECHELLE),
      durationTicks: Math.max(1, Math.round((e.tic - debut.tic) * ECHELLE)),
      velocity: debut.velocite / 127,
    })
  }
  // pistes dans l'ordre des canaux du projet
  midi.tracks.sort((a, b) => projet.canaux.findIndex((c) => c.nom === a.name) - projet.canaux.findIndex((c) => c.nom === b.name))
  return midi
}

export function versMidiOctets(projet: Projet, options: OptionsExport): Uint8Array {
  return versMidi(projet, options).toArray()
}

/** Nom de fichier sûr : « Démo » -> « Démo.mid ». */
export function nomDeFichier(nom: string, extension: string): string {
  const propre = nom.replace(/[\\/:*?"<>|]+/g, ' ').replace(/\s+/g, ' ').trim() || 'projet'
  return `${propre}.${extension}`
}
