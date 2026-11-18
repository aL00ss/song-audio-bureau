// Vue de l'instrument sélectionné : elle s'allume pendant la lecture et se joue à la souris, au
// doigt et au clavier de l'ordinateur. Clavier pour les instruments mélodiques ; pads disposés
// comme une batterie pour les canaux de batterie. Les vues dédiées (manche, cordes, vents)
// arrivent à l'étape 7.

import { nomDuSon } from '../../musique/synthes'
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { audio, notesEnDirect, useStudio } from '../../etat/studio'
import { caractere, disposition, etiquettes, HAUTEUR_NOIRE, nombreDeBlanches, toucheSous } from '../../musique/clavier'
import { familleInstrument, nomNote, nomSonBatterie, vueInstrument } from '../../musique/gm'
import { type Canal, notesAuTic, patternActif } from '../../projet/modele'
import { useImage } from '../commun/useImage'
import { accordagePour } from '../../musique/manche'
import { CUIVRES_A_PISTONS } from '../../musique/vents'
import { Harpe, Manche, Pistons } from './VuesAvancees'

const TOUCHES = disposition()
const NB_BLANCHES = nombreDeBlanches(TOUCHES)
const egal = (a: string, b: string) => a === b

/** Hauteurs qui sonnent sur ces canaux : pattern à la position de lecture, et jeu en direct. */
function useNotesActives(canaux: readonly Canal[]): Set<number> {
  const cle = useImage(() => {
    const { projet } = useStudio.getState()
    const hauteurs = new Set<number>()
    const t = audio.transport
    if (t?.etat === 'lecture') {
      const actives = notesAuTic(patternActif(projet), t.position)
      for (const c of canaux) for (const n of actives.get(c.id) ?? []) hauteurs.add(n.hauteur)
    }
    for (const c of canaux) for (const h of notesEnDirect(c.id)) hauteurs.add(h)
    return [...hauteurs].sort((a, b) => a - b).join(',')
  }, egal)
  return useMemo(() => new Set(cle ? cle.split(',').map(Number) : []), [cle])
}

