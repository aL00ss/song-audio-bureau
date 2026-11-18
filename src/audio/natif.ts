// Client du moteur sonore Rust (application de bureau). L'interface lui envoie la configuration
// du mixer et des événements datés sur l'horloge du moteur ; elle lit en retour l'horloge, les
// crêtes des pistes et les voix des canaux, relevées toutes les MESURES_MS.

import { Channel, invoke } from '@tauri-apps/api/core'
import { ticsEnSecondes } from '../musique/temps'
import { longueurExport } from '../projet/exportMidi'
import { attribuerCanauxMidi, patternActif, type Projet } from '../projet/modele'
import type { BanqueSupplementaire } from './banques'
import type { MoteurAudio } from './interface'
import type { NiveauStereo } from './mixer'
import { routageMixer } from './routage'
import { pistesAudibles } from './niveaux'
import { type EtatTransport, evenementsDuMorceau, evenementsDuPattern, longueurDuMorceau, type ModeLecture, type TransportStudio } from './transport'

/** Canaux et pistes du moteur Rust. */
export const CANAUX_NATIFS = 32
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
  canaux: { canal: number; programme: number; batterie: boolean; volume: number; pan: number; piste: number; source: string }[]
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
    canaux.push({
      canal: midi,
      programme: canal.programme,
      batterie: canal.batterie,
      volume: canal.volume,
      pan: canal.pan,
      piste: routage.get(midi) ?? 0,
      source: canal.batterie ? 'soundfont' : canal.source,
    })
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

/** La session pour le transport du moteur : [début, fin, canal, hauteur, vélocité] par note. */
export interface SessionNative {
  tempo: number
  longueur: number
  temps_par_mesure: number
  notes: [number, number, number, number, number][]
}

/**
 * Ce que le transport du moteur doit jouer : les notes du pattern actif (mode pattern) ou de
 * tout l'arrangement (mode morceau), swing et muets appliqués, comme le transport web.
 */
export function sessionNative(projet: Projet, mode: ModeLecture): SessionNative {
  const pattern = patternActif(projet)
  const brut = mode === 'morceau' ? evenementsDuMorceau(projet) : evenementsDuPattern(projet, pattern)
  // les événements viennent par paires allumage/extinction : on les rassemble en notes
  const ouvertes = new Map<string, { tic: number; velocite: number }[]>()
  const notes: SessionNative['notes'] = []
  for (const e of brut) {
    const cle = `${e.canal}:${e.hauteur}`
    if (e.allume) {
      const pile = ouvertes.get(cle) ?? []
      pile.push({ tic: e.tic, velocite: e.velocite })
      ouvertes.set(cle, pile)
    } else {
      const debut = ouvertes.get(cle)?.shift()
      if (debut) notes.push([debut.tic, e.tic, e.canal, e.hauteur, Math.max(1, debut.velocite)])
    }
  }
  notes.sort((a, b) => a[0] - b[0])
  const longueur = mode === 'morceau' ? longueurDuMorceau(projet) : Math.max(1, pattern.longueur)
  return { tempo: projet.tempo, longueur, temps_par_mesure: projet.signature.numerateur, notes }
}

export interface LectureNative {
  etat: EtatTransport
  position: number
  position_absolue: number
  horloge: number
  tempo: number
  longueur: number
}

interface Mesures {
  horloge: number
  cretes: [number, number][]
  voix: number[]
  lecture: LectureNative
}

export class MoteurNatif implements MoteurAudio {
  readonly banquesSupplementaires = false
  readonly #frequence: number
  /** Appareil de sortie, ou null : pas de carte son (le moteur tourne sans rien faire entendre). */
  readonly sortie: string | null
  /** Décalage entre l'horloge du moteur et performance.now(), en secondes (lissé). */
  #decalage: number | null = null
  #dernierTemps = 0
  #mesures: Mesures = { horloge: 0, cretes: [], voix: [], lecture: { etat: 'arret', position: 0, position_absolue: 0, horloge: 0, tempo: 120, longueur: 384 } }
  #surMesures = new Set<(lecture: LectureNative) => void>()
  #lot: EvenementNatif[] = []
  #envoiPrevu = false

  private constructor(frequence: number, sortie: string | null) {
    this.#frequence = frequence
    this.sortie = sortie
  }

  static async creer(): Promise<MoteurNatif> {
    const infos = await invoke<{ frequence: number; sortie: string | null }>('moteur_demarrer')
    const moteur = new MoteurNatif(infos.frequence, infos.sortie)
    moteur.#recevoir(await invoke<Mesures>('moteur_mesures'))
    // ensuite, le moteur pousse ses mesures à la cadence de l'affichage
    const canal = new Channel<Mesures>()
    canal.onmessage = (mesures) => moteur.#recevoir(mesures)
    await invoke('moteur_abonner', { canal })
    return moteur
  }

