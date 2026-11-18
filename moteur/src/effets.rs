//! Traitements du mixer, échantillon par échantillon, sans allocation : filtres d'égaliseur
//! (formules de l'« Audio EQ Cookbook », les mêmes que la Web Audio API), panoramique stéréo,
//! réverbération (Freeverb) et limiteur du maître.

use std::f32::consts::PI;

/// Filtre biquadratique (forme directe I).
#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: [f32; 2],
    x2: [f32; 2],
    y1: [f32; 2],
    y2: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TypeFiltre {
    /// Plateau grave (pente S = 1, comme le « lowshelf » de la Web Audio API).
    PlateauGrave,
    /// Cloche, avec son facteur de qualité.
    Cloche(f32),
    /// Plateau aigu (pente S = 1).
    PlateauAigu,
}

impl Biquad {
    pub fn neutre() -> Self {
        Biquad { b0: 1.0, ..Default::default() }
    }

    /// Règle les coefficients ; l'état (mémoire du filtre) est gardé pour éviter les clics.
    pub fn regler(&mut self, type_filtre: TypeFiltre, frequence: f32, gain_db: f32, frequence_echantillonnage: f32) {
        if gain_db.abs() < 1e-3 {
            (self.b0, self.b1, self.b2, self.a1, self.a2) = (1.0, 0.0, 0.0, 0.0, 0.0);
            return;
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * frequence / frequence_echantillonnage;
        let (cos, sin) = (w0.cos(), w0.sin());
        let (b0, b1, b2, a0, a1, a2) = match type_filtre {
            TypeFiltre::Cloche(q) => {
                let alpha = sin / (2.0 * q);
                (1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a)
            }
            TypeFiltre::PlateauGrave => {
                let alpha = sin / 2.0 * 2f32.sqrt();
                let racine = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + racine),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - racine),
                    (a + 1.0) + (a - 1.0) * cos + racine,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - racine,
                )
            }
            TypeFiltre::PlateauAigu => {
                let alpha = sin / 2.0 * 2f32.sqrt();
                let racine = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + racine),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - racine),
                    (a + 1.0) - (a - 1.0) * cos + racine,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - racine,
                )
            }
        };
        (self.b0, self.b1, self.b2, self.a1, self.a2) = (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    }

    /// Filtre un échantillon du côté `cote` (0 = gauche, 1 = droite).
    #[inline]
    pub fn traiter(&mut self, cote: usize, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1[cote] + self.b2 * self.x2[cote] - self.a1 * self.y1[cote] - self.a2 * self.y2[cote];
        self.x2[cote] = self.x1[cote];
        self.x1[cote] = x;
        self.y2[cote] = self.y1[cote];
        self.y1[cote] = if y.abs() < 1e-20 { 0.0 } else { y };
        self.y1[cote]
    }
}

/// Panoramique d'un signal stéréo, même loi que le StereoPannerNode de la Web Audio API :
/// à gauche, le canal droit est versé dans le gauche à puissance constante (et inversement).
#[inline]
pub fn panoramique(pan: f32, gauche: f32, droite: f32) -> (f32, f32) {
    let p = pan.clamp(-1.0, 1.0);
    if p <= 0.0 {
        let x = (p + 1.0) * PI / 2.0;
        (gauche + droite * x.cos(), droite * x.sin())
    } else {
        let x = p * PI / 2.0;
        (gauche * x.cos(), droite + gauche * x.sin())
    }
}

/// Loi du fader : quadratique, 0,8 = 0 dB (comme la version web).
#[inline]
pub fn gain_du_fader(position: f32) -> f32 {
    let p = position.clamp(0.0, 1.0);
    (p / 0.8) * (p / 0.8)
}

// ------------------------------------------------------------------ réverbération (Freeverb)

const PEIGNES: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const PASSE_TOUT: [usize; 4] = [556, 441, 341, 225];
const ECART_STEREO: usize = 23;
const TAILLE_MAX: usize = 1617 + 23;

