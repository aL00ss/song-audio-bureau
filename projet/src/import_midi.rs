//! Import d'un fichier MIDI : chaque piste devient un canal avec son instrument General MIDI,
//! toutes les notes vont dans un pattern « Morceau », la pédale forte est inscrite dans la durée
//! des notes. Même résultat que la version web (vérifié note par note sur les morceaux fournis),
//! avec une amélioration : une piste qui mélange plusieurs canaux MIDI (fichiers « format 0 »)
//! donne un instrument par canal au lieu d'un seul.

use std::collections::BTreeMap;

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

use crate::gm::CANAL_BATTERIE;
use crate::modele::{NoteMusicale, OptionsCanal, Projet, creer_pattern, creer_projet};
use crate::temps::{Signature, TICS_PAR_NOIRE, arrondir_a_la_mesure};

const CONTROLEUR_PEDALE: u8 = 64;
/// Résolution supposée quand le fichier compte en images SMPTE (rare).
const PPQ_PAR_DEFAUT: u32 = 480;

/// Intervalles [début, fin[ (en tics) pendant lesquels la pédale forte est enfoncée ; une pédale
/// jamais relâchée court jusqu'à la fin (`None`).
pub fn intervalles_pedale(evenements: &[(u32, bool)]) -> Vec<(u32, Option<u32>)> {
    let mut tries = evenements.to_vec();
    tries.sort_by_key(|e| e.0);
    let mut intervalles = Vec::new();
    let mut debut: Option<u32> = None;
    for (tic, enfoncee) in tries {
        match (enfoncee, debut) {
            (true, None) => debut = Some(tic),
            (false, Some(d)) => {
                intervalles.push((d, Some(tic)));
                debut = None;
            }
            _ => {}
        }
    }
    if let Some(d) = debut {
        intervalles.push((d, None));
    }
    intervalles
}

/// Applique la pédale : une note relâchée pendant que la pédale est enfoncée continue jusqu'au
/// relâchement de la pédale, sauf si la même touche est rejouée avant.
pub fn appliquer_pedale(notes: &[NoteMusicale], intervalles: &[(u32, Option<u32>)]) -> Vec<NoteMusicale> {
    let mut triees = notes.to_vec();
    triees.sort_by_key(|n| (n.tic, n.hauteur));
    let mut fins: Vec<u32> = triees
        .iter()
        .map(|n| {
            let lache = n.tic + n.duree;
            match intervalles.iter().find(|(d, f)| *d <= lache && f.is_none_or(|f| lache < f)) {
                Some((_, Some(fin))) => *fin,
                _ => lache,
            }
        })
        .collect();
    let mut prochaine: BTreeMap<u8, u32> = BTreeMap::new();
    for i in (0..triees.len()).rev() {
        if let Some(&suivante) = prochaine.get(&triees[i].hauteur) {
            if suivante > triees[i].tic && fins[i] > suivante {
                fins[i] = suivante;
            }
        }
        prochaine.insert(triees[i].hauteur, triees[i].tic);
    }
    triees.iter().zip(fins).map(|(n, fin)| NoteMusicale { duree: n.duree.max(fin.saturating_sub(n.tic)).max(1), ..*n }).collect()
}

#[derive(Clone, Copy)]
struct Allumage {
    tic: u32,
    canal: u8,
    hauteur: u8,
    velocite: u8,
}

/// Ce qu'on garde d'une piste du fichier.
#[derive(Default)]
struct PisteLue {
    nom: String,
    /// (tic, canal, hauteur, vélocité) des allumages, dans l'ordre du fichier.
    allumages: Vec<Allumage>,
    /// (tic, canal, hauteur) des extinctions (y compris les allumages de vélocité nulle).
    extinctions: Vec<(u32, u8, u8)>,
    /// Premier changement de programme de la piste, et de chaque canal.
    programme: Option<u8>,
    programmes: BTreeMap<u8, u8>,
    /// (tic, canal, enfoncée) des pédales fortes.
    pedale: Vec<(u32, u8, bool)>,
}

/// Apparie chaque allumage à la première extinction de même hauteur qui ne le précède pas (comme
/// la version web) ; `meme_canal` : n'apparier qu'au sein d'un canal. Les notes sans extinction
/// sont ignorées.
fn apparier(allumages: &[Allumage], extinctions: &[(u32, u8, u8)], meme_canal: bool) -> Vec<(Allumage, u32)> {
    let mut restantes: Vec<(u32, u8, u8)> = extinctions.to_vec();
    let mut notes = Vec::with_capacity(allumages.len());
    for a in allumages {
        let trouvee = restantes.iter().position(|&(t, c, h)| h == a.hauteur && t >= a.tic && (!meme_canal || c == a.canal));
        if let Some(i) = trouvee {
            let (t, _, _) = restantes.remove(i);
            notes.push((*a, t - a.tic));
        }
    }
    notes
}

