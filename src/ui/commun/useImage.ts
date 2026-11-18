import { useEffect, useRef, useState } from 'react'

/** Valeur relue à chaque image d'animation (position du transport, voix actives…). */
export function useImage<T>(lire: () => T, egal: (a: T, b: T) => boolean = Object.is): T {
  const [valeur, setValeur] = useState(lire)
  const lecture = useRef(lire)
  const comparaison = useRef(egal)
  useEffect(() => {
    lecture.current = lire
    comparaison.current = egal
  })
  useEffect(() => {
    let id = 0
    const boucle = () => {
      const suivante = lecture.current()
      setValeur((precedente) => (comparaison.current(precedente, suivante) ? precedente : suivante))
      id = requestAnimationFrame(boucle)
    }
    id = requestAnimationFrame(boucle)
    return () => cancelAnimationFrame(id)
  }, [])
  return valeur
}

export const tableauxEgaux = <T,>(a: readonly T[], b: readonly T[]): boolean =>
  a.length === b.length && a.every((v, i) => Object.is(v, b[i]))
