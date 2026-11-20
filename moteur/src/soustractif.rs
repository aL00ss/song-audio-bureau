//! Synthétiseur soustractif intégré : deux oscillateurs à bande limitée (PolyBLEP, sans
//! repliement audible), un filtre passe-bas résonant à variables d'état (forme « TPT », stable
//! quelle que soit la fréquence de coupure), une enveloppe de volume et une de filtre.
//! Seize voix, sans aucune allocation pendant le jeu.

use std::f32::consts::PI;

use crate::source::Source;

const VOIX: usize = 16;
/// Échantillons entre deux mises à jour des coefficients du filtre.
const RYTHME_CONTROLE: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Onde {
    DentDeScie,
    Carree,
    Sinus,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Enveloppe {
    pub attaque: f32,
    pub declin: f32,
    pub maintien: f32,
    pub relachement: f32,
}

/// Réglages d'un son (un préréglage).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reglages {
    pub nom: &'static str,
    pub onde1: Onde,
    pub onde2: Onde,
    /// Hauteur du second oscillateur, en demi-tons (désaccord fin compris).
    pub ecart2: f32,
    /// Part du second oscillateur (0 à 1).
    pub melange: f32,
    /// Fréquence de coupure de base, en Hz.
    pub coupure: f32,
    /// Résonance (0 à 0,95).
    pub resonance: f32,
    /// Octaves d'ouverture du filtre ajoutées par son enveloppe au sommet.
    pub profondeur: f32,
    pub volume: Enveloppe,
    pub filtre: Enveloppe,
    /// Glissement de hauteur (portamento), en secondes ; 0 = aucun.
    pub glissement: f32,
    pub niveau: f32,
}

/// Les préréglages, dans l'ordre des numéros de programme du canal.
pub const PREREGLAGES: [Reglages; 6] = [
    Reglages {
        nom: "Basse ronde",
        onde1: Onde::DentDeScie,
        onde2: Onde::Carree,
        ecart2: -12.0,
        melange: 0.5,
        coupure: 180.0,
        resonance: 0.35,
        profondeur: 3.0,
        volume: Enveloppe { attaque: 0.002, declin: 0.25, maintien: 0.7, relachement: 0.08 },
        filtre: Enveloppe { attaque: 0.001, declin: 0.18, maintien: 0.15, relachement: 0.1 },
        glissement: 0.0,
        niveau: 0.32,
    },
    Reglages {
        nom: "Lead acide",
        onde1: Onde::DentDeScie,
        onde2: Onde::DentDeScie,
        ecart2: 0.08,
        melange: 0.5,
        coupure: 400.0,
        resonance: 0.8,
        profondeur: 4.0,
        volume: Enveloppe { attaque: 0.003, declin: 0.2, maintien: 0.8, relachement: 0.12 },
        filtre: Enveloppe { attaque: 0.002, declin: 0.22, maintien: 0.2, relachement: 0.15 },
        glissement: 0.05,
        niveau: 0.22,
    },
    Reglages {
        nom: "Nappe chaude",
        onde1: Onde::DentDeScie,
        onde2: Onde::DentDeScie,
        ecart2: 0.12,
        melange: 0.5,
        coupure: 900.0,
        resonance: 0.15,
        profondeur: 1.2,
        volume: Enveloppe { attaque: 0.6, declin: 1.0, maintien: 0.85, relachement: 1.2 },
        filtre: Enveloppe { attaque: 0.9, declin: 1.5, maintien: 0.6, relachement: 1.2 },
        glissement: 0.0,
        niveau: 0.16,
    },
    Reglages {
        nom: "Cuivres synthé",
        onde1: Onde::DentDeScie,
        onde2: Onde::DentDeScie,
        ecart2: -0.06,
        melange: 0.5,
        coupure: 600.0,
        resonance: 0.2,
        profondeur: 2.4,
        volume: Enveloppe { attaque: 0.04, declin: 0.3, maintien: 0.85, relachement: 0.25 },
        filtre: Enveloppe { attaque: 0.08, declin: 0.4, maintien: 0.5, relachement: 0.25 },
        glissement: 0.0,
        niveau: 0.2,
    },
    Reglages {
        nom: "Pluck",
        onde1: Onde::Carree,
        onde2: Onde::DentDeScie,
        ecart2: 12.0,
        melange: 0.3,
        coupure: 300.0,
        resonance: 0.4,
        profondeur: 4.5,
        volume: Enveloppe { attaque: 0.001, declin: 0.35, maintien: 0.0, relachement: 0.2 },
        filtre: Enveloppe { attaque: 0.001, declin: 0.15, maintien: 0.0, relachement: 0.15 },
        glissement: 0.0,
        niveau: 0.28,
    },
    Reglages {
        nom: "Sinus pur",
        onde1: Onde::Sinus,
        onde2: Onde::Sinus,
        ecart2: 12.0,
        melange: 0.15,
        coupure: 12000.0,
        resonance: 0.0,
        profondeur: 0.0,
        volume: Enveloppe { attaque: 0.005, declin: 0.3, maintien: 0.8, relachement: 0.15 },
        filtre: Enveloppe { attaque: 0.001, declin: 0.1, maintien: 1.0, relachement: 0.1 },
        glissement: 0.0,
        niveau: 0.3,
    },
];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Etape {
    Repos,
    Attaque,
    Declin,
    Maintien,
    Relachement,
}

