//! song-audio, version de bureau : la fenêtre Tauri et les commandes qui relient l'interface
//! au moteur sonore (crate `moteur`).

use std::sync::{Arc, Mutex};

use moteur::{CANAUX, Configuration, Evenement, Message, Moteur, PISTES, ReglageCanal, ReglagePiste, SoundFont};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State, ipc::Response, path::BaseDirectory};

const BANQUE_GM: &str = "banques/GeneralUser-GS.sf2";

struct EtatMoteur {
    banque: Arc<SoundFont>,
    moteur: Mutex<Option<Moteur>>,
    erreur: Mutex<Option<String>>,
}

/// Événement venu de l'interface : instant en secondes sur l'horloge du moteur ; une vélocité
/// nulle relâche la note.
#[derive(Deserialize, Clone, Copy)]
struct EvenementIpc {
    temps: f64,
    canal: u8,
    hauteur: u8,
    velocite: u8,
}

#[derive(Deserialize)]
struct CanalIpc {
    canal: u8,
    programme: u8,
    batterie: bool,
    volume: f32,
    pan: f32,
    piste: u8,
}

#[derive(Deserialize)]
struct PisteIpc {
    index: u8,
    volume: f32,
    pan: f32,
    audible: bool,
    eq: [f32; 3],
    reverb: f32,
}

#[derive(Deserialize)]
struct ConfigurationIpc {
    canaux: Vec<CanalIpc>,
    pistes: Vec<PisteIpc>,
}

impl ConfigurationIpc {
    fn vers_moteur(&self) -> Configuration {
        let mut c = Configuration::default();
        for canal in &self.canaux {
            if let Some(r) = c.canaux.get_mut(canal.canal as usize) {
                *r = ReglageCanal {
                    actif: true,
                    programme: canal.programme,
                    batterie: canal.batterie,
                    volume: canal.volume,
                    pan: canal.pan,
                    piste: canal.piste,
                };
            }
        }
        for piste in &self.pistes {
            if let Some(r) = c.pistes.get_mut(piste.index as usize) {
                *r = ReglagePiste { existe: true, volume: piste.volume, pan: piste.pan, audible: piste.audible, eq: piste.eq, reverb: piste.reverb };
            }
        }
        c
    }
}

fn vers_evenement(e: EvenementIpc, frequence: u32) -> Evenement {
    let echantillon = (e.temps.max(0.0) * frequence as f64).round() as u64;
    let message = if e.velocite > 0 {
        Message::NoteOn { hauteur: e.hauteur, velocite: e.velocite }
    } else {
        Message::NoteOff { hauteur: e.hauteur }
    };
    Evenement { echantillon, canal: e.canal, message }
}

#[derive(Serialize)]
struct InfosMoteur {
    frequence: u32,
    /// Appareil de sortie ; None : aucune carte son, le moteur tourne sans rien faire entendre.
    sortie: Option<String>,
    canaux: usize,
    pistes: usize,
}

#[derive(Serialize)]
struct MesuresIpc {
    /// Échantillons rendus (horloge du moteur) au moment des mesures.
    horloge: u64,
    cretes: Vec<[f32; 2]>,
    voix: Vec<u32>,
}

/// Démarre la sortie audio (une seule fois) et renvoie ses caractéristiques.
#[tauri::command]
fn moteur_demarrer(etat: State<EtatMoteur>) -> Result<InfosMoteur, String> {
    let mut moteur = etat.moteur.lock().map_err(|e| e.to_string())?;
    if moteur.is_none() {
        match Moteur::demarrer(etat.banque.clone()) {
            Ok(m) => *moteur = Some(m),
            Err(e) => {
                *etat.erreur.lock().map_err(|e| e.to_string())? = Some(e.clone());
                return Err(e);
            }
        }
    }
    let m = moteur.as_ref().expect("moteur démarré");
    Ok(InfosMoteur { frequence: m.frequence(), sortie: m.sortie().map(str::to_string), canaux: CANAUX, pistes: PISTES })
}

fn avec_moteur<T>(etat: &State<EtatMoteur>, action: impl FnOnce(&mut Moteur) -> T) -> Result<T, String> {
    let mut moteur = etat.moteur.lock().map_err(|e| e.to_string())?;
    moteur.as_mut().map(action).ok_or_else(|| "moteur non démarré".to_string())
}

#[tauri::command]
fn moteur_evenements(etat: State<EtatMoteur>, evenements: Vec<EvenementIpc>) -> Result<(), String> {
    avec_moteur(&etat, |m| {
        let frequence = m.frequence();
        for e in evenements {
            m.programmer(vers_evenement(e, frequence));
        }
    })
}

#[tauri::command]
fn moteur_tout_couper(etat: State<EtatMoteur>) -> Result<(), String> {
    avec_moteur(&etat, |m| m.tout_couper())
}

#[tauri::command]
fn moteur_configurer(etat: State<EtatMoteur>, configuration: ConfigurationIpc) -> Result<(), String> {
    let c = configuration.vers_moteur();
    avec_moteur(&etat, |m| m.configurer(c))
}

#[tauri::command]
fn moteur_mesures(etat: State<EtatMoteur>) -> Result<MesuresIpc, String> {
    avec_moteur(&etat, |m| {
        let mesures = m.mesures();
        MesuresIpc { horloge: m.horloge(), cretes: mesures.cretes, voix: mesures.voix }
    })
}

/// Export : rendu hors ligne, renvoyé tel quel en octets WAV.
#[tauri::command]
async fn moteur_rendre_wav(
    etat: State<'_, EtatMoteur>,
    configuration: ConfigurationIpc,
    evenements: Vec<EvenementIpc>,
    duree: f64,
) -> Result<Response, String> {
    const FREQUENCE: u32 = 44100;
    let banque = etat.banque.clone();
    let c = configuration.vers_moteur();
    let evenements: Vec<Evenement> = evenements.into_iter().map(|e| vers_evenement(e, FREQUENCE)).collect();
    let octets = tauri::async_runtime::spawn_blocking(move || {
        moteur::rendu::rendre(&banque, FREQUENCE, &c, &evenements, duree).map(|(g, d)| moteur::rendu::wav(&g, &d, FREQUENCE))
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(Response::new(octets))
}

/// Version de l'application, lisible depuis l'interface (vérifie que le pont Tauri répond).
#[tauri::command]
fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn lancer() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
            }
            let chemin = app.path().resolve(BANQUE_GM, BaseDirectory::Resource)?;
            let banque = moteur::charger_banque(&chemin)?;
            app.manage(EtatMoteur { banque, moteur: Mutex::new(None), erreur: Mutex::new(None) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            version,
            moteur_demarrer,
            moteur_evenements,
            moteur_tout_couper,
            moteur_configurer,
            moteur_mesures,
            moteur_rendre_wav
        ])
        .run(tauri::generate_context!())
        .expect("échec du lancement de l'application");
}
