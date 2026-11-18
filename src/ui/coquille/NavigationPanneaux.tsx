// Boutons des panneaux (barre d'état) : ouvrent ou mettent au premier plan chaque panneau.

import { type IdPanneau, PANNEAUX } from './panneaux'

export function NavigationPanneaux({ ouvrirPanneau, panneauxOuverts }: {
  ouvrirPanneau: (id: IdPanneau) => void
  panneauxOuverts: ReadonlySet<string>
}) {
  return (
    <nav aria-label="Panneaux" className="flex flex-wrap items-center gap-0.5">
      {PANNEAUX.map((p) => (
        <button
          key={p.id}
          type="button"
          data-testid={`ouvrir-${p.id}`}
          aria-pressed={panneauxOuverts.has(p.id)}
          title={p.raccourci ? `${p.titre} (${p.raccourci})` : p.titre}
          onClick={() => ouvrirPanneau(p.id)}
          className="rounded px-1.5 text-discret hover:bg-panneau-2 hover:text-texte aria-pressed:text-texte"
        >
          {p.titre}
        </button>
      ))}
    </nav>
  )
}
