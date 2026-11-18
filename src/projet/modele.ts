// Modèle du projet : canaux (instruments), patterns, playlist, pistes de mixage.
// Fonctions pures : chaque modification renvoie un nouveau projet (immer).

import { nomDuSon, type SourceCanal } from '../musique/synthes'
import { produce } from 'immer'
import { CANAL_BATTERIE } from '../musique/gm'
import { SIGNATURE_DEFAUT, type Signature, ticsParMesure } from '../musique/temps'

export const VERSION_PROJET = 1
export const NB_CANAUX_MIDI_MAX = 64

export interface NoteMusicale {
  /** Début, en tics depuis le début du pattern. */
  tic: number
  /** Durée en tics (au moins 1). */
  duree: number
  /** Numéro MIDI (0 à 127). */
  hauteur: number
  /** Vélocité MIDI (1 à 127). */
  velocite: number
}

export interface Canal {
  id: string
  nom: string
  /** Programme General MIDI (0 à 127). */
  programme: number
  batterie: boolean
  /** 0 à 1. */
  volume: number
  /** -1 (gauche) à 1 (droite). */
  pan: number
  muet: boolean
  solo: boolean
  /** Piste du mixer qui reçoit ce canal (0 = maître). */
  pisteMixage: number
  couleur: string
  /** Hauteur des pas du séquenceur : Do central, ou le son de batterie (36 grosse caisse…). */
  noteDefaut: number
  /** Ce qui produit le son : la banque General MIDI, ou un synthé intégré (bureau). */
  source: SourceCanal
}

export interface Pattern {
  id: string
  nom: string
  /** Longueur en tics. */
  longueur: number
  /** Notes par identifiant de canal. */
  notes: Record<string, NoteMusicale[]>
}

export interface ClipPlaylist {
  id: string
  pattern: string
  piste: number
  debut: number
}

export interface Egaliseur {
  /** Gain de chaque bande, en dB (-12 à +12). */
  grave: number
  medium: number
  aigu: number
}

export interface PisteMixage {
  index: number
  nom: string
  /** Position du fader, 0 à 1 (0,8 = 0 dB). */
  volume: number
  pan: number
  muet: boolean
  solo: boolean
  eq: Egaliseur
  /** Envoi vers la réverbération commune, 0 à 1. */
  reverb: number
}

export const EQ_NEUTRE: Egaliseur = { grave: 0, medium: 0, aigu: 0 }
export const REVERB_DEFAUT = 0.18

export function creerPiste(index: number, nom: string): PisteMixage {
  return { index, nom, volume: 0.8, pan: 0, muet: false, solo: false, eq: { ...EQ_NEUTRE }, reverb: index === 0 ? 0 : REVERB_DEFAUT }
}

export interface Projet {
  version: typeof VERSION_PROJET
  nom: string
  /** Noires par minute. */
  tempo: number
  /** Swing global, de 0 à 1 (voir pas.ts). */
  swing: number
  signature: Signature
  canaux: Canal[]
  patterns: Pattern[]
  patternActif: string
  playlist: ClipPlaylist[]
  mixage: PisteMixage[]
}

const COULEURS = ['#f5a524', '#4cc38a', '#5aa9f5', '#e5679a', '#a78bfa', '#f97362', '#2dd4bf', '#facc15']

let compteur = 0
/** Identifiant court et unique dans la session. */
export function nouvelId(prefixe: string): string {
  compteur += 1
  return `${prefixe}-${Date.now().toString(36)}-${compteur.toString(36)}`
}

export function creerPattern(nom = 'Pattern 1', longueur = ticsParMesure()): Pattern {
  return { id: nouvelId('pattern'), nom, longueur, notes: {} }
}

export function creerProjet(nom = 'Nouveau projet'): Projet {
  const pattern = creerPattern()
  return {
    version: VERSION_PROJET,
    nom,
    tempo: 120,
    swing: 0,
    signature: { ...SIGNATURE_DEFAUT },
    canaux: [],
    patterns: [pattern],
    patternActif: pattern.id,
    playlist: [],
    mixage: [creerPiste(0, 'Maître')],
  }
}

