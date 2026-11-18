import { describe, expect, it } from 'vitest'
import { evenementsDuPattern } from '../audio/transport'
import { attribuerCanauxMidi, patternActif } from './modele'
import { MODELES, modele } from './modeles'
import { deserialiser, serialiser } from './serialisation'

describe('modèles de projet', () => {
  it.each(MODELES.map((m) => [m.id, m] as const))('%s : projet valide, relu à l’identique, jouable', (_, m) => {
    const p = m.creer()
    expect(p.canaux.length).toBeGreaterThan(0)
    expect(p.playlist.length).toBeGreaterThan(0)
    expect(deserialiser(serialiser(p))).toEqual(p)
    expect(() => attribuerCanauxMidi(p.canaux)).not.toThrow()
    if (m.id !== 'orchestre' && m.id !== 'piano' && m.id !== 'vide') expect(evenementsDuPattern(p, patternActif(p)).length).toBeGreaterThan(0)
  })

  it('chaque appel donne un projet neuf (identifiants différents)', () => {
    const a = modele('hip-hop')!.creer()
    const b = modele('hip-hop')!.creer()
    expect(a.canaux[0].id).not.toBe(b.canaux[0].id)
  })

  it('les réglages du modèle sont appliqués', () => {
    expect(modele('electro')!.creer().tempo).toBe(128)
    expect(modele('hip-hop')!.creer().swing).toBeGreaterThan(0)
  })
})
