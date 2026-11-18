// Projet de démonstration écrit dans le code (aucun fichier extérieur, aucune licence à
// gérer) : quatre mesures en do majeur, Do – La mineur – Fa – Sol, sur quatre instruments.

import { produce } from 'immer'
import { TICS_PAR_NOIRE, ticsParMesure } from '../musique/temps'
import { ajouterCanal, creerProjet, nouvelId, type NoteMusicale, type Projet } from './modele'

const MESURE = ticsParMesure()
const CROCHE = TICS_PAR_NOIRE / 2

// accords (piano et cordes) et fondamentales (basse), une mesure chacun
const ACCORDS = [
  [60, 64, 67], // Do
  [57, 60, 64], // La mineur
  [53, 57, 60], // Fa
  [55, 59, 62], // Sol
]
const BASSES = [36, 33, 29, 31]

const note = (tic: number, duree: number, hauteur: number, velocite: number): NoteMusicale => ({ tic, duree, hauteur, velocite })

export function creerDemo(): Projet {
  let projet = creerProjet('Démo')
  const piano = ajouterCanal(projet, { programme: 0, nom: 'Piano' })
  projet = piano.projet
  const basse = ajouterCanal(projet, { programme: 33, nom: 'Basse' })
  projet = basse.projet
  const cordes = ajouterCanal(projet, { programme: 48, nom: 'Cordes', volume: 0.65 })
  projet = cordes.projet
  const batterie = ajouterCanal(projet, { batterie: true, nom: 'Batterie' })
  projet = batterie.projet

  const notesPiano: NoteMusicale[] = []
  const notesBasse: NoteMusicale[] = []
  const notesCordes: NoteMusicale[] = []
  const notesBatterie: NoteMusicale[] = []
  ACCORDS.forEach((accord, m) => {
    const debut = m * MESURE
    // piano : l'accord en noires
    for (let temps = 0; temps < 4; temps++) {
      for (const h of accord) notesPiano.push(note(debut + temps * TICS_PAR_NOIRE, TICS_PAR_NOIRE - 8, h, temps === 0 ? 96 : 78))
    }
    // basse : fondamentale, quinte, fondamentale à l'octave
    const f = BASSES[m]
    notesBasse.push(note(debut, TICS_PAR_NOIRE * 2 - 8, f, 100))
    notesBasse.push(note(debut + 2 * TICS_PAR_NOIRE, TICS_PAR_NOIRE - 8, f + 7, 90))
    notesBasse.push(note(debut + 3 * TICS_PAR_NOIRE, TICS_PAR_NOIRE - 8, f + 12, 90))
    // cordes : l'accord tenu, une octave plus bas
    for (const h of accord) notesCordes.push(note(debut, MESURE - 4, h - 12, 70))
    // batterie : grosse caisse sur 1 et 3, caisse claire sur 2 et 4, charleston en croches
    for (let i = 0; i < 8; i++) notesBatterie.push(note(debut + i * CROCHE, CROCHE / 2, 42, i % 2 === 0 ? 90 : 64))
    for (const temps of [0, 2]) notesBatterie.push(note(debut + temps * TICS_PAR_NOIRE, CROCHE / 2, 36, 110))
    for (const temps of [1, 3]) notesBatterie.push(note(debut + temps * TICS_PAR_NOIRE, CROCHE / 2, 38, 100))
  })
  notesBatterie.push(note(0, CROCHE, 49, 100)) // crash au début

  const tri = (a: NoteMusicale, b: NoteMusicale) => a.tic - b.tic || a.hauteur - b.hauteur
  return produce(projet, (p) => {
    p.tempo = 100
    const pattern = p.patterns[0]
    pattern.nom = 'Démo'
    pattern.longueur = 4 * MESURE
    pattern.notes[piano.id] = notesPiano.sort(tri)
    pattern.notes[basse.id] = notesBasse.sort(tri)
    pattern.notes[cordes.id] = notesCordes.sort(tri)
    pattern.notes[batterie.id] = notesBatterie.sort(tri)
    p.playlist = [{ id: nouvelId('clip'), pattern: pattern.id, piste: 0, debut: 0 }]
  })
}