struct Peigne {
    tampon: Vec<f32>,
    longueur: usize,
    index: usize,
    memoire: f32,
}

impl Peigne {
    fn new(longueur: usize) -> Self {
        Peigne { tampon: vec![0.0; TAILLE_MAX * 2], longueur, index: 0, memoire: 0.0 }
    }
    #[inline]
    fn traiter(&mut self, x: f32, retour: f32, amorti: f32) -> f32 {
        let y = self.tampon[self.index];
        self.memoire = y * (1.0 - amorti) + self.memoire * amorti;
        self.tampon[self.index] = x + self.memoire * retour;
        self.index = (self.index + 1) % self.longueur;
        y
    }
}

struct PasseTout {
    tampon: Vec<f32>,
    longueur: usize,
    index: usize,
}

impl PasseTout {
    fn new(longueur: usize) -> Self {
        PasseTout { tampon: vec![0.0; TAILLE_MAX], longueur, index: 0 }
    }
    #[inline]
    fn traiter(&mut self, x: f32) -> f32 {
        let tampon = self.tampon[self.index];
        let y = -x + tampon;
        self.tampon[self.index] = x + tampon * 0.5;
        self.index = (self.index + 1) % self.longueur;
        y
    }
}

/// Réverbération commune (salle moyenne), alimentée par les envois des pistes.
pub struct Reverb {
    peignes: [Vec<Peigne>; 2],
    passe_tout: [Vec<PasseTout>; 2],
}

impl Reverb {
    pub fn new(frequence: f32) -> Self {
        let echelle = frequence / 44100.0;
        let cote = |ecart: usize| {
            (
                PEIGNES.iter().map(|&l| Peigne::new(((l + ecart) as f32 * echelle) as usize)).collect::<Vec<_>>(),
                PASSE_TOUT.iter().map(|&l| PasseTout::new(((l + ecart) as f32 * echelle) as usize)).collect::<Vec<_>>(),
            )
        };
        let (pg, ag) = cote(0);
        let (pd, ad) = cote(ECART_STEREO);
        Reverb { peignes: [pg, pd], passe_tout: [ag, ad] }
    }

    /// Entrée stéréo -> sortie de réverbération stéréo (signal mouillé seulement).
    #[inline]
    pub fn traiter(&mut self, gauche: f32, droite: f32) -> (f32, f32) {
        let entree = (gauche + droite) * 0.015;
        let mut sortie = [0.0f32; 2];
        for (cote, s) in sortie.iter_mut().enumerate() {
            for peigne in self.peignes[cote].iter_mut() {
                *s += peigne.traiter(entree, 0.84, 0.2);
            }
            for filtre in self.passe_tout[cote].iter_mut() {
                *s = filtre.traiter(*s);
            }
        }
        (sortie[0], sortie[1])
    }
}

// ------------------------------------------------------------------ limiteur du maître

/// Compresseur à seuil -3 dB, rapport 20, attaque 2 ms, relâchement 150 ms (comme la version web).
pub struct Limiteur {
    enveloppe: f32,
    attaque: f32,
    relachement: f32,
}

const SEUIL: f32 = 0.707_946; // -3 dB
const RAPPORT: f32 = 20.0;

impl Limiteur {
    pub fn new(frequence: f32) -> Self {
        let coefficient = |secondes: f32| (-1.0 / (secondes * frequence)).exp();
        Limiteur { enveloppe: 0.0, attaque: coefficient(0.002), relachement: coefficient(0.150) }
    }