export interface OptionsCanal {
  nom?: string
  source?: SourceCanal
  programme?: number
  batterie?: boolean
  volume?: number
  pan?: number
  noteDefaut?: number
}

export const NOTE_DEFAUT_MELODIQUE = 60
export const NOTE_DEFAUT_BATTERIE = 36

/** Ajoute un canal et sa piste de mixage ; renvoie le projet et l'identifiant du canal. */
export function ajouterCanal(projet: Projet, options: OptionsCanal = {}): { projet: Projet; id: string } {
  const id = nouvelId('canal')
  const programme = options.programme ?? 0
  const batterie = options.batterie ?? false
  const suivant = produce(projet, (p) => {
    const index = p.mixage.length
    const source = batterie ? 'soundfont' : (options.source ?? 'soundfont')
    const nom = options.nom ?? (batterie ? 'Batterie' : nomDuSon({ source, programme }))
    p.mixage.push(creerPiste(index, nom))
    p.canaux.push({
      id,
      nom,
      programme,
      batterie,
      volume: options.volume ?? 0.8,
      pan: options.pan ?? 0,
      muet: false,
      solo: false,
      pisteMixage: index,
      couleur: COULEURS[p.canaux.length % COULEURS.length],
      noteDefaut: options.noteDefaut ?? (batterie ? NOTE_DEFAUT_BATTERIE : NOTE_DEFAUT_MELODIQUE),
      source,
    })
  })
  return { projet: suivant, id }
}

export function modifierCanal(projet: Projet, id: string, modifications: Partial<Omit<Canal, 'id'>>): Projet {
  return produce(projet, (p) => {
    const canal = p.canaux.find((c) => c.id === id)
    if (canal) Object.assign(canal, modifications)
  })
}

export function patternActif(projet: Projet): Pattern {
  const pattern = projet.patterns.find((p) => p.id === projet.patternActif)
  if (!pattern) throw new Error(`Pattern actif introuvable : ${projet.patternActif}`)
  return pattern
}

/** Canaux qui sonnent : si au moins un canal est en solo, seuls les solos ; sinon tous les non muets. */
export function canauxAudibles(canaux: readonly Canal[]): Set<string> {
  const solos = canaux.filter((c) => c.solo)
  const retenus = solos.length ? solos : canaux.filter((c) => !c.muet)
  return new Set(retenus.map((c) => c.id))
}

/**
 * Canal MIDI de chaque canal du projet. La première batterie va sur le canal 10 (indice 9),
 * comme le veut General MIDI ; chaque autre canal, batterie comprise, a son propre canal MIDI
 * (0, 1, 2… en sautant le 9), pour garder un volume et un panoramique indépendants. Le moteur
 * règle chaque canal MIDI en batterie ou non. Au-delà de 16, le synthétiseur ajoute des canaux.
 */
export function attribuerCanauxMidi(canaux: readonly Canal[]): Map<string, number> {
  const attribution = new Map<string, number>()
  const premiereBatterie = canaux.find((c) => c.batterie)
  let suivant = 0
  for (const canal of canaux) {
    if (canal === premiereBatterie) {
      attribution.set(canal.id, CANAL_BATTERIE)
      continue
    }
    if (suivant === CANAL_BATTERIE) suivant += 1
    if (suivant >= NB_CANAUX_MIDI_MAX) throw new Error(`Trop d'instruments : ${NB_CANAUX_MIDI_MAX - 1} au maximum`)
    attribution.set(canal.id, suivant)
    suivant += 1
  }
  return attribution
}

/** Notes du pattern dont la touche est enfoncée au tic donné, par canal. */
export function notesAuTic(pattern: Pattern, tic: number): Map<string, NoteMusicale[]> {
  const resultat = new Map<string, NoteMusicale[]>()
  for (const [canal, notes] of Object.entries(pattern.notes)) {
    const actives = notes.filter((n) => n.tic <= tic && tic < n.tic + n.duree)
    if (actives.length) resultat.set(canal, actives)
  }
  return resultat
}
