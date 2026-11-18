//! Actions du rack sur les canaux : cloner, supprimer, déplacer, renommer, colorer, remplacer
//! le son. Fonctions pures sur le projet (testées).

use crate::modele::{Canal, Projet, SourceCanal, creer_piste, nom_du_son, nouvel_id};

/// Couleurs proposées pour les canaux (les mêmes que celles données à la création).
pub const PALETTE: [&str; 16] = [
    "#f5a524", "#4cc38a", "#5aa9f5", "#e5679a", "#a78bfa", "#f97362", "#2dd4bf", "#facc15",
    "#fb923c", "#84cc16", "#38bdf8", "#f472b6", "#c084fc", "#ef4444", "#14b8a6", "#94a3b8",
];

fn rang(projet: &Projet, id: &str) -> Option<usize> {
    projet.canaux.iter().position(|c| c.id == id)
}

/// Nom libre dérivé de `nom` : « Nom 2 », « Nom 3 »… (un nom qui finit déjà par un numéro le
/// remplace).
pub fn nom_libre(projet: &Projet, nom: &str) -> String {
    let base = match nom.rsplit_once(' ') {
        Some((debut, fin)) if !debut.is_empty() && fin.parse::<u32>().is_ok() => debut,
        _ => nom,
    };
    (2..).map(|n| format!("{base} {n}")).find(|candidat| !projet.canaux.iter().any(|c| &c.nom == candidat)).unwrap_or_else(|| base.to_string())
}

/// Copie du canal juste après lui, avec ses notes dans chaque pattern et sa propre piste de
/// mixage (mêmes réglages que celle de l'original). Renvoie l'identifiant de la copie.
pub fn cloner_canal(projet: &mut Projet, id: &str) -> Option<String> {
    let r = rang(projet, id)?;
    let original = projet.canaux[r].clone();
    let nouvel = nouvel_id("canal");
    let nom = nom_libre(projet, &original.nom);
    let index = projet.mixage.iter().map(|p| p.index + 1).max().unwrap_or(1);
    let mut piste = projet.mixage.iter().find(|p| p.index == original.piste_mixage && p.index != 0).cloned().unwrap_or_else(|| creer_piste(index, &nom));
    piste.index = index;
    piste.nom = nom.clone();
    projet.mixage.push(piste);
    projet.canaux.insert(r + 1, Canal { id: nouvel.clone(), nom, piste_mixage: index, solo: false, ..original });
    for pattern in projet.patterns.iter_mut() {
        if let Some(notes) = pattern.notes.get(id).cloned() {
            pattern.notes.insert(nouvel.clone(), notes);
        }
    }
    Some(nouvel)
}

/// Retire le canal et ses notes de tous les patterns. Sa piste de mixage reste (d'autres
/// canaux peuvent y être routés). Faux si le canal n'existe pas.
pub fn supprimer_canal(projet: &mut Projet, id: &str) -> bool {
    let Some(r) = rang(projet, id) else { return false };
    projet.canaux.remove(r);
    for pattern in projet.patterns.iter_mut() {
        pattern.notes.remove(id);
    }
    true
}

/// Déplace le canal au rang voulu (borné à la liste).
pub fn deplacer_canal(projet: &mut Projet, id: &str, vers: usize) {
    let Some(r) = rang(projet, id) else { return };
    let canal = projet.canaux.remove(r);
    let vers = vers.min(projet.canaux.len());
    projet.canaux.insert(vers, canal);
}

/// Renomme le canal ; un nom vide (après retrait des espaces) est refusé.
pub fn renommer_canal(projet: &mut Projet, id: &str, nom: &str) -> bool {
    let nom = nom.trim();
    match projet.canal_mut(id) {
        Some(c) if !nom.is_empty() => {
            c.nom = nom.to_string();
            true
        }
        _ => false,
    }
}

pub fn colorer_canal(projet: &mut Projet, id: &str, couleur: &str) {
    if let Some(c) = projet.canal_mut(id) {
        c.couleur = couleur.to_string();
    }
}

