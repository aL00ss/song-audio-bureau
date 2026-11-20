//! Banques SoundFont : lecture brute (RIFF), décompression des SoundFont 3 (échantillons Ogg
//! Vorbis) et fusion de banques plus fidèles dans la banque General MIDI. Le résultat est une
//! seule banque SF2, que le moteur joue comme avant : les programmes remplacés (le piano par
//! Salamander, les cordes par VSCO…) prennent le son de la banque ajoutée, tout le reste garde
//! celui de GeneralUser GS.

use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use rustysynth::SoundFont;

/// Banque où sont rangés les presets remplacés (aucun instrument ne la demande).
const BANQUE_CACHEE: u16 = 999;
/// Trames de silence après chaque échantillon (exigées par la norme SoundFont 2).
const MARGE: usize = 46;
const VORBIS: u16 = 0x10;
const GEN_INSTRUMENT: u16 = 41;
const GEN_ECHANTILLON: u16 = 53;

/// Une banque SoundFont décomposée : les échantillons 16 bits et les enregistrements bruts des
/// neuf tableaux du « pdta » (chacun avec son enregistrement terminal).
#[derive(Clone, Debug, Default)]
pub struct BanqueBrute {
    info: Vec<u8>,
    /// Échantillons 16 bits (SF2) ; en SF3, flux Ogg à décoder (voir `donnees_sf3`).
    echantillons: Vec<i16>,
    donnees_sf3: Vec<u8>,
    phdr: Vec<Vec<u8>>,
    pbag: Vec<Vec<u8>>,
    pmod: Vec<Vec<u8>>,
    pgen: Vec<Vec<u8>>,
    inst: Vec<Vec<u8>>,
    ibag: Vec<Vec<u8>>,
    imod: Vec<Vec<u8>>,
    igen: Vec<Vec<u8>>,
    shdr: Vec<Vec<u8>>,
}

fn u16_a(o: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([o[i], o[i + 1]])
}
fn u32_a(o: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([o[i], o[i + 1], o[i + 2], o[i + 3]])
}
fn ecrire_u16(o: &mut [u8], i: usize, v: u16) {
    o[i..i + 2].copy_from_slice(&v.to_le_bytes());
}
fn ecrire_u32(o: &mut [u8], i: usize, v: u32) {
    o[i..i + 4].copy_from_slice(&v.to_le_bytes());
}

/// Blocs (identifiant, contenu) d'une suite de blocs RIFF.
fn blocs(o: &[u8]) -> Result<Vec<([u8; 4], &[u8])>, String> {
    let mut r = Vec::new();
    let mut i = 0;
    while i + 8 <= o.len() {
        let id = [o[i], o[i + 1], o[i + 2], o[i + 3]];
        let n = u32_a(o, i + 4) as usize;
        let contenu = o.get(i + 8..i + 8 + n).ok_or("bloc RIFF tronqué")?;
        r.push((id, contenu));
        i += 8 + n + n % 2;
    }
    Ok(r)
}

fn enregistrements(o: &[u8], taille: usize, nom: &str) -> Result<Vec<Vec<u8>>, String> {
    if o.len() % taille != 0 || o.len() < taille {
        return Err(format!("tableau {nom} de taille invalide"));
    }
    Ok(o.chunks(taille).map(<[u8]>::to_vec).collect())
}

