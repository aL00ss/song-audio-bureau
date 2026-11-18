//! Recherche de fichiers MIDI en ligne : l'API publique de BitMidi (plus de 100 000 fichiers).
//! Fonctions pures (mise en forme, lecture des réponses) ; le téléchargement est fait par
//! l'application.

use serde_json::Value;

pub const API_BITMIDI: &str = "https://bitmidi.com/api/midi/search";
pub const ORIGINE_BITMIDI: &str = "https://bitmidi.com";

#[derive(Clone, Debug, PartialEq)]
pub struct ResultatRecherche {
    pub id: String,
    pub titre: String,
    pub nom_fichier: String,
    pub url: String,
    pub ecoutes: u64,
}

/// « chopin-ballade_1.mid » -> « chopin ballade 1 ».
pub fn nom_lisible(nom: &str) -> String {
    let sans_extension = {
        let bas = nom.to_lowercase();
        if bas.ends_with(".midi") {
            &nom[..nom.len() - 5]
        } else if bas.ends_with(".mid") {
            &nom[..nom.len() - 4]
        } else {
            nom
        }
    };
    let espaces: String = sans_extension.chars().map(|c| if c == '-' || c == '_' { ' ' } else { c }).collect();
    let propre = espaces.split_whitespace().collect::<Vec<_>>().join(" ");
    if propre.is_empty() { "Sans titre".into() } else { propre }
}

/// Encodage d'un composant d'adresse, comme `encodeURIComponent`.
pub fn encoder_composant(texte: &str) -> String {
    let mut sortie = String::new();
    for octet in texte.bytes() {
        let garde = octet.is_ascii_alphanumeric() || matches!(octet, b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')');
        if garde {
            sortie.push(octet as char);
        } else {
            sortie.push_str(&format!("%{octet:02X}"));
        }
    }
    sortie
}

pub fn adresse_recherche(requete: &str, page: u32) -> String {
    format!("{API_BITMIDI}?q={}&page={page}", encoder_composant(requete.trim()))
}

/// Adresse absolue d'un lien relatif à BitMidi.
fn absolue(lien: &str) -> String {
    if lien.starts_with("http://") || lien.starts_with("https://") {
        lien.to_string()
    } else if lien.starts_with('/') {
        format!("{ORIGINE_BITMIDI}{lien}")
    } else {
        format!("{ORIGINE_BITMIDI}/{lien}")
    }
}

/// Résultats jouables d'une réponse de l'API (ceux sans fichier sont ignorés).
pub fn lire_resultats(reponse: &Value) -> Vec<ResultatRecherche> {
    let Some(resultats) = reponse.get("result").and_then(|r| r.get("results")).and_then(Value::as_array) else { return Vec::new() };
    resultats
        .iter()
        .filter_map(|r| {
            let lien = r.get("downloadUrl")?.as_str().filter(|l| !l.is_empty())?;
            let nom = match r.get("name") {
                Some(Value::String(s)) => s.clone(),
                Some(autre) => autre.to_string(),
                None => String::new(),
            };
            let id = match r.get("id") {
                Some(Value::String(s)) => s.clone(),
                Some(autre) => autre.to_string(),
                None => String::new(),
            };
            Some(ResultatRecherche {
                id: format!("bitmidi-{id}"),
                titre: nom_lisible(&nom),
                nom_fichier: nom,
                url: absolue(lien),
                ecoutes: r.get("plays").and_then(Value::as_u64).unwrap_or(0),
            })
        })
        .collect()
}

/// « 1 234 écoutes » (espace insécable fine entre les milliers, comme en français).
pub fn formater_ecoutes(n: u64) -> String {
    let chiffres = n.to_string();
    let mut groupes = Vec::new();
    let octets = chiffres.as_bytes();
    let mut fin = octets.len();
    while fin > 0 {
        let debut = fin.saturating_sub(3);
        groupes.push(&chiffres[debut..fin]);
        fin = debut;
    }
    groupes.reverse();
    format!("{} écoute{}", groupes.join("\u{202f}"), if n > 1 { "s" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_et_adresses() {
        assert_eq!(nom_lisible("chopin-ballade_1.mid"), "chopin ballade 1");
        assert_eq!(nom_lisible("Fur Elise.MIDI"), "Fur Elise");
        assert_eq!(nom_lisible(".mid"), "Sans titre");
        assert_eq!(adresse_recherche("  lettre à élise ", 0), "https://bitmidi.com/api/midi/search?q=lettre%20%C3%A0%20%C3%A9lise&page=0");
        assert_eq!(encoder_composant("a&b=c?"), "a%26b%3Dc%3F");
    }

    #[test]
    fn lecture_d_une_reponse() {
        let reponse: Value = serde_json::from_str(
            r#"{"result":{"results":[
                {"id":12,"name":"chopin-nocturne.mid","downloadUrl":"/uploads/12.mid","plays":1500},
                {"id":"13","name":"sans-fichier.mid","downloadUrl":""},
                {"id":14,"name":"complet.mid","downloadUrl":"https://cdn.example/14.mid"}
            ]}}"#,
        )
        .unwrap();
        let r = lire_resultats(&reponse);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0], ResultatRecherche { id: "bitmidi-12".into(), titre: "chopin nocturne".into(), nom_fichier: "chopin-nocturne.mid".into(), url: "https://bitmidi.com/uploads/12.mid".into(), ecoutes: 1500 });
        assert_eq!(r[1].url, "https://cdn.example/14.mid");
        assert_eq!(r[1].ecoutes, 0);
        assert!(lire_resultats(&serde_json::json!({"autre": 1})).is_empty());
    }

    #[test]
    fn ecoutes_en_francais() {
        assert_eq!(formater_ecoutes(1), "1 écoute");
        assert_eq!(formater_ecoutes(0), "0 écoute");
        assert_eq!(formater_ecoutes(1234567), "1\u{202f}234\u{202f}567 écoutes");
    }
}
