// Vues dédiées : manche (guitare, basse, cordes frottées), harpe, pistons des cuivres. Elles
// s'allument pendant la lecture et se jouent à la souris.

import { useMemo, useRef } from 'react'
import { audio, notesEnDirect, useStudio } from '../../etat/studio'
import { nomNote } from '../../musique/gm'
import { ACCORDAGES, doigter, type NomAccordage, type Position, positionCase, positionsPossibles } from '../../musique/manche'
import { CORDES_HARPE, cordeHarpe, CUIVRES_A_PISTONS, pistons } from '../../musique/vents'
import { type Canal, type NoteMusicale, patternActif } from '../../projet/modele'
import { useImage } from '../commun/useImage'

/** Notes du canal qui sonnent : du pattern à la position de lecture, et jouées en direct. */
function useNotesQuiSonnent(canal: Canal): { pattern: NoteMusicale[]; direct: number[] } {
  const cle = useImage(() => {
    const t = audio.transport
    const notes = patternActif(useStudio.getState().projet).notes[canal.id] ?? []
    const indices: number[] = []
    if (t?.etat === 'lecture') {
      const p = t.position
      notes.forEach((n, i) => {
        if (n.tic <= p && p < n.tic + Math.max(n.duree, 8)) indices.push(i)
      })
    }
    return `${indices.join(',')}|${[...notesEnDirect(canal.id)].sort((a, b) => a - b).join(',')}`
  })
  const notes = patternActif(useStudio.getState().projet).notes[canal.id] ?? []
  const [partieA, partieB] = cle.split('|')
  return {
    pattern: partieA ? partieA.split(',').map((i) => notes[Number(i)]).filter(Boolean) : [],
    direct: partieB ? partieB.split(',').map(Number) : [],
  }
}

// ------------------------------------------------------------------------ manche

const REPERES = [3, 5, 7, 9, 15, 17, 19]
const AUCUNE_NOTE: NoteMusicale[] = []

