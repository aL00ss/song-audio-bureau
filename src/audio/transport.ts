// Transport : l'horloge commune. Il programme les notes du pattern un peu à l'avance sur
// l'horloge audio (précise à l'échantillon) et tourne en boucle sur le pattern.
// Aucune dépendance à la Web Audio API : l'horloge, la destination et la minuterie sont
// injectées, ce qui permet de le tester entièrement hors navigateur.

import { attribuerCanauxMidi, canauxAudibles, type Pattern, type Projet } from '../projet/modele'
import { decalageSwing } from '../projet/pas'
import { arrondirALaMesure, secondesEnTics, ticsEnSecondes } from '../musique/temps'

export interface Destination {
  noteOn(canal: number, hauteur: number, velocite: number, temps: number): void
  noteOff(canal: number, hauteur: number, temps: number): void
  /** Coupe tout ce qui sonne ou est programmé. */
  toutCouper(): void
  /** Premier instant où de nouvelles notes peuvent être programmées sans être annulées. */
  disponibleA?(): number
}

export interface Horloge {
  /** Temps audio courant, en secondes. */
  maintenant(): number
}

export interface Minuterie {
  demarrer(rappel: () => void, periodeMs: number): unknown
  arreter(jeton: unknown): void
}

export type EtatTransport = 'arret' | 'lecture' | 'pause'

export interface Evenement {
  tic: number
  allume: boolean
  canal: number
  hauteur: number
  velocite: number
}

export const PERIODE_MS = 25
export const HORIZON = 0.12 // secondes programmées à l'avance
export const DELAI_DEPART = 0.05

/**
 * Événements note-on / note-off d'un pattern, triés par tic (à tic égal, les relâchements
 * d'abord). Les canaux muets (ou hors solo) sont ignorés ; les notes qui dépassent la fin du
 * pattern sont coupées à la fin ; le swing retarde les notes des pas impairs.
 */
export function evenementsDuPattern(projet: Projet, pattern: Pattern): Evenement[] {
  const canauxMidi = attribuerCanauxMidi(projet.canaux)
  const audibles = canauxAudibles(projet.canaux)
  const evenements: Evenement[] = []
  for (const canal of projet.canaux) {
    if (!audibles.has(canal.id)) continue
    const midi = canauxMidi.get(canal.id)
    if (midi === undefined) continue
    for (const note of pattern.notes[canal.id] ?? []) {
      if (note.tic >= pattern.longueur) continue
      const decalage = decalageSwing(note.tic, projet.swing ?? 0)
      const debut = note.tic + decalage
      const fin = Math.min(note.tic + note.duree + decalage, pattern.longueur)
      if (debut >= fin) continue
      evenements.push({ tic: debut, allume: true, canal: midi, hauteur: note.hauteur, velocite: note.velocite })
      evenements.push({ tic: fin, allume: false, canal: midi, hauteur: note.hauteur, velocite: 0 })
    }
  }
  return evenements.sort((a, b) => a.tic - b.tic || Number(a.allume) - Number(b.allume))
}

/** Mode de lecture : le pattern actif en boucle (PAT), ou tout l'arrangement de la playlist (SONG). */
export type ModeLecture = 'pattern' | 'morceau'

/** Fin du morceau : la fin du dernier clip, arrondie à la mesure (au moins une mesure). */
export function longueurDuMorceau(projet: Projet): number {
  let fin = 0
  for (const clip of projet.playlist) {
    const pattern = projet.patterns.find((p) => p.id === clip.pattern)
    if (pattern) fin = Math.max(fin, clip.debut + pattern.longueur)
  }
  return arrondirALaMesure(fin, projet.signature)
}

/** Événements de tout l'arrangement : ceux de chaque clip, décalés à son début. */
export function evenementsDuMorceau(projet: Projet): Evenement[] {
  const parPattern = new Map<string, Evenement[]>()
  const evenements: Evenement[] = []
  for (const clip of projet.playlist) {
    const pattern = projet.patterns.find((p) => p.id === clip.pattern)
    if (!pattern) continue
    if (!parPattern.has(pattern.id)) parPattern.set(pattern.id, evenementsDuPattern(projet, pattern))
    for (const e of parPattern.get(pattern.id)!) evenements.push({ ...e, tic: e.tic + clip.debut })
  }
  return evenements.sort((a, b) => a.tic - b.tic || Number(a.allume) - Number(b.allume))
}

export const minuterieNavigateur: Minuterie = {
  demarrer: (rappel, periodeMs) => setInterval(rappel, periodeMs),
  arreter: (jeton) => clearInterval(jeton as ReturnType<typeof setInterval>),
}

/** Ce que le studio attend d'un transport (celui du navigateur ou celui du moteur Rust). */
export interface TransportStudio {
  readonly etat: EtatTransport
  readonly tempo: number
  /** Longueur de la boucle, en tics. */
  readonly longueur: number
  /** Position absolue en tics, boucles comprises. */
  readonly positionAbsolue: number
  /** Position dans la boucle, en tics. */
  readonly position: number
  readonly mode: ModeLecture
  surEtat(ecouteur: (etat: EtatTransport) => void): () => void
  definirProjet(projet: Projet, mode?: ModeLecture): void
  jouer(): void
  pause(): void
  arreter(): void
  chercher(tic: number): void
  /** Métronome (transport du moteur Rust seulement). */
  readonly metronomeDisponible: boolean
  metronome(actif: boolean): void
}

export class Transport implements TransportStudio {
  readonly metronomeDisponible = false
  metronome(): void {
    // pas de métronome dans la version navigateur
  }

