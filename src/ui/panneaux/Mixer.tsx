// Mixer : le maître puis une tranche par piste (égaliseur 3 bandes, envoi de réverbération,
// panoramique, muet, solo, fader, vumètre stéréo). Les canaux du rack y sont routés (le numéro
// de piste se choisit dans le Channel Rack).

import { useRef } from 'react'
import { gainDuFader, hauteurVumetre, pistesAudibles, texteDb } from '../../audio/niveaux'
import { audio, useStudio } from '../../etat/studio'
import type { PisteMixage } from '../../projet/modele'
import { BoutonRotatif } from '../commun/BoutonRotatif'
import { Fader } from '../commun/Fader'
import { useImage } from '../commun/useImage'

const DESCENTE = 0.85 // retombée du vumètre à chaque image (≈ 20 dB par seconde)

/** Vumètre stéréo d'une piste : crête lue à chaque image, retombée progressive. */
function Vumetre({ index }: { index: number }) {
  const memoire = useRef({ gauche: 0, droite: 0 })
  const cle = useImage(() => {
    const crete = audio.moteur?.niveauPiste(index) ?? { gauche: 0, droite: 0 }
    const m = memoire.current
    m.gauche = Math.max(crete.gauche, m.gauche * DESCENTE)
    m.droite = Math.max(crete.droite, m.droite * DESCENTE)
    return `${hauteurVumetre(m.gauche).toFixed(3)}:${hauteurVumetre(m.droite).toFixed(3)}`
  })
  const [g, d] = cle.split(':').map(Number)
  const barre = (h: number, cote: string) => (
    <div className="relative h-28 w-2 overflow-hidden rounded-sm bg-black/60" data-cote={cote} data-niveau={h}>
      <div
        className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-[#3ec46d] via-[#e5d84a] via-85% to-[#f05a4f]"
        style={{ height: '100%', clipPath: `inset(${(1 - h) * 100}% 0 0 0)` }}
      />
    </div>
  )
  return (
    <div className="flex gap-0.5" data-testid={`vumetre-${index}`} aria-hidden>
      {barre(g, 'gauche')}
      {barre(d, 'droite')}
    </div>
  )
}

function Tranche({ piste, canaux, audible }: { piste: PisteMixage; canaux: string[]; audible: boolean }) {
  const modifierPiste = useStudio((s) => s.modifierPiste)
  const maitre = piste.index === 0
  const regler = (m: Partial<PisteMixage>) => modifierPiste(piste.index, m)
  const db = (v: number) => `${v > 0 ? '+' : ''}${v.toFixed(1)} dB`
  return (
    <section
      data-testid={`tranche-${piste.index}`}
      aria-label={`Piste ${maitre ? 'maître' : piste.index} ${piste.nom}`}
      className={`flex w-[7.25rem] shrink-0 flex-col items-center gap-1 rounded border border-black/60 p-1 [&>*]:shrink-0 ${
        maitre ? 'bg-[#2e2a22]' : 'bg-[#272c33]'
      } ${audible || maitre ? '' : 'opacity-60'}`}
    >
      <span className="w-full truncate text-center font-mono text-[10px] text-discret">{maitre ? 'M' : piste.index}</span>
      <span className="w-full truncate text-center font-semibold" title={canaux.join(', ') || 'Aucun canal'}>{piste.nom}</span>
      {!maitre && (
        <div className="grid grid-cols-4 gap-x-0.5 text-[8px] text-discret">
          {(['grave', 'medium', 'aigu'] as const).map((bande) => (
            <label key={bande} className="flex flex-col items-center">
              <BoutonRotatif
                etiquette={`Égaliseur ${bande} ${piste.nom}`}
                valeur={piste.eq[bande]}
                min={-12}
                max={12}
                ancre={0}
                defaut={0}
                pas={0.5}
                texte={db}
                surChangement={(v) => regler({ eq: { ...piste.eq, [bande]: v } })}
              />
              {bande === 'grave' ? 'Grave' : bande === 'medium' ? 'Méd.' : 'Aigu'}
            </label>
          ))}
          <label className="flex flex-col items-center">
            <BoutonRotatif
              etiquette={`Réverbération ${piste.nom}`}
              valeur={piste.reverb}
              defaut={0.18}
              texte={(v) => `${Math.round(v * 100)} %`}
              surChangement={(v) => regler({ reverb: v })}
            />
            Rév.
          </label>
        </div>
      )}
      {maitre && <span className="h-[34px]" aria-hidden />}
      {!maitre && (
        <BoutonRotatif
          etiquette={`Panoramique piste ${piste.nom}`}
          valeur={piste.pan}
          min={-1}
          max={1}
          ancre={0}
          defaut={0}
          texte={(v) => (Math.abs(v) < 0.01 ? 'centre' : `${Math.round(Math.abs(v) * 100)} % ${v < 0 ? 'gauche' : 'droite'}`)}
          surChangement={(v) => regler({ pan: v })}
        />
      )}
      <div className="flex gap-1">
        <button
          type="button"
          aria-pressed={!piste.muet}
          aria-label={`Activer ${piste.nom}`}
          title="Piste active (cliquer pour couper)"
          onClick={() => regler({ muet: !piste.muet })}
          className="size-4 rounded-full border border-black/60 bg-[#3a414b] aria-pressed:bg-actif"
        />
        {!maitre && (
          <button
            type="button"
            aria-pressed={piste.solo}
            aria-label={`Solo ${piste.nom}`}
            title="Solo"
            onClick={() => regler({ solo: !piste.solo })}
            className="h-4 rounded border border-black/60 bg-[#3a414b] px-1 text-[9px] font-bold aria-pressed:bg-accent aria-pressed:text-black"
          >
            S
          </button>
        )}
      </div>
      <div className="flex items-end gap-1">
        <Fader
          etiquette={`Volume piste ${piste.nom}`}
          valeur={piste.volume}
          texte={(v) => texteDb(gainDuFader(v))}
          surChangement={(v) => regler({ volume: v })}
        />
        <Vumetre index={piste.index} />
      </div>
      <span className="font-mono text-[10px] text-discret">{texteDb(gainDuFader(piste.volume))}</span>
    </section>
  )
}

export function Mixer() {
  const { projet } = useStudio()
  const audibles = pistesAudibles(projet.mixage)
  return (
    <div className="flex h-full gap-1 overflow-auto bg-[#1f242a] p-1.5">
      {projet.mixage.map((piste) => (
        <Tranche
          key={piste.index}
          piste={piste}
          audible={audibles.has(piste.index)}
          canaux={projet.canaux.filter((c) => c.pisteMixage === piste.index).map((c) => c.nom)}
        />
      ))}
    </div>
  )
}
