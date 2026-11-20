//! Sortie audio en direct (cpal). Le fil audio ne fait que rendre : les commandes arrivent par
//! une file sans verrou (rtrb), les anciennes configurations repartent par une autre pour être
//! libérées hors du fil audio, et les mesures sont publiées sans jamais attendre un verrou.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rustysynth::SoundFont;

use crate::noyau::{BLOC_MAX, Canal, Configuration, Evenement, Mesures, Noyau, Piste, Stockage};
use crate::transport::{ActionTransport, EtatTransport, Session, SessionPreparee, Transport};

pub enum Commande {
    Evenement(Evenement),
    Configurer(Box<Configuration>),
    ToutCouper,
    Session(Box<SessionPreparee>),
    Transport(ActionTransport),
    /// Listes plus grandes pour le graphe (préparées hors du fil audio).
    Stockage(Box<Stockage>),
    /// Un canal ou une piste, construits hors du fil audio, à installer à leur numéro.
    InstallerCanal(u32, Box<Canal>),
    InstallerPiste(u32, Box<Piste>),
    /// Un instrument d'échantillons (chargé hors du fil audio) pour un canal.
    Instrument(u32, Option<Arc<crate::echantillonneur::InstrumentEchantillons>>),
}

/// Ce que le fil audio rend au fil de l'interface pour être libéré hors du fil audio.
pub enum Dechet {
    Configuration(Box<Configuration>),
    Session(Box<SessionPreparee>),
    Stockage(Box<Stockage>),
    Canal(Box<Canal>),
    Piste(Box<Piste>),
    Instrument(Arc<crate::echantillonneur::InstrumentEchantillons>),
}

/// Capacité de départ du graphe (le noyau en réserve autant à sa création).
const CAPACITE_DEPART: usize = 64;

/// État du transport publié par le fil audio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EtatLecture {
    pub etat: EtatTransport,
    /// Position dans la boucle et position absolue, en tics, à l'instant `horloge`.
    pub position: f64,
    pub position_absolue: f64,
    pub horloge: u64,
    pub tempo: f64,
    pub longueur: u32,
}

/// État partagé entre le fil audio et l'interface.
struct Partage {
    horloge: AtomicU64,
    mesures: Mutex<Mesures>,
    lecture: Mutex<EtatLecture>,
}

pub struct Moteur {
    commandes: rtrb::Producer<Commande>,
    poubelle: rtrb::Consumer<Dechet>,
    partage: Arc<Partage>,
    frequence: u32,
    banque: Arc<SoundFont>,
    /// Capacité des listes du graphe dans le fil audio (canaux, pistes).
    capacite: (usize, usize),
    /// Canaux et pistes déjà construits et envoyés (numéros 0..n).
    installes: (usize, usize),
    /// Nom de l'appareil de sortie, ou None : pas de carte son, horloge silencieuse.
    sortie: Option<String>,
    /// Abandonner cet émetteur arrête le fil qui garde le flux audio ouvert.
    _arret: mpsc::Sender<()>,
}

