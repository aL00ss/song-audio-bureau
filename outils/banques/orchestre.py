"""Sections d'orchestre légères pour le site, tirées de VSCO 2 Community Edition
(Versilian Studios, CC0) : cordes, cuivres et bois, une banque SF3 par famille.

Le dépôt VSCO 2 CE ne fournit que des fichiers WAV dont le nom porte la note et la nuance
(« VlnEns_susVib_A3_v2.wav »). Pour chaque instrument, on garde la nuance la plus douce et la
plus forte, un seul « round robin », on mesure la vraie hauteur (les octaves ne sont pas
numérotées pareil d'un instrument à l'autre), puis on fabrique une boucle en fondu enchaîné
dans la partie tenue, pour que les notes longues tiennent.

Usage :
  python orchestre.py liste <arbre.json>          fichiers à télécharger (arbre de l'API GitHub)
  python orchestre.py construire <dossier VSCO> <dossier de sortie>
"""

from __future__ import annotations

import json
import os
import re
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass

import numpy as np
import soundfile as sf

import sf3
import sfz

QUALITE = 0.45
BOUCLE_DEBUT = 1.0  # s : la boucle commence après l'attaque
BOUCLE_DUREE = 1.6  # s
FONDU_BOUCLE = 0.25  # s de fondu enchaîné à la jonction
# La nuance douce enregistrée (pp) est bien plus faible que la forte (ff) : elle est remontée à
# ÉCART_COUCHES dB sous la forte, pour qu'aucun saut de volume ne se fasse entendre à SEUIL_FORTE.
ECART_COUCHES = 5.0
SEUIL_FORTE = 64

# Niveau : chaque note est ramenée à un même niveau efficace (couche forte à CIBLE_EFFICACE, sans
# dépasser sa crête), car les enregistrements varient de plus de 20 dB d'une note à l'autre.
# Puis chaque programme est atténué pour s'aligner sur GeneralUser GS : écarts mesurés dans le
# studio (rendu hors ligne d'une note tenue, comparé au même programme joué par GeneralUser GS).
CIBLE_EFFICACE = 0.2
ATTENUATIONS_DB = {41: 6.2, 44: 3.6, 45: 14.9, 46: 6.4, 48: 3.7, 56: 3.1, 57: 1.1, 59: 10.8, 73: 1.3}


@dataclass
class Source:
    dossier: str
    bas: int = 0  # touches couvertes par cette source (avant correction d'octave)
    haut: int = 127


@dataclass
class Programme:
    numero: int
    nom: str
    sources: list[Source]
    tenu: bool = True  # boucle (sinon le son s'éteint de lui-même : pizz, harpe)
    relachement: float = 0.35
    # réglages ajoutés au niveau du preset (secondes) : l'instrument d'un autre programme est réutilisé
    attaque: float = 0.0
    relachement_preset: float = 0.0


FAMILLES: dict[str, tuple[str, list[Programme]]] = {
    'cordes': ('Cordes', [
        Programme(40, 'Violon', [Source('Strings/Solo Violin/Arco Vib')]),
        Programme(41, 'Alto', [Source('Strings/Viola Section/susvib')]),
        Programme(42, 'Violoncelle', [Source('Strings/Cello Section/susvib')]),
        Programme(43, 'Contrebasse', [Source('Strings/Solo Contrabass/SusVib')]),
        Programme(44, 'Cordes en trémolo', [Source('Strings/Violin Section/Trem')]),
        Programme(45, 'Cordes en pizzicato', [Source('Strings/Cello Section/pizzT'), Source('Strings/Violin Section/Pizz')], tenu=False, relachement=0.25),
        Programme(46, 'Harpe', [Source('Strings/Harp')], tenu=False, relachement=0.8),
        Programme(48, 'Ensemble de cordes', [Source('Strings/Cello Section/susvib'), Source('Strings/Viola Section/susvib'), Source('Strings/Violin Section/susVib')]),
        Programme(49, 'Ensemble de cordes lent', [Source('Strings/Cello Section/susvib'), Source('Strings/Viola Section/susvib'), Source('Strings/Violin Section/susVib')], attaque=0.35, relachement_preset=0.3),
    ]),
    'cuivres': ('Cuivres', [
        Programme(56, 'Trompette', [Source('Brass/Trumpet/sus')], relachement=0.2),
        Programme(57, 'Trombone', [Source('Brass/Tenor Trombone/sus')], relachement=0.2),
        Programme(58, 'Tuba', [Source('Brass/Tuba/sus')], relachement=0.2),
        Programme(59, 'Trompette bouchée', [Source('Brass/Trumpet/straightM-sus')], relachement=0.2),
        Programme(60, 'Cor', [Source('Brass/F Horn/sus')], relachement=0.3),
    ]),
    'bois': ('Bois', [
        Programme(68, 'Hautbois', [Source('Woodwinds/Oboe/Vib')], relachement=0.2),
        Programme(70, 'Basson', [Source('Woodwinds/Bassoon/sus')], relachement=0.2),
        Programme(71, 'Clarinette', [Source('Woodwinds/Clarinet/susLong')], relachement=0.2),
        Programme(72, 'Piccolo', [Source('Woodwinds/Piccolo/Sus')], relachement=0.2),
        Programme(73, 'Flûte', [Source('Woodwinds/Flute/susvib')], relachement=0.2),
    ]),
}

