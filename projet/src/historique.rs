//! Annuler et rétablir : une pile d'états du projet, chacun nommé d'après ce qui a changé
//! (« Notes de Piano », « Clip posé »…), pour la liste visible de l'historique. Changer de pattern
//! édité ne compte pas comme une étape.

use crate::modele::Projet;

/// Étapes gardées au plus (les plus anciennes partent).
pub const ETAPES_MAX: usize = 200;

#[derive(Clone, Debug, PartialEq)]
pub struct Etape {
    /// L'état du projet avant cette étape (en annulant) ou après (en rétablissant).
    pub projet: Projet,
    pub libelle: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Historique {
    passe: Vec<Etape>,
    futur: Vec<Etape>,
}

/// Vrai si les deux projets ne diffèrent que par le pattern édité.
pub fn meme_contenu(a: &Projet, b: &Projet) -> bool {
    a.pattern_actif != b.pattern_actif && Projet { pattern_actif: b.pattern_actif.clone(), ..a.clone() } == *b
}

/// Ce qui a changé entre deux états, en quelques mots.
pub fn decrire_changement(avant: &Projet, apres: &Projet) -> String {
    use std::cmp::Ordering::*;
    let nom_canal = |id: &str| apres.canal(id).or_else(|| avant.canal(id)).map(|c| c.nom.clone()).unwrap_or_default();
    match apres.canaux.len().cmp(&avant.canaux.len()) {
        Greater => return "Instrument ajouté".into(),
        Less => return "Instrument supprimé".into(),
        Equal => {}
    }
    if avant.canaux.iter().zip(&apres.canaux).any(|(a, b)| a.id != b.id) {
        return "Ordre des instruments".into();
    }
    if let Some((a, b)) = avant.canaux.iter().zip(&apres.canaux).find(|(a, b)| a != b) {
        return if a.nom != b.nom { format!("Nom de « {} »", b.nom) } else { format!("Réglages de {}", b.nom) };
    }
    match apres.patterns.len().cmp(&avant.patterns.len()) {
        Greater => return "Pattern ajouté".into(),
        Less => return "Pattern supprimé".into(),
        Equal => {}
    }
    for (a, b) in avant.patterns.iter().zip(&apres.patterns) {
        if a.id != b.id {
            return "Ordre des patterns".into();
        }
        if a.nom != b.nom {
            return format!("Nom du pattern « {} »", b.nom);
        }
        if a.longueur != b.longueur {
            return format!("Longueur de {}", b.nom);
        }
        if a.notes != b.notes {
            let canal = b.notes.keys().chain(a.notes.keys()).find(|id| a.notes.get(*id) != b.notes.get(*id));
            return canal.map(|id| format!("Notes de {}", nom_canal(id))).unwrap_or_else(|| "Notes".into());
        }
    }
    match apres.playlist.len().cmp(&avant.playlist.len()) {
        Greater => return "Clip posé".into(),
        Less => return "Clip supprimé".into(),
        Equal if avant.playlist != apres.playlist => return "Clip déplacé".into(),
        Equal => {}
    }
    match apres.mixage.len().cmp(&avant.mixage.len()) {
        Greater => return "Piste de mixage ajoutée".into(),
        Less => return "Pistes de mixage retirées".into(),
        Equal => {}
    }
    if let Some((_, b)) = avant.mixage.iter().zip(&apres.mixage).find(|(a, b)| a != b) {
        return format!("Mixage : {}", b.nom);
    }
    if avant.tempo != apres.tempo {
        return "Tempo".into();
    }
    if avant.swing != apres.swing {
        return "Swing".into();
    }
    if avant.nom != apres.nom || avant.infos != apres.infos || avant.signature != apres.signature {
        return "Infos du projet".into();
    }
    "Modification".into()
}

impl Historique {
    /// Consigne une étape : `avant` est l'état d'avant la modification qui a donné `apres`.
    pub fn consigner(&mut self, avant: Projet, apres: &Projet) {
        let libelle = decrire_changement(&avant, apres);
        self.passe.push(Etape { projet: avant, libelle });
        if self.passe.len() > ETAPES_MAX {
            self.passe.remove(0);
        }
        self.futur.clear();
    }

    /// Revient à l'état d'avant la dernière étape ; `actuel` passe dans le futur.
    pub fn annuler(&mut self, actuel: &Projet) -> Option<Projet> {
        let etape = self.passe.pop()?;
        self.futur.push(Etape { projet: actuel.clone(), libelle: etape.libelle.clone() });
        Some(etape.projet)
    }

    pub fn retablir(&mut self, actuel: &Projet) -> Option<Projet> {
        let etape = self.futur.pop()?;
        self.passe.push(Etape { projet: actuel.clone(), libelle: etape.libelle.clone() });
        Some(etape.projet)
    }

    /// Libellé de l'étape qu'annulerait « Annuler ».
    pub fn a_annuler(&self) -> Option<&str> {
        self.passe.last().map(|e| e.libelle.as_str())
    }

