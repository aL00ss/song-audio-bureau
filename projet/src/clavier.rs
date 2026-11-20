//! Géométrie d'un clavier de piano et correspondance avec le clavier de l'ordinateur. Fonctions
//! pures, reprises de la version web avec ses tests.
//!
//! Les touches de l'ordinateur sont repérées par leur position physique (nom de code comme
//! « KeyQ ») : la disposition est la même en AZERTY et en QWERTY, seules les lettres affichées
//! changent.

pub const NOTE_MIN: u8 = 21; // La0
pub const NOTE_MAX: u8 = 108; // Do8
pub const LARGEUR_NOIRE: f32 = 0.58;
pub const HAUTEUR_NOIRE: f32 = 0.63;

pub fn est_noire(midi: i32) -> bool {
    matches!(midi.rem_euclid(12), 1 | 3 | 6 | 8 | 10)
}

/// Décalage du centre de chaque touche noire par rapport à la jointure de ses voisines blanches.
fn decalage_noire(classe: i32) -> f32 {
    match classe {
        1 => -0.07,
        3 => 0.07,
        6 => -0.1,
        10 => 0.1,
        _ => 0.0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touche {
    pub midi: u8,
    pub noire: bool,
    /// Bord gauche, en largeurs de touche blanche depuis la première touche.
    pub x: f32,
    pub largeur: f32,
}

/// Touches de `debut` à `fin` (inclus).
pub fn disposition(debut: u8, fin: u8) -> Vec<Touche> {
    let mut touches = Vec::new();
    let mut blanches = 0.0f32;
    for midi in debut..=fin {
        if est_noire(midi as i32) {
            let centre = blanches + decalage_noire(midi as i32 % 12);
            touches.push(Touche { midi, noire: true, x: centre - LARGEUR_NOIRE / 2.0, largeur: LARGEUR_NOIRE });
        } else {
            touches.push(Touche { midi, noire: false, x: blanches, largeur: 1.0 });
            blanches += 1.0;
        }
    }
    touches
}

pub fn nombre_de_blanches(touches: &[Touche]) -> usize {
    touches.iter().filter(|t| !t.noire).count()
}

/// Touche sous un point (x en largeurs de blanche, y de 0 en haut à 1 en bas), ou None.
pub fn touche_sous(touches: &[Touche], x: f32, y: f32) -> Option<u8> {
    if !(0.0..=1.0).contains(&y) {
        return None;
    }
    if y <= HAUTEUR_NOIRE {
        if let Some(t) = touches.iter().find(|t| t.noire && x >= t.x && x < t.x + t.largeur) {
            return Some(t.midi);
        }
    }
    touches.iter().find(|t| !t.noire && x >= t.x && x < t.x + 1.0).map(|t| t.midi)
}

// ------------------------------------------------------- clavier de l'ordinateur

const RANGEE_BASSE: [&str; 17] =
    ["KeyZ", "KeyS", "KeyX", "KeyD", "KeyC", "KeyV", "KeyG", "KeyB", "KeyH", "KeyN", "KeyJ", "KeyM", "Comma", "KeyL", "Period", "Semicolon", "Slash"];
const RANGEE_HAUTE: [&str; 20] = [
    "KeyQ", "Digit2", "KeyW", "Digit3", "KeyE", "KeyR", "Digit5", "KeyT", "Digit6", "KeyY", "Digit7", "KeyU", "KeyI", "Digit9", "KeyO", "Digit0", "KeyP",
    "BracketLeft", "Equal", "BracketRight",
];

/// Demi-tons au-dessus du do de base pour une touche physique.
pub fn demi_tons(code: &str) -> Option<i32> {
    RANGEE_BASSE.iter().position(|c| *c == code).map(|i| i as i32).or_else(|| RANGEE_HAUTE.iter().position(|c| *c == code).map(|i| 12 + i as i32))
}

pub const BASE_DEFAUT: i32 = 48; // la rangée du haut commence au do central (60)
pub const BASE_MIN: i32 = 24;
pub const BASE_MAX: i32 = 84;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DispositionClavier {
    #[default]
    Azerty,
    Qwerty,
}

impl DispositionClavier {
    pub fn nom(self) -> &'static str {
        match self {
            DispositionClavier::Azerty => "AZERTY",
            DispositionClavier::Qwerty => "QWERTY",
        }
    }

    pub fn autre(self) -> Self {
        match self {
            DispositionClavier::Azerty => DispositionClavier::Qwerty,
            DispositionClavier::Qwerty => DispositionClavier::Azerty,
        }
    }
}

/// Caractère gravé sur une touche physique.
pub fn caractere(code: &str, clavier: DispositionClavier) -> String {
    let azerty = [
        ("KeyQ", "A"), ("KeyW", "Z"), ("KeyA", "Q"), ("KeyZ", "W"), ("KeyM", ","), ("Comma", ";"), ("Period", ":"), ("Slash", "!"),
        ("Semicolon", "M"), ("Digit2", "é"), ("Digit3", "\""), ("Digit5", "("), ("Digit6", "-"), ("Digit7", "è"), ("Digit9", "ç"),
        ("Digit0", "à"), ("BracketLeft", "^"), ("Equal", "="), ("BracketRight", "$"),
    ];
    let qwerty = [("Comma", ","), ("Period", "."), ("Slash", "/"), ("Semicolon", ";"), ("BracketLeft", "["), ("Equal", "="), ("BracketRight", "]")];
    let table: &[(&str, &str)] = match clavier {
        DispositionClavier::Azerty => &azerty,
        DispositionClavier::Qwerty => &qwerty,
    };
    if let Some((_, c)) = table.iter().find(|(k, _)| *k == code) {
        return c.to_string();
    }
    code.strip_prefix("Key").or_else(|| code.strip_prefix("Digit")).unwrap_or("").to_string()
}

/// Note jouée par une touche physique, ou None.
pub fn note_du_code(code: &str, base: i32) -> Option<u8> {
    let midi = base + demi_tons(code)?;
    (0..=127).contains(&midi).then_some(midi as u8)
}

/// Note -> caractère à afficher sur la touche du piano.
pub fn etiquettes(base: i32, clavier: DispositionClavier) -> std::collections::BTreeMap<u8, String> {
    RANGEE_BASSE.iter().chain(RANGEE_HAUTE.iter()).filter_map(|code| Some((note_du_code(code, base)?, caractere(code, clavier)))).collect()
}

pub fn decaler_base(base: i32, octaves: i32) -> i32 {
    (base + 12 * octaves).clamp(BASE_MIN, BASE_MAX)
}

/// Disposition devinée d'après une touche : la touche physique « KeyQ » qui écrit « a » est un
/// clavier AZERTY.
pub fn disposition_d_apres(code: &str, texte: &str) -> Option<DispositionClavier> {
    match (code, texte.to_lowercase().as_str()) {
        ("KeyQ", "a") | ("KeyA", "q") | ("KeyW", "z") | ("KeyZ", "w") => Some(DispositionClavier::Azerty),
        ("KeyQ", "q") | ("KeyA", "a") | ("KeyW", "w") | ("KeyZ", "z") => Some(DispositionClavier::Qwerty),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- les tests de la version web (edition.test.ts, « clavier »), traduits

    #[test]
    fn touches_88_dont_52_blanches_de_la0_a_do8() {
        let t = disposition(NOTE_MIN, NOTE_MAX);
        assert_eq!(t.len(), 88);
        assert_eq!(nombre_de_blanches(&t), 52);
        assert_eq!((t[0].midi, t[87].midi), (21, 108));
        assert!(est_noire(61));
        assert!(!est_noire(60));
    }

    #[test]
    fn touche_sous_le_pointeur_noire_en_haut_blanche_en_bas() {
        let t = disposition(48, 72);
        let do_diese = t.iter().find(|x| x.midi == 49).unwrap();
        assert_eq!(touche_sous(&t, do_diese.x + do_diese.largeur / 2.0, 0.3), Some(49));
        assert_eq!(touche_sous(&t, do_diese.x + do_diese.largeur / 2.0, 0.9), Some(48));
        assert_eq!(touche_sous(&t, 0.5, 0.9), Some(48));
        assert_eq!(touche_sous(&t, -1.0, 0.5), None);
        assert_eq!(touche_sous(&t, 0.5, 1.5), None);
    }

    #[test]
    fn clavier_de_l_ordinateur_rangee_du_haut_au_do_central() {
        assert_eq!(note_du_code("KeyQ", BASE_DEFAUT), Some(60));
        assert_eq!(note_du_code("KeyZ", BASE_DEFAUT), Some(48));
        assert_eq!(note_du_code("Digit2", BASE_DEFAUT), Some(61));
        assert_eq!(note_du_code("Space", BASE_DEFAUT), None);
        assert_eq!(caractere("KeyQ", DispositionClavier::Azerty), "A");
        assert_eq!(caractere("KeyQ", DispositionClavier::Qwerty), "Q");
        assert_eq!(etiquettes(BASE_DEFAUT, DispositionClavier::Azerty)[&60], "A");
        assert_eq!(decaler_base(BASE_DEFAUT, 10), 84);
        assert_eq!(decaler_base(BASE_DEFAUT, -1), 36);
    }

    #[test]
    fn disposition_devinee_d_apres_une_frappe() {
        assert_eq!(disposition_d_apres("KeyQ", "a"), Some(DispositionClavier::Azerty));
        assert_eq!(disposition_d_apres("KeyQ", "Q"), Some(DispositionClavier::Qwerty));
        assert_eq!(disposition_d_apres("KeyE", "e"), None);
    }
}
