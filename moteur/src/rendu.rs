//! Rendu hors ligne (export) : un noyau neuf joue la liste d'événements aussi vite que possible,
//! puis le résultat est écrit en WAV 16 bits stéréo.

use std::sync::Arc;

use rustysynth::SoundFont;

use crate::noyau::{BLOC_MAX, Configuration, Evenement, Noyau};
use crate::transport::{ActionTransport, Session, Transport};

/// Rend `duree` secondes ; les événements sont datés en échantillons à `frequence`.
pub fn rendre(
    banque: &Arc<SoundFont>,
    frequence: u32,
    configuration: &Configuration,
    evenements: &[Evenement],
    duree: f64,
) -> Result<(Vec<f32>, Vec<f32>), String> {
    let mut noyau = Noyau::new(banque, frequence)?;
    noyau.preparer(configuration)?;
    noyau.configurer(configuration);
    for e in evenements {
        noyau.programmer(*e);
    }
    let n = (duree.max(0.0) * frequence as f64).ceil() as usize;
    let (mut gauche, mut droite) = (vec![0.0; n], vec![0.0; n]);
    for debut in (0..n).step_by(BLOC_MAX) {
        let fin = (debut + BLOC_MAX).min(n);
        noyau.rendre(&mut gauche[debut..fin], &mut droite[debut..fin]);
    }
    Ok((gauche, droite))
}

/// Rend une session par le même transport que la lecture en direct (une seule fois, sans
/// boucle), puis `queue` secondes pour laisser les sons s'éteindre.
pub fn rendre_session(
    banque: &Arc<SoundFont>,
    frequence: u32,
    configuration: &Configuration,
    session: &Session,
    queue: f64,
) -> Result<(Vec<f32>, Vec<f32>), String> {
    let mut noyau = Noyau::new(banque, frequence)?;
    noyau.preparer(configuration)?;
    noyau.configurer(configuration);
    let mut transport = Transport::new();
    let mut session = session.clone();
    session.boucler = false;
    let duree = session.longueur as f64 / (session.tempo * crate::transport::TICS_PAR_NOIRE / 60.0) + queue.max(0.0);
    transport.definir_session(session, &mut noyau);
    transport.agir(ActionTransport::Jouer { decompte: false }, &mut noyau);
    let n = (duree * frequence as f64).ceil() as usize;
    let (mut gauche, mut droite) = (vec![0.0; n], vec![0.0; n]);
    for debut in (0..n).step_by(BLOC_MAX) {
        let fin = (debut + BLOC_MAX).min(n);
        transport.avancer(&mut noyau, fin - debut);
        noyau.rendre(&mut gauche[debut..fin], &mut droite[debut..fin]);
    }
    Ok((gauche, droite))
}

/// Fichier WAV (PCM 16 bits, stéréo).
pub fn wav(gauche: &[f32], droite: &[f32], frequence: u32) -> Vec<u8> {
    let trames = gauche.len().min(droite.len());
    let donnees = trames * 4;
    let mut octets = Vec::with_capacity(44 + donnees);
    let mut ecrire = |b: &[u8]| octets.extend_from_slice(b);
    ecrire(b"RIFF");
    ecrire(&(36 + donnees as u32).to_le_bytes());
    ecrire(b"WAVEfmt ");
    ecrire(&16u32.to_le_bytes());
    ecrire(&1u16.to_le_bytes()); // PCM
    ecrire(&2u16.to_le_bytes()); // stéréo
    ecrire(&frequence.to_le_bytes());
    ecrire(&(frequence * 4).to_le_bytes());
    ecrire(&4u16.to_le_bytes());
    ecrire(&16u16.to_le_bytes());
    ecrire(b"data");
    ecrire(&(donnees as u32).to_le_bytes());
    for i in 0..trames {
        for v in [gauche[i], droite[i]] {
            let entier = (v.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            octets.extend_from_slice(&entier.to_le_bytes());
        }
    }
    octets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noyau::Message;
    use crate::noyau::tests::{FE, banque, configuration, hauteur};

    #[test]
    fn rendu_hors_ligne_identique_et_wav_valide() {
        let evenements = [
            Evenement { echantillon: 0, canal: 0, message: Message::NoteOn { hauteur: 64, velocite: 100 } },
            Evenement { echantillon: FE as u64, canal: 0, message: Message::NoteOff { hauteur: 64 } },
        ];
        let c = configuration(0, false);
        let (g1, d1) = rendre(&banque(), FE, &c, &evenements, 2.0).unwrap();
        let (g2, _) = rendre(&banque(), FE, &c, &evenements, 2.0).unwrap();
        assert_eq!(g1, g2, "deux rendus identiques");
        assert_eq!(g1.len(), 2 * FE as usize);
        assert!((hauteur(&g1, 4410) - 64.0).abs() < 0.2);
        let octets = wav(&g1, &d1, FE);
        assert_eq!(octets.len(), 44 + g1.len() * 4);
        assert_eq!(&octets[..4], b"RIFF");
        assert_eq!(&octets[8..16], b"WAVEfmt ");
    }

    #[test]
    fn rendu_d_une_session_une_seule_fois_puis_la_queue() {
        use crate::transport::NoteSession;
        // tempo 120, une mesure (2 s) ; une note au premier temps ; 1 s de queue
        let session = Session {
            tempo: 120.0,
            longueur: 384,
            temps_par_mesure: 4,
            notes: vec![NoteSession { debut: 0, fin: 48, canal: 0, hauteur: 60, velocite: 100 }],
            boucler: true,
        };
        let (g, _) = rendre_session(&banque(), FE, &configuration(0, false), &session, 1.0).unwrap();
        assert_eq!(g.len(), 3 * FE as usize);
        let efficace = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        assert!(efficace(&g[..4410]) > 0.01, "la note au début");
        // pas de reprise de la boucle à 2 s : seulement la fin du son qui s'éteint
        let i = 2 * FE as usize;
        assert!(efficace(&g[i..i + 4410]) < efficace(&g[..4410]) * 0.2);
    }
}
