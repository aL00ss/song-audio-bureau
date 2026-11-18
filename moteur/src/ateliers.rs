//! Rendu sur tous les cœurs. Des fils de calcul (autant que de cœurs, moins un) se partagent les
//! canaux à rendre pendant un bloc audio, sans verrou ni allocation.
//!
//! Un seul entier atomique de 64 bits décrit le travail en cours : numéro du bloc (16 bits),
//! nombre de tâches (24 bits) et prochaine tâche à prendre (24 bits). Chaque fil réserve sa
//! tâche par une comparaison-échange sur cet entier : un fil en retard, qui aurait vu un bloc
//! précédent, échoue forcément (le numéro a changé) et ne peut jamais prendre une tâche d'un
//! autre bloc. Le fil audio publie le travail, réveille les fils, travaille lui aussi, puis
//! attend (en tournant, quelques microsecondes) que toutes les tâches soient finies.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle, Thread};
use std::time::Duration;

const BITS_INDEX: u32 = 24;
const MASQUE: u64 = (1 << BITS_INDEX) - 1;
/// Tâches par bloc au plus (16 millions).
pub const TACHES_MAX: usize = MASQUE as usize;

fn emballer(bloc: u64, total: usize, prochain: usize) -> u64 {
    ((bloc & 0xFFFF) << (2 * BITS_INDEX)) | ((total as u64 & MASQUE) << BITS_INDEX) | (prochain as u64 & MASQUE)
}

fn deballer(v: u64) -> (u64, usize, usize) {
    (v >> (2 * BITS_INDEX), ((v >> BITS_INDEX) & MASQUE) as usize, (v & MASQUE) as usize)
}

/// Une tâche : rendre l'élément `index` (le contexte est derrière le pointeur, valide pendant
/// tout le bloc).
pub type FonctionTache = unsafe fn(contexte: *const (), index: usize);

struct Partage {
    travail: AtomicU64,
    termines: AtomicUsize,
    contexte: AtomicUsize,
    fonction: AtomicUsize,
    arret: AtomicBool,
}

pub struct Ateliers {
    partage: Arc<Partage>,
    fils: Vec<(Thread, JoinHandle<()>)>,
    bloc: u64,
}

impl Ateliers {
    /// Autant de fils que de cœurs, moins un (le fil audio travaille aussi) ; au plus `maximum`.
    pub fn new(maximum: usize) -> Self {
        let coeurs = thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        Self::avec_fils(coeurs.saturating_sub(1).min(maximum))
    }

    pub fn avec_fils(nombre: usize) -> Self {
        let partage = Arc::new(Partage {
            travail: AtomicU64::new(emballer(0, 0, 0)),
            termines: AtomicUsize::new(0),
            contexte: AtomicUsize::new(0),
            fonction: AtomicUsize::new(0),
            arret: AtomicBool::new(false),
        });
        let fils = (0..nombre)
            .filter_map(|i| {
                let p = partage.clone();
                thread::Builder::new()
                    .name(format!("song-audio-calcul-{i}"))
                    .spawn(move || boucle(&p))
                    .ok()
                    .map(|h| (h.thread().clone(), h))
            })
            .collect();
        Ateliers { partage, fils, bloc: 0 }
    }

    pub fn nombre_de_fils(&self) -> usize {
        self.fils.len()
    }

    /// Exécute `fonction(contexte, i)` pour i de 0 à `total` - 1, réparti sur tous les fils ;
    /// rend la main quand tout est fini.
    ///
    /// # Sûreté
    /// `fonction` doit pouvoir être appelée depuis plusieurs fils à la fois avec des indices
    /// différents, et `contexte` rester valide jusqu'au retour.
    pub unsafe fn executer(&mut self, contexte: *const (), fonction: FonctionTache, total: usize) {
        let total = total.min(TACHES_MAX);
        if total == 0 {
            return;
        }
        let p = &*self.partage;
        self.bloc = (self.bloc + 1) & 0xFFFF;
        p.termines.store(0, Ordering::Relaxed);
        p.contexte.store(contexte as usize, Ordering::Relaxed);
        p.fonction.store(fonction as usize, Ordering::Relaxed);
        // publication : tout ce qui précède devient visible aux fils qui voient ce bloc
        p.travail.store(emballer(self.bloc, total, 0), Ordering::Release);
        for (fil, _) in &self.fils {
            fil.unpark();
        }
        travailler(p, self.bloc);
        while p.termines.load(Ordering::Acquire) < total {
            std::hint::spin_loop();
        }
    }
}

