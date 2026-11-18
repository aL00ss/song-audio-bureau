import { useEffect } from 'react'
import { audio, useStudio } from './etat/studio'
import { attribuerCanauxMidi } from './projet/modele'
import { Studio } from './ui/coquille/Studio'

export default function App() {
  const initialiser = useStudio((s) => s.initialiser)

  useEffect(() => {
    void initialiser()
  }, [initialiser])

  useEffect(() => {
    // accès pour les tests navigateur et la console
    Object.assign(window, {
      __studio: {
        etat: useStudio.getState,
        /** Le magasin lui-même (tests : régler un état sans passer par l'interface). */
        magasin: useStudio,
        audio,
        /** Canal MIDI qui joue un canal du projet. */
        canalMidi: (id: string) => attribuerCanauxMidi(useStudio.getState().projet.canaux).get(id),
        /** Voix qui sonnent sur chaque canal du projet, dans l'ordre des canaux. */
        voixParCanal: () => {
          const { projet } = useStudio.getState()
          const attribution = attribuerCanauxMidi(projet.canaux)
          return projet.canaux.map((c) => audio.moteur?.voix(attribution.get(c.id) ?? -1) ?? 0)
        },
      },
    })
  }, [])

  return <Studio />
}
