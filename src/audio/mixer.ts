// Mixer audio (Web Audio). Chaque piste reçoit la sortie séparée des canaux MIDI qui lui sont
// routés, puis : égaliseur 3 bandes -> fader -> panoramique -> muet -> vumètre stéréo, avec un
// envoi (après le fader) vers une réverbération commune. Le maître : fader -> limiteur ->
// vumètre -> sortie.

import type { PisteMixage } from '../projet/modele'
import { gainDuFader, pistesAudibles } from './niveaux'

/** Les nœuds dont le mixer a besoin du synthétiseur. */
export interface SourceCanaux {
  connectChannel(cible: AudioNode, canal: number): AudioNode
  disconnectChannel(cible: AudioNode, canal: number): void
}

export interface NiveauStereo {
  gauche: number
  droite: number
}

class Vumetre {
  readonly entree: ChannelSplitterNode
  readonly #gauche: AnalyserNode
  readonly #droite: AnalyserNode
  readonly #tampon: Float32Array<ArrayBuffer>

  constructor(ctx: BaseAudioContext) {
    this.entree = ctx.createChannelSplitter(2)
    this.#gauche = ctx.createAnalyser()
    this.#droite = ctx.createAnalyser()
    this.#gauche.fftSize = this.#droite.fftSize = 1024
    this.entree.connect(this.#gauche, 0)
    this.entree.connect(this.#droite, 1)
    this.#tampon = new Float32Array(1024)
  }

  /** Crête de chaque côté sur le dernier bloc (≈ 23 ms). */
  crete(): NiveauStereo {
    const mesurer = (a: AnalyserNode) => {
      a.getFloatTimeDomainData(this.#tampon)
      let max = 0
      for (const v of this.#tampon) max = Math.max(max, Math.abs(v))
      return max
    }
    return { gauche: mesurer(this.#gauche), droite: mesurer(this.#droite) }
  }
}

/** Réponse impulsionnelle d'une salle : bruit stéréo qui décroît, aigus amortis plus vite. */
function reponseDeSalle(ctx: BaseAudioContext, duree = 2.2): AudioBuffer {
  const n = Math.round(duree * ctx.sampleRate)
  const tampon = ctx.createBuffer(2, n, ctx.sampleRate)
  let graine = 7
  const hasard = () => {
    graine = (graine * 16807) % 2147483647
    return graine / 2147483647 - 0.5
  }
  for (let c = 0; c < 2; c++) {
    const donnees = tampon.getChannelData(c)
    let doux = 0
    for (let i = 0; i < n; i++) {
      const t = i / ctx.sampleRate
      doux += (hasard() - doux) * (0.6 - 0.45 * Math.min(1, t / duree))
      donnees[i] = doux * Math.exp(-t / (duree / 6.9)) * (t < 0.006 ? 0 : 1)
    }
  }
  return tampon
}

const lisser = (param: AudioParam, valeur: number, ctx: BaseAudioContext) => param.setTargetAtTime(valeur, ctx.currentTime, 0.015)

class Tranche {
  readonly entree: GainNode
  readonly #grave: BiquadFilterNode
  readonly #medium: BiquadFilterNode
  readonly #aigu: BiquadFilterNode
  readonly #fader: GainNode
  readonly #pan: StereoPannerNode
  readonly #muet: GainNode
  readonly #envoi: GainNode
  readonly vumetre: Vumetre
  readonly sortie: GainNode

  constructor(ctx: BaseAudioContext, reverb: AudioNode) {
    this.entree = ctx.createGain()
    this.#grave = Object.assign(ctx.createBiquadFilter(), { type: 'lowshelf' as BiquadFilterType })
    this.#grave.frequency.value = 200
    this.#medium = Object.assign(ctx.createBiquadFilter(), { type: 'peaking' as BiquadFilterType })
    this.#medium.frequency.value = 1200
    this.#medium.Q.value = 0.8
    this.#aigu = Object.assign(ctx.createBiquadFilter(), { type: 'highshelf' as BiquadFilterType })
    this.#aigu.frequency.value = 5000
    this.#fader = ctx.createGain()
    this.#pan = ctx.createStereoPanner()
    this.#muet = ctx.createGain()
    this.#envoi = ctx.createGain()
    this.vumetre = new Vumetre(ctx)
    this.sortie = this.#muet
    this.entree.connect(this.#grave).connect(this.#medium).connect(this.#aigu).connect(this.#fader).connect(this.#pan).connect(this.#muet)
    this.#muet.connect(this.vumetre.entree)
    this.#muet.connect(this.#envoi)
    this.#envoi.connect(reverb)
  }

  regler(ctx: BaseAudioContext, piste: PisteMixage, audible: boolean): void {
    lisser(this.#fader.gain, gainDuFader(piste.volume), ctx)
    lisser(this.#pan.pan, piste.pan, ctx)
    lisser(this.#muet.gain, audible ? 1 : 0, ctx)
    lisser(this.#grave.gain, piste.eq.grave, ctx)
    lisser(this.#medium.gain, piste.eq.medium, ctx)
    lisser(this.#aigu.gain, piste.eq.aigu, ctx)
    lisser(this.#envoi.gain, piste.reverb, ctx)
  }

  deconnecter(): void {
    this.#muet.disconnect()
    this.#envoi.disconnect()
  }
}

export class Mixeur {
  readonly #ctx: BaseAudioContext
  readonly #source: SourceCanaux
  readonly #maitreEntree: GainNode
  readonly #maitreFader: GainNode
  readonly #limiteur: DynamicsCompressorNode
  readonly #vumetreMaitre: Vumetre
  readonly #reverb: ConvolverNode
  readonly #tranches = new Map<number, Tranche>()
  /** Canal MIDI -> nœud auquel sa sortie est réellement reliée (pour pouvoir l'en débrancher). */
  #routage = new Map<number, AudioNode>()

  constructor(ctx: BaseAudioContext, source: SourceCanaux, destination: AudioNode) {
    this.#ctx = ctx
    this.#source = source
    this.#maitreEntree = ctx.createGain()
    this.#maitreFader = ctx.createGain()
    this.#limiteur = ctx.createDynamicsCompressor()
    this.#limiteur.threshold.value = -3
    this.#limiteur.knee.value = 0
    this.#limiteur.ratio.value = 20
    this.#limiteur.attack.value = 0.002
    this.#limiteur.release.value = 0.15
    this.#vumetreMaitre = new Vumetre(ctx)
    this.#reverb = ctx.createConvolver()
    this.#reverb.buffer = reponseDeSalle(ctx)
    this.#reverb.connect(this.#maitreEntree)
    this.#maitreEntree.connect(this.#maitreFader).connect(this.#limiteur)
    this.#limiteur.connect(this.#vumetreMaitre.entree)
    this.#limiteur.connect(destination)
  }

  /** Crée, règle ou retire les tranches pour qu'elles correspondent aux pistes du projet. */
  configurer(pistes: readonly PisteMixage[]): void {
    const audibles = pistesAudibles(pistes)
    const voulues = new Set(pistes.filter((p) => p.index !== 0).map((p) => p.index))
    for (const [index, tranche] of this.#tranches) {
      if (!voulues.has(index)) {
        tranche.deconnecter()
        this.#tranches.delete(index)
      }
    }
    for (const piste of pistes) {
      if (piste.index === 0) {
        lisser(this.#maitreFader.gain, piste.muet ? 0 : gainDuFader(piste.volume), this.#ctx)
        continue
      }
      let tranche = this.#tranches.get(piste.index)
      if (!tranche) {
        tranche = new Tranche(this.#ctx, this.#reverb)
        tranche.sortie.connect(this.#maitreEntree)
        this.#tranches.set(piste.index, tranche)
      }
      tranche.regler(this.#ctx, piste, audibles.has(piste.index))
    }
  }

  /**
   * Route chaque canal MIDI vers sa piste (piste absente ou 0 : directement au maître).
   * Ne refait que les connexions qui changent, y compris quand une tranche a été recréée ;
   * à appeler après configurer().
   */
  router(routage: ReadonlyMap<number, number>): void {
    const entree = (piste: number) => this.#tranches.get(piste)?.entree ?? this.#maitreEntree
    for (const [canal, noeud] of this.#routage) {
      const piste = routage.get(canal)
      if (piste === undefined || entree(piste) !== noeud) {
        this.#source.disconnectChannel(noeud, canal)
        this.#routage.delete(canal)
      }
    }
    for (const [canal, piste] of routage) {
      if (this.#routage.has(canal)) continue
      const noeud = entree(piste)
      this.#source.connectChannel(noeud, canal)
      this.#routage.set(canal, noeud)
    }
  }

  /** Crête stéréo d'une piste (0 = maître), après fader, panoramique et muet. */
  niveau(piste: number): NiveauStereo {
    if (piste === 0) return this.#vumetreMaitre.crete()
    return this.#tranches.get(piste)?.vumetre.crete() ?? { gauche: 0, droite: 0 }
  }
}
