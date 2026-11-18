//! Temps musical : le projet compte en tics (96 par noire), le transport convertit en secondes.

use serde::{Deserialize, Serialize};

pub const TICS_PAR_NOIRE: u32 = 96;
/// Un pas du séquenceur : une double croche.
pub const TICS_PAR_PAS: u32 = TICS_PAR_NOIRE / 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub numerateur: u32,
    pub denominateur: u32,
}

pub const SIGNATURE_DEFAUT: Signature = Signature { numerateur: 4, denominateur: 4 };

impl Default for Signature {
    fn default() -> Self {
        SIGNATURE_DEFAUT
    }
}

pub fn tics_en_secondes(tics: f64, tempo: f64) -> f64 {
    (tics / TICS_PAR_NOIRE as f64) * (60.0 / tempo)
}

pub fn secondes_en_tics(secondes: f64, tempo: f64) -> f64 {
    secondes * tempo * TICS_PAR_NOIRE as f64 / 60.0
}

pub fn tics_par_temps(signature: Signature) -> u32 {
    TICS_PAR_NOIRE * 4 / signature.denominateur.max(1)
}

pub fn tics_par_mesure(signature: Signature) -> u32 {
    tics_par_temps(signature) * signature.numerateur.max(1)
}

/// Arrondit une durée en tics à la mesure entière supérieure (au moins une mesure).
pub fn arrondir_a_la_mesure(tics: u32, signature: Signature) -> u32 {
    let mesure = tics_par_mesure(signature);
    tics.div_ceil(mesure).max(1) * mesure
}

/// Position lisible « mesure:temps:tic », comptée à partir de 1:1:00.
pub fn formater_position(tics: f64, signature: Signature) -> String {
    let t = tics.max(0.0).floor() as u32;
    let par_temps = tics_par_temps(signature);
    let par_mesure = tics_par_mesure(signature);
    format!("{}:{}:{:02}", t / par_mesure + 1, (t % par_mesure) / par_temps + 1, t % par_temps)
}

/// Durée lisible « m:ss.d » : 72,34 -> « 1:12.3 ».
pub fn formater_secondes(secondes: f64) -> String {
    let dixiemes = (secondes.max(0.0) * 10.0).floor() as u64;
    format!("{}:{:02}.{}", dixiemes / 600, (dixiemes % 600) / 10, dixiemes % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions_et_affichage() {
        assert_eq!(tics_en_secondes(96.0, 120.0), 0.5);
        assert_eq!(secondes_en_tics(0.5, 120.0), 96.0);
        assert_eq!(tics_par_mesure(SIGNATURE_DEFAUT), 384);
        assert_eq!(tics_par_mesure(Signature { numerateur: 3, denominateur: 4 }), 288);
        assert_eq!(arrondir_a_la_mesure(0, SIGNATURE_DEFAUT), 384);
        assert_eq!(arrondir_a_la_mesure(385, SIGNATURE_DEFAUT), 768);
        assert_eq!(formater_position(0.0, SIGNATURE_DEFAUT), "1:1:00");
        assert_eq!(formater_position(384.0 + 96.0 + 5.0, SIGNATURE_DEFAUT), "2:2:05");
        assert_eq!(formater_secondes(72.34), "1:12.3");
    }
}
