// Menu Fichier : nouveau projet, ouvrir et enregistrer un projet, exporter en MIDI ou en WAV.

import { useState } from 'react'
import { projetVide, useStudio } from '../../etat/studio'
import { nomDeFichier } from '../../projet/exportMidi'
import { telecharger } from '../commun/telecharger'

export const EXTENSION_PROJET = 'song-audio.json'

const entree = 'block w-full px-3 py-1.5 text-left hover:bg-accent/20 disabled:opacity-40'

export function MenuFichier() {
  const { projet, chargement, exportEnCours, remplacerProjet, ouvrirProjet, projetEnJson, exporterMidi, exporterWav } = useStudio()
  const [ouvert, setOuvert] = useState(false)
  const pret = chargement.phase === 'pret'
  const action = (f: () => void | Promise<void>) => () => {
    setOuvert(false)
    void f()
  }

  return (
    <div className="relative" onBlur={(e) => !e.currentTarget.contains(e.relatedTarget) && setOuvert(false)}>
      <button
        type="button"
        data-testid="menu-fichier"
        aria-haspopup="menu"
        aria-expanded={ouvert}
        onClick={() => setOuvert((o) => !o)}
        className="h-7 rounded border border-bord bg-panneau-2 px-2.5 hover:border-discret"
      >
        Fichier
      </button>
      {ouvert && (
        <div role="menu" className="absolute left-0 top-8 z-50 min-w-52 rounded border border-bord bg-panneau py-1 shadow-lg">
          <button type="button" role="menuitem" className={entree} onClick={action(() => remplacerProjet(projetVide()))}>
            Nouveau projet
          </button>
          <label htmlFor="ouvrir-projet" role="menuitem" tabIndex={0} className={`${entree} cursor-pointer`} onClick={() => setTimeout(() => setOuvert(false))}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                document.getElementById('ouvrir-projet')?.click()
              }
            }}
          >
            Ouvrir un projet…
          </label>
          <button
            type="button"
            role="menuitem"
            className={entree}
            onClick={action(() => telecharger(projetEnJson(), nomDeFichier(projet.nom, EXTENSION_PROJET), 'application/json'))}
          >
            Enregistrer le projet
          </button>
          <hr className="my-1 border-bord" />
          <button
            type="button"
            role="menuitem"
            className={entree}
            onClick={action(() => telecharger(exporterMidi(), nomDeFichier(projet.nom, 'mid'), 'audio/midi'))}
          >
            Exporter en MIDI
          </button>
          <button
            type="button"
            role="menuitem"
            disabled={!pret || exportEnCours}
            className={entree}
            onClick={action(async () => {
              const wav = await exporterWav()
              if (wav) telecharger(wav, nomDeFichier(projet.nom, 'wav'))
            })}
          >
            Exporter en WAV
          </button>
        </div>
      )}
      <input
        id="ouvrir-projet"
        type="file"
        data-testid="ouvrir-projet"
        accept=".json,application/json"
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0]
          e.target.value = ''
          if (f) void ouvrirProjet(f)
        }}
      />
    </div>
  )
}
