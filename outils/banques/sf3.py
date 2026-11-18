"""Écriture d'une banque SoundFont 3 (SF2 dont les échantillons sont compressés en Ogg Vorbis).

Format : RIFF « sfbk » avec trois listes (INFO, sdta, pdta). En SF3, chaque échantillon est un
flux Ogg complet dans le bloc smpl ; début et fin de l'en-tête d'échantillon (shdr) y sont des
positions en octets, et le bit 0x10 du type d'échantillon signale la compression. Un son stéréo
est une paire d'échantillons mono liés (gauche / droite), chacun dans sa propre zone.
"""

from __future__ import annotations

import io
import struct
from dataclasses import dataclass, field

import numpy as np
import soundfile as sf

# générateurs utilisés (numéros de la norme SoundFont 2.04)
PAN = 17
ATTAQUE_VOL = 34
MAINTIEN_VOL = 35
DECROISSANCE_VOL = 36
SOUTIEN_VOL = 37
RELACHEMENT_VOL = 38
INSTRUMENT = 41
TOUCHES = 43
VELOCITES = 44
ATTENUATION = 48
REGLAGE_FIN = 52
ECHANTILLON = 53
MODE_ECHANTILLON = 54
NOTE_DE_BASE = 58

# types d'échantillon
MONO, DROITE, GAUCHE = 1, 2, 4
VORBIS = 0x10


def temps_en_timecents(secondes: float) -> int:
    return round(1200 * np.log2(max(secondes, 0.001)))


@dataclass
class Echantillon:
    nom: str
    donnees: np.ndarray  # mono, flottants entre -1 et 1
    frequence: int
    note_de_base: int
    boucle: tuple[int, int] | None = None  # en trames, depuis le début de l'échantillon
    type: int = MONO
    lien: int = 0


@dataclass
class Zone:
    generateurs: list[tuple[int, int]]  # (générateur, valeur) ; touches et vélocités en premier
    echantillon: int | None = None  # None : zone globale


@dataclass
class Instrument:
    nom: str
    zones: list[Zone]
    # modulateurs de la zone globale : (source, destination, quantité, source de quantité, transformation)
    modulateurs: list[tuple[int, int, int, int, int]] = field(default_factory=list)
    # générateurs de la zone globale, appliqués à toutes les zones (atténuation commune…)
    globaux: list[tuple[int, int]] = field(default_factory=list)


@dataclass
class Preset:
    nom: str
    banque: int
    programme: int
    instrument: int
    # générateurs de la zone du preset, ajoutés à ceux de l'instrument (attaque plus lente…)
    generateurs: list[tuple[int, int]] = field(default_factory=list)


def plage(bas: int, haut: int) -> int:
    """Valeur d'un générateur de plage (touches ou vélocités)."""
    return bas | (haut << 8)


def encoder_vorbis(donnees: np.ndarray, frequence: int, qualite: float) -> bytes:
    tampon = io.BytesIO()
    donnees = np.ascontiguousarray(donnees, dtype=np.float32)
    # compression_level : 0 = meilleure qualité, 1 = fichier le plus petit. Écriture par blocs :
    # libsndfile déborde de la pile quand on lui donne un long signal Vorbis d'un coup.
    with sf.SoundFile(tampon, 'w', frequence, 1, format='OGG', subtype='VORBIS', compression_level=1 - qualite) as f:
        for debut in range(0, len(donnees), 4096):
            f.write(donnees[debut : debut + 4096])
    return tampon.getvalue()


def _bloc(identifiant: bytes, contenu: bytes) -> bytes:
    if len(contenu) % 2:
        contenu += b'\0'
    return identifiant + struct.pack('<I', len(contenu)) + contenu


def _liste(type_: bytes, blocs: list[bytes]) -> bytes:
    return _bloc(b'LIST', type_ + b''.join(blocs))


def _nom(texte: str, longueur: int = 20) -> bytes:
    return texte.encode('latin-1', 'replace')[: longueur - 1].ljust(longueur, b'\0')


def _texte(texte: str) -> bytes:
    octets = texte.encode('latin-1', 'replace') + b'\0'
    return octets + (b'\0' if len(octets) % 2 else b'')


