//! Transport dans le moteur : il lit lui-même la session (les notes du pattern ou de tout
//! l'arrangement, compilées par l'interface) et programme chaque note dans le bloc audio où
//! elle tombe, à l'échantillon près. Plus aucune programmation à l'avance depuis l'interface.

use crate::noyau::{Evenement, Message, Noyau};

pub const TICS_PAR_NOIRE: f64 = 96.0;

/// Une note de la session, en tics depuis le début de la boucle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteSession {
    pub debut: u32,
    pub fin: u32,
    /// Numéro du canal dans le graphe (sans limite).
    pub canal: u32,
    pub hauteur: u8,
    pub velocite: u8,
    /// Panoramique propre à la note (-100 à 100) et réglage fin (cents).
    pub pan: i8,
    pub cents: i16,
}

/// Ce que le transport joue : tempo, longueur de la boucle, notes, métronome.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub tempo: f64,
    /// Longueur de la boucle, en tics.
    pub longueur: u32,
    /// Temps par mesure (pour l'accent du métronome).
    pub temps_par_mesure: u32,
    pub notes: Vec<NoteSession>,
    /// Reprendre au début à la fin (sinon : arrêt, les sons finissent de s'éteindre).
    pub boucler: bool,
}

impl Default for Session {
    fn default() -> Self {
        Session { tempo: 120.0, longueur: 384, temps_par_mesure: 4, notes: Vec::new(), boucler: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EtatTransport {
    Arret,
    Lecture,
    Pause,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActionTransport {
    /// Lecture depuis la position courante ; `decompte` : une mesure de clics avant.
    Jouer { decompte: bool },
    Pause,
    Arreter,
    /// Saute à un tic de la boucle.
    Chercher(f64),
    Metronome(bool),
}

/// Bascule (allumage ou extinction) d'une note, triée par tic ; à tic égal, les extinctions
/// d'abord (une note répétée se relâche avant de rejouer).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bascule {
    tic: u32,
    canal: u32,
    hauteur: u8,
    /// 0 : extinction.
    velocite: u8,
    /// Panoramique et réglage fin propres à la note (allumage seulement).
    pan: i8,
    cents: i16,
}

/// Session prête à jouer : ses bascules sont triées hors du fil audio (allocation comprise).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SessionPreparee {
    pub session: Session,
    bascules: Vec<Bascule>,
    /// Notes allumées par canal, dimensionné pour cette session (rempli à l'échange).
    allumees: Vec<[bool; 128]>,
}

impl SessionPreparee {
    pub fn new(session: Session) -> Self {
        let mut bascules = Vec::with_capacity(session.notes.len() * 2);
        for n in &session.notes {
            if n.debut >= session.longueur {
                continue;
            }
            let fin = n.fin.min(session.longueur).max(n.debut + 1);
            bascules.push(Bascule { tic: n.debut, canal: n.canal, hauteur: n.hauteur, velocite: n.velocite.max(1), pan: n.pan, cents: n.cents });
            bascules.push(Bascule { tic: fin, canal: n.canal, hauteur: n.hauteur, velocite: 0, pan: 0, cents: 0 });
        }
        bascules.sort_by(|a, b| a.tic.cmp(&b.tic).then((a.velocite > 0).cmp(&(b.velocite > 0))));
        let canaux = session.notes.iter().map(|n| n.canal as usize + 1).max().unwrap_or(0);
        SessionPreparee { session, bascules, allumees: vec![[false; 128]; canaux] }
    }
}

pub struct Transport {
    prepa: SessionPreparee,
    etat: EtatTransport,
    /// Position absolue en tics (les tours de boucle comptent), utile à l'enregistrement.
    position: f64,
    /// Tics de décompte restant avant de vraiment jouer.
    decompte: f64,
    metronome: bool,
    /// Notes allumées par le transport (pour les relâcher à l'arrêt ou si elles disparaissent).
    allumees: Vec<[bool; 128]>,
}

impl Default for Transport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport {
    pub fn new() -> Self {
        Transport {
            prepa: SessionPreparee::new(Session::default()),
            etat: EtatTransport::Arret,
            position: 0.0,
            decompte: 0.0,
            metronome: false,
            allumees: Vec::new(),
        }
    }

    pub fn etat(&self) -> EtatTransport {
        self.etat
    }

    /// Position absolue (tours de boucle compris), en tics.
    pub fn position_absolue(&self) -> f64 {
        self.position
    }

    /// Position dans la boucle, en tics.
    pub fn position(&self) -> f64 {
        self.position.rem_euclid(self.prepa.session.longueur.max(1) as f64)
    }

    pub fn session(&self) -> &Session {
        &self.prepa.session
    }

    fn tics_par_echantillon(&self, frequence: u32) -> f64 {
        self.prepa.session.tempo * TICS_PAR_NOIRE / 60.0 / frequence as f64
    }

    /// Remplace la session (même en pleine lecture : la position est gardée). Les notes allumées
    /// qui ne sont plus jouées à cet endroit sont relâchées. Commodité hors du fil audio.
    pub fn definir_session(&mut self, session: Session, noyau: &mut Noyau) {
        let mut prepa = SessionPreparee::new(session);
        self.echanger_session(&mut prepa, noyau);
    }

    /// Met `prepa` en place et y range l'ancienne session (à libérer hors du fil audio).
    /// Sans allocation : utilisable dans le fil audio.
    pub fn echanger_session(&mut self, prepa: &mut SessionPreparee, noyau: &mut Noyau) {
        let position = self.position();
        for canal in 0..self.allumees.len() {
            for hauteur in 0..128 {
                if !self.allumees[canal][hauteur] {
                    continue;
                }
                let encore = canal < prepa.allumees.len()
                    && prepa.session.notes.iter().any(|n| {
                        n.canal as usize == canal && n.hauteur as usize == hauteur && (n.debut as f64) <= position && position < n.fin as f64
                    });
                if encore {
                    // la note reste allumée : son état passe dans le tableau de la nouvelle session
                    prepa.allumees[canal][hauteur] = true;
                } else {
                    self.relacher(noyau, canal as u32, hauteur as u8);
                }
            }
        }
        std::mem::swap(&mut self.prepa, prepa);
        // le tableau des notes allumées suit la session (l'ancien repart avec l'ancienne)
        std::mem::swap(&mut self.allumees, &mut self.prepa.allumees);
    }

    fn relacher(&mut self, noyau: &mut Noyau, canal: u32, hauteur: u8) {
        let instant = noyau.horloge();
        noyau.programmer(Evenement { echantillon: instant, canal, message: Message::NoteOff { hauteur } });
        self.allumees[canal as usize][hauteur as usize] = false;
    }

    fn relacher_tout(&mut self, noyau: &mut Noyau) {
        for canal in 0..self.allumees.len() {
            for hauteur in 0..128 {
                if self.allumees[canal][hauteur] {
                    self.relacher(noyau, canal as u32, hauteur as u8);
                }
            }
        }
    }

    pub fn agir(&mut self, action: ActionTransport, noyau: &mut Noyau) {
        match action {
            ActionTransport::Jouer { decompte } => {
                if self.etat != EtatTransport::Lecture {
                    self.etat = EtatTransport::Lecture;
                    self.decompte = if decompte { self.prepa.session.temps_par_mesure as f64 * TICS_PAR_NOIRE } else { 0.0 };
                }
            }
            ActionTransport::Pause => {
                if self.etat == EtatTransport::Lecture {
                    self.relacher_tout(noyau);
                    noyau.couper_clics();
                    self.position = self.position();
                    self.etat = EtatTransport::Pause;
                }
            }
            ActionTransport::Arreter => {
                self.relacher_tout(noyau);
                noyau.couper_clics();
                self.position = 0.0;
                self.decompte = 0.0;
                self.etat = EtatTransport::Arret;
            }
            ActionTransport::Chercher(tic) => {
                self.relacher_tout(noyau);
                let longueur = self.prepa.session.longueur.max(1) as f64;
                self.position = tic.rem_euclid(longueur);
            }
            ActionTransport::Metronome(actif) => self.metronome = actif,
        }
    }

    /// Programme dans le noyau tout ce qui tombe dans les `n` prochains échantillons, puis avance.
    /// À appeler juste avant `noyau.rendre` sur ces mêmes échantillons.
    pub fn avancer(&mut self, noyau: &mut Noyau, n: usize) {
        if self.etat != EtatTransport::Lecture || n == 0 {
            return;
        }
        let pas = self.tics_par_echantillon(noyau.frequence());
        let depart = noyau.horloge();
        let mut fait = 0.0f64; // échantillons déjà couverts dans ce bloc
        let total = n as f64;

        // décompte : des clics, sans avancer la position
        if self.decompte > 0.0 {
            let debut = self.prepa.session.temps_par_mesure as f64 * TICS_PAR_NOIRE - self.decompte;
            if self.decompte >= total * pas - 1e-9 {
                // tout le bloc est du décompte (comparaison tolérante : pas de reste infime)
                self.clics(noyau, depart, debut, debut + total * pas, 0.0, pas, true);
                self.decompte = (self.decompte - total * pas).max(0.0);
                return;
            }
            self.clics(noyau, depart, debut, debut + self.decompte, 0.0, pas, true);
            fait = self.decompte / pas;
            self.decompte = 0.0;
        }

        let longueur = self.prepa.session.longueur.max(1) as f64;
        while fait < total {
            if !self.prepa.session.boucler && self.position >= longueur {
                // fin d'un rendu sans boucle : on s'arrête, les notes finissent de s'éteindre
                self.etat = EtatTransport::Arret;
                self.position = 0.0;
                return;
            }
            let dans_boucle = self.position.rem_euclid(longueur);
            let reste_boucle = longueur - dans_boucle;
            let tics = ((total - fait) * pas).min(reste_boucle);
            let (a, b) = (dans_boucle, dans_boucle + tics);
            // notes dont la bascule tombe dans [a, b)
            let premier = self.prepa.bascules.partition_point(|x| (x.tic as f64) < a);
            for i in premier..self.prepa.bascules.len() {
                let x = self.prepa.bascules[i];
                if (x.tic as f64) >= b {
                    break;
                }
                let echantillon = depart + (fait + (x.tic as f64 - a) / pas).round() as u64;
                let Some(notes) = self.allumees.get_mut(x.canal as usize) else { continue };
                let allumee = &mut notes[x.hauteur as usize];
                if x.velocite > 0 {
                    noyau.programmer(Evenement { echantillon, canal: x.canal, message: if x.pan != 0 || x.cents != 0 {
                        Message::NoteOnExpressive { hauteur: x.hauteur, velocite: x.velocite, pan: x.pan, cents: x.cents }
                    } else {
                        Message::NoteOn { hauteur: x.hauteur, velocite: x.velocite }
                    } });
                    *allumee = true;
                } else if *allumee {
                    noyau.programmer(Evenement { echantillon, canal: x.canal, message: Message::NoteOff { hauteur: x.hauteur } });
                    *allumee = false;
                }
            }
            if self.metronome {
                self.clics(noyau, depart, a, b, fait, pas, false);
            }
            self.position += tics;
            fait += tics / pas;
            if tics <= 0.0 {
                break;
            }
        }
    }

    /// Clics du métronome sur les temps compris dans [a, b) (tics), accent sur le premier temps.
    #[allow(clippy::too_many_arguments)]
    fn clics(&self, noyau: &mut Noyau, depart: u64, a: f64, b: f64, deja: f64, pas: f64, decompte: bool) {
        let mut temps = ((a - 1e-6) / TICS_PAR_NOIRE).ceil() * TICS_PAR_NOIRE;
        while temps < b {
            let numero = (temps / TICS_PAR_NOIRE).round() as u32;
            let accent = numero % self.prepa.session.temps_par_mesure.max(1) == 0;
            let echantillon = depart + (deja + (temps - a) / pas).round() as u64;
            noyau.clic(echantillon, accent || decompte && numero == 0);
            temps += TICS_PAR_NOIRE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noyau::tests::{FE, banque, configuration, regler};

    fn efficace(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
    }

    /// Rend `secondes` en faisant avancer le transport bloc par bloc (blocs irréguliers).
    fn jouer(transport: &mut Transport, noyau: &mut Noyau, secondes: f32) -> Vec<f32> {
        let n = (secondes * FE as f32) as usize;
        let (mut g, mut d) = (vec![0.0; n], vec![0.0; n]);
        let mut fait = 0;
        for taille in [256usize, 441, 1000, 37].iter().cycle() {
            if fait >= n {
                break;
            }
            let fin = (fait + taille).min(n);
            transport.avancer(noyau, fin - fait);
            noyau.rendre(&mut g[fait..fin], &mut d[fait..fin]);
            fait = fin;
        }
        g
    }

    /// Début du son (premier échantillon au-dessus d'un seuil) à partir de `depuis`.
    fn attaque(x: &[f32], depuis: usize) -> usize {
        depuis + x[depuis..].iter().position(|v| v.abs() > 0.002).expect("aucun son")
    }

    fn session(notes: Vec<NoteSession>) -> Session {
        // tempo 120 : une noire = 0,5 s ; boucle d'une mesure = 2 s
        Session { tempo: 120.0, longueur: 384, temps_par_mesure: 4, notes, boucler: true }
    }

    #[test]
    fn les_notes_tombent_a_l_echantillon_pres_et_la_boucle_reprend() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        regler(&mut noyau, &configuration(0, false)); // piano
        let mut t = Transport::new();
        t.definir_session(session(vec![NoteSession { debut: 192, fin: 240, canal: 0, hauteur: 72, velocite: 110, pan: 0, cents: 0 }]), &mut noyau);
        t.agir(ActionTransport::Jouer { decompte: false }, &mut noyau);
        let g = jouer(&mut t, &mut noyau, 3.2);
        // la note est au 3e temps : 1 s, puis 3 s au tour suivant
        // l'attaque du piano et les blocs internes du synthétiseur (32 échantillons) retardent un
        // peu le son audible ; d'un tour de boucle à l'autre, l'écart doit être exact
        let premiere = attaque(&g, 0);
        assert!((premiere as i64 - FE as i64).abs() < 150, "attaque à {premiere}");
        let seconde = attaque(&g, (2.9 * FE as f32) as usize);
        assert!((seconde as i64 - premiere as i64 - 2 * FE as i64).abs() <= 32, "tour de boucle : {}", seconde - premiere);
        assert!(efficace(&g[..(0.95 * FE as f32) as usize]) < 1e-5);
        assert!((t.position_absolue() - 3.2 * 192.0).abs() < 0.5);
        assert!((t.position() - (3.2 * 192.0 - 384.0)).abs() < 0.5);
    }

    #[test]
    fn pause_arret_et_recherche() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        regler(&mut noyau, &configuration(48, false)); // cordes tenues
        let mut t = Transport::new();
        t.definir_session(session(vec![NoteSession { debut: 0, fin: 384, canal: 0, hauteur: 60, velocite: 100, pan: 0, cents: 0 }]), &mut noyau);
        t.agir(ActionTransport::Jouer { decompte: false }, &mut noyau);
        jouer(&mut t, &mut noyau, 0.5);
        t.agir(ActionTransport::Pause, &mut noyau);
        assert_eq!(t.etat(), EtatTransport::Pause);
        assert!((t.position() - 96.0).abs() < 0.5);
        let g = jouer(&mut t, &mut noyau, 1.5);
        assert!(efficace(&g[(1.2 * FE as f32) as usize..]) < 1e-3, "plus rien après la pause");
        t.agir(ActionTransport::Chercher(500.0), &mut noyau);
        assert!((t.position() - 116.0).abs() < 1e-9, "recherche ramenée dans la boucle");
        t.agir(ActionTransport::Arreter, &mut noyau);
        assert_eq!((t.etat(), t.position()), (EtatTransport::Arret, 0.0));
    }

    #[test]
    fn note_retiree_en_pleine_lecture_relachee_les_autres_continuent() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        let mut c = configuration(48, false);
        c.canaux.push(c.canaux[0]);
        regler(&mut noyau, &c);
        let a = NoteSession { debut: 0, fin: 384, canal: 0, hauteur: 60, velocite: 100, pan: 0, cents: 0 };
        let b = NoteSession { debut: 0, fin: 384, canal: 1, hauteur: 67, velocite: 100, pan: 0, cents: 0 };
        let mut t = Transport::new();
        t.definir_session(session(vec![a, b]), &mut noyau);
        t.agir(ActionTransport::Jouer { decompte: false }, &mut noyau);
        jouer(&mut t, &mut noyau, 0.3);
        t.definir_session(session(vec![b]), &mut noyau);
        assert!(!t.allumees[0][60] && t.allumees[1][67]);
    }

    #[test]
    fn metronome_et_decompte() {
        let mut noyau = Noyau::new(&banque(), FE).unwrap();
        let mut t = Transport::new();
        t.definir_session(session(vec![]), &mut noyau);
        t.agir(ActionTransport::Metronome(true), &mut noyau);
        t.agir(ActionTransport::Jouer { decompte: true }, &mut noyau);
        let g = jouer(&mut t, &mut noyau, 2.6);
        // décompte : 4 clics (0 ; 0,5 ; 1 ; 1,5 s), puis le premier temps de la mesure à 2 s
        for seconde in [0.0f32, 0.5, 1.0, 1.5, 2.0] {
            let i = (seconde * FE as f32) as usize;
            assert!(efficace(&g[i..i + 400]) > 0.01, "clic à {seconde} s");
            assert!(efficace(&g[i + 8000..i + 9000]) < 1e-3, "silence après le clic de {seconde} s");
        }
        assert!(t.position() > 0.0 && t.position() < 120.0, "la position n'avance qu'après le décompte");
    }
}