export function Manche({ canal, accordage: nomAccordage }: { canal: Canal; accordage: NomAccordage }) {
  const { projet, jouerNote, relacherNote } = useStudio()
  const accordage = ACCORDAGES[nomAccordage]
  const notes = patternActif(projet).notes[canal.id] ?? AUCUNE_NOTE
  // doigté de tout le pattern : les notes qui commencent ensemble forment un accord
  const doigtes = useMemo(() => {
    const parTic = new Map<number, number[]>()
    notes.forEach((n, i) => parTic.set(n.tic, [...(parTic.get(n.tic) ?? []), i]))
    const groupes = [...parTic.entries()].sort((a, b) => a[0] - b[0]).map(([, indices]) => indices)
    const resultat = doigter(groupes.map((g) => g.map((i) => notes[i].hauteur)), accordage)
    const parNote = new Map<NoteMusicale, Position | null>()
    groupes.forEach((g, k) => g.forEach((i, j) => parNote.set(notes[i], resultat[k].positions[j])))
    return parNote
  }, [notes, accordage])
  const { pattern, direct } = useNotesQuiSonnent(canal)
  const allumees: { position: Position; direct: boolean }[] = []
  for (const n of pattern) {
    const p = doigtes.get(n)
    if (p) allumees.push({ position: p, direct: false })
  }
  for (const h of direct) {
    const p = positionsPossibles(h, accordage).sort((a, b) => a.case - b.case)[0]
    if (p) allumees.push({ position: p, direct: true })
  }

  const L = 1000
  const H = 40 + accordage.cordes.length * 26
  // noms des cordes à gauche, puis les cordes à vide, puis le sillet (x = SILLET)
  const SILLET = 62
  const xCase = (c: number) => SILLET + positionCase(c, accordage.cases) * (L - SILLET - 20)
  const centreCase = (c: number) => (c === 0 ? SILLET - 14 : (xCase(c - 1) + xCase(c)) / 2)
  const yCorde = (corde: number) => 20 + (accordage.cordes.length - 1 - corde) * 26 // corde aiguë en haut
  const tenue = useRef<{ note: number } | null>(null)

  const noteSous = (e: React.PointerEvent<SVGSVGElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    const x = ((e.clientX - r.left) / r.width) * L
    const y = ((e.clientY - r.top) / r.height) * H
    const corde = accordage.cordes.length - 1 - Math.round((y - 20) / 26)
    if (corde < 0 || corde >= accordage.cordes.length) return null
    // case 0 : à gauche du sillet ; sinon la première case dont la frette est à droite du point
    let c = 0
    if (x > SILLET) {
      c = accordage.cases
      for (let k = 1; k <= accordage.cases; k++) {
        if (x <= xCase(k)) {
          c = k
          break
        }
      }
    }
    return accordage.cordes[corde] + c
  }

  return (
    <div className="flex h-full flex-col">
      <p className="shrink-0 px-2 py-1 text-discret">
        <span className="font-semibold" style={{ color: canal.couleur }}>{canal.nom}</span> · {accordage.nom}, {accordage.cordes.length} cordes (
        {accordage.cordes.map(nomNote).join(' ')}) · {accordage.frettes ? 'doigté calculé pour garder la main resserrée' : 'touche sans frettes'}
      </p>
      <svg
        data-testid="manche"
        viewBox={`0 0 ${L} ${H}`}
        preserveAspectRatio="none"
        className="min-h-0 w-full flex-1 touch-none select-none"
        onPointerDown={(e) => {
          const note = noteSous(e)
          if (note === null) return
          e.currentTarget.setPointerCapture(e.pointerId)
          tenue.current = { note }
          jouerNote(note, 100, canal.id)
        }}
        onPointerUp={() => {
          if (tenue.current) relacherNote(tenue.current.note, canal.id)
          tenue.current = null
        }}
        onPointerCancel={() => {
          if (tenue.current) relacherNote(tenue.current.note, canal.id)
          tenue.current = null
        }}
      >
        <rect x={SILLET} y={8} width={L - SILLET - 20} height={H - 16} rx={4} fill={accordage.frettes ? '#3b2a1e' : '#1c1612'} />
        {accordage.frettes &&
          REPERES.filter((c) => c <= accordage.cases).map((c) => (
            <circle key={c} cx={centreCase(c)} cy={H / 2} r={5} fill="#d8cbb0" opacity={0.6} />
          ))}
        {accordage.frettes && accordage.cases >= 12 && (
          <>
            <circle cx={centreCase(12)} cy={H / 2 - 18} r={5} fill="#d8cbb0" opacity={0.6} />
            <circle cx={centreCase(12)} cy={H / 2 + 18} r={5} fill="#d8cbb0" opacity={0.6} />
          </>
        )}
        <rect x={SILLET - 4} y={8} width={6} height={H - 16} fill="#e9e1cf" />
        {accordage.frettes && Array.from({ length: accordage.cases }, (_, k) => k + 1).map((c) => (
          <rect key={c} x={xCase(c) - 1} y={8} width={2.5} height={H - 16} fill="#b8b8b8" />
        ))}
        {accordage.cordes.map((vide, corde) => (
          <g key={corde}>
            <line x1={SILLET - 26} x2={L - 20} y1={yCorde(corde)} y2={yCorde(corde)} stroke="#d9d4c7" strokeWidth={1 + (accordage.cordes.length - corde) * 0.35} />
            <text x={4} y={yCorde(corde) + 4} fontSize={12} fill="#8b949e">{nomNote(vide)}</text>
          </g>
        ))}
        {allumees.map(({ position, direct: enDirect }, i) => (
          <circle
            key={i}
            data-corde={position.corde}
            data-case={position.case}
            cx={centreCase(position.case)}
            cy={yCorde(position.corde)}
            r={9}
            fill={enDirect ? '#f5a524' : canal.couleur}
            stroke="#000"
          />
        ))}
      </svg>
    </div>
  )
}

// ------------------------------------------------------------------------ harpe

