//! Les sources de son d'un canal. Un canal joue la source choisie dans son réglage ; chaque
//! canal possède à l'avance une source de chaque type intégré, si bien que changer de type ne
//! demande aucune allocation dans le fil audio. Les greffons (CLAP, VST3) seront une source de
//! plus, dans un processus séparé.

use std::sync::Arc;

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use crate::echantillonneur::InstrumentEchantillons;

/// Ce qu'un instrument doit savoir faire pour jouer dans un canal.
pub trait Source: Send {
    /// Programme (instrument ou préréglage), batterie, volume (0 à 1), panoramique (-1 à 1).
    fn regler(&mut self, programme: u8, batterie: bool, volume: f32, pan: f32);
    fn note_on(&mut self, hauteur: u8, velocite: u8);
    fn note_off(&mut self, hauteur: u8);
    /// Coupe net toutes les voix.
    fn tout_couper(&mut self);
    /// Voix qui sonnent (pour les témoins d'activité) ; 0 si la source ne sait pas les compter.
    fn voix(&self) -> u32 {
        0
    }
    /// Remplace le contenu de `gauche` et `droite` par la suite du son.
    fn rendre(&mut self, gauche: &mut [f32], droite: &mut [f32]);
    /// Change l'instrument d'échantillons (seul l'échantillonneur en a un) ; rend l'ancien, qui
    /// doit être libéré hors du fil audio.
    fn changer_instrument(&mut self, instrument: Option<Arc<InstrumentEchantillons>>) -> Option<Arc<InstrumentEchantillons>> {
        instrument
    }
}

/// Types de sources intégrées, dans l'ordre où chaque canal les range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TypeSource {
    /// Banque SoundFont 2 (General MIDI : 128 instruments et les batteries).
    #[default]
    SoundFont,
    /// Synthétiseur soustractif intégré (préréglages choisis par le programme).
    Soustractif,
    /// Échantillonneur (instrument SFZ donné au canal à part).
    Echantillons,
}

pub const TYPES_SOURCES: usize = 3;

impl TypeSource {
    pub fn index(self) -> usize {
        match self {
            TypeSource::SoundFont => 0,
            TypeSource::Soustractif => 1,
            TypeSource::Echantillons => 2,
        }
    }
}

/// Crée une source de chaque type intégré, rangées selon `TypeSource::index`.
pub fn sources_integrees(banque: &Arc<SoundFont>, frequence: u32) -> Result<[Box<dyn Source>; TYPES_SOURCES], String> {
    Ok([
        Box::new(LecteurSoundFont::new(banque, frequence)?),
        Box::new(crate::soustractif::Soustractif::new(frequence)),
        Box::new(crate::echantillonneur::Echantillonneur::new(frequence)),
    ])
}

// ------------------------------------------------------------------ lecteur SoundFont

const CANAL_MELODIQUE: i32 = 0;
const CANAL_BATTERIE: i32 = 9;
const NOTE_ON: i32 = 0x90;
const NOTE_OFF: i32 = 0x80;
const CONTROLE: i32 = 0xB0;
const PROGRAMME: i32 = 0xC0;

/// Un synthétiseur rustysynth pour un seul canal : les instruments sur son canal 1, la
/// batterie sur son canal 10 (rustysynth ne joue la batterie que là).
pub struct LecteurSoundFont {
    synth: Synthesizer,
    batterie: bool,
    programme: Option<u8>,
    /// Notes tenues (rustysynth ne donne pas le nombre de ses voix).
    tenues: [bool; 128],
}

impl LecteurSoundFont {
    pub fn new(banque: &Arc<SoundFont>, frequence: u32) -> Result<Self, String> {
        let mut reglages = SynthesizerSettings::new(frequence as i32);
        reglages.enable_reverb_and_chorus = false; // la réverbération est celle du mixer
        reglages.maximum_polyphony = 64;
        // blocs internes de 32 échantillons (0,7 ms) : un événement attend au plus un bloc
        reglages.block_size = 32;
        let synth = Synthesizer::new(banque, &reglages).map_err(|e| format!("synthétiseur : {e:?}"))?;
        Ok(LecteurSoundFont { synth, batterie: false, programme: None, tenues: [false; 128] })
    }

    fn canal(&self) -> i32 {
        if self.batterie { CANAL_BATTERIE } else { CANAL_MELODIQUE }
    }
}

impl Source for LecteurSoundFont {
    fn regler(&mut self, programme: u8, batterie: bool, volume: f32, pan: f32) {
        if batterie != self.batterie {
            self.synth.note_off_all_channel(self.canal(), true);
            self.tenues = [false; 128];
            self.batterie = batterie;
            self.programme = None;
        }
        let canal = self.canal();
        if !batterie && self.programme != Some(programme) {
            self.synth.process_midi_message(canal, PROGRAMME, programme as i32, 0);
            self.programme = Some(programme);
        }
        self.synth.process_midi_message(canal, CONTROLE, 7, (volume.clamp(0.0, 1.0) * 127.0).round() as i32);
        self.synth.process_midi_message(canal, CONTROLE, 10, (((pan.clamp(-1.0, 1.0) + 1.0) / 2.0) * 127.0).round() as i32);
    }

    fn note_on(&mut self, hauteur: u8, velocite: u8) {
        self.synth.process_midi_message(self.canal(), NOTE_ON, hauteur as i32, velocite as i32);
        self.tenues[hauteur as usize & 127] = true;
    }

    fn note_off(&mut self, hauteur: u8) {
        self.synth.process_midi_message(self.canal(), NOTE_OFF, hauteur as i32, 0);
        self.tenues[hauteur as usize & 127] = false;
    }

    fn tout_couper(&mut self) {
        self.synth.note_off_all(true);
        self.tenues = [false; 128];
    }

    fn voix(&self) -> u32 {
        self.tenues.iter().filter(|&&t| t).count() as u32
    }

    fn rendre(&mut self, gauche: &mut [f32], droite: &mut [f32]) {
        self.synth.render(gauche, droite);
    }
}