function Clavier({ canal }: { canal: Canal }) {
  const { clavier, jouerNote, relacherNote, changerDisposition } = useStudio()
  const actives = useNotesActives([canal])
  const racine = useRef<HTMLDivElement>(null)
  const [largeur, setLargeur] = useState(18)
  const pointeurs = useRef(new Map<number, number>())
  const lettres = etiquettes(clavier.base, clavier.disposition)

  useLayoutEffect(() => {
    const el = racine.current
    if (!el) return
    const observer = new ResizeObserver(() => setLargeur(Math.max(16, el.clientWidth / NB_BLANCHES)))
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  // garder visible l'octave jouée au clavier de l'ordinateur
  useEffect(() => {
    const el = racine.current
    const reperes = TOUCHES.find((t) => t.midi === clavier.base + 12)
    if (el && reperes) el.scrollLeft = reperes.x * largeur - el.clientWidth / 2 + 3.5 * largeur
  }, [clavier.base, largeur])

  const noteSous = (e: React.PointerEvent<HTMLDivElement>) => {
    const piste = e.currentTarget.getBoundingClientRect()
    const x = (e.clientX - piste.left) / largeur
    const y = (e.clientY - piste.top) / piste.height
    const midi = toucheSous(TOUCHES, x, Math.min(1, Math.max(0, y)))
    if (midi === null) return null
    const hauteur = TOUCHES.find((t) => t.midi === midi)!.noire ? HAUTEUR_NOIRE : 1
    return { midi, velocite: 40 + 87 * Math.min(1, Math.max(0, y / hauteur)) }
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-3 border-b border-black/50 px-2 py-1 text-discret">
        <span>
          <span className="font-semibold" style={{ color: canal.couleur }}>{canal.nom}</span> · {nomDuSon(canal)} ({canal.source === 'soustractif' ? 'synthé intégré' : familleInstrument(canal.programme)})
        </span>
        <span>
          Clavier{' '}
          <button type="button" onClick={() => changerDisposition()} className="rounded border border-bord px-1 font-semibold text-texte" title="Changer la disposition du clavier">
            {clavier.disposition.toUpperCase()}
          </button>{' '}
          : {caractere('KeyZ', clavier.disposition)}…{caractere('Slash', clavier.disposition)} et {caractere('KeyQ', clavier.disposition)}…{caractere('BracketRight', clavier.disposition)} · ← → octave
        </span>
        {vueInstrument(canal.programme) === 'vents' && !CUIVRES_A_PISTONS[canal.programme] && (
          <span className="ml-auto" title="Les doigtés des bois (clés de flûte, de saxophone…) ne sont pas représentés">Bois : joués au clavier</span>
        )}
      </div>
      <div ref={racine} className="relative min-h-0 flex-1 overflow-x-auto overflow-y-hidden">
        <div
          data-testid="clavier"
          className="relative h-full min-h-[90px] touch-none select-none"
          style={{ width: largeur * NB_BLANCHES }}
          onPointerDown={(e) => {
            if (e.pointerType === 'mouse' && e.button !== 0) return
            const cible = noteSous(e)
            if (!cible) return
            e.currentTarget.setPointerCapture(e.pointerId)
            pointeurs.current.set(e.pointerId, cible.midi)
            jouerNote(cible.midi, cible.velocite, canal.id)
          }}
          onPointerMove={(e) => {
            const actuelle = pointeurs.current.get(e.pointerId)
            if (actuelle === undefined) return
            const cible = noteSous(e)
            if (!cible || cible.midi === actuelle) return
            relacherNote(actuelle, canal.id) // glissando
            pointeurs.current.set(e.pointerId, cible.midi)
            jouerNote(cible.midi, cible.velocite, canal.id)
          }}
          onPointerUp={(e) => {
            const midi = pointeurs.current.get(e.pointerId)
            pointeurs.current.delete(e.pointerId)
            if (midi !== undefined) relacherNote(midi, canal.id)
          }}
          onPointerCancel={(e) => {
            const midi = pointeurs.current.get(e.pointerId)
            pointeurs.current.delete(e.pointerId)
            if (midi !== undefined) relacherNote(midi, canal.id)
          }}
          onContextMenu={(e) => e.preventDefault()}
        >
          {TOUCHES.map((t) => {
            const active = actives.has(t.midi)
            return (
              <div
                key={t.midi}
                data-midi={t.midi}
                data-active={active || undefined}
                title={nomNote(t.midi)}
                className={`absolute top-0 flex flex-col items-center justify-end rounded-b ${
                  t.noire ? 'z-[1] h-[63%] border border-black bg-gradient-to-b from-[#2a2d33] to-[#0f1013] text-[#bbb]' : 'h-full border border-[#888] bg-gradient-to-b from-[#f4f1ea] to-[#dcd7cb] text-[#555]'
                }`}
                style={{
                  left: t.x * largeur,
                  width: t.largeur * largeur,
                  ...(active ? { background: canal.couleur, boxShadow: `0 0 8px ${canal.couleur}` } : {}),
                }}
              >
                {lettres.has(t.midi) && <span className="text-[10px] font-semibold">{lettres.get(t.midi)}</span>}
                {t.midi % 12 === 0 && <span className="pb-0.5 text-[9px]">{nomNote(t.midi)}</span>}
              </div>
            )
          })}
        </div>
      </div>
    </div>
  )
}

/** Pads disposés comme une batterie : cymbales, charleston, toms, caisses, grosse caisse, percussions. */
const KIT: readonly { titre: string; sons: number[] }[] = [
  { titre: 'Cymbales', sons: [49, 57, 55, 52, 51, 53] },
  { titre: 'Charleston', sons: [42, 44, 46] },
  { titre: 'Toms', sons: [50, 48, 47, 45, 43, 41] },
  { titre: 'Caisses', sons: [38, 40, 37, 39] },
  { titre: 'Grosse caisse', sons: [36, 35] },
  { titre: 'Percussions', sons: [54, 56, 69, 70, 75, 76, 77, 60, 61, 62, 63, 64, 80, 81] },
]

function Batterie({ canal }: { canal: Canal }) {
  const { projet, jouerNote, relacherNote } = useStudio()
  const batteries = projet.canaux.filter((c) => c.batterie)
  const actives = useNotesActives(batteries)
  /** Un pad joue la ligne de batterie de ce son si elle existe, sinon le canal sélectionné. */
  const canalPour = (note: number) => batteries.find((c) => c.noteDefaut === note)?.id ?? canal.id
  return (
    <div className="flex h-full flex-col gap-1 overflow-auto p-2">
      <p className="text-discret">
        Batterie : <span style={{ color: canal.couleur }}>{canal.nom}</span> · les pads jouent le kit General MIDI
      </p>
      <div className="flex flex-wrap gap-3">
        {KIT.map((groupe) => (
          <div key={groupe.titre} role="group" aria-label={groupe.titre} className="flex flex-col gap-1">
            <span className="text-[10px] uppercase tracking-wider text-discret">{groupe.titre}</span>
            <div className="flex flex-wrap gap-1">
              {groupe.sons.map((note) => {
                const active = actives.has(note)
                return (
                  <button
                    key={note}
                    type="button"
                    data-pad={note}
                    data-active={active || undefined}
                    aria-label={nomSonBatterie(note)}
                    title={`${nomSonBatterie(note)} (${note})`}
                    onPointerDown={(e) => {
                      e.currentTarget.setPointerCapture(e.pointerId)
                      jouerNote(note, 110, canalPour(note))
                    }}
                    onPointerUp={() => relacherNote(note, canalPour(note))}
                    onPointerCancel={() => relacherNote(note, canalPour(note))}
                    className="flex h-12 w-[4.5rem] items-end rounded-md border border-black/60 bg-gradient-to-b from-[#3d444e] to-[#2a2f36] p-1 text-left text-[10px] leading-tight shadow-[inset_0_1px_0_#4c5662] data-active:border-accent"
                    style={active ? { background: canal.couleur, color: '#111', boxShadow: `0 0 10px ${canal.couleur}` } : undefined}
                  >
                    {nomSonBatterie(note)}
                  </button>
                )
              })}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

/** « Jouer avec l'orchestre » : la partie du canal se tait, s'allume en guide, et l'on compte les notes justes. */
function BarrePartie({ canal }: { canal: Canal }) {
  const { partie, commencerPartie, arreterPartie } = useStudio()
  const enCours = partie?.canalId === canal.id
  const total = enCours ? partie.justes + partie.fausses : 0
  return (
    <div className="flex shrink-0 items-center gap-2 border-b border-bord bg-panneau-2 px-2 py-1 text-[12px]">
      <button
        type="button"
        data-testid="jouer-partie"
        aria-pressed={enCours}
        onClick={() => (enCours ? arreterPartie() : commencerPartie(canal.id))}
        title="La partie de cet instrument se tait et sert de guide : jouez-la vous-même avec le reste du morceau"
        className="h-6 rounded border border-bord px-2 hover:border-discret aria-pressed:border-accent aria-pressed:bg-accent aria-pressed:font-semibold aria-pressed:text-black"
      >
        {enCours ? 'Arrêter de jouer la partie' : 'Jouer la partie'}
      </button>
      {enCours && (
        <span data-testid="score-partie" className="text-discret" aria-live="polite">
          <span className="text-actif">{partie.justes} justes</span> · <span className="text-red-400">{partie.fausses} fausses</span>
          {total > 0 && <> · {Math.round((partie.justes / total) * 100)} %</>}
        </span>
      )}
    </div>
  )
}

function VueDuCanal({ canal }: { canal: Canal }) {
  if (canal.batterie) return <Batterie canal={canal} />
  const accordage = accordagePour(canal.programme)
  if (accordage) return <Manche canal={canal} accordage={accordage} />
  if (canal.programme === 46) return <Harpe canal={canal} />
  if (CUIVRES_A_PISTONS[canal.programme]) {
    return (
      <div className="flex h-full flex-col">
        <Pistons canal={canal} />
        <div className="min-h-0 flex-1">
          <Clavier canal={canal} />
        </div>
      </div>
    )
  }
  return <Clavier canal={canal} />
}

export function VueInstrument() {
  const { projet, canalSelectionne } = useStudio()
  const canal = projet.canaux.find((c) => c.id === canalSelectionne)
  if (!canal) return <p className="p-3 text-discret">Sélectionnez un instrument dans le Channel Rack.</p>
  return (
    <div className="flex h-full flex-col">
      <BarrePartie canal={canal} />
      <div className="min-h-0 flex-1">
        <VueDuCanal canal={canal} />
      </div>
    </div>
  )
}
