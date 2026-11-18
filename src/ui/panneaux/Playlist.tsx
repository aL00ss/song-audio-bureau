// Playlist : l'arrangement. À gauche les patterns (choisir, renommer par double clic,
// supprimer) ; à droite une règle des mesures (clic : aller à cet endroit du morceau) et des
// pistes où l'on pose des clips du pattern actif (clic), qu'on déplace (glisser) ou supprime
// (clic droit). Les clips s'aimantent à la mesure. Double clic sur un clip : choisir son pattern.

import { useRef, useState } from 'react'
import { audio, useStudio } from '../../etat/studio'
import { ticsParMesure } from '../../musique/temps'
import type { Pattern, Projet } from '../../projet/modele'
import { nombreDePistes } from '../../projet/playlist'
import { useImage } from '../commun/useImage'

const H_PISTE = 34
const L_ENTETE = 64
const COULEURS = ['#5aa9f5', '#f5a524', '#4cc38a', '#e5679a', '#a78bfa', '#f97362', '#2dd4bf', '#facc15']
const couleurPattern = (projet: Projet, id: string) => COULEURS[Math.max(0, projet.patterns.findIndex((p) => p.id === id)) % COULEURS.length]

/** Aperçu des notes d'un pattern dans un clip (tous canaux confondus). */
function ApercuClip({ pattern }: { pattern: Pattern }) {
  const notes = Object.values(pattern.notes).flat()
  if (!notes.length) return null
  const basse = Math.min(...notes.map((n) => n.hauteur))
  const haute = Math.max(...notes.map((n) => n.hauteur))
  const etendue = Math.max(12, haute - basse + 1)
  return (
    <svg viewBox={`0 0 ${pattern.longueur} ${etendue}`} preserveAspectRatio="none" className="pointer-events-none absolute inset-x-1 bottom-1 top-3.5" aria-hidden>
      {notes.map((n, i) => (
        <rect key={i} x={n.tic} y={haute - n.hauteur} width={Math.max(n.duree, pattern.longueur / 200)} height={1} fill="rgba(0,0,0,0.55)" />
      ))}
    </svg>
  )
}

function ListePatterns() {
  const { projet, choisirPattern, renommerPattern, supprimerPattern } = useStudio()
  const [edition, setEdition] = useState<string | null>(null)
  return (
    <aside aria-label="Patterns" className="flex w-40 shrink-0 flex-col gap-0.5 overflow-auto border-r border-black/50 p-1">
      <h3 className="px-1 text-[11px] font-semibold uppercase tracking-wider text-discret">Patterns</h3>
      {projet.patterns.map((p) => (
        <div key={p.id} className="group flex items-center gap-1">
          <span className="size-2.5 shrink-0 rounded-sm" style={{ background: couleurPattern(projet, p.id) }} />
          {edition === p.id ? (
            <input
              autoFocus
              aria-label="Nom du pattern"
              defaultValue={p.nom}
              onBlur={(e) => {
                renommerPattern(p.id, e.target.value)
                setEdition(null)
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') e.currentTarget.blur()
                if (e.key === 'Escape') setEdition(null)
              }}
              className="h-5 min-w-0 flex-1 rounded border border-accent bg-black/40 px-1 text-texte"
            />
          ) : (
            <button
              type="button"
              data-testid="pattern"
              aria-pressed={p.id === projet.patternActif}
              onClick={() => choisirPattern(p.id)}
              onDoubleClick={() => setEdition(p.id)}
              title="Clic : choisir · double clic : renommer"
              className="min-w-0 flex-1 truncate rounded px-1 text-left hover:bg-panneau-2 aria-pressed:bg-panneau-2 aria-pressed:text-accent"
            >
              {p.nom}
            </button>
          )}
          {projet.patterns.length > 1 && (
            <button
              type="button"
              aria-label={`Supprimer ${p.nom}`}
              title="Supprimer ce pattern et ses clips"
              onClick={() => supprimerPattern(p.id)}
              className="invisible px-1 text-discret hover:text-accent group-hover:visible"
            >
              ×
            </button>
          )}
        </div>
      ))}
    </aside>
  )
}

