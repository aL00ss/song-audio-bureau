// Géométrie d'un clavier de piano et correspondance avec le clavier de l'ordinateur.
// Fonctions pures, reprises des projets piano-js et piano-python.

export const NOTE_MIN = 21 // La0
export const NOTE_MAX = 108 // Do8
const NOIRES = new Set([1, 3, 6, 8, 10])
/** Décalage du centre de chaque touche noire par rapport à la jointure de ses voisines blanches. */
const DECALAGE_NOIRE: Record<number, number> = { 1: -0.07, 3: 0.07, 6: -0.1, 8: 0, 10: 0.1 }
export const LARGEUR_NOIRE = 0.58
export const HAUTEUR_NOIRE = 0.63

export const estNoire = (midi: number): boolean => NOIRES.has(((midi % 12) + 12) % 12)

export interface Touche {
  midi: number
  noire: boolean
  /** Bord gauche, en largeurs de touche blanche depuis la première touche. */
  x: number
  largeur: number
}

/** Touches de `debut` à `fin` (inclus) ; le clavier commence et finit sur une touche blanche. */
export function disposition(debut = NOTE_MIN, fin = NOTE_MAX): Touche[] {
  const touches: Touche[] = []
  let blanches = 0
  for (let midi = debut; midi <= fin; midi++) {
    if (estNoire(midi)) {
      const centre = blanches + DECALAGE_NOIRE[midi % 12]
      touches.push({ midi, noire: true, x: centre - LARGEUR_NOIRE / 2, largeur: LARGEUR_NOIRE })
    } else {
      touches.push({ midi, noire: false, x: blanches, largeur: 1 })
      blanches++
    }
  }
  return touches
}

export const nombreDeBlanches = (touches: readonly Touche[]): number => touches.filter((t) => !t.noire).length

/** Touche sous un point (x en largeurs de blanche, y de 0 en haut à 1 en bas), ou null. */
export function toucheSous(touches: readonly Touche[], x: number, y: number): number | null {
  if (y < 0 || y > 1) return null
  if (y <= HAUTEUR_NOIRE) {
    const noire = touches.find((t) => t.noire && x >= t.x && x < t.x + t.largeur)
    if (noire) return noire.midi
  }
  const blanche = touches.find((t) => !t.noire && x >= t.x && x < t.x + 1)
  return blanche?.midi ?? null
}

// ------------------------------------------------------- clavier de l'ordinateur
// Repéré par la position physique des touches (`event.code`) : la disposition est la même en
// AZERTY et en QWERTY, seules les lettres affichées changent.

const RANGEE_BASSE = ['KeyZ', 'KeyS', 'KeyX', 'KeyD', 'KeyC', 'KeyV', 'KeyG', 'KeyB', 'KeyH', 'KeyN', 'KeyJ', 'KeyM',
  'Comma', 'KeyL', 'Period', 'Semicolon', 'Slash']
const RANGEE_HAUTE = ['KeyQ', 'Digit2', 'KeyW', 'Digit3', 'KeyE', 'KeyR', 'Digit5', 'KeyT', 'Digit6', 'KeyY', 'Digit7',
  'KeyU', 'KeyI', 'Digit9', 'KeyO', 'Digit0', 'KeyP', 'BracketLeft', 'Equal', 'BracketRight']

/** Code physique -> demi-tons au-dessus du do de base. */
export const DEMI_TONS: ReadonlyMap<string, number> = new Map([
  ...RANGEE_BASSE.map((code, i) => [code, i] as const),
  ...RANGEE_HAUTE.map((code, i) => [code, 12 + i] as const),
])

export const BASE_DEFAUT = 48 // la rangée du haut commence au do central (60)
export const BASE_MIN = 24
export const BASE_MAX = 84

export type DispositionClavier = 'azerty' | 'qwerty'

const AZERTY: Record<string, string> = { KeyQ: 'A', KeyW: 'Z', KeyA: 'Q', KeyZ: 'W', KeyM: ',', Comma: ';', Period: ':', Slash: '!',
  Semicolon: 'M', Digit2: 'é', Digit3: '"', Digit5: '(', Digit6: '-', Digit7: 'è', Digit9: 'ç', Digit0: 'à',
  BracketLeft: '^', Equal: '=', BracketRight: '$' }
const QWERTY: Record<string, string> = { Comma: ',', Period: '.', Slash: '/', Semicolon: ';', BracketLeft: '[', Equal: '=', BracketRight: ']' }

export function caractere(code: string, clavier: DispositionClavier = 'azerty'): string {
  const table = clavier === 'azerty' ? AZERTY : QWERTY
  if (table[code]) return table[code]
  if (code.startsWith('Key')) return code.slice(3)
  if (code.startsWith('Digit')) return code.slice(5)
  return ''
}

/** Note jouée par une touche physique, ou null. */
export function noteDuCode(code: string, base = BASE_DEFAUT): number | null {
  const demiTons = DEMI_TONS.get(code)
  if (demiTons === undefined) return null
  const midi = base + demiTons
  return midi >= 0 && midi <= 127 ? midi : null
}

/** Note -> caractère à afficher sur la touche du piano. */
export function etiquettes(base = BASE_DEFAUT, clavier: DispositionClavier = 'azerty'): Map<number, string> {
  const resultat = new Map<number, string>()
  for (const code of [...RANGEE_BASSE, ...RANGEE_HAUTE]) {
    const midi = noteDuCode(code, base)
    if (midi !== null) resultat.set(midi, caractere(code, clavier))
  }
  return resultat
}

export function decalerBase(base: number, octaves: number): number {
  return Math.min(BASE_MAX, Math.max(BASE_MIN, base + 12 * octaves))
}

/** Devine la disposition du clavier, sinon d'après la langue du navigateur. */
export async function detecterDisposition(nav: Navigator | undefined = globalThis.navigator): Promise<DispositionClavier> {
  try {
    const clavier = (nav as Navigator & { keyboard?: { getLayoutMap?: () => Promise<Map<string, string>> } })?.keyboard
    const carte = await clavier?.getLayoutMap?.()
    const q = carte?.get('KeyQ')
    if (q) return q.toLowerCase() === 'a' ? 'azerty' : 'qwerty'
  } catch {
    // API indisponible ou refusée
  }
  return (nav?.language ?? 'fr').toLowerCase().startsWith('fr') ? 'azerty' : 'qwerty'
}
