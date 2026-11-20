//! Sauvegarde du projet en JSON versionné, avec une vérification stricte au chargement (mêmes
//! règles et mêmes valeurs par défaut que la version web) : un fichier abîmé ou d'une autre
//! version est refusé avec un message clair.

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use serde_json::{Map, Value};

use crate::modele::{Canal, ClipPlaylist, Egaliseur, InfosProjet, NoteMusicale, Pattern, PisteMixage, Projet, REVERB_DEFAUT, SourceCanal, VERSION_PROJET};
use crate::temps::Signature;

#[derive(Debug, Clone, PartialEq)]
pub struct ErreurProjet(pub String);

impl fmt::Display for ErreurProjet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ErreurProjet {}

type R<T> = Result<T, ErreurProjet>;

fn erreur<T>(message: String) -> R<T> {
    Err(ErreurProjet(message))
}

pub fn serialiser(projet: &Projet) -> String {
    serde_json::to_string(projet).unwrap_or_default()
}

fn objet<'a>(v: &'a Value, chemin: &str) -> R<&'a Map<String, Value>> {
    v.as_object().ok_or_else(|| ErreurProjet(format!("{chemin} : objet attendu")))
}

fn champ<'a>(o: &'a Map<String, Value>, nom: &str) -> &'a Value {
    o.get(nom).unwrap_or(&Value::Null)
}

fn nombre(v: &Value, chemin: &str, min: f64, max: f64) -> R<f64> {
    match v.as_f64() {
        Some(n) if n.is_finite() && n >= min && n <= max => Ok(n),
        _ => erreur(format!("{chemin} : nombre entre {min} et {max} attendu")),
    }
}

fn entier(v: &Value, chemin: &str, min: f64, max: f64) -> R<u64> {
    let n = nombre(v, chemin, min, max)?;
    if n.fract() != 0.0 {
        return erreur(format!("{chemin} : nombre entier attendu"));
    }
    Ok(n as u64)
}

fn texte(v: &Value, chemin: &str) -> R<String> {
    v.as_str().map(str::to_string).ok_or_else(|| ErreurProjet(format!("{chemin} : texte attendu")))
}

fn booleen(v: &Value, chemin: &str) -> R<bool> {
    v.as_bool().ok_or_else(|| ErreurProjet(format!("{chemin} : vrai ou faux attendu")))
}

fn tableau<'a>(v: &'a Value, chemin: &str) -> R<&'a Vec<Value>> {
    v.as_array().ok_or_else(|| ErreurProjet(format!("{chemin} : liste attendue")))
}

const INFINI: f64 = f64::INFINITY;

fn lire_note(v: &Value, chemin: &str) -> R<NoteMusicale> {
    let o = objet(v, chemin)?;
    Ok(NoteMusicale {
        tic: entier(champ(o, "tic"), &format!("{chemin}.tic"), 0.0, u32::MAX as f64)? as u32,
        duree: entier(champ(o, "duree"), &format!("{chemin}.duree"), 1.0, u32::MAX as f64)? as u32,
        hauteur: entier(champ(o, "hauteur"), &format!("{chemin}.hauteur"), 0.0, 127.0)? as u8,
        velocite: entier(champ(o, "velocite"), &format!("{chemin}.velocite"), 1.0, 127.0)? as u8,
        pan: match o.get("pan") {
            None => 0,
            Some(v) => entier_signe(v, &format!("{chemin}.pan"), -100.0, 100.0)? as i8,
        },
        fin: match o.get("fin") {
            None => 0,
            Some(v) => entier_signe(v, &format!("{chemin}.fin"), -100.0, 100.0)? as i16,
        },
        relachement: match o.get("relachement") {
            None => crate::modele::RELACHEMENT_DEFAUT,
            Some(v) => entier(v, &format!("{chemin}.relachement"), 1.0, 127.0)? as u8,
        },
        decalage: match o.get("decalage") {
            None => 0,
            Some(v) => entier(v, &format!("{chemin}.decalage"), 0.0, (crate::temps::TICS_PAR_PAS - 1) as f64)? as u8,
        },
    })
}

/// Entier relatif dans des bornes.
fn entier_signe(v: &Value, chemin: &str, min: f64, max: f64) -> R<i64> {
    let n = nombre(v, chemin, min, max)?;
    if n.fract() != 0.0 {
        return erreur(format!("{chemin} : nombre entier attendu"));
    }
    Ok(n as i64)
}

