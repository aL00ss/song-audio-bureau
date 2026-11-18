// Fichiers de projet dans l'application de bureau : vraies boîtes de dialogue du système,
// lecture et écriture directes (le navigateur, lui, télécharge et importe).

import { invoke } from '@tauri-apps/api/core'
import { open, save } from '@tauri-apps/plugin-dialog'

const FILTRES = [{ name: 'Projet song-audio', extensions: ['song-audio.json', 'json'] }]

/** Demande un projet à ouvrir ; null si l'utilisateur renonce. */
export async function choisirProjet(): Promise<{ chemin: string; contenu: string } | null> {
  const chemin = await open({ title: 'Ouvrir un projet', multiple: false, directory: false, filters: FILTRES })
  if (typeof chemin !== 'string') return null
  return { chemin, contenu: await invoke<string>('projet_lire', { chemin }) }
}

/** Demande où enregistrer ; null si l'utilisateur renonce. */
export async function choisirEmplacement(nomPropose: string): Promise<string | null> {
  const chemin = await save({ title: 'Enregistrer le projet', defaultPath: nomPropose, filters: FILTRES })
  if (!chemin) return null
  return chemin.toLowerCase().endsWith('.json') ? chemin : `${chemin}.song-audio.json`
}

export async function ecrireProjet(chemin: string, contenu: string): Promise<void> {
  await invoke('projet_ecrire', { chemin, contenu })
}

/** Nom du fichier sans son dossier ni son extension de projet. */
export function nomDuFichier(chemin: string): string {
  return (chemin.split(/[\\/]/).pop() ?? chemin).replace(/\.song-audio\.json$|\.json$/i, '')
}
