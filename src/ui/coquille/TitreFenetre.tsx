// Garde le titre de la fenêtre à jour : nom du projet, et un point s'il n'est pas enregistré.

import { useEffect } from 'react'
import { estBureau, titreFenetre, useStudio } from '../../etat/studio'

export function TitreFenetre() {
  const titre = useStudio(titreFenetre)
  useEffect(() => {
    document.title = titre
    if (estBureau()) {
      void import('@tauri-apps/api/window').then(({ getCurrentWindow }) => getCurrentWindow().setTitle(titre)).catch(() => {})
    }
  }, [titre])
  return null
}
