import { describe, expect, it } from 'vitest'
import { type ApiDock, basculerPanneau, DISPOSITION_DEFAUT, dispositionParDefaut, ouvrirPanneau, PANNEAUX, type PanneauDock } from './panneaux'
import { actionPourTouche, saisieEnCours, type Touche } from './raccourcis'

/** Fausse API Dockview : garde la liste des panneaux, leurs positions et le panneau actif. */
function fausseApi() {
  const panneaux: (PanneauDock & { position?: { referencePanel: string; direction: string } })[] = []
  let actif: string | null = null
  const api: ApiDock = {
    get panels() {
      return panneaux
    },
    getPanel: (id) => panneaux.find((p) => p.id === id),
    addPanel(options) {
      const p = {
        id: options.id,
        position: options.position,
        api: {
          get isActive() {
            return actif === options.id
          },
          isVisible: true,
          setActive: () => {
            actif = options.id
          },
          close: () => {
            panneaux.splice(panneaux.indexOf(p), 1)
            if (actif === options.id) actif = null
          },
        },
      }
      panneaux.push(p)
      actif = options.id
      return p
    },
    clear() {
      panneaux.length = 0
      actif = null
    },
  }
  return { api, panneaux, actif: () => actif }
}

const touche = (code: string, autres: Partial<Touche> = {}): Touche => ({
  key: code, code, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, repeat: false,
  cible: { balise: 'BODY' }, ...autres,
})

describe('panneaux', () => {
  it('six panneaux, et les raccourcis F5 à F9 des studios', () => {
    expect(PANNEAUX.map((p) => [p.id, p.raccourci])).toEqual([
      ['navigateur', 'F8'], ['playlist', 'F5'], ['rack', 'F6'], ['pianoroll', 'F7'], ['instrument', null], ['mixer', 'F9'],
    ])
  })

  it('la disposition par défaut place chaque panneau à côté du précédent', () => {
    const { api, panneaux } = fausseApi()
    dispositionParDefaut(api)
    expect(panneaux.map((p) => p.id)).toEqual(DISPOSITION_DEFAUT.map((d) => d.id))
    expect(panneaux.find((p) => p.id === 'rack')?.position).toEqual({ referencePanel: 'playlist', direction: 'below' })
    expect(panneaux.find((p) => p.id === 'mixer')?.position).toEqual({ referencePanel: 'instrument', direction: 'within' })
    // le rack, zone de travail principale, est au premier plan
    expect(api.getPanel('rack')!.api.isActive).toBe(true)
    dispositionParDefaut(api) // recommencer ne duplique rien
    expect(panneaux).toHaveLength(6)
  })

  it('basculer ferme le panneau au premier plan, rouvre un panneau fermé à sa place', () => {
    const { api, panneaux, actif } = fausseApi()
    dispositionParDefaut(api)
    api.getPanel('rack')!.api.setActive()
    expect(basculerPanneau(api, 'rack')).toBe('ferme')
    expect(api.getPanel('rack')).toBeUndefined()
    expect(basculerPanneau(api, 'rack')).toBe('ouvert')
    expect(panneaux.find((p) => p.id === 'rack')?.position).toEqual({ referencePanel: 'playlist', direction: 'below' })
    // un panneau ouvert mais pas au premier plan est mis au premier plan, pas fermé
    expect(basculerPanneau(api, 'navigateur')).toBe('ouvert')
    expect(actif()).toBe('navigateur')
    expect(api.getPanel('navigateur')).toBeDefined()
  })

  it('si la place habituelle a disparu, le panneau va à côté d’un voisin ouvert', () => {
    const { api, panneaux } = fausseApi()
    dispositionParDefaut(api)
    api.getPanel('playlist')!.api.close()
    api.getPanel('rack')!.api.close()
    ouvrirPanneau(api, 'rack')
    expect(panneaux.find((p) => p.id === 'rack')?.position).toEqual({ referencePanel: 'navigateur', direction: 'right' })
    api.clear()
    ouvrirPanneau(api, 'mixer')
    expect(panneaux.map((p) => p.id)).toEqual(['mixer'])
  })
})

