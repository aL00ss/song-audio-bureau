// Barre du haut : transport (lecture, arrêt, mode), tempo, afficheurs, volume général,
// boutons des panneaux.

import { BANQUES } from '../../audio/banques'
import { audio, useStudio } from '../../etat/studio'
import { formaterPosition, formaterSecondes, ticsEnSecondes } from '../../musique/temps'
import { BoutonRotatif } from '../commun/BoutonRotatif'
import { useImage } from '../commun/useImage'
import { MenuFichier } from './MenuFichier'

const bouton = 'h-7 rounded border border-bord bg-panneau-2 px-2.5 hover:border-discret disabled:opacity-40'

function Afficheur({ testId, titre, children }: { testId: string; titre: string; children: React.ReactNode }) {
  return (
    <span
      data-testid={testId}
      title={titre}
      className="min-w-[5.5rem] rounded border border-black/60 bg-[#0d1013] px-2 py-1 text-center font-mono text-[13px] text-actif shadow-[inset_0_1px_3px_#000]"
    >
      {children}
    </span>
  )
}

export function BarreTransport() {
  const {
    transport, chargement, projet, enregistrement, mode, basculerEnregistrement, basculerLecture, arreter, changerTempo,
    changerVolumeMaitre, changerMode, choisirPattern, nouveauPattern, dupliquerPattern, exportEnCours, banques, metronomeDisponible, metronome, basculerMetronome,
  } = useStudio()
  const banqueEnCours = BANQUES.find((b) => banques[b.id]?.phase === 'chargement')
  const progressionBanque = banqueEnCours ? banques[banqueEnCours.id] : null
  const position = useImage(() => audio.transport?.position ?? 0)
  const pret = chargement.phase === 'pret'
  const statut =
    chargement.phase === 'son'
      ? `Chargement des sons… ${Math.round(chargement.progression * 100)} %`
      : chargement.phase === 'erreur'
        ? chargement.message
        : exportEnCours
          ? 'Export WAV en cours…'
          : 'Prêt'

  return (
    <header className="flex flex-wrap items-center gap-2 border-b border-black/50 bg-gradient-to-b from-[#2f353d] to-[#262b32] px-2 py-1.5">
      <span className="mr-1 text-[13px] font-bold tracking-wide">
        song<span className="text-accent">-audio</span>
      </span>

      <MenuFichier />

      <div className="flex items-center gap-1" role="group" aria-label="Transport">
        <button
          type="button"
          data-testid="lecture"
          disabled={!pret}
          onClick={() => void basculerLecture()}
          title="Lecture / pause (Espace)"
          className={`h-7 w-20 rounded font-semibold disabled:opacity-40 ${transport === 'lecture' ? 'bg-actif text-black' : 'bg-accent text-black'}`}
        >
          {transport === 'lecture' ? 'Pause' : 'Lecture'}
        </button>
        <button type="button" data-testid="stop" disabled={!pret} onClick={arreter} title="Arrêt (Maj+Espace)" className={bouton}>
          Stop
        </button>
        <button
          type="button"
          data-testid="enregistrer"
          aria-pressed={enregistrement}
          onClick={basculerEnregistrement}
          title="Enregistrement : les notes jouées pendant la lecture s'écrivent dans le pattern"
          className={`${bouton} aria-pressed:border-red-500 aria-pressed:bg-red-600/80 aria-pressed:text-white`}
        >
          <span aria-hidden className="mr-1 text-red-400 in-aria-pressed:text-white">●</span>Rec
        </button>
        {metronomeDisponible && (
          <button
            type="button"
            data-testid="metronome"
            aria-pressed={metronome}
            onClick={basculerMetronome}
            title="Métronome : un clic à chaque temps, plus aigu au premier temps de la mesure"
            className={`${bouton} aria-pressed:border-accent aria-pressed:bg-accent aria-pressed:text-black`}
          >
            Métro
          </button>
        )}
        <span className="ml-1 inline-flex overflow-hidden rounded border border-bord" role="group" aria-label="Mode de lecture">
          {(['pattern', 'morceau'] as const).map((m) => (
            <button
              key={m}
              type="button"
              data-testid={`mode-${m}`}
              aria-pressed={mode === m}
              onClick={() => changerMode(m)}
              title={m === 'pattern' ? 'Le pattern actif en boucle' : "L'arrangement de la playlist"}
              className="px-2 py-1 text-discret aria-pressed:bg-accent/90 aria-pressed:font-semibold aria-pressed:text-black"
            >
              {m === 'pattern' ? 'PAT' : 'SONG'}
            </button>
          ))}
        </span>
      </div>

      <span className="flex items-center gap-1" role="group" aria-label="Pattern actif">
        <select
          data-testid="choix-pattern"
          aria-label="Pattern actif"
          value={projet.patternActif}
          onChange={(e) => choisirPattern(e.target.value)}
          className="h-7 max-w-36 rounded border border-black/60 bg-[#0d1013] px-1 text-[12px] text-accent"
        >
          {projet.patterns.map((p) => (
            <option key={p.id} value={p.id}>{p.nom}</option>
          ))}
        </select>
        <button type="button" data-testid="nouveau-pattern" onClick={nouveauPattern} title="Nouveau pattern" className={bouton}>＋</button>
        <button type="button" data-testid="dupliquer-pattern" onClick={dupliquerPattern} title="Dupliquer le pattern actif" className={bouton}>⧉</button>
      </span>

      <label className="flex items-center gap-1 text-discret" title="Tempo, noires par minute">
        <input
          type="number"
          data-testid="tempo"
          aria-label="Tempo"
          min={20}
          max={300}
          step={1}
          value={projet.tempo}
          onChange={(e) => changerTempo(Number(e.target.value))}
          className="h-7 w-[4.5rem] rounded border border-black/60 bg-[#0d1013] px-1.5 text-right font-mono text-[13px] text-accent"
        />
        BPM
      </label>

      <Afficheur testId="position" titre="Position : mesure:temps:tic">{formaterPosition(position, projet.signature)}</Afficheur>
      <Afficheur testId="temps" titre="Temps écoulé dans le pattern">{formaterSecondes(ticsEnSecondes(position, projet.tempo))}</Afficheur>

      <span className="flex items-center gap-1 text-discret" title="Volume général">
        <BoutonRotatif
          etiquette="Volume général"
          taille="moyen"
          valeur={projet.mixage[0]?.volume ?? 0.8}
          defaut={0.8}
          texte={(v) => `${Math.round(v * 100)} %`}
          surChangement={changerVolumeMaitre}
        />
      </span>


      {banqueEnCours && progressionBanque?.phase === 'chargement' && (
        <span data-testid="chargement-banque" className="ml-auto text-discret" title="Son plus fidèle, téléchargé à la demande">
          {banqueEnCours.nom} : {Math.round(progressionBanque.progression * 100)} %
        </span>
      )}
      <span data-testid="statut" className={`${banqueEnCours ? '' : 'ml-auto '}text-discret`}>{statut}</span>
    </header>
  )
}
