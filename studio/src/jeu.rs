//! Jeu en direct : notes jouées à la souris (piano roll, vue d'instrument) ou au clavier de
//! l'ordinateur, envoyées tout de suite au moteur, et enregistrées dans le pattern pendant la
//! lecture si l'enregistrement est armé.
//!
//! Les touches de l'ordinateur sont lues par leur position physique : la rangée du bas et la
//! rangée du haut forment deux octaves de piano, en AZERTY comme en QWERTY.

use std::collections::{BTreeSet, HashMap};

use eframe::egui::{self, Key};
use moteur::{EtatTransport, Evenement, Message};
use projet::clavier::{BASE_DEFAUT, DispositionClavier, decaler_base, note_du_code};
use projet::lecture::ModeLecture;
use projet::temps::TICS_PAR_NOIRE;

use crate::app::Studio;

/// Une note tenue : combien d'appuis la tiennent (souris et clavier peuvent tenir la même),
/// d'où elle part (position absolue pendant la lecture) et sa vélocité.
#[derive(Clone, Debug, PartialEq)]
struct Tenue {
    appuis: u32,
    debut: Option<f64>,
    velocite: u8,
}

/// État du jeu en direct.
#[derive(Debug)]
pub struct Jeu {
    /// (canal, hauteur) -> note tenue.
    tenues: HashMap<(String, u8), Tenue>,
    /// Touche physique enfoncée -> note et canal joués (pour les relâcher).
    touches: HashMap<&'static str, (u8, String)>,
    /// Do de la rangée du bas (la rangée du haut commence une octave plus haut).
    pub base: i32,
    pub disposition: DispositionClavier,
    /// Vrai une fois la disposition devinée d'après une frappe (on ne la devine qu'une fois).
    disposition_vue: bool,
    /// Enregistrement armé : les notes jouées pendant la lecture s'inscrivent dans le pattern.
    pub enregistrement: bool,
}

impl Default for Jeu {
    fn default() -> Self {
        Jeu { tenues: HashMap::new(), touches: HashMap::new(), base: BASE_DEFAUT, disposition: DispositionClavier::Azerty, disposition_vue: false, enregistrement: false }
    }
}

/// Nom de code (comme « KeyQ ») d'une touche physique qui joue une note.
pub fn code_de(touche: Key) -> Option<&'static str> {
    Some(match touche {
        Key::Z => "KeyZ",
        Key::S => "KeyS",
        Key::X => "KeyX",
        Key::D => "KeyD",
        Key::C => "KeyC",
        Key::V => "KeyV",
        Key::G => "KeyG",
        Key::B => "KeyB",
        Key::H => "KeyH",
        Key::N => "KeyN",
        Key::J => "KeyJ",
        Key::M => "KeyM",
        Key::Comma => "Comma",
        Key::L => "KeyL",
        Key::Period => "Period",
        Key::Semicolon => "Semicolon",
        Key::Slash => "Slash",
        Key::Q => "KeyQ",
        Key::Num2 => "Digit2",
        Key::W => "KeyW",
        Key::Num3 => "Digit3",
        Key::E => "KeyE",
        Key::R => "KeyR",
        Key::Num5 => "Digit5",
        Key::T => "KeyT",
        Key::Num6 => "Digit6",
        Key::Y => "KeyY",
        Key::Num7 => "Digit7",
        Key::U => "KeyU",
        Key::I => "KeyI",
        Key::Num9 => "Digit9",
        Key::O => "KeyO",
        Key::Num0 => "Digit0",
        Key::P => "KeyP",
        Key::OpenBracket => "BracketLeft",
        Key::Equals => "Equal",
        Key::CloseBracket => "BracketRight",
        _ => return None,
    })
}

/// Disposition devinée d'après une frappe : la touche physique du Q de QWERTY qui écrit « A »
/// est un clavier AZERTY.
fn disposition_d_apres(physique: Key, logique: Key) -> Option<DispositionClavier> {
    match (physique, logique) {
        (Key::Q, Key::A) | (Key::A, Key::Q) | (Key::W, Key::Z) | (Key::Z, Key::W) => Some(DispositionClavier::Azerty),
        (Key::Q, Key::Q) | (Key::A, Key::A) | (Key::W, Key::W) | (Key::Z, Key::Z) => Some(DispositionClavier::Qwerty),
        _ => None,
    }
}

