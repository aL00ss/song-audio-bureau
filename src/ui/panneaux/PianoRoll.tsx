// Piano roll de l'instrument sélectionné, dessiné sur un Canvas (des milliers de notes restent
// fluides). Clic dans le vide : une note ; glisser une note : la déplacer ; glisser son bord
// droit : la redimensionner ; clic droit (glissé) : effacer ; Ctrl + glisser : sélection au
// lasso ; Suppr, Ctrl+A, Échap, flèches. En bas, le couloir de vélocité.

import { useCallback, useEffect, useRef, useState } from 'react'
import { audio, useStudio } from '../../etat/studio'
import { estNoire } from '../../musique/clavier'
import { nomNote, nomSonBatterie } from '../../musique/gm'
import { TICS_PAR_NOIRE, TICS_PAR_PAS, ticsParMesure } from '../../musique/temps'
import {
  aimanter, aimanterBas, ajouterNote, definirVelocite, deplacerNotes, etendue, GRILLES, noteSous, notesDansRectangle,
  quantifier, redimensionnerNotes, supprimerNotes,
} from '../../projet/edition'
import { type Canal, type NoteMusicale, patternActif } from '../../projet/modele'

const H_LIGNE = 12 // px par demi-ton
const L_CLAVIER = 54
const H_VELOCITE = 56
const BORD = 6 // px du bord droit d'une note qui servent à la redimensionner
const ZOOM_MIN = 0.1
const ZOOM_MAX = 4

type Geste =
  | { type: 'deplacer'; tic: number; hauteur: number; depart: NoteMusicale[]; indices: Set<number> }
  | { type: 'redimensionner'; tic: number; depart: NoteMusicale[]; indices: Set<number> }
  | { type: 'lasso'; tic: number; hauteur: number; ticFin: number; hauteurFin: number; ajout: Set<number> }
  | { type: 'effacer' }
  | { type: 'velocite' }
  | { type: 'ecoute'; hauteur: number }

interface Vue {
  zoom: number // px par tic
  x: number // défilement horizontal, px
  y: number // défilement vertical, px depuis la note 127
  largeur: number
  hauteur: number
}

export function PianoRoll() {
  const { projet, canalSelectionne } = useStudio()
  const canal = projet.canaux.find((c) => c.id === canalSelectionne)
  if (!canal) return <p className="p-3 text-discret">Sélectionnez un instrument dans le Channel Rack.</p>
  return <Editeur key={canal.id} canal={canal} />
}

