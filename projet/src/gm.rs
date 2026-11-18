//! General MIDI : les 128 instruments, leurs familles, et les sons de batterie du canal 10.
//! Mêmes noms que la version web.

pub const CANAL_BATTERIE: u8 = 9; // canal 10 en numérotation musicale

const NOMS: [&str; 128] = [
    // Pianos
    "Piano à queue", "Piano droit brillant", "Piano électrique à queue", "Piano bastringue",
    "Piano électrique 1", "Piano électrique 2", "Clavecin", "Clavinet",
    // Percussions chromatiques
    "Célesta", "Glockenspiel", "Boîte à musique", "Vibraphone",
    "Marimba", "Xylophone", "Cloches tubulaires", "Dulcimer",
    // Orgues
    "Orgue à tirettes", "Orgue percussif", "Orgue rock", "Orgue d'église",
    "Harmonium", "Accordéon", "Harmonica", "Bandonéon",
    // Guitares
    "Guitare classique", "Guitare folk", "Guitare jazz", "Guitare électrique claire",
    "Guitare électrique étouffée", "Guitare saturée", "Guitare distorsion", "Harmoniques de guitare",
    // Basses
    "Contrebasse jazz", "Basse électrique (doigts)", "Basse électrique (médiator)", "Basse fretless",
    "Basse slap 1", "Basse slap 2", "Basse synthé 1", "Basse synthé 2",
    // Cordes
    "Violon", "Alto", "Violoncelle", "Contrebasse",
    "Cordes trémolo", "Cordes pizzicato", "Harpe", "Timbales",
    // Ensembles
    "Ensemble de cordes 1", "Ensemble de cordes 2", "Cordes synthé 1", "Cordes synthé 2",
    "Chœur « aah »", "Voix « ooh »", "Voix synthé", "Coup d'orchestre",
    // Cuivres
    "Trompette", "Trombone", "Tuba", "Trompette bouchée",
    "Cor", "Section de cuivres", "Cuivres synthé 1", "Cuivres synthé 2",
    // Anches
    "Saxophone soprano", "Saxophone alto", "Saxophone ténor", "Saxophone baryton",
    "Hautbois", "Cor anglais", "Basson", "Clarinette",
    // Flûtes
    "Piccolo", "Flûte", "Flûte à bec", "Flûte de Pan",
    "Bouteille soufflée", "Shakuhachi", "Sifflement", "Ocarina",
    // Synthés solistes
    "Synthé carré", "Synthé dent de scie", "Synthé calliope", "Synthé chiff",
    "Synthé charang", "Synthé voix", "Synthé quintes", "Synthé basse et solo",
    // Nappes
    "Nappe new age", "Nappe chaude", "Nappe polysynth", "Nappe chœur",
    "Nappe archet", "Nappe métallique", "Nappe halo", "Nappe balayage",
    // Effets synthé
    "Effet pluie", "Effet bande originale", "Effet cristal", "Effet atmosphère",
    "Effet brillance", "Effet gobelins", "Effet échos", "Effet science-fiction",
    // Instruments du monde
    "Sitar", "Banjo", "Shamisen", "Koto",
    "Kalimba", "Cornemuse", "Violon folk", "Shehnai",
    // Percussions
    "Clochette", "Agogo", "Steel drum", "Woodblock",
    "Taiko", "Tom mélodique", "Batterie synthé", "Cymbale inversée",
    // Bruitages
    "Bruit de frette", "Respiration", "Vagues", "Chant d’oiseau",
    "Sonnerie de téléphone", "Hélicoptère", "Applaudissements", "Coup de feu",
];

pub const FAMILLES: [&str; 16] = [
    "Pianos", "Percussions chromatiques", "Orgues", "Guitares", "Basses", "Cordes", "Ensembles",
    "Cuivres", "Anches", "Flûtes", "Synthés solistes", "Nappes", "Effets synthé", "Monde",
    "Percussions", "Bruitages",
];

pub fn nom_instrument(programme: u8) -> &'static str {
    NOMS[programme as usize & 127]
}

pub fn famille_instrument(programme: u8) -> &'static str {
    FAMILLES[(programme as usize & 127) / 8]
}

/// Sons de batterie General MIDI (notes 35 à 81).
const BATTERIE: [&str; 47] = [
    "Grosse caisse acoustique", "Grosse caisse", "Cerclage", "Caisse claire",
    "Clap", "Caisse claire électrique", "Tom basse grave", "Charleston fermé",
    "Tom basse aigu", "Charleston pédale", "Tom grave", "Charleston ouvert",
    "Tom médium grave", "Tom médium aigu", "Crash 1", "Tom aigu",
    "Ride 1", "Cymbale chinoise", "Cloche de ride", "Tambourin",
    "Splash", "Cloche à vache", "Crash 2", "Vibraslap",
    "Ride 2", "Bongo aigu", "Bongo grave", "Conga aiguë étouffée",
    "Conga aiguë ouverte", "Conga grave", "Timbale aiguë", "Timbale grave",
    "Agogo aigu", "Agogo grave", "Cabasa", "Maracas",
    "Sifflet court", "Sifflet long", "Guiro court", "Guiro long",
    "Claves", "Woodblock aigu", "Woodblock grave", "Cuica étouffée",
    "Cuica ouverte", "Triangle étouffé", "Triangle ouvert",
];

pub fn nom_son_batterie(note: u8) -> String {
    match note {
        35..=81 => BATTERIE[(note - 35) as usize].to_string(),
        _ => format!("Percussion {note}"),
    }
}

const NOMS_NOTES: [&str; 12] = ["Do", "Do♯", "Ré", "Ré♯", "Mi", "Fa", "Fa♯", "Sol", "Sol♯", "La", "La♯", "Si"];

/// Nom français d'une note, octave française (La3 = 440 Hz).
pub fn nom_note(midi: u8) -> String {
    format!("{}{}", NOMS_NOTES[midi as usize % 12], midi as i32 / 12 - 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms() {
        assert_eq!(nom_instrument(0), "Piano à queue");
        assert_eq!(nom_instrument(127), "Coup de feu");
        assert_eq!(famille_instrument(40), "Cordes");
        assert_eq!(nom_son_batterie(36), "Grosse caisse");
        assert_eq!(nom_son_batterie(81), "Triangle ouvert");
        assert_eq!(nom_son_batterie(20), "Percussion 20");
        assert_eq!(nom_note(69), "La3");
        assert_eq!(nom_note(60), "Do3");
    }
}