  #recevoir(mesures: Mesures): void {
    // les crêtes sont celles accumulées depuis l'envoi précédent
    this.#mesures = mesures
    for (const ecouteur of this.#surMesures) ecouteur(mesures.lecture)
    const decalage = mesures.horloge / this.#frequence - performance.now() / 1000
    this.#decalage = this.#decalage === null ? decalage : this.#decalage + (decalage - this.#decalage) * 0.1
  }

  get frequence(): number {
    return this.#frequence
  }

  /** Dernier état du transport relevé dans le moteur. */
  get lecture(): LectureNative {
    return this.#mesures.lecture
  }

  surMesures(ecouteur: (lecture: LectureNative) => void): () => void {
    this.#surMesures.add(ecouteur)
    return () => this.#surMesures.delete(ecouteur)
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
    // rendu par le même transport que la lecture : une fois, puis la queue
    const octets = await invoke<ArrayBuffer>('moteur_rendre_wav', {
      configuration: configurationNative(projet),
      session: sessionNative(projet, mode),
      queue: QUEUE_EXPORT,
    })
    return new Blob([octets], { type: 'audio/wav' })
  }
}

/**
 * Transport tenu par le moteur Rust : l'interface ne fait que lui envoyer la session et les
 * ordres ; la position affichée est celle du moteur, prolongée entre deux relevés.
 */
export class TransportNatif implements TransportStudio {
  readonly metronomeDisponible = true
  readonly #moteur: MoteurNatif
  #mode: ModeLecture = 'pattern'
  #etat: EtatTransport = 'arret'
  #tempo = 120
  #longueur = 384
  /** Position connue localement (après un ordre, avant que le moteur ne la confirme). */
  #ancre: { position: number; temps: number } | null = { position: 0, temps: 0 }
  #ecouteurs = new Set<(etat: EtatTransport) => void>()

  constructor(moteur: MoteurNatif) {
    this.#moteur = moteur
    moteur.surMesures((lecture) => {
      // le moteur s'arrête seul en fin de rendu sans boucle ; ici, on suit son état
      if (lecture.etat !== this.#etat && this.#ancre === null) this.#changerEtat(lecture.etat)
      if (this.#ancre && lecture.etat === this.#etat) this.#ancre = null
    })
  }

  get etat(): EtatTransport {
    return this.#etat
  }
  get tempo(): number {
    return this.#tempo
  }
  get longueur(): number {
    return this.#longueur
  }
  get mode(): ModeLecture {
    return this.#mode
  }

  get positionAbsolue(): number {
    if (this.#ancre) return this.#ancre.position
    const l = this.#moteur.lecture
    if (l.etat !== 'lecture') return l.position_absolue
    // prolonge la position relevée jusqu'à maintenant, sur l'horloge du moteur
    const ecoule = Math.max(0, this.#moteur.maintenant() - l.horloge / this.#moteur.frequence)
    return l.position_absolue + (ecoule * l.tempo * 96) / 60
  }

  get position(): number {
    return ((this.positionAbsolue % this.#longueur) + this.#longueur) % this.#longueur
  }

  surEtat(ecouteur: (etat: EtatTransport) => void): () => void {
    this.#ecouteurs.add(ecouteur)
    return () => this.#ecouteurs.delete(ecouteur)
  }

  definirProjet(projet: Projet, mode: ModeLecture = this.#mode): void {
    if (mode !== this.#mode) {
      this.arreter()
      this.#mode = mode
    }
    const session = sessionNative(projet, mode)
    this.#tempo = session.tempo
    this.#longueur = session.longueur
    void invoke('moteur_session', { session })
  }

  #agir(action: object, etat: EtatTransport, position: number): void {
    this.#ancre = { position, temps: performance.now() }
    void invoke('moteur_transport', { action })
    this.#changerEtat(etat)
  }

  jouer(): void {
    if (this.#etat === 'lecture') return
    this.#agir({ type: 'jouer', decompte: false }, 'lecture', this.positionAbsolue)
    // pendant la lecture, la position vient du moteur dès le prochain relevé
    this.#ancre = null
  }

  pause(): void {
    if (this.#etat !== 'lecture') return
    this.#agir({ type: 'pause' }, 'pause', this.position)
  }

  arreter(): void {
    this.#agir({ type: 'arreter' }, 'arret', 0)
  }

  chercher(tic: number): void {
    const cible = ((tic % this.#longueur) + this.#longueur) % this.#longueur
    this.#ancre = { position: cible, temps: performance.now() }
    void invoke('moteur_transport', { action: { type: 'chercher', tic: cible } })
  }

  metronome(actif: boolean): void {
    void invoke('moteur_transport', { action: { type: 'metronome', actif } })
  }

  #changerEtat(etat: EtatTransport): void {
    if (etat === this.#etat) return
    this.#etat = etat
    for (const ecouteur of this.#ecouteurs) ecouteur(etat)
  }
}
