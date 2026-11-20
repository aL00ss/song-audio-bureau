//! Instruments d'échantillons (fichiers SFZ) : chargés dans un fil à part, partagés entre les
//! canaux qui jouent le même fichier, et donnés au moteur canal par canal (le numéro d'un canal
//! dans le moteur est son rang dans le projet : un déplacement de canal est suivi).

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use moteur::echantillonneur::InstrumentEchantillons;
use projet::modele::OptionsCanal;
use projet::{SourceCanal};

use crate::app::Studio;

type Chargement = (String, Result<(Arc<InstrumentEchantillons>, Vec<String>), String>);

#[derive(Debug)]
pub struct EtatInstruments {
    /// Instruments chargés, par chemin de fichier SFZ.
    charges: HashMap<String, Arc<InstrumentEchantillons>>,
    en_cours: HashSet<String>,
    /// Fichiers illisibles (on ne réessaie pas à chaque image).
    erreurs: HashMap<String, String>,
    envoi: Sender<Chargement>,
    reception: Receiver<Chargement>,
    /// Instrument que le moteur a pour chaque rang de canal.
    envoyes: Vec<Option<String>>,
}

impl Default for EtatInstruments {
    fn default() -> Self {
        let (envoi, reception) = channel();
        EtatInstruments { charges: HashMap::new(), en_cours: HashSet::new(), erreurs: HashMap::new(), envoi, reception, envoyes: Vec::new() }
    }
}

impl EtatInstruments {
    pub fn charge(&self, chemin: &str) -> Option<&Arc<InstrumentEchantillons>> {
        self.charges.get(chemin)
    }

    pub fn en_cours(&self) -> bool {
        !self.en_cours.is_empty()
    }

    pub fn erreur(&self, chemin: &str) -> Option<&str> {
        self.erreurs.get(chemin).map(String::as_str)
    }
}

impl Studio {
    /// Ajoute un canal échantillonneur qui joue ce fichier SFZ.
    pub fn ajouter_echantillonneur(&mut self, chemin: &Path) {
        let nom = chemin.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Échantillonneur".into());
        self.ajouter_canal(OptionsCanal {
            source: SourceCanal::Echantillons,
            instrument: Some(chemin.to_string_lossy().to_string()),
            nom: Some(nom),
            ..Default::default()
        });
    }

    /// Lance le chargement des instruments demandés par le projet, relève ceux qui sont prêts et
    /// donne au moteur ceux qui lui manquent (appelé à chaque image, après la synchronisation).
    pub fn suivre_instruments(&mut self, ctx: Option<&eframe::egui::Context>) {
        // chargements finis
        while let Ok((chemin, resultat)) = self.instruments.reception.try_recv() {
            self.instruments.en_cours.remove(&chemin);
            match resultat {
                Ok((instrument, avertissements)) => {
                    let nom = instrument.nom.clone();
                    let zones = instrument.zones.len();
                    self.instruments.charges.insert(chemin, instrument);
                    self.message = Some(if avertissements.is_empty() {
                        format!("Instrument « {nom} » chargé ({zones} zones).")
                    } else {
                        format!("Instrument « {nom} » chargé ({zones} zones) ; {} échantillons illisibles.", avertissements.len())
                    });
                }
                Err(e) => {
                    self.message = Some(format!("Instrument illisible : {e}"));
                    self.instruments.erreurs.insert(chemin, e);
                }
            }
        }
        // chargements à lancer
        let demandes: Vec<String> = self
            .projet
            .canaux
            .iter()
            .filter(|c| c.source == SourceCanal::Echantillons)
            .filter_map(|c| c.instrument.clone())
            .filter(|c| !self.instruments.charges.contains_key(c) && !self.instruments.en_cours.contains(c) && !self.instruments.erreurs.contains_key(c))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        for chemin in demandes {
            self.instruments.en_cours.insert(chemin.clone());
            let envoi = self.instruments.envoi.clone();
            let contexte = ctx.cloned();
            std::thread::spawn(move || {
                let resultat = InstrumentEchantillons::depuis_sfz(Path::new(&chemin)).map(|(i, a)| (Arc::new(i), a));
                let _ = envoi.send((chemin, resultat));
                if let Some(c) = contexte {
                    c.request_repaint();
                }
            });
        }
        // le moteur : l'instrument de chaque rang
        let voulus: Vec<Option<String>> = self
            .projet
            .canaux
            .iter()
            .map(|c| c.instrument.clone().filter(|i| c.source == SourceCanal::Echantillons && self.instruments.charges.contains_key(i)))
            .collect();
        let rangs = voulus.len().max(self.instruments.envoyes.len());
        self.instruments.envoyes.resize(rangs, None);
        for rang in 0..rangs {
            let voulu = voulus.get(rang).cloned().flatten();
            if self.instruments.envoyes[rang] == voulu {
                continue;
            }
            let instrument = voulu.as_ref().and_then(|c| self.instruments.charges.get(c)).cloned();
            if let Some(m) = self.moteur_mut() {
                m.changer_instrument(rang as u32, instrument);
            }
            self.instruments.envoyes[rang] = voulu;
        }
        self.instruments.envoyes.truncate(voulus.len());
    }

    /// Les instruments d'échantillons du projet, par rang de canal (pour le rendu hors ligne).
    pub fn instruments_pour_rendu(&self) -> Vec<(u32, Arc<InstrumentEchantillons>)> {
        self.projet
            .canaux
            .iter()
            .enumerate()
            .filter(|(_, c)| c.source == SourceCanal::Echantillons)
            .filter_map(|(r, c)| c.instrument.as_ref().and_then(|i| self.instruments.charges.get(i)).map(|i| (r as u32, i.clone())))
            .collect()
    }

    /// Le moteur a été remplacé (démarrage) : il faudra tout lui redonner.
    pub(crate) fn instruments_a_renvoyer(&mut self) {
        self.instruments.envoyes.clear();
    }
}
