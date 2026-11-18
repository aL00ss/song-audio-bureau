// Moteur sonore : un seul synthétiseur SpessaSynth (AudioWorklet) et une banque General MIDI.
// Chaque canal du projet joue sur son canal MIDI ; la batterie sur le canal 10.

import { WorkletSynthesizer } from 'spessasynth_lib'
import urlProcesseur from 'spessasynth_lib/dist/spessasynth_processor.min.js?url'
import { attribuerCanauxMidi, type Projet } from '../projet/modele'
import type { MoteurAudio } from './interface'
import { routageMixer } from './routage'
import type { ModeLecture } from './transport'
import { BANQUE_GM, type BanqueSupplementaire, ordreDePriorite } from './banques'
import { Mixeur, type NiveauStereo } from './mixer'

const CC_VOLUME = 7
const CC_PAN = 10
const CC_REVERB = 91 // la réverbération est celle du mixer : celle du synthétiseur est coupée
const CC_CHORUS = 93
/** Le synthétiseur a 16 sorties séparées, une par canal MIDI (au-delà, elles se partagent). */
const SORTIES_SEPAREES = 16
const CC_COUPER_LE_SON = 120 // All Sound Off : coupe net
/** Canaux MIDI créés dès le départ : de quoi tenir 16 instruments et plusieurs batteries. */
const CANAUX_DE_DEPART = 32

export type Progression = (recus: number, total: number) => void

/** Télécharge un fichier en suivant l'avancement (taille inconnue : total = 0). */
async function telecharger(url: string, progression?: Progression): Promise<ArrayBuffer> {
  const reponse = await fetch(url)
  if (!reponse.ok || !reponse.body) throw new Error(`Téléchargement impossible : ${url} (HTTP ${reponse.status})`)
  const total = Number(reponse.headers.get('content-length')) || 0
  const lecteur = reponse.body.getReader()
  const morceaux: Uint8Array[] = []
  let recus = 0
  for (;;) {
    const { done, value } = await lecteur.read()
    if (done) break
    morceaux.push(value)
    recus += value.length
    progression?.(recus, total)
  }
  const resultat = new Uint8Array(recus)
  let position = 0
  for (const m of morceaux) {
    resultat.set(m, position)
    position += m.length
  }
  return resultat.buffer
}


/**
 * Porte le synthétiseur à `total` canaux MIDI. Le processeur annonce les réglages initiaux d'un
 * nouveau canal avant de dire qu'il l'a créé, et la bibliothèque échoue sur ces annonces
 * (« Cannot read properties of undefined ») : on les fait taire le temps de la création, et
 * l'on inscrit nous-mêmes les canaux du côté de la bibliothèque.
 */
export function creerCanaux(synth: WorkletSynthesizer, total: number): void {
  const nombre = total - synth.midiChannels.length
  if (nombre <= 0) return
  synth.setSystemParameter('eventsEnabled', false)
  for (let i = 0; i < nombre; i++) {
    synth.addNewChannel()
    ;(synth as unknown as { addNewChannelInternal(): void }).addNewChannelInternal()
  }
  synth.setSystemParameter('eventsEnabled', true)
}

export { routageMixer }

export class MoteurSon implements MoteurAudio {
  readonly banquesSupplementaires = true
  readonly contexte: AudioContext
  readonly synth: WorkletSynthesizer
  /** Copie de la banque de sons (l'original est transféré au processeur), pour l'export WAV. */
  readonly banque: ArrayBuffer
  /** Copies des banques supplémentaires chargées, pour l'export WAV. */
  readonly #supplementaires = new Map<string, ArrayBuffer>()
  readonly #chargements = new Map<string, Promise<void>>()
  #dernierProjet: Projet | null = null
  /**
   * Canaux MIDI demandés au synthétiseur. addNewChannel() est asynchrone : midiChannels ne
   * s'allonge qu'à la confirmation du processeur, on ne peut donc pas s'y fier pour compter.
   */
  #canauxDemandes = CANAUX_DE_DEPART
  readonly mixeur: Mixeur
  readonly #analyseur: AnalyserNode
  readonly #onde = new Float32Array(1024)
  /** Notes programmées dans le futur : [canal, instant]. Voir toutCouper(). */
  #enAttente: [number, number][] = []
  #disponibleA = 0

