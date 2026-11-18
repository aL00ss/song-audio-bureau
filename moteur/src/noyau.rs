//! Cœur du moteur : synthétiseurs, événements datés et mixer. Indépendant de la carte son,
//! il sert aussi bien au rendu en direct (appelé par le fil audio) qu'au rendu hors ligne
//! (export WAV, tests).
//!
//! Un synthétiseur par canal : rustysynth ne joue la batterie que sur son canal 10, alors que
//! chaque batterie du projet a son propre canal. Chaque canal a donc sa sortie séparée, qui
//! alimente la piste de mixage où il est routé.

use std::sync::Arc;

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use crate::effets::{gain_du_fader, panoramique, Biquad, Limiteur, Reverb, TypeFiltre};

/// Canaux disponibles (comme la version web).
pub const CANAUX: usize = 32;
/// Pistes de mixage, maître compris (index 0).
pub const PISTES: usize = 33;
/// Plus long bloc rendu d'un coup ; au-delà, le rendu est découpé.
pub const BLOC_MAX: usize = 1024;
/// Événements en attente au plus (au-delà, les plus lointains sont ignorés).
const FILE_MAX: usize = 16384;

const CANAL_MELODIQUE: i32 = 0;
const CANAL_BATTERIE: i32 = 9;
const NOTE_ON: i32 = 0x90;
const NOTE_OFF: i32 = 0x80;
const CONTROLE: i32 = 0xB0;
const PROGRAMME: i32 = 0xC0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Message {
    NoteOn { hauteur: u8, velocite: u8 },
    NoteOff { hauteur: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Evenement {
    /// Instant, en échantillons depuis le démarrage du moteur.
    pub echantillon: u64,
    pub canal: u8,
    pub message: Message,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReglageCanal {
    pub actif: bool,
    pub programme: u8,
    pub batterie: bool,
    /// Volume du canal (0 à 1, contrôleur 7).
    pub volume: f32,
    /// Panoramique du canal (-1 à 1, contrôleur 10).
    pub pan: f32,
    /// Piste de mixage qui reçoit ce canal.
    pub piste: u8,
}

impl Default for ReglageCanal {
    fn default() -> Self {
        ReglageCanal { actif: false, programme: 0, batterie: false, volume: 0.8, pan: 0.0, piste: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReglagePiste {
    pub existe: bool,
    /// Position du fader (0 à 1, 0,8 = 0 dB).
    pub volume: f32,
    pub pan: f32,
    /// Muet ou hors solo : la piste ne sort plus.
    pub audible: bool,
    /// Égaliseur, en dB : grave (200 Hz), médium (1,2 kHz), aigu (5 kHz).
    pub eq: [f32; 3],
    /// Envoi vers la réverbération (0 à 1).
    pub reverb: f32,
}

impl Default for ReglagePiste {
    fn default() -> Self {
        ReglagePiste { existe: false, volume: 0.8, pan: 0.0, audible: true, eq: [0.0; 3], reverb: 0.0 }
    }
}

/// Tout le réglage du mixer et des canaux, envoyé d'un bloc au fil audio.
#[derive(Clone, Debug, PartialEq)]
pub struct Configuration {
    pub canaux: [ReglageCanal; CANAUX],
    pub pistes: [ReglagePiste; PISTES],
}

impl Default for Configuration {
    fn default() -> Self {
        let mut pistes = [ReglagePiste::default(); PISTES];
        pistes[0].existe = true;
        Configuration { canaux: [ReglageCanal::default(); CANAUX], pistes }
    }
}

/// Mesures pour l'interface : crête de chaque piste et nombre de voix de chaque canal.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesures {
    pub cretes: Vec<[f32; 2]>,
    pub voix: Vec<u32>,
}

struct Canal {
    synth: Synthesizer,
    reglage: ReglageCanal,
    gauche: Vec<f32>,
    droite: Vec<f32>,
    /// Notes tenues (pour compter les voix sans accès à celles du synthétiseur).
    tenues: [bool; 128],
    /// Le canal a produit du son dans le dernier bloc.
    sonne: bool,
}

impl Canal {
    fn canal_midi(&self) -> i32 {
        if self.reglage.batterie { CANAL_BATTERIE } else { CANAL_MELODIQUE }
    }

    fn appliquer(&mut self, reglage: ReglageCanal) {
        let ancien = self.reglage;
        self.reglage = reglage;
        if ancien.batterie != reglage.batterie {
            let midi = if ancien.batterie { CANAL_BATTERIE } else { CANAL_MELODIQUE };
            self.synth.note_off_all_channel(midi, true);
            self.tenues = [false; 128];
        }
        let midi = self.canal_midi();
        if !reglage.batterie && (ancien.programme != reglage.programme || ancien.batterie || !ancien.actif) {
            self.synth.process_midi_message(midi, PROGRAMME, reglage.programme as i32, 0);
        }
        self.synth.process_midi_message(midi, CONTROLE, 7, (reglage.volume.clamp(0.0, 1.0) * 127.0).round() as i32);
        self.synth.process_midi_message(midi, CONTROLE, 10, (((reglage.pan.clamp(-1.0, 1.0) + 1.0) / 2.0) * 127.0).round() as i32);
    }
}

struct Piste {
    reglage: ReglagePiste,
    filtres: [Biquad; 3],
    gauche: Vec<f32>,
    droite: Vec<f32>,
    crete: [f32; 2],
}

pub struct Noyau {
    frequence: u32,
    /// Échantillons déjà rendus : l'horloge du moteur.
    horloge: u64,
    canaux: Vec<Canal>,
    pistes: Vec<Piste>,
    file: Vec<Evenement>,
    reverb: Reverb,
    limiteur: Limiteur,
    envoi: [Vec<f32>; 2],
}

impl Noyau {
    pub fn new(banque: &Arc<SoundFont>, frequence: u32) -> Result<Self, String> {
        let mut reglages = SynthesizerSettings::new(frequence as i32);
        reglages.enable_reverb_and_chorus = false; // la réverbération est celle du mixer
        reglages.maximum_polyphony = 64;
        let mut canaux = Vec::with_capacity(CANAUX);
        for _ in 0..CANAUX {
            let synth = Synthesizer::new(banque, &reglages).map_err(|e| format!("synthétiseur : {e:?}"))?;
            canaux.push(Canal {
                synth,
                reglage: ReglageCanal::default(),
                gauche: vec![0.0; BLOC_MAX],
                droite: vec![0.0; BLOC_MAX],
                tenues: [false; 128],
                sonne: false,
            });
        }
        let pistes = (0..PISTES)
            .map(|_| Piste {
                reglage: ReglagePiste::default(),
                filtres: [Biquad::neutre(); 3],
                gauche: vec![0.0; BLOC_MAX],
                droite: vec![0.0; BLOC_MAX],
                crete: [0.0; 2],
            })
            .collect();
        let mut noyau = Noyau {
            frequence,
            horloge: 0,
            canaux,
            pistes,
            file: Vec::with_capacity(FILE_MAX),
            reverb: Reverb::new(frequence as f32),
            limiteur: Limiteur::new(frequence as f32),
            envoi: [vec![0.0; BLOC_MAX], vec![0.0; BLOC_MAX]],
        };
        noyau.configurer(&Configuration::default());
        Ok(noyau)
    }

    pub fn frequence(&self) -> u32 {
        self.frequence
    }

    pub fn horloge(&self) -> u64 {
        self.horloge
    }

    /// Convertit un instant en secondes (horloge du moteur) en échantillons.
    pub fn echantillon(&self, secondes: f64) -> u64 {
        (secondes.max(0.0) * self.frequence as f64).round() as u64
    }

    pub fn configurer(&mut self, configuration: &Configuration) {
        for (canal, reglage) in self.canaux.iter_mut().zip(configuration.canaux.iter()) {
            if *reglage != canal.reglage || !canal.reglage.actif {
                canal.appliquer(*reglage);
            }
        }
        let fe = self.frequence as f32;
        for (piste, reglage) in self.pistes.iter_mut().zip(configuration.pistes.iter()) {
            if piste.reglage.eq != reglage.eq || !piste.reglage.existe {
                piste.filtres[0].regler(TypeFiltre::PlateauGrave, 200.0, reglage.eq[0], fe);
                piste.filtres[1].regler(TypeFiltre::Cloche(0.8), 1200.0, reglage.eq[1], fe);
                piste.filtres[2].regler(TypeFiltre::PlateauAigu, 5000.0, reglage.eq[2], fe);
            }
            piste.reglage = *reglage;
        }
    }

    /// Ajoute un événement à la file (gardée triée ; à instant égal, l'ordre d'arrivée).
    pub fn programmer(&mut self, evenement: Evenement) {
        if self.file.len() >= FILE_MAX || evenement.canal as usize >= CANAUX {
            return;
        }
        let position = self.file.partition_point(|e| e.echantillon <= evenement.echantillon);
        self.file.insert(position, evenement);
    }

    /// Coupe net ce qui sonne et oublie ce qui était programmé.
    pub fn tout_couper(&mut self) {
        self.file.clear();
        for canal in self.canaux.iter_mut() {
            canal.synth.note_off_all(true);
            canal.tenues = [false; 128];
        }
    }

    fn executer(&mut self, evenement: Evenement) {
        let canal = &mut self.canaux[evenement.canal as usize];
        let midi = canal.canal_midi();
        match evenement.message {
            Message::NoteOn { hauteur, velocite } => {
                canal.synth.process_midi_message(midi, NOTE_ON, hauteur as i32, velocite as i32);
                canal.tenues[hauteur as usize & 127] = true;
            }
            Message::NoteOff { hauteur } => {
                canal.synth.process_midi_message(midi, NOTE_OFF, hauteur as i32, 0);
                canal.tenues[hauteur as usize & 127] = false;
            }
        }
    }

    /// Rend la suite du son dans `gauche` et `droite` (même longueur), en jouant les événements
    /// à leur instant.
    pub fn rendre(&mut self, gauche: &mut [f32], droite: &mut [f32]) {
        let mut fait = 0;
        while fait < gauche.len() {
            // jusqu'au prochain événement, au plus BLOC_MAX échantillons
            let reste = (gauche.len() - fait).min(BLOC_MAX);
            let prochain = self.file.first().map(|e| e.echantillon);
            let longueur = match prochain {
                Some(t) if t <= self.horloge => 0,
                Some(t) => ((t - self.horloge) as usize).min(reste),
                None => reste,
            };
            if longueur == 0 {
                let evenement = self.file.remove(0);
                self.executer(evenement);
                continue;
            }
            self.rendre_bloc(&mut gauche[fait..fait + longueur], &mut droite[fait..fait + longueur]);
            fait += longueur;
        }
    }

    fn rendre_bloc(&mut self, gauche: &mut [f32], droite: &mut [f32]) {
        let n = gauche.len();
        for piste in self.pistes.iter_mut() {
            piste.gauche[..n].fill(0.0);
            piste.droite[..n].fill(0.0);
        }
        // 1. chaque canal actif dans sa piste
        for canal in self.canaux.iter_mut() {
            if !canal.reglage.actif {
                continue;
            }
            canal.synth.render(&mut canal.gauche[..n], &mut canal.droite[..n]);
            let mut crete = 0f32;
            let index = canal.reglage.piste as usize;
            let piste = &mut self.pistes[if index < PISTES && index > 0 { index } else { 0 }];
            for i in 0..n {
                crete = crete.max(canal.gauche[i].abs()).max(canal.droite[i].abs());
                piste.gauche[i] += canal.gauche[i];
                piste.droite[i] += canal.droite[i];
            }
            canal.sonne = crete > 1e-4;
        }
        // 2. les pistes (hors maître) : égaliseur, fader, panoramique, muet, envoi réverbération
        self.envoi[0][..n].fill(0.0);
        self.envoi[1][..n].fill(0.0);
        let (maitre, autres) = self.pistes.split_at_mut(1);
        let maitre = &mut maitre[0];
        for piste in autres.iter_mut() {
            if !piste.reglage.existe {
                continue;
            }
            let gain = if piste.reglage.audible { gain_du_fader(piste.reglage.volume) } else { 0.0 };
            let mut crete = [0f32; 2];
            for i in 0..n {
                let (mut g, mut d) = (piste.gauche[i], piste.droite[i]);
                for filtre in piste.filtres.iter_mut() {
                    g = filtre.traiter(0, g);
                    d = filtre.traiter(1, d);
                }
                let (g, d) = panoramique(piste.reglage.pan, g * gain, d * gain);
                crete = [crete[0].max(g.abs()), crete[1].max(d.abs())];
                maitre.gauche[i] += g;
                maitre.droite[i] += d;
                self.envoi[0][i] += g * piste.reglage.reverb;
                self.envoi[1][i] += d * piste.reglage.reverb;
            }
            piste.crete = [piste.crete[0].max(crete[0]), piste.crete[1].max(crete[1])];
        }
        // 3. le maître : réverbération, fader, limiteur
        let gain = gain_du_fader(maitre.reglage.volume);
        let mut crete = [0f32; 2];
        for i in 0..n {
            let (rg, rd) = self.reverb.traiter(self.envoi[0][i], self.envoi[1][i]);
            let (g, d) = self.limiteur.traiter((maitre.gauche[i] + rg) * gain, (maitre.droite[i] + rd) * gain);
            gauche[i] = g;
            droite[i] = d;
            crete = [crete[0].max(g.abs()), crete[1].max(d.abs())];
        }
        maitre.crete = [maitre.crete[0].max(crete[0]), maitre.crete[1].max(crete[1])];
        self.horloge += n as u64;
    }

    /// Ajoute les crêtes du moment à celles déjà accumulées dans `mesures` (le maximum est
    /// gardé jusqu'à ce que l'interface les lise) et met à jour les voix. Sans allocation une
    /// fois `mesures` à la bonne taille.
    pub fn cumuler_mesures(&mut self, mesures: &mut Mesures) {
        if mesures.cretes.len() != PISTES || mesures.voix.len() != CANAUX {
            mesures.cretes.resize(PISTES, [0.0; 2]);
            mesures.voix.resize(CANAUX, 0);
        }
        for (piste, cumul) in self.pistes.iter_mut().zip(mesures.cretes.iter_mut()) {
            *cumul = [cumul[0].max(piste.crete[0]), cumul[1].max(piste.crete[1])];
            piste.crete = [0.0; 2];
        }
        for (canal, voix) in self.canaux.iter().zip(mesures.voix.iter_mut()) {
            let tenues = canal.tenues.iter().filter(|&&t| t).count() as u32;
            *voix = if tenues > 0 { tenues } else { u32::from(canal.sonne) };
        }
    }

    /// Crêtes depuis la dernière lecture (puis remises à zéro) et voix de chaque canal.
    pub fn mesurer(&mut self, mesures: &mut Mesures) {
        mesures.cretes.clear();
        for piste in self.pistes.iter_mut() {
            mesures.cretes.push(piste.crete);
            piste.crete = [0.0; 2];
        }
        mesures.voix.clear();
        for canal in self.canaux.iter() {
            let tenues = canal.tenues.iter().filter(|&&t| t).count() as u32;
            mesures.voix.push(if tenues > 0 { tenues } else { u32::from(canal.sonne) });
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::fs::File;
    use std::sync::OnceLock;

    pub fn banque() -> Arc<SoundFont> {
        static BANQUE: OnceLock<Arc<SoundFont>> = OnceLock::new();
        BANQUE
            .get_or_init(|| {
                let chemin = concat!(env!("CARGO_MANIFEST_DIR"), "/../src-tauri/banques/GeneralUser-GS.sf2");
                Arc::new(SoundFont::new(&mut File::open(chemin).expect("banque introuvable")).expect("banque illisible"))
            })
            .clone()
    }

    pub const FE: u32 = 44100;

    /// Un canal (piano par défaut) routé sur la piste 1.
    pub fn configuration(programme: u8, batterie: bool) -> Configuration {
        let mut c = Configuration::default();
        c.canaux[0] = ReglageCanal { actif: true, programme, batterie, volume: 0.8, pan: 0.0, piste: 1 };
        c.pistes[1] = ReglagePiste { existe: true, ..Default::default() };
        c
    }

    pub fn rendre(noyau: &mut Noyau, secondes: f32) -> (Vec<f32>, Vec<f32>) {
        let n = (secondes * FE as f32) as usize;
        let (mut g, mut d) = (vec![0.0; n], vec![0.0; n]);
        // en petits morceaux irréguliers, comme une carte son
        let mut fait = 0;
        for taille in [128usize, 441, 1000, 2048].iter().cycle() {
            if fait >= n {
                break;
            }
            let fin = (fait + taille).min(n);
            noyau.rendre(&mut g[fait..fin], &mut d[fait..fin]);
            fait = fin;
        }
        (g, d)
    }

    pub fn note(noyau: &mut Noyau, secondes: f64, hauteur: u8, duree: f64) {
        let debut = noyau.echantillon(secondes);
        let fin = noyau.echantillon(secondes + duree);
        noyau.programmer(Evenement { echantillon: debut, canal: 0, message: Message::NoteOn { hauteur, velocite: 100 } });
        noyau.programmer(Evenement { echantillon: fin, canal: 0, message: Message::NoteOff { hauteur } });
    }

    fn efficace(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    /// Hauteur (numéro MIDI) par autocorrélation, sur une fenêtre de 0,2 s.
    pub fn hauteur(x: &[f32], debut: usize) -> f32 {
        let n = (0.2 * FE as f32) as usize;
        let (min, max) = (FE as usize / 2000, FE as usize / 25);
        let r: Vec<f32> = (min..max).map(|lag| (0..n).map(|i| x[debut + i] * x[debut + i + lag]).sum()).collect();
        let plus_haut = r.iter().cloned().fold(f32::MIN, f32::max);
        let pic = (1..r.len() - 1).find(|&i| r[i] >= 0.8 * plus_haut && r[i] >= r[i - 1] && r[i] >= r[i + 1]).unwrap();
        let (a, b, c) = (r[pic - 1], r[pic], r[pic + 1]);
        let decalage = if a - 2.0 * b + c != 0.0 { 0.5 * (a - c) / (a - 2.0 * b + c) } else { 0.0 };
        let frequence = FE as f32 / ((min + pic) as f32 + decalage);
        69.0 + 12.0 * (frequence / 440.0).log2()
    }

    #[test]
    fn une_note_juste_a_son_instant() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        noyau.configurer(&configuration(0, false));
        note(&mut noyau, 0.25, 69, 1.0);
        let (g, _) = rendre(&mut noyau, 1.5);
        let quart = (0.25 * FE as f32) as usize;
        assert!(efficace(&g[..quart - 64]) < 1e-5, "silence avant la note");
        assert!(efficace(&g[quart..quart + 4410]) > 0.01, "la note sonne à son instant");
        let h = hauteur(&g, quart + 4410);
        assert!((h - 69.0).abs() < 0.2, "la 440 Hz, mesuré {h}");
    }

    #[test]
    fn batterie_sur_un_canal_quelconque() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        let mut c = configuration(0, true);
        c.canaux[5] = c.canaux[0];
        c.canaux[0].actif = false;
        noyau.configurer(&c);
        noyau.programmer(Evenement { echantillon: 0, canal: 5, message: Message::NoteOn { hauteur: 38, velocite: 110 } });
        let (g, _) = rendre(&mut noyau, 0.3);
        assert!(efficace(&g) > 0.01, "la caisse claire sonne");
    }

    #[test]
    fn tout_couper_annule_le_programme() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        noyau.configurer(&configuration(48, false));
        note(&mut noyau, 0.0, 60, 2.0);
        note(&mut noyau, 0.5, 64, 1.0);
        rendre(&mut noyau, 0.2);
        noyau.tout_couper();
        let (g, _) = rendre(&mut noyau, 1.0);
        assert!(efficace(&g[441..]) < 1e-4, "plus rien après l'arrêt");
    }

    #[test]
    fn fader_muet_et_panoramique_de_piste() {
        let niveau = |reglage: ReglagePiste| {
            let mut noyau = Noyau::new(&banque(), FE).unwrap();
            let mut c = configuration(0, false);
            c.pistes[1] = ReglagePiste { existe: true, ..reglage };
            noyau.configurer(&c);
            note(&mut noyau, 0.0, 60, 1.0);
            let (g, d) = rendre(&mut noyau, 0.5);
            (efficace(&g), efficace(&d))
        };
        let normal = niveau(ReglagePiste::default());
        let moitie = niveau(ReglagePiste { volume: 0.4, ..Default::default() });
        assert!((moitie.0 / normal.0 - 0.25).abs() < 0.02, "fader à 0,4 = -12 dB");
        assert_eq!(niveau(ReglagePiste { audible: false, ..Default::default() }), (0.0, 0.0));
        let gauche = niveau(ReglagePiste { pan: -1.0, ..Default::default() });
        assert!(gauche.1 < 1e-6 && gauche.0 > normal.0);
    }

    #[test]
    fn mesures_cretes_et_voix() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        noyau.configurer(&configuration(0, false));
        note(&mut noyau, 0.0, 60, 1.0);
        rendre(&mut noyau, 0.3);
        let mut m = Mesures::default();
        noyau.mesurer(&mut m);
        assert_eq!(m.voix[0], 1);
        assert_eq!(m.voix[1], 0);
        assert!(m.cretes[1][0] > 0.01 && m.cretes[0][0] > 0.01);
        noyau.mesurer(&mut m);
        assert_eq!(m.cretes[1], [0.0, 0.0], "remises à zéro après lecture");
    }
}
