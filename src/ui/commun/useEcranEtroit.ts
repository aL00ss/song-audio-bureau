// Petit écran (téléphone) : le studio y montre un panneau à la fois.

import { useEffect, useState } from 'react'

/** Largeur sous laquelle le studio passe à un panneau à la fois. */
export const LARGEUR_ETROITE = 768

/** Vrai tant que la fenêtre est plus étroite que LARGEUR_ETROITE. */
export function useEcranEtroit(): boolean {
  const requete = `(max-width: ${LARGEUR_ETROITE - 1}px)`
  const [etroit, setEtroit] = useState(() => window.matchMedia(requete).matches)
  useEffect(() => {
    const media = window.matchMedia(requete)
    const surChangement = () => setEtroit(media.matches)
    media.addEventListener('change', surChangement)
    return () => media.removeEventListener('change', surChangement)
  }, [requete])
  return etroit
}
