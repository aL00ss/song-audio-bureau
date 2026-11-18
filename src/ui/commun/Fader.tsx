// Fader vertical du mixer : un curseur natif (accessible au clavier et aux lecteurs d'écran)
// orienté verticalement, habillé comme une glissière de console.

interface Props {
  etiquette: string
  valeur: number
  defaut?: number
  texte: (valeur: number) => string
  surChangement: (valeur: number) => void
}

export function Fader({ etiquette, valeur, defaut = 0.8, texte, surChangement }: Props) {
  return (
    <input
      type="range"
      aria-label={etiquette}
      aria-valuetext={texte(valeur)}
      title={`${etiquette} : ${texte(valeur)} (double clic : ${texte(defaut)})`}
      min={0}
      max={1}
      step={0.005}
      value={valeur}
      onChange={(e) => surChangement(Number(e.target.value))}
      onDoubleClick={() => surChangement(defaut)}
      className="fader h-28 w-6 cursor-ns-resize"
    />
  )
}
