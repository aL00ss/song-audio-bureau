//! Graphe audio du moteur, sans limite de taille : des canaux (chacun joue une *source* : lecteur
//! SoundFont, synthé intégré, demain greffons) qui alimentent des pistes de mixage, puis le
//! maître. Indépendant de la carte son : il sert au rendu en direct (appelé par le fil audio)
//! comme au rendu hors ligne (export, tests).
//!
//! Règles du temps réel : dans `rendre`, `programmer`, `configurer`, `installer_*` et
//! `echanger_stockage`, aucune allocation ni attente. Les canaux et les pistes sont construits
//! ailleurs (`Canal::new`, `Piste::new`) et installés tout prêts ; quand les listes doivent
//! grandir, des listes plus grandes, préparées ailleurs, sont échangées.
//!
//! Coût : un canal sans note et dont le son s'est éteint s'endort et ne coûte plus rien ; les
//! canaux éveillés sont rendus en parallèle sur tous les cœurs.

use std::sync::Arc;

use rustysynth::SoundFont;

use crate::ateliers::Ateliers;
use crate::effets::{Biquad, Limiteur, Reverb, TypeFiltre, gain_du_fader, panoramique};
use crate::source::{Source, TYPES_SOURCES, TypeSource, sources_integrees};

/// Plus long bloc rendu d'un coup ; au-delà, le rendu est découpé.
pub const BLOC_MAX: usize = 1024;
/// Événements en attente au plus (au-delà, les plus lointains sont ignorés).
const FILE_MAX: usize = 65536;
/// En dessous de ce nombre de canaux éveillés, le rendu reste sur le fil audio.
const SEUIL_PARALLELE: usize = 4;
/// Blocs silencieux d'affilée avant qu'un canal sans note ne s'endorme.
const BLOCS_AVANT_SOMMEIL: u32 = 4;
const SILENCE: f32 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Message {
    NoteOn { hauteur: u8, velocite: u8 },
    /// Note avec panoramique (-100 à 100) et réglage fin (cents) qui lui sont propres.
    NoteOnExpressive { hauteur: u8, velocite: u8, pan: i8, cents: i16 },
    NoteOff { hauteur: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Evenement {
    /// Instant, en échantillons depuis le démarrage du moteur.
    pub echantillon: u64,
    /// Numéro du canal dans le graphe (sans limite).
    pub canal: u32,
    pub message: Message,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReglageCanal {
    pub actif: bool,
    pub programme: u8,
    pub batterie: bool,
    /// Volume du canal (0 à 1).
    pub volume: f32,
    /// Panoramique du canal (-1 à 1).
    pub pan: f32,
    /// Piste de mixage qui reçoit ce canal (0 = maître).
    pub piste: u32,
    /// Ce qui produit le son du canal.
    pub source: TypeSource,
}

impl Default for ReglageCanal {
    fn default() -> Self {
        ReglageCanal { actif: false, programme: 0, batterie: false, volume: 0.8, pan: 0.0, piste: 0, source: TypeSource::SoundFont }
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

/// Réglages de tous les canaux et de toutes les pistes ; l'indice est leur numéro.
#[derive(Clone, Debug, PartialEq)]
pub struct Configuration {
    pub canaux: Vec<ReglageCanal>,
    pub pistes: Vec<ReglagePiste>,
}

impl Default for Configuration {
    /// Aucun canal, le maître seul.
    fn default() -> Self {
        Configuration { canaux: Vec::new(), pistes: vec![ReglagePiste { existe: true, ..Default::default() }] }
    }
}

impl Configuration {
    /// `canaux` canaux inactifs et `pistes` pistes (le maître existe toujours).
    pub fn avec(canaux: usize, pistes: usize) -> Self {
        let mut c = Configuration { canaux: vec![ReglageCanal::default(); canaux], pistes: vec![ReglagePiste::default(); pistes.max(1)] };
        c.pistes[0].existe = true;
        c
    }
}

/// Mesures pour l'interface : crête de chaque piste et voix de chaque canal.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesures {
    pub cretes: Vec<[f32; 2]>,
    pub voix: Vec<u32>,
}

pub struct Canal {
    /// Une source de chaque type intégré ; seule celle du réglage joue.
    sources: [Box<dyn Source>; TYPES_SOURCES],
    reglage: ReglageCanal,
    gauche: Vec<f32>,
    droite: Vec<f32>,
    /// Crête du dernier bloc rendu.
    crete: f32,
    blocs_silencieux: u32,
    endormi: bool,
}

impl Canal {
    /// Construit un canal (hors du fil audio : voix, tampons, synthétiseurs).
    pub fn new(banque: &Arc<SoundFont>, frequence: u32) -> Result<Box<Canal>, String> {
        Ok(Box::new(Canal {
            sources: sources_integrees(banque, frequence)?,
            reglage: ReglageCanal::default(),
            gauche: vec![0.0; BLOC_MAX],
            droite: vec![0.0; BLOC_MAX],
            crete: 0.0,
            blocs_silencieux: 0,
            endormi: true,
        }))
    }

    fn source(&mut self) -> &mut dyn Source {
        self.sources[self.reglage.source.index()].as_mut()
    }

    fn appliquer(&mut self, reglage: ReglageCanal) {
        if reglage.source != self.reglage.source {
            self.source().tout_couper();
        }
        let premier = !self.reglage.actif && reglage.actif;
        if reglage != self.reglage || premier {
            self.reglage = reglage;
            self.source().regler(reglage.programme, reglage.batterie, reglage.volume, reglage.pan);
        }
    }

    fn reveiller(&mut self) {
        self.endormi = false;
        self.blocs_silencieux = 0;
    }

    fn voix(&self) -> u32 {
        let voix = self.sources[self.reglage.source.index()].voix();
        if voix > 0 { voix } else { u32::from(!self.endormi && self.crete > 1e-4) }
    }

    /// Rend `n` échantillons de la source dans les tampons du canal.
    fn rendre_source(&mut self, n: usize) {
        let index = self.reglage.source.index();
        let (g, d) = (&mut self.gauche[..n], &mut self.droite[..n]);
        self.sources[index].rendre(g, d);
        self.crete = g.iter().chain(d.iter()).fold(0.0f32, |m, v| m.max(v.abs()));
    }
}

pub struct Piste {
    reglage: ReglagePiste,
    filtres: [Biquad; 3],
    gauche: Vec<f32>,
    droite: Vec<f32>,
    crete: [f32; 2],
    /// Reçoit au moins un canal éveillé dans ce bloc (sinon, rien à traiter).
    utilisee: bool,
}

impl Piste {
    pub fn new() -> Box<Piste> {
        Box::new(Piste {
            reglage: ReglagePiste::default(),
            filtres: [Biquad::neutre(); 3],
            gauche: vec![0.0; BLOC_MAX],
            droite: vec![0.0; BLOC_MAX],
            crete: [0.0; 2],
            utilisee: false,
        })
    }
}

/// Les listes du graphe, préparées hors du fil audio pour l'agrandir.
pub struct Stockage {
    pub canaux: Vec<Option<Box<Canal>>>,
    pub pistes: Vec<Option<Box<Piste>>>,
    pub eveilles: Vec<u32>,
}

impl Stockage {
    pub fn avec_capacite(canaux: usize, pistes: usize) -> Self {
        Stockage { canaux: Vec::with_capacity(canaux), pistes: Vec::with_capacity(pistes), eveilles: Vec::with_capacity(canaux) }
    }
}

pub struct Noyau {
    frequence: u32,
    banque: Arc<SoundFont>,
    /// Échantillons déjà rendus : l'horloge du moteur.
    horloge: u64,
    canaux: Vec<Option<Box<Canal>>>,
    pistes: Vec<Option<Box<Piste>>>,
    /// Canaux à rendre dans le bloc en cours (liste réutilisée, sans allocation).
    eveilles: Vec<u32>,
    file: Vec<Evenement>,
    reverb: Reverb,
    limiteur: Limiteur,
    envoi: [Vec<f32>; 2],
    /// Clics du métronome à faire entendre : (début en échantillons, accent).
    clics: Vec<(u64, bool)>,
    ateliers: Ateliers,
}

/// Clics en attente au plus (sans allocation dans le fil audio).
const CLICS_MAX: usize = 32;
const CLIC_DUREE: f32 = 0.06;
const CLIC_NIVEAU: f32 = 0.35;

/// Contexte du rendu parallèle : la liste des canaux et celle des canaux éveillés, plus la
/// longueur du bloc. Valide pendant tout l'appel à `Ateliers::executer`.
struct ContexteRendu {
    canaux: *mut Option<Box<Canal>>,
    eveilles: *const u32,
    n: usize,
}

/// Rend la source d'un canal éveillé. Chaque indice désigne un canal différent : aucun canal
/// n'est touché par deux fils à la fois.
unsafe fn rendre_un_canal(contexte: *const (), index: usize) {
    unsafe {
        let c = &*(contexte as *const ContexteRendu);
        let numero = *c.eveilles.add(index) as usize;
        if let Some(canal) = (*c.canaux.add(numero)).as_mut() {
            canal.rendre_source(c.n);
        }
    }
}

impl Noyau {
    pub fn new(banque: &Arc<SoundFont>, frequence: u32) -> Result<Self, String> {
        let mut noyau = Noyau {
            frequence,
            banque: banque.clone(),
            horloge: 0,
            canaux: Vec::with_capacity(64),
            pistes: Vec::with_capacity(64),
            eveilles: Vec::with_capacity(64),
            file: Vec::with_capacity(FILE_MAX),
            reverb: Reverb::new(frequence as f32),
            limiteur: Limiteur::new(frequence as f32),
            envoi: [vec![0.0; BLOC_MAX], vec![0.0; BLOC_MAX]],
            clics: Vec::with_capacity(CLICS_MAX),
            ateliers: Ateliers::new(31),
        };
        noyau.installer_piste(0, Piste::new());
        noyau.configurer(&Configuration::default());
        Ok(noyau)
    }

    pub fn frequence(&self) -> u32 {
        self.frequence
    }

    pub fn horloge(&self) -> u64 {
        self.horloge
    }

    pub fn fils_de_calcul(&self) -> usize {
        self.ateliers.nombre_de_fils()
    }

    /// Convertit un instant en secondes (horloge du moteur) en échantillons.
    pub fn echantillon(&self, secondes: f64) -> u64 {
        (secondes.max(0.0) * self.frequence as f64).round() as u64
    }

    pub fn capacite(&self) -> (usize, usize) {
        (self.canaux.capacity(), self.pistes.capacity())
    }

    pub fn canaux_installes(&self) -> usize {
        self.canaux.len()
    }

    pub fn pistes_installees(&self) -> usize {
        self.pistes.len()
    }

    // ---------------------------------------------------------------- structure du graphe

    /// Remplace les listes par d'autres, plus grandes, préparées ailleurs ; les canaux et pistes
    /// y sont déplacés (pas de copie, pas d'allocation). Renvoie les anciennes listes (vides) à
    /// libérer hors du fil audio.
    pub fn echanger_stockage(&mut self, mut nouveau: Stockage) -> Stockage {
        nouveau.canaux.extend(self.canaux.drain(..).take(nouveau.canaux.capacity()));
        nouveau.pistes.extend(self.pistes.drain(..).take(nouveau.pistes.capacity()));
        Stockage {
            canaux: std::mem::replace(&mut self.canaux, nouveau.canaux),
            pistes: std::mem::replace(&mut self.pistes, nouveau.pistes),
            eveilles: std::mem::replace(&mut self.eveilles, nouveau.eveilles),
        }
    }

    /// Installe un canal à son numéro (dans la capacité déjà réservée). Renvoie l'ancien canal,
    /// ou le nouveau s'il n'y a pas la place (rien n'est perdu ni alloué).
    pub fn installer_canal(&mut self, numero: u32, canal: Box<Canal>) -> Option<Box<Canal>> {
        let numero = numero as usize;
        if numero >= self.canaux.capacity() {
            return Some(canal);
        }
        if numero >= self.canaux.len() {
            self.canaux.resize_with(numero + 1, || None);
        }
        self.canaux[numero].replace(canal)
    }

    /// Donne un instrument d'échantillons au canal ; rend l'ancien (à libérer hors du fil audio).
    pub fn changer_instrument(&mut self, numero: u32, instrument: Option<Arc<crate::echantillonneur::InstrumentEchantillons>>) -> Option<Arc<crate::echantillonneur::InstrumentEchantillons>> {
        match self.canaux.get_mut(numero as usize) {
            Some(Some(canal)) => canal.sources[TypeSource::Echantillons.index()].changer_instrument(instrument),
            _ => instrument,
        }
    }

    pub fn installer_piste(&mut self, numero: u32, piste: Box<Piste>) -> Option<Box<Piste>> {
        let numero = numero as usize;
        if numero >= self.pistes.capacity() {
            return Some(piste);
        }
        if numero >= self.pistes.len() {
            self.pistes.resize_with(numero + 1, || None);
        }
        self.pistes[numero].replace(piste)
    }

    /// Hors du fil audio (rendu hors ligne, tests) : construit tout ce qu'il faut pour cette
    /// configuration (avec allocation).
    pub fn preparer(&mut self, configuration: &Configuration) -> Result<(), String> {
        let (canaux, pistes) = (configuration.canaux.len(), configuration.pistes.len());
        if canaux > self.canaux.capacity() || pistes > self.pistes.capacity() {
            let stockage = Stockage::avec_capacite(canaux.max(self.canaux.capacity()) * 2, pistes.max(self.pistes.capacity()) * 2);
            self.echanger_stockage(stockage);
        }
        for i in self.canaux.len()..canaux {
            self.installer_canal(i as u32, Canal::new(&self.banque, self.frequence)?);
        }
        for i in self.pistes.len()..pistes {
            self.installer_piste(i as u32, Piste::new());
        }
        Ok(())
    }

    pub fn configurer(&mut self, configuration: &Configuration) {
        for (i, canal) in self.canaux.iter_mut().enumerate() {
            let Some(canal) = canal else { continue };
            let reglage = configuration.canaux.get(i).copied().unwrap_or_default();
            if !reglage.actif && canal.reglage.actif {
                canal.source().tout_couper();
                canal.endormi = true;
            }
            canal.appliquer(reglage);
        }
        let fe = self.frequence as f32;
        for (i, piste) in self.pistes.iter_mut().enumerate() {
            let Some(piste) = piste else { continue };
            let mut reglage = configuration.pistes.get(i).copied().unwrap_or_default();
            if i == 0 {
                reglage.existe = true;
            }
            if piste.reglage.eq != reglage.eq || !piste.reglage.existe {
                piste.filtres[0].regler(TypeFiltre::PlateauGrave, 200.0, reglage.eq[0], fe);
                piste.filtres[1].regler(TypeFiltre::Cloche(0.8), 1200.0, reglage.eq[1], fe);
                piste.filtres[2].regler(TypeFiltre::PlateauAigu, 5000.0, reglage.eq[2], fe);
            }
            piste.reglage = reglage;
        }
    }

    // ---------------------------------------------------------------- événements

    /// Ajoute un événement à la file (gardée triée ; à instant égal, l'ordre d'arrivée).
    pub fn programmer(&mut self, evenement: Evenement) {
        if self.file.len() >= FILE_MAX {
            return;
        }
        let position = self.file.partition_point(|e| e.echantillon <= evenement.echantillon);
        self.file.insert(position, evenement);
    }

    /// Programme un clic de métronome (plus aigu sur l'accent).
    pub fn clic(&mut self, echantillon: u64, accent: bool) {
        if self.clics.len() < CLICS_MAX {
            self.clics.push((echantillon, accent));
        }
    }

    /// Oublie les clics de métronome pas encore entendus (à l'arrêt du transport).
    pub fn couper_clics(&mut self) {
        self.clics.clear();
    }

    /// Coupe net ce qui sonne et oublie ce qui était programmé.
    pub fn tout_couper(&mut self) {
        self.file.clear();
        self.clics.clear();
        for canal in self.canaux.iter_mut().flatten() {
            for source in canal.sources.iter_mut() {
                source.tout_couper();
            }
        }
    }

    fn executer(&mut self, evenement: Evenement) {
        let Some(Some(canal)) = self.canaux.get_mut(evenement.canal as usize) else { return };
        if !canal.reglage.actif {
            return;
        }
        match evenement.message {
            Message::NoteOn { hauteur, velocite } => {
                canal.reveiller();
                canal.source().note_on(hauteur, velocite);
            }
            Message::NoteOnExpressive { hauteur, velocite, pan, cents } => {
                canal.reveiller();
                canal.source().note_on_expressive(hauteur, velocite, pan as f32 / 100.0, cents as f32 / 100.0);
            }
            Message::NoteOff { hauteur } => canal.source().note_off(hauteur),
        }
    }

    // ---------------------------------------------------------------- rendu

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
        // 1. les canaux éveillés : leur source rend, en parallèle s'ils sont nombreux
        self.eveilles.clear();
        for (i, canal) in self.canaux.iter().enumerate() {
            if let Some(c) = canal {
                if c.reglage.actif && !c.endormi && self.eveilles.len() < self.eveilles.capacity() {
                    self.eveilles.push(i as u32);
                }
            }
        }
        if self.eveilles.len() >= SEUIL_PARALLELE && self.ateliers.nombre_de_fils() > 0 {
            let contexte = ContexteRendu { canaux: self.canaux.as_mut_ptr(), eveilles: self.eveilles.as_ptr(), n };
            // SÛRETÉ : chaque indice désigne un canal distinct ; les listes ne bougent pas
            // pendant l'appel, qui ne rend la main qu'une fois tout fini.
            unsafe { self.ateliers.executer(&contexte as *const ContexteRendu as *const (), rendre_un_canal, self.eveilles.len()) };
        } else {
            for &i in &self.eveilles {
                if let Some(c) = self.canaux[i as usize].as_mut() {
                    c.rendre_source(n);
                }
            }
        }

        // 2. chaque canal dans sa piste ; les canaux silencieux sans note s'endorment
        for piste in self.pistes.iter_mut().flatten() {
            piste.utilisee = false;
        }
        for &i in &self.eveilles {
            let Some(canal) = self.canaux[i as usize].as_mut() else { continue };
            let voulue = canal.reglage.piste as usize;
            let index = match self.pistes.get(voulue) {
                Some(Some(p)) if p.reglage.existe => voulue,
                _ => 0,
            };
            let Some(piste) = self.pistes[index].as_mut() else { continue };
            if !piste.utilisee {
                piste.gauche[..n].fill(0.0);
                piste.droite[..n].fill(0.0);
                piste.utilisee = true;
            }
            for k in 0..n {
                piste.gauche[k] += canal.gauche[k];
                piste.droite[k] += canal.droite[k];
            }
            let sans_note = canal.sources[canal.reglage.source.index()].voix() == 0;
            if sans_note && canal.crete < SILENCE {
                canal.blocs_silencieux += 1;
                if canal.blocs_silencieux >= BLOCS_AVANT_SOMMEIL {
                    canal.endormi = true;
                }
            } else {
                canal.blocs_silencieux = 0;
            }
        }

        // 3. les pistes : égaliseur, fader, panoramique, muet, envoi réverbération
        self.envoi[0][..n].fill(0.0);
        self.envoi[1][..n].fill(0.0);
        let (maitre, autres) = self.pistes.split_at_mut(1);
        let maitre = maitre[0].as_mut().expect("le maître existe toujours");
        if !maitre.utilisee {
            maitre.gauche[..n].fill(0.0);
            maitre.droite[..n].fill(0.0);
        }
        for piste in autres.iter_mut().flatten() {
            if !piste.reglage.existe || !piste.utilisee {
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

        // 4. le maître : réverbération, fader (coupé si le maître est muet), limiteur
        let gain = if maitre.reglage.audible { gain_du_fader(maitre.reglage.volume) } else { 0.0 };
        let mut crete = [0f32; 2];
        for i in 0..n {
            let (rg, rd) = self.reverb.traiter(self.envoi[0][i], self.envoi[1][i]);
            let (g, d) = self.limiteur.traiter((maitre.gauche[i] + rg) * gain, (maitre.droite[i] + rd) * gain);
            gauche[i] = g;
            droite[i] = d;
            crete = [crete[0].max(g.abs()), crete[1].max(d.abs())];
        }
        maitre.utilisee = false;

        // 5. le métronome, par-dessus (il ne passe pas par le limiteur)
        if !self.clics.is_empty() {
            let fe = self.frequence as f32;
            let duree = (CLIC_DUREE * fe) as u64;
            for &(debut, accent) in self.clics.iter() {
                let frequence = if accent { 1760.0 } else { 1320.0 };
                for i in 0..n {
                    let t = self.horloge + i as u64;
                    if t < debut || t >= debut + duree {
                        continue;
                    }
                    let x = (t - debut) as f32 / fe;
                    let v = CLIC_NIVEAU * (2.0 * std::f32::consts::PI * frequence * x).sin() * (-x / 0.012).exp();
                    gauche[i] = (gauche[i] + v).clamp(-1.0, 1.0);
                    droite[i] = (droite[i] + v).clamp(-1.0, 1.0);
                    // le vumètre du maître montre ce qu'on entend, métronome compris
                    crete = [crete[0].max(gauche[i].abs()), crete[1].max(droite[i].abs())];
                }
            }
            let fin = self.horloge + n as u64;
            self.clics.retain(|&(debut, _)| debut + duree > fin);
        }
        maitre.crete = [maitre.crete[0].max(crete[0]), maitre.crete[1].max(crete[1])];
        self.horloge += n as u64;
    }

    // ---------------------------------------------------------------- mesures

    /// Ajoute les crêtes du moment à celles déjà accumulées dans `mesures` (le maximum est
    /// gardé jusqu'à ce que l'interface les lise) et met à jour les voix. Sans allocation :
    /// `mesures` est dimensionné par l'interface ; ce qui dépasse est ignoré.
    pub fn cumuler_mesures(&mut self, mesures: &mut Mesures) {
        for (piste, cumul) in self.pistes.iter_mut().zip(mesures.cretes.iter_mut()) {
            if let Some(p) = piste {
                *cumul = [cumul[0].max(p.crete[0]), cumul[1].max(p.crete[1])];
                p.crete = [0.0; 2];
            }
        }
        for (canal, voix) in self.canaux.iter().zip(mesures.voix.iter_mut()) {
            *voix = canal.as_ref().map(|c| c.voix()).unwrap_or(0);
        }
    }

    /// Crêtes depuis la dernière lecture (puis remises à zéro) et voix de chaque canal (avec
    /// allocation : hors du fil audio).
    pub fn mesurer(&mut self, mesures: &mut Mesures) {
        mesures.cretes = vec![[0.0; 2]; self.pistes.len()];
        mesures.voix = vec![0; self.canaux.len()];
        self.cumuler_mesures(mesures);
    }

    /// Canaux éveillés (pour les tests et les indicateurs de charge).
    pub fn canaux_eveilles(&self) -> usize {
        self.canaux.iter().flatten().filter(|c| c.reglage.actif && !c.endormi).count()
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
                let chemin = concat!(env!("CARGO_MANIFEST_DIR"), "/../ressources/banques/GeneralUser-GS.sf2");
                Arc::new(SoundFont::new(&mut File::open(chemin).expect("banque introuvable")).expect("banque illisible"))
            })
            .clone()
    }

    pub const FE: u32 = 44100;

    /// Un canal (piano par défaut) routé sur la piste 1.
    pub fn configuration(programme: u8, batterie: bool) -> Configuration {
        let mut c = Configuration::avec(1, 2);
        c.canaux[0] = ReglageCanal { actif: true, programme, batterie, volume: 0.8, pan: 0.0, piste: 1, source: TypeSource::SoundFont };
        c.pistes[1] = ReglagePiste { existe: true, ..Default::default() };
        c
    }

    /// Prépare (hors temps réel) puis applique une configuration.
    pub fn regler(noyau: &mut Noyau, c: &Configuration) {
        noyau.preparer(c).unwrap();
        noyau.configurer(c);
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
        note_sur(noyau, 0, secondes, hauteur, duree);
    }

    pub fn note_sur(noyau: &mut Noyau, canal: u32, secondes: f64, hauteur: u8, duree: f64) {
        let debut = noyau.echantillon(secondes);
        let fin = noyau.echantillon(secondes + duree);
        noyau.programmer(Evenement { echantillon: debut, canal, message: Message::NoteOn { hauteur, velocite: 100 } });
        noyau.programmer(Evenement { echantillon: fin, canal, message: Message::NoteOff { hauteur } });
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
        regler(&mut noyau, &configuration(0, false));
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
        let mut c = Configuration::avec(6, 2);
        c.canaux[5] = configuration(0, true).canaux[0];
        c.pistes[1].existe = true;
        regler(&mut noyau, &c);
        noyau.programmer(Evenement { echantillon: 0, canal: 5, message: Message::NoteOn { hauteur: 38, velocite: 110 } });
        let (g, _) = rendre(&mut noyau, 0.3);
        assert!(efficace(&g) > 0.01, "la caisse claire sonne");
    }

    #[test]
    fn tout_couper_annule_le_programme() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        regler(&mut noyau, &configuration(48, false));
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
            regler(&mut noyau, &c);
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
    fn maitre_muet_plus_rien_ne_sort() {
        let niveau = |audible: bool| {
            let mut noyau = Noyau::new(&banque(), FE).unwrap();
            let mut c = configuration(0, false);
            c.pistes[0].audible = audible;
            regler(&mut noyau, &c);
            note(&mut noyau, 0.0, 60, 1.0);
            let (g, d) = rendre(&mut noyau, 0.3);
            (efficace(&g), efficace(&d))
        };
        assert!(niveau(true).0 > 0.001);
        assert_eq!(niveau(false), (0.0, 0.0));
    }

    #[test]
    fn mesures_cretes_et_voix() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        let mut c = configuration(0, false);
        c.canaux.push(ReglageCanal::default());
        regler(&mut noyau, &c);
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

    #[test]
    fn le_graphe_grandit_sans_limite_et_les_canaux_s_endorment() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        // 300 canaux, 300 pistes : bien au-delà des anciens 32 canaux
        let mut c = Configuration::avec(300, 301);
        for i in 0..300 {
            c.canaux[i] = ReglageCanal { actif: true, programme: (i % 100) as u8, piste: (i + 1) as u32, ..Default::default() };
            c.pistes[i + 1].existe = true;
        }
        regler(&mut noyau, &c);
        assert_eq!(noyau.canaux_installes(), 300);
        // au repos, tout dort : rien ne coûte
        rendre(&mut noyau, 0.1);
        assert_eq!(noyau.canaux_eveilles(), 0);
        // une note sur le 250e canal le réveille et sonne sur sa piste
        note_sur(&mut noyau, 249, 0.0, 60, 0.3);
        rendre(&mut noyau, 0.2);
        assert_eq!(noyau.canaux_eveilles(), 1);
        let mut m = Mesures::default();
        noyau.mesurer(&mut m);
        assert!(m.cretes[250][0] > 0.01, "piste 250 : {:?}", m.cretes[250]);
        assert_eq!(m.voix[249], 1);
        // une fois la note éteinte, il se rendort
        rendre(&mut noyau, 3.0);
        assert_eq!(noyau.canaux_eveilles(), 0);
    }

    #[test]
    fn rendu_parallele_identique_au_rendu_sequentiel() {
        let rendu = |fils: usize| {
            let mut noyau = Noyau::new(&banque(), FE).unwrap();
            noyau.ateliers = Ateliers::avec_fils(fils);
            let mut c = Configuration::avec(40, 41);
            for i in 0..40 {
                c.canaux[i] = ReglageCanal { actif: true, programme: (i * 3 % 100) as u8, piste: (i + 1) as u32, volume: 0.5, ..Default::default() };
                c.pistes[i + 1].existe = true;
            }
            regler(&mut noyau, &c);
            for i in 0..40u32 {
                note_sur(&mut noyau, i, 0.01 * i as f64, 48 + (i % 24) as u8, 0.4);
            }
            rendre(&mut noyau, 0.8).0
        };
        assert_eq!(rendu(0), rendu(3), "le partage du travail ne change pas le son");
    }
}