impl Moteur {
    /// Ouvre la sortie audio par défaut et démarre le rendu.
    pub fn demarrer(banque: Arc<SoundFont>) -> Result<Moteur, String> {
        let (envoi_commandes, commandes) = rtrb::RingBuffer::<Commande>::new(16384);
        let (envoi_poubelle, poubelle) = rtrb::RingBuffer::<Dechet>::new(4096);
        let partage = Arc::new(Partage {
            horloge: AtomicU64::new(0),
            mesures: Mutex::new(Mesures { cretes: vec![[0.0; 2]; 1], voix: Vec::new() }),
            lecture: Mutex::new(EtatLecture { etat: EtatTransport::Arret, position: 0.0, position_absolue: 0.0, horloge: 0, tempo: 120.0, longueur: 384 }),
        });
        let (arret, attente) = mpsc::channel::<()>();
        let (pret, demarrage) = mpsc::channel::<Result<(u32, Option<String>), String>>();
        let partage_audio = partage.clone();
        let banque_audio = banque.clone();

        // Le flux cpal n'est pas transférable d'un fil à l'autre sur toutes les plateformes :
        // un fil dédié le crée, le garde, et le ferme quand le moteur est abandonné.
        thread::Builder::new()
            .name("song-audio-sortie".into())
            .spawn(move || {
                let rendu = Rendu {
                    commandes,
                    poubelle: envoi_poubelle,
                    partage: partage_audio,
                    gauche: vec![0.0; BLOC_MAX],
                    droite: vec![0.0; BLOC_MAX],
                    transport: Transport::new(),
                };
                match trouver_sortie() {
                    Ok((appareil, nom, config)) => {
                        let frequence = config.sample_rate;
                        let resultat = Noyau::new(&banque_audio, frequence).and_then(|noyau| ouvrir_flux(&appareil, config, noyau, rendu));
                        match resultat {
                            Ok(flux) => {
                                let _ = pret.send(Ok((frequence, Some(nom))));
                                let _ = attente.recv(); // jusqu'à l'abandon du moteur
                                drop(flux);
                            }
                            Err(e) => {
                                let _ = pret.send(Err(e));
                            }
                        }
                    }
                    Err(raison) => {
                        eprintln!("pas de sortie audio ({raison}) : horloge silencieuse");
                        horloge_silencieuse(&banque_audio, rendu, pret, attente);
                    }
                }
            })
            .map_err(|e| format!("fil audio : {e}"))?;

        let (frequence, sortie) = demarrage.recv().map_err(|_| "le fil audio s'est arrêté".to_string())??;
        Ok(Moteur {
            commandes: envoi_commandes,
            poubelle,
            partage,
            frequence,
            banque,
            capacite: (CAPACITE_DEPART, CAPACITE_DEPART),
            installes: (0, 1),
            sortie,
            _arret: arret,
        })
    }

    pub fn frequence(&self) -> u32 {
        self.frequence
    }

    /// Nom de l'appareil de sortie ; None si aucune carte son n'est disponible.
    pub fn sortie(&self) -> Option<&str> {
        self.sortie.as_deref()
    }

    /// Échantillons rendus depuis le démarrage.
    pub fn horloge(&self) -> u64 {
        self.partage.horloge.load(Ordering::Acquire)
    }