/// Prend et exécute des tâches du bloc `bloc` tant qu'il en reste.
fn travailler(p: &Partage, bloc: u64) {
    loop {
        let v = p.travail.load(Ordering::Acquire);
        let (b, total, prochain) = deballer(v);
        if b != bloc || prochain >= total {
            return;
        }
        if p.travail.compare_exchange_weak(v, emballer(b, total, prochain + 1), Ordering::AcqRel, Ordering::Acquire).is_ok() {
            // la tâche est à nous : le bloc ne peut pas finir (ni le contexte changer) avant elle
            let contexte = p.contexte.load(Ordering::Relaxed) as *const ();
            let fonction: FonctionTache = unsafe { std::mem::transmute::<usize, FonctionTache>(p.fonction.load(Ordering::Relaxed)) };
            unsafe { fonction(contexte, prochain) };
            p.termines.fetch_add(1, Ordering::Release);
        }
    }
}

fn boucle(p: &Partage) {
    let mut dernier = u64::MAX;
    while !p.arret.load(Ordering::Acquire) {
        let (bloc, total, prochain) = deballer(p.travail.load(Ordering::Acquire));
        if bloc != dernier && prochain < total {
            travailler(p, bloc);
            dernier = bloc;
            continue;
        }
        dernier = bloc;
        // un peu d'attente active (les blocs se suivent de près), puis sommeil court
        let mut nouveau = false;
        for _ in 0..2000 {
            std::hint::spin_loop();
            let (b, t, n) = deballer(p.travail.load(Ordering::Relaxed));
            if b != dernier && n < t {
                nouveau = true;
                break;
            }
        }
        if !nouveau {
            thread::park_timeout(Duration::from_millis(2));
        }
    }
}

impl Drop for Ateliers {
    fn drop(&mut self) {
        self.partage.arret.store(true, Ordering::Release);
        for (fil, _) in &self.fils {
            fil.unpark();
        }
        for (_, h) in self.fils.drain(..) {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    struct Compteurs(Vec<AtomicU32>);

    unsafe fn incrementer(contexte: *const (), index: usize) {
        let c = unsafe { &*(contexte as *const Compteurs) };
        c.0[index].fetch_add(1, Ordering::Relaxed);
    }

    #[test]
    fn chaque_tache_exactement_une_fois_sur_des_milliers_de_blocs() {
        let mut a = Ateliers::avec_fils(3);
        let c = Compteurs((0..500).map(|_| AtomicU32::new(0)).collect());
        for bloc in 0..3000usize {
            let total = 1 + bloc % 500;
            unsafe { a.executer(&c as *const Compteurs as *const (), incrementer, total) };
            // chaque bloc est fini au retour : les tâches 0..total ont été faites une fois de plus
        }
        let attendu = |i: usize| (0..3000usize).filter(|b| i < 1 + b % 500).count() as u32;
        for (i, compteur) in c.0.iter().enumerate() {
            assert_eq!(compteur.load(Ordering::Relaxed), attendu(i), "tâche {i}");
        }
    }

    #[test]
    fn sans_fil_le_travail_se_fait_quand_meme() {
        let mut a = Ateliers::avec_fils(0);
        let c = Compteurs((0..10).map(|_| AtomicU32::new(0)).collect());
        unsafe { a.executer(&c as *const Compteurs as *const (), incrementer, 10) };
        assert!(c.0.iter().all(|x| x.load(Ordering::Relaxed) == 1));
    }
}
