//! Rendu hors ligne (export) : un noyau neuf joue la liste d'événements aussi vite que possible,
//! puis le résultat est écrit en WAV 16 bits stéréo.

use std::sync::Arc;

use rustysynth::SoundFont;

use crate::noyau::{BLOC_MAX, Configuration, Evenement, Noyau};

/// Rend `duree` secondes ; les événements sont datés en échantillons à `frequence`.
pub fn rendre(
    banque: &Arc<SoundFont>,
    frequence: u32,
    configuration: &Configuration,
    evenements: &[Evenement],
    duree: f64,
) -> Result<(Vec<f32>, Vec<f32>), String> {
    let mut noyau = Noyau::new(banque, frequence)?;
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
        assert_eq!(&octets[..4], b"RIFF");
        assert_eq!(&octets[8..16], b"WAVEfmt ");
        assert_eq!(octets.len(), 44 + g1.len() * 4);
    }
}
