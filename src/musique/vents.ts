// Cuivres à pistons et harpe : doigtés et cordes.

/**
 * Pistons d'une trompette en si bémol pour une note écrite en son réel (MIDI). Chaque partiel
 * (harmonique) de l'instrument se joue à vide, puis les pistons abaissent le son :
 * 2 = un demi-ton, 1 = un ton, 1+2 ou 3 = un ton et demi, 2+3 = deux tons, 1+3 = deux tons et
 * demi, 1+2+3 = trois tons. On choisit le partiel le plus proche au-dessus de la note.
 */
// Notes à vide d'une trompette en si♭, en son réel : 2e partiel si♭3, puis fa4, si♭4, ré5, fa5,
// la♭5 (trop bas en pratique, mais jouable), si♭5. Sa note la plus grave est mi3 (trois pistons).
const PARTIELS_TROMPETTE = [58, 65, 70, 74, 77, 80, 82]
const PISTONS_PAR_DEMI_TONS: readonly (readonly number[])[] = [[], [2], [1], [1, 2], [2, 3], [1, 3], [1, 2, 3]]

export function pistons(note: number, partiels: readonly number[] = PARTIELS_TROMPETTE): number[] | null {
  for (const partiel of partiels) {
    const ecart = partiel - note
    if (ecart >= 0 && ecart <= 6) return [...PISTONS_PAR_DEMI_TONS[ecart]]
  }
  return null
}

/** Tessitures, en son réel, des cuivres à pistons (au-delà : hors de l'instrument). */
export const CUIVRES_A_PISTONS: Readonly<Record<number, { nom: string; partiels: readonly number[] }>> = {
  56: { nom: 'Trompette', partiels: PARTIELS_TROMPETTE },
  58: { nom: 'Tuba', partiels: PARTIELS_TROMPETTE.map((p) => p - 24) },
  59: { nom: 'Trompette bouchée', partiels: PARTIELS_TROMPETTE },
  60: { nom: 'Cor', partiels: [41, 48, 53, 57, 60, 63, 65, 67, 69, 71, 72, 74] },
  61: { nom: 'Section de cuivres', partiels: PARTIELS_TROMPETTE },
}

/** Cordes d'une harpe de concert : diatonique, de do1 (24) à sol7 (103). */
export const CORDES_HARPE: readonly number[] = (() => {
  const gamme = [0, 2, 4, 5, 7, 9, 11]
  const cordes: number[] = []
  for (let n = 24; n <= 103; n++) if (gamme.includes(n % 12)) cordes.push(n)
  return cordes
})()

/** Corde de harpe la plus proche (les altérations se font aux pédales : on garde la corde du dessous). */
export function cordeHarpe(note: number): number {
  let choisie = CORDES_HARPE[0]
  for (const c of CORDES_HARPE) if (c <= note) choisie = c
  return choisie
}
