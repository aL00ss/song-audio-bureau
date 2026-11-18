//! Préréglages du synthé soustractif intégré, dans l'ordre du moteur (moteur/src/soustractif.rs),
//! et l'instrument General MIDI le plus proche (pour la version navigateur).

pub struct PrereglageSynthe {
    pub nom: &'static str,
    pub equivalent_gm: u8,
}

pub const PREREGLAGES_SOUSTRACTIF: [PrereglageSynthe; 6] = [
    PrereglageSynthe { nom: "Basse ronde", equivalent_gm: 38 },
    PrereglageSynthe { nom: "Lead acide", equivalent_gm: 81 },
    PrereglageSynthe { nom: "Nappe chaude", equivalent_gm: 89 },
    PrereglageSynthe { nom: "Cuivres synthé", equivalent_gm: 62 },
    PrereglageSynthe { nom: "Pluck", equivalent_gm: 84 },
    PrereglageSynthe { nom: "Sinus pur", equivalent_gm: 80 },
];
