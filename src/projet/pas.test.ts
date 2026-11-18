import { describe, expect, it } from 'vitest'
import { evenementsDuPattern } from '../audio/transport'
import { CANAL_BATTERIE } from '../musique/gm'
import { creerDemo } from './demo'
import { ajouterCanal, attribuerCanauxMidi, creerProjet, patternActif, type Projet } from './modele'
import {
  decalageSwing, definirNombreDePas, definirNoteDefaut, definirPas, definirVelocitePas, DUREE_PAS, estEnModePas,
  nombreDePas, PAS_MAX, VELOCITE_PAS, velocitesDesPas,
} from './pas'
import { deserialiser, serialiser } from './serialisation'

function avecBatterie(): { projet: Projet; id: string } {
  return ajouterCanal(creerProjet(), { batterie: true, nom: 'Grosse caisse' })
}

describe('séquenceur pas à pas', () => {
  it('un pattern neuf a 16 pas ; un canal vide est en mode pas', () => {
    const { projet, id } = avecBatterie()
    const pattern = patternActif(projet)
    expect(nombreDePas(pattern)).toBe(16)
    expect(projet.canaux[0].noteDefaut).toBe(36)
    expect(estEnModePas([], projet.canaux[0])).toBe(true)
    expect(velocitesDesPas(pattern, projet.canaux[0])).toEqual(new Array(16).fill(0))
    expect(id).toBe(projet.canaux[0].id)
  })

  it('activer un pas crée une note d’une double croche à la hauteur par défaut ; rebasculer l’enlève', () => {
    let { projet, id } = avecBatterie()
    projet = definirPas(projet, id, 4)
    projet = definirPas(projet, id, 0)
    expect(patternActif(projet).notes[id]).toEqual([
      { tic: 0, duree: DUREE_PAS, hauteur: 36, velocite: VELOCITE_PAS },
      { tic: 96, duree: DUREE_PAS, hauteur: 36, velocite: VELOCITE_PAS },
    ])
    expect(velocitesDesPas(patternActif(projet), projet.canaux[0]).slice(0, 5)).toEqual([100, 0, 0, 0, 100])
    projet = definirPas(projet, id, 4)
    expect(patternActif(projet).notes[id]).toHaveLength(1)
    // forcer un état : peindre plusieurs fois le même pas ne le duplique pas
    projet = definirPas(projet, id, 2, true)
    projet = definirPas(projet, id, 2, true)
    expect(patternActif(projet).notes[id]).toHaveLength(2)
    projet = definirPas(projet, id, 2, false)
    projet = definirPas(projet, id, 2, false)
    expect(patternActif(projet).notes[id]).toHaveLength(1)
    // hors du pattern : sans effet
    expect(definirPas(projet, id, 16)).toBe(projet)
  })

  it('la vélocité d’un pas actif se règle entre 1 et 127', () => {
    let { projet, id } = avecBatterie()
    projet = definirPas(projet, id, 3)
    projet = definirVelocitePas(projet, id, 3, 200)
    expect(velocitesDesPas(patternActif(projet), projet.canaux[0])[3]).toBe(127)
    projet = definirVelocitePas(projet, id, 3, -5)
    expect(velocitesDesPas(patternActif(projet), projet.canaux[0])[3]).toBe(1)
    expect(definirVelocitePas(projet, id, 5, 50)).toEqual(projet) // pas inactif : rien
  })

  it('la longueur du pattern va de 1 à 512 pas', () => {
    const projet = creerProjet()
    expect(nombreDePas(patternActif(definirNombreDePas(projet, 32)))).toBe(32)
    expect(nombreDePas(patternActif(definirNombreDePas(projet, 0)))).toBe(1)
    expect(nombreDePas(patternActif(definirNombreDePas(projet, 9999)))).toBe(PAS_MAX)
  })

  it('un canal aux notes libres (morceau importé) n’est pas en mode pas', () => {
    const demo = creerDemo()
    const pattern = patternActif(demo)
    expect(demo.canaux.map((c) => estEnModePas(pattern.notes[c.id], c))).toEqual([false, false, false, false])
    const canal = { noteDefaut: 60 }
    expect(estEnModePas([{ tic: 24, duree: 22, hauteur: 60, velocite: 90 }], canal)).toBe(true)
    expect(estEnModePas([{ tic: 25, duree: 22, hauteur: 60, velocite: 90 }], canal)).toBe(false)
    expect(estEnModePas([{ tic: 24, duree: 22, hauteur: 62, velocite: 90 }], canal)).toBe(false)
    expect(estEnModePas([{ tic: 24, duree: 22, hauteur: 60, velocite: 90 }, { tic: 24, duree: 22, hauteur: 60, velocite: 50 }], canal)).toBe(false)
  })

  it('changer le son de batterie déplace les pas avec lui', () => {
    let { projet, id } = avecBatterie()
    projet = definirPas(projet, id, 0)
    projet = definirPas(projet, id, 8)
    projet = definirNoteDefaut(projet, id, 38)
    expect(projet.canaux[0].noteDefaut).toBe(38)
    expect(patternActif(projet).notes[id].map((n) => n.hauteur)).toEqual([38, 38])
    expect(velocitesDesPas(patternActif(projet), projet.canaux[0])[8]).toBe(100)
  })
})

describe('swing', () => {
  it('retarde seulement les pas impairs, jusqu’à un demi-pas', () => {
    expect(decalageSwing(24, 0)).toBe(0)
    expect(decalageSwing(24, 1)).toBe(12)
    expect(decalageSwing(24 * 3, 0.5)).toBe(6)
    expect(decalageSwing(48, 1)).toBe(0) // pas pair
    expect(decalageSwing(30, 1)).toBe(0) // hors d'un début de pas
  })

  it('le transport applique le swing aux attaques et aux relâchements', () => {
    let { projet, id } = avecBatterie()
    projet = definirPas(projet, id, 0)
    projet = definirPas(projet, id, 1)
    projet = { ...projet, swing: 1 }
    const e = evenementsDuPattern(projet, patternActif(projet))
    expect(e.map((x) => [x.tic, x.allume])).toEqual([[0, true], [DUREE_PAS, false], [24 + 12, true], [24 + 12 + DUREE_PAS, false]])
  })
})

describe('canaux MIDI et sauvegarde', () => {
  it('la première batterie sur le canal 10, les autres batteries sur leur propre canal', () => {
    let p = creerProjet()
    const ids: string[] = []
    for (const [batterie, nom] of [[true, 'Grosse caisse'], [false, 'Piano'], [true, 'Caisse claire'], [true, 'Charleston']] as const) {
      const r = ajouterCanal(p, { batterie, nom })
      p = r.projet
      ids.push(r.id)
    }
    const midi = attribuerCanauxMidi(p.canaux)
    expect(ids.map((id) => midi.get(id))).toEqual([CANAL_BATTERIE, 0, 1, 2])
  })

  it('les projets enregistrés avant le séquenceur se rechargent (hauteur par défaut, swing)', () => {
    const ancien = JSON.parse(serialiser(creerDemo()))
    delete ancien.swing
    for (const c of ancien.canaux) delete c.noteDefaut
    const projet = deserialiser(JSON.stringify(ancien))
    expect(projet.swing).toBe(0)
    expect(projet.canaux.map((c) => c.noteDefaut)).toEqual([60, 60, 60, 36])
    expect(() => deserialiser(JSON.stringify({ ...ancien, swing: 2 }))).toThrow(/projet\.swing/)
  })
})