describe('raccourcis', () => {
  it('Ctrl+Z annule, Ctrl+Y et Ctrl+Maj+Z rétablissent, sauf dans un champ de saisie', () => {
    expect(actionPourTouche(touche('KeyZ', { ctrlKey: true }))).toEqual({ type: 'annuler' })
    expect(actionPourTouche(touche('KeyZ', { metaKey: true }))).toEqual({ type: 'annuler' })
    expect(actionPourTouche(touche('KeyY', { ctrlKey: true }))).toEqual({ type: 'retablir' })
    expect(actionPourTouche(touche('KeyZ', { ctrlKey: true, shiftKey: true }))).toEqual({ type: 'retablir' })
    expect(actionPourTouche(touche('KeyZ', { ctrlKey: true, cible: { balise: 'INPUT', type: 'text' } }))).toBeNull()
    expect(actionPourTouche(touche('KeyZ', { ctrlKey: true, altKey: true }))).toBeNull()
  })

  it('F5 à F9 ouvrent les panneaux, même pendant une saisie', () => {
    expect(actionPourTouche(touche('F6'))).toEqual({ type: 'panneau', id: 'rack' })
    expect(actionPourTouche(touche('F9', { cible: { balise: 'INPUT', type: 'number' } }))).toEqual({ type: 'panneau', id: 'mixer' })
    expect(actionPourTouche(touche('F5', { repeat: true }))).toBeNull()
  })

  it('Espace lance ou met en pause, Maj+Espace arrête, sauf pendant une saisie', () => {
    expect(actionPourTouche(touche('Space'))).toEqual({ type: 'lecture' })
    expect(actionPourTouche(touche('Space', { shiftKey: true }))).toEqual({ type: 'stop' })
    expect(actionPourTouche(touche('Space', { cible: { balise: 'BUTTON' } }))).toEqual({ type: 'lecture' })
    expect(actionPourTouche(touche('Space', { cible: { balise: 'INPUT', type: 'text' } }))).toBeNull()
    expect(actionPourTouche(touche('Space', { cible: { balise: 'DIV', editable: true } }))).toBeNull()
    expect(actionPourTouche(touche('Space', { ctrlKey: true }))).toBeNull()
  })

  it('les rangées du clavier jouent des notes, les flèches changent d’octave', () => {
    expect(actionPourTouche(touche('KeyQ'))).toEqual({ type: 'note', code: 'KeyQ' })
    expect(actionPourTouche(touche('Digit2'))).toEqual({ type: 'note', code: 'Digit2' })
    expect(actionPourTouche(touche('KeyA'))).toBeNull() // pas sur les rangées de notes
    expect(actionPourTouche(touche('KeyQ', { cible: { balise: 'INPUT', type: 'search' } }))).toBeNull()
    expect(actionPourTouche(touche('KeyQ', { ctrlKey: true }))).toBeNull()
    expect(actionPourTouche(touche('ArrowLeft'))).toEqual({ type: 'octave', sens: -1 })
    expect(actionPourTouche(touche('ArrowRight'))).toEqual({ type: 'octave', sens: 1 })
    expect(actionPourTouche(touche('ArrowRight', { cible: { balise: 'INPUT', type: 'range' } }))).toBeNull()
    expect(actionPourTouche(touche('ArrowRight', { cible: { balise: 'DIV', role: 'slider' } }))).toBeNull()
  })

  it('reconnaît les champs de saisie', () => {
    expect(saisieEnCours({ balise: 'INPUT' })).toBe(true)
    expect(saisieEnCours({ balise: 'INPUT', type: 'search' })).toBe(true)
    expect(saisieEnCours({ balise: 'INPUT', type: 'checkbox' })).toBe(false)
    expect(saisieEnCours({ balise: 'INPUT', type: 'range' })).toBe(false)
    expect(saisieEnCours({ balise: 'TEXTAREA' })).toBe(true)
    expect(saisieEnCours({ balise: 'BUTTON' })).toBe(false)
  })
})