NUANCES = {'pp': 1, 'p': 2, 'mp': 3, 'mf': 4, 'f': 5, 'ff': 6, 'fff': 7}
RE_NOTE = re.compile(r'(?:^|_)([A-G]#?)(-?\d)(?=_|\.|$)')
RE_VELOCITE = re.compile(r'_v(\d+)(?=_|\.|$)')
RE_NUANCE = re.compile(r'_(pp|p|mp|mf|f|ff|fff)(?=_|\.|$)')
RE_RR = re.compile(r'_(?:rr|RR)(\d+)|_(\d)(?=_|\.|$)|(?:staccato|sustain)(\d)')


def lire_nom(nom: str) -> tuple[int, int, int] | None:
    """(note écrite, nuance, numéro de prise) d'après le nom du fichier, ou None."""
    base = os.path.splitext(nom)[0]
    m = RE_NOTE.search(base)
    if not m:
        return None
    note = sfz.note(m.group(1) + m.group(2))
    v = RE_VELOCITE.search(base)
    n = RE_NUANCE.search(base)
    nuance = int(v.group(1)) if v else NUANCES[n.group(1)] if n else 1
    reste = base[m.end():]
    r = RE_RR.search(reste)
    prise = int(next(g for g in r.groups() if g)) if r else 1
    return note, nuance, prise


def choisir(fichiers: list[str]) -> dict[int, list[tuple[int, str]]]:
    """Par note écrite : la nuance la plus douce et la plus forte, première prise seulement."""
    par_note: dict[int, dict[int, tuple[int, str]]] = defaultdict(dict)
    for chemin in fichiers:
        lu = lire_nom(os.path.basename(chemin))
        if not lu:
            continue
        note, nuance, prise = lu
        deja = par_note[note].get(nuance)
        if not deja or prise < deja[0]:
            par_note[note][nuance] = (prise, chemin)
    resultat = {}
    for note, nuances in par_note.items():
        cles = sorted(nuances)
        gardees = sorted({cles[0], cles[-1]})
        resultat[note] = [(k, nuances[k][1]) for k in gardees]
    return resultat


def tous_les_programmes():
    for famille, (_, programmes) in FAMILLES.items():
        for p in programmes:
            yield famille, p


def lister(arbre: str) -> None:
    fichiers = [x['path'] for x in json.load(open(arbre))['tree'] if x['type'] == 'blob' and x['path'].lower().endswith('.wav')]
    voulus = set()
    for _, programme in tous_les_programmes():
        for source in programme.sources:
            dans = [f for f in fichiers if os.path.dirname(f) == source.dossier]
            for couches in choisir(dans).values():
                voulus.update(c for _, c in couches)
    for f in sorted(voulus):
        print(f)


# ---------------------------------------------------------------- hauteur et boucle

