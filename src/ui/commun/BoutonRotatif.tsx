// Bouton rotatif du studio : enveloppe le Knob d'audio/ui (bibliothèque encore avant la 1.0),
// pour qu'une évolution de son API ne touche que ce fichier.

import { Knob } from '@audio-ui/react'

interface Props {
  etiquette: string
  valeur: number
  min?: number
  max?: number
  /** Valeur rétablie par un double clic. */
  defaut?: number
  /** Point de départ de l'arc (0 pour un panoramique). */
  ancre?: number
  pas?: number
  taille?: 'petit' | 'moyen'
  /** Texte lu par les lecteurs d'écran et affiché au survol. */
  texte?: (valeur: number) => string
  surChangement: (valeur: number) => void
}

export function BoutonRotatif({
  etiquette, valeur, min = 0, max = 1, defaut, ancre, pas = 0.01, taille = 'petit', texte, surChangement,
}: Props) {
  const dimension = taille === 'petit' ? 'size-6' : 'size-9'
  const description = texte ? texte(valeur) : String(Math.round(valeur * 100) / 100)
  return (
    <Knob.Root
      value={valeur}
      min={min}
      max={max}
      step={pas}
      defaultValue={defaut ?? valeur}
      anchor={ancre}
      dragMode="vertical"
      onValueChange={(v) => surChangement(v)}
      className="inline-flex"
      title={`${etiquette} : ${description}`}
    >
      <Knob.Control className={`${dimension} relative cursor-ns-resize`}>
        <Knob.Track className="absolute inset-0 stroke-[#151a20]" strokeWidth={6} />
        <Knob.Indicator className="absolute inset-0 stroke-accent" strokeWidth={6} />
        <Knob.Thumb
          aria-label={etiquette}
          getAriaValueText={() => description}
          className="absolute inset-[18%] rounded-full bg-[#3a424d] shadow-[inset_0_1px_0_#4c5662] focus-within:outline-2 focus-within:outline-accent"
        >
          <span aria-hidden className="absolute left-1/2 top-[8%] h-[38%] w-0.5 -translate-x-1/2 rounded bg-texte" />
        </Knob.Thumb>
      </Knob.Control>
    </Knob.Root>
  )
}