impl Studio {
    /// Position depuis le début de la lecture (boucles comprises), en tics, prolongée depuis le
    /// dernier bloc rendu.
    pub fn position_absolue(&self) -> f64 {
        let Some(m) = self.moteur() else { return 0.0 };
        let l = m.lecture();
        if l.etat != EtatTransport::Lecture {
            return l.position_absolue;
        }
        let ecoule = m.horloge().saturating_sub(l.horloge) as f64 / m.frequence() as f64;
        l.position_absolue + ecoule * l.tempo * TICS_PAR_NOIRE as f64 / 60.0
    }

    fn rang(&self, canal: &str) -> Option<u32> {
        self.projet.canaux.iter().position(|c| c.id == canal).map(|r| r as u32)
    }

    fn envoyer_note(&mut self, canal: &str, message: Message, delai_secondes: f64) {
        self.synchroniser(); // un canal tout juste ajouté doit exister dans le moteur
        // une couche n'a pas de son : la note part vers chacun de ses enfants
        let rangs: Vec<u32> = match self.projet.canal(canal) {
            Some(c) if c.source == projet::SourceCanal::Couche => c.enfants.iter().filter_map(|e| self.rang(e)).collect(),
            Some(_) => self.rang(canal).into_iter().collect(),
            None => Vec::new(),
        };
        if let Some(m) = self.moteur_mut() {
            let echantillon = m.horloge() + (delai_secondes * m.frequence() as f64) as u64;
            for rang in rangs {
                m.programmer(Evenement { echantillon, canal: rang, message });
            }
        }
    }

    /// Joue une note sur un canal, jusqu'à `relacher_note`.
    pub fn jouer_note(&mut self, hauteur: u8, velocite: u8, canal: &str) {
        if hauteur > 127 || self.rang(canal).is_none() {
            return;
        }
        let cle = (canal.to_string(), hauteur);
        if let Some(tenue) = self.jeu.tenues.get_mut(&cle) {
            tenue.appuis += 1;
            return;
        }
        let velocite = velocite.clamp(1, 127);
        self.envoyer_note(canal, Message::NoteOn { hauteur, velocite }, 0.0);
        let debut = (self.etat_transport() == EtatTransport::Lecture).then(|| self.position_absolue());
        self.jeu.tenues.insert(cle, Tenue { appuis: 1, debut, velocite });
    }

    /// Relâche une note ; si l'enregistrement est armé et que la note a commencé pendant la
    /// lecture, elle s'inscrit dans le pattern actif, à sa place dans la boucle.
    pub fn relacher_note(&mut self, hauteur: u8, canal: &str) {
        let cle = (canal.to_string(), hauteur);
        let Some(tenue) = self.jeu.tenues.get_mut(&cle) else { return };
        tenue.appuis -= 1;
        if tenue.appuis > 0 {
            return;
        }
        let tenue = self.jeu.tenues.remove(&cle).expect("tenue présente");
        self.envoyer_note(canal, Message::NoteOff { hauteur }, 0.0);
        if let (true, Some(debut), EtatTransport::Lecture) = (self.jeu.enregistrement, tenue.debut, self.etat_transport()) {
            let fin = self.position_absolue();
            let boucle = self.moteur().map(|m| m.lecture().longueur).unwrap_or(0).max(1) as f64;
            // en mode morceau, la note va dans le pattern actif, ramenée à sa longueur
            let longueur = self.projet.pattern_actif().longueur.max(1);
            let dans_la_boucle = if self.mode == ModeLecture::Pattern { debut % boucle } else { (debut % boucle) % longueur as f64 };
            let tic = (dans_la_boucle.floor() as u32).min(longueur - 1);
            let duree = ((fin - debut).round() as i64).clamp(1, (longueur - tic) as i64) as u32;
            let id = canal.to_string();
            self.modifier(|p| {
                if p.canal(&id).is_some() {
                    p.pattern_actif_mut().notes.entry(id).or_default().push(projet::NoteMusicale { tic, duree, hauteur, velocite: tenue.velocite, ..Default::default() });
                }
            });
        }
    }

    /// Fait entendre une note brièvement (clic sur une note, une touche du piano roll).
    pub fn ecouter(&mut self, hauteur: u8, velocite: u8, canal: &str) {
        if self.jeu.tenues.contains_key(&(canal.to_string(), hauteur)) {
            return; // déjà tenue : on ne la coupe pas
        }
        self.envoyer_note(canal, Message::NoteOn { hauteur, velocite: velocite.clamp(1, 127) }, 0.0);
        self.envoyer_note(canal, Message::NoteOff { hauteur }, 0.22);
    }