function Editeur({ canal }: { canal: Canal }) {
  const { remplacerNotes, jouerNote, relacherNote } = useStudio()
  const toile = useRef<HTMLCanvasElement>(null)
  const cadre = useRef<HTMLDivElement>(null)
  const vue = useRef<Vue>({ zoom: 0.6, x: 0, y: (127 - 72) * H_LIGNE, largeur: 0, hauteur: 0 })
  const geste = useRef<Geste | null>(null)
  const [selection, setSelection] = useState<Set<number>>(new Set())
  const selectionRef = useRef(selection)
  const [grille, setGrille] = useState(TICS_PAR_PAS)
  const dureeNouvelle = useRef(TICS_PAR_PAS * 2)

  const notes = (): NoteMusicale[] => patternActif(useStudio.getState().projet).notes[canal.id] ?? []
  const choisir = useCallback((s: Set<number>) => {
    selectionRef.current = s
    setSelection(s)
  }, [])
  const ecrire = (n: NoteMusicale[]) => remplacerNotes(canal.id, n)

  /** Fait entendre une note brièvement. */
  const ecouter = useCallback((hauteur: number, velocite = 100) => {
    jouerNote(hauteur, velocite, canal.id)
    setTimeout(() => relacherNote(hauteur, canal.id), 220)
  }, [canal.id, jouerNote, relacherNote])

  // ---------------------------------------------------------------- géométrie
  const hBas = () => vue.current.hauteur - H_VELOCITE
  const versTic = (x: number) => (x - L_CLAVIER + vue.current.x) / vue.current.zoom
  const versHauteur = (y: number) => 127 - Math.floor((y + vue.current.y) / H_LIGNE)
  const xDe = (tic: number) => L_CLAVIER + tic * vue.current.zoom - vue.current.x
  const yDe = (hauteur: number) => (127 - hauteur) * H_LIGNE - vue.current.y
  const borner = () => {
    const v = vue.current
    v.y = Math.min(Math.max(0, v.y), Math.max(0, 128 * H_LIGNE - hBas()))
    v.x = Math.max(0, v.x)
  }

  // ------------------------------------------------------------------ dessin
  const dessiner = useCallback(() => {
    const c = toile.current
    if (!c) return
    const ctx = c.getContext('2d')
    if (!ctx) return
    const v = vue.current
    const { projet } = useStudio.getState()
    const pattern = patternActif(projet)
    const liste = pattern.notes[canal.id] ?? []
    const choix = selectionRef.current
    const dpr = window.devicePixelRatio || 1
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    const bas = hBas()

    // fond : lignes de notes (touches noires plus sombres)
    ctx.fillStyle = '#20252b'
    ctx.fillRect(0, 0, v.largeur, v.hauteur)
    const hMax = Math.min(127, versHauteur(0))
    const hMin = Math.max(0, versHauteur(bas))
    for (let h = hMin; h <= hMax; h++) {
      const y = yDe(h)
      ctx.fillStyle = estNoire(h) ? '#1a1e23' : '#262c33'
      ctx.fillRect(L_CLAVIER, y, v.largeur, H_LIGNE)
      ctx.fillStyle = h % 12 === 0 ? '#3a424c' : '#1d2126'
      ctx.fillRect(L_CLAVIER, y + H_LIGNE - 1, v.largeur, 1)
    }
    // grille verticale : pas, temps, mesures ; zone après la fin du pattern assombrie
    const mesure = ticsParMesure(projet.signature)
    const debutTic = Math.max(0, Math.floor(versTic(L_CLAVIER) / TICS_PAR_PAS) * TICS_PAR_PAS)
    const finTic = versTic(v.largeur)
    for (let t = debutTic; t <= finTic; t += TICS_PAR_PAS) {
      const x = Math.round(xDe(t))
      ctx.fillStyle = t % mesure === 0 ? '#56606c' : t % TICS_PAR_NOIRE === 0 ? '#353d46' : '#2a3038'
      ctx.fillRect(x, 0, 1, bas)
    }
    const xFin = xDe(pattern.longueur)
    if (xFin < v.largeur) {
      ctx.fillStyle = 'rgba(0,0,0,0.35)'
      ctx.fillRect(Math.max(L_CLAVIER, xFin), 0, v.largeur, bas)
    }

    // lasso
    const g = geste.current
    if (g?.type === 'lasso') {
      const x1 = xDe(Math.min(g.tic, g.ticFin))
      const x2 = xDe(Math.max(g.tic, g.ticFin))
      const y1 = yDe(Math.max(g.hauteur, g.hauteurFin))
      const y2 = yDe(Math.min(g.hauteur, g.hauteurFin)) + H_LIGNE
      ctx.fillStyle = 'rgba(245,165,36,0.12)'
      ctx.fillRect(x1, y1, x2 - x1, y2 - y1)
      ctx.strokeStyle = '#f5a524'
      ctx.strokeRect(x1 + 0.5, y1 + 0.5, x2 - x1, y2 - y1)
    }

    // notes
    liste.forEach((n, i) => {
      const x = xDe(n.tic)
      const l = Math.max(2, n.duree * v.zoom)
      if (x + l < L_CLAVIER || x > v.largeur) return
      const y = yDe(n.hauteur)
      if (y + H_LIGNE < 0 || y > bas) return
      ctx.globalAlpha = 0.55 + (n.velocite / 127) * 0.45
      ctx.fillStyle = canal.couleur
      ctx.fillRect(x + 0.5, y + 1, l - 1, H_LIGNE - 2)
      ctx.globalAlpha = 1
      ctx.strokeStyle = choix.has(i) ? '#ffffff' : 'rgba(0,0,0,0.6)'
      ctx.lineWidth = choix.has(i) ? 2 : 1
      ctx.strokeRect(x + 0.5, y + 1.5, l - 1, H_LIGNE - 3)
      ctx.lineWidth = 1
    })

    // tête de lecture
    const t = audio.transport
    if (t?.etat === 'lecture') {
      const x = xDe(t.position)
      ctx.fillStyle = '#4cc38a'
      ctx.fillRect(Math.round(x), 0, 2, bas)
    }

    // clavier vertical
    for (let h = hMin; h <= hMax; h++) {
      const y = yDe(h)
      const noire = estNoire(h)
      ctx.fillStyle = noire ? '#15171b' : '#e9e5dc'
      ctx.fillRect(0, y, noire ? L_CLAVIER * 0.62 : L_CLAVIER, H_LIGNE)
      ctx.fillStyle = '#8b8577'
      ctx.fillRect(0, y + H_LIGNE - 1, L_CLAVIER, 1)
      if (h % 12 === 0) {
        ctx.fillStyle = '#333'
        ctx.font = '9px system-ui'
        ctx.fillText(nomNote(h), L_CLAVIER - 26, y + H_LIGNE - 3)
      }
    }
    ctx.fillStyle = '#11151a'
    ctx.fillRect(L_CLAVIER - 1, 0, 1, bas)

    // couloir de vélocité
    ctx.fillStyle = '#171b20'
    ctx.fillRect(0, bas, v.largeur, H_VELOCITE)
    ctx.fillStyle = '#11151a'
    ctx.fillRect(0, bas, v.largeur, 1)
    ctx.fillStyle = '#8b949e'
    ctx.font = '10px system-ui'
    ctx.fillText('Vélocité', 6, bas + 14)
    liste.forEach((n, i) => {
      const x = xDe(n.tic)
      if (x < L_CLAVIER || x > v.largeur) return
      const h = (n.velocite / 127) * (H_VELOCITE - 8)
      ctx.fillStyle = choix.has(i) ? '#ffffff' : canal.couleur
      ctx.fillRect(Math.round(x), bas + H_VELOCITE - h - 2, 3, h)
    })
  }, [canal.couleur, canal.id]) // eslint-disable-line react-hooks/exhaustive-deps

  // redessin permanent (tête de lecture, notes modifiées ailleurs) et taille du Canvas
  useEffect(() => {
    let id = 0
    const boucle = () => {
      dessiner()
      id = requestAnimationFrame(boucle)
    }
    id = requestAnimationFrame(boucle)
    return () => cancelAnimationFrame(id)
  }, [dessiner])

  useEffect(() => {
    const el = cadre.current
    const c = toile.current
    if (!el || !c) return
    const observer = new ResizeObserver(() => {
      const dpr = window.devicePixelRatio || 1
      vue.current.largeur = el.clientWidth
      vue.current.hauteur = el.clientHeight
      c.width = Math.round(el.clientWidth * dpr)
      c.height = Math.round(el.clientHeight * dpr)
      c.style.width = `${el.clientWidth}px`
      c.style.height = `${el.clientHeight}px`
      borner()
    })
    observer.observe(el)
    // vue de départ : centrée sur les notes du canal, ou autour du do central
    const e = etendue(notes())
    const centre = e ? (e.basse + e.haute) / 2 : canal.batterie ? 42 : 66
    vue.current.y = (127 - centre) * H_LIGNE - Math.max(0, el.clientHeight - H_VELOCITE) / 2
    borner()
    return () => observer.disconnect()
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // molette : défilement vertical, Maj : horizontal, Ctrl : zoom (écouteur non passif)
  useEffect(() => {
    const c = toile.current
    if (!c) return
    const surMolette = (e: WheelEvent) => {
      e.preventDefault()
      const v = vue.current
      if (e.ctrlKey) {
        const ticSouris = versTic(e.offsetX)
        v.zoom = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, v.zoom * (e.deltaY < 0 ? 1.15 : 1 / 1.15)))
        v.x = ticSouris * v.zoom - (e.offsetX - L_CLAVIER)
      } else if (e.shiftKey) {
        v.x += e.deltaY || e.deltaX
      } else {
        v.y += e.deltaY
        v.x += e.deltaX
      }
      borner()
    }
    c.addEventListener('wheel', surMolette, { passive: false })
    return () => c.removeEventListener('wheel', surMolette)
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // accès pour les tests navigateur : position écran d'une note
  useEffect(() => {
    Object.assign(window, {
      __pianoRoll: {
        versEcran: (tic: number, hauteur: number) => {
          const r = toile.current!.getBoundingClientRect()
          return { x: r.left + xDe(tic), y: r.top + yDe(hauteur) + H_LIGNE / 2 }
        },
        velocite: (tic: number, velocite: number) => {
          const r = toile.current!.getBoundingClientRect()
          return { x: r.left + xDe(tic) + 1, y: r.top + hBas() + H_VELOCITE - 2 - (velocite / 127) * (H_VELOCITE - 8) }
        },
        vue: () => ({ ...vue.current }),
      },
    })
  })

  // ---------------------------------------------------------------- gestes
  const position = (e: React.PointerEvent) => {
    const r = toile.current!.getBoundingClientRect()
    return { x: e.clientX - r.left, y: e.clientY - r.top }
  }

  const reglerVelocite = (x: number, y: number) => {
    const liste = notes()
    const tic = versTic(x)
    const rayon = 5 / vue.current.zoom
    const indices = liste.map((_, i) => i).filter((i) => Math.abs(liste[i].tic - tic) <= rayon)
    if (!indices.length) return
    const velocite = ((hBas() + H_VELOCITE - 2 - y) / (H_VELOCITE - 8)) * 127
    ecrire(definirVelocite(liste, new Set(indices), velocite))
  }

  const surAppui = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const { x, y } = position(e)
    e.currentTarget.focus()
    e.currentTarget.setPointerCapture(e.pointerId)
    if (y >= hBas()) {
      geste.current = { type: 'velocite' }
      reglerVelocite(x, y)
      return
    }
    const hauteur = versHauteur(y)
    if (x < L_CLAVIER) {
      if (hauteur < 0 || hauteur > 127) return
      geste.current = { type: 'ecoute', hauteur }
      jouerNote(hauteur, 100, canal.id)
      return
    }
    const tic = versTic(x)
    const liste = notes()
    const i = noteSous(liste, tic, hauteur)
    if (e.button === 2) {
      geste.current = { type: 'effacer' }
      if (i >= 0) {
        ecrire(supprimerNotes(liste, new Set([i])))
        choisir(new Set())
      }
      return
    }
    if (e.button !== 0) return
    if (e.ctrlKey || e.metaKey) {
      geste.current = { type: 'lasso', tic, hauteur, ticFin: tic, hauteurFin: hauteur, ajout: e.shiftKey ? new Set(selectionRef.current) : new Set() }
      return
    }
    if (i >= 0) {
      let choix = selectionRef.current
      if (!choix.has(i)) choix = e.shiftKey ? new Set([...choix, i]) : new Set([i])
      choisir(choix)
      const n = liste[i]
      const bordDroit = xDe(n.tic + n.duree)
      if (bordDroit - x <= BORD) geste.current = { type: 'redimensionner', tic, depart: liste, indices: choix }
      else geste.current = { type: 'deplacer', tic, hauteur, depart: liste, indices: choix }
      dureeNouvelle.current = n.duree
      ecouter(n.hauteur, n.velocite)
      return
    }
    // dans le vide : une nouvelle note, qu'on peut aussitôt déplacer
    if (hauteur < 0 || hauteur > 127) return
    const nouvelle: NoteMusicale = { tic: aimanterBas(tic, grille), duree: dureeNouvelle.current, hauteur, velocite: 100 }
    const suivantes = ajouterNote(liste, nouvelle)
    ecrire(suivantes)
    const choix = new Set([suivantes.length - 1])
    choisir(choix)
    geste.current = { type: 'deplacer', tic, hauteur, depart: suivantes, indices: choix }
    ecouter(hauteur)
  }

  const surDeplacement = (e: React.PointerEvent<HTMLCanvasElement>) => {
    const { x, y } = position(e)
    const g = geste.current
    const c = e.currentTarget
    if (!g) {
      // curseur : redimensionner près d'un bord droit, déplacer sur une note
      const liste = notes()
      const i = y < hBas() && x >= L_CLAVIER ? noteSous(liste, versTic(x), versHauteur(y)) : -1
      c.style.cursor = i < 0 ? (x < L_CLAVIER ? 'pointer' : 'crosshair') : xDe(liste[i].tic + liste[i].duree) - x <= BORD ? 'ew-resize' : 'move'
      return
    }
    const tic = versTic(x)
    const hauteur = versHauteur(y)
    if (g.type === 'deplacer') {
      const premiere = Math.min(...[...g.indices].map((i) => g.depart[i].tic))
      const dt = aimanter(premiere + (tic - g.tic), grille) - premiere
      const dh = hauteur - g.hauteur
      ecrire(deplacerNotes(g.depart, g.indices, dt, dh))
    } else if (g.type === 'redimensionner') {
      const dt = grille > 0 ? Math.round((tic - g.tic) / grille) * grille : Math.round(tic - g.tic)
      const resultat = redimensionnerNotes(g.depart, g.indices, dt, Math.max(1, grille / 2))
      ecrire(resultat)
      const premiere = [...g.indices][0]
      if (premiere !== undefined) dureeNouvelle.current = resultat[premiere].duree
    } else if (g.type === 'lasso') {
      g.ticFin = tic
      g.hauteurFin = hauteur
      choisir(new Set([...g.ajout, ...notesDansRectangle(notes(), g.tic, tic, g.hauteur, hauteur)]))
    } else if (g.type === 'effacer') {
      const liste = notes()
      const i = noteSous(liste, tic, hauteur)
      if (i >= 0) ecrire(supprimerNotes(liste, new Set([i])))
    } else if (g.type === 'velocite') {
      reglerVelocite(x, y)
    } else if (g.type === 'ecoute' && hauteur !== g.hauteur && hauteur >= 0 && hauteur <= 127) {
      relacherNote(g.hauteur, canal.id)
      jouerNote(hauteur, 100, canal.id)
      g.hauteur = hauteur
    }
  }

  const surRelache = () => {
    const g = geste.current
    if (g?.type === 'ecoute') relacherNote(g.hauteur, canal.id)
    geste.current = null
  }

  const surTouche = (e: React.KeyboardEvent<HTMLCanvasElement>) => {
    const liste = notes()
    const choix = selectionRef.current
    const arreter = () => {
      e.preventDefault()
      e.stopPropagation() // le studio ne doit pas aussi changer d'octave ou jouer une note
    }
    if ((e.key === 'Delete' || e.key === 'Backspace') && choix.size) {
      arreter()
      ecrire(supprimerNotes(liste, choix))
      choisir(new Set())
    } else if ((e.ctrlKey || e.metaKey) && e.code === 'KeyA') {
      arreter()
      choisir(new Set(liste.map((_, i) => i)))
    } else if (e.key === 'Escape' && choix.size) {
      arreter()
      choisir(new Set())
    } else if (choix.size && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
      arreter()
      ecrire(deplacerNotes(liste, choix, 0, (e.key === 'ArrowUp' ? 1 : -1) * (e.shiftKey ? 12 : 1)))
    } else if (choix.size && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      arreter()
      ecrire(deplacerNotes(liste, choix, (e.key === 'ArrowRight' ? 1 : -1) * (grille || 1), 0))
    }
  }

  const nbNotes = notes().length
  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b border-black/50 px-2 py-1 text-discret">
        <span>
          <span className="font-semibold" style={{ color: canal.couleur }}>{canal.nom}</span>
          {canal.batterie && <> · {nomSonBatterie(canal.noteDefaut)}</>} · <span data-testid="nb-notes">{nbNotes}</span> notes
          {selection.size > 0 && <> · <span data-testid="nb-selection">{selection.size}</span> sélectionnée{selection.size > 1 ? 's' : ''}</>}
        </span>
        <label className="flex items-center gap-1">
          Grille
          <select
            data-testid="grille"
            value={grille}
            onChange={(e) => setGrille(Number(e.target.value))}
            className="h-6 rounded border border-black/50 bg-[#1f242a] text-texte"
          >
            {GRILLES.map((g) => (
              <option key={g.nom} value={g.tics}>{g.nom}</option>
            ))}
          </select>
        </label>
        <button
          type="button"
          data-testid="quantifier"
          onClick={() => ecrire(quantifier(notes(), selectionRef.current, grille))}
          title="Ramener le début des notes (sélectionnées, sinon toutes) sur la grille"
          className="h-6 rounded border border-bord bg-panneau-2 px-2 text-texte hover:border-discret"
        >
          Quantifier
        </button>
        <span className="ml-auto text-[11px]">Molette : défiler · Maj : horizontal · Ctrl : zoom · Ctrl+glisser : lasso</span>
      </div>
      <div ref={cadre} className="relative min-h-0 flex-1">
        <canvas
          ref={toile}
          data-testid="piano-roll"
          tabIndex={0}
          aria-label={`Piano roll de ${canal.nom}`}
          className="absolute inset-0 touch-none outline-none"
          onPointerDown={surAppui}
          onPointerMove={surDeplacement}
          onPointerUp={surRelache}
          onPointerCancel={surRelache}
          onKeyDown={surTouche}
          onContextMenu={(e) => e.preventDefault()}
        />
      </div>
    </div>
  )
}
