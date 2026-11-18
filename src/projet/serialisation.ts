// Sauvegarde du projet en JSON versionné, avec une vérification stricte au chargement :
// un fichier abîmé ou d'une autre version est refusé avec un message clair.

import { type Canal, type NoteMusicale, type Pattern, type Projet, VERSION_PROJET } from './modele'

export class ErreurProjet extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ErreurProjet'
  }
}

export function serialiser(projet: Projet): string {
  return JSON.stringify(projet)
}

type Inconnu = Record<string, unknown>

const estObjet = (v: unknown): v is Inconnu => typeof v === 'object' && v !== null && !Array.isArray(v)

function nombre(v: unknown, chemin: string, min = -Infinity, max = Infinity): number {
  if (typeof v !== 'number' || !Number.isFinite(v) || v < min || v > max) {
    throw new ErreurProjet(`${chemin} : nombre entre ${min} et ${max} attendu`)
  }
  return v
}

function entier(v: unknown, chemin: string, min = -Infinity, max = Infinity): number {
  const n = nombre(v, chemin, min, max)
  if (!Number.isInteger(n)) throw new ErreurProjet(`${chemin} : nombre entier attendu`)
  return n
}

function texte(v: unknown, chemin: string): string {
  if (typeof v !== 'string') throw new ErreurProjet(`${chemin} : texte attendu`)
  return v
}

function booleen(v: unknown, chemin: string): boolean {
  if (typeof v !== 'boolean') throw new ErreurProjet(`${chemin} : vrai ou faux attendu`)
  return v
}

function tableau(v: unknown, chemin: string): unknown[] {
  if (!Array.isArray(v)) throw new ErreurProjet(`${chemin} : liste attendue`)
  return v
}

function objet(v: unknown, chemin: string): Inconnu {
  if (!estObjet(v)) throw new ErreurProjet(`${chemin} : objet attendu`)
  return v
}

function lireNote(v: unknown, chemin: string): NoteMusicale {
  const o = objet(v, chemin)
  return {
    tic: entier(o.tic, `${chemin}.tic`, 0),
    duree: entier(o.duree, `${chemin}.duree`, 1),
    hauteur: entier(o.hauteur, `${chemin}.hauteur`, 0, 127),
    velocite: entier(o.velocite, `${chemin}.velocite`, 1, 127),
  }
}

function lireCanal(v: unknown, chemin: string): Canal {
  const o = objet(v, chemin)
  return {
    id: texte(o.id, `${chemin}.id`),
    nom: texte(o.nom, `${chemin}.nom`),
    programme: entier(o.programme, `${chemin}.programme`, 0, 127),
    batterie: booleen(o.batterie, `${chemin}.batterie`),
    volume: nombre(o.volume, `${chemin}.volume`, 0, 1),
    pan: nombre(o.pan, `${chemin}.pan`, -1, 1),
    muet: booleen(o.muet, `${chemin}.muet`),
    solo: booleen(o.solo, `${chemin}.solo`),
    pisteMixage: entier(o.pisteMixage, `${chemin}.pisteMixage`, 0),
    couleur: texte(o.couleur, `${chemin}.couleur`),
    // absent des projets enregistrés avant le séquenceur pas à pas
    noteDefaut: o.noteDefaut === undefined ? (o.batterie === true ? 36 : 60) : entier(o.noteDefaut, `${chemin}.noteDefaut`, 0, 127),
  }
}

function lirePattern(v: unknown, chemin: string, canaux: Set<string>): Pattern {
  const o = objet(v, chemin)
  const notesBrutes = objet(o.notes, `${chemin}.notes`)
  const notes: Record<string, NoteMusicale[]> = {}
  for (const [canal, liste] of Object.entries(notesBrutes)) {
    if (!canaux.has(canal)) throw new ErreurProjet(`${chemin}.notes : canal inconnu « ${canal} »`)
    notes[canal] = tableau(liste, `${chemin}.notes.${canal}`).map((n, i) => lireNote(n, `${chemin}.notes.${canal}[${i}]`))
  }
  return {
    id: texte(o.id, `${chemin}.id`),
    nom: texte(o.nom, `${chemin}.nom`),
    longueur: entier(o.longueur, `${chemin}.longueur`, 1),
    notes,
  }
}

