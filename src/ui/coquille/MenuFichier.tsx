// Menu Fichier : nouveau projet, ouvrir et enregistrer un projet, exporter en MIDI ou en WAV.

import { useState } from 'react'
import { estBureau, useStudio } from '../../etat/studio'
import { nomDeFichier } from '../../projet/exportMidi'
import { MODELES } from '../../projet/modeles'
import { telecharger } from '../commun/telecharger'

import { enregistrerOuTelecharger, ouvrirProjetSelonPlateforme } from './actionsFichier'

const entree = 'block w-full px-3 py-1.5 text-left hover:bg-accent/20 disabled:opacity-40'

export function MenuFichier() {
  const { projet, chargement, exportEnCours, ouvrirProjet, exporterMidi, exporterWav, peutAnnuler, peutRetablir, annuler, retablir, nouveauDepuisModele } =
    useStudio()
  const [modeles, setModeles] = useState(false)
  const bureau = estBureau()
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
          <button type="button" role="menuitem" disabled={!peutAnnuler} className={entree} onClick={action(annuler)}>
            Annuler <span className="float-right text-discret">Ctrl+Z</span>
          </button>
          <button type="button" role="menuitem" disabled={!peutRetablir} className={entree} onClick={action(retablir)}>
            Rétablir <span className="float-right text-discret">Ctrl+Y</span>
          </button>
          <hr className="my-1 border-bord" />
          <button type="button" role="menuitem" className={entree} onClick={action(() => nouveauDepuisModele('vide'))}>
            Nouveau projet
          </button>
          <button
            type="button"
            role="menuitem"
            aria-expanded={modeles}
            data-testid="menu-modeles"
            className={entree}
            onClick={() => setModeles((m) => !m)}
          >
            Nouveau depuis un modèle <span className="float-right text-discret">{modeles ? '▾' : '▸'}</span>
          </button>
          {modeles &&
            MODELES.map((m) => (
              <button
                key={m.id}
                type="button"
                role="menuitem"
                data-testid={`modele-${m.id}`}
                className={`${entree} pl-6`}
                title={m.description}
                onClick={action(() => nouveauDepuisModele(m.id))}
              >
                {m.nom} <span className="block text-[11px] text-discret">{m.description}</span>
              </button>
            ))}
          {bureau ? (
            <button type="button" role="menuitem" className={entree} onClick={action(() => ouvrirProjetSelonPlateforme())}>
              Ouvrir un projet… <span className="float-right text-discret">Ctrl+O</span>
            </button>
          ) : (
          <label htmlFor="ouvrir-projet" role="menuitem" tabIndex={0} className={`${entree} cursor-pointer`} onClick={() => setTimeout(() => setOuvert(false))}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                document.getElementById('ouvrir-projet')?.click()
              }
            }}
          >
            Ouvrir un projet… <span className="float-right text-discret">Ctrl+O</span>
          </label>
          )}
          <button type="button" role="menuitem" className={entree} onClick={action(() => enregistrerOuTelecharger(false))}>
            Enregistrer le projet <span className="float-right text-discret">Ctrl+S</span>
          </button>
          {bureau && (
            <button type="button" role="menuitem" className={entree} onClick={action(() => enregistrerOuTelecharger(true))}>
              Enregistrer sous… <span className="float-right text-discret">Ctrl+Maj+S</span>
            </button>
          )}
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