export function Playlist() {
  const { projet, mode, ajouterClip, deplacerClip, supprimerClip, choisirPattern, changerMode } = useStudio()
  const [pxParMesure, setPxParMesure] = useState(64)
  const mesure = ticsParMesure(projet.signature)
  const pxParTic = pxParMesure / mesure
  const pistes = nombreDePistes(projet)
  const finMorceau = Math.max(0, ...projet.playlist.map((c) => c.debut + (projet.patterns.find((p) => p.id === c.pattern)?.longueur ?? 0)))
  const nbMesures = Math.max(32, Math.ceil(finMorceau / mesure) + 8)
  const glisse = useRef<{ id: string; departTic: number; departPiste: number; x0: number; y0: number } | null>(null)
  const zone = useRef<HTMLDivElement>(null)
  const dernierAppui = useRef<{ id: string; t: number } | null>(null)
  const tete = useImage(() => (audio.transport?.mode === 'morceau' && audio.transport.etat !== 'arret' ? audio.transport.position : -1))

  const versTicPiste = (e: React.PointerEvent | React.MouseEvent) => {
    const r = zone.current!.getBoundingClientRect()
    return { tic: (e.clientX - r.left) / pxParTic, piste: Math.floor((e.clientY - r.top) / H_PISTE) }
  }

  return (
    <div className="flex h-full">
      <ListePatterns />
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex shrink-0 items-center gap-3 border-b border-black/50 px-2 py-1 text-discret">
          <span>
            Mode <span className="font-semibold text-texte">{mode === 'morceau' ? 'SONG' : 'PAT'}</span>
            {mode === 'pattern' && (
              <button type="button" onClick={() => changerMode('morceau')} className="ml-2 text-accent underline underline-offset-2">
                jouer le morceau
              </button>
            )}
          </span>
          <span className="flex items-center gap-1">
            Zoom
            <button type="button" aria-label="Zoom arrière" onClick={() => setPxParMesure((z) => Math.max(16, z / 1.5))} className="rounded border border-bord px-1.5">−</button>
            <button type="button" aria-label="Zoom avant" onClick={() => setPxParMesure((z) => Math.min(512, z * 1.5))} className="rounded border border-bord px-1.5">+</button>
          </span>
          <span className="ml-auto text-[11px]">Clic : poser le pattern actif · glisser : déplacer · clic droit : supprimer · double clic : choisir</span>
        </div>
        <div className="min-h-0 flex-1 overflow-auto" data-testid="playlist">
          <div className="relative" style={{ width: L_ENTETE + nbMesures * pxParMesure }}>
            {/* règle des mesures */}
            <div
              data-testid="regle"
              className="sticky top-0 z-10 ml-16 flex h-5 cursor-pointer border-b border-black/60 bg-[#1f242a]"
              style={{ width: nbMesures * pxParMesure }}
              onPointerDown={(e) => {
                const r = e.currentTarget.getBoundingClientRect()
                const tic = Math.floor((e.clientX - r.left) / pxParTic / mesure) * mesure
                changerMode('morceau')
                audio.transport?.chercher(tic)
              }}
            >
              {Array.from({ length: nbMesures }, (_, i) => (
                <span key={i} className="shrink-0 border-l border-[#3a424c] pl-1 font-mono text-[10px] text-discret" style={{ width: pxParMesure }}>
                  {i + 1}
                </span>
              ))}
            </div>
            <div className="flex">
              {/* noms des pistes */}
              <div className="sticky left-0 z-[5] w-16 shrink-0 bg-panneau">
                {Array.from({ length: pistes }, (_, i) => (
                  <div key={i} className="flex items-center border-b border-black/50 px-1.5 text-discret" style={{ height: H_PISTE }}>
                    Piste {i + 1}
                  </div>
                ))}
              </div>
              {/* pistes et clips */}
              <div
                ref={zone}
                data-testid="pistes-playlist"
                className="relative"
                style={{
                  width: nbMesures * pxParMesure,
                  height: pistes * H_PISTE,
                  backgroundImage: `linear-gradient(90deg, #3a424c 1px, transparent 1px), linear-gradient(#11151a 1px, transparent 1px)`,
                  backgroundSize: `${pxParMesure}px ${H_PISTE}px, ${pxParMesure}px ${H_PISTE}px`,
                  backgroundColor: '#22272e',
                }}
                onContextMenu={(e) => e.preventDefault()}
                onPointerDown={(e) => {
                  if (e.button !== 0 || e.target !== e.currentTarget) return
                  const { tic, piste } = versTicPiste(e)
                  if (piste < 0 || piste >= pistes) return
                  ajouterClip(piste, Math.floor(tic / mesure) * mesure)
                }}
                onPointerMove={(e) => {
                  const g = glisse.current
                  if (!g) return
                  const dx = e.clientX - g.x0
                  const dy = e.clientY - g.y0
                  const debut = Math.max(0, Math.round((g.departTic + dx / pxParTic) / mesure) * mesure)
                  const piste = Math.min(pistes - 1, Math.max(0, g.departPiste + Math.round(dy / H_PISTE)))
                  deplacerClip(g.id, piste, debut)
                }}
                onPointerUp={() => (glisse.current = null)}
                onPointerCancel={() => (glisse.current = null)}
              >
                {projet.playlist.map((clip) => {
                  const pattern = projet.patterns.find((p) => p.id === clip.pattern)
                  if (!pattern) return null
                  return (
                    <div
                      key={clip.id}
                      data-clip={clip.id}
                      data-piste={clip.piste}
                      data-debut={clip.debut}
                      title={`${pattern.nom} — mesure ${clip.debut / mesure + 1}`}
                      onPointerDown={(e) => {
                        e.stopPropagation()
                        if (e.button === 2) {
                          supprimerClip(clip.id)
                          return
                        }
                        if (e.button !== 0) return
                        // double clic : deux appuis rapprochés sur le même clip (la capture du pointeur
                        // pour le glisser enverrait sinon le double clic à la zone, pas au clip)
                        const precedent = dernierAppui.current
                        dernierAppui.current = { id: clip.id, t: e.timeStamp }
                        if (precedent?.id === clip.id && e.timeStamp - precedent.t < 400) {
                          choisirPattern(pattern.id)
                          return
                        }
                        zone.current!.setPointerCapture(e.pointerId)
                        glisse.current = { id: clip.id, departTic: clip.debut, departPiste: clip.piste, x0: e.clientX, y0: e.clientY }
                      }}
                      className={`absolute cursor-grab overflow-hidden rounded-[3px] border text-[10px] font-semibold leading-3 text-black ${
                        pattern.id === projet.patternActif ? 'border-white' : 'border-black/60'
                      }`}
                      style={{
                        left: clip.debut * pxParTic,
                        top: clip.piste * H_PISTE + 2,
                        width: Math.max(4, pattern.longueur * pxParTic - 1),
                        height: H_PISTE - 4,
                        background: couleurPattern(projet, pattern.id),
                      }}
                    >
                      <span className="pointer-events-none block truncate px-1 pt-0.5">{pattern.nom}</span>
                      <ApercuClip pattern={pattern} />
                    </div>
                  )
                })}
                {tete >= 0 && <div className="pointer-events-none absolute inset-y-0 w-0.5 bg-actif" style={{ left: tete * pxParTic }} />}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