#[derive(Clone, Copy, Debug)]
struct EtatEnveloppe {
    etape: Etape,
    valeur: f32,
}

/// Coefficients d'une enveloppe à une fréquence d'échantillonnage donnée (calculés une fois par
/// bloc, pas à chaque échantillon).
#[derive(Clone, Copy, Debug)]
struct Pentes {
    attaque: f32,
    declin: f32,
    maintien: f32,
    relachement: f32,
}

impl Pentes {
    fn new(e: &Enveloppe, fe: f32) -> Self {
        let pente = |secondes: f32| 1.0 - (-1.0 / (secondes.max(0.0005) * fe)).exp();
        Pentes { attaque: pente(e.attaque), declin: pente(e.declin), maintien: e.maintien, relachement: pente(e.relachement) }
    }
}

impl EtatEnveloppe {
    const fn repos() -> Self {
        EtatEnveloppe { etape: Etape::Repos, valeur: 0.0 }
    }

    /// Avance d'un échantillon (segments exponentiels, plus naturels à l'oreille).
    #[inline]
    fn avancer(&mut self, e: &Pentes) -> f32 {
        match self.etape {
            Etape::Repos => {}
            Etape::Attaque => {
                // vise un peu au-dessus de 1 pour arriver en un temps fini
                self.valeur += (1.2 - self.valeur) * e.attaque;
                if self.valeur >= 1.0 {
                    self.valeur = 1.0;
                    self.etape = Etape::Declin;
                }
            }
            Etape::Declin => {
                self.valeur += (e.maintien - self.valeur) * e.declin;
                if (self.valeur - e.maintien).abs() < 1e-4 {
                    self.etape = Etape::Maintien;
                }
            }
            Etape::Maintien => self.valeur = e.maintien,
            Etape::Relachement => {
                self.valeur -= self.valeur * e.relachement;
                if self.valeur < 1e-5 {
                    self.valeur = 0.0;
                    self.etape = Etape::Repos;
                }
            }
        }
        self.valeur
    }
}

#[derive(Clone, Copy, Debug)]
struct Voix {
    hauteur: u8,
    tenue: bool,
    velocite: f32,
    frequence: f32,
    cible: f32,
    phases: [f32; 2],
    volume: EtatEnveloppe,
    filtre: EtatEnveloppe,
    /// États du filtre (deux intégrateurs), gauche et droite confondus (voix mono).
    ic1: f32,
    ic2: f32,
    /// Ordre d'allumage, pour voler la voix la plus ancienne.
    age: u64,
    /// Panoramique propre à la note (-1 à 1, ajouté à celui du canal).
    pan: f32,
}