    #[inline]
    pub fn traiter(&mut self, gauche: f32, droite: f32) -> (f32, f32) {
        let crete = gauche.abs().max(droite.abs());
        let k = if crete > self.enveloppe { self.attaque } else { self.relachement };
        self.enveloppe = crete + (self.enveloppe - crete) * k;
        if self.enveloppe <= SEUIL {
            return (gauche, droite);
        }
        // au-dessus du seuil, la sortie ne monte que d'un vingtième de l'excès (en dB)
        let exces_db = 20.0 * (self.enveloppe / SEUIL).log10();
        let gain = 10f32.powf(-exces_db * (1.0 - 1.0 / RAPPORT) / 20.0);
        (gauche * gain, droite * gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sinus(frequence: f32, fe: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| (2.0 * PI * frequence * i as f32 / fe).sin()).collect()
    }

    fn efficace(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    fn gain_filtre(type_filtre: TypeFiltre, f0: f32, db: f32, f_test: f32) -> f32 {
        let fe = 48000.0;
        let mut filtre = Biquad::neutre();
        filtre.regler(type_filtre, f0, db, fe);
        let x = sinus(f_test, fe, 48000);
        let y: Vec<f32> = x.iter().map(|&v| filtre.traiter(0, v)).collect();
        20.0 * (efficace(&y[24000..]) / efficace(&x[24000..])).log10()
    }

    #[test]
    fn egaliseur_grave_medium_aigu() {
        // +6 dB : pleinement appliqué loin dans sa zone, rien loin de l'autre côté
        assert!((gain_filtre(TypeFiltre::PlateauGrave, 200.0, 6.0, 40.0) - 6.0).abs() < 0.5);
        assert!(gain_filtre(TypeFiltre::PlateauGrave, 200.0, 6.0, 5000.0).abs() < 0.3);
        assert!((gain_filtre(TypeFiltre::Cloche(0.8), 1200.0, -9.0, 1200.0) + 9.0).abs() < 0.3);
        assert!((gain_filtre(TypeFiltre::PlateauAigu, 5000.0, 6.0, 16000.0) - 6.0).abs() < 0.6);
        assert!(gain_filtre(TypeFiltre::PlateauAigu, 5000.0, 6.0, 100.0).abs() < 0.3);
        assert!(gain_filtre(TypeFiltre::Cloche(0.8), 1200.0, 0.0, 1200.0).abs() < 0.01);
    }

    #[test]
    fn panoramique_et_fader() {
        let (g, d) = panoramique(0.0, 1.0, 1.0);
        assert!((g - 1.0).abs() < 1e-6 && (d - 1.0).abs() < 1e-6);
        let (g, d) = panoramique(-1.0, 1.0, 1.0);
        assert!((g - 2.0).abs() < 1e-5 && d.abs() < 1e-5);
        let (g, d) = panoramique(1.0, 1.0, 1.0);
        assert!(g.abs() < 1e-5 && (d - 2.0).abs() < 1e-5);
        assert!((gain_du_fader(0.8) - 1.0).abs() < 1e-6);
        assert!((gain_du_fader(0.4) - 0.25).abs() < 1e-6);
        assert_eq!(gain_du_fader(0.0), 0.0);
    }

    #[test]
    fn limiteur_retient_les_cretes() {
        let fe = 48000.0;
        let mut limiteur = Limiteur::new(fe);
        let x = sinus(220.0, fe, 48000);
        let fort: Vec<f32> = x.iter().map(|v| limiteur.traiter(v * 4.0, v * 4.0).0).collect();
        let crete = fort[24000..].iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(crete < 1.0, "crête {crete}");
        let mut limiteur = Limiteur::new(fe);
        let doux: Vec<f32> = x.iter().map(|v| limiteur.traiter(v * 0.3, v * 0.3).0).collect();
        assert!((efficace(&doux[24000..]) - efficace(&x[24000..]) * 0.3).abs() < 1e-4);
    }

    #[test]
    fn reverb_prolonge_le_son() {
        let mut reverb = Reverb::new(48000.0);
        let mut queue = 0f32;
        for i in 0..48000 {
            let x = if i < 100 { 1.0 } else { 0.0 };
            let (g, d) = reverb.traiter(x, x);
            if i > 24000 {
                queue = queue.max(g.abs()).max(d.abs());
            }
        }
        assert!(queue > 1e-4, "la réverbération s'éteint trop vite : {queue}");
    }
}