impl BanqueBrute {
    /// Lit une banque SF2 ou SF3.
    pub fn lire(octets: &[u8]) -> Result<BanqueBrute, String> {
        if octets.len() < 12 || &octets[0..4] != b"RIFF" || &octets[8..12] != b"sfbk" {
            return Err("ce n'est pas une banque SoundFont".into());
        }
        let mut b = BanqueBrute::default();
        let mut brut_smpl: &[u8] = &[];
        for (id, contenu) in blocs(&octets[12..])? {
            if &id != b"LIST" || contenu.len() < 4 {
                continue;
            }
            match &contenu[0..4] {
                b"INFO" => b.info = contenu[4..].to_vec(),
                b"sdta" => {
                    for (sid, s) in blocs(&contenu[4..])? {
                        if &sid == b"smpl" {
                            brut_smpl = s;
                        }
                    }
                }
                b"pdta" => {
                    for (pid, p) in blocs(&contenu[4..])? {
                        match &pid {
                            b"phdr" => b.phdr = enregistrements(p, 38, "phdr")?,
                            b"pbag" => b.pbag = enregistrements(p, 4, "pbag")?,
                            b"pmod" => b.pmod = enregistrements(p, 10, "pmod")?,
                            b"pgen" => b.pgen = enregistrements(p, 4, "pgen")?,
                            b"inst" => b.inst = enregistrements(p, 22, "inst")?,
                            b"ibag" => b.ibag = enregistrements(p, 4, "ibag")?,
                            b"imod" => b.imod = enregistrements(p, 10, "imod")?,
                            b"igen" => b.igen = enregistrements(p, 4, "igen")?,
                            b"shdr" => b.shdr = enregistrements(p, 46, "shdr")?,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        if b.phdr.is_empty() || b.shdr.is_empty() || b.inst.is_empty() {
            return Err("banque SoundFont incomplète".into());
        }
        if b.shdr.iter().any(|s| u16_a(s, 44) & VORBIS != 0) {
            b.donnees_sf3 = brut_smpl.to_vec();
        } else {
            b.echantillons = brut_smpl.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        }
        Ok(b)
    }

    /// Décode les échantillons Ogg Vorbis d'une banque SF3 : la banque devient une SF2 ordinaire
    /// (positions en trames, boucles absolues, marge de silence après chaque échantillon).
    pub fn decompresser(&mut self) -> Result<(), String> {
        if self.donnees_sf3.is_empty() {
            return Ok(());
        }
        // chaque échantillon est un flux indépendant : décodés en parallèle, sur tous les cœurs
        let dernier = self.shdr.len() - 1;
        let positions: Vec<(usize, usize)> = self.shdr[..dernier].iter().map(|s| (u32_a(s, 20) as usize, u32_a(s, 24) as usize)).collect();
        let fils = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(positions.len().max(1));
        let donnees = &self.donnees_sf3;
        // chaque fil prend l'échantillon suivant dès qu'il est libre (les longs ne bloquent pas)
        let suivant = std::sync::atomic::AtomicUsize::new(0);
        let positions = &positions;
        let mut decodes: Vec<(usize, Result<Vec<i16>, String>)> = std::thread::scope(|portee| {
            let travaux: Vec<_> = (0..fils)
                .map(|_| {
                    portee.spawn(|| {
                        let mut faits = Vec::new();
                        loop {
                            let i = suivant.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(&(debut, fin)) = positions.get(i) else { break };
                            let r = donnees.get(debut..fin).ok_or_else(|| "échantillon Vorbis hors des données".to_string()).and_then(decoder_vorbis);
                            faits.push((i, r));
                        }
                        faits
                    })
                })
                .collect();
            travaux.into_iter().flat_map(|t| t.join().expect("décodage Vorbis")).collect()
        });
        decodes.sort_by_key(|(i, _)| *i);
        let decodes = decodes.into_iter().map(|(_, r)| r);
        let mut echantillons = Vec::new();
        for (s, pcm) in self.shdr[..dernier].iter_mut().zip(decodes) {
            let pcm = pcm?;
            let depart = echantillons.len() as u32;
            echantillons.extend_from_slice(&pcm);
            echantillons.extend(std::iter::repeat_n(0i16, MARGE));
            let (boucle_debut, boucle_fin) = (u32_a(s, 28), u32_a(s, 32));
            ecrire_u32(s, 20, depart);
            ecrire_u32(s, 24, depart + pcm.len() as u32);
            ecrire_u32(s, 28, depart + boucle_debut);
            ecrire_u32(s, 32, depart + boucle_fin);
            let genre = u16_a(s, 44) & !VORBIS;
            ecrire_u16(s, 44, genre);
        }
        self.echantillons = echantillons;
        self.donnees_sf3.clear();
        Ok(())
    }

    /// (banque, programme) de chaque preset (sans le terminal).
    pub fn presets(&self) -> Vec<(u16, u16)> {
        self.phdr[..self.phdr.len() - 1].iter().map(|p| (u16_a(p, 22), u16_a(p, 20))).collect()
    }

    /// Range les presets (banque 0) de ces programmes dans une banque que personne ne demande.
    fn cacher(&mut self, programmes: &[u8], garder: bool) {
        let n = self.phdr.len() - 1;
        for p in self.phdr[..n].iter_mut() {
            let (programme, banque) = (u16_a(p, 20), u16_a(p, 22));
            let vise = banque == 0 && programmes.iter().any(|&q| q as u16 == programme);
            if vise != garder {
                ecrire_u16(p, 22, BANQUE_CACHEE);
            }
        }
    }

    /// Ajoute les presets de `autre` qui remplacent ces programmes (banque 0) ; ceux de cette
    /// banque pour les mêmes programmes sont cachés. Les deux banques doivent être décompressées.
    pub fn fusionner(&mut self, mut autre: BanqueBrute, programmes: &[u8]) {
        self.cacher(programmes, false);
        autre.cacher(programmes, true);
        let sans_terminal = |v: &Vec<Vec<u8>>| v.len() - 1;
        let (d_pbag, d_pmod, d_pgen) = (sans_terminal(&self.pbag), sans_terminal(&self.pmod), sans_terminal(&self.pgen));
        let (d_inst, d_ibag, d_imod, d_igen) = (sans_terminal(&self.inst), sans_terminal(&self.ibag), sans_terminal(&self.imod), sans_terminal(&self.igen));
        let d_shdr = sans_terminal(&self.shdr);
        let d_smpl = self.echantillons.len() as u32;

        // enlève les terminaux, ajoute les enregistrements décalés, remet les terminaux
        let fin = |v: &mut Vec<Vec<u8>>| v.pop().expect("terminal");
        let (t_phdr, t_pbag, t_pmod, t_pgen) = (fin(&mut self.phdr), fin(&mut self.pbag), fin(&mut self.pmod), fin(&mut self.pgen));
        let (t_inst, t_ibag, t_imod, t_igen, t_shdr) = (fin(&mut self.inst), fin(&mut self.ibag), fin(&mut self.imod), fin(&mut self.igen), fin(&mut self.shdr));
        let tous = |v: &Vec<Vec<u8>>| v[..v.len() - 1].to_vec();
        for mut p in tous(&autre.phdr) {
            let i = u16_a(&p, 24) as usize + d_pbag;
            ecrire_u16(&mut p, 24, i as u16);
            self.phdr.push(p);
        }
        for mut b in tous(&autre.pbag) {
            let (g, m) = (u16_a(&b, 0) as usize + d_pgen, u16_a(&b, 2) as usize + d_pmod);
            ecrire_u16(&mut b, 0, g as u16);
            ecrire_u16(&mut b, 2, m as u16);
            self.pbag.push(b);
        }
        self.pmod.extend(tous(&autre.pmod));
        for mut g in tous(&autre.pgen) {
            if u16_a(&g, 0) == GEN_INSTRUMENT {
                let i = u16_a(&g, 2) as usize + d_inst;
                ecrire_u16(&mut g, 2, i as u16);
            }
            self.pgen.push(g);
        }
        for mut i in tous(&autre.inst) {
            let b = u16_a(&i, 20) as usize + d_ibag;
            ecrire_u16(&mut i, 20, b as u16);
            self.inst.push(i);
        }
        for mut b in tous(&autre.ibag) {
            let (g, m) = (u16_a(&b, 0) as usize + d_igen, u16_a(&b, 2) as usize + d_imod);
            ecrire_u16(&mut b, 0, g as u16);
            ecrire_u16(&mut b, 2, m as u16);
            self.ibag.push(b);
        }
        self.imod.extend(tous(&autre.imod));
        for mut g in tous(&autre.igen) {
            if u16_a(&g, 0) == GEN_ECHANTILLON {
                let s = u16_a(&g, 2) as usize + d_shdr;
                ecrire_u16(&mut g, 2, s as u16);
            }
            self.igen.push(g);
        }
        for mut s in tous(&autre.shdr) {
            for champ in [20, 24, 28, 32] {
                let v = u32_a(&s, champ) + d_smpl;
                ecrire_u32(&mut s, champ, v);
            }
            // lien stéréo : index d'un autre échantillon de la même banque
            if u16_a(&s, 44) & 0x6 != 0 {
                let lien = u16_a(&s, 42) as usize + d_shdr;
                ecrire_u16(&mut s, 42, lien as u16);
            }
            self.shdr.push(s);
        }
        self.echantillons.extend_from_slice(&autre.echantillons);

        // terminaux, avec les nouveaux totaux
        let mut t_phdr = t_phdr;
        ecrire_u16(&mut t_phdr, 24, self.pbag.len() as u16);
        self.phdr.push(t_phdr);
        let mut t_pbag = t_pbag;
        ecrire_u16(&mut t_pbag, 0, self.pgen.len() as u16);
        ecrire_u16(&mut t_pbag, 2, self.pmod.len() as u16);
        self.pbag.push(t_pbag);
        self.pmod.push(t_pmod);
        self.pgen.push(t_pgen);
        let mut t_inst = t_inst;
        ecrire_u16(&mut t_inst, 20, self.ibag.len() as u16);
        self.inst.push(t_inst);
        let mut t_ibag = t_ibag;
        ecrire_u16(&mut t_ibag, 0, self.igen.len() as u16);
        ecrire_u16(&mut t_ibag, 2, self.imod.len() as u16);
        self.ibag.push(t_ibag);
        self.imod.push(t_imod);
        self.igen.push(t_igen);
        self.shdr.push(t_shdr);
    }

    /// La banque en SF2 (octets d'un fichier .sf2), écrite d'un seul jet (les échantillons, des
    /// centaines de Mo, ne sont copiés qu'une fois).
    pub fn ecrire(&self) -> Vec<u8> {
        // les échantillons d'un bloc (16 bits petit-boutiste, l'ordre du fichier)
        let smpl: std::borrow::Cow<[u8]> = if cfg!(target_endian = "little") {
            std::borrow::Cow::Borrowed(bytemuck::cast_slice(&self.echantillons))
        } else {
            std::borrow::Cow::Owned(self.echantillons.iter().flat_map(|e| e.to_le_bytes()).collect())
        };
        let pdta: [(&[u8; 4], &Vec<Vec<u8>>); 9] = [
            (b"phdr", &self.phdr),
            (b"pbag", &self.pbag),
            (b"pmod", &self.pmod),
            (b"pgen", &self.pgen),
            (b"inst", &self.inst),
            (b"ibag", &self.ibag),
            (b"imod", &self.imod),
            (b"igen", &self.igen),
            (b"shdr", &self.shdr),
        ];
        let bloc = |n: usize| 8 + n + n % 2;
        let taille_info = 4 + self.info.len();
        let taille_sdta = 4 + bloc(smpl.len());
        let taille_pdta = 4 + pdta.iter().map(|(_, v)| bloc(v.iter().map(Vec::len).sum())).sum::<usize>();
        let taille = 4 + bloc(taille_info) + bloc(taille_sdta) + bloc(taille_pdta);
        let mut o = Vec::with_capacity(8 + taille);
        let entete = |o: &mut Vec<u8>, id: &[u8; 4], n: usize| {
            o.extend_from_slice(id);
            o.extend_from_slice(&(n as u32).to_le_bytes());
        };
        let bourrage = |o: &mut Vec<u8>, n: usize| {
            if n % 2 == 1 {
                o.push(0);
            }
        };
        entete(&mut o, b"RIFF", taille);
        o.extend_from_slice(b"sfbk");
        entete(&mut o, b"LIST", taille_info);
        o.extend_from_slice(b"INFO");
        o.extend_from_slice(&self.info);
        bourrage(&mut o, taille_info);
        entete(&mut o, b"LIST", taille_sdta);
        o.extend_from_slice(b"sdta");
        entete(&mut o, b"smpl", smpl.len());
        o.extend_from_slice(&smpl);
        bourrage(&mut o, smpl.len());
        entete(&mut o, b"LIST", taille_pdta);
        o.extend_from_slice(b"pdta");
        for (id, v) in pdta {
            let n: usize = v.iter().map(Vec::len).sum();
            entete(&mut o, id, n);
            for e in v {
                o.extend_from_slice(e);
            }
            bourrage(&mut o, n);
        }
        o
    }
}

/// Décode un flux Ogg Vorbis mono en échantillons 16 bits.
fn decoder_vorbis(flux: &[u8]) -> Result<Vec<i16>, String> {
    let mut lecteur = lewton::inside_ogg::OggStreamReader::new(Cursor::new(flux)).map_err(|e| format!("Vorbis : {e:?}"))?;
    let canaux = lecteur.ident_hdr.audio_channels.max(1) as usize;
    let mut pcm = Vec::new();
    while let Some(paquet) = lecteur.read_dec_packet_itl().map_err(|e| format!("Vorbis : {e:?}"))? {
        // une seule voie attendue ; sinon on garde la première
        pcm.extend(paquet.into_iter().step_by(canaux));
    }
    Ok(pcm)
}

/// Une banque plus fidèle qui remplace des programmes General MIDI.
#[derive(Clone, Debug)]
pub struct BanqueSupplementaire {
    pub fichier: &'static str,
    pub nom: &'static str,
    pub programmes: &'static [u8],
}

/// Les banques supplémentaires fournies (dans `ressources/sons`), comme la version web.
pub const BANQUES_SUPPLEMENTAIRES: [BanqueSupplementaire; 4] = [
    BanqueSupplementaire { fichier: "piano-salamander.sf3", nom: "Piano à queue (Salamander)", programmes: &[0] },
    BanqueSupplementaire { fichier: "orchestre-cordes.sf3", nom: "Cordes (VSCO 2)", programmes: &[40, 41, 42, 43, 44, 45, 46, 48, 49] },
    BanqueSupplementaire { fichier: "orchestre-cuivres.sf3", nom: "Cuivres (VSCO 2)", programmes: &[56, 57, 58, 59, 60] },
    BanqueSupplementaire { fichier: "orchestre-bois.sf3", nom: "Bois (VSCO 2)", programmes: &[68, 70, 71, 72, 73] },
];

/// La banque General MIDI, enrichie des banques supplémentaires trouvées dans `dossier_sons`
/// (une banque absente ou illisible est ignorée : GeneralUser GS joue ce programme).
pub fn charger_banque_enrichie(chemin_gm: &Path, dossier_sons: Option<&Path>) -> Result<(Arc<SoundFont>, Vec<&'static str>), String> {
    let octets = std::fs::read(chemin_gm).map_err(|e| format!("banque {} : {e}", chemin_gm.display()))?;
    let Some(dossier) = dossier_sons else {
        return SoundFont::new(&mut Cursor::new(octets)).map(|b| (Arc::new(b), Vec::new())).map_err(|e| format!("banque illisible : {e:?}"));
    };
    let mut banque = BanqueBrute::lire(&octets)?;
    banque.decompresser()?;
    // les banques se décodent en même temps ; la fusion suit, dans l'ordre du catalogue
    let decodees: Vec<Option<BanqueBrute>> = std::thread::scope(|portee| {
        let fils: Vec<_> = BANQUES_SUPPLEMENTAIRES
            .iter()
            .map(|s| {
                portee.spawn(move || {
                    let mut autre = BanqueBrute::lire(&std::fs::read(dossier.join(s.fichier)).ok()?).ok()?;
                    autre.decompresser().ok()?;
                    Some(autre)
                })
            })
            .collect();
        fils.into_iter().map(|f| f.join().ok().flatten()).collect()
    });
    let mut chargees = Vec::new();
    for (s, autre) in BANQUES_SUPPLEMENTAIRES.iter().zip(decodees) {
        if let Some(autre) = autre {
            banque.fusionner(autre, s.programmes);
            chargees.push(s.nom);
        }
    }
    let sf2 = banque.ecrire();
    drop(banque); // libère les échantillons avant que rustysynth ne lise sa copie
    let sf = SoundFont::new(&mut Cursor::new(sf2)).map_err(|e| format!("banque fusionnée illisible : {e:?}"))?;
    Ok((Arc::new(sf), chargees))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustysynth::{Synthesizer, SynthesizerSettings};

    fn ressources() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../ressources")
    }

    /// Joue une note d'un programme et renvoie l'énergie et le signal.
    fn jouer(banque: &Arc<SoundFont>, programme: i32, hauteur: i32) -> Vec<f32> {
        let mut s = Synthesizer::new(banque, &SynthesizerSettings::new(44100)).unwrap();
        s.process_midi_message(0, 0xC0, programme, 0);
        s.note_on(0, hauteur, 100);
        let (mut g, mut d) = (vec![0.0; 22050], vec![0.0; 22050]);
        s.render(&mut g, &mut d);
        g
    }

    fn efficace(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn la_banque_gm_reecrite_est_identique_au_son() {
        let chemin = ressources().join("banques/GeneralUser-GS.sf2");
        let octets = std::fs::read(&chemin).unwrap();
        let brute = BanqueBrute::lire(&octets).unwrap();
        let reecrite = Arc::new(SoundFont::new(&mut Cursor::new(brute.ecrire())).unwrap());
        let origine = Arc::new(SoundFont::new(&mut Cursor::new(octets)).unwrap());
        for programme in [0, 33, 40, 56] {
            assert_eq!(jouer(&reecrite, programme, 60), jouer(&origine, programme, 60), "programme {programme}");
        }
    }

    #[test]
    fn sf3_decompressee_et_fusionnee() {
        let sons = ressources().join("sons");
        let mut piano = BanqueBrute::lire(&std::fs::read(sons.join("piano-salamander.sf3")).unwrap()).unwrap();
        assert!(!piano.donnees_sf3.is_empty());
        piano.decompresser().unwrap();
        assert!(piano.echantillons.len() > 100_000, "des échantillons décodés");
        assert!(piano.presets().contains(&(0, 0)), "le piano est au programme 0");
        let (banque, chargees) = charger_banque_enrichie(&ressources().join("banques/GeneralUser-GS.sf2"), Some(&sons)).unwrap();
        assert_eq!(chargees.len(), 4, "{chargees:?}");
        let gm = crate::charger_banque(&ressources().join("banques/GeneralUser-GS.sf2")).unwrap();
        // le piano et les cordes changent de son ; la basse (33) reste celle de GeneralUser
        let (p_enrichi, p_gm) = (jouer(&banque, 0, 60), jouer(&gm, 0, 60));
        assert!(efficace(&p_enrichi) > 0.005, "le piano de Salamander sonne : {}", efficace(&p_enrichi));
        assert_ne!(p_enrichi, p_gm);
        assert!(efficace(&jouer(&banque, 40, 67)) > 0.002, "violon de VSCO");
        assert_ne!(jouer(&banque, 40, 67), jouer(&gm, 40, 67));
        assert_eq!(jouer(&banque, 33, 40), jouer(&gm, 33, 40), "la basse n'est pas touchée");
        // la batterie (banque 128) non plus
        let mut s1 = Synthesizer::new(&banque, &SynthesizerSettings::new(44100)).unwrap();
        let mut s2 = Synthesizer::new(&gm, &SynthesizerSettings::new(44100)).unwrap();
        for s in [&mut s1, &mut s2] {
            s.note_on(9, 36, 110);
        }
        let mut a = (vec![0.0; 8000], vec![0.0; 8000]);
        let mut b = (vec![0.0; 8000], vec![0.0; 8000]);
        s1.render(&mut a.0, &mut a.1);
        s2.render(&mut b.0, &mut b.1);
        assert_eq!(a.0, b.0);
    }

    #[test]
    fn fichiers_refuses() {
        assert!(BanqueBrute::lire(b"pas une banque").is_err());
        assert!(BanqueBrute::lire(b"RIFF\x04\x00\x00\x00sfbk").is_err());
        let dossier_vide = std::env::temp_dir();
        let (_, chargees) = charger_banque_enrichie(&ressources().join("banques/GeneralUser-GS.sf2"), Some(&dossier_vide)).unwrap();
        assert!(chargees.is_empty(), "aucune banque supplémentaire : la GM seule");
    }
}
