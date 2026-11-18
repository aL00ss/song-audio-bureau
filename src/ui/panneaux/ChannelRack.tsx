// Channel Rack : une ligne par instrument (diode d'activité, muet, panoramique, volume, piste
// de mixage, nom, solo) suivie de sa grille de pas, ou d'un aperçu des notes quand elles ne
// tiennent pas sur des pas (morceau importé). Sous les lignes, la vélocité de chaque pas du
// canal sélectionné.

import { useEffect, useRef } from 'react'
import { audio, useStudio } from '../../etat/studio'
import { nomInstrument, nomSonBatterie } from '../../musique/gm'
import { TICS_PAR_PAS } from '../../musique/temps'
import { attribuerCanauxMidi, type Canal, type NoteMusicale, patternActif } from '../../projet/modele'
import { estEnModePas, nombreDePas, PAS_MAX, velocitesDesPas } from '../../projet/pas'
import { BoutonRotatif } from '../commun/BoutonRotatif'
import { tableauxEgaux, useImage } from '../commun/useImage'

const SONS_BATTERIE = Array.from({ length: 81 - 35 + 1 }, (_, i) => 35 + i)

/** Pas joué en ce moment (-1 à l'arrêt). */
function usePasCourant(): number {
  return useImage(() => (audio.transport?.etat === 'lecture' ? Math.floor(audio.transport.position / TICS_PAR_PAS) : -1))
}

/** Peinture à la souris : le premier pas touché décide si l'on allume ou si l'on éteint. */
const peinture: { canal: string | null; actif: boolean } = { canal: null, actif: true }

function GrillePas({ canal, velocites, pasCourant }: { canal: Canal; velocites: number[]; pasCourant: number }) {
  const definirPas = useStudio((s) => s.definirPas)
  useEffect(() => {
    const fin = () => {
      peinture.canal = null
    }
    window.addEventListener('pointerup', fin)
    window.addEventListener('pointercancel', fin)
    return () => {
      window.removeEventListener('pointerup', fin)
      window.removeEventListener('pointercancel', fin)
    }
  }, [])
  return (
    <div role="group" aria-label={`Pas de ${canal.nom}`} data-testid="grille-pas" className="flex gap-px" onContextMenu={(e) => e.preventDefault()}>
      {velocites.map((velocite, i) => {
        const actif = velocite > 0
        const groupe = Math.floor(i / 4) % 2 === 0
        return (
          <button
            key={i}
            type="button"
            data-pas={i}
            aria-pressed={actif}
            aria-label={`Pas ${i + 1} ${canal.nom}`}
            onPointerDown={(e) => {
              if (e.button !== 0 && e.button !== 2) return
              e.currentTarget.releasePointerCapture?.(e.pointerId) // laisse le survol passer aux pas voisins
              peinture.canal = canal.id
              peinture.actif = e.button === 2 ? false : !actif
              definirPas(canal.id, i, peinture.actif)
            }}
            onPointerEnter={() => {
              if (peinture.canal === canal.id) definirPas(canal.id, i, peinture.actif)
            }}
            onClick={(e) => {
              if (e.detail === 0) definirPas(canal.id, i) // clavier (Entrée)
            }}
            className={`h-6 w-[18px] shrink-0 rounded-[3px] border border-black/60 ${i % 4 === 0 ? 'ml-1 first:ml-0' : ''} ${
              actif ? '' : groupe ? 'bg-[#3d434c]' : 'bg-[#4b3a40]'
            } ${pasCourant === i ? 'outline outline-1 outline-actif' : ''}`}
            style={actif ? { background: canal.couleur, opacity: 0.45 + (velocite / 127) * 0.55 } : undefined}
          />
        )
      })}
    </div>
  )
}

/** Aperçu des notes d'un canal sur toute la longueur du pattern. */
function Apercu({ notes, longueur, couleur, largeur }: { notes: readonly NoteMusicale[]; longueur: number; couleur: string; largeur: number }) {
  if (!notes.length) return <div className="h-6 rounded-sm bg-black/30" style={{ width: largeur }} />
  const basse = Math.min(...notes.map((n) => n.hauteur))
  const haute = Math.max(...notes.map((n) => n.hauteur))
  const etendue = Math.max(12, haute - basse + 1)
  return (
    <svg
      data-testid="apercu"
      viewBox={`0 0 ${longueur} ${etendue}`}
      preserveAspectRatio="none"
      className="h-6 rounded-sm bg-black/30"
      style={{ width: largeur }}
      aria-label="Aperçu des notes (le piano roll les édite)"
    >
      {notes.map((n, i) => (
        <rect key={i} x={n.tic} y={haute - n.hauteur} width={Math.max(n.duree, longueur / 400)} height={1} fill={couleur} opacity={0.4 + (n.velocite / 127) * 0.6} />
      ))}
    </svg>
  )
}

