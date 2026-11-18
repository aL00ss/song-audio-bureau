// Enregistrer et ouvrir un projet, selon la plateforme (bureau ou navigateur).

import { estBureau, useStudio } from '../../etat/studio'
import { nomDeFichier } from '../../projet/exportMidi'
import { telecharger } from '../commun/telecharger'

export const EXTENSION_PROJET = 'song-audio.json'

/** Enregistre : dans un fichier sur le bureau, par un téléchargement dans le navigateur. */
export async function enregistrerOuTelecharger(sous: boolean): Promise<void> {
  const etat = useStudio.getState()
  if (estBureau()) {
    await (sous ? etat.enregistrerSous() : etat.enregistrer())
    return
  }
  telecharger(etat.projetEnJson(), nomDeFichier(etat.projet.nom, EXTENSION_PROJET), 'application/json')
  useStudio.setState({ modifie: false })
}

/** Ouvre : boîte de dialogue du système sur le bureau, sélecteur de fichier dans le navigateur. */
export function ouvrirProjetSelonPlateforme(): void {
  if (estBureau()) void useStudio.getState().ouvrirDepuisDisque()
  else document.getElementById('ouvrir-projet')?.click()
}