def frequence_fondamentale(signal: np.ndarray, frequence: int) -> float | None:
    """Autocorrélation sur une demi-seconde de la partie tenue (méthode simple, suffisante ici)."""
    debut = min(int(0.3 * frequence), len(signal) // 3)
    x = signal[debut : debut + int(0.5 * frequence)].astype(np.float64)
    if len(x) < 2048 or np.max(np.abs(x)) < 1e-4:
        return None
    x = x - x.mean()
    n = 1 << int(np.ceil(np.log2(2 * len(x))))
    spectre = np.fft.rfft(x, n)
    auto = np.fft.irfft(spectre * np.conj(spectre))[: len(x)]
    auto /= auto[0]
    mini, maxi = int(frequence / 2000), int(frequence / 25)
    zone = auto[mini:maxi]
    seuil = 0.8 * zone.max()
    pic = next(i for i in range(1, len(zone) - 1) if zone[i] >= seuil and zone[i] >= zone[i - 1] and zone[i] >= zone[i + 1])
    # interpolation parabolique autour du pic
    a, b, c = zone[pic - 1], zone[pic], zone[pic + 1]
    decalage = 0.5 * (a - c) / (a - 2 * b + c) if (a - 2 * b + c) else 0
    return frequence / (mini + pic + decalage)


def note_mesuree(signal: np.ndarray, frequence: int) -> float | None:
    f = frequence_fondamentale(signal, frequence)
    return None if f is None else 69 + 12 * np.log2(f / 440)


def correction_octave(echantillons: list[tuple[int, np.ndarray, int]]) -> int:
    """Décalage (multiple de 12) entre les notes écrites et les notes mesurées, le plus fréquent."""
    votes = Counter()
    for ecrite, signal, frequence in echantillons:
        mesuree = note_mesuree(signal, frequence)
        if mesuree is not None:
            votes[12 * round((mesuree - ecrite) / 12)] += 1
    return votes.most_common(1)[0][0] if votes else 0


def boucler(donnees: np.ndarray, frequence: int) -> tuple[np.ndarray, tuple[int, int]]:
    """Coupe l'échantillon à la fin d'une boucle dont la jonction est un fondu enchaîné."""
    longueur = len(donnees)
    debut = int(BOUCLE_DEBUT * frequence)
    fin = debut + int(BOUCLE_DUREE * frequence)
    if fin + 1 > longueur:  # échantillon court : boucle sur sa partie centrale
        debut, fin = int(longueur * 0.35), int(longueur * 0.85)
    fondu = min(int(FONDU_BOUCLE * frequence), (fin - debut) // 2, debut)
    sortie = donnees[:fin].copy()
    rampe = np.linspace(0, 1, fondu, dtype=np.float32)[:, None]
    # juste avant la fin, on glisse vers ce qui précède le début : le saut fin -> début est continu
    sortie[fin - fondu : fin] = sortie[fin - fondu : fin] * np.sqrt(1 - rampe) + donnees[debut - fondu : debut] * np.sqrt(rampe)
    return sortie, (debut, fin)


def raccourcir(donnees: np.ndarray, frequence: int, secondes: float) -> np.ndarray:
    longueur = min(len(donnees), int(secondes * frequence))
    sortie = donnees[:longueur].copy()
    fondu = min(int(0.4 * frequence), longueur // 2)
    sortie[-fondu:] *= np.linspace(1, 0, fondu, dtype=np.float32)[:, None] ** 2
    return sortie


def niveau_crete(signal: np.ndarray) -> float:
    return float(np.max(np.abs(signal))) or 1.0


def niveau_tenu(signal: np.ndarray, frequence: int) -> float:
    """Niveau efficace de la partie tenue (après l'attaque), ou de tout le son s'il est court."""
    partie = signal[int(0.3 * frequence) : int(1.3 * frequence)]
    if len(partie) < frequence // 4:
        partie = signal
    return float(np.sqrt(np.mean(partie**2))) or 1e-9


def normaliser_note(couches: list) -> list:
    """Même gain pour toutes les couches d'une note : la forte atteint CIBLE_EFFICACE (crête < 0,99)."""
    fort = couches[-1]
    gain = min(CIBLE_EFFICACE / niveau_tenu(fort[1], fort[2]), 0.99 / max(niveau_crete(c[1]) for c in couches))
    return [(n, d * gain, f, c) for n, d, f, c in couches]


def equilibrer_couches(couches: list) -> list:
    """Remonte la couche douce à ECART_COUCHES dB sous la forte (sans dépasser la crête)."""
    if len(couches) < 2:
        return couches
    (n0, doux, f0, c0), fort = couches[0], couches[-1]
    cible = niveau_tenu(fort[1], fort[2]) * 10 ** (-ECART_COUCHES / 20)
    gain = min(max(cible / niveau_tenu(doux, f0), 1.0), 0.99 / niveau_crete(doux))
    return [(n0, doux * gain, f0, c0), *couches[1:]]


# ---------------------------------------------------------------- construction

def construire_famille(racine: str, famille: str, sortie: str) -> None:
    nom_famille, programmes = FAMILLES[famille]
    echantillons: list[sf3.Echantillon] = []
    instruments: list[sf3.Instrument] = []
    presets: list[sf3.Preset] = []
    deja: dict[tuple[str, ...], int] = {}  # même suite de sources : même instrument réutilisé
    partages: dict[tuple[str, int, bool], tuple[int, int]] = {}  # même fichier : mêmes échantillons
    for programme in programmes:
        cle = tuple(s.dossier for s in programme.sources) + (str(programme.tenu),)
        reglages = []
        if programme.attaque:
            # ajouté aux timecents de l'instrument : 1200 * log2(attaque / 1 ms)
            reglages.append((sf3.ATTAQUE_VOL, sf3.temps_en_timecents(programme.attaque) - sf3.temps_en_timecents(0.001)))
        if programme.relachement_preset:
            reglages.append((sf3.RELACHEMENT_VOL, 1200))  # relâchement deux fois plus long
        if cle in deja:
            presets.append(sf3.Preset(programme.nom, 0, programme.numero, deja[cle], reglages))
            print(f'  {programme.numero:3d} {programme.nom} : sons de {instruments[deja[cle]].nom}', flush=True)
            continue
        zones = construire_zones(racine, programme, echantillons, partages)
        globaux = [(sf3.RELACHEMENT_VOL, sf3.temps_en_timecents(programme.relachement))]
        if programme.numero in ATTENUATIONS_DB:
            # SpessaSynth, comme les synthétiseurs EMU, n'applique que 40 % de l'atténuation écrite
            globaux.append((sf3.ATTENUATION, round(ATTENUATIONS_DB[programme.numero] * 10 / 0.4)))
        modulateurs = [(0x0502, sf3.ATTENUATION, 0, 0, 0), (0x0102, sf3.ATTENUATION, 200, 0, 0)]
        instruments.append(sf3.Instrument(programme.nom, zones, modulateurs, globaux))
        deja.setdefault(cle, len(instruments) - 1)
        presets.append(sf3.Preset(programme.nom, 0, programme.numero, len(instruments) - 1, reglages))
        print(f'  {programme.numero:3d} {programme.nom} : {len(zones) // 2} sons', flush=True)
    sf3.ecrire_sf3(
        sortie,
        f'{nom_famille} (VSCO 2 CE, allégé)',
        echantillons,
        instruments,
        presets,
        copyright='VSCO 2 Community Edition, Versilian Studios, CC0 1.0',
        commentaire='Allégé pour song-audio : deux nuances, boucles en fondu enchaîné, Ogg Vorbis.',
        qualite=QUALITE,
    )
    print(f'{sortie} : {os.path.getsize(sortie) / 1e6:.1f} Mo, {len(echantillons)} échantillons')


def charger_source(racine: str, source: Source) -> list[tuple[int, list[tuple[int, np.ndarray, int]]]]:
    """[(note réelle, [(nuance, données stéréo, fréquence)])], hauteurs corrigées d'octave."""
    dossier = os.path.join(racine, source.dossier)
    fichiers = [os.path.join(source.dossier, f) for f in os.listdir(dossier) if f.lower().endswith('.wav')]
    notes = {}
    for note, couches in choisir(fichiers).items():
        if not source.bas <= note <= source.haut:
            continue
        lues = []
        for nuance, chemin in couches:
            chemin_complet = os.path.join(racine, chemin)
            donnees, frequence = sf.read(chemin_complet, dtype='float32', always_2d=True)
            if donnees.shape[1] == 1:
                donnees = np.repeat(donnees, 2, axis=1)
            lues.append((nuance, donnees[:, :2], frequence, chemin))
        notes[note] = lues
    octave = correction_octave([(n, c[-1][1].mean(axis=1), c[-1][2]) for n, c in notes.items()])
    if octave:
        print(f'    {source.dossier} : octave corrigée de {octave:+d}', flush=True)
    resultat = []
    for n, c in notes.items():
        reelle = n + octave
        # quelques fichiers sont mal nommés (« F#5 » qui joue un fa) : un écart net et stable
        # d'un ou deux demi-tons est corrigé ; un écart d'octave vient du détecteur, on l'ignore
        mesures = [note_mesuree(donnees.mean(axis=1), frequence) for _, donnees, frequence, _ in c]
        ecarts = [m - reelle for m in mesures if m is not None]
        if ecarts and all(abs(e - round(ecarts[0])) < 0.15 for e in ecarts) and 1 <= abs(round(ecarts[0])) <= 2:
            print(f'    {os.path.basename(c[0][3])} : note corrigée de {round(ecarts[0]):+d} demi-ton', flush=True)
            reelle += round(ecarts[0])
        resultat.append((reelle, c))
    return sorted(resultat, key=lambda x: x[0])


def construire_zones(racine: str, programme: Programme, echantillons: list[sf3.Echantillon], partages: dict) -> list[sf3.Zone]:
    notes = []
    for source in programme.sources:
        for note, couches in charger_source(racine, source):
            if notes and note <= notes[-1][0]:
                continue  # la source suivante ne reprend qu'au-dessus de la précédente
            notes.append((note, normaliser_note(equilibrer_couches(couches))))
    zones = []
    for i, (note, couches) in enumerate(notes):
        bas = 0 if i == 0 else (notes[i - 1][0] + note) // 2 + 1
        haut = 127 if i == len(notes) - 1 else (note + notes[i + 1][0]) // 2
        for k, (_, donnees, frequence, chemin) in enumerate(couches):
            vel_bas, vel_haut = (1, 127) if len(couches) == 1 else ((1, SEUIL_FORTE - 1) if k == 0 else (SEUIL_FORTE, 127))
            cle = (chemin, note, programme.tenu)
            if cle not in partages:
                if programme.tenu:
                    donnees, boucle = boucler(donnees, frequence)
                else:
                    donnees, boucle = raccourcir(donnees, frequence, 5), None
                g, d = len(echantillons), len(echantillons) + 1
                nom = f'{programme.numero}_{note}_{k}'
                echantillons.append(sf3.Echantillon(f'{nom}G', donnees[:, 0], frequence, note, boucle, sf3.GAUCHE, d))
                echantillons.append(sf3.Echantillon(f'{nom}D', donnees[:, 1], frequence, note, boucle, sf3.DROITE, g))
                partages[cle] = (g, d)
            g, d = partages[cle]
            for indice, pan in ((g, -500), (d, 500)):
                generateurs = [
                    (sf3.TOUCHES, sf3.plage(bas, haut)),
                    (sf3.VELOCITES, sf3.plage(vel_bas, vel_haut)),
                    (sf3.PAN, pan),
                    (sf3.NOTE_DE_BASE, note),
                ]
                if programme.tenu:
                    generateurs.append((sf3.MODE_ECHANTILLON, 1))
                zones.append(sf3.Zone(generateurs, indice))
    return zones


if __name__ == '__main__':
    if sys.argv[1] == 'liste':
        lister(sys.argv[2])
    else:
        racine, dossier_sortie = sys.argv[2], sys.argv[3]
        for famille in sys.argv[4:] or FAMILLES:
            print(famille, flush=True)
            construire_famille(racine, famille, os.path.join(dossier_sortie, f'orchestre-{famille}.sf3'))