const largeurGrille = (pas: number) => pas * 19 + Math.floor((pas - 1) / 4) * 4
/** Largeur fixe de la zone de contrôles d'une ligne : la grille de vélocité s'aligne dessus. */
const LARGEUR_CONTROLES = 'w-[426px]'

function Ligne({ canal, voix, selectionne, pasCourant }: { canal: Canal; voix: number; selectionne: boolean; pasCourant: number }) {
  const { projet, modifierCanal, selectionnerCanal, definirNoteDefaut, routerCanal } = useStudio()
  const pattern = patternActif(projet)
  const notes = pattern.notes[canal.id] ?? []
  const enPas = estEnModePas(notes, canal)
  return (
    <div
      data-testid="canal"
      data-selectionne={selectionne || undefined}
      data-mode={enPas ? 'pas' : 'apercu'}
      className={`flex w-max min-w-full items-center gap-2 border-b border-black/40 py-1 pr-2 ${selectionne ? 'bg-[#323944]' : ''}`}
    >
      <div className={`sticky left-0 z-[1] flex ${LARGEUR_CONTROLES} shrink-0 items-center gap-1.5 pl-1.5 pr-1 ${selectionne ? 'bg-[#323944]' : 'bg-panneau'}`}>
        <span
          aria-label={voix > 0 ? 'joue' : 'silencieux'}
          data-voix={voix}
          className="size-2 shrink-0 rounded-full"
          style={{ background: voix > 0 ? canal.couleur : '#3a414b', boxShadow: voix > 0 ? `0 0 6px ${canal.couleur}` : undefined }}
        />
        <input
          type="checkbox"
          aria-label={`Couper ${canal.nom}`}
          title="Couper (muet)"
          checked={canal.muet}
          onChange={(e) => modifierCanal(canal.id, { muet: e.target.checked })}
          className="size-3.5 shrink-0 cursor-pointer appearance-none rounded-full border border-black/60 bg-actif checked:bg-[#3a414b]"
        />
        <BoutonRotatif
          etiquette={`Panoramique ${canal.nom}`}
          valeur={canal.pan}
          min={-1}
          max={1}
          ancre={0}
          defaut={0}
          texte={(v) => (Math.abs(v) < 0.01 ? 'centre' : `${Math.round(Math.abs(v) * 100)} % ${v < 0 ? 'gauche' : 'droite'}`)}
          surChangement={(v) => modifierCanal(canal.id, { pan: v })}
        />
        <BoutonRotatif
          etiquette={`Volume ${canal.nom}`}
          valeur={canal.volume}
          defaut={0.8}
          texte={(v) => `${Math.round(v * 100)} %`}
          surChangement={(v) => modifierCanal(canal.id, { volume: v })}
        />
        <select
          aria-label={`Piste de mixage de ${canal.nom}`}
          title="Piste du mixer qui reçoit ce canal"
          value={canal.pisteMixage}
          onChange={(e) => routerCanal(canal.id, Number(e.target.value))}
          className="h-5 w-9 shrink-0 appearance-none rounded border border-black/50 bg-[#1f242a] text-center font-mono text-[11px] text-discret"
        >
          {projet.mixage.map((m) => (
            <option key={m.index} value={m.index}>{m.index === 0 ? 'M' : m.index}</option>
          ))}
        </select>
        <button
          type="button"
          data-testid="nom-canal"
          onClick={() => selectionnerCanal(canal.id)}
          title={canal.batterie ? `Batterie : ${nomSonBatterie(canal.noteDefaut)}` : nomInstrument(canal.programme)}
          aria-pressed={selectionne}
          className="h-6 w-32 shrink-0 truncate rounded border border-black/50 bg-gradient-to-b from-[#4a525d] to-[#3b424b] px-2 text-left font-semibold aria-pressed:border-accent/70"
          style={{ color: canal.couleur }}
        >
          {canal.nom}
        </button>
        <label className="flex shrink-0 items-center gap-0.5 text-[10px] text-discret" title="Solo">
          <input
            type="checkbox"
            aria-label={`Solo ${canal.nom}`}
            checked={canal.solo}
            onChange={(e) => modifierCanal(canal.id, { solo: e.target.checked })}
            className="accent-accent"
          />
          S
        </label>
        {canal.batterie && enPas ? (
          <select
            aria-label={`Son de ${canal.nom}`}
            value={canal.noteDefaut}
            onChange={(e) => definirNoteDefaut(canal.id, Number(e.target.value))}
            className="h-6 w-28 shrink-0 rounded border border-black/50 bg-[#1f242a] text-[11px] text-texte"
          >
            {SONS_BATTERIE.map((n) => (
              <option key={n} value={n}>{nomSonBatterie(n)}</option>
            ))}
          </select>
        ) : (
          <span className="w-28 shrink-0" />
        )}
      </div>
      {enPas ? (
        <GrillePas canal={canal} velocites={velocitesDesPas(pattern, canal)} pasCourant={pasCourant} />
      ) : (
        <Apercu notes={notes} longueur={pattern.longueur} couleur={canal.couleur} largeur={Math.min(largeurGrille(nombreDePas(pattern)), 900)} />
      )}
    </div>
  )
}