/// Remplace le son d'un canal mélodique (instrument General MIDI ou préréglage du synthé) en
/// gardant ses notes et ses réglages. Un canal qui portait le nom de son ancien son prend celui
/// du nouveau ; un nom choisi par l'utilisateur est gardé.
pub fn remplacer_son(projet: &mut Projet, id: &str, source: SourceCanal, programme: u8) {
    let Some(c) = projet.canal_mut(id) else { return };
    if c.batterie {
        return;
    }
    let nom_auto = c.nom == nom_du_son(c.source, c.programme);
    c.source = source;
    c.programme = programme;
    if nom_auto {
        c.nom = nom_du_son(source, programme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modele::{NoteMusicale, OptionsCanal, creer_pattern, creer_projet};
    use crate::pas::definir_pas;
    use crate::serialisation::{deserialiser, serialiser};

    fn trois() -> (Projet, [String; 3]) {
        let mut p = creer_projet("Essai");
        let a = p.ajouter_canal(OptionsCanal { batterie: true, nom: Some("Grosse caisse".into()), ..Default::default() });
        let b = p.ajouter_canal(OptionsCanal { programme: 33, ..Default::default() });
        let c = p.ajouter_canal(OptionsCanal { programme: 0, ..Default::default() });
        (p, [a, b, c])
    }

    fn noms(p: &Projet) -> Vec<&str> {
        p.canaux.iter().map(|c| c.nom.as_str()).collect()
    }

    #[test]
    fn cloner_copie_reglages_notes_et_piste() {
        let (mut p, [a, b, _]) = trois();
        definir_pas(&mut p, &a, 0, None);
        definir_pas(&mut p, &a, 4, None);
        // un deuxième pattern avec des notes du canal
        let mut deuxieme = creer_pattern("Pattern 2", 384);
        deuxieme.notes.insert(a.clone(), vec![NoteMusicale { tic: 48, duree: 22, hauteur: 36, velocite: 70 }]);
        p.patterns.push(deuxieme);
        p.canal_mut(&a).unwrap().volume = 0.5;
        p.canal_mut(&a).unwrap().solo = true;
        p.mixage[1].reverb = 0.4;
        let copie = cloner_canal(&mut p, &a).unwrap();
        assert_eq!(noms(&p), vec!["Grosse caisse", "Grosse caisse 2", "Basse électrique (doigts)", "Piano à queue"]);
        let c = p.canal(&copie).unwrap();
        assert_eq!((c.volume, c.batterie, c.note_defaut, c.solo), (0.5, true, 36, false));
        assert_eq!(p.patterns[0].notes[&copie], p.patterns[0].notes[&a]);
        assert_eq!(p.patterns[1].notes[&copie][0].velocite, 70);
        // sa propre piste, avec les réglages de celle de l'original
        assert_eq!(c.piste_mixage, 4);
        assert_eq!((p.mixage[4].index, p.mixage[4].nom.as_str(), p.mixage[4].reverb), (4, "Grosse caisse 2", 0.4));
        // une copie de la copie : « 3 », pas « 2 2 »
        let encore = cloner_canal(&mut p, &copie).unwrap();
        assert_eq!(p.canal(&encore).unwrap().nom, "Grosse caisse 3");
        assert_eq!(cloner_canal(&mut p, "inconnu"), None);
        let _ = b;
        // le projet reste valide
        assert_eq!(deserialiser(&serialiser(&p)).unwrap(), p);
    }

    #[test]
    fn supprimer_retire_le_canal_et_ses_notes() {
        let (mut p, [a, b, c]) = trois();
        definir_pas(&mut p, &a, 0, None);
        definir_pas(&mut p, &b, 0, None);
        let pistes = p.mixage.len();
        assert!(supprimer_canal(&mut p, &a));
        assert_eq!(noms(&p), vec!["Basse électrique (doigts)", "Piano à queue"]);
        assert!(!p.pattern_actif().notes.contains_key(&a));
        assert!(p.pattern_actif().notes.contains_key(&b));
        assert_eq!(p.mixage.len(), pistes, "les pistes restent");
        assert!(!supprimer_canal(&mut p, &a), "déjà supprimé");
        assert!(supprimer_canal(&mut p, &b) && supprimer_canal(&mut p, &c));
        assert!(p.canaux.is_empty());
        assert_eq!(deserialiser(&serialiser(&p)).unwrap(), p);
    }

    #[test]
    fn deplacer_dans_les_bornes() {
        let (mut p, [a, _, c]) = trois();
        deplacer_canal(&mut p, &a, 2);
        assert_eq!(noms(&p), vec!["Basse électrique (doigts)", "Piano à queue", "Grosse caisse"]);
        deplacer_canal(&mut p, &a, 0);
        assert_eq!(noms(&p)[0], "Grosse caisse");
        deplacer_canal(&mut p, &c, 99);
        assert_eq!(noms(&p)[2], "Piano à queue");
        deplacer_canal(&mut p, "inconnu", 0);
        assert_eq!(p.canaux.len(), 3);
    }

    #[test]
    fn renommer_colorer_remplacer() {
        let (mut p, [a, b, c]) = trois();
        assert!(renommer_canal(&mut p, &a, "  Kick  "));
        assert_eq!(p.canal(&a).unwrap().nom, "Kick");
        assert!(!renommer_canal(&mut p, &a, "   "));
        assert_eq!(p.canal(&a).unwrap().nom, "Kick");
        colorer_canal(&mut p, &a, PALETTE[9]);
        assert_eq!(p.canal(&a).unwrap().couleur, "#84cc16");
        // nom automatique : il suit le son
        remplacer_son(&mut p, &b, SourceCanal::Soundfont, 38);
        assert_eq!((p.canal(&b).unwrap().programme, p.canal(&b).unwrap().nom.as_str()), (38, "Basse synthé 1"));
        remplacer_son(&mut p, &b, SourceCanal::Soustractif, 0);
        assert_eq!(p.canal(&b).unwrap().source, SourceCanal::Soustractif);
        assert_eq!(p.canal(&b).unwrap().nom, nom_du_son(SourceCanal::Soustractif, 0));
        // nom choisi : gardé
        renommer_canal(&mut p, &c, "Mon piano");
        remplacer_son(&mut p, &c, SourceCanal::Soundfont, 4);
        assert_eq!((p.canal(&c).unwrap().programme, p.canal(&c).unwrap().nom.as_str()), (4, "Mon piano"));
        // une batterie ne change pas de son ici (son de batterie : la liste du rack)
        remplacer_son(&mut p, &a, SourceCanal::Soundfont, 10);
        assert_eq!(p.canal(&a).unwrap().programme, 0);
    }

    #[test]
    fn noms_libres() {
        let (mut p, [a, ..]) = trois();
        assert_eq!(nom_libre(&p, "Grosse caisse"), "Grosse caisse 2");
        renommer_canal(&mut p, &a, "Grosse caisse 2");
        assert_eq!(nom_libre(&p, "Grosse caisse 2"), "Grosse caisse 3");
        assert_eq!(nom_libre(&p, "Pad 1984"), "Pad 2");
        assert_eq!(nom_libre(&p, "5"), "5 2");
    }
}
