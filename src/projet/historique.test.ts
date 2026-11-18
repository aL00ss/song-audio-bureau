import { describe, expect, it } from 'vitest'
import { annuler, enregistrer, HISTORIQUE_MAX, historiqueVide, REGROUPEMENT_MS, retablir } from './historique'
import { creerProjet, type Projet } from './modele'

const avecTempo = (tempo: number): Projet => ({ ...creerProjet(), tempo })

describe('annuler / rétablir', () => {
  it('annule puis rétablit dans l’ordre', () => {
    const [a, b, c] = [avecTempo(100), avecTempo(110), avecTempo(120)]
    let h = enregistrer(historiqueVide(), a, 0)
    h = enregistrer(h, b, 1000)
    // courant : c
    const u1 = annuler(h, c)!
    expect(u1.projet).toBe(b)
    const u2 = annuler(u1.historique, u1.projet)!
    expect(u2.projet).toBe(a)
    expect(annuler(u2.historique, u2.projet)).toBeNull()
    const r1 = retablir(u2.historique, u2.projet)!
    expect(r1.projet).toBe(b)
    const r2 = retablir(r1.historique, r1.projet)!
    expect(r2.projet).toBe(c)
    expect(retablir(r2.historique, r2.projet)).toBeNull()
  })

  it('une modification après une annulation efface le futur', () => {
    const [a, b] = [avecTempo(100), avecTempo(110)]
    const h = enregistrer(historiqueVide(), a, 0)
    const u = annuler(h, b)!
    const apres = enregistrer(u.historique, u.projet, 5000)
    expect(apres.futur).toEqual([])
    expect(retablir(apres, avecTempo(130))).toBeNull()
  })

  it('les gestes rapprochés (un bouton qu’on tourne) forment une seule étape', () => {
    const debut = avecTempo(100)
    let h = enregistrer(historiqueVide(), debut, 0)
    for (let i = 1; i < 20; i++) h = enregistrer(h, avecTempo(100 + i), i * (REGROUPEMENT_MS / 4))
    expect(h.passe).toHaveLength(1)
    expect(annuler(h, avecTempo(150))!.projet).toBe(debut)
    // après une pause, une nouvelle étape
    h = enregistrer(h, avecTempo(150), 100_000)
    expect(h.passe).toHaveLength(2)
  })

  it('l’historique est borné', () => {
    let h = historiqueVide()
    for (let i = 0; i < HISTORIQUE_MAX + 50; i++) h = enregistrer(h, avecTempo(60 + (i % 100)), i * 1000)
    expect(h.passe).toHaveLength(HISTORIQUE_MAX)
  })
})
