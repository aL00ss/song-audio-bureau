// Export WAV : rendu hors ligne du projet avec le même synthétiseur (même banque, même état des
// canaux) et le même mixer (égaliseurs, panoramiques, réverbération, maître) que le studio.

import { BasicMIDI } from 'spessasynth_core'
import { audioBufferToWav, WorkletSynthesizer } from 'spessasynth_lib'
import urlProcesseur from 'spessasynth_lib/dist/spessasynth_processor.min.js?url'
import { ticsEnSecondes } from '../musique/temps'
import { longueurExport, versMidiOctets } from '../projet/exportMidi'
import type { Projet } from '../projet/modele'
import type { ModeLecture } from './transport'
import { Mixeur } from './mixer'
import { creerCanaux, type MoteurSon, routageMixer } from './moteur'

export const FREQUENCE_EXPORT = 44100
/** Secondes ajoutées après la dernière mesure, pour laisser les sons et la réverbération s'éteindre. */
export const QUEUE = 3

export function dureeExport(projet: Projet, mode: ModeLecture): number {
  return ticsEnSecondes(longueurExport(projet, mode), projet.tempo) + QUEUE
}

/** Rend le projet en audio stéréo (AudioBuffer). */
export async function rendre(moteur: MoteurSon, projet: Projet, mode: ModeLecture): Promise<AudioBuffer> {
  const octets = versMidiOctets(projet, { mode, canauxInternes: true })
  const contexte = new OfflineAudioContext({
    numberOfChannels: 2,
    length: Math.ceil(dureeExport(projet, mode) * FREQUENCE_EXPORT),
    sampleRate: FREQUENCE_EXPORT,
  })
  await contexte.audioWorklet.addModule(urlProcesseur)
  const synth = new WorkletSynthesizer(contexte)
  // autant de canaux que le synthétiseur du studio, avant que l'instantané ne les règle
  creerCanaux(synth, moteur.synth.midiChannels.length)
  const mixeur = new Mixeur(contexte, synth, contexte.destination)
  mixeur.configurer(projet.mixage)
  mixeur.router(routageMixer(projet))
  // l'état des canaux du studio (instruments, batteries, volumes, panoramiques, réverbération
  // interne coupée) est repris tel quel
  const instantane = await moteur.synth.getSnapshot()
  const tampon = octets.buffer.slice(octets.byteOffset, octets.byteOffset + octets.byteLength) as ArrayBuffer
  await synth.startOfflineRender({
    midiSequence: BasicMIDI.fromArrayBuffer(tampon, projet.nom),
    snapshot: instantane as Parameters<WorkletSynthesizer['startOfflineRender']>[0]['snapshot'],
    loopCount: 0,
    soundBankList: moteur.banquesPourRendu().map((soundBankBuffer) => ({ bankOffset: 0, soundBankBuffer })),
  })
  const rendu = await contexte.startRendering()
  synth.destroy()
  return rendu
}

export async function exporterWav(moteur: MoteurSon, projet: Projet, mode: ModeLecture): Promise<Blob> {
  // pas de normalisation : le fichier garde le niveau du mixage (le limiteur du maître évite
  // la saturation), comme on l'entend dans le studio
  return audioBufferToWav(await rendre(moteur, projet, mode), { normalizeAudio: false })
}