def ecrire_sf3(
    chemin: str,
    nom: str,
    echantillons: list[Echantillon],
    instruments: list[Instrument],
    presets: list[Preset],
    copyright: str,
    commentaire: str,
    qualite: float = 0.4,
) -> None:
    # ---- sdta : flux Ogg à la suite
    smpl = bytearray()
    entetes = []
    for e in echantillons:
        ogg = encoder_vorbis(e.donnees, e.frequence, qualite)
        debut = len(smpl)
        smpl += ogg
        fin = len(smpl)
        boucle = e.boucle or (0, 0)
        entetes.append(
            _nom(e.nom)
            + struct.pack('<IIIIIBbHH', debut, fin, boucle[0], boucle[1], e.frequence, e.note_de_base, 0, e.lien, e.type | VORBIS)
        )
    entetes.append(_nom('EOS') + struct.pack('<IIIIIBbHH', 0, 0, 0, 0, 0, 0, 0, 0, 0))

    # ---- instruments
    inst, ibag, imod, igen = [], [], [], []
    for instrument in instruments:
        inst.append(_nom(instrument.nom) + struct.pack('<H', len(ibag)))
        globale = Zone(instrument.globaux)
        for i, zone in enumerate([globale, *instrument.zones]):
            ibag.append(struct.pack('<HH', len(igen), len(imod)))
            if i == 0:
                for m in instrument.modulateurs:
                    imod.append(struct.pack('<HHhHH', *m))
            for g, v in zone.generateurs:
                # les plages sont deux octets ; les autres valeurs, des entiers signés
                igen.append(struct.pack('<HH' if g in (TOUCHES, VELOCITES) else '<Hh', g, v))
            if zone.echantillon is not None:
                igen.append(struct.pack('<HH', ECHANTILLON, zone.echantillon))
    inst.append(_nom('EOI') + struct.pack('<H', len(ibag)))
    ibag.append(struct.pack('<HH', len(igen), len(imod)))
    imod.append(b'\0' * 10)
    igen.append(b'\0' * 4)

    # ---- presets (une zone chacun, qui renvoie vers son instrument)
    phdr, pbag, pgen = [], [], []
    for p in presets:
        phdr.append(_nom(p.nom) + struct.pack('<HHHIII', p.programme, p.banque, len(pbag), 0, 0, 0))
        pbag.append(struct.pack('<HH', len(pgen), 0))
        for g, v in p.generateurs:
            pgen.append(struct.pack('<Hh', g, v))
        pgen.append(struct.pack('<HH', INSTRUMENT, p.instrument))
    phdr.append(_nom('EOP') + struct.pack('<HHHIII', 0, 0, len(pbag), 0, 0, 0))
    pbag.append(struct.pack('<HH', len(pgen), 0))
    pmod = [b'\0' * 10]
    pgen.append(b'\0' * 4)

    info = _liste(b'INFO', [
        _bloc(b'ifil', struct.pack('<HH', 3, 0)),
        _bloc(b'isng', _texte('EMU8000')),
        _bloc(b'INAM', _texte(nom)),
        _bloc(b'ICOP', _texte(copyright)),
        _bloc(b'ICMT', _texte(commentaire)),
        _bloc(b'ISFT', _texte('song-audio outils/banques')),
    ])
    sdta = _liste(b'sdta', [_bloc(b'smpl', bytes(smpl))])
    pdta = _liste(b'pdta', [
        _bloc(b'phdr', b''.join(phdr)),
        _bloc(b'pbag', b''.join(pbag)),
        _bloc(b'pmod', b''.join(pmod)),
        _bloc(b'pgen', b''.join(pgen)),
        _bloc(b'inst', b''.join(inst)),
        _bloc(b'ibag', b''.join(ibag)),
        _bloc(b'imod', b''.join(imod)),
        _bloc(b'igen', b''.join(igen)),
        _bloc(b'shdr', b''.join(entetes)),
    ])
    with open(chemin, 'wb') as f:
        f.write(_bloc(b'RIFF', b'sfbk' + info + sdta + pdta))
