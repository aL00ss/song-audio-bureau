// Client du moteur sonore Rust (application de bureau). L'interface lui envoie la configuration
// du mixer et des événements datés sur l'horloge du moteur ; elle lit en retour l'horloge, les
// crêtes des pistes et les voix des canaux, relevées toutes les MESURES_MS.

import { invoke } from '@tauri-apps/api/core'
import { ticsEnSecondes } from '../musique/temps'
import { longueurExport } from '../projet/exportMidi'
import { attribuerCanauxMidi, patternActif, type Projet } from '../projet/modele'
import type { BanqueSupplementaire } from './banques'
import type { MoteurAudio } from './interface'
import type { NiveauStereo } from './mixer'
import { routageMixer } from './routage'
import { pistesAudibles } from './niveaux'
import { evenementsDuMorceau, evenementsDuPattern, type ModeLecture } from './transport'

/** Canaux et pistes du moteur Rust. */
export const CANAUX_NATIFS = 32
const MESURES_MS = 40
/** Secondes ajoutées après la fin, pour laisser les sons s'éteindre (comme l'export web). */
export const QUEUE_EXPORT = 3

export interface EvenementNatif {
  temps: number
  canal: number
  hauteur: number
  /** 0 : relâcher la note. */
  velocite: number
}

export interface ConfigurationNative {
  canaux: { canal: number; programme: number; batterie: boolean; volume: number; pan: number; piste: number }[]
  pistes: { index: number; volume: number; pan: number; audible: boolean; eq: [number, number, number]; reverb: number }[]
}

/** Le projet vu par le moteur Rust : réglage de chaque canal MIDI et de chaque piste. */
export function configurationNative(projet: Projet): ConfigurationNative {
  const attribution = attribuerCanauxMidi(projet.canaux)
  const routage = routageMixer(projet, CANAUX_NATIFS)
  const canaux: ConfigurationNative['canaux'] = []
  for (const canal of projet.canaux) {
    const midi = attribution.get(canal.id)
    if (midi === undefined || midi >= CANAUX_NATIFS) continue
    canaux.push({ canal: midi, programme: canal.programme, batterie: canal.batterie, volume: canal.volume, pan: canal.pan, piste: routage.get(midi) ?? 0 })
  }
  const audibles = pistesAudibles(projet.mixage)
  const pistes = projet.mixage.map((p) => ({
    index: p.index,
    volume: p.volume,
    pan: p.pan,
    audible: p.index === 0 || audibles.has(p.index),
    eq: [p.eq.grave, p.eq.medium, p.eq.aigu] as [number, number, number],
    reverb: p.reverb,
  }))
  return { canaux, pistes }
}

/** Tout ce qui doit sonner pour l'export, en secondes depuis le début, et sa durée totale. */
export function evenementsExport(projet: Projet, mode: ModeLecture): { evenements: EvenementNatif[]; duree: number } {
  const brut = mode === 'morceau' ? evenementsDuMorceau(projet) : evenementsDuPattern(projet, patternActif(projet))
  const evenements = brut.map((e) => ({
    temps: ticsEnSecondes(e.tic, projet.tempo),
    canal: e.canal,
    hauteur: e.hauteur,
    velocite: e.allume ? Math.max(1, e.velocite) : 0,
  }))
  return { evenements, duree: ticsEnSecondes(longueurExport(projet, mode), projet.tempo) + QUEUE_EXPORT }
}

interface Mesures {
  horloge: number
  cretes: [number, number][]
  voix: number[]
}

export class MoteurNatif implements MoteurAudio {
  readonly banquesSupplementaires = false
  readonly #frequence: number
  /** Appareil de sortie, ou null : pas de carte son (le moteur tourne sans rien faire entendre). */
  readonly sortie: string | null
  /** Décalage entre l'horloge du moteur et performance.now(), en secondes (lissé). */
  #decalage: number | null = null
  #dernierTemps = 0
  #mesures: Mesures = { horloge: 0, cretes: [], voix: [] }
  #lot: EvenementNatif[] = []
  #envoiPrevu = false

  private constructor(frequence: number, sortie: string | null) {
    this.#frequence = frequence
    this.sortie = sortie
  }

  static async creer(): Promise<MoteurNatif> {
    const infos = await invoke<{ frequence: number; sortie: string | null }>('moteur_demarrer')
    const moteur = new MoteurNatif(infos.frequence, infos.sortie)
    await moteur.#mesurer()
    setInterval(() => void moteur.#mesurer(), MESURES_MS)
    return moteur
  }

  async #mesurer(): Promise<void> {
    try {
      const mesures = await invoke<Mesures>('moteur_mesures')
      // les crêtes sont celles accumulées depuis le relevé précédent
      this.#mesures = mesures
      const decalage = mesures.horloge / this.#frequence - performance.now() / 1000
      this.#decalage = this.#decalage === null ? decalage : this.#decalage + (decalage - this.#decalage) * 0.1
    } catch (erreur) {
      console.warn('Mesures du moteur indisponibles :', erreur)
    }
  }

  async demarrer(): Promise<void> {
    // la sortie du moteur Rust tourne depuis la création
  }

  maintenant(): number {
    const estime = performance.now() / 1000 + (this.#decalage ?? 0)
    this.#dernierTemps = Math.max(this.#dernierTemps, estime)
    return this.#dernierTemps
  }

  #envoyer(evenement: EvenementNatif): void {
    this.#lot.push(evenement)
    if (this.#envoiPrevu) return
    this.#envoiPrevu = true
    queueMicrotask(() => {
      this.#envoiPrevu = false
      const evenements = this.#lot
      this.#lot = []
      invoke('moteur_evenements', { evenements }).catch((e) => console.warn('Événements perdus :', e))
    })
  }

  noteOn(canal: number, hauteur: number, velocite: number, temps?: number): void {
    this.#envoyer({ temps: temps ?? this.maintenant(), canal, hauteur, velocite: Math.max(1, Math.min(127, Math.round(velocite))) })
  }

  noteOff(canal: number, hauteur: number, temps?: number): void {
    this.#envoyer({ temps: temps ?? this.maintenant(), canal, hauteur, velocite: 0 })
  }

  toutCouper(): void {
    this.#lot = []
    void invoke('moteur_tout_couper')
  }

  appliquerProjet(projet: Projet): void {
    void invoke('moteur_configurer', { configuration: configurationNative(projet) })
  }

  niveauPiste(index: number): NiveauStereo {
    const [gauche, droite] = this.#mesures.cretes[index] ?? [0, 0]
    return { gauche, droite }
  }

  niveau(): number {
    const [g, d] = this.#mesures.cretes[0] ?? [0, 0]
    return Math.max(g, d) * Math.SQRT1_2
  }

  voix(canal: number): number {
    return this.#mesures.voix[canal] ?? 0
  }

  chargerBanque(banque: BanqueSupplementaire): Promise<void> {
    return Promise.reject(new Error(`banque « ${banque.nom} » : à venir dans le moteur de bureau`))
  }

  async exporterWav(projet: Projet, mode: ModeLecture): Promise<Blob> {
    const { evenements, duree } = evenementsExport(projet, mode)
    const octets = await invoke<ArrayBuffer>('moteur_rendre_wav', { configuration: configurationNative(projet), evenements, duree })
    return new Blob([octets], { type: 'audio/wav' })
  }
}
