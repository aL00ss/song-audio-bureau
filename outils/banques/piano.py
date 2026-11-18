"""Piano léger pour le site : Salamander Grand Piano V3 (Alexander Holm, CC BY 3.0) réduit à
quelques couches de vélocité, queues raccourcies et fondues, compressé en SF3.

Usage : python piano.py <dossier contenant SalamanderGrandPianoV3.sfz> <sortie.sf3>
(à lancer dans un environnement avec numpy et soundfile)
"""

from __future__ import annotations

import os
import sys
from collections import defaultdict

import numpy as np
import soundfile as sf

import sf3
import sfz

COUCHES = 5
QUALITE = 0.35
FONDU = 0.6  # secondes de fondu à la fin de chaque échantillon raccourci
ATTENUATION_CB = 190  # centibels (SpessaSynth, comme les EMU, en applique 40 % : ≈ 7,6 dB)


def duree_gardee(note: int) -> float:
    """Les graves résonnent longtemps, les aigus s'éteignent vite : on garde de 14 s à 4 s."""
    return float(np.clip(14 - (note - 21) * 0.16, 4, 14))


def choisir_couches(regions: list[dict], nombre: int) -> list[tuple[dict, int, int]]:
    """Garde `nombre` couches réparties (la plus forte incluse) et recalcule leurs vélocités."""
    regions = sorted(regions, key=lambda r: int(r.get('lovel', 1)))
    n = len(regions)
    indices = sorted({round(n - 1 - i * (n - 1) / (nombre - 1)) for i in range(nombre)}) if n > nombre else list(range(n))
    resultat = []
    bas = 1
    for k, i in enumerate(indices):
        haut = 127 if k == len(indices) - 1 else int(regions[i].get('hivel', 127))
        resultat.append((regions[i], bas, haut))
        bas = haut + 1
    return resultat


def preparer(fichier: str, note: int) -> tuple[np.ndarray, int]:
    donnees, frequence = sf.read(fichier, dtype='float32', always_2d=True)
    longueur = min(len(donnees), int(duree_gardee(note) * frequence))
    donnees = donnees[:longueur].copy()
    fondu = min(int(FONDU * frequence), longueur // 2)
    donnees[-fondu:] *= np.linspace(1, 0, fondu, dtype=np.float32)[:, None] ** 2
    return donnees, frequence


def main(dossier: str, sortie: str) -> None:
    regions = [
        r for r in sfz.lire(os.path.join(dossier, 'SalamanderGrandPianoV3.sfz'))
        if 'trigger' not in r and int(r.get('lokey', -1)) >= 0
    ]
    par_touches = defaultdict(list)
    for r in regions:
        par_touches[(int(r['lokey']), int(r['hikey']), int(r['pitch_keycenter']))].append(r)

    echantillons: list[sf3.Echantillon] = []
    zones: list[sf3.Zone] = []
    for (bas, haut, centre), groupe in sorted(par_touches.items()):
        for region, vel_bas, vel_haut in choisir_couches(groupe, COUCHES):
            donnees, frequence = preparer(region['_fichier'], centre)
            nom = os.path.splitext(os.path.basename(region['_fichier']))[0]
            relachement = float(region.get('ampeg_release', 1))
            g, d = len(echantillons), len(echantillons) + 1
            echantillons.append(sf3.Echantillon(f'{nom}G', donnees[:, 0], frequence, centre, type=sf3.GAUCHE, lien=d))
            echantillons.append(sf3.Echantillon(f'{nom}D', donnees[:, -1], frequence, centre, type=sf3.DROITE, lien=g))
            for indice, pan in ((g, -500), (d, 500)):
                zones.append(sf3.Zone([
                    (sf3.TOUCHES, sf3.plage(bas, haut)),
                    (sf3.VELOCITES, sf3.plage(vel_bas, vel_haut)),
                    (sf3.PAN, pan),
                    (sf3.RELACHEMENT_VOL, sf3.temps_en_timecents(relachement)),
                    (sf3.NOTE_DE_BASE, centre),
                ], indice))
        print(f'touches {bas}-{haut} : {len(choisir_couches(groupe, COUCHES))} couches', flush=True)

    # Les couches portent déjà les nuances : la courbe de vélocité par défaut (jusqu'à -96 dB)
    # est remplacée par une pente douce, comme amp_veltrack=73 dans le SFZ d'origine (≈ -11 dB).
    modulateurs = [
        (0x0502, sf3.ATTENUATION, 0, 0, 0),  # annule le modulateur par défaut (vélocité concave)
        (0x0102, sf3.ATTENUATION, 120, 0, 0),  # vélocité linéaire décroissante -> jusqu'à 12 dB
    ]
    # niveau aligné sur le piano de GeneralUser GS (mesuré dans le studio : ≈ 7,5 dB plus fort)
    instrument = sf3.Instrument('Salamander', zones, modulateurs, globaux=[(sf3.ATTENUATION, ATTENUATION_CB)])
    sf3.ecrire_sf3(
        sortie,
        'Piano à queue (Salamander, allégé)',
        echantillons,
        [instrument],
        [sf3.Preset('Piano à queue', 0, 0, 0)],
        copyright='Salamander Grand Piano V3 par Alexander Holm, CC BY 3.0',
        commentaire='Allégé pour song-audio : 5 couches de vélocité, queues raccourcies, Ogg Vorbis.',
        qualite=QUALITE,
    )
    print(f'{sortie} : {os.path.getsize(sortie) / 1e6:.1f} Mo, {len(echantillons)} échantillons')


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2])