    pub fn a_retablir(&self) -> Option<&str> {
        self.futur.last().map(|e| e.libelle.as_str())
    }

    /// Libellés des étapes passées, de la plus récente à la plus ancienne.
    pub fn etapes(&self) -> Vec<&str> {
        self.passe.iter().rev().map(|e| e.libelle.as_str()).collect()
    }

    pub fn vider(&mut self) {
        self.passe.clear();
        self.futur.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::{OptionsCanal, creer_projet};
    use crate::pas::definir_pas;
    use crate::playlist::{ajouter_clip, deplacer_clip, nouveau_pattern};

    #[test]
    fn annuler_et_retablir_dans_l_ordre() {
        let mut h = Historique::default();
        let p0 = creer_projet("Essai");
        let mut p1 = p0.clone();
        let id = p1.ajouter_canal(OptionsCanal { nom: Some("Piano".into()), ..Default::default() });
        h.consigner(p0.clone(), &p1);
        let mut p2 = p1.clone();
        definir_pas(&mut p2, &id, 0, None);
        h.consigner(p1.clone(), &p2);
        assert_eq!(h.etapes(), vec!["Notes de Piano", "Instrument ajouté"]);
        assert_eq!(h.annuler(&p2), Some(p1.clone()));
        assert_eq!(h.a_retablir(), Some("Notes de Piano"));
        assert_eq!(h.annuler(&p1), Some(p0.clone()));
        assert_eq!(h.annuler(&p0), None, "rien de plus à annuler");
        assert_eq!(h.retablir(&p0), Some(p1.clone()));
        assert_eq!(h.retablir(&p1), Some(p2.clone()));
        assert_eq!(h.retablir(&p2), None);
        // une nouvelle étape efface le futur
        h.annuler(&p2);
        h.consigner(p1.clone(), &p0);
        assert_eq!(h.a_retablir(), None);
        h.vider();
        assert_eq!(h.a_annuler(), None);
    }

    #[test]
    fn au_plus_200_etapes() {
        let mut h = Historique::default();
        let mut p = creer_projet("Essai");
        for i in 0..250 {
            let avant = p.clone();
            p.tempo = 60.0 + i as f64;
            h.consigner(avant, &p);
        }
        assert_eq!(h.etapes().len(), ETAPES_MAX);
        let mut n = 0;
        while let Some(x) = h.annuler(&p) {
            p = x;
            n += 1;
        }
        assert_eq!(n, ETAPES_MAX);
        assert_eq!(p.tempo, 60.0 + 49.0, "les 50 plus anciennes sont parties");
    }

    #[test]
    fn libelles_d_apres_ce_qui_a_change() {
        let mut p = creer_projet("Essai");
        let id = p.ajouter_canal(OptionsCanal { nom: Some("Basse".into()), ..Default::default() });
        let d = |f: &dyn Fn(&mut Projet)| {
            let mut q = p.clone();
            f(&mut q);
            decrire_changement(&p, &q)
        };
        assert_eq!(d(&|q| q.tempo = 90.0), "Tempo");
        assert_eq!(d(&|q| q.swing = 0.5), "Swing");
        assert_eq!(d(&|q| q.canaux[0].volume = 0.1), "Réglages de Basse");
        assert_eq!(d(&|q| q.canaux[0].nom = "Basse 2".into()), "Nom de « Basse 2 »");
        assert_eq!(d(&|q| q.canaux.clear()), "Instrument supprimé");
        assert_eq!(d(&|q| definir_pas(q, &id, 3, None)), "Notes de Basse");
        assert_eq!(d(&|q| q.pattern_actif_mut().longueur = 768), "Longueur de Pattern 1");
        assert_eq!(d(&|q| { nouveau_pattern(q); }), "Pattern ajouté");
        assert_eq!(d(&|q| q.mixage[1].volume = 0.2), "Mixage : Basse");
        assert_eq!(d(&|q| q.infos.auteur = "Moi".into()), "Infos du projet");
        let pattern = p.pattern_actif.clone();
        assert_eq!(d(&|q| { ajouter_clip(q, &pattern, 0, 0); }), "Clip posé");
        let mut avec_clip = p.clone();
        let clip = ajouter_clip(&mut avec_clip, &pattern, 0, 0).unwrap();
        let mut deplace = avec_clip.clone();
        deplacer_clip(&mut deplace, &clip, 2, 384);
        assert_eq!(decrire_changement(&avec_clip, &deplace), "Clip déplacé");
    }

    #[test]
    fn changer_de_pattern_edite_n_est_pas_une_etape() {
        let mut p = creer_projet("Essai");
        let deuxieme = nouveau_pattern(&mut p);
        let mut q = p.clone();
        q.pattern_actif = p.patterns[0].id.clone();
        assert!(meme_contenu(&p, &q));
        q.tempo = 70.0;
        assert!(!meme_contenu(&p, &q));
        assert!(!meme_contenu(&p, &p), "rien de changé : ce n'est pas un simple changement de pattern");
        let _ = deuxieme;
    }
}
