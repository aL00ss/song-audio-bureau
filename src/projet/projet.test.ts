import { Midi } from '@tonejs/midi'
import { describe, expect, it } from 'vitest'
import { CANAL_BATTERIE } from '../musique/gm'
import { creerDemo } from './demo'
import { appliquerPedale, depuisMidi, intervallesPedale } from './importMidi'
import {
  ajouterCanal, attribuerCanauxMidi, canauxAudibles, creerProjet, modifierCanal, notesAuTic, patternActif,
} from './modele'
import { deserialiser, ErreurProjet, serialiser } from './serialisation'

describe('modèle du projet', () => {
  it('un projet neuf a un pattern actif et une piste maître', () => {
    const p = creerProjet('Essai')
    expect(p.nom).toBe('Essai')
    expect(p.tempo).toBe(120)
    expect(patternActif(p).longueur).toBe(384)
    expect(p.mixage.map((m) => m.nom)).toEqual(['Maître'])
  })

  it('ajouter un canal crée aussi sa piste de mixage, sans modifier le projet d’origine', () => {
    const p0 = creerProjet()
    const { projet: p1, id } = ajouterCanal(p0, { programme: 40 })
    expect(p0.canaux).toHaveLength(0)
    expect(p1.canaux[0]).toMatchObject({ id, nom: 'Violon', programme: 40, batterie: false, pisteMixage: 1 })
    expect(p1.mixage[1].nom).toBe('Violon')
    const p2 = modifierCanal(p1, id, { muet: true, volume: 0.3 })
    expect(p2.canaux[0]).toMatchObject({ muet: true, volume: 0.3 })
    expect(p1.canaux[0].muet).toBe(false)
  })

  it('attribue la batterie au canal 10 et saute ce canal pour les autres', () => {
    let p = creerProjet()
    const ids: string[] = []
    for (let i = 0; i < 11; i++) {
      const r = ajouterCanal(p, { programme: i, batterie: i === 3 })
      p = r.projet
      ids.push(r.id)
    }
    const midi = attribuerCanauxMidi(p.canaux)
    expect(midi.get(ids[3])).toBe(CANAL_BATTERIE)
    expect(ids.filter((_, i) => i !== 3).map((id) => midi.get(id))).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 10])
  })

  it('solo et muet décident des canaux audibles', () => {
    let p = creerProjet()
    const a = ajouterCanal(p, {}); p = a.projet
    const b = ajouterCanal(p, {}); p = b.projet
    const c = ajouterCanal(p, {}); p = c.projet
    expect(canauxAudibles(p.canaux)).toEqual(new Set([a.id, b.id, c.id]))
    p = modifierCanal(p, b.id, { muet: true })
    expect(canauxAudibles(p.canaux)).toEqual(new Set([a.id, c.id]))
    p = modifierCanal(p, c.id, { solo: true })
    expect(canauxAudibles(p.canaux)).toEqual(new Set([c.id]))
  })

  it('retrouve les touches enfoncées à un instant donné', () => {
    const demo = creerDemo()
    const pattern = patternActif(demo)
    const [piano, basse] = demo.canaux
    const actives = notesAuTic(pattern, 10)
    expect(actives.get(piano.id)?.map((n) => n.hauteur)).toEqual([60, 64, 67])
    expect(actives.get(basse.id)?.map((n) => n.hauteur)).toEqual([36])
  })
})

describe('projet de démonstration', () => {
  it('quatre instruments sur quatre mesures, la batterie comprise', () => {
    const demo = creerDemo()
    expect(demo.canaux.map((c) => c.nom)).toEqual(['Piano', 'Basse', 'Cordes', 'Batterie'])
    expect(demo.canaux[3].batterie).toBe(true)
    const pattern = patternActif(demo)
    expect(pattern.longueur).toBe(4 * 384)
    for (const canal of demo.canaux) {
      const notes = pattern.notes[canal.id]
      expect(notes.length).toBeGreaterThan(0)
      expect(notes.every((n) => n.tic + n.duree <= pattern.longueur)).toBe(true)
    }
  })
})

describe('sauvegarde', () => {
  it('un projet sauvegardé puis rechargé est identique', () => {
    const demo = creerDemo()
    expect(deserialiser(serialiser(demo))).toEqual(demo)
  })

  it('refuse un fichier abîmé avec un message clair', () => {
    expect(() => deserialiser('pas du json')).toThrow(ErreurProjet)
    expect(() => deserialiser('{"version": 99}')).toThrow(/Version de projet non prise en charge/)
    const demo = JSON.parse(serialiser(creerDemo()))
    demo.canaux[0].programme = 300
    expect(() => deserialiser(JSON.stringify(demo))).toThrow(/projet\.canaux\[0\]\.programme/)
    const autre = JSON.parse(serialiser(creerDemo()))
    autre.patterns[0].notes['inconnu'] = []
    expect(() => deserialiser(JSON.stringify(autre))).toThrow(/canal inconnu/)
    const sansPattern = JSON.parse(serialiser(creerDemo()))
    sansPattern.patternActif = 'nulle-part'
    expect(() => deserialiser(JSON.stringify(sansPattern))).toThrow(/patternActif/)
  })
})

