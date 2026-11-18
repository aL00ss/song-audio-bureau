// Temps musical : le projet compte en tics (96 par noire), le transport convertit en secondes.

export const TICS_PAR_NOIRE = 96
/** Un pas du séquenceur : une double croche. */
export const TICS_PAR_PAS = TICS_PAR_NOIRE / 4

export interface Signature {
  numerateur: number
  denominateur: number
}

export const SIGNATURE_DEFAUT: Signature = { numerateur: 4, denominateur: 4 }

export function ticsEnSecondes(tics: number, tempo: number): number {
  return (tics / TICS_PAR_NOIRE) * (60 / tempo)
}

export function secondesEnTics(secondes: number, tempo: number): number {
  return (secondes * tempo * TICS_PAR_NOIRE) / 60
}

export function ticsParTemps(signature: Signature = SIGNATURE_DEFAUT): number {
  return (TICS_PAR_NOIRE * 4) / signature.denominateur
}

export function ticsParMesure(signature: Signature = SIGNATURE_DEFAUT): number {
  return ticsParTemps(signature) * signature.numerateur
}

/** Arrondit une durée en tics à la mesure entière supérieure (au moins une mesure). */
export function arrondirALaMesure(tics: number, signature: Signature = SIGNATURE_DEFAUT): number {
  const mesure = ticsParMesure(signature)
  return Math.max(1, Math.ceil(tics / mesure)) * mesure
}

/** Position lisible « mesure:temps:tic », comptée à partir de 1:1:00. */
export function formaterPosition(tics: number, signature: Signature = SIGNATURE_DEFAUT): string {
  const t = Math.max(0, Math.floor(tics))
  const parTemps = ticsParTemps(signature)
  const mesure = Math.floor(t / ticsParMesure(signature)) + 1
  const temps = Math.floor((t % ticsParMesure(signature)) / parTemps) + 1
  const reste = t % parTemps
  return `${mesure}:${temps}:${String(reste).padStart(2, '0')}`
}

/** Durée lisible « m:ss.d » : 72.34 -> « 1:12.3 ». */
export function formaterSecondes(secondes: number): string {
  const dixiemes = Math.max(0, Math.floor(secondes * 10))
  const minutes = Math.floor(dixiemes / 600)
  const s = Math.floor((dixiemes % 600) / 10)
  return `${minutes}:${String(s).padStart(2, '0')}.${dixiemes % 10}`
}