export function deserialiser(json: string): Projet {
  let brut: unknown
  try {
    brut = JSON.parse(json)
  } catch {
    throw new ErreurProjet("Le fichier n'est pas un projet valide (JSON illisible)")
  }
  const o = objet(brut, 'projet')
  if (o.version !== VERSION_PROJET) {
    throw new ErreurProjet(`Version de projet non prise en charge : ${String(o.version)} (attendue : ${VERSION_PROJET})`)
  }
  const signature = objet(o.signature, 'projet.signature')
  const canaux = tableau(o.canaux, 'projet.canaux').map((c, i) => lireCanal(c, `projet.canaux[${i}]`))
  const ids = new Set(canaux.map((c) => c.id))
  if (ids.size !== canaux.length) throw new ErreurProjet('projet.canaux : identifiants en double')
  const patterns = tableau(o.patterns, 'projet.patterns').map((p, i) => lirePattern(p, `projet.patterns[${i}]`, ids))
  if (!patterns.length) throw new ErreurProjet('projet.patterns : au moins un pattern attendu')
  const patternActif = texte(o.patternActif, 'projet.patternActif')
  if (!patterns.some((p) => p.id === patternActif)) throw new ErreurProjet('projet.patternActif : pattern inconnu')
  const idsPatterns = new Set(patterns.map((p) => p.id))
  const mixage = tableau(o.mixage, 'projet.mixage').map((m, i) => {
    const piste = objet(m, `projet.mixage[${i}]`)
    const chemin = `projet.mixage[${i}]`
    const index = entier(piste.index, `${chemin}.index`, 0)
    // égaliseur, solo et réverbération : absents des projets enregistrés avant le mixer
    const eq = piste.eq === undefined ? { grave: 0, medium: 0, aigu: 0 } : objet(piste.eq, `${chemin}.eq`)
    return {
      index,
      nom: texte(piste.nom, `${chemin}.nom`),
      volume: nombre(piste.volume, `${chemin}.volume`, 0, 1),
      pan: nombre(piste.pan, `${chemin}.pan`, -1, 1),
      muet: booleen(piste.muet, `${chemin}.muet`),
      solo: piste.solo === undefined ? false : booleen(piste.solo, `${chemin}.solo`),
      eq: {
        grave: nombre(eq.grave, `${chemin}.eq.grave`, -24, 24),
        medium: nombre(eq.medium, `${chemin}.eq.medium`, -24, 24),
        aigu: nombre(eq.aigu, `${chemin}.eq.aigu`, -24, 24),
      },
      reverb: piste.reverb === undefined ? (index === 0 ? 0 : 0.18) : nombre(piste.reverb, `${chemin}.reverb`, 0, 1),
    }
  })
  return {
    version: VERSION_PROJET,
    nom: texte(o.nom, 'projet.nom'),
    tempo: nombre(o.tempo, 'projet.tempo', 10, 999),
    swing: o.swing === undefined ? 0 : nombre(o.swing, 'projet.swing', 0, 1),
    signature: {
      numerateur: entier(signature.numerateur, 'projet.signature.numerateur', 1, 32),
      denominateur: entier(signature.denominateur, 'projet.signature.denominateur', 1, 32),
    },
    canaux,
    patterns,
    patternActif,
    playlist: tableau(o.playlist, 'projet.playlist').map((c, i) => {
      const clip = objet(c, `projet.playlist[${i}]`)
      const pattern = texte(clip.pattern, `projet.playlist[${i}].pattern`)
      if (!idsPatterns.has(pattern)) throw new ErreurProjet(`projet.playlist[${i}].pattern : pattern inconnu`)
      return {
        id: texte(clip.id, `projet.playlist[${i}].id`),
        pattern,
        piste: entier(clip.piste, `projet.playlist[${i}].piste`, 0),
        debut: entier(clip.debut, `projet.playlist[${i}].debut`, 0),
      }
    }),
    mixage,
  }
}
