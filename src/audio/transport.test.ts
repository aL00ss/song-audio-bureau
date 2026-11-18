import { describe, expect, it } from 'vitest'
import { creerDemo } from '../projet/demo'
import { ajouterCanal, creerProjet, modifierCanal, type Projet } from '../projet/modele'
import { DELAI_DEPART, type Destination, evenementsDuPattern, HORIZON, type Minuterie, Transport } from './transport'
import { produce } from 'immer'

interface Appel { type: 'on' | 'off' | 'coupe'; canal?: number; hauteur?: number; temps?: number }

function banc(projet: Projet) {
  const horloge = { t: 10, maintenant() { return this.t } }
  const appels: Appel[] = []
  const destination: Destination = {
    noteOn: (canal, hauteur, _v, temps) => appels.push({ type: 'on', canal, hauteur, temps }),
    noteOff: (canal, hauteur, temps) => appels.push({ type: 'off', canal, hauteur, temps }),
    toutCouper: () => appels.push({ type: 'coupe' }),
  }
  let minuterieActive = false
  const minuterie: Minuterie = { demarrer: () => { minuterieActive = true; return 1 }, arreter: () => { minuterieActive = false } }
  const transport = new Transport(horloge, destination, minuterie)
  transport.definirProjet(projet)
  /** Fait avancer le temps par pas de 25 ms, comme la minuterie réelle. */
  const avancer = (secondes: number) => {
    for (let fait = 0; fait < secondes - 1e-9; fait += 0.025) {
      horloge.t += 0.025
      transport.programmer()
    }
  }
  return { transport, horloge, appels, avancer, minuterieActive: () => minuterieActive }
}

/** Projet simple : un piano, une noire à chaque temps (60, 62, 64, 65), tempo 120. */
function gamme(): Projet {
  const r = ajouterCanal(creerProjet(), { programme: 0 })
  return produce(r.projet, (p) => {
    p.patterns[0].notes[r.id] = [60, 62, 64, 65].map((hauteur, i) => ({ tic: i * 96, duree: 48, hauteur, velocite: 100 }))
  })
}

describe('événements du pattern', () => {
  it('relâchements avant attaques à tic égal, notes coupées à la fin du pattern', () => {
    const r = ajouterCanal(creerProjet(), {})
    const p = produce(r.projet, (pr) => {
      pr.patterns[0].notes[r.id] = [
        { tic: 0, duree: 96, hauteur: 60, velocite: 90 },
        { tic: 96, duree: 1000, hauteur: 60, velocite: 90 },
        { tic: 400, duree: 10, hauteur: 70, velocite: 90 }, // après la fin (384) : ignorée
      ]
    })
    const e = evenementsDuPattern(p, p.patterns[0])
    expect(e.map((x) => [x.tic, x.allume])).toEqual([[0, true], [96, false], [96, true], [384, false]])
  })

  it('les canaux muets ne sont pas joués', () => {
    const demo = creerDemo()
    const muet = modifierCanal(demo, demo.canaux[3].id, { muet: true })
    const canaux = new Set(evenementsDuPattern(muet, muet.patterns[0]).map((e) => e.canal))
    expect(canaux).toEqual(new Set([0, 1, 2]))
    expect(new Set(evenementsDuPattern(demo, demo.patterns[0]).map((e) => e.canal))).toEqual(new Set([0, 1, 2, 9]))
  })
})

