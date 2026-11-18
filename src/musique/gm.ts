// General MIDI : les 128 instruments, leurs familles, la vue qui les représente,
// et les sons de batterie du canal 10.

export type TypeVue = 'clavier' | 'lames' | 'manche' | 'cordes-frottees' | 'harpe' | 'vents' | 'batterie'

export const CANAL_BATTERIE = 9 // canal 10 en numérotation musicale

const NOMS: readonly string[] = [
  // Pianos
  'Piano à queue', 'Piano droit brillant', 'Piano électrique à queue', 'Piano bastringue',
  'Piano électrique 1', 'Piano électrique 2', 'Clavecin', 'Clavinet',
  // Percussions chromatiques
  'Célesta', 'Glockenspiel', 'Boîte à musique', 'Vibraphone',
  'Marimba', 'Xylophone', 'Cloches tubulaires', 'Dulcimer',
  // Orgues
  'Orgue à tirettes', 'Orgue percussif', 'Orgue rock', "Orgue d'église",
  'Harmonium', 'Accordéon', 'Harmonica', 'Bandonéon',
  // Guitares
  'Guitare classique', 'Guitare folk', 'Guitare jazz', 'Guitare électrique claire',
  'Guitare électrique étouffée', 'Guitare saturée', 'Guitare distorsion', 'Harmoniques de guitare',
  // Basses
  'Contrebasse jazz', 'Basse électrique (doigts)', 'Basse électrique (médiator)', 'Basse fretless',
  'Basse slap 1', 'Basse slap 2', 'Basse synthé 1', 'Basse synthé 2',
  // Cordes
  'Violon', 'Alto', 'Violoncelle', 'Contrebasse',
  'Cordes trémolo', 'Cordes pizzicato', 'Harpe', 'Timbales',
  // Ensembles
  'Ensemble de cordes 1', 'Ensemble de cordes 2', 'Cordes synthé 1', 'Cordes synthé 2',
  'Chœur « aah »', 'Voix « ooh »', 'Voix synthé', "Coup d'orchestre",
  // Cuivres
  'Trompette', 'Trombone', 'Tuba', 'Trompette bouchée',
  'Cor', 'Section de cuivres', 'Cuivres synthé 1', 'Cuivres synthé 2',
  // Anches
  'Saxophone soprano', 'Saxophone alto', 'Saxophone ténor', 'Saxophone baryton',
  'Hautbois', 'Cor anglais', 'Basson', 'Clarinette',
  // Flûtes
  'Piccolo', 'Flûte', 'Flûte à bec', 'Flûte de Pan',
  'Bouteille soufflée', 'Shakuhachi', 'Sifflement', 'Ocarina',
  // Synthés solistes
  'Synthé carré', 'Synthé dent de scie', 'Synthé calliope', 'Synthé chiff',
  'Synthé charang', 'Synthé voix', 'Synthé quintes', 'Synthé basse et solo',
  // Nappes
  'Nappe new age', 'Nappe chaude', 'Nappe polysynth', 'Nappe chœur',
  'Nappe archet', 'Nappe métallique', 'Nappe halo', 'Nappe balayage',
  // Effets synthé
  'Effet pluie', 'Effet bande originale', 'Effet cristal', 'Effet atmosphère',
  'Effet brillance', 'Effet gobelins', 'Effet échos', 'Effet science-fiction',
  // Instruments du monde
  'Sitar', 'Banjo', 'Shamisen', 'Koto',
  'Kalimba', 'Cornemuse', 'Violon folk', 'Shehnai',
  // Percussions
  'Clochette', 'Agogo', 'Steel drum', 'Woodblock',
  'Taiko', 'Tom mélodique', 'Batterie synthé', 'Cymbale inversée',
  // Bruitages
  'Bruit de frette', 'Respiration', 'Vagues', 'Chant d’oiseau',
  'Sonnerie de téléphone', 'Hélicoptère', 'Applaudissements', 'Coup de feu',
]

const FAMILLES: readonly string[] = [
  'Pianos', 'Percussions chromatiques', 'Orgues', 'Guitares', 'Basses', 'Cordes', 'Ensembles',
  'Cuivres', 'Anches', 'Flûtes', 'Synthés solistes', 'Nappes', 'Effets synthé', 'Monde',
  'Percussions', 'Bruitages',
]

export function nomInstrument(programme: number): string {
  return NOMS[programme] ?? `Instrument ${programme}`
}

export function familleInstrument(programme: number): string {
  return FAMILLES[Math.floor(programme / 8)] ?? 'Autres'
}

/** La vue qui représente un instrument : clavier, manche, batterie… */
export function vueInstrument(programme: number, batterie = false): TypeVue {
  if (batterie) return 'batterie'
  if (programme >= 8 && programme <= 15) return 'lames'
  if ((programme >= 24 && programme <= 39) || programme === 104 || programme === 105 || programme === 106 || programme === 107) return 'manche'
  if (programme >= 40 && programme <= 45) return 'cordes-frottees'
  if (programme === 46) return 'harpe'
  if (programme >= 56 && programme <= 79) return 'vents'
  return 'clavier'
}

export const LISTE_INSTRUMENTS = NOMS.map((nom, programme) => ({ programme, nom, famille: familleInstrument(programme) }))

/** Sons de batterie General MIDI (notes 35 à 81). */
const BATTERIE: Readonly<Record<number, string>> = {
  35: 'Grosse caisse acoustique', 36: 'Grosse caisse', 37: 'Cerclage', 38: 'Caisse claire',
  39: 'Clap', 40: 'Caisse claire électrique', 41: 'Tom basse grave', 42: 'Charleston fermé',
  43: 'Tom basse aigu', 44: 'Charleston pédale', 45: 'Tom grave', 46: 'Charleston ouvert',
  47: 'Tom médium grave', 48: 'Tom médium aigu', 49: 'Crash 1', 50: 'Tom aigu',
  51: 'Ride 1', 52: 'Cymbale chinoise', 53: 'Cloche de ride', 54: 'Tambourin',
  55: 'Splash', 56: 'Cloche à vache', 57: 'Crash 2', 58: 'Vibraslap',
  59: 'Ride 2', 60: 'Bongo aigu', 61: 'Bongo grave', 62: 'Conga aiguë étouffée',
  63: 'Conga aiguë ouverte', 64: 'Conga grave', 65: 'Timbale aiguë', 66: 'Timbale grave',
  67: 'Agogo aigu', 68: 'Agogo grave', 69: 'Cabasa', 70: 'Maracas',
  71: 'Sifflet court', 72: 'Sifflet long', 73: 'Guiro court', 74: 'Guiro long',
  75: 'Claves', 76: 'Woodblock aigu', 77: 'Woodblock grave', 78: 'Cuica étouffée',
  79: 'Cuica ouverte', 80: 'Triangle étouffé', 81: 'Triangle ouvert',
}

export function nomSonBatterie(note: number): string {
  return BATTERIE[note] ?? `Percussion ${note}`
}

const NOMS_NOTES = ['Do', 'Do♯', 'Ré', 'Ré♯', 'Mi', 'Fa', 'Fa♯', 'Sol', 'Sol♯', 'La', 'La♯', 'Si']

/** Nom français d'une note, octave française (La3 = 440 Hz). */
export function nomNote(midi: number): string {
  return `${NOMS_NOTES[((midi % 12) + 12) % 12]}${Math.floor(midi / 12) - 2}`
}