  #horloge: Horloge
  #destination: Destination
  #minuterie: Minuterie
  #jeton: unknown = null
  #projet: Projet | null = null
  #evenements: Evenement[] = []
  #longueur = 1
  #mode: ModeLecture = 'pattern'
  #tempo = 120
  #etat: EtatTransport = 'arret'
  /** Instant audio qui correspond au tic #origineTic. */
  #origineAudio = 0
  #origineTic = 0
  /** Tic absolu (boucles comprises) jusqu'auquel tout est programmé. */
  #ticProgramme = 0
  #ecouteurs = new Set<(etat: EtatTransport) => void>()

  constructor(horloge: Horloge, destination: Destination, minuterie: Minuterie = minuterieNavigateur) {
    this.#horloge = horloge
    this.#destination = destination
    this.#minuterie = minuterie
  }

  get etat(): EtatTransport {
    return this.#etat
  }

  get tempo(): number {
    return this.#tempo
  }

  /** Longueur de la boucle (le pattern actif), en tics. */
  get longueur(): number {
    return this.#longueur
  }

  /** Position absolue en tics, boucles comprises. */
  get positionAbsolue(): number {
    if (this.#etat !== 'lecture') return this.#origineTic
    const ecoule = Math.max(0, this.#horloge.maintenant() - this.#origineAudio)
    return this.#origineTic + secondesEnTics(ecoule, this.#tempo)
  }

  /** Position dans le pattern, en tics (0 ≤ position < longueur). */
  get position(): number {
    return this.positionAbsolue % this.#longueur
  }

  surEtat(ecouteur: (etat: EtatTransport) => void): () => void {
    this.#ecouteurs.add(ecouteur)
    return () => this.#ecouteurs.delete(ecouteur)
  }

  get mode(): ModeLecture {
    return this.#mode
  }

  /** Charge (ou recharge après une modification) le projet joué, dans le mode donné. */
  definirProjet(projet: Projet, mode: ModeLecture = this.#mode): void {
    const pattern = projet.patterns.find((p) => p.id === projet.patternActif)
    if (!pattern) throw new Error('Pattern actif introuvable')
    if (mode !== this.#mode) {
      // l'échelle du temps change (un pattern / tout le morceau) : on repart du début
      this.arreter()
      this.#mode = mode
    }
    if (this.#etat === 'lecture' && projet.tempo !== this.#tempo) this.#reancrer()
    this.#projet = projet
    this.#tempo = projet.tempo
    if (mode === 'morceau') {
      this.#longueur = longueurDuMorceau(projet)
      this.#evenements = evenementsDuMorceau(projet)
    } else {
      this.#longueur = Math.max(1, pattern.longueur)
      this.#evenements = evenementsDuPattern(projet, pattern)
    }
  }

  jouer(): void {
    if (!this.#projet || this.#etat === 'lecture') return
    // après un arrêt, des annulations peuvent encore être programmées : on démarre après elles
    this.#origineAudio = Math.max(this.#horloge.maintenant() + DELAI_DEPART, this.#destination.disponibleA?.() ?? 0)
    this.#ticProgramme = this.#origineTic
    this.#changerEtat('lecture')
    this.#jeton = this.#minuterie.demarrer(() => this.programmer(), PERIODE_MS)
    this.programmer()
  }

  pause(): void {
    if (this.#etat !== 'lecture') return
    const position = this.position
    this.#couper()
    this.#origineTic = position
    this.#changerEtat('pause')
  }

  arreter(): void {
    this.#couper()
    this.#origineTic = 0
    if (this.#etat !== 'arret') this.#changerEtat('arret')
  }

  /** Saute à un tic du pattern. */
  chercher(tic: number): void {
    const cible = ((tic % this.#longueur) + this.#longueur) % this.#longueur
    if (this.#etat === 'lecture') {
      this.#couper()
      this.#origineTic = cible
      this.#etat = 'pause'
      this.jouer()
    } else {
      this.#origineTic = cible
    }
  }

  /** Programme tous les événements qui tombent dans l'horizon à venir. */
  programmer(): void {
    if (this.#etat !== 'lecture') return
    const limite = this.#ticAuTemps(this.#horloge.maintenant() + HORIZON)
    const debut = this.#ticProgramme
    if (limite <= debut) return
    const L = this.#longueur
    // Le cycle précédent peut avoir des relâchements pile sur la frontière (tic == L).
    for (let cycle = Math.max(0, Math.floor(debut / L) - 1); cycle * L < limite; cycle++) {
      const base = cycle * L
      for (const e of this.#evenements) {
        const absolu = base + e.tic
        if (absolu < debut) continue
        if (absolu >= limite) break
        const temps = this.#origineAudio + ticsEnSecondes(absolu - this.#origineTic, this.#tempo)
        if (e.allume) this.#destination.noteOn(e.canal, e.hauteur, e.velocite, temps)
        else this.#destination.noteOff(e.canal, e.hauteur, temps)
      }
    }
    this.#ticProgramme = limite
  }

  #ticAuTemps(temps: number): number {
    return this.#origineTic + secondesEnTics(temps - this.#origineAudio, this.#tempo)
  }

  /** Le tempo change en cours de lecture : on repart de la position courante. */
  #reancrer(): void {
    const position = this.positionAbsolue
    this.#origineAudio = this.#horloge.maintenant()
    this.#origineTic = position
  }

  #couper(): void {
    if (this.#jeton !== null) this.#minuterie.arreter(this.#jeton)
    this.#jeton = null
    this.#destination.toutCouper()
  }

  #changerEtat(etat: EtatTransport): void {
    this.#etat = etat
    for (const ecouteur of this.#ecouteurs) ecouteur(etat)
  }
}