    fn envoyer(&mut self, commande: Commande) {
        while let Ok(ancienne) = self.poubelle.pop() {
            drop(ancienne);
        }
        // file pleine : on attend un peu que le fil audio la vide (jamais l'inverse) ; un
        // événement de note est perdu plutôt que de bloquer longtemps
        let mut commande = commande;
        for _ in 0..200 {
            match self.commandes.push(commande) {
                Ok(()) => return,
                Err(rtrb::PushError::Full(c)) => {
                    commande = c;
                    if matches!(commande, Commande::Evenement(_)) {
                        return;
                    }
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }

    pub fn programmer(&mut self, evenement: Evenement) {
        self.envoyer(Commande::Evenement(evenement));
    }

    /// Applique une configuration, quelle que soit sa taille : le graphe grandit d'abord (listes
    /// plus grandes, puis canaux et pistes construits ici, hors du fil audio), puis les réglages
    /// suivent. Les commandes arrivent dans l'ordre : le fil audio a tout avant les réglages.
    pub fn configurer(&mut self, configuration: Configuration) -> Result<(), String> {
        let (canaux, pistes) = (configuration.canaux.len(), configuration.pistes.len().max(1));
        if canaux > self.capacite.0 || pistes > self.capacite.1 {
            let nouvelle = (canaux.next_power_of_two().max(self.capacite.0), pistes.next_power_of_two().max(self.capacite.1));
            self.envoyer(Commande::Stockage(Box::new(Stockage::avec_capacite(nouvelle.0, nouvelle.1))));
            self.capacite = nouvelle;
        }
        for numero in self.installes.0..canaux {
            let canal = Canal::new(&self.banque, self.frequence)?;
            self.envoyer(Commande::InstallerCanal(numero as u32, canal));
            self.installes.0 = numero + 1;
        }
        for numero in self.installes.1..pistes {
            self.envoyer(Commande::InstallerPiste(numero as u32, Piste::new()));
            self.installes.1 = numero + 1;
        }
        {
            // les mesures suivent la taille du graphe (le fil audio ne fait que les remplir)
            let mut mesures = self.partage.mesures.lock().unwrap_or_else(|e| e.into_inner());
            if mesures.voix.len() < self.installes.0 {
                mesures.voix.resize(self.installes.0, 0);
            }
            if mesures.cretes.len() < self.installes.1 {
                mesures.cretes.resize(self.installes.1, [0.0; 2]);
            }
        }
        self.envoyer(Commande::Configurer(Box::new(configuration)));
        Ok(())
    }

    /// Canaux et pistes construits dans le graphe.
    pub fn taille_du_graphe(&self) -> (usize, usize) {
        self.installes
    }

    pub fn tout_couper(&mut self) {
        self.envoyer(Commande::ToutCouper);
    }

    /// Donne un instrument d'échantillons au canal de ce numéro (déjà installé dans le graphe).
    pub fn changer_instrument(&mut self, canal: u32, instrument: Option<Arc<crate::echantillonneur::InstrumentEchantillons>>) {
        self.envoyer(Commande::Instrument(canal, instrument));
    }

    pub fn definir_session(&mut self, session: Session) {
        // préparée ici (tri, allocation), hors du fil audio
        self.envoyer(Commande::Session(Box::new(SessionPreparee::new(session))));
    }

    pub fn transport(&mut self, action: ActionTransport) {
        self.envoyer(Commande::Transport(action));
    }

    /// Dernier état du transport publié par le fil audio.
    pub fn lecture(&self) -> EtatLecture {
        *self.partage.lecture.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Mesures depuis le dernier appel (les crêtes repartent de zéro).
    pub fn mesures(&self) -> Mesures {
        let mut mesures = self.partage.mesures.lock().unwrap_or_else(|e| e.into_inner());
        let copie = mesures.clone();
        for crete in mesures.cretes.iter_mut() {
            *crete = [0.0; 2];
        }
        copie
    }
}

/// Fréquence de l'horloge silencieuse quand il n'y a pas de carte son.
const FREQUENCE_SANS_SORTIE: u32 = 48000;

fn trouver_sortie() -> Result<(cpal::Device, String, cpal::StreamConfig), String> {
    let hote = cpal::default_host();
    let appareil = hote.default_output_device().ok_or("aucune sortie audio")?;
    let reglage = appareil.default_output_config().map_err(|e| format!("sortie audio : {e}"))?;
    if reglage.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!("format de sortie non pris en charge : {:?}", reglage.sample_format()));
    }
    let nom = appareil.description().map(|d| d.name().to_string()).unwrap_or_else(|_| "sortie audio".into());
    Ok((appareil, nom, reglage.config()))
}

/// Sans carte son utilisable, le moteur tourne quand même, au rythme réel, sans rien faire
/// entendre : transport, mesures et export fonctionnent.
fn horloge_silencieuse(
    banque: &Arc<SoundFont>,
    mut rendu: Rendu,
    pret: mpsc::Sender<Result<(u32, Option<String>), String>>,
    attente: mpsc::Receiver<()>,
) {
    let frequence = FREQUENCE_SANS_SORTIE;
    let mut noyau = match Noyau::new(banque, frequence) {
        Ok(n) => n,
        Err(e) => {
            let _ = pret.send(Err(e));
            return;
        }
    };
    let _ = pret.send(Ok((frequence, None)));
    let bloc = (frequence / 100) as usize; // 10 ms
    let mut tampon = vec![0.0f32; bloc * 2];
    let depart = Instant::now();
    let mut rendus = 0u64;
    while let Err(mpsc::TryRecvError::Empty) = attente.try_recv() {
        rendu.remplir(&mut noyau, &mut tampon, 2);
        rendus += bloc as u64;
        let echeance = depart + Duration::from_secs_f64(rendus as f64 / frequence as f64);
        if let Some(reste) = echeance.checked_duration_since(Instant::now()) {
            thread::sleep(reste);
        }
    }
}

/// Ce dont le rendu a besoin à chaque bloc, avec ou sans carte son.
struct Rendu {
    commandes: rtrb::Consumer<Commande>,
    poubelle: rtrb::Producer<Dechet>,
    partage: Arc<Partage>,
    gauche: Vec<f32>,
    droite: Vec<f32>,
    transport: Transport,
}

impl Rendu {
    /// Applique les commandes reçues puis remplit `sortie` (entrelacée, `voies` voies).
    fn remplir(&mut self, noyau: &mut Noyau, sortie: &mut [f32], voies: usize) {
        while let Ok(commande) = self.commandes.pop() {
            match commande {
                Commande::Evenement(e) => noyau.programmer(e),
                Commande::ToutCouper => noyau.tout_couper(),
                Commande::Configurer(configuration) => {
                    noyau.configurer(&configuration);
                    // libérée par l'autre fil : pas de désallocation ici
                    let _ = self.poubelle.push(Dechet::Configuration(configuration));
                }
                Commande::Stockage(mut boite) => {
                    // les nouvelles listes entrent, les anciennes (vides) repartent dans la même boîte
                    let nouveau = std::mem::replace(&mut *boite, Stockage::avec_capacite(0, 0));
                    *boite = noyau.echanger_stockage(nouveau);
                    let _ = self.poubelle.push(Dechet::Stockage(boite));
                }
                Commande::InstallerCanal(numero, canal) => {
                    if let Some(ancien) = noyau.installer_canal(numero, canal) {
                        let _ = self.poubelle.push(Dechet::Canal(ancien));
                    }
                }
                Commande::Instrument(numero, instrument) => {
                    // l'ancien instrument repart : sa dernière référence ne meurt pas ici
                    if let Some(ancien) = noyau.changer_instrument(numero, instrument) {
                        let _ = self.poubelle.push(Dechet::Instrument(ancien));
                    }
                }
                Commande::InstallerPiste(numero, piste) => {
                    if let Some(ancienne) = noyau.installer_piste(numero, piste) {
                        let _ = self.poubelle.push(Dechet::Piste(ancienne));
                    }
                }
                Commande::Session(mut session) => {
                    // la nouvelle session entre, l'ancienne repart dans la même boîte
                    self.transport.echanger_session(&mut session, noyau);
                    let _ = self.poubelle.push(Dechet::Session(session));
                }
                Commande::Transport(action) => self.transport.agir(action, noyau),
            }
        }
        for morceau in sortie.chunks_mut(BLOC_MAX * voies) {
            let n = morceau.len() / voies;
            self.transport.avancer(noyau, n);
            noyau.rendre(&mut self.gauche[..n], &mut self.droite[..n]);
            for (i, trame) in morceau.chunks_mut(voies).enumerate() {
                trame[0] = self.gauche[i];
                if voies > 1 {
                    trame[1] = self.droite[i];
                    for v in trame.iter_mut().skip(2) {
                        *v = 0.0;
                    }
                }
            }
        }
        self.partage.horloge.store(noyau.horloge(), Ordering::Release);
        if let Ok(mut lecture) = self.partage.lecture.try_lock() {
            let session = self.transport.session();
            *lecture = EtatLecture {
                etat: self.transport.etat(),
                position: self.transport.position(),
                position_absolue: self.transport.position_absolue(),
                horloge: noyau.horloge(),
                tempo: session.tempo,
                longueur: session.longueur,
            };
        }
        if let Ok(mut mesures) = self.partage.mesures.try_lock() {
            noyau.cumuler_mesures(&mut mesures);
        }
    }
}

fn ouvrir_flux(appareil: &cpal::Device, config: cpal::StreamConfig, mut noyau: Noyau, mut rendu: Rendu) -> Result<cpal::Stream, String> {
    let voies = config.channels as usize;
    let flux = appareil
        .build_output_stream::<f32, _, _>(
            config,
            move |sortie: &mut [f32], _| rendu.remplir(&mut noyau, sortie, voies),
            |erreur| eprintln!("erreur du flux audio : {erreur}"),
            None,
        )
        .map_err(|e| format!("flux audio : {e}"))?;
    flux.play().map_err(|e| format!("lecture audio : {e}"))?;
    Ok(flux)
}