impl Voix {
    const fn repos() -> Self {
        Voix {
            hauteur: 0,
            tenue: false,
            velocite: 0.0,
            frequence: 440.0,
            cible: 440.0,
            phases: [0.0; 2],
            volume: EtatEnveloppe::repos(),
            filtre: EtatEnveloppe::repos(),
            ic1: 0.0,
            ic2: 0.0,
            age: 0,
            pan: 0.0,
        }
    }

    fn active(&self) -> bool {
        self.volume.etape != Etape::Repos
    }
}

/// Correction PolyBLEP d'une discontinuité (t : phase dans [0, 1), dt : pas de phase).
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

#[inline]
fn oscillateur(onde: Onde, phase: f32, dt: f32) -> f32 {
    match onde {
        Onde::Sinus => (2.0 * PI * phase).sin(),
        Onde::DentDeScie => 2.0 * phase - 1.0 - poly_blep(phase, dt),
        Onde::Carree => {
            let mut v = if phase < 0.5 { 1.0 } else { -1.0 };
            v += poly_blep(phase, dt);
            v -= poly_blep((phase + 0.5) % 1.0, dt);
            v
        }
    }
}

fn hauteur_en_hz(hauteur: f32) -> f32 {
    440.0 * 2f32.powf((hauteur - 69.0) / 12.0)
}

pub struct Soustractif {
    fe: f32,
    reglages: Reglages,
    voix: [Voix; VOIX],
    compteur: u64,
    gain: f32,
    pan: f32,
}

impl Soustractif {
    pub fn new(frequence: u32) -> Self {
        Soustractif { fe: frequence as f32, reglages: PREREGLAGES[0], voix: [Voix::repos(); VOIX], compteur: 0, gain: 1.0, pan: 0.0 }
    }

    /// Voix qui sonnent (pour les tests et les mesures).
    pub fn voix_actives(&self) -> usize {
        self.voix.iter().filter(|v| v.active()).count()
    }
}

impl Source for Soustractif {
    fn regler(&mut self, programme: u8, _batterie: bool, volume: f32, pan: f32) {
        self.reglages = PREREGLAGES[programme as usize % PREREGLAGES.len()];
        // même courbe que le contrôleur 7 d'un synthétiseur General MIDI
        self.gain = volume.clamp(0.0, 1.0).powi(2) / 0.64;
        self.pan = pan.clamp(-1.0, 1.0);
    }

    fn note_on(&mut self, hauteur: u8, velocite: u8) {
        self.note_on_expressive(hauteur, velocite, 0.0, 0.0);
    }

    fn note_on_expressive(&mut self, hauteur: u8, velocite: u8, pan: f32, demi_tons: f32) {
        self.compteur += 1;
        let frequence = hauteur_en_hz(hauteur as f32 + demi_tons);
        // glissement : la nouvelle note part de la hauteur de la dernière voix jouée
        let depart = if self.reglages.glissement > 0.0 {
            self.voix.iter().filter(|v| v.active()).max_by_key(|v| v.age).map(|v| v.frequence).unwrap_or(frequence)
        } else {
            frequence
        };
        // voix libre, sinon la plus ancienne
        let index = self
            .voix
            .iter()
            .position(|v| !v.active())
            .unwrap_or_else(|| (0..VOIX).min_by_key(|&i| self.voix[i].age).unwrap_or(0));
        let v = &mut self.voix[index];
        *v = Voix::repos();
        v.hauteur = hauteur;
        v.tenue = true;
        v.velocite = (velocite as f32 / 127.0).powf(1.5);
        v.frequence = depart;
        v.cible = frequence;
        v.volume.etape = Etape::Attaque;
        v.filtre.etape = Etape::Attaque;
        v.age = self.compteur;
        v.pan = pan.clamp(-1.0, 1.0);
    }

