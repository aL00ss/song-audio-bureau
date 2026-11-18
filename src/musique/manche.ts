// Instruments à cordes et à manche : accordages, positions possibles d'une note, et choix des
// doigtés par programmation dynamique (le moins d'écart de main et de déplacement possible).

export interface Accordage {
  nom: string
  /** Notes des cordes à vide, de la plus grave à la plus aiguë. */
  cordes: readonly number[]
  /** Nombre de cases (ou de demi-tons jouables au-delà de la corde à vide). */
  cases: number
  frettes: boolean
}

export const ACCORDAGES = {
  guitare: { nom: 'Guitare', cordes: [40, 45, 50, 55, 59, 64], cases: 20, frettes: true },
  basse: { nom: 'Basse', cordes: [28, 33, 38, 43], cases: 20, frettes: true },
  violon: { nom: 'Violon', cordes: [55, 62, 69, 76], cases: 15, frettes: false },
  alto: { nom: 'Alto', cordes: [48, 55, 62, 69], cases: 15, frettes: false },
  violoncelle: { nom: 'Violoncelle', cordes: [36, 43, 50, 57], cases: 15, frettes: false },
  contrebasse: { nom: 'Contrebasse', cordes: [28, 33, 38, 43], cases: 15, frettes: false },
} as const satisfies Record<string, Accordage>

export type NomAccordage = keyof typeof ACCORDAGES

/** Accordage d'un programme General MIDI (null si ce n'est pas un instrument à manche). */
export function accordagePour(programme: number): NomAccordage | null {
  if (programme >= 24 && programme <= 31) return 'guitare'
  if (programme >= 32 && programme <= 39) return 'basse'
  if (programme === 104 || programme === 105 || programme === 106 || programme === 107) return 'guitare' // sitar, banjo, shamisen, koto
  if (programme === 40 || programme === 110) return 'violon'
  if (programme === 41) return 'alto'
  if (programme === 42) return 'violoncelle'
  if (programme === 43) return 'contrebasse'
  return null
}

export interface Position {
  corde: number
  case: number
}

/** Toutes les façons de jouer une note : (corde, case) avec 0 ≤ case ≤ cases. */
export function positionsPossibles(note: number, accordage: Accordage): Position[] {
  const resultat: Position[] = []
  accordage.cordes.forEach((vide, corde) => {
    const c = note - vide
    if (c >= 0 && c <= accordage.cases) resultat.push({ corde, case: c })
  })
  return resultat
}

export const ECART_MAX = 4 // cases entre l'index et l'auriculaire

/** Coût d'une position de main pour un accord (cases à vide exclues de l'écart). */
function coutAccord(positions: readonly Position[]): { cout: number; centre: number | null; ecart: number } {
  const appuyees = positions.filter((p) => p.case > 0).map((p) => p.case)
  if (!appuyees.length) return { cout: 0, centre: null, ecart: 0 }
  const min = Math.min(...appuyees)
  const max = Math.max(...appuyees)
  const ecart = max - min
  // écart impossible très pénalisé ; légère préférence pour le bas du manche (sinon, à coût égal,
  // une case 12 vaudrait une case 7) ; les cases au-delà de la 12e encore un peu plus
  const cout = (ecart > ECART_MAX ? 100 * (ecart - ECART_MAX) : 0) + ecart * 2 + min * 0.3 + Math.max(0, min - 12) * 0.4
  return { cout, centre: (min + max) / 2, ecart }
}

/**
 * Doigtés possibles d'un accord : une corde différente par note. Si l'accord a plus de notes que
 * de cordes possibles, les notes les plus graves en trop sont laissées de côté (indice -1).
 */
function combinaisons(notes: readonly number[], accordage: Accordage, limite = 400): (Position | null)[][] {
  const options = notes.map((n) => positionsPossibles(n, accordage))
  const resultats: (Position | null)[][] = []
  const courant: (Position | null)[] = []
  const prises = new Set<number>()
  const explorer = (i: number) => {
    if (resultats.length >= limite) return
    if (i === notes.length) {
      resultats.push([...courant])
      return
    }
    let place = false
    for (const p of options[i]) {
      if (prises.has(p.corde)) continue
      place = true
      prises.add(p.corde)
      courant.push(p)
      explorer(i + 1)
      courant.pop()
      prises.delete(p.corde)
    }
    if (!place) {
      courant.push(null) // note injouable ici (hors tessiture ou plus de corde libre)
      explorer(i + 1)
      courant.pop()
    }
  }
  explorer(0)
  return resultats
}

export interface Doigte {
  /** Position de chaque note de l'accord, dans l'ordre donné (null : non jouée sur le manche). */
  positions: (Position | null)[]
  ecart: number
}

/**
 * Doigtés d'une suite d'accords par programmation dynamique : pour chaque accord, le doigté qui
 * minimise l'écart de la main, les cases hautes et le déplacement depuis l'accord précédent.
 */
export function doigter(accords: readonly (readonly number[])[], accordage: Accordage): Doigte[] {
  type Etat = { positions: (Position | null)[]; cout: number; centre: number | null; ecart: number; precedent: number }
  const etapes: Etat[][] = []
  for (const accord of accords) {
    const candidats = combinaisons(accord, accordage)
    const avant = etapes.at(-1)
    const etats: Etat[] = candidats.map((positions) => {
      const placees = positions.filter((p): p is Position => p !== null)
      const local = coutAccord(placees)
      const manquantes = positions.length - placees.length
      let meilleur = 0
      let precedent = -1
      if (avant?.length) {
        meilleur = Infinity
        avant.forEach((e, i) => {
          const deplacement = e.centre === null || local.centre === null ? 0 : Math.abs(e.centre - local.centre) * 1.5
          if (e.cout + deplacement < meilleur) {
            meilleur = e.cout + deplacement
            precedent = i
          }
        })
      }
      return { positions, cout: meilleur + local.cout + manquantes * 1000, centre: local.centre ?? avant?.[precedent]?.centre ?? null, ecart: local.ecart, precedent }
    })
    etapes.push(etats.length ? etats : [{ positions: accord.map(() => null), cout: (avant?.[0]?.cout ?? 0) + 1000 * accord.length, centre: null, ecart: 0, precedent: 0 }])
  }
  // remontée du meilleur chemin
  const resultat: Doigte[] = new Array(etapes.length)
  let indice = etapes.length ? etapes.at(-1)!.reduce((m, e, i, t) => (e.cout < t[m].cout ? i : m), 0) : -1
  for (let k = etapes.length - 1; k >= 0; k--) {
    const etat = etapes[k][Math.max(0, indice)]
    resultat[k] = { positions: etat.positions, ecart: etat.ecart }
    indice = etat.precedent
  }
  return resultat
}

/** Position horizontale (0 à 1) d'une case sur un manche à frettes (tempérament égal). */
export function positionCase(c: number, cases: number): number {
  const longueur = (n: number) => 1 - 2 ** (-n / 12)
  return longueur(c) / longueur(cases + 1)
}
