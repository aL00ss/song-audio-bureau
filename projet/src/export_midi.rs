//! Export du projet en fichier MIDI (format 1, 480 tics par noire). Les notes viennent de la même
//! liste que la lecture : le swing, les canaux coupés et le mode (pattern ou morceau) sont donc
//! respectés. Chaque canal du projet devient une piste avec son instrument General MIDI. Repris
//! de la version web avec ses tests.

use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

use crate::gm::CANAL_BATTERIE;
use crate::lecture::{ModeLecture, longueur_du_morceau, notes_a_jouer};
use crate::modele::{Projet, attribuer_canaux_midi};
use crate::temps::TICS_PAR_NOIRE;

const PPQ: u32 = 480;
const ECHELLE: u32 = PPQ / TICS_PAR_NOIRE;

/// Longueur exportée, en tics du projet.
pub fn longueur_export(projet: &Projet, mode: ModeLecture) -> u32 {
    match mode {
        ModeLecture::Morceau => longueur_du_morceau(projet),
        ModeLecture::Pattern => projet.pattern_actif().longueur,
    }
}

/// Fichier MIDI du projet. `canaux_internes` : chaque canal garde son canal MIDI interne au lieu
/// de mettre toutes les batteries sur le canal 10 (fichier standard).
pub fn vers_midi(projet: &Projet, mode: ModeLecture, canaux_internes: bool) -> Vec<u8> {
    let (notes, _) = notes_a_jouer(projet, mode);
    let attribution = attribuer_canaux_midi(&projet.canaux);
    let noms: Vec<Vec<u8>> = projet.canaux.iter().map(|c| c.nom.as_bytes().to_vec()).collect();
    let nom_projet = projet.nom.as_bytes().to_vec();

    let evenement = |delta: u32, kind| TrackEvent { delta: u28::new(delta), kind };
    // piste de tempo : nom du morceau, tempo, signature
    let denominateur = (projet.signature.denominateur.max(1) as f64).log2().round() as u8;
    let mut pistes: Vec<Vec<TrackEvent>> = vec![vec![
        evenement(0, TrackEventKind::Meta(MetaMessage::TrackName(&nom_projet))),
        evenement(0, TrackEventKind::Meta(MetaMessage::Tempo(u24::new((60_000_000.0 / projet.tempo.max(1.0)).round() as u32)))),
        evenement(0, TrackEventKind::Meta(MetaMessage::TimeSignature(projet.signature.numerateur as u8, denominateur, 24, 8))),
        evenement(0, TrackEventKind::Meta(MetaMessage::EndOfTrack)),
    ]];
    // une piste par canal qui a des notes, dans l'ordre des canaux du projet
    for (rang, canal) in projet.canaux.iter().enumerate() {
        let siennes: Vec<_> = notes.iter().filter(|n| n.canal as usize == rang).collect();
        if siennes.is_empty() {
            continue;
        }
        let interne = attribution.get(&canal.id).copied().unwrap_or(0);
        let numero = if canal.batterie && !canaux_internes { CANAL_BATTERIE } else { (interne % 16) as u8 };
        let canal_midi = u4::new(numero);
        // (instant, extinction d'abord, message)
        let mut messages: Vec<(u32, u8, MidiMessage)> = Vec::new();
        for n in siennes {
            let (debut, fin) = (n.debut * ECHELLE, (n.fin * ECHELLE).max(n.debut * ECHELLE + 1));
            messages.push((debut, 1, MidiMessage::NoteOn { key: u7::new(n.hauteur.min(127)), vel: u7::new(n.velocite.clamp(1, 127)) }));
            messages.push((fin, 0, MidiMessage::NoteOff { key: u7::new(n.hauteur.min(127)), vel: u7::new(0) }));
        }
        messages.sort_by_key(|(t, ordre, _)| (*t, *ordre));
        let mut piste = vec![evenement(0, TrackEventKind::Meta(MetaMessage::TrackName(&noms[rang])))];
        if !canal.batterie {
            piste.push(evenement(0, TrackEventKind::Midi { channel: canal_midi, message: MidiMessage::ProgramChange { program: u7::new(canal.programme.min(127)) } }));
        }
        let mut precedent = 0;
        for (t, _, message) in messages {
            piste.push(evenement(t - precedent, TrackEventKind::Midi { channel: canal_midi, message }));
            precedent = t;
        }
        piste.push(evenement(0, TrackEventKind::Meta(MetaMessage::EndOfTrack)));
        pistes.push(piste);
    }
    let smf = Smf { header: Header::new(Format::Parallel, Timing::Metrical(u15::new(PPQ as u16))), tracks: pistes };
    let mut octets = Vec::new();
    smf.write_std(&mut octets).expect("écriture en mémoire");
    octets
}