/** Vélocité de chaque pas actif du canal sélectionné : on la règle en glissant verticalement. */
function EditeurVelocite({ canal }: { canal: Canal }) {
  const { projet, definirVelocitePas } = useStudio()
  const velocites = velocitesDesPas(patternActif(projet), canal)
  const glisse = useRef(false)
  const regler = (e: React.PointerEvent<HTMLDivElement>, i: number) => {
    const cadre = e.currentTarget.getBoundingClientRect()
    const v = Math.round((1 - (e.clientY - cadre.top) / cadre.height) * 127)
    if (velocites[i] > 0) definirVelocitePas(canal.id, i, v)
  }
  return (
    <div className="flex w-max min-w-full items-end gap-2 border-t border-black/50 bg-[#1f242a] py-1 pr-2">
      <div className={`sticky left-0 z-[1] ${LARGEUR_CONTROLES} shrink-0 self-stretch bg-[#1f242a] pl-2 text-discret`}>
        Vélocité : <span style={{ color: canal.couleur }}>{canal.nom}</span>
      </div>
      <div className="flex gap-px" data-testid="velocites" onPointerUp={() => (glisse.current = false)} onPointerLeave={() => (glisse.current = false)}>
        {velocites.map((v, i) => (
          <div
            key={i}
            data-pas={i}
            role="slider"
            aria-label={`Vélocité du pas ${i + 1}`}
            aria-valuemin={1}
            aria-valuemax={127}
            aria-valuenow={v || undefined}
            aria-disabled={v === 0}
            tabIndex={v > 0 ? 0 : -1}
            onPointerDown={(e) => {
              glisse.current = true
              regler(e, i)
            }}
            onPointerMove={(e) => glisse.current && regler(e, i)}
            onKeyDown={(e) => {
              if (v === 0) return
              const delta = e.key === 'ArrowUp' ? 4 : e.key === 'ArrowDown' ? -4 : 0
              if (delta) {
                e.preventDefault()
                definirVelocitePas(canal.id, i, v + delta)
              }
            }}
            className={`relative h-10 w-[18px] shrink-0 rounded-[2px] bg-black/40 ${i % 4 === 0 ? 'ml-1 first:ml-0' : ''} ${v ? 'cursor-ns-resize' : ''}`}
          >
            {v > 0 && <span className="absolute inset-x-0 bottom-0 rounded-[2px]" style={{ height: `${(v / 127) * 100}%`, background: canal.couleur }} />}
          </div>
        ))}
      </div>
    </div>
  )
}

export function ChannelRack() {
  const { projet, canalSelectionne, definirNombreDePas, changerSwing } = useStudio()
  const pattern = patternActif(projet)
  const attribution = attribuerCanauxMidi(projet.canaux)
  const voix = useImage(() => projet.canaux.map((c) => audio.moteur?.voix(attribution.get(c.id) ?? -1) ?? 0), tableauxEgaux)
  const pasCourant = usePasCourant()
  const selection = projet.canaux.find((c) => c.id === canalSelectionne)
  const selectionEnPas = selection ? estEnModePas(pattern.notes[selection.id] ?? [], selection) : false

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b border-black/50 bg-panneau px-2 py-1 text-discret">
        <span>
          <span className="font-semibold text-texte">{pattern.nom}</span> · {projet.canaux.length} instruments
        </span>
        <label className="flex items-center gap-1" title="Longueur du pattern, en pas (doubles croches)">
          Pas
          <input
            type="number"
            data-testid="nombre-pas"
            min={1}
            max={PAS_MAX}
            value={nombreDePas(pattern)}
            onChange={(e) => {
              const n = Number(e.target.value)
              if (Number.isFinite(n) && n >= 1) definirNombreDePas(n)
            }}
            className="h-6 w-14 rounded border border-black/60 bg-[#0d1013] px-1 text-right font-mono text-accent"
          />
        </label>
        <span className="flex items-center gap-1" title="Swing : retarde les pas impairs">
          Swing
          <BoutonRotatif
            etiquette="Swing"
            valeur={projet.swing}
            defaut={0}
            texte={(v) => `${Math.round(v * 100)} %`}
            surChangement={changerSwing}
          />
        </span>
        <span className="ml-auto text-[11px]">Clic : allumer/éteindre · glisser : peindre · clic droit : effacer</span>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {projet.canaux.map((canal, i) => (
          <Ligne key={canal.id} canal={canal} voix={voix[i] ?? 0} selectionne={canal.id === canalSelectionne} pasCourant={pasCourant} />
        ))}
        {!projet.canaux.length && <p className="p-3 text-discret">Aucun instrument : ajoutez-en depuis le navigateur (F8).</p>}
        {selection && selectionEnPas && <EditeurVelocite canal={selection} />}
      </div>
    </div>
  )
}