describe('import MIDI', () => {
  function fichierDeTest(): Midi {
    const midi = new Midi()
    midi.header.setTempo(90)
    midi.header.timeSignatures.push({ ticks: 0, timeSignature: [3, 4] })
    midi.header.update()
    const piano = midi.addTrack()
    piano.name = 'Main droite'
    piano.instrument.number = 0
    piano.addNote({ midi: 60, ticks: 0, durationTicks: 480, velocity: 0.8 })
    piano.addNote({ midi: 64, ticks: 480, durationTicks: 240, velocity: 0.5 })
    piano.addCC({ number: 64, ticks: 400, value: 1 })
    piano.addCC({ number: 64, ticks: 1200, value: 0 })
    const violon = midi.addTrack()
    violon.instrument.number = 40
    violon.channel = 1
    violon.addNote({ midi: 76, ticks: 960, durationTicks: 960, velocity: 1 })
    const batterie = midi.addTrack()
    batterie.channel = 9
    batterie.addNote({ midi: 36, ticks: 0, durationTicks: 60, velocity: 1 })
    midi.addTrack() // piste vide : ignorée
    // aller-retour binaire, comme un vrai fichier
    return new Midi(midi.toArray())
  }

  it('chaque piste devient un canal avec son instrument, la batterie reconnue', () => {
    const p = depuisMidi(fichierDeTest(), 'Essai')
    expect(p.canaux.map((c) => [c.nom, c.programme, c.batterie])).toEqual([
      ['Main droite', 0, false],
      ['Violon', 40, false],
      ['Batterie', 0, true],
    ])
    expect(p.tempo).toBe(90)
    expect(p.signature).toEqual({ numerateur: 3, denominateur: 4 })
    expect(p.playlist).toHaveLength(1)
  })

  it('convertit les tics à 96 par noire, la vélocité en 1–127, applique la pédale', () => {
    const p = depuisMidi(fichierDeTest())
    const pattern = patternActif(p)
    const [piano, violon, batterie] = p.canaux
    // 480 tics par noire dans le fichier -> 96 ; pédale de 400 à 1200 (80 à 240) : les deux
    // notes sont relâchées pédale enfoncée et durent jusqu'à 240. (@tonejs/midi écrit la
    // vélocité arrondie vers le bas : 0,8 -> 101/127.)
    expect(pattern.notes[piano.id]).toEqual([
      { tic: 0, duree: 240, hauteur: 60, velocite: 101 },
      { tic: 96, duree: 144, hauteur: 64, velocite: 63 },
    ])
    expect(pattern.notes[violon.id]).toEqual([{ tic: 192, duree: 192, hauteur: 76, velocite: 127 }])
    expect(pattern.notes[batterie.id]).toEqual([{ tic: 0, duree: 12, hauteur: 36, velocite: 127 }])
    // fin à 384 tics, mesures de 3 noires (288) -> deux mesures
    expect(pattern.longueur).toBe(576)
  })

  it('la pédale prolonge une note jusqu’à son relâchement, sauf si la touche est rejouée', () => {
    const intervalles = intervallesPedale([{ tic: 0, enfoncee: true }, { tic: 300, enfoncee: false }])
    expect(intervalles).toEqual([[0, 300]])
    const notes = appliquerPedale([
      { tic: 0, duree: 50, hauteur: 60, velocite: 80 },
      { tic: 100, duree: 50, hauteur: 60, velocite: 80 },
      { tic: 0, duree: 50, hauteur: 64, velocite: 80 },
    ], intervalles)
    expect(notes.map((n) => [n.tic, n.hauteur, n.duree])).toEqual([[0, 60, 100], [0, 64, 300], [100, 60, 200]])
    // pédale jamais relâchée : la note garde sa durée
    expect(appliquerPedale([{ tic: 0, duree: 10, hauteur: 60, velocite: 80 }], [[0, Infinity]])[0].duree).toBe(10)
  })
})

describe('recherche en ligne (BitMidi)', () => {
  it('met en forme les noms de fichiers et l’adresse de recherche', async () => {
    const { adresseRecherche, formaterEcoutes, nomLisible } = await import('./recherche')
    expect(nomLisible('chopin-ballade_1.mid')).toBe('chopin ballade 1')
    expect(nomLisible('Clair-De-Lune.MIDI')).toBe('Clair De Lune')
    expect(nomLisible('.mid')).toBe('Sans titre')
    expect(adresseRecherche('  satie gymnopédie ')).toBe('https://bitmidi.com/api/midi/search?q=satie%20gymnop%C3%A9die&page=0')
    expect(formaterEcoutes(10781)).toBe('10\u202f781 écoutes')
    expect(formaterEcoutes(1)).toBe('1 écoute')
  })

  it('ne garde que les résultats jouables, et signale une panne', async () => {
    const { chercherBitMidi, lireResultats } = await import('./recherche')
    expect(lireResultats(null)).toEqual([])
    expect(lireResultats({ result: { results: 'n importe quoi' } })).toEqual([])
    let adresse = ''
    const recuperer = (async (url: string) => {
      adresse = url
      return {
        ok: true,
        json: async () => ({ result: { results: [
          { id: 23776, name: 'chopin-ballade1.mid', downloadUrl: '/uploads/23776.mid', plays: 10781 },
          { id: 2, name: 'sans-fichier.mid' },
        ] } }),
      }
    }) as unknown as typeof fetch
    expect(await chercherBitMidi('chopin', recuperer)).toEqual([
      { id: 'bitmidi-23776', titre: 'chopin ballade1', nomFichier: 'chopin-ballade1.mid', url: 'https://bitmidi.com/uploads/23776.mid', ecoutes: 10781 },
    ])
    expect(adresse).toContain('q=chopin')
    const enPanne = (async () => ({ ok: false, status: 503 })) as unknown as typeof fetch
    await expect(chercherBitMidi('x', enPanne)).rejects.toThrow(/503/)
  })
})
