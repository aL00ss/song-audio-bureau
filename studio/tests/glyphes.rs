//! Garde-fou : chaque caractère des textes affichés (chaînes du code de l'interface et du
//! modèle, hors tests) doit exister dans la police de l'application. Sinon, il s'afficherait
//! comme un carré vide.

use std::path::Path;

use eframe::egui;

/// Contenu des chaînes de caractères d'un fichier Rust, avant ses tests. Analyse simple (chaînes
/// ordinaires et brutes, commentaires ignorés) : suffisante pour notre code.
fn chaines(source: &str) -> Vec<String> {
    let source = source.split("#[cfg(test)]").next().unwrap_or(source);
    let octets: Vec<char> = source.chars().collect();
    let mut chaines = Vec::new();
    let mut i = 0;
    while i < octets.len() {
        let c = octets[i];
        // commentaires
        if c == '/' && octets.get(i + 1) == Some(&'/') {
            while i < octets.len() && octets[i] != '\n' {
                i += 1;
            }
            continue;
        }
        // caractère isolé ('a', '"') : sauté
        if c == '\'' && octets.get(i + 2) == Some(&'\'') {
            i += 3;
            continue;
        }
        if c == '\'' && octets.get(i + 1) == Some(&'\\') {
            while i < octets.len() && !(octets[i] == '\'' && i > 0 && octets[i - 1] != '\\' && octets[i - 2] == '\\') {
                i += 1;
                if i > 0 && octets[i] == '\'' {
                    break;
                }
            }
            i += 1;
            continue;
        }
        // chaîne brute r"…" ou r#"…"#
        if c == 'r' && (octets.get(i + 1) == Some(&'"') || octets.get(i + 1) == Some(&'#')) && (i == 0 || !octets[i - 1].is_alphanumeric()) {
            let mut j = i + 1;
            let mut dieses = 0;
            while octets.get(j) == Some(&'#') {
                dieses += 1;
                j += 1;
            }
            if octets.get(j) == Some(&'"') {
                let debut = j + 1;
                let mut k = debut;
                'cherche: while k < octets.len() {
                    if octets[k] == '"' && (0..dieses).all(|d| octets.get(k + 1 + d) == Some(&'#')) {
                        break 'cherche;
                    }
                    k += 1;
                }
                chaines.push(octets[debut..k.min(octets.len())].iter().collect());
                i = k + 1 + dieses;
                continue;
            }
        }
        // chaîne ordinaire
        if c == '"' {
            let mut texte = String::new();
            let mut k = i + 1;
            while k < octets.len() && octets[k] != '"' {
                if octets[k] == '\\' {
                    k += 2;
                    continue;
                }
                texte.push(octets[k]);
                k += 1;
            }
            chaines.push(texte);
            i = k + 1;
            continue;
        }
        i += 1;
    }
    chaines
}

#[test]
fn la_police_dessine_tous_les_caracteres_affiches() {
    let ctx = egui::Context::default();
    let mut sortie = ctx.run_ui(Default::default(), |_| {});
    sortie.textures_delta.clear();
    let racine = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut manquants = Vec::new();
    let mut verifies = 0;
    for dossier in [racine.join("src"), racine.join("../projet/src")] {
        for entree in std::fs::read_dir(&dossier).unwrap().flatten() {
            let chemin = entree.path();
            if chemin.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&chemin).unwrap();
            for texte in chaines(&source) {
                for c in texte.chars().filter(|c| !c.is_ascii()) {
                    verifies += 1;
                    // police des textes (la police à chasse fixe ne sert qu'aux afficheurs en chiffres)
                    if !ctx.fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(14.0), c)) {
                        manquants.push(format!("{c:?} (U+{:04X}) dans « {texte} » ({})", c as u32, chemin.file_name().unwrap().to_string_lossy()));
                    }
                }
            }
        }
    }
    manquants.dedup();
    assert!(verifies > 100, "{verifies} caractères vérifiés seulement : l'analyse des chaînes est cassée");
    assert!(manquants.is_empty(), "caractères absents de la police :\n{}", manquants.join("\n"));
}

#[test]
fn l_analyse_des_chaines_trouve_ce_qu_il_faut() {
    let source = "let a = \"Lecture\"; // « commentaire ▸ »\nlet b = r#\"brut ♯\"#; let c = 'x'; let d = \"é\\\"t\";\n#[cfg(test)]\nlet e = \"ignoré ✓\";";
    assert_eq!(chaines(source), vec!["Lecture", "brut ♯", "ét"]);
}
