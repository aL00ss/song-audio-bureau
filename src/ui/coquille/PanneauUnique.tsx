// Petits écrans (téléphone) : un seul panneau à la fois, choisi dans une rangée d'onglets.
// Les panneaux ancrables n'ont pas la place d'y tenir côte à côte.

import { useState } from 'react'
import { ChannelRack } from '../panneaux/ChannelRack'
import { Mixer } from '../panneaux/Mixer'
import { Navigateur } from '../panneaux/Navigateur'
import { PianoRoll } from '../panneaux/PianoRoll'
import { Playlist } from '../panneaux/Playlist'
import { VueInstrument } from '../panneaux/VueInstrument'
import { type IdPanneau, PANNEAUX } from './panneaux'

const CONTENUS: Record<IdPanneau, () => React.ReactNode> = {
  navigateur: () => <Navigateur />,
  rack: () => <ChannelRack />,
  playlist: () => <Playlist />,
  pianoroll: () => <PianoRoll />,
  instrument: () => <VueInstrument />,
  mixer: () => <Mixer />,
}

export function PanneauUnique() {
  const [actif, setActif] = useState<IdPanneau>('rack')
  return (
    <div className="flex h-full flex-col" data-testid="panneau-unique">
      <nav role="tablist" aria-label="Panneaux" className="flex shrink-0 overflow-x-auto border-b border-black/50 bg-panneau">
        {PANNEAUX.map((p) => (
          <button
            key={p.id}
            type="button"
            role="tab"
            aria-selected={actif === p.id}
            data-testid={`onglet-${p.id}`}
            onClick={() => setActif(p.id)}
            className="shrink-0 border-b-2 border-transparent px-3 py-2 text-discret aria-selected:border-accent aria-selected:text-texte"
          >
            {p.titre}
          </button>
        ))}
      </nav>
      <section role="tabpanel" aria-label={PANNEAUX.find((p) => p.id === actif)!.titre} className="min-h-0 flex-1 overflow-auto">
        {CONTENUS[actif]()}
      </section>
    </div>
  )
}
