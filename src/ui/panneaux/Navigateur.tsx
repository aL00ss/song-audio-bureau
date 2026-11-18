// Navigateur : recherche de fichiers MIDI en ligne (BitMidi), morceaux de démonstration,
// import de fichiers, et les 128 instruments General MIDI.

import { useEffect, useRef, useState } from 'react'
import { useStudio } from '../../etat/studio'
import { LISTE_INSTRUMENTS } from '../../musique/gm'
import { chercherBitMidi, formaterEcoutes, type ResultatRecherche } from '../../projet/recherche'

const FAMILLES = [...new Set(LISTE_INSTRUMENTS.map((i) => i.famille))]
const BASE = import.meta.env.BASE_URL

const titreSection = 'mb-1 text-[11px] font-semibold uppercase tracking-wider text-discret'
const ligne = 'flex w-full items-center gap-1.5 rounded px-2 py-0.5 text-left hover:bg-panneau-2 disabled:opacity-50'

interface FicheMorceau {
  id: string
  titre: string
  compositeur: string
  fichier: string
  credit: string
}

type EtatRecherche =
  | { phase: 'repos' }
  | { phase: 'recherche' }
  | { phase: 'resultats'; resultats: ResultatRecherche[] }
  | { phase: 'erreur' }

function RechercheEnLigne() {
  const { chargerMidiDepuisUrl, morceauEnChargement } = useStudio()
  const [requete, setRequete] = useState('')
  const [recherche, setRecherche] = useState<EtatRecherche>({ phase: 'repos' })

  useEffect(() => {
    const texte = requete.trim()
    if (texte.length < 2) return
    const annulation = new AbortController()
    const minuterie = setTimeout(async () => {
      setRecherche({ phase: 'recherche' })
      try {
        const resultats = await chercherBitMidi(texte, fetch, annulation.signal)
        setRecherche({ phase: 'resultats', resultats: resultats.slice(0, 30) })
      } catch {
        if (!annulation.signal.aborted) setRecherche({ phase: 'erreur' })
      }
    }, 350)
    return () => {
      clearTimeout(minuterie)
      annulation.abort()
    }
  }, [requete])

  // moins de deux lettres : aucune recherche, rien à afficher (sans attendre l'effet)
  const affichee: EtatRecherche = requete.trim().length < 2 ? { phase: 'repos' } : recherche

  return (
    <section aria-label="Recherche en ligne">
      <h3 className={titreSection}>Rechercher en ligne</h3>
      <input
        type="search"
        data-testid="recherche"
        aria-label="Rechercher un fichier MIDI en ligne"
        placeholder="Chopin, Daft Punk, Gymnopédie…"
        value={requete}
        onChange={(e) => setRequete(e.target.value)}
        aria-busy={affichee.phase === 'recherche'}
        className="h-7 w-full rounded border border-bord bg-black/40 px-2 text-texte placeholder:text-discret focus:border-accent focus:outline-none aria-busy:border-accent/60"
      />
      {affichee.phase === 'erreur' && <p className="mt-1 px-1 text-accent">Recherche indisponible : vérifiez la connexion.</p>}
      {affichee.phase === 'resultats' && !affichee.resultats.length && (
        <p className="mt-1 px-1 text-discret">Aucun fichier trouvé pour « {requete.trim()} ».</p>
      )}
      {affichee.phase === 'resultats' && affichee.resultats.length > 0 && (
        <ul data-testid="resultats" className="mt-1 max-h-64 overflow-auto">
          {affichee.resultats.map((r) => (
            <li key={r.id}>
              <button
                type="button"
                data-testid="resultat"
                disabled={morceauEnChargement !== null}
                onClick={() => void chargerMidiDepuisUrl(r.url, r.titre)}
                title={`${r.nomFichier} — charger et jouer`}
                className={ligne}
              >
                <span aria-hidden className={morceauEnChargement === r.titre ? 'animate-pulse text-accent' : 'text-accent'}>▸</span>
                <span className="min-w-0 flex-1 truncate">{r.titre}</span>
                <span className="shrink-0 text-[10px] text-discret">{formaterEcoutes(r.ecoutes)}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      <p className="mt-1 px-1 text-[10px] text-discret">
        Fichiers de <a href="https://bitmidi.com" target="_blank" rel="noopener" className="underline">BitMidi</a> : chaque piste garde son instrument.
      </p>
    </section>
  )
}

function Morceaux() {
  const { chargerMidiDepuisUrl, chargerDemo, nouveauProjet, morceauEnChargement } = useStudio()
  const [morceaux, setMorceaux] = useState<FicheMorceau[]>([])
  useEffect(() => {
    fetch(`${BASE}morceaux/catalogue.json`)
      .then((r) => (r.ok ? r.json() : []))
      .then(setMorceaux)
      .catch(() => setMorceaux([]))
  }, [])
  return (
    <section aria-label="Morceaux">
      <h3 className={titreSection}>Morceaux</h3>
      <button type="button" data-testid="nouveau-projet" onClick={nouveauProjet} title="Projet vide avec un kit de batterie" className={ligne}>
        <span aria-hidden className="text-accent">＋</span> Nouveau projet
      </button>
      <button type="button" data-testid="charger-demo" onClick={chargerDemo} className={ligne}>
        <span aria-hidden className="text-accent">▸</span> Démo (4 instruments)
      </button>
      {morceaux.map((m) => (
        <button
          key={m.id}
          type="button"
          data-testid="morceau"
          disabled={morceauEnChargement !== null}
          onClick={() => void chargerMidiDepuisUrl(`${BASE}${m.fichier}`, m.titre)}
          title={m.credit}
          className={ligne}
        >
          <span aria-hidden className={morceauEnChargement === m.titre ? 'animate-pulse text-accent' : 'text-accent'}>▸</span>
          <span className="min-w-0 flex-1 truncate">{m.titre}</span>
          <span className="shrink-0 truncate text-[10px] text-discret">{m.compositeur.split(' ').at(-1)}</span>
        </button>
      ))}
    </section>
  )
}

function ImportFichier() {
  const { importerMidi } = useStudio()
  const fichier = useRef<HTMLInputElement>(null)
  const [survol, setSurvol] = useState(false)
  return (
    <section aria-label="Fichiers MIDI">
      <h3 className={titreSection}>Fichiers MIDI</h3>
      <div
        data-testid="zone-import"
        onDragOver={(e) => {
          e.preventDefault()
          setSurvol(true)
        }}
        onDragLeave={() => setSurvol(false)}
        onDrop={(e) => {
          e.preventDefault()
          setSurvol(false)
          const f = e.dataTransfer.files[0]
          if (f) void importerMidi(f)
        }}
        className={`rounded border border-dashed p-2 text-center text-discret ${survol ? 'border-accent text-accent' : 'border-bord'}`}
      >
        Glisser un fichier .mid ici
        <br />
        ou{' '}
        <button type="button" onClick={() => fichier.current?.click()} className="text-accent underline underline-offset-2">
          choisir un fichier
        </button>
        <input
          ref={fichier}
          type="file"
          data-testid="import"
          accept=".mid,.midi,audio/midi"
          hidden
          onChange={(e) => {
            const f = e.target.files?.[0]
            if (f) void importerMidi(f)
            e.target.value = ''
          }}
        />
      </div>
    </section>
  )
}

function Instruments() {
  const { ajouterInstrument } = useStudio()
  return (
    <section aria-label="Instruments" className="min-h-0">
      <h3 className={titreSection}>Instruments</h3>
      <button type="button" onClick={() => ajouterInstrument(0, true)} title="Ajouter une ligne de batterie (un son, en pas à pas)" className={ligne}>
        ＋ Batterie
      </button>
      {FAMILLES.map((famille) => (
        <details key={famille}>
          <summary className="cursor-pointer select-none rounded px-1 py-0.5 hover:bg-panneau-2">{famille}</summary>
          <ul className="ml-3 border-l border-bord pl-1">
            {LISTE_INSTRUMENTS.filter((i) => i.famille === famille).map((i) => (
              <li key={i.programme}>
                <button
                  type="button"
                  onClick={() => ajouterInstrument(i.programme)}
                  title={`Ajouter : ${i.nom} (programme ${i.programme + 1})`}
                  className="w-full truncate rounded px-1 py-px text-left hover:bg-panneau-2"
                >
                  ＋ {i.nom}
                </button>
              </li>
            ))}
          </ul>
        </details>
      ))}
    </section>
  )
}

export function Navigateur() {
  return (
    <div className="flex h-full flex-col gap-3 overflow-auto p-2">
      <RechercheEnLigne />
      <Morceaux />
      <ImportFichier />
      <Instruments />
    </div>
  )
}