/// Nom de fichier sûr : « Démo » -> « Démo.mid ».
pub fn nom_de_fichier(nom: &str, extension: &str) -> String {
    let remplace: String = nom.chars().map(|c| if "\\/:*?\"<>|".contains(c) { ' ' } else { c }).collect();
    let propre = remplace.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}.{extension}", if propre.is_empty() { "projet" } else { &propre })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::creer_demo;
    use crate::import_midi::depuis_midi;
    use crate::modele::{OptionsCanal, creer_projet};
    use crate::pas::definir_pas;
    use crate::playlist::{ajouter_clip, nouveau_pattern};

    /// Pistes du fichier (hors piste de tempo) : nom, programme, canal, attaques (tics à 480).
    fn relire(octets: &[u8]) -> Vec<(String, Option<u8>, u8, Vec<u32>)> {
        let smf = Smf::parse(octets).unwrap();
        smf.tracks
            .iter()
            .skip(1)
            .map(|piste| {
                let (mut nom, mut programme, mut canal, mut t, mut attaques) = (String::new(), None, 0, 0, Vec::new());
                for e in piste {
                    t += e.delta.as_int();
                    match e.kind {
                        TrackEventKind::Meta(MetaMessage::TrackName(n)) => nom = String::from_utf8_lossy(n).to_string(),
                        TrackEventKind::Midi { channel, message: MidiMessage::ProgramChange { program } } => {
                            programme = Some(program.as_int());
                            canal = channel.as_int();
                        }
                        TrackEventKind::Midi { channel, message: MidiMessage::NoteOn { vel, .. } } if vel.as_int() > 0 => {
                            canal = channel.as_int();
                            attaques.push(t);
                        }
                        _ => {}
                    }
                }
                (nom, programme, canal, attaques)
            })
            .collect()
    }

    // ---- les tests de la version web (exportMidi.test.ts), traduits

    #[test]
    fn aller_retour_la_demo_reimportee_a_les_memes_instruments_et_notes() {
        let demo = creer_demo();
        let octets = vers_midi(&demo, ModeLecture::Pattern, false);
        let pistes = relire(&octets);
        assert_eq!(
            pistes.iter().map(|(n, p, c, _)| (n.as_str(), *p, *c)).collect::<Vec<_>>(),
            vec![("Piano", Some(0), 0), ("Basse", Some(33), 1), ("Cordes", Some(48), 2), ("Batterie", None, 9)]
        );
        let relu = depuis_midi(&octets, "relu").unwrap();
        assert_eq!(relu.tempo, 100.0);
        let (avant, apres) = (demo.pattern_actif(), relu.pattern_actif());
        for (i, canal) in demo.canaux.iter().enumerate() {
            let a: Vec<_> = avant.notes[&canal.id].iter().map(|n| (n.tic, n.duree, n.hauteur, n.velocite)).collect();
            let b: Vec<_> = apres.notes[&relu.canaux[i].id].iter().map(|n| (n.tic, n.duree, n.hauteur, n.velocite)).collect();
            assert_eq!(b, a, "{}", canal.nom);
        }
    }

    #[test]
    fn les_canaux_coupes_ne_sont_pas_exportes_le_swing_est_applique() {
        let mut p = creer_projet("Essai");
        let a = p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Grosse caisse".into()), ..Default::default() });
        let b = p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Caisse claire".into()), note_defaut: Some(38), ..Default::default() });
        definir_pas(&mut p, &a, 0, None);
        definir_pas(&mut p, &a, 1, None);
        definir_pas(&mut p, &b, 4, None);
        p.canal_mut(&b).unwrap().muet = true;
        p.swing = 1.0;
        let pistes = relire(&vers_midi(&p, ModeLecture::Pattern, false));
        assert_eq!(pistes.len(), 1);
        assert_eq!(pistes[0].3, vec![0, (24 + 12) * 5], "2e pas retardé d'un demi-pas");
    }

    #[test]
    fn batteries_canal_10_dans_un_fichier_standard_canaux_internes_pour_le_rendu() {
        let mut p = creer_projet("Essai");
        let a = p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Grosse caisse".into()), ..Default::default() });
        let b = p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Caisse claire".into()), note_defaut: Some(38), ..Default::default() });
        definir_pas(&mut p, &a, 0, None);
        definir_pas(&mut p, &b, 4, None);
        assert_eq!(relire(&vers_midi(&p, ModeLecture::Pattern, false)).iter().map(|x| x.2).collect::<Vec<_>>(), vec![9, 9]);
        assert_eq!(relire(&vers_midi(&p, ModeLecture::Pattern, true)).iter().map(|x| x.2).collect::<Vec<_>>(), vec![9, 0]);
    }

    #[test]
    fn mode_morceau_les_clips_a_leur_place() {
        let mut p = creer_projet("Essai");
        let a = p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Grosse caisse".into()), ..Default::default() });
        definir_pas(&mut p, &a, 0, None);
        let premier = p.pattern_actif.clone();
        let deuxieme = nouveau_pattern(&mut p);
        definir_pas(&mut p, &a, 2, None);
        ajouter_clip(&mut p, &premier, 0, 0);
        ajouter_clip(&mut p, &deuxieme, 0, 384);
        ajouter_clip(&mut p, &premier, 0, 768);
        assert_eq!(longueur_export(&p, ModeLecture::Morceau), 3 * 384);
        let attaques: Vec<u32> = relire(&vers_midi(&p, ModeLecture::Morceau, false))[0].3.iter().map(|t| t / 5).collect();
        assert_eq!(attaques, vec![0, 384 + 48, 768]);
        assert_eq!(longueur_export(&p, ModeLecture::Pattern), 384);
    }

    #[test]
    fn nom_de_fichier_sur() {
        assert_eq!(nom_de_fichier("Démo", "mid"), "Démo.mid");
        assert_eq!(nom_de_fichier("a/b:c?", "wav"), "a b c.wav");
        assert_eq!(nom_de_fichier("   ", "mid"), "projet.mid");
    }

    #[test]
    fn plus_de_seize_canaux_sans_plafond() {
        let mut p = creer_projet("Grand");
        for i in 0..40u8 {
            let id = p.ajouter_canal(OptionsCanal { programme: i, ..Default::default() });
            definir_pas(&mut p, &id, (i % 16) as u32, None);
        }
        let pistes = relire(&vers_midi(&p, ModeLecture::Pattern, false));
        assert_eq!(pistes.len(), 40, "toutes les pistes, au-delà de 16");
        assert!(pistes.iter().all(|x| x.2 != 9), "le canal 10 reste aux batteries");
        assert_eq!(depuis_midi(&vers_midi(&p, ModeLecture::Pattern, false), "relu").unwrap().pattern_actif().notes.values().map(Vec::len).sum::<usize>(), 40);
    }
}