    fn note_off(&mut self, hauteur: u8) {
        for v in self.voix.iter_mut().filter(|v| v.tenue && v.hauteur == hauteur) {
            v.tenue = false;
            v.volume.etape = Etape::Relachement;
            v.filtre.etape = Etape::Relachement;
        }
    }

    fn tout_couper(&mut self) {
        self.voix = [Voix::repos(); VOIX];
    }

    fn voix(&self) -> u32 {
        self.voix_actives() as u32
    }

    fn rendre(&mut self, gauche: &mut [f32], droite: &mut [f32]) {
        gauche.fill(0.0);
        droite.fill(0.0);
        let r = self.reglages;
        let fe = self.fe;
        let glisse = if r.glissement > 0.0 { 1.0 - (-1.0 / (r.glissement * fe)).exp() } else { 1.0 };
        let k = 2.0 - 2.0 * r.resonance.clamp(0.0, 0.95); // amortissement du filtre
        let rapport2 = 2f32.powf(r.ecart2 / 12.0);
        let (pentes_volume, pentes_filtre) = (Pentes::new(&r.volume, fe), Pentes::new(&r.filtre, fe));
        let gain_voix = r.niveau;
        let (pan_canal, gain_canal) = (self.pan, self.gain);
        for v in self.voix.iter_mut().filter(|v| v.active()) {
            // stéréo : panoramique à puissance constante (celui du canal et celui de la note)
            let angle = ((pan_canal + v.pan).clamp(-1.0, 1.0) + 1.0) * PI / 4.0;
            let (gg, gd) = (angle.cos() * std::f32::consts::SQRT_2 * gain_canal, angle.sin() * std::f32::consts::SQRT_2 * gain_canal);
            let (mut g, mut a1) = (0.0f32, 1.0f32);
            for i in 0..gauche.len() {
                v.frequence += (v.cible - v.frequence) * glisse;
                let dt1 = v.frequence / fe;
                let dt2 = dt1 * rapport2;
                let son = oscillateur(r.onde1, v.phases[0], dt1) * (1.0 - r.melange) + oscillateur(r.onde2, v.phases[1], dt2) * r.melange;
                v.phases[0] += dt1;
                if v.phases[0] >= 1.0 {
                    v.phases[0] -= 1.0;
                }
                v.phases[1] += dt2;
                if v.phases[1] >= 1.0 {
                    v.phases[1] -= v.phases[1].floor();
                }
                // filtre : coupure ouverte par son enveloppe, bornée sous Nyquist ; ses
                // coefficients (tangente, puissance) changent au rythme de contrôle, tous les
                // RYTHME_CONTROLE échantillons
                let env_f = v.filtre.avancer(&pentes_filtre);
                if i % RYTHME_CONTROLE == 0 {
                    let coupure = (r.coupure * 2f32.powf(r.profondeur * env_f)).min(fe * 0.45);
                    g = (PI * coupure / fe).tan();
                    a1 = 1.0 / (1.0 + g * (g + k));
                }
                let v1 = a1 * (v.ic1 + g * (son - v.ic2));
                let v2 = v.ic2 + g * v1;
                v.ic1 = 2.0 * v1 - v.ic1;
                v.ic2 = 2.0 * v2 - v.ic2;
                let filtre = v2;
                let amp = v.volume.avancer(&pentes_volume) * v.velocite * gain_voix;
                gauche[i] += filtre * amp * gg;
                droite[i] += filtre * amp * gd;
                if !v.active() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noyau::tests::{FE, hauteur};

    fn rendre(s: &mut Soustractif, secondes: f32) -> Vec<f32> {
        let n = (secondes * FE as f32) as usize;
        let (mut g, mut d) = (vec![0.0; n], vec![0.0; n]);
        for debut in (0..n).step_by(512) {
            let fin = (debut + 512).min(n);
            s.rendre(&mut g[debut..fin], &mut d[debut..fin]);
        }
        g
    }

    fn efficace(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    #[test]
    fn chaque_preglage_joue_juste() {
        for (programme, r) in PREREGLAGES.iter().enumerate() {
            let mut s = Soustractif::new(FE);
            s.regler(programme as u8, false, 0.8, 0.0);
            s.note_on(57, 110);
            let g = rendre(&mut s, 0.6);
            assert!(efficace(&g[FE as usize / 10..]) > 0.005, "{} sonne", r.nom);
            let h = hauteur(&g, (0.3 * FE as f32) as usize);
            // l'autocorrélation peut tomber sur l'octave : on compare la classe de hauteur
            let ecart = ((h - 57.0).rem_euclid(12.0) + 6.0).rem_euclid(12.0) - 6.0;
            assert!(ecart.abs() < 0.25, "{} : hauteur {h}", r.nom);
        }
    }

    #[test]
    fn relachement_puis_silence_et_voix_liberees() {
        let mut s = Soustractif::new(FE);
        s.regler(0, false, 0.8, 0.0);
        s.note_on(40, 100);
        rendre(&mut s, 0.3);
        assert_eq!(s.voix(), 1);
        s.note_off(40);
        let g = rendre(&mut s, 1.0);
        assert!(efficace(&g[(0.8 * FE as f32) as usize..]) < 1e-5);
        assert_eq!(s.voix(), 0);
    }

    #[test]
    fn panoramique_et_hauteur_propres_a_la_note() {
        let mut s = Soustractif::new(FE);
        s.regler(5, false, 0.8, 0.0); // sinus pur
        s.note_on_expressive(69, 100, -1.0, 1.0);
        let (mut g, mut d) = (vec![0.0f32; 22050], vec![0.0f32; 22050]);
        s.rendre(&mut g, &mut d);
        let efficace = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        assert!(efficace(&d[4410..]) < 1e-4 && efficace(&g[4410..]) > 0.01, "tout à gauche");
        let passages = g[4410..].windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count() as f32 / ((22050 - 4410) as f32 / FE as f32);
        assert!((passages - 466.16).abs() < 6.0, "un demi-ton au-dessus du la : {passages} Hz");
    }

    #[test]
    fn plus_de_notes_que_de_voix_sans_panique() {
        let mut s = Soustractif::new(FE);
        s.regler(2, false, 0.8, 0.0);
        for h in 30..80 {
            s.note_on(h, 90);
        }
        assert_eq!(s.voix(), VOIX as u32);
        let g = rendre(&mut s, 0.2);
        assert!(g.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn filtre_plus_ferme_son_plus_sombre() {
        // énergie de la dérivée rapportée à celle du signal : grandit avec les aigus
        let brillance = |coupure: f32| {
            let mut s = Soustractif::new(FE);
            s.reglages = Reglages { coupure, profondeur: 0.0, ..PREREGLAGES[0] };
            s.gain = 1.0;
            s.note_on(45, 110);
            let g = rendre(&mut s, 0.5);
            let derivee: Vec<f32> = g.windows(2).map(|w| w[1] - w[0]).collect();
            efficace(&derivee) / efficace(&g)
        };
        let (sombre, clair) = (brillance(300.0), brillance(6000.0));
        assert!(sombre * 2.0 < clair, "sombre {sombre}, clair {clair}");
    }

    #[test]
    fn panoramique_et_volume_du_canal() {
        let mut s = Soustractif::new(FE);
        s.regler(5, false, 0.8, -1.0);
        s.note_on(69, 100);
        let n = FE as usize / 2;
        let (mut g, mut d) = (vec![0.0; n], vec![0.0; n]);
        s.rendre(&mut g, &mut d);
        assert!(efficace(&d) < 1e-6 && efficace(&g) > 0.01);
    }
}
