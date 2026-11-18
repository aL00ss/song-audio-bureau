//! song-audio, version de bureau : la fenêtre Tauri et les commandes qui relient l'interface
//! au moteur sonore (crate `moteur`).

use std::sync::{Arc, Mutex};

use moteur::{
    ActionTransport, CANAUX, Configuration, EtatTransport, Evenement, Message, Moteur, NoteSession, PISTES, ReglageCanal, ReglagePiste,
    Session, SoundFont, TypeSource,
};
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
    /// « soundfont » (par défaut) ou « soustractif ».
    #[serde(default)]
    source: String,
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
                    source: if canal.source == "soustractif" { TypeSource::Soustractif } else { TypeSource::SoundFont },
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

/// La session telle que l'interface la compile (notes du pattern ou de l'arrangement).
#[derive(Deserialize)]
struct SessionIpc {
    tempo: f64,
    longueur: u32,
    temps_par_mesure: u32,
    /// [début, fin, canal, hauteur, vélocité] : compact, une session peut compter des milliers de notes.
    notes: Vec<(u32, u32, u8, u8, u8)>,
}

impl SessionIpc {
    fn vers_moteur(&self) -> Session {
        Session {
            tempo: self.tempo.clamp(10.0, 999.0),
            longueur: self.longueur.max(1),
            temps_par_mesure: self.temps_par_mesure.max(1),
            notes: self.notes.iter().map(|&(debut, fin, canal, hauteur, velocite)| NoteSession { debut, fin, canal, hauteur, velocite }).collect(),
            boucler: true,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ActionIpc {
    Jouer { decompte: bool },
    Pause,
    Arreter,
    Chercher { tic: f64 },
    Metronome { actif: bool },
}

#[derive(Serialize, Clone)]
struct LectureIpc {
    etat: &'static str,
    position: f64,
    position_absolue: f64,
    horloge: u64,
    tempo: f64,
    longueur: u32,
}

#[derive(Serialize)]
struct InfosMoteur {
    frequence: u32,
    /// Appareil de sortie ; None : aucune carte son, le moteur tourne sans rien faire entendre.
    sortie: Option<String>,
    canaux: usize,
    pistes: usize,
}

#[derive(Serialize, Clone)]
struct MesuresIpc {
    /// Échantillons rendus (horloge du moteur) au moment des mesures.
    horloge: u64,
    cretes: Vec<[f32; 2]>,
    voix: Vec<u32>,
    lecture: LectureIpc,
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
    avec_moteur(&etat, mesurer)
}

/// Cadence à laquelle le moteur pousse ses mesures vers l'interface (60 par seconde).
const CADENCE_MESURES: std::time::Duration = std::time::Duration::from_millis(16);

/// L'interface s'abonne : le moteur lui pousse ses mesures (horloge, crêtes, voix, transport)
/// à la cadence de l'affichage, au lieu d'être interrogé. L'envoi s'arrête quand l'interface
/// ne reçoit plus (fenêtre fermée, rechargée).
#[tauri::command]
fn moteur_abonner(app: tauri::AppHandle, canal: tauri::ipc::Channel<MesuresIpc>) {
    std::thread::Builder::new()
        .name("song-audio-mesures".into())
        .spawn(move || {
            loop {
                let etat = app.state::<EtatMoteur>();
                let mesures = match avec_moteur(&etat, mesurer) {
                    Ok(m) => m,
                    Err(_) => break,
                };
                if canal.send(mesures).is_err() {
                    break;
                }
                std::thread::sleep(CADENCE_MESURES);
            }
        })
        .expect("fil des mesures");
}

fn mesurer(m: &mut Moteur) -> MesuresIpc {
    {
        let mesures = m.mesures();
        let l = m.lecture();
        let etat = match l.etat {
            EtatTransport::Arret => "arret",
            EtatTransport::Lecture => "lecture",
            EtatTransport::Pause => "pause",
        };
        let lecture = LectureIpc { etat, position: l.position, position_absolue: l.position_absolue, horloge: l.horloge, tempo: l.tempo, longueur: l.longueur };
        MesuresIpc { horloge: m.horloge(), cretes: mesures.cretes, voix: mesures.voix, lecture }
    }
}

#[tauri::command]
fn moteur_session(etat: State<EtatMoteur>, session: SessionIpc) -> Result<(), String> {
    let s = session.vers_moteur();
    avec_moteur(&etat, |m| m.definir_session(s))
}

#[tauri::command]
fn moteur_transport(etat: State<EtatMoteur>, action: ActionIpc) -> Result<(), String> {
    let a = match action {
        ActionIpc::Jouer { decompte } => ActionTransport::Jouer { decompte },
        ActionIpc::Pause => ActionTransport::Pause,
        ActionIpc::Arreter => ActionTransport::Arreter,
        ActionIpc::Chercher { tic } => ActionTransport::Chercher(tic),
        ActionIpc::Metronome { actif } => ActionTransport::Metronome(actif),
    };
    avec_moteur(&etat, |m| m.transport(a))
}

/// Export : rendu hors ligne de la session par le même transport, renvoyé en octets WAV.
#[tauri::command]
async fn moteur_rendre_wav(etat: State<'_, EtatMoteur>, configuration: ConfigurationIpc, session: SessionIpc, queue: f64) -> Result<Response, String> {
    const FREQUENCE: u32 = 44100;
    let banque = etat.banque.clone();
    let c = configuration.vers_moteur();
    let s = session.vers_moteur();
    let octets = tauri::async_runtime::spawn_blocking(move || {
        moteur::rendu::rendre_session(&banque, FREQUENCE, &c, &s, queue).map(|(g, d)| moteur::rendu::wav(&g, &d, FREQUENCE))
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(Response::new(octets))
}

/// Extensions de fichier que l'interface peut lire et écrire (rien d'autre sur le disque).
const EXTENSIONS_PROJET: [&str; 2] = [".song-audio.json", ".json"];

fn verifier_chemin_projet(chemin: &str) -> Result<std::path::PathBuf, String> {
    let bas = chemin.to_lowercase();
    if !EXTENSIONS_PROJET.iter().any(|e| bas.ends_with(e)) {
        return Err(format!("« {chemin} » n'est pas un fichier de projet"));
    }
    Ok(std::path::PathBuf::from(chemin))
}

#[tauri::command]
fn projet_lire(chemin: String) -> Result<String, String> {
    let chemin = verifier_chemin_projet(&chemin)?;
    std::fs::read_to_string(&chemin).map_err(|e| format!("lecture de {} : {e}", chemin.display()))
}

/// Écrit d'abord un fichier temporaire, puis le renomme : un plantage en pleine écriture ne
/// laisse jamais un projet à moitié écrit.
#[tauri::command]
fn projet_ecrire(chemin: String, contenu: String) -> Result<(), String> {
    let chemin = verifier_chemin_projet(&chemin)?;
    let temporaire = chemin.with_extension("json.tmp");
    std::fs::write(&temporaire, contenu).map_err(|e| format!("écriture de {} : {e}", temporaire.display()))?;
    std::fs::rename(&temporaire, &chemin).map_err(|e| format!("enregistrement de {} : {e}", chemin.display()))
}

/// Version de l'application, lisible depuis l'interface (vérifie que le pont Tauri répond).
#[tauri::command]
fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn lancer() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            moteur_abonner,
            moteur_session,
            moteur_transport,
            moteur_rendre_wav,
            projet_lire,
            projet_ecrire
        ])
        .run(tauri::generate_context!())
        .expect("échec du lancement de l'application");
}
