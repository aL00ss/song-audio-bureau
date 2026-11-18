import { describe, expect, it } from 'vitest'
import { ajouterCanal, creerProjet } from '../projet/modele'
import { BANQUE_GM, type BanqueSupplementaire, banquesNecessaires, ordreDePriorite } from './banques'

const catalogue: BanqueSupplementaire[] = [
  { id: 'piano', fichier: 'p.sf3', nom: 'Piano', programmes: [0] },
  { id: 'cordes', fichier: 'c.sf3', nom: 'Cordes', programmes: [40, 48] },
]

describe('banques supplémentaires', () => {
  it('seules les banques des instruments du projet sont demandées', () => {
    let p = creerProjet()
    expect(banquesNecessaires(p, catalogue)).toEqual([])
    p = ajouterCanal(p, { programme: 48 }).projet
    expect(banquesNecessaires(p, catalogue).map((b) => b.id)).toEqual(['cordes'])
    p = ajouterCanal(p, { programme: 0 }).projet
    expect(banquesNecessaires(p, catalogue).map((b) => b.id)).toEqual(['piano', 'cordes'])
  })

  it('une batterie ne demande pas le piano (son programme ne compte pas)', () => {
    const p = ajouterCanal(creerProjet(), { batterie: true, programme: 0 }).projet
    expect(banquesNecessaires(p, catalogue)).toEqual([])
  })

  it('priorité : banques supplémentaires dans l’ordre du catalogue, General MIDI en dernier', () => {
    expect(ordreDePriorite(['cordes', 'piano'], catalogue)).toEqual(['piano', 'cordes', BANQUE_GM])
    expect(ordreDePriorite([], catalogue)).toEqual([BANQUE_GM])
  })
})