fn lire_canal(v: &Value, chemin: &str) -> R<Canal> {
    let o = objet(v, chemin)?;
    let batterie = booleen(champ(o, "batterie"), &format!("{chemin}.batterie"))?;
    Ok(Canal {
        id: texte(champ(o, "id"), &format!("{chemin}.id"))?,
        nom: texte(champ(o, "nom"), &format!("{chemin}.nom"))?,
        programme: entier(champ(o, "programme"), &format!("{chemin}.programme"), 0.0, 127.0)? as u8,
        batterie,
        volume: nombre(champ(o, "volume"), &format!("{chemin}.volume"), 0.0, 1.0)? as f32,
        pan: nombre(champ(o, "pan"), &format!("{chemin}.pan"), -1.0, 1.0)? as f32,
        muet: booleen(champ(o, "muet"), &format!("{chemin}.muet"))?,
        solo: booleen(champ(o, "solo"), &format!("{chemin}.solo"))?,
        piste_mixage: entier(champ(o, "pisteMixage"), &format!("{chemin}.pisteMixage"), 0.0, INFINI)? as usize,
        couleur: texte(champ(o, "couleur"), &format!("{chemin}.couleur"))?,
        // absent des projets enregistrés avant le séquenceur pas à pas
        note_defaut: match o.get("noteDefaut") {
            None => {
                if batterie {
                    36
                } else {
                    60
                }
            }
            Some(v) => entier(v, &format!("{chemin}.noteDefaut"), 0.0, 127.0)? as u8,
        },
        // absent des projets d'avant les synthés intégrés
        source: match (batterie, o.get("source").and_then(Value::as_str)) {
            (false, Some("soustractif")) => SourceCanal::Soustractif,
            (_, Some("echantillons")) => SourceCanal::Echantillons,
            _ => SourceCanal::Soundfont,
        },
        instrument: match o.get("instrument") {
            None | Some(Value::Null) => None,
            Some(v) => Some(texte(v, &format!("{chemin}.instrument"))?),
        },
        swing: match o.get("swing") {
            None => 1.0,
            Some(v) => nombre(v, &format!("{chemin}.swing"), 0.0, 1.0)? as f32,
        },
        groupe: match o.get("groupe") {
            None | Some(Value::Null) => None,
            Some(v) => Some(texte(v, &format!("{chemin}.groupe"))?),
        },
    })
}

fn lire_pattern(v: &Value, chemin: &str, canaux: &HashSet<String>) -> R<Pattern> {
    let o = objet(v, chemin)?;
    let mut notes = BTreeMap::new();
    for (canal, liste) in objet(champ(o, "notes"), &format!("{chemin}.notes"))? {
        if !canaux.contains(canal) {
            return erreur(format!("{chemin}.notes : canal inconnu « {canal} »"));
        }
        let liste = tableau(liste, &format!("{chemin}.notes.{canal}"))?;
        let lues = liste.iter().enumerate().map(|(i, n)| lire_note(n, &format!("{chemin}.notes.{canal}[{i}]"))).collect::<R<Vec<_>>>()?;
        notes.insert(canal.clone(), lues);
    }
    Ok(Pattern {
        id: texte(champ(o, "id"), &format!("{chemin}.id"))?,
        nom: texte(champ(o, "nom"), &format!("{chemin}.nom"))?,
        longueur: entier(champ(o, "longueur"), &format!("{chemin}.longueur"), 1.0, u32::MAX as f64)? as u32,
        notes,
    })
}

fn lire_piste(v: &Value, chemin: &str) -> R<PisteMixage> {
    let o = objet(v, chemin)?;
    let index = entier(champ(o, "index"), &format!("{chemin}.index"), 0.0, INFINI)? as usize;
    // égaliseur, solo et réverbération : absents des projets enregistrés avant le mixer
    let eq = match o.get("eq") {
        None => Egaliseur::default(),
        Some(v) => {
            let e = objet(v, &format!("{chemin}.eq"))?;
            let bande = |nom: &str| nombre(champ(e, nom), &format!("{chemin}.eq.{nom}"), -24.0, 24.0).map(|x| x as f32);
            Egaliseur { grave: bande("grave")?, medium: bande("medium")?, aigu: bande("aigu")? }
        }
    };
    Ok(PisteMixage {
        index,
        nom: texte(champ(o, "nom"), &format!("{chemin}.nom"))?,
        volume: nombre(champ(o, "volume"), &format!("{chemin}.volume"), 0.0, 1.0)? as f32,
        pan: nombre(champ(o, "pan"), &format!("{chemin}.pan"), -1.0, 1.0)? as f32,
        muet: booleen(champ(o, "muet"), &format!("{chemin}.muet"))?,
        solo: match o.get("solo") {
            None => false,
            Some(v) => booleen(v, &format!("{chemin}.solo"))?,
        },
        eq,
        reverb: match o.get("reverb") {
            None => {
                if index == 0 {
                    0.0
                } else {
                    REVERB_DEFAUT
                }
            }
            Some(v) => nombre(v, &format!("{chemin}.reverb"), 0.0, 1.0)? as f32,
        },
    })
}

