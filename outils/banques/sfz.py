"""Lecture simplifiée d'un fichier SFZ : régions avec les réglages hérités de <global>,
<master> et <group>. Suffit pour les banques utilisées ici (Salamander, VSCO 2 CE)."""

from __future__ import annotations

import os
import re

NOMS = {'c': 0, 'd': 2, 'e': 4, 'f': 5, 'g': 7, 'a': 9, 'b': 11}


def note(valeur: str) -> int:
    """« 60 », « c4 », « f#3 », « Eb2 » -> numéro MIDI (do4 = 60)."""
    valeur = valeur.strip()
    if re.fullmatch(r'-?\d+', valeur):
        return int(valeur)
    m = re.fullmatch(r'([a-gA-G])([#b]?)(-?\d+)', valeur)
    if not m:
        raise ValueError(f'note illisible : {valeur}')
    hauteur = NOMS[m.group(1).lower()] + {'#': 1, 'b': -1, '': 0}[m.group(2)]
    return (int(m.group(3)) + 1) * 12 + hauteur


def lire(chemin: str) -> list[dict[str, str]]:
    texte = open(chemin, encoding='utf-8', errors='replace').read()
    texte = re.sub(r'//[^\n]*', '', texte)
    dossier = os.path.dirname(chemin)
    niveaux: dict[str, dict[str, str]] = {'global': {}, 'master': {}, 'group': {}}
    regions: list[dict[str, str]] = []
    courant: dict[str, str] | None = None
    defines: dict[str, str] = {}
    for m in re.finditer(r'<(\w+)>|#define\s+(\$\w+)\s+(\S+)|(\w+)=((?:(?!\s+\w+=|\s*<|\s*//).)*)', texte, re.S):
        if m.group(1):
            entete = m.group(1)
            if entete == 'region':
                courant = {**niveaux['global'], **niveaux['master'], **niveaux['group']}
                regions.append(courant)
            elif entete in niveaux:
                if entete == 'global':
                    niveaux['master'] = {}
                if entete in ('global', 'master'):
                    niveaux['group'] = {}
                niveaux[entete] = {}
                courant = niveaux[entete]
            else:
                courant = {}  # <control>, <curve>… : seul default_path de <control> est utilisé
                if entete == 'control':
                    niveaux['control'] = courant
        elif m.group(2):
            defines[m.group(2)] = m.group(3)
        elif courant is not None:
            valeur = m.group(5).strip()
            for k, v in defines.items():
                valeur = valeur.replace(k, v)
            courant[m.group(4)] = valeur
    chemin_defaut = niveaux.get('control', {}).get('default_path', '')
    for r in regions:
        if 'sample' in r:
            r['_fichier'] = os.path.normpath(os.path.join(dossier, chemin_defaut, r['sample'].replace('\\', '/')))
        for cle in ('lokey', 'hikey', 'key', 'pitch_keycenter'):
            if cle in r:
                r[cle] = str(note(r[cle]))
        if 'key' in r:
            r.setdefault('lokey', r['key'])
            r.setdefault('hikey', r['key'])
            r.setdefault('pitch_keycenter', r['key'])
        r.setdefault('pitch_keycenter', '60')  # valeur par défaut de la norme SFZ
    return regions
