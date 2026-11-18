// Les panneaux du studio, leur disposition par défaut, leur ouverture et leur fermeture.
// Dépend seulement d'une petite partie de l'API de Dockview, décrite ici : on peut donc la
// tester avec une fausse API.

export type IdPanneau = 'navigateur' | 'playlist' | 'rack' | 'pianoroll' | 'mixer' | 'instrument'

export interface DefinitionPanneau {
  id: IdPanneau
  titre: string
  raccourci: string | null
}

export const PANNEAUX: readonly DefinitionPanneau[] = [
  { id: 'navigateur', titre: 'Navigateur', raccourci: 'F8' },
  { id: 'playlist', titre: 'Playlist', raccourci: 'F5' },
  { id: 'rack', titre: 'Channel Rack', raccourci: 'F6' },
  { id: 'pianoroll', titre: 'Piano roll', raccourci: 'F7' },
  { id: 'instrument', titre: 'Instrument', raccourci: null },
  { id: 'mixer', titre: 'Mixer', raccourci: 'F9' },
]

export const definitionPanneau = (id: IdPanneau): DefinitionPanneau => PANNEAUX.find((p) => p.id === id)!

export type Direction = 'left' | 'right' | 'above' | 'below' | 'within'

/** Où placer chaque panneau, dans l'ordre de création : à côté d'un panneau déjà placé. */
export const DISPOSITION_DEFAUT: readonly { id: IdPanneau; reference?: IdPanneau; direction?: Direction }[] = [
  { id: 'navigateur' },
  { id: 'playlist', reference: 'navigateur', direction: 'right' },
  { id: 'rack', reference: 'playlist', direction: 'below' },
  { id: 'pianoroll', reference: 'rack', direction: 'right' },
  { id: 'instrument', reference: 'rack', direction: 'below' },
  { id: 'mixer', reference: 'instrument', direction: 'within' },
]

/** Le strict nécessaire de l'API Dockview. */
export interface PanneauDock {
  id: string
  api: {
    isActive: boolean
    isVisible: boolean
    setActive(): void
    close(): void
    setSize?(taille: { width?: number; height?: number }): void
  }
}

export interface ApiDock {
  readonly panels: readonly PanneauDock[]
  getPanel(id: string): PanneauDock | undefined
  addPanel(options: {
    id: string
    component: string
    title: string
    position?: { referencePanel: string; direction: Direction }
  }): unknown
  clear(): void
}

function ajouter(api: ApiDock, id: IdPanneau, reference?: IdPanneau, direction?: Direction): void {
  const options = { id, component: id, title: definitionPanneau(id).titre }
  if (reference && direction && api.getPanel(reference)) api.addPanel({ ...options, position: { referencePanel: reference, direction } })
  else api.addPanel(options)
}

/** Tailles de départ (px) : navigateur étroit, rack haut, playlist et instrument plus bas. */
export const TAILLES_DEFAUT: Partial<Record<IdPanneau, { width?: number; height?: number }>> = {
  navigateur: { width: 230 },
  playlist: { height: 150 },
  pianoroll: { width: 420 },
  instrument: { height: 250 }, // le mixer partage ce groupe ; le rack garde la hauteur restante
}

export function dispositionParDefaut(api: ApiDock): void {
  api.clear()
  for (const { id, reference, direction } of DISPOSITION_DEFAUT) ajouter(api, id, reference, direction)
  for (const [id, taille] of Object.entries(TAILLES_DEFAUT)) api.getPanel(id)?.api.setSize?.(taille)
  api.getPanel('rack')?.api.setActive()
}

/**
 * Ouvre un panneau fermé à sa place habituelle (ou à côté du premier voisin encore ouvert),
 * ou le met au premier plan s'il est déjà ouvert.
 */
export function ouvrirPanneau(api: ApiDock, id: IdPanneau): void {
  const existant = api.getPanel(id)
  if (existant) {
    existant.api.setActive()
    return
  }
  const index = DISPOSITION_DEFAUT.findIndex((d) => d.id === id)
  const voisins = [...DISPOSITION_DEFAUT.slice(0, index).reverse(), ...DISPOSITION_DEFAUT.slice(index + 1)]
  const place = DISPOSITION_DEFAUT[index]
  if (place.reference && api.getPanel(place.reference)) return ajouter(api, id, place.reference, place.direction)
  const voisin = voisins.find((v) => api.getPanel(v.id))
  if (voisin) return ajouter(api, id, voisin.id, 'right')
  ajouter(api, id)
}

/** Raccourci F5–F9 : ferme le panneau s'il est au premier plan, sinon l'ouvre ou l'y met. */
export function basculerPanneau(api: ApiDock, id: IdPanneau): 'ouvert' | 'ferme' {
  const existant = api.getPanel(id)
  if (existant && existant.api.isActive && existant.api.isVisible) {
    existant.api.close()
    return 'ferme'
  }
  ouvrirPanneau(api, id)
  return 'ouvert'
}

export const CLE_DISPOSITION = 'song-audio.disposition.v1'