pub fn deserialiser(json: &str) -> R<Projet> {
    let brut: Value = serde_json::from_str(json).map_err(|_| ErreurProjet("Le fichier n'est pas un projet valide (JSON illisible)".into()))?;
    let o = objet(&brut, "projet")?;
    if champ(o, "version").as_u64() != Some(VERSION_PROJET as u64) {
        return erreur(format!("Version de projet non prise en charge : {} (attendue : {VERSION_PROJET})", champ(o, "version")));
    }
    let signature = objet(champ(o, "signature"), "projet.signature")?;
    let canaux = tableau(champ(o, "canaux"), "projet.canaux")?
        .iter()
        .enumerate()
        .map(|(i, c)| lire_canal(c, &format!("projet.canaux[{i}]")))
        .collect::<R<Vec<_>>>()?;
    let ids: HashSet<String> = canaux.iter().map(|c| c.id.clone()).collect();
    if ids.len() != canaux.len() {
        return erreur("projet.canaux : identifiants en double".into());
    }
    let patterns = tableau(champ(o, "patterns"), "projet.patterns")?
        .iter()
        .enumerate()
        .map(|(i, p)| lire_pattern(p, &format!("projet.patterns[{i}]"), &ids))
        .collect::<R<Vec<_>>>()?;
    if patterns.is_empty() {
        return erreur("projet.patterns : au moins un pattern attendu".into());
    }
    let pattern_actif = texte(champ(o, "patternActif"), "projet.patternActif")?;
    if !patterns.iter().any(|p| p.id == pattern_actif) {
        return erreur("projet.patternActif : pattern inconnu".into());
    }
    let ids_patterns: HashSet<&str> = patterns.iter().map(|p| p.id.as_str()).collect();
    let mut playlist = Vec::new();
    for (i, c) in tableau(champ(o, "playlist"), "projet.playlist")?.iter().enumerate() {
        let chemin = format!("projet.playlist[{i}]");
        let clip = objet(c, &chemin)?;
        let pattern = texte(champ(clip, "pattern"), &format!("{chemin}.pattern"))?;
        if !ids_patterns.contains(pattern.as_str()) {
            return erreur(format!("{chemin}.pattern : pattern inconnu"));
        }
        playlist.push(ClipPlaylist {
            id: texte(champ(clip, "id"), &format!("{chemin}.id"))?,
            pattern,
            piste: entier(champ(clip, "piste"), &format!("{chemin}.piste"), 0.0, u32::MAX as f64)? as u32,
            debut: entier(champ(clip, "debut"), &format!("{chemin}.debut"), 0.0, u32::MAX as f64)? as u32,
        });
    }
    let mixage = tableau(champ(o, "mixage"), "projet.mixage")?
        .iter()
        .enumerate()
        .map(|(i, m)| lire_piste(m, &format!("projet.mixage[{i}]")))
        .collect::<R<Vec<_>>>()?;
    Ok(Projet {
        version: VERSION_PROJET,
        nom: texte(champ(o, "nom"), "projet.nom")?,
        tempo: nombre(champ(o, "tempo"), "projet.tempo", 10.0, 999.0)?,
        swing: match o.get("swing") {
            None => 0.0,
            Some(v) => nombre(v, "projet.swing", 0.0, 1.0)?,
        },
        signature: Signature {
            numerateur: entier(champ(signature, "numerateur"), "projet.signature.numerateur", 1.0, 32.0)? as u32,
            denominateur: entier(champ(signature, "denominateur"), "projet.signature.denominateur", 1.0, 32.0)? as u32,
        },
        canaux,
        patterns,
        pattern_actif,
        playlist,
        mixage,
        infos: match o.get("infos") {
            None => InfosProjet::default(),
            Some(v) => {
                let i = objet(v, "projet.infos")?;
                let lire = |nom: &str| match i.get(nom) {
                    None => Ok(String::new()),
                    Some(t) => texte(t, &format!("projet.infos.{nom}")),
                };
                InfosProjet { auteur: lire("auteur")?, genre: lire("genre")?, notes: lire("notes")? }
            }
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::creer_demo;

    #[test]
    fn aller_retour_identique() {
        let demo = creer_demo();
        let relu = deserialiser(&serialiser(&demo)).unwrap();
        assert_eq!(relu, demo);
    }

    #[test]
    fn canal_echantillonneur_garde_son_instrument() {
        let mut p = creer_demo();
        p.ajouter_canal(crate::modele::OptionsCanal {
            source: SourceCanal::Echantillons,
            instrument: Some("Banques/Piano/piano.sfz".into()),
            nom: Some("Piano échantillonné".into()),
            ..Default::default()
        });
        let json = serialiser(&p);
        assert!(json.contains("\"source\":\"echantillons\""));
        assert_eq!(deserialiser(&json).unwrap(), p);
        // les autres canaux n'écrivent pas de champ « instrument »
        assert_eq!(json.matches("\"instrument\"").count(), 1);
    }

    #[test]
    fn fiche_du_projet_gardee_et_absente_si_vide() {
        let mut p = creer_demo();
        assert!(!serialiser(&p).contains("infos"), "fiche vide : rien d'écrit (fichier identique à la version web)");
        p.infos.auteur = "Alex".into();
        p.infos.notes = "Refrain à retravailler
mesure 9".into();
        let json = serialiser(&p);
        assert!(json.contains("\"infos\""));
        assert_eq!(deserialiser(&json).unwrap(), p);
        let abime = json.replace("\"auteur\":\"Alex\"", "\"auteur\":3");
        assert!(deserialiser(&abime).unwrap_err().0.contains("projet.infos.auteur"));
    }

    /// Un projet tel que la version web l'enregistre (champs ajoutés au fil des étapes absents).
    const ANCIEN_PROJET_WEB: &str = r##"{"version":1,"nom":"Ancien","tempo":97,"signature":{"numerateur":4,"denominateur":4},
        "canaux":[{"id":"canal-a","nom":"Piano","programme":0,"batterie":false,"volume":0.8,"pan":0,"muet":false,"solo":false,"pisteMixage":1,"couleur":"#f5a524"},
                  {"id":"canal-b","nom":"Batterie","programme":0,"batterie":true,"volume":0.8,"pan":0,"muet":false,"solo":false,"pisteMixage":2,"couleur":"#4cc38a"}],
        "patterns":[{"id":"pattern-1","nom":"Pattern 1","longueur":384,"notes":{"canal-a":[{"tic":0,"duree":96,"hauteur":60,"velocite":100}]}}],
        "patternActif":"pattern-1","playlist":[{"id":"clip-1","pattern":"pattern-1","piste":0,"debut":0}],
        "mixage":[{"index":0,"nom":"Maître","volume":0.8,"pan":0,"muet":false},{"index":1,"nom":"Piano","volume":0.8,"pan":0,"muet":false},{"index":2,"nom":"Batterie","volume":0.8,"pan":0,"muet":false}]}"##;

    #[test]
    fn projet_de_la_version_web_relu_avec_les_valeurs_par_defaut() {
        let p = deserialiser(ANCIEN_PROJET_WEB).unwrap();
        assert_eq!(p.tempo, 97.0);
        assert_eq!(p.swing, 0.0);
        assert_eq!((p.canaux[0].note_defaut, p.canaux[1].note_defaut), (60, 36));
        assert_eq!(p.canaux[0].source, SourceCanal::Soundfont);
        assert_eq!((p.mixage[0].reverb, p.mixage[1].reverb), (0.0, REVERB_DEFAUT));
        assert!(!p.mixage[1].solo);
        // et réécrit dans la forme complète, que la version web relit
        let json = serialiser(&p);
        assert!(json.contains("\"noteDefaut\":60") && json.contains("\"pisteMixage\":1") && json.contains("\"source\":\"soundfont\""));
    }

    #[test]
    fn fichiers_refuses_avec_un_message_clair() {
        assert_eq!(deserialiser("pas du json").unwrap_err().0, "Le fichier n'est pas un projet valide (JSON illisible)");
        assert!(deserialiser(&ANCIEN_PROJET_WEB.replace("\"version\":1", "\"version\":2")).unwrap_err().0.contains("Version"));
        let mauvais = ANCIEN_PROJET_WEB.replace("\"hauteur\":60", "\"hauteur\":200");
        assert_eq!(deserialiser(&mauvais).unwrap_err().0, "projet.patterns[0].notes.canal-a[0].hauteur : nombre entre 0 et 127 attendu");
        let inconnu = ANCIEN_PROJET_WEB.replace("\"patternActif\":\"pattern-1\"", "\"patternActif\":\"autre\"");
        assert!(deserialiser(&inconnu).unwrap_err().0.contains("pattern inconnu"));
    }
}
