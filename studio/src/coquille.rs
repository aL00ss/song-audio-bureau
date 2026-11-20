//! La coquille du studio : panneaux ancrables (egui_dock), disposition par défaut, raccourcis F5
//! à F9 pour ouvrir ou fermer un panneau. La disposition est gardée d'une session à l'autre.

use eframe::egui::{self, WidgetText};
use egui_dock::{DockState, NodeIndex, TabViewer};
use serde::{Deserialize, Serialize};

use crate::app::Studio;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Panneau {
    Navigateur,
    Rack,
    Arrangement,
    PianoRoll,
    Instrument,
    Mixage,
}

impl Panneau {
    pub const TOUS: [Panneau; 6] = [Panneau::Navigateur, Panneau::Arrangement, Panneau::Rack, Panneau::PianoRoll, Panneau::Instrument, Panneau::Mixage];

    pub fn titre(self) -> &'static str {
        match self {
            Panneau::Navigateur => "Navigateur",
            Panneau::Rack => "Rack d'instruments",
            Panneau::Arrangement => "Arrangement",
            Panneau::PianoRoll => "Piano roll",
            Panneau::Instrument => "Instrument",
            Panneau::Mixage => "Mixage",
        }
    }

    pub fn touche(self) -> Option<egui::Key> {
        match self {
            Panneau::Arrangement => Some(egui::Key::F5),
            Panneau::Rack => Some(egui::Key::F6),
            Panneau::PianoRoll => Some(egui::Key::F7),
            Panneau::Navigateur => Some(egui::Key::F8),
            Panneau::Mixage => Some(egui::Key::F9),
            Panneau::Instrument => None,
        }
    }
}

/// Navigateur à gauche ; arrangement en haut ; rack et piano roll côte à côte ; instrument et
/// mixage en onglets en bas.
pub fn disposition_par_defaut() -> DockState<Panneau> {
    let mut dock = DockState::new(vec![Panneau::Navigateur]);
    let surface = dock.main_surface_mut();
    let [_, droite] = surface.split_right(NodeIndex::root(), 0.17, vec![Panneau::Arrangement]);
    let [_, milieu] = surface.split_below(droite, 0.22, vec![Panneau::Rack]);
    let [rack, _] = surface.split_right(milieu, 0.58, vec![Panneau::PianoRoll]);
    surface.split_below(rack, 0.62, vec![Panneau::Instrument, Panneau::Mixage]);
    dock
}

/// Ouvre un panneau fermé ; ferme un panneau ouvert au premier plan ; amène au premier plan un
/// panneau ouvert caché derrière un autre onglet.
pub fn basculer(dock: &mut DockState<Panneau>, panneau: Panneau) {
    match dock.find_tab(&panneau) {
        None => dock.push_to_focused_leaf(panneau),
        Some(chemin) => {
            let au_premier_plan = dock
                .iter_leaves()
                .any(|(_, feuille)| feuille.tabs.get(feuille.active.0) == Some(&panneau));
            if au_premier_plan {
                dock.remove_tab(chemin);
            } else {
                let _ = dock.set_active_tab(chemin);
            }
        }
    }
}

/// Ouvre le panneau s'il est fermé et l'amène au premier plan (sans jamais le fermer).
pub fn montrer(dock: &mut DockState<Panneau>, panneau: Panneau) {
    match dock.find_tab(&panneau) {
        None => dock.push_to_focused_leaf(panneau),
        Some(chemin) => {
            let _ = dock.set_active_tab(chemin);
        }
    }
}

pub fn ouverts(dock: &DockState<Panneau>) -> Vec<Panneau> {
    dock.iter_all_tabs().map(|(_, p)| *p).collect()
}

/// Ce que chaque onglet affiche.
pub struct Vue<'a> {
    pub studio: &'a mut Studio,
}

impl TabViewer for Vue<'_> {
    type Tab = Panneau;

    fn id(&mut self, tab: &mut Panneau) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Panneau) -> WidgetText {
        tab.titre().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Panneau) {
        match tab {
            Panneau::Navigateur => self.studio.navigateur(ui),
            Panneau::Rack => self.studio.rack(ui),
            Panneau::PianoRoll => self.studio.piano_roll(ui),
            Panneau::Instrument => self.studio.vue_instrument(ui),
            Panneau::Mixage => self.studio.mixeur(ui),
            Panneau::Arrangement => self.studio.arrangement(ui),
        }
    }

    fn closeable(&mut self, _tab: &mut Panneau) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disposition_par_defaut_complete() {
        let dock = disposition_par_defaut();
        let mut tous = ouverts(&dock);
        tous.sort_by_key(|p| Panneau::TOUS.iter().position(|q| q == p));
        assert_eq!(tous, Panneau::TOUS.to_vec());
    }

    #[test]
    fn montrer_ouvre_et_ne_ferme_jamais() {
        let mut dock = disposition_par_defaut();
        montrer(&mut dock, Panneau::PianoRoll);
        assert!(ouverts(&dock).contains(&Panneau::PianoRoll), "déjà ouvert : reste ouvert");
        basculer(&mut dock, Panneau::PianoRoll);
        assert!(!ouverts(&dock).contains(&Panneau::PianoRoll));
        montrer(&mut dock, Panneau::PianoRoll);
        assert!(ouverts(&dock).contains(&Panneau::PianoRoll));
    }

    #[test]
    fn basculer_ferme_puis_rouvre() {
        let mut dock = disposition_par_defaut();
        basculer(&mut dock, Panneau::Rack);
        assert!(!ouverts(&dock).contains(&Panneau::Rack));
        basculer(&mut dock, Panneau::Rack);
        assert!(ouverts(&dock).contains(&Panneau::Rack));
        // le mixage, caché derrière l'onglet Instrument : il passe au premier plan, sans se fermer
        basculer(&mut dock, Panneau::Mixage);
        assert!(ouverts(&dock).contains(&Panneau::Mixage));
    }

    /// Stockage en mémoire, comme celui d'eframe (même sérialisation).
    #[derive(Default)]
    struct Memoire(std::collections::HashMap<String, String>);

    impl eframe::Storage for Memoire {
        fn get_string(&self, cle: &str) -> Option<String> {
            self.0.get(cle).cloned()
        }
        fn set_string(&mut self, cle: &str, valeur: String) {
            self.0.insert(cle.to_string(), valeur);
        }
        fn remove_string(&mut self, cle: &str) {
            self.0.remove(cle);
        }
        fn flush(&mut self) {}
    }

    #[test]
    fn la_disposition_se_garde() {
        let mut dock = disposition_par_defaut();
        basculer(&mut dock, Panneau::Mixage);
        basculer(&mut dock, Panneau::Mixage); // fermé
        let mut memoire = Memoire::default();
        eframe::set_value(&mut memoire, "disposition", &dock);
        let relu: DockState<Panneau> = eframe::get_value(&memoire, "disposition").expect("disposition relue");
        assert_eq!(ouverts(&relu), ouverts(&dock));
        assert!(!ouverts(&relu).contains(&Panneau::Mixage));
    }
}
