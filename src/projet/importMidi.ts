// Import d'un fichier MIDI (analysé par @tonejs/midi) : chaque piste devient un canal avec
// son instrument General MIDI, toutes les notes vont dans un pattern « Morceau ».

import type { Midi } from '@tonejs/midi'
import { produce } from 'immer'
import { CANAL_BATTERIE } from '../musique/gm'
import { arrondirALaMesure, TICS_PAR_NOIRE } from '../musique/temps'
import { ajouterCanal, creerPattern, creerProjet, type NoteMusicale, nouvelId, type Projet } from './modele'

const CONTROLEUR_PEDALE = 64

/** Intervalles [début, fin[ (en tics) pendant lesquels la pédale forte est enfoncée. */
export function intervallesPedale(evenements: { tic: number; enfoncee: boolean }[]): [number, number][] {
  const intervalles: [number, number][] = []
  let debut: number | null = null
  for (const e of [...evenements].sort((a, b) => a.tic - b.tic)) {
    if (e.enfoncee && debut === null) debut = e.tic
    else if (!e.enfoncee && debut !== null) {
      intervalles.push([debut, e.tic])
      debut = null
    }
  }
  if (debut !== null) intervalles.push([debut, Infinity])
  return intervalles
}

/**
 * Applique la pédale : une note relâchée pendant que la pédale est enfoncée continue jusqu'au
 * relâchement de la pédale, sauf si la même touche est rejouée avant. Le séquenceur n'envoie
 * pas de pédale : sa durée est inscrite dans les notes.
 */
export function appliquerPedale(notes: NoteMusicale[], intervalles: [number, number][]): NoteMusicale[] {
  const triees = [...notes].sort((a, b) => a.tic - b.tic || a.hauteur - b.hauteur)
  const fins = triees.map((n) => {
    const lache = n.tic + n.duree
    const tenue = intervalles.find(([d, f]) => d <= lache && lache < f)
    return tenue && Number.isFinite(tenue[1]) ? tenue[1] : lache
  })
  const prochaine = new Map<number, number>()
  for (let i = triees.length - 1; i >= 0; i--) {
    const suivante = prochaine.get(triees[i].hauteur)
    if (suivante !== undefined && suivante > triees[i].tic && fins[i] > suivante) fins[i] = suivante
    prochaine.set(triees[i].hauteur, triees[i].tic)
  }
  return triees.map((n, i) => ({ ...n, duree: Math.max(n.duree, fins[i] - n.tic, 1) }))
}

export function depuisMidi(midi: Midi, nom = 'Morceau importé'): Projet {
  const echelle = TICS_PAR_NOIRE / (midi.header.ppq || 480)
  const versTics = (t: number) => Math.round(t * echelle)
  let projet = creerProjet(nom)
  const tempo = midi.header.tempos[0]?.bpm
  const mesure = midi.header.timeSignatures[0]?.timeSignature
  projet = produce(projet, (p) => {
    if (tempo && Number.isFinite(tempo)) p.tempo = Math.round(tempo * 100) / 100
    if (mesure?.length === 2) p.signature = { numerateur: mesure[0], denominateur: mesure[1] }
  })

  const pattern = creerPattern('Morceau')
  let fin = 0
  for (const piste of midi.tracks) {
    if (!piste.notes.length) continue
    const batterie = piste.channel === CANAL_BATTERIE || piste.instrument.percussion
    const programme = Math.min(127, Math.max(0, piste.instrument.number || 0))
    const resultat = ajouterCanal(projet, {
      programme,
      batterie,
      nom: piste.name?.trim() || undefined,
    })
    projet = resultat.projet
    let notes: NoteMusicale[] = piste.notes.map((n) => ({
      tic: Math.max(0, versTics(n.ticks)),
      duree: Math.max(1, versTics(n.durationTicks)),
      hauteur: n.midi,
      velocite: Math.min(127, Math.max(1, Math.round(n.velocity * 127))),
    }))
    const pedale = (piste.controlChanges[CONTROLEUR_PEDALE] ?? []).map((cc) => ({
      tic: versTics(cc.ticks),
      enfoncee: cc.value >= 0.5,
    }))
    if (pedale.length && !batterie) notes = appliquerPedale(notes, intervallesPedale(pedale))
    else notes.sort((a, b) => a.tic - b.tic || a.hauteur - b.hauteur)
    for (const n of notes) fin = Math.max(fin, n.tic + n.duree)
    pattern.notes[resultat.id] = notes
  }
  pattern.longueur = arrondirALaMesure(fin, projet.signature)

  return produce(projet, (p) => {
    p.patterns = [pattern]
    p.patternActif = pattern.id
    p.playlist = [{ id: nouvelId('clip'), pattern: pattern.id, piste: 0, debut: 0 }]
  })
}