fn texte(octets: &[u8]) -> String {
    match std::str::from_utf8(octets) {
        Ok(s) => s.to_string(),
        // les vieux fichiers sont souvent en Latin-1
        Err(_) => octets.iter().map(|&b| b as char).collect(),
    }
}

/// Lit un fichier MIDI et en fait un projet.
pub fn depuis_midi(octets: &[u8], nom: &str) -> Result<Projet, String> {
    let smf = Smf::parse(octets).map_err(|e| format!("Fichier MIDI illisible : {e}"))?;
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => (t.as_int() as u32).max(1),
        Timing::Timecode(..) => PPQ_PAR_DEFAUT,
    };
    let vers_tics = |t: u64| -> u32 { ((t as f64) * TICS_PAR_NOIRE as f64 / ppq as f64).round() as u32 };

    let mut tempos: Vec<(u64, f64)> = Vec::new();
    let mut signatures: Vec<(u64, Signature)> = Vec::new();
    let mut pistes: Vec<PisteLue> = Vec::new();
    for evenements in &smf.tracks {
        let mut piste = PisteLue::default();
        let mut temps: u64 = 0;
        for e in evenements {
            temps += e.delta.as_int() as u64;
            match e.kind {
                TrackEventKind::Midi { channel, message } => {
                    let canal = channel.as_int();
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            piste.allumages.push(Allumage { tic: temps as u32, canal, hauteur: key.as_int(), velocite: vel.as_int() })
                        }
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => piste.extinctions.push((temps as u32, canal, key.as_int())),
                        MidiMessage::ProgramChange { program } => {
                            piste.programme.get_or_insert(program.as_int());
                            piste.programmes.entry(canal).or_insert(program.as_int());
                        }
                        MidiMessage::Controller { controller, value } if controller.as_int() == CONTROLEUR_PEDALE => {
                            piste.pedale.push((temps as u32, canal, value.as_int() >= 64))
                        }
                        _ => {}
                    }
                }
                TrackEventKind::Meta(MetaMessage::Tempo(us)) if us.as_int() > 0 => tempos.push((temps, 60_000_000.0 / us.as_int() as f64)),
                TrackEventKind::Meta(MetaMessage::TimeSignature(numerateur, puissance, _, _)) if numerateur > 0 => signatures.push((
                    temps,
                    Signature { numerateur: numerateur as u32, denominateur: 1u32.checked_shl(puissance as u32).unwrap_or(4).max(1) },
                )),
                TrackEventKind::Meta(MetaMessage::TrackName(n)) if piste.nom.is_empty() => piste.nom = texte(n),
                _ => {}
            }
        }
        pistes.push(piste);
    }

    let mut projet = creer_projet(nom);
    tempos.sort_by_key(|t| t.0);
    signatures.sort_by_key(|s| s.0);
    if let Some(&(_, bpm)) = tempos.first() {
        if bpm.is_finite() {
            projet.tempo = (bpm * 100.0).round() / 100.0;
        }
    }
    if let Some(&(_, s)) = signatures.first() {
        projet.signature = s;
    }

    let mut pattern = creer_pattern("Morceau", 384);
    let mut fin = 0u32;
    for piste in &pistes {
        if piste.allumages.is_empty() {
            continue;
        }
        let canaux: Vec<u8> = {
            let mut c: Vec<u8> = piste.allumages.iter().map(|a| a.canal).collect();
            c.sort_unstable();
            c.dedup();
            c
        };
        // une piste sur un seul canal : comme la version web ; plusieurs canaux : un instrument
        // par canal
        let groupes: Vec<(Option<u8>, Vec<(Allumage, u32)>)> = if canaux.len() == 1 {
            vec![(None, apparier(&piste.allumages, &piste.extinctions, false))]
        } else {
            let paires = apparier(&piste.allumages, &piste.extinctions, true);
            canaux.iter().map(|&c| (Some(c), paires.iter().filter(|(a, _)| a.canal == c).copied().collect())).collect()
        };
        for (canal_seul, paires) in groupes {
            if paires.is_empty() {
                continue;
            }
            // le canal retenu est celui de la dernière note (comme la version web)
            let canal_midi = canal_seul.unwrap_or_else(|| piste.allumages.last().map(|a| a.canal).unwrap_or(0));
            let batterie = canal_midi == CANAL_BATTERIE;
            let programme = match canal_seul {
                Some(c) => piste.programmes.get(&c).copied().or(piste.programme),
                None => piste.programme,
            }
            .unwrap_or(0)
            .min(127);
            let nom_piste = piste.nom.trim();
            let nom = match (nom_piste.is_empty(), canal_seul) {
                (true, _) => None,
                (false, None) => Some(nom_piste.to_string()),
                (false, Some(c)) => Some(format!("{nom_piste} (canal {})", c + 1)),
            };
            let id = projet.ajouter_canal(OptionsCanal { programme, batterie, nom, ..Default::default() });
            let mut notes: Vec<NoteMusicale> = paires
                .iter()
                .map(|(a, duree)| NoteMusicale {
                    tic: vers_tics(a.tic as u64),
                    duree: vers_tics(*duree as u64).max(1),
                    hauteur: a.hauteur,
                    velocite: a.velocite.clamp(1, 127),
                })
                .collect();
            let pedale: Vec<(u32, bool)> =
                piste.pedale.iter().filter(|(_, c, _)| canal_seul.is_none_or(|s| s == *c)).map(|&(t, _, e)| (vers_tics(t as u64), e)).collect();
            if !pedale.is_empty() && !batterie {
                notes = appliquer_pedale(&notes, &intervalles_pedale(&pedale));
            } else {
                notes.sort_by_key(|n| (n.tic, n.hauteur));
            }
            for n in &notes {
                fin = fin.max(n.tic + n.duree);
            }
            pattern.notes.insert(id, notes);
        }
    }
    pattern.longueur = arrondir_a_la_mesure(fin, projet.signature);
    let id = pattern.id.clone();
    projet.patterns = vec![pattern];
    projet.pattern_actif = id.clone();
    projet.playlist.clear();
    projet.ajouter_clip(&id, 0, 0);
    Ok(projet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use midly::num::{u4, u7, u15, u24, u28};
    use midly::{Format, Header, TrackEvent};
    use serde_json::Value;

    fn note(t: u32, canal: u8, message: MidiMessage) -> (u32, TrackEventKind<'static>) {
        (t, TrackEventKind::Midi { channel: u4::new(canal), message })
    }

    fn on(t: u32, canal: u8, h: u8, v: u8) -> (u32, TrackEventKind<'static>) {
        note(t, canal, MidiMessage::NoteOn { key: u7::new(h), vel: u7::new(v) })
    }

    fn off(t: u32, canal: u8, h: u8) -> (u32, TrackEventKind<'static>) {
        note(t, canal, MidiMessage::NoteOff { key: u7::new(h), vel: u7::new(0) })
    }

    /// Fabrique un fichier MIDI (480 tics par noire) à partir de pistes d'événements datés.
    fn fichier(format: Format, pistes: Vec<Vec<(u32, TrackEventKind<'static>)>>) -> Vec<u8> {
        let mut smf = Smf::new(Header::new(format, Timing::Metrical(u15::new(480))));
        for mut evenements in pistes {
            evenements.sort_by_key(|e| e.0);
            let mut precedent = 0;
            let mut piste: Vec<TrackEvent> = evenements
                .into_iter()
                .map(|(t, kind)| {
                    let e = TrackEvent { delta: u28::new(t - precedent), kind };
                    precedent = t;
                    e
                })
                .collect();
            piste.push(TrackEvent { delta: u28::new(0), kind: TrackEventKind::Meta(MetaMessage::EndOfTrack) });
            smf.tracks.push(piste);
        }
        let mut octets = Vec::new();
        smf.write_std(&mut octets).unwrap();
        octets
    }

    #[test]
    fn identique_a_la_version_web_sur_les_morceaux_fournis() {
        let dossier = concat!(env!("CARGO_MANIFEST_DIR"), "/../ressources/");
        let reference: Value = serde_json::from_str(include_str!("../tests/donnees/reference-import-web.json")).unwrap();
        let mut verifies = 0;
        for (fichier, attendu) in reference.as_object().unwrap() {
            let octets = std::fs::read(format!("{dossier}{fichier}")).unwrap();
            let p = depuis_midi(&octets, "essai").unwrap();
            assert_eq!(p.tempo, attendu["tempo"].as_f64().unwrap(), "{fichier} : tempo");
            assert_eq!([p.signature.numerateur, p.signature.denominateur], [attendu["signature"][0].as_u64().unwrap() as u32, attendu["signature"][1].as_u64().unwrap() as u32]);
            assert_eq!(p.pattern_actif().longueur as u64, attendu["longueur"].as_u64().unwrap(), "{fichier} : longueur");
            let canaux = attendu["canaux"].as_array().unwrap();
            assert_eq!(p.canaux.len(), canaux.len(), "{fichier} : nombre de canaux");
            for (canal, a) in p.canaux.iter().zip(canaux) {
                assert_eq!(canal.nom, a["nom"].as_str().unwrap());
                assert_eq!(canal.programme as u64, a["programme"].as_u64().unwrap());
                assert_eq!(canal.batterie, a["batterie"].as_bool().unwrap());
                let notes: Vec<[u64; 4]> = p.pattern_actif().notes[&canal.id].iter().map(|n| [n.tic as u64, n.duree as u64, n.hauteur as u64, n.velocite as u64]).collect();
                let attendues: Vec<[u64; 4]> = a["notes"].as_array().unwrap().iter().map(|n| [0, 1, 2, 3].map(|i| n[i].as_u64().unwrap())).collect();
                assert_eq!(notes, attendues, "{fichier} : notes de « {} »", canal.nom);
                verifies += notes.len();
            }
        }
        assert!(verifies > 8000, "{verifies} notes vérifiées");
    }

    #[test]
    fn tempo_signature_programme_batterie_et_noms() {
        let octets = fichier(
            Format::Parallel,
            vec![
                vec![(0, TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000)))), (0, TrackEventKind::Meta(MetaMessage::TimeSignature(3, 2, 24, 8)))],
                vec![
                    (0, TrackEventKind::Meta(MetaMessage::TrackName(b"Violon"))),
                    note(0, 0, MidiMessage::ProgramChange { program: u7::new(40) }),
                    on(0, 0, 67, 90),
                    off(480, 0, 67),
                ],
                vec![on(240, 9, 36, 110), on(360, 9, 36, 0)],
            ],
        );
        let p = depuis_midi(&octets, "Essai").unwrap();
        assert_eq!(p.tempo, 120.0);
        assert_eq!((p.signature.numerateur, p.signature.denominateur), (3, 4));
        assert_eq!(p.canaux.len(), 2, "violon et batterie");
        assert_eq!((p.canaux[0].nom.as_str(), p.canaux[0].programme), ("Violon", 40));
        let n = p.pattern_actif().notes[&p.canaux[0].id][0];
        assert_eq!((n.tic, n.duree, n.hauteur, n.velocite), (0, 96, 67, 90));
        assert!(p.canaux[1].batterie, "notes sur le canal 10 : batterie");
        let caisse = p.pattern_actif().notes[&p.canaux[1].id][0];
        assert_eq!((caisse.tic, caisse.duree, caisse.hauteur), (48, 24, 36), "vélocité nulle = extinction");
        assert_eq!(p.pattern_actif().longueur, 288, "une mesure à 3/4");
    }

    #[test]
    fn note_sans_extinction_ignoree_et_velocite_nulle_vaut_extinction() {
        let octets = fichier(Format::SingleTrack, vec![vec![on(0, 0, 60, 100), on(480, 0, 60, 0), on(960, 0, 64, 80)]]);
        let p = depuis_midi(&octets, "Essai").unwrap();
        let notes = &p.pattern_actif().notes[&p.canaux[0].id];
        assert_eq!(notes.len(), 1, "le mi sans extinction est ignoré, comme dans la version web");
        assert_eq!((notes[0].tic, notes[0].duree), (0, 96));
    }

    #[test]
    fn format_0_un_instrument_par_canal() {
        let octets = fichier(
            Format::SingleTrack,
            vec![vec![
                (0, TrackEventKind::Meta(MetaMessage::TrackName(b"Tout"))),
                note(0, 0, MidiMessage::ProgramChange { program: u7::new(0) }),
                note(0, 1, MidiMessage::ProgramChange { program: u7::new(33) }),
                on(0, 0, 72, 90),
                on(0, 1, 36, 100),
                on(0, 9, 38, 110),
                off(480, 0, 72),
                off(480, 1, 36),
                off(120, 9, 38),
            ]],
        );
        let p = depuis_midi(&octets, "Essai").unwrap();
        let noms: Vec<(&str, u8, bool)> = p.canaux.iter().map(|c| (c.nom.as_str(), c.programme, c.batterie)).collect();
        assert_eq!(noms, vec![("Tout (canal 1)", 0, false), ("Tout (canal 2)", 33, false), ("Tout (canal 10)", 0, true)]);
        for c in &p.canaux {
            assert_eq!(p.pattern_actif().notes[&c.id].len(), 1, "{}", c.nom);
        }
    }

    #[test]
    fn pedale_prolonge_les_notes_sauf_touche_rejouee() {
        let notes = vec![
            NoteMusicale { tic: 0, duree: 10, hauteur: 60, velocite: 100 },
            NoteMusicale { tic: 20, duree: 10, hauteur: 64, velocite: 100 },
            NoteMusicale { tic: 50, duree: 10, hauteur: 60, velocite: 100 },
        ];
        let pedale = intervalles_pedale(&[(5, true), (100, false)]);
        let r = appliquer_pedale(&notes, &pedale);
        assert_eq!(r.iter().map(|n| n.duree).collect::<Vec<_>>(), vec![50, 80, 50], "le do s'arrête quand il est rejoué");
        assert_eq!(intervalles_pedale(&[(5, true)]), vec![(5, None)]);
    }

    #[test]
    fn fichier_abime_refuse() {
        assert!(depuis_midi(b"pas un fichier midi", "x").unwrap_err().contains("illisible"));
    }
}
