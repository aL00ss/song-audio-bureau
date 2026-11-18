// Recherche de fichiers MIDI en ligne : l'API publique de BitMidi (plus de 100 000 fichiers,
// interrogeable depuis le navigateur). Les fonctions de mise en forme sont pures ;
// `chercherBitMidi` reçoit `fetch` en paramètre pour être testée sans réseau.

export const API_BITMIDI = 'https://bitmidi.com/api/midi/search'
export const ORIGINE_BITMIDI = 'https://bitmidi.com'

export interface ResultatRecherche {
  id: string
  titre: string
  nomFichier: string
  url: string
  ecoutes: number
}

/** « chopin-ballade_1.mid » -> « chopin ballade 1 ». */
export function nomLisible(nom: string): string {
  return String(nom).replace(/\.midi?$/i, '').replace(/[-_]+/g, ' ').replace(/\s+/g, ' ').trim() || 'Sans titre'
}

export function adresseRecherche(requete: string, page = 0): string {
  return `${API_BITMIDI}?q=${encodeURIComponent(requete.trim())}&page=${page}`
}

interface ReponseBitMidi {
  result?: { results?: { id: number | string; name: string; downloadUrl?: string; plays?: number }[] }
}

/** Résultats jouables d'une réponse de l'API (ceux sans fichier sont ignorés). */
export function lireResultats(reponse: unknown): ResultatRecherche[] {
  const resultats = (reponse as ReponseBitMidi)?.result?.results
  if (!Array.isArray(resultats)) return []
  return resultats
    .filter((r) => typeof r?.downloadUrl === 'string' && r.downloadUrl.length > 0)
    .map((r) => ({
      id: `bitmidi-${r.id}`,
      titre: nomLisible(r.name),
      nomFichier: String(r.name),
      url: new URL(r.downloadUrl!, ORIGINE_BITMIDI).href,
      ecoutes: Number(r.plays) || 0,
    }))
}

export async function chercherBitMidi(
  requete: string,
  recuperer: typeof fetch = globalThis.fetch,
  signal?: AbortSignal,
): Promise<ResultatRecherche[]> {
  const reponse = await recuperer(adresseRecherche(requete), { signal })
  if (!reponse.ok) throw new Error(`Recherche impossible (HTTP ${reponse.status})`)
  return lireResultats(await reponse.json())
}

const NOMBRE = new Intl.NumberFormat('fr-FR')
export const formaterEcoutes = (n: number): string => `${NOMBRE.format(n)} écoute${n > 1 ? 's' : ''}`