  private constructor(contexte: AudioContext, synth: WorkletSynthesizer, banque: ArrayBuffer) {
    this.contexte = contexte
    this.synth = synth
    this.banque = banque
    // Chaque canal MIDI sort séparément vers sa tranche du mixer ; la sortie principale du
    // synthétiseur (tous les canaux mélangés) n'est pas utilisée.
    const sortie = contexte.createGain()
    this.#analyseur = contexte.createAnalyser()
    this.#analyseur.fftSize = 1024
    sortie.connect(this.#analyseur)
    sortie.connect(contexte.destination)
    this.mixeur = new Mixeur(contexte, synth, sortie)
    // un canal ajouté (au-delà de 16) se règle dès que la bibliothèque le connaît
    synth.eventHandler.addEvent('channelAdded', 'song-audio', () => {
      if (this.#dernierProjet) this.appliquerProjet(this.#dernierProjet)
    })
  }

  static async creer(urlBanque: string, progression?: Progression): Promise<MoteurSon> {
    const contexte = new AudioContext({ latencyHint: 'interactive' })
    const [banque] = await Promise.all([telecharger(urlBanque, progression), contexte.audioWorklet.addModule(urlProcesseur)])
    const copie = banque.slice(0)
    const synth = new WorkletSynthesizer(contexte)
    await synth.soundBankManager.addSoundBank(banque, BANQUE_GM)
    await synth.isReady
    creerCanaux(synth, CANAUX_DE_DEPART)
    return new MoteurSon(contexte, synth, copie)
  }

  /** À appeler depuis un geste de l'utilisateur : les navigateurs bloquent le son avant. */
  async demarrer(): Promise<void> {
    if (this.contexte.state !== 'running') await this.contexte.resume()
  }

  get actif(): boolean {
    return this.contexte.state === 'running'
  }

  maintenant(): number {
    return this.contexte.currentTime
  }

  /** Instruments, volumes et panoramiques de chaque canal ; pistes et routage du mixer. */
  appliquerProjet(projet: Projet): void {
    this.#dernierProjet = projet
    const attribution = attribuerCanauxMidi(projet.canaux)
    const plusHaut = Math.max(-1, ...attribution.values())
    for (; this.#canauxDemandes <= plusHaut; this.#canauxDemandes++) this.synth.addNewChannel()
    for (const canal of projet.canaux) {
      const midi = attribution.get(canal.id)
      // Canal pas encore créé côté bibliothèque : ses réglages renverraient des événements
      // qu'elle ne saurait pas placer ; il sera réglé à l'événement channelAdded.
      if (midi === undefined || midi >= this.synth.midiChannels.length) continue
      this.synth.midiChannels[midi].setDrums(canal.batterie)
      if (!canal.batterie) this.synth.programChange(midi, canal.programme)
      this.synth.controllerChange(midi, CC_VOLUME, Math.round(canal.volume * 127))
      this.synth.controllerChange(midi, CC_PAN, Math.round(((canal.pan + 1) / 2) * 127))
      this.synth.controllerChange(midi, CC_REVERB, 0)
      this.synth.controllerChange(midi, CC_CHORUS, 0)
    }
    this.mixeur.configurer(projet.mixage)
    this.mixeur.router(routageMixer(projet, SORTIES_SEPAREES))
  }

  /**
   * Télécharge une banque supplémentaire et la place avant GeneralUser GS ; les canaux qui
   * jouent un de ses instruments passent aussitôt dessus. Un seul téléchargement par banque.
   */
  chargerBanque(banque: BanqueSupplementaire, url: string, progression?: Progression): Promise<void> {
    let chargement = this.#chargements.get(banque.id)
    if (!chargement) {
      chargement = (async () => {
        const octets = await telecharger(url, progression)
        const copie = octets.slice(0)
        await this.synth.soundBankManager.addSoundBank(octets, banque.id)
        this.#supplementaires.set(banque.id, copie)
        this.synth.soundBankManager.priorityOrder = ordreDePriorite(this.#supplementaires.keys())
        if (this.#dernierProjet) this.appliquerProjet(this.#dernierProjet)
      })()
      chargement.catch(() => this.#chargements.delete(banque.id)) // un nouvel essai reste possible
      this.#chargements.set(banque.id, chargement)
    }
    return chargement
  }

  banqueChargee(id: string): boolean {
    return this.#supplementaires.has(id)
  }

  /** Banques chargées, de la plus prioritaire à GeneralUser GS (pour un rendu hors ligne). */
  banquesPourRendu(): ArrayBuffer[] {
    return ordreDePriorite(this.#supplementaires.keys()).map((id) => (id === BANQUE_GM ? this.banque : this.#supplementaires.get(id)!).slice(0))
  }

  async exporterWav(projet: Projet, mode: ModeLecture): Promise<Blob> {
    const { exporterWav } = await import('./exportWav')
    return exporterWav(this, projet, mode)
  }

  /** Crête stéréo d'une piste du mixer (0 = maître). */
  niveauPiste(index: number): NiveauStereo {
    return this.mixeur.niveau(index)
  }

  noteOn(canal: number, hauteur: number, velocite: number, temps?: number): void {
    if (temps !== undefined && temps > this.contexte.currentTime) {
      this.#enAttente.push([canal, temps])
      if (this.#enAttente.length > 2048) this.#oublierPassees()
      this.synth.noteOn(canal, hauteur, velocite, { time: temps })
    } else {
      this.synth.noteOn(canal, hauteur, velocite)
    }
  }

  noteOff(canal: number, hauteur: number, temps?: number): void {
    if (temps !== undefined && temps > this.contexte.currentTime) this.synth.noteOff(canal, hauteur, { time: temps })
    else this.synth.noteOff(canal, hauteur)
  }

  /**
   * Coupe ce qui sonne et annule ce qui est programmé. SpessaSynth ne permet pas de vider sa
   * file de messages datés : pour chaque note encore en attente, on programme au même instant
   * (juste après elle, le tri de la file étant stable) une coupure nette du canal (CC 120), et
   * la note n'est jamais entendue. Un simple relâchement (CC 123) ne suffit pas : mesuré, un
   * piano relâché dès son attaque sonne aussi fort qu'une note normale. Contrepartie : sur ces
   * canaux, la fin des notes relâchées par l'arrêt est coupée au plus HORIZON plus tard.
   */
  toutCouper(): void {
    const maintenant = this.contexte.currentTime
    this.synth.stopAll(false)
    const instants = new Map<string, [number, number]>()
    for (const [canal, temps] of this.#enAttente) if (temps > maintenant) instants.set(`${canal}:${temps}`, [canal, temps])
    for (const [canal, temps] of instants.values()) {
      this.synth.controllerChange(canal, CC_COUPER_LE_SON, 0, { time: temps })
      this.#disponibleA = Math.max(this.#disponibleA, temps + 0.005)
    }
    this.#enAttente = []
  }

  /** Premier instant où de nouvelles notes ne risquent plus d'être coupées par toutCouper(). */
  disponibleA(): number {
    return this.#disponibleA
  }

  /** Niveau efficace du mixage (0 = silence). */
  niveau(): number {
    this.#analyseur.getFloatTimeDomainData(this.#onde)
    let somme = 0
    for (const v of this.#onde) somme += v * v
    return Math.sqrt(somme / this.#onde.length)
  }

  /** Nombre de voix qui sonnent sur un canal MIDI. */
  voix(canal: number): number {
    return this.synth.midiChannels[canal]?.voiceCount ?? 0
  }

  #oublierPassees(): void {
    const maintenant = this.contexte.currentTime
    this.#enAttente = this.#enAttente.filter(([, t]) => t > maintenant)
  }
}