    /// Relâche toutes les notes tenues (fenêtre quittée touche enfoncée, par exemple).
    pub fn relacher_tout(&mut self) {
        self.jeu.touches.clear();
        let cles: Vec<(String, u8)> = self.jeu.tenues.keys().cloned().collect();
        for (canal, hauteur) in cles {
            if let Some(t) = self.jeu.tenues.get_mut(&(canal.clone(), hauteur)) {
                t.appuis = 1;
            }
            self.relacher_note(hauteur, &canal);
        }
    }

    /// Hauteurs tenues en direct sur un canal.
    pub fn notes_en_direct(&self, canal: &str) -> BTreeSet<u8> {
        self.jeu.tenues.keys().filter(|(c, _)| c == canal).map(|(_, h)| *h).collect()
    }

    pub fn changer_octave(&mut self, sens: i32) {
        self.jeu.base = decaler_base(self.jeu.base, sens);
    }

    pub fn basculer_enregistrement(&mut self) {
        self.jeu.enregistrement = !self.jeu.enregistrement;
    }

    /// Le clavier de l'ordinateur joue le canal choisi (sauf pendant une saisie de texte ou avec
    /// Ctrl, Alt ou Maj) ; ← → changent d'octave quand le piano roll n'a pas le focus.
    pub fn lire_clavier(&mut self, ctx: &egui::Context, fleches_libres: bool) {
        let saisie = ctx.text_edit_focused();
        let (evenements, focus_perdu) = ctx.input(|i| {
            let focus_perdu = i.events.iter().any(|e| matches!(e, egui::Event::WindowFocused(false)));
            (i.events.clone(), focus_perdu)
        });
        if focus_perdu {
            self.relacher_tout();
            return;
        }
        for evenement in evenements {
            let egui::Event::Key { key, physical_key, pressed, repeat, modifiers } = evenement else { continue };
            let physique = physical_key.unwrap_or(key);
            if !pressed {
                // un relâchement passe toujours, même si une saisie a commencé entre-temps
                if let Some(code) = code_de(physique) {
                    if let Some((hauteur, canal)) = self.jeu.touches.remove(code) {
                        self.relacher_note(hauteur, &canal);
                    }
                }
                continue;
            }
            if saisie || repeat || modifiers.ctrl || modifiers.command || modifiers.alt {
                continue;
            }
            if !self.jeu.disposition_vue {
                if let Some(d) = disposition_d_apres(physique, key) {
                    self.jeu.disposition = d;
                    self.jeu.disposition_vue = true;
                }
            }
            if fleches_libres && (key == Key::ArrowLeft || key == Key::ArrowRight) {
                self.changer_octave(if key == Key::ArrowLeft { -1 } else { 1 });
                continue;
            }
            if modifiers.shift {
                continue;
            }
            let (Some(code), Some(canal)) = (code_de(physique), self.canal_selectionne.clone()) else { continue };
            if self.jeu.touches.contains_key(code) {
                continue;
            }
            if let Some(hauteur) = note_du_code(code, self.jeu.base) {
                self.jeu.touches.insert(code, (hauteur, canal.clone()));
                self.jouer_note(hauteur, 96, &canal);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toutes_les_touches_de_note_ont_un_code() {
        let touches = [
            Key::Z, Key::S, Key::X, Key::D, Key::C, Key::V, Key::G, Key::B, Key::H, Key::N, Key::J, Key::M, Key::Comma, Key::L, Key::Period,
            Key::Semicolon, Key::Slash, Key::Q, Key::Num2, Key::W, Key::Num3, Key::E, Key::R, Key::Num5, Key::T, Key::Num6, Key::Y, Key::Num7,
            Key::U, Key::I, Key::Num9, Key::O, Key::Num0, Key::P, Key::OpenBracket, Key::Equals, Key::CloseBracket,
        ];
        let notes: BTreeSet<u8> = touches.iter().map(|t| note_du_code(code_de(*t).unwrap(), BASE_DEFAUT).unwrap()).collect();
        // les deux rangées se chevauchent sur une quarte (do à mi), comme dans la version web
        assert_eq!(notes.len(), 32);
        assert_eq!((notes.first(), notes.last()), (Some(&48), Some(&79)));
        assert_eq!(code_de(Key::Space), None);
        assert_eq!(disposition_d_apres(Key::Q, Key::A), Some(DispositionClavier::Azerty));
        assert_eq!(disposition_d_apres(Key::Q, Key::Q), Some(DispositionClavier::Qwerty));
    }
}