export function Harpe({ canal }: { canal: Canal }) {
  const { jouerNote, relacherNote } = useStudio()
  const { pattern, direct } = useNotesQuiSonnent(canal)
  const allumees = new Set([...pattern.map((n) => cordeHarpe(n.hauteur)), ...direct.map(cordeHarpe)])
  const pointeur = useRef<number | null>(null)
  const cordeSous = (e: React.PointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    const i = Math.floor(((e.clientX - r.left) / r.width) * CORDES_HARPE.length)
    return CORDES_HARPE[Math.min(CORDES_HARPE.length - 1, Math.max(0, i))]
  }
  return (
    <div className="flex h-full flex-col">
      <p className="shrink-0 px-2 py-1 text-discret">
        <span className="font-semibold" style={{ color: canal.couleur }}>{canal.nom}</span> · {CORDES_HARPE.length} cordes (do rouges, fa bleues) · glisser pour un glissando
      </p>
      <div
        data-testid="harpe"
        className="flex min-h-0 flex-1 touch-none select-none items-stretch gap-px px-2 pb-2"
        onPointerDown={(e) => {
          const c = cordeSous(e)
          e.currentTarget.setPointerCapture(e.pointerId)
          pointeur.current = c
          jouerNote(c, 90, canal.id)
        }}
        onPointerMove={(e) => {
          if (pointeur.current === null) return
          const c = cordeSous(e)
          if (c === pointeur.current) return
          relacherNote(pointeur.current, canal.id)
          pointeur.current = c
          jouerNote(c, 90, canal.id)
        }}
        onPointerUp={() => {
          if (pointeur.current !== null) relacherNote(pointeur.current, canal.id)
          pointeur.current = null
        }}
      >
        {CORDES_HARPE.map((c, i) => {
          const couleur = c % 12 === 0 ? '#d9534f' : c % 12 === 5 ? '#4a7fd6' : '#d9d4c7'
          const actif = allumees.has(c)
          return (
            <div key={c} data-corde-harpe={c} data-active={actif || undefined} className="flex flex-1 justify-center" title={nomNote(c)}>
              <div
                className="rounded-full"
                style={{
                  width: actif ? 4 : 2,
                  marginTop: `${(i / CORDES_HARPE.length) * 40}%`,
                  background: actif ? canal.couleur : couleur,
                  boxShadow: actif ? `0 0 8px ${canal.couleur}` : undefined,
                }}
              />
            </div>
          )
        })}
      </div>
    </div>
  )
}

// ------------------------------------------------------------------------ pistons

export function Pistons({ canal }: { canal: Canal }) {
  const { pattern, direct } = useNotesQuiSonnent(canal)
  const instrument = CUIVRES_A_PISTONS[canal.programme]
  const hauteurs = [...pattern.map((n) => n.hauteur), ...direct]
  const note = hauteurs.length ? Math.max(...hauteurs) : null
  const enfonces = note === null ? null : pistons(note, instrument.partiels)
  return (
    <div className="flex shrink-0 items-center gap-4 border-b border-black/50 px-3 py-2" data-testid="pistons">
      <span className="text-discret">
        <span className="font-semibold" style={{ color: canal.couleur }}>{instrument.nom}</span> · pistons
      </span>
      {[1, 2, 3].map((p) => {
        const bas = enfonces?.includes(p) ?? false
        return (
          <span
            key={p}
            data-piston={p}
            data-enfonce={bas || undefined}
            className="flex size-9 items-center justify-center rounded-full border-2 border-black/70 font-bold"
            style={{
              background: bas ? canal.couleur : 'linear-gradient(#e2d6a8, #b49b4c)',
              transform: bas ? 'translateY(3px)' : undefined,
              color: '#222',
            }}
          >
            {p}
          </span>
        )
      })}
      <span className="font-mono text-texte" data-testid="note-cuivre">
        {note === null ? '—' : enfonces === null ? `${nomNote(note)} (hors tessiture)` : nomNote(note)}
      </span>
    </div>
  )
}
