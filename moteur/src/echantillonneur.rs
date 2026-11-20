//! L'échantillonneur : joue un instrument fait d'échantillons (lu depuis un fichier SFZ). Le
//! chargement (lecture des WAV et FLAC, préparation des régions) se fait hors du fil audio ; le
//! jeu n'alloue rien : les voix sont réservées d'avance, l'instrument est partagé (Arc).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::sfz::{Boucle, Declenchement, Region, lire_sfz};
use crate::source::Source;

/// Voix jouables en même temps par un canal échantillonneur.
pub const VOIX_MAX: usize = 64;

/// Les valeurs d'un échantillon : gardées en 16 bits quand le fichier l'est (deux fois moins de
/// mémoire, sans rien perdre), en flottants sinon.
#[derive(Debug, PartialEq)]
pub enum Donnees {
    Entiers(Vec<i16>),
    Flottants(Vec<f32>),
}

impl Donnees {
    pub fn len(&self) -> usize {
        match self {
            Donnees::Entiers(v) => v.len(),
            Donnees::Flottants(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Mémoire occupée, en octets.
    pub fn octets(&self) -> usize {
        match self {
            Donnees::Entiers(v) => v.len() * 2,
            Donnees::Flottants(v) => v.len() * 4,
        }
    }

    #[inline]
    fn valeur(&self, i: usize) -> f32 {
        match self {
            Donnees::Entiers(v) => v[i] as f32 * (1.0 / 32768.0),
            Donnees::Flottants(v) => v[i],
        }
    }
}

/// Un échantillon en mémoire : trames entrelacées (1 ou 2 voies), entre -1 et 1.
#[derive(Debug, PartialEq)]
pub struct Echantillon {
    pub frequence: u32,
    pub voies: usize,
    pub donnees: Donnees,
    /// Boucle trouvée dans le fichier (bloc « smpl » d'un WAV), en trames.
    pub boucle: Option<(u64, u64)>,
}

impl Echantillon {
    pub fn trames(&self) -> usize {
        self.donnees.len() / self.voies.max(1)
    }

    /// Valeur (gauche, droite) à une position fractionnaire (interpolation linéaire).
    #[inline]
    fn lire(&self, position: f64) -> (f32, f32) {
        let i = position as usize;
        let f = (position - i as f64) as f32;
        let n = self.trames();
        if i + 1 >= n {
            return if i < n { self.trame(i) } else { (0.0, 0.0) };
        }
        let (a, b) = (self.trame(i), self.trame(i + 1));
        (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f)
    }

    #[inline]
    fn trame(&self, i: usize) -> (f32, f32) {
        if self.voies == 1 {
            let v = self.donnees.valeur(i);
            (v, v)
        } else {
            (self.donnees.valeur(i * self.voies), self.donnees.valeur(i * self.voies + 1))
        }
    }
}

/// Lit un fichier WAV ou FLAC.
pub fn charger_echantillon(chemin: &Path) -> Result<Echantillon, String> {
    let extension = chemin.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let erreur = |e: String| format!("{} : {e}", chemin.display());
    match extension.as_str() {
        "flac" => {
            let mut lecteur = claxon::FlacReader::open(chemin).map_err(|e| erreur(e.to_string()))?;
            let info = lecteur.streaminfo();
            let donnees = if info.bits_per_sample <= 16 {
                let decalage = 16 - info.bits_per_sample;
                Donnees::Entiers(lecteur.samples().map(|s| s.map(|v| (v << decalage) as i16)).collect::<Result<Vec<_>, _>>().map_err(|e| erreur(e.to_string()))?)
            } else {
                let echelle = 1.0 / (1u64 << (info.bits_per_sample - 1)) as f32;
                Donnees::Flottants(lecteur.samples().map(|s| s.map(|v| v as f32 * echelle)).collect::<Result<Vec<_>, _>>().map_err(|e| erreur(e.to_string()))?)
            };
            Ok(Echantillon { frequence: info.sample_rate, voies: info.channels as usize, donnees, boucle: None })
        }
        _ => {
            let octets = std::fs::read(chemin).map_err(|e| erreur(e.to_string()))?;
            let boucle = boucle_wav(&octets);
            let mut lecteur = hound::WavReader::new(std::io::Cursor::new(octets)).map_err(|e| erreur(e.to_string()))?;
            let spec = lecteur.spec();
            let donnees = match spec.sample_format {
                hound::SampleFormat::Float => lecteur.samples::<f32>().collect::<Result<Vec<_>, _>>().map(Donnees::Flottants),
                hound::SampleFormat::Int if spec.bits_per_sample <= 16 => {
                    let decalage = 16 - spec.bits_per_sample;
                    lecteur.samples::<i32>().map(|s| s.map(|v| (v << decalage) as i16)).collect::<Result<Vec<_>, _>>().map(Donnees::Entiers)
                }
                hound::SampleFormat::Int => {
                    let echelle = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
                    lecteur.samples::<i32>().map(|s| s.map(|v| v as f32 * echelle)).collect::<Result<Vec<_>, _>>().map(Donnees::Flottants)
                }
            }
            .map_err(|e| erreur(e.to_string()))?;
            Ok(Echantillon { frequence: spec.sample_rate, voies: spec.channels as usize, donnees, boucle })
        }
    }
}

/// Première boucle du bloc « smpl » d'un WAV, s'il y en a une.
fn boucle_wav(o: &[u8]) -> Option<(u64, u64)> {
    let mut i = 12;
    while i + 8 <= o.len() {
        let taille = u32::from_le_bytes(o[i + 4..i + 8].try_into().ok()?) as usize;
        if &o[i..i + 4] == b"smpl" && taille >= 36 + 24 {
            let b = &o[i + 8..];
            let nombre = u32::from_le_bytes(b[28..32].try_into().ok()?);
            if nombre > 0 {
                let debut = u32::from_le_bytes(b[36 + 8..36 + 12].try_into().ok()?) as u64;
                let fin = u32::from_le_bytes(b[36 + 12..36 + 16].try_into().ok()?) as u64;
                return (fin > debut).then_some((debut, fin));
            }
        }
        i += 8 + taille + taille % 2;
    }
    None
}

/// Une région prête à jouer : ses réglages, son échantillon et son gain fixe (amplitude et
/// contrôleurs à leur valeur de départ).
#[derive(Debug)]
pub struct Zone {
    pub region: Region,
    pub echantillon: Arc<Echantillon>,
    pub gain: f32,
}

/// Un instrument chargé : ses zones, et ses touches de changement d'articulation.
#[derive(Debug, Default)]
pub struct InstrumentEchantillons {
    pub nom: String,
    pub zones: Vec<Zone>,
    pub commutation: Option<(u8, u8)>,
    pub commutateur_defaut: Option<u8>,
}

impl InstrumentEchantillons {
    /// Charge un fichier SFZ et tous ses échantillons (un échantillon partagé par plusieurs
    /// régions n'est lu qu'une fois). Les échantillons manquants sont signalés, pas fatals.
    pub fn depuis_sfz(chemin: &Path) -> Result<(InstrumentEchantillons, Vec<String>), String> {
        let texte = std::fs::read_to_string(chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        let dossier = chemin.parent().unwrap_or(Path::new("."));
        let sfz = lire_sfz(&texte, dossier);
        let controleurs = sfz.controleurs;
        let commutation = sfz.regions.iter().find_map(|r| r.commutation);
        let commutateur_defaut = sfz.regions.iter().find_map(|r| r.commutateur_defaut);
        let mut avertissements = Vec::new();
        let mut lus: HashMap<PathBuf, Arc<Echantillon>> = HashMap::new();
        let mut zones = Vec::new();
        // les régions hors des plages de contrôleurs (à leur valeur de départ) ne jouent pas
        for region in sfz.regions.into_iter().filter(|r| r.dans_les_plages(&controleurs)) {
            let echantillon = match lus.get(&region.echantillon) {
                Some(e) => e.clone(),
                None => match charger_echantillon(&region.echantillon) {
                    Ok(e) => {
                        let e = Arc::new(e);
                        lus.insert(region.echantillon.clone(), e.clone());
                        e
                    }
                    Err(e) => {
                        avertissements.push(e);
                        continue;
                    }
                },
            };
            let gain = region.gain_amplitude(&controleurs);
            zones.push(Zone { region, echantillon, gain });
        }
        if zones.is_empty() {
            return Err(format!("{} : aucun échantillon lisible", chemin.display()));
        }
        let nom = chemin.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        Ok((InstrumentEchantillons { nom, zones, commutation, commutateur_defaut }, avertissements))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Retard,
    Attaque,
    Tenue,
    Declin,
    Maintien,
    Relachement,
}

#[derive(Clone, Copy, Debug)]
struct Voix {
    active: bool,
    zone: usize,
    hauteur: u8,
    position: f64,
    pas: f64,
    gain: (f32, f32),
    phase: Phase,
    /// Niveau de l'enveloppe (0 à 1) et temps passé dans la phase, en échantillons.
    niveau: f32,
    temps: u32,
    /// Touche relâchée (la boucle tenue s'arrête, le relâchement commence).
    relachee: bool,
    /// Numéro d'ordre (pour voler la plus ancienne quand toutes sont prises).
    age: u64,
    /// Facteur de relâchement par échantillon.
    facteur_relachement: f32,
}

impl Default for Voix {
    fn default() -> Self {
        Voix {
            active: false,
            zone: 0,
            hauteur: 0,
            position: 0.0,
            pas: 1.0,
            gain: (0.0, 0.0),
            phase: Phase::Attaque,
            niveau: 0.0,
            temps: 0,
            relachee: false,
            age: 0,
            facteur_relachement: 0.0,
        }
    }
}

/// La source « échantillonneur » d'un canal.
pub struct Echantillonneur {
    instrument: Option<Arc<InstrumentEchantillons>>,
    frequence: f32,
    voix: [Voix; VOIX_MAX],
    compteur: u64,
    volume: f32,
    pan: f32,
    /// Prochaine prise de chaque alternance (par note et par cycle).
    alternances: [u32; 128],
    /// Touches tenues : début (en échantillons rendus) et vélocité, pour les relâchements.
    tenues: [Option<(u64, u8)>; 128],
    horloge: u64,
    /// Articulation choisie par la dernière touche de commutation.
    commutateur: Option<u8>,
    /// État du tirage au sort des prises (xorshift).
    alea: u32,
}

impl Echantillonneur {
    pub fn new(frequence: u32) -> Self {
        Echantillonneur {
            instrument: None,
            frequence: frequence as f32,
            voix: [Voix::default(); VOIX_MAX],
            compteur: 0,
            volume: 0.8,
            pan: 0.0,
            alternances: [0; 128],
            tenues: [None; 128],
            horloge: 0,
            commutateur: None,
            alea: 0x9E37_79B9,
        }
    }

    /// Tirage au sort entre 0 et 1 (sans allocation, pour le fil audio).
    fn tirage(&mut self) -> f32 {
        self.alea ^= self.alea << 13;
        self.alea ^= self.alea >> 17;
        self.alea ^= self.alea << 5;
        (self.alea >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Change d'instrument (hors du fil audio, ou en lui donnant un Arc déjà prêt : rien n'est
    /// alloué). Les voix en cours se taisent.
    pub fn changer_instrument(&mut self, instrument: Option<Arc<InstrumentEchantillons>>) -> Option<Arc<InstrumentEchantillons>> {
        self.tout_couper();
        self.commutateur = instrument.as_ref().and_then(|i| i.commutateur_defaut);
        std::mem::replace(&mut self.instrument, instrument)
    }

    pub fn instrument(&self) -> Option<&Arc<InstrumentEchantillons>> {
        self.instrument.as_ref()
    }

    fn demarrer(&mut self, zone_index: usize, hauteur: u8, velocite: u8, attenuation_db: f32, pan_note: f32, demi_tons_note: f32) {
        let Some(instrument) = &self.instrument else { return };
        let zone = &instrument.zones[zone_index];
        let r = &zone.region;
        // coupe : les voix du groupe visé s'arrêtent
        if r.groupe != 0 {
            for v in self.voix.iter_mut().filter(|v| v.active) {
                if instrument.zones[v.zone].region.coupe == Some(r.groupe) {
                    v.relachee = true;
                    v.phase = Phase::Relachement;
                    // coupure franche : -60 dB en 6 ms
                    v.facteur_relachement = (-6.9 / (0.006 * self.frequence)).exp();
                }
            }
        }
        // hauteur : écart à la note racine selon le suivi du clavier, réglage fin, transposition
        let demi_tons = (hauteur as f32 - r.note_racine as f32) * r.suivi_clavier / 100.0 + r.transposition as f32 + r.accord / 100.0 + demi_tons_note;
        let pas = 2f64.powf(demi_tons as f64 / 12.0) * zone.echantillon.frequence as f64 / self.frequence as f64;
        // volume : réglage, vélocité (courbe en carré), panoramique à puissance constante
        let v = velocite as f32 / 127.0;
        let suivi = r.suivi_velocite / 100.0;
        let gain_vel = r.gain_courbe(velocite).unwrap_or((1.0 - suivi) + suivi * v * v);
        let gain = 10f32.powf((r.volume_db - attenuation_db) / 20.0) * gain_vel * zone.gain;
        let p = ((r.pan / 100.0 + self.pan + pan_note).clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
        // une voix libre, sinon la plus ancienne
        let index = self.voix.iter().position(|v| !v.active).unwrap_or_else(|| (0..VOIX_MAX).min_by_key(|&i| self.voix[i].age).unwrap_or(0));
        self.compteur += 1;
        self.voix[index] = Voix {
            active: true,
            zone: zone_index,
            hauteur,
            position: r.decalage as f64,
            pas,
            gain: (gain * p.cos(), gain * p.sin()),
            phase: if r.retard > 0.0 { Phase::Retard } else { Phase::Attaque },
            niveau: 0.0,
            temps: 0,
            relachee: false,
            age: self.compteur,
            facteur_relachement: (-1.0 / (r.relachement.max(0.001) * self.frequence / 6.9)).exp(),
        };
    }

    /// Zones qui conviennent à cette touche, cette vélocité et ce déclenchement (en tenant
    /// compte de l'alternance des prises).
    fn zones_pour(&mut self, hauteur: u8, velocite: u8, declenchement: Declenchement) -> ([usize; 16], usize) {
        let tirage = self.tirage();
        let commutateur = self.commutateur;
        let mut choisies = [0usize; 16];
        let mut n = 0;
        let Some(instrument) = &self.instrument else { return (choisies, 0) };
        let autres_tenues = self.tenues.iter().enumerate().any(|(h, t)| h != hauteur as usize && t.is_some());
        let mut cycle = 0;
        for (i, z) in instrument.zones.iter().enumerate() {
            let r = &z.region;
            let convient = match (r.declenchement, declenchement) {
                (Declenchement::Relachement, Declenchement::Relachement) => true,
                (Declenchement::Attaque, Declenchement::Attaque) => true,
                (Declenchement::Premiere, Declenchement::Attaque) => !autres_tenues,
                (Declenchement::Legato, Declenchement::Attaque) => autres_tenues,
                _ => false,
            };
            if !convient || hauteur < r.touche_basse || hauteur > r.touche_haute || velocite < r.velocite_basse || velocite > r.velocite_haute {
                continue;
            }
            // une seule des prises tirées au sort, et seulement l'articulation choisie
            if tirage < r.alea.0 || tirage >= r.alea.1 || (r.commutateur.is_some() && r.commutateur != commutateur) {
                continue;
            }
            if r.sequence.0 > 1 {
                cycle = cycle.max(r.sequence.0);
                if (self.alternances[hauteur as usize] % r.sequence.0) + 1 != r.sequence.1 {
                    continue;
                }
            }
            if n < choisies.len() {
                choisies[n] = i;
                n += 1;
            }
        }
        if cycle > 1 && declenchement == Declenchement::Attaque {
            self.alternances[hauteur as usize] = self.alternances[hauteur as usize].wrapping_add(1);
        }
        (choisies, n)
    }
}

impl Source for Echantillonneur {
    fn changer_instrument(&mut self, instrument: Option<Arc<InstrumentEchantillons>>) -> Option<Arc<InstrumentEchantillons>> {
        Echantillonneur::changer_instrument(self, instrument)
    }

    fn regler(&mut self, _programme: u8, _batterie: bool, volume: f32, pan: f32) {
        self.volume = volume;
        self.pan = pan;
    }

    fn note_on(&mut self, hauteur: u8, velocite: u8) {
        self.note_on_expressive(hauteur, velocite, 0.0, 0.0);
    }

    fn note_on_expressive(&mut self, hauteur: u8, velocite: u8, pan: f32, demi_tons: f32) {
        if velocite == 0 {
            return self.note_off(hauteur);
        }
        // une touche de commutation change d'articulation, sans jouer
        if let Some((bas, haut)) = self.instrument.as_ref().and_then(|i| i.commutation) {
            if (bas..=haut).contains(&hauteur) {
                self.commutateur = Some(hauteur);
                return;
            }
        }
        let (zones, n) = self.zones_pour(hauteur, velocite, Declenchement::Attaque);
        self.tenues[hauteur as usize & 127] = Some((self.horloge, velocite));
        for &z in &zones[..n] {
            self.demarrer(z, hauteur, velocite, 0.0, pan, demi_tons);
        }
    }

    fn note_off(&mut self, hauteur: u8) {
        let Some(instrument) = self.instrument.clone() else { return };
        for v in self.voix.iter_mut().filter(|v| v.active && v.hauteur == hauteur && !v.relachee) {
            let r = &instrument.zones[v.zone].region;
            if r.boucle == Boucle::UneFois || r.declenchement == Declenchement::Relachement {
                continue;
            }
            v.relachee = true;
            v.phase = Phase::Relachement;
        }
        // bruits de relâchement, amortis selon le temps de touche tenue
        if let Some((debut, velocite)) = self.tenues[hauteur as usize & 127].take() {
            let tenue = (self.horloge - debut) as f32 / self.frequence;
            let (zones, n) = self.zones_pour(hauteur, velocite, Declenchement::Relachement);
            for &z in &zones[..n] {
                let amortissement = instrument.zones[z].region.amortissement_relachement * tenue;
                self.demarrer(z, hauteur, velocite, amortissement, 0.0, 0.0);
            }
        }
    }

    fn tout_couper(&mut self) {
        for v in self.voix.iter_mut() {
            v.active = false;
        }
        self.tenues = [None; 128];
    }

    fn voix(&self) -> u32 {
        self.voix.iter().filter(|v| v.active).count() as u32
    }

    fn rendre(&mut self, gauche: &mut [f32], droite: &mut [f32]) {
        gauche.fill(0.0);
        droite.fill(0.0);
        self.horloge += gauche.len() as u64;
        let Some(instrument) = &self.instrument else { return };
        let fe = self.frequence;
        let volume = self.volume * self.volume; // même loi que les autres sources
        for v in self.voix.iter_mut().filter(|v| v.active) {
            let zone = &instrument.zones[v.zone];
            let r = &zone.region;
            let e = &zone.echantillon;
            let trames = e.trames() as f64;
            let fin = r.fin.map(|f| (f as f64).min(trames)).unwrap_or(trames);
            let boucle = match (r.boucle_debut, r.boucle_fin, e.boucle) {
                (Some(d), Some(f), _) => Some((d as f64, f as f64)),
                (None, None, Some((d, f))) => Some((d as f64, f as f64)),
                (d, f, b) => b.map(|(bd, bf)| (d.map(|x| x as f64).unwrap_or(bd as f64), f.map(|x| x as f64).unwrap_or(bf as f64))),
            };
            let boucle_active = |relachee: bool| match r.boucle {
                Boucle::Continue => boucle,
                Boucle::Tenue if !relachee => boucle,
                _ => None,
            };
            let (n_retard, n_attaque, n_tenue) = ((r.retard * fe) as u32, (r.attaque * fe).max(1.0) as u32, (r.tenue * fe) as u32);
            let maintien = r.maintien / 100.0;
            let facteur_declin = if r.declin > 0.0 { (-1.0 / (r.declin * fe / 6.9)).exp() } else { 0.0 };
            for (g, d) in gauche.iter_mut().zip(droite.iter_mut()) {
                // enveloppe
                match v.phase {
                    Phase::Retard => {
                        v.temps += 1;
                        if v.temps >= n_retard {
                            (v.phase, v.temps) = (Phase::Attaque, 0);
                        }
                        continue;
                    }
                    Phase::Attaque => {
                        v.temps += 1;
                        v.niveau = (v.temps as f32 / n_attaque as f32).min(1.0);
                        if v.temps >= n_attaque {
                            (v.phase, v.temps, v.niveau) = (Phase::Tenue, 0, 1.0);
                        }
                    }
                    Phase::Tenue => {
                        v.temps += 1;
                        if v.temps >= n_tenue {
                            (v.phase, v.temps) = (Phase::Declin, 0);
                        }
                    }
                    Phase::Declin => {
                        v.niveau = maintien + (v.niveau - maintien) * facteur_declin;
                        if (v.niveau - maintien).abs() < 1e-4 {
                            (v.phase, v.niveau) = (Phase::Maintien, maintien);
                        }
                    }
                    Phase::Maintien => {}
                    Phase::Relachement => {
                        v.niveau *= v.facteur_relachement;
                        if v.niveau < 1e-4 {
                            v.active = false;
                            break;
                        }
                    }
                }
                let (a, b) = e.lire(v.position);
                let niveau = v.niveau * volume;
                *g += a * v.gain.0 * niveau;
                *d += b * v.gain.1 * niveau;
                v.position += v.pas;
                if let Some((debut, fin_boucle)) = boucle_active(v.relachee) {
                    if fin_boucle > debut && v.position >= fin_boucle {
                        v.position = debut + (v.position - fin_boucle) % (fin_boucle - debut);
                    }
                } else if v.position >= fin {
                    v.active = false;
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un WAV mono de `trames` trames : une sinusoïde à la fréquence voulue.
    fn ecrire_wav(chemin: &Path, frequence_son: f32, trames: usize, boucle: Option<(u32, u32)>) {
        let spec = hound::WavSpec { channels: 1, sample_rate: 44100, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(chemin, spec).unwrap();
        for i in 0..trames {
            let v = (i as f32 / 44100.0 * frequence_son * std::f32::consts::TAU).sin() * 0.5;
            w.write_sample((v * i16::MAX as f32) as i16).unwrap();
        }
        w.finalize().unwrap();
        if let Some((d, f)) = boucle {
            // ajoute un bloc « smpl » avec une boucle
            let mut o = std::fs::read(chemin).unwrap();
            let mut smpl = vec![0u8; 36 + 24];
            smpl[28..32].copy_from_slice(&1u32.to_le_bytes());
            smpl[36 + 8..36 + 12].copy_from_slice(&d.to_le_bytes());
            smpl[36 + 12..36 + 16].copy_from_slice(&f.to_le_bytes());
            o.extend_from_slice(b"smpl");
            o.extend_from_slice(&(smpl.len() as u32).to_le_bytes());
            o.extend_from_slice(&smpl);
            let taille = (o.len() - 8) as u32;
            o[4..8].copy_from_slice(&taille.to_le_bytes());
            std::fs::write(chemin, o).unwrap();
        }
    }

    /// Fréquence dominante, par comptage des passages par zéro.
    fn frequence(x: &[f32]) -> f32 {
        let passages = x.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        passages as f32 / (x.len() as f32 / 44100.0)
    }

    fn efficace(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    fn dossier(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("song-audio-ech-{nom}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn rendre(e: &mut Echantillonneur, secondes: f32) -> Vec<f32> {
        let n = (secondes * 44100.0) as usize;
        let (mut g, mut d) = (vec![0.0; n], vec![0.0; n]);
        for debut in (0..n).step_by(256) {
            let fin = (debut + 256).min(n);
            e.rendre(&mut g[debut..fin], &mut d[debut..fin]);
        }
        g
    }

    fn instrument(sfz: &str, d: &Path) -> Arc<InstrumentEchantillons> {
        std::fs::write(d.join("i.sfz"), sfz).unwrap();
        Arc::new(InstrumentEchantillons::depuis_sfz(&d.join("i.sfz")).unwrap().0)
    }

    #[test]
    fn hauteur_transposee_et_couches_de_velocite() {
        let d = dossier("hauteur");
        ecrire_wav(&d.join("la.wav"), 440.0, 44100, None);
        ecrire_wav(&d.join("la fort.wav"), 440.0, 44100, None);
        let i = instrument("<region> sample=la.wav pitch_keycenter=69 hivel=80\n<region> sample=la fort.wav pitch_keycenter=69 lovel=81 volume=6", &d);
        assert_eq!(i.zones.len(), 2);
        let mut e = Echantillonneur::new(44100);
        e.changer_instrument(Some(i));
        // une octave plus haut : 880 Hz
        e.note_on(81, 70);
        let s = rendre(&mut e, 0.3);
        assert!((frequence(&s) - 880.0).abs() < 15.0, "{} Hz", frequence(&s));
        assert_eq!(e.voix(), 1, "une seule couche à vélocité 70");
        e.tout_couper();
        // la couche forte (+6 dB) à vélocité 127 est nettement plus forte que la douce à 70
        e.note_on(69, 70);
        let doux = efficace(&rendre(&mut e, 0.2));
        e.tout_couper();
        e.note_on(69, 127);
        let fort = efficace(&rendre(&mut e, 0.2));
        assert!(fort > doux * 3.0, "doux {doux}, fort {fort}");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn enveloppe_boucle_et_relachement() {
        let d = dossier("boucle");
        ecrire_wav(&d.join("tenu.wav"), 441.0, 4410, Some((1000, 4000)));
        let i = instrument("<region> sample=tenu.wav key=69 loop_mode=loop_sustain ampeg_release=0.05", &d);
        let mut e = Echantillonneur::new(44100);
        e.changer_instrument(Some(i));
        e.note_on(69, 100);
        // l'échantillon dure 0,1 s, mais la boucle du fichier le fait tenir
        let tenu = rendre(&mut e, 1.0);
        assert!(efficace(&tenu[40000..]) > 0.05, "la boucle tient le son");
        e.note_off(69);
        let apres = rendre(&mut e, 0.5);
        assert!(efficace(&apres[11025..]) < 1e-3, "relâché : le son s'éteint en 50 ms");
        assert_eq!(e.voix(), 0);
        // sans boucle, le son s'arrête à la fin de l'échantillon
        let i = instrument("<region> sample=tenu.wav key=69", &d);
        e.changer_instrument(Some(i));
        e.note_on(69, 100);
        let court = rendre(&mut e, 0.5);
        assert!(efficace(&court[8820..]) < 1e-6);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn alternance_coupe_et_bruit_de_relachement() {
        let d = dossier("batterie");
        for (nom, f) in [("a.wav", 300.0), ("b.wav", 600.0), ("ouvert.wav", 1000.0), ("rel.wav", 200.0)] {
            ecrire_wav(&d.join(nom), f, 44100, None);
        }
        let sfz = "<group> pitch_keytrack=0 loop_mode=one_shot
            <region> sample=a.wav key=42 group=1 seq_length=2 seq_position=1
            <region> sample=b.wav key=42 group=1 seq_length=2 seq_position=2
            <region> sample=ouvert.wav key=46 group=2 off_by=1
            <region> sample=rel.wav key=60 trigger=release loop_mode=no_loop";
        let i = instrument(sfz, &d);
        let mut e = Echantillonneur::new(44100);
        e.changer_instrument(Some(i));
        // alternance : 300 Hz puis 600 Hz puis 300 Hz
        let mut prises = Vec::new();
        for _ in 0..3 {
            e.note_on(42, 100);
            prises.push(frequence(&rendre(&mut e, 0.1)));
            e.tout_couper();
        }
        assert!((prises[0] - 300.0).abs() < 20.0 && (prises[1] - 600.0).abs() < 20.0 && (prises[2] - 300.0).abs() < 20.0, "{prises:?}");
        // le charleston fermé (groupe 1) coupe l'ouvert (coupé par 1)
        e.note_on(46, 100);
        rendre(&mut e, 0.05);
        e.note_on(42, 100);
        rendre(&mut e, 0.05);
        assert_eq!(e.voix(), 1, "l'ouvert s'est tu");
        e.tout_couper();
        // une note sans région d'attaque, avec un bruit de relâchement
        e.note_on(60, 100);
        assert_eq!(e.voix(), 0);
        rendre(&mut e, 0.1);
        e.note_off(60);
        assert_eq!(e.voix(), 1, "le bruit de relâchement joue");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn prises_au_hasard_et_touches_de_commutation() {
        let d = dossier("hasard");
        for (nom, f) in [("a.wav", 300.0), ("b.wav", 600.0), ("staccato.wav", 1000.0)] {
            ecrire_wav(&d.join(nom), f, 44100, None);
        }
        let sfz = "<global> sw_lokey=24 sw_hikey=25 sw_default=24
            <group> sw_last=24 <region> sample=a.wav key=60 lorand=0 hirand=0.5 pitch_keytrack=0
            <region> sample=b.wav key=60 lorand=0.5 hirand=1 pitch_keytrack=0
            <group> sw_last=25 <region> sample=staccato.wav key=60 pitch_keytrack=0";
        let i = instrument(sfz, &d);
        assert_eq!((i.commutation, i.commutateur_defaut), (Some((24, 25)), Some(24)));
        let mut e = Echantillonneur::new(44100);
        e.changer_instrument(Some(i));
        // une seule prise à la fois, et les deux finissent par sortir
        let (mut a, mut b) = (0, 0);
        for _ in 0..40 {
            e.note_on(60, 100);
            assert_eq!(e.voix(), 1, "une seule prise par note");
            let f = frequence(&rendre(&mut e, 0.05));
            if (f - 300.0).abs() < 40.0 {
                a += 1;
            } else if (f - 600.0).abs() < 40.0 {
                b += 1;
            }
            e.tout_couper();
        }
        assert_eq!(a + b, 40);
        assert!(a > 5 && b > 5, "tirage équilibré : {a} / {b}");
        // touche 25 : l'articulation staccato ; la touche elle-même ne sonne pas
        e.note_on(25, 100);
        assert_eq!(e.voix(), 0);
        e.note_on(60, 100);
        assert!((frequence(&rendre(&mut e, 0.05)) - 1000.0).abs() < 40.0);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn echantillons_manquants_signales() {
        let d = dossier("manque");
        ecrire_wav(&d.join("la.wav"), 440.0, 1000, None);
        std::fs::write(d.join("i.sfz"), "<region> sample=la.wav <region> sample=absent.wav").unwrap();
        let (i, avertissements) = InstrumentEchantillons::depuis_sfz(&d.join("i.sfz")).unwrap();
        assert_eq!(i.zones.len(), 1);
        assert_eq!(avertissements.len(), 1);
        std::fs::write(d.join("vide.sfz"), "<region> sample=absent.wav").unwrap();
        assert!(InstrumentEchantillons::depuis_sfz(&d.join("vide.sfz")).is_err());
        std::fs::remove_dir_all(&d).ok();
    }
}