describe('transport', () => {
  it('programme chaque note à l’instant exact, peu à l’avance', () => {
    const { transport, appels, avancer } = banc(gamme())
    transport.jouer()
    const depart = 10 + DELAI_DEPART
    // au départ, seule la première note est dans l'horizon
    expect(appels.filter((a) => a.type === 'on').map((a) => a.hauteur)).toEqual([60])
    avancer(1.6)
    const attaques = appels.filter((a) => a.type === 'on')
    expect(attaques.map((a) => a.hauteur)).toEqual([60, 62, 64, 65])
    attaques.forEach((a, i) => expect(a.temps).toBeCloseTo(depart + i * 0.5, 9))
    const relachements = appels.filter((a) => a.type === 'off')
    relachements.forEach((a, i) => expect(a.temps).toBeCloseTo(depart + i * 0.5 + 0.25, 9))
    // jamais programmé plus loin que l'horizon
    for (const a of appels) if (a.temps !== undefined) expect(a.temps).toBeLessThanOrEqual(10 + 1.6 + HORIZON + 1e-9)
  })

  it('tourne en boucle sans perdre ni doubler de note', () => {
    const { transport, appels, avancer } = banc(gamme())
    transport.jouer()
    avancer(2 * 2 + 0.3) // deux tours complets (2 s chacun) et un peu plus
    const attaques = appels.filter((a) => a.type === 'on')
    expect(attaques.map((a) => a.hauteur)).toEqual([60, 62, 64, 65, 60, 62, 64, 65, 60])
    const temps = attaques.map((a) => a.temps!)
    temps.slice(1).forEach((t, i) => expect(t - temps[i]).toBeCloseTo(0.5, 9))
    expect(transport.position).toBeGreaterThanOrEqual(0)
    expect(transport.position).toBeLessThan(384)
  })

  it('pause, reprise au même endroit, arrêt au début', () => {
    const { transport, appels, avancer, horloge, minuterieActive } = banc(gamme())
    transport.jouer()
    avancer(1.05 + DELAI_DEPART) // 1,05 s de lecture : un peu après le 3e temps
    transport.pause()
    expect(transport.etat).toBe('pause')
    expect(minuterieActive()).toBe(false)
    expect(appels.at(-1)?.type).toBe('coupe')
    const position = transport.position
    expect(position).toBeCloseTo(192 + 0.05 * 192, 6)
    horloge.t += 30
    expect(transport.position).toBe(position)
    appels.length = 0
    transport.jouer()
    avancer(1)
    expect(appels.filter((a) => a.type === 'on').map((a) => a.hauteur)).toEqual([65, 60])
    transport.arreter()
    expect(transport.etat).toBe('arret')
    expect(transport.position).toBe(0)
  })

  it('chercher saute à un tic, en lecture comme à l’arrêt', () => {
    const { transport, appels, avancer } = banc(gamme())
    transport.chercher(288)
    expect(transport.position).toBe(288)
    transport.jouer()
    expect(appels.filter((a) => a.type === 'on').map((a) => a.hauteur)).toEqual([65])
    avancer(0.3)
    transport.chercher(96 + 384 * 5) // au-delà de la boucle : ramené dans le pattern
    expect(transport.etat).toBe('lecture')
    expect(appels.filter((a) => a.type === 'on').map((a) => a.hauteur).at(-1)).toBe(62)
  })

  it('un changement de tempo en lecture repart de la position courante', () => {
    const { transport, avancer } = banc(gamme())
    transport.jouer()
    avancer(0.5 + DELAI_DEPART)
    const avant = transport.position
    transport.definirProjet(produce(gamme(), (p) => { p.tempo = 60 }))
    expect(transport.tempo).toBe(60)
    expect(transport.position).toBeCloseTo(avant, 6)
    avancer(1)
    expect(transport.position).toBeCloseTo(avant + 96, 1) // une seconde à 60 = une noire
  })

  it('les écouteurs suivent l’état', () => {
    const { transport } = banc(gamme())
    const etats: string[] = []
    const retirer = transport.surEtat((e) => etats.push(e))
    transport.jouer()
    transport.pause()
    transport.jouer()
    transport.arreter()
    retirer()
    transport.jouer()
    expect(etats).toEqual(['lecture', 'pause', 'lecture', 'arret'])
  })

  it('après un arrêt, ne redémarre qu’après les annulations encore programmées', () => {
    const horloge = { t: 10, maintenant() { return this.t } }
    const attaques: number[] = []
    const destination: Destination = {
      noteOn: (_c, _h, _v, temps) => attaques.push(temps),
      noteOff: () => {},
      toutCouper: () => {},
      disponibleA: () => 10.2,
    }
    const transport = new Transport(horloge, destination, { demarrer: () => 1, arreter: () => {} })
    transport.definirProjet(gamme())
    transport.jouer()
    expect(attaques).toEqual([]) // 10,2 s est au-delà de l'horizon : rien n'est encore programmé
    horloge.t = 10.1
    transport.programmer()
    expect(attaques[0]).toBeCloseTo(10.2, 9) // et non 10 + DELAI_DEPART
  })
})
