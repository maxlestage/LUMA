//! La photothèque : un catalogue local de toutes les photos ajoutées, quel que soit
//! leur dossier.
//!
//! Le catalogue est un index, pas une copie : pour chaque photo, ce que disent son
//! en‑tête (date de prise de vue, boîtier, objectif, ISO, ouverture, vitesse, focale)
//! et son XMP (note, drapeau, libellé, mots‑clés). Les photos et leurs XMP restent la
//! référence ; l'index se reconstruit à partir d'eux.
//!
//! Mettre un dossier à jour ne relit que ce qui a changé : un fichier dont la taille et
//! la date de modification sont celles de l'index, avec un XMP inchangé, n'est pas
//! rouvert. Les en‑têtes sont lus en parallèle, sans décoder les pixels.

pub mod backup;
pub mod collections;
pub mod fingerprint;
pub mod geotag;
pub mod import;
pub mod map;
pub mod query;
pub mod stacks;
mod store;
pub mod watch;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use luma_project::copies;
use luma_project::xmp::{self, Flag, Meta};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

pub use collections::{Collection, Content};
pub use query::Query;

/// La version de l'ancien index JSON ; un index d'une autre version est reconstruit.
const VERSION: u32 = 1;

/// Une photo du catalogue.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub path: PathBuf,
    /// Taille et date de modification (secondes) du fichier, pour savoir s'il a changé.
    pub size: u64,
    pub modified: u64,
    /// Date de modification du XMP, 0 sans XMP.
    pub sidecar: u64,
    /// La date de prise de vue, `AAAA-MM-JJTHH:MM:SS` : celle de l'EXIF, à défaut celle
    /// du fichier. L'ordre alphabétique est l'ordre chronologique.
    pub date: String,
    pub make: String,
    pub model: String,
    pub lens: String,
    pub iso: Option<u32>,
    pub aperture: Option<f32>,
    /// En secondes.
    pub exposure_time: Option<f32>,
    /// En millimètres.
    pub focal_length: Option<f32>,
    /// La position de la prise de vue : latitude et longitude, en degrés (nord et est
    /// positifs).
    pub gps: Option<[f64; 2]>,
    pub rating: u8,
    /// 1 : retenue, -1 : rejetée.
    pub flag: i8,
    pub label: String,
    pub keywords: Vec<String>,
}

impl Entry {
    /// Le boîtier, marque comprise (« Sony ILCE-7M3 »).
    pub fn camera(&self) -> String {
        camera_name(&self.make, &self.model)
    }

    pub fn flag(&self) -> Flag {
        match self.flag {
            1 => Flag::Pick,
            -1 => Flag::Reject,
            _ => Flag::None,
        }
    }

    fn set_meta(&mut self, meta: Meta) {
        self.rating = meta.rating;
        self.flag = match meta.flag {
            Flag::Pick => 1,
            Flag::Reject => -1,
            Flag::None => 0,
        };
        self.label = meta.label;
        self.keywords = meta.keywords;
        // Une position placée après coup (trace GPX, carte) passe devant celle du fichier.
        if let Some(p) = meta.position {
            self.gps = Some([p.latitude, p.longitude]);
        }
    }
}

/// Le bilan d'une mise à jour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Update {
    /// Photos nouvelles dans l'index.
    pub added: usize,
    /// Photos ou XMP modifiés depuis, relus.
    pub updated: usize,
    pub unchanged: usize,
    /// Photos disparues du disque, retirées.
    pub removed: usize,
}

/// Le catalogue : les photos, rangées par chemin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    entries: BTreeMap<PathBuf, Entry>,
    collections: Vec<Collection>,
}

/// L'ancien index, en JSON : encore lu, plus écrit.
#[derive(Deserialize)]
struct Stored {
    version: u32,
    entries: Vec<Entry>,
    /// Absentes des catalogues d'avant les collections.
    #[serde(default)]
    collections: Vec<Collection>,
}

/// Le fichier du catalogue, dans le dossier de LÜMA : sur Mac
/// `~/Library/Application Support/LÜMA/Catalogue.lumacat`, sur iPhone et iPad
/// `Library/Application Support/Catalogue.lumacat` (hors de l'app Fichiers), ailleurs
/// `~/.local/share/luma/catalogue.lumacat` (Windows : `%APPDATA%\LÜMA\Catalogue.lumacat`).
/// `LUMA_CATALOG` le remplace. Le format est
/// décrit dans `store.rs`.
pub fn default_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LUMA_CATALOG") {
        return Some(PathBuf::from(p));
    }
    if cfg!(windows) {
        return Some(luma_core::windows_dir(false)?.join("Catalogue.lumacat"));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    Some(if cfg!(target_os = "ios") {
        home.join("Library/Application Support/Catalogue.lumacat")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support/LÜMA/Catalogue.lumacat")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map_or_else(|| home.join(".local/share"), PathBuf::from)
            .join("luma/catalogue.lumacat")
    })
}

impl Catalog {
    /// Lit un catalogue, binaire ou JSON (le format d'avant) ; vide s'il n'existe pas,
    /// est illisible ou d'une autre version. Un catalogue illisible (abîmé, ou d'une
    /// version plus récente de LÜMA) est d'abord mis dans les sauvegardes : le prochain
    /// enregistrement ne le fait pas disparaître.
    pub fn load(path: &Path) -> Catalog {
        let loaded = Self::decode_file(path);
        // Une seule fois : relu chaque jour, le même fichier ne chasse pas, copie après
        // copie, les bonnes sauvegardes d'avant.
        let kept = || {
            let newest = backup::list(path).into_iter().next();
            newest.is_some_and(|b| std::fs::read(b).ok() == std::fs::read(path).ok())
        };
        if loaded.is_none() && path.is_file() && !kept() {
            let _ = backup::backup(path);
        }
        loaded.unwrap_or_default()
    }

    fn decode_file(path: &Path) -> Option<Catalog> {
        // Le catalogue d'avant le format binaire, en JSON, à côté.
        let old = path.with_extension("json");
        let Ok(bytes) = std::fs::read(path).or_else(|_| std::fs::read(&old)) else {
            return Some(Catalog::default());
        };
        if store::is_binary(&bytes) {
            return store::decode(&bytes);
        }
        let stored: Stored = serde_json::from_slice(&bytes).ok()?;
        (stored.version == VERSION).then(|| Catalog {
            entries: stored.entries.into_iter().map(|e| (e.path.clone(), e)).collect(),
            collections: stored.collections,
        })
    }

    /// Enregistre le catalogue. L'écriture passe par un fichier temporaire renommé : un
    /// arrêt en plein milieu laisse l'ancien index intact.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, store::encode(self))?;
        std::fs::rename(&tmp, path)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, path: &Path) -> Option<&Entry> {
        self.entries.get(path)
    }

    /// Ajoute une photo déjà lue, ou remplace celle du même chemin.
    pub fn insert(&mut self, e: Entry) {
        self.entries.insert(e.path.clone(), e);
    }

    /// Toutes les photos, par chemin.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    /// Les photos de la plus ancienne à la plus récente (date de prise de vue, puis
    /// chemin).
    pub fn by_date(&self) -> Vec<&Entry> {
        self.search(&Query::default())
    }

    /// Les photos qui répondent à la recherche, par date de prise de vue.
    pub fn search(&self, query: &Query) -> Vec<&Entry> {
        let all: Vec<&Entry> = self.entries.values().collect();
        let mut v: Vec<&Entry> = all.into_par_iter().filter(|e| query.matches(e)).collect();
        v.par_sort_unstable_by(|a, b| a.date.cmp(&b.date).then_with(|| a.path.cmp(&b.path)));
        v
    }

    /// Le nombre de photos qui répondent à la recherche : celui de [`Catalog::search`],
    /// sans rien trier ni garder.
    pub fn count(&self, query: &Query) -> usize {
        let all: Vec<&Entry> = self.entries.values().collect();
        all.into_par_iter().filter(|e| query.matches(e)).count()
    }

    /// Les mots‑clés du catalogue et leur nombre de photos, par ordre alphabétique
    /// sans tenir compte des majuscules ni des accents (« été » et « Été » ne font
    /// qu'un, rangé à « e »).
    pub fn keywords(&self) -> Vec<(String, usize)> {
        facet(distinct(
            self.entries
                .values()
                .flat_map(|e| e.keywords.iter().map(String::as_str)),
        ))
    }

    /// Les boîtiers du catalogue et leur nombre de photos. Les paires marque et modèle
    /// sont comptées d'abord : le nom d'un boîtier ne s'écrit qu'une fois par paire.
    pub fn cameras(&self) -> Vec<(String, usize)> {
        let pairs = distinct(self.entries.values().map(|e| (e.make.as_str(), e.model.as_str())));
        facet(
            pairs
                .into_iter()
                .map(|((make, model), n)| (camera_name(make, model), n))
                .filter(|(c, _)| !c.is_empty()),
        )
    }

    /// Les libellés du catalogue et leur nombre de photos.
    pub fn labels(&self) -> Vec<(String, usize)> {
        facet(distinct(
            self.entries.values().map(|e| e.label.trim()).filter(|l| !l.is_empty()),
        ))
    }

    /// Le nombre de photos de chaque note, de 0 (pas encore notée) à 5.
    pub fn ratings(&self) -> [usize; 6] {
        let mut n = [0; 6];
        for e in self.entries.values() {
            n[usize::from(e.rating.min(5))] += 1;
        }
        n
    }

    /// Les objectifs du catalogue et leur nombre de photos.
    pub fn lenses(&self) -> Vec<(String, usize)> {
        facet(distinct(
            self.entries.values().map(|e| e.lens.as_str()).filter(|l| !l.is_empty()),
        ))
    }

    /// Les photos par date de prise de vue : chaque année (la plus récente d'abord), son
    /// nombre de photos, et ses mois (`"06"`, du plus récent au plus ancien) avec le leur.
    /// Une photo sans date lisible n'y figure pas.
    pub fn dates(&self) -> Vec<Year> {
        let mut years: BTreeMap<&str, BTreeMap<&str, usize>> = BTreeMap::new();
        for e in self.entries.values() {
            let (Some(year), Some(month)) = (e.date.get(..4), e.date.get(5..7)) else {
                continue;
            };
            if !year.bytes().all(|b| b.is_ascii_digit()) || !month.parse::<u8>().is_ok_and(|m| (1..=12).contains(&m)) {
                continue;
            }
            *years.entry(year).or_default().entry(month).or_default() += 1;
        }
        years
            .into_iter()
            .rev()
            .map(|(year, months)| Year {
                year: year.to_owned(),
                count: months.values().sum(),
                months: months.into_iter().rev().map(|(m, n)| (m.to_owned(), n)).collect(),
            })
            .collect()
    }

    /// Les dossiers du catalogue et leur nombre de photos, rangés par chemin. Les photos
    /// d'un même dossier se suivent (le catalogue est rangé par chemin) : chaque dossier
    /// n'est copié qu'une fois.
    pub fn folder_counts(&self) -> Vec<(PathBuf, usize)> {
        let mut out: Vec<(PathBuf, usize)> = Vec::new();
        let mut index: HashMap<&Path, usize> = HashMap::new();
        let mut last: Option<(&Path, usize)> = None;
        for p in self.entries.keys() {
            let Some(dir) = p.parent() else { continue };
            let i = match last {
                Some((d, i)) if d == dir => i,
                _ => *index.entry(dir).or_insert_with(|| {
                    out.push((dir.to_path_buf(), 0));
                    out.len() - 1
                }),
            };
            out[i].1 += 1;
            last = Some((dir, i));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Les dossiers du catalogue, sans doublon.
    pub fn folders(&self) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = self
            .entries
            .keys()
            .filter_map(|p| p.parent().map(Path::to_path_buf))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Ajoute un dossier (et ses sous‑dossiers si `recursive`), ou le remet à jour : les
    /// nouvelles photos entrent, les modifiées sont relues, les disparues sortent.
    pub fn add_folder(&mut self, dir: &Path, recursive: bool) -> Update {
        let mut files = Vec::new();
        list(dir, recursive, &mut files);
        let mut update = Update::default();

        // Les photos disparues de ce dossier.
        let gone: Vec<PathBuf> = self
            .entries
            .keys()
            .filter(|p| {
                p.parent()
                    .is_some_and(|d| d == dir || (recursive && d.starts_with(dir)))
            })
            .filter(|p| !exists(p))
            .cloned()
            .collect();
        for p in gone {
            self.entries.remove(&p);
            update.removed += 1;
        }

        let stamps: Vec<(PathBuf, u64, u64, u64)> = files
            .into_iter()
            .map(|p| {
                // Une copie virtuelle : la taille et la date de son original.
                let (size, modified) = stamp(&copies::source(&p));
                let sidecar = stamp(&xmp::sidecar_path(&p)).1;
                (p, size, modified, sidecar)
            })
            .collect();
        let stale: Vec<_> = stamps
            .into_iter()
            .filter(|(p, size, modified, sidecar)| match self.entries.get(p) {
                Some(e) if (e.size, e.modified, e.sidecar) == (*size, *modified, *sidecar) => {
                    update.unchanged += 1;
                    false
                }
                Some(_) => {
                    update.updated += 1;
                    true
                }
                None => {
                    update.added += 1;
                    true
                }
            })
            .collect();
        let read: Vec<Entry> = stale
            .into_par_iter()
            .map(|(path, size, modified, sidecar)| read_entry(path, size, modified, sidecar))
            .collect();
        for e in read {
            self.entries.insert(e.path.clone(), e);
        }
        update
    }

    /// Une photo renommée (renommage par lot) : son entrée et sa place dans les
    /// collections manuelles suivent, sans rien relire.
    pub fn rename(&mut self, from: &Path, to: &Path) {
        if let Some(mut e) = self.entries.remove(from) {
            e.path = to.to_path_buf();
            let (_, modified) = stamp(to);
            if modified != 0 {
                e.modified = modified;
            }
            e.sidecar = stamp(&xmp::sidecar_path(to)).1;
            self.entries.insert(to.to_path_buf(), e);
        }
        for c in &mut self.collections {
            if let Content::Manual(list) = &mut c.content {
                for p in list.iter_mut().filter(|p| *p == from) {
                    *p = to.to_path_buf();
                }
            }
        }
    }

    /// Relit le XMP d'une photo (après un changement de note ou de mots‑clés dans l'app).
    pub fn refresh_meta(&mut self, path: &Path) {
        if let Some(e) = self.entries.get_mut(path) {
            let side = xmp::sidecar_path(path);
            e.sidecar = stamp(&side).1;
            e.set_meta(read_meta(&side));
        }
    }

    /// Renomme un mot‑clé (`to`) ou le retire (`None`) dans toutes les photos du
    /// catalogue qui l'ont, majuscules et accents mis à part : leurs XMP sont réécrits
    /// (réglages, note et le reste gardés), puis relus. Rend le nombre de photos.
    pub fn edit_keyword(&mut self, word: &str, to: Option<&str>) -> Result<usize, String> {
        let paths: Vec<PathBuf> = self
            .entries
            .values()
            .filter(|e| e.keywords.iter().any(|k| query::fold(k) == query::fold(word)))
            .map(|e| e.path.clone())
            .collect();
        for p in &paths {
            let mut meta = photo_meta(p);
            rename_keyword(&mut meta.keywords, word, to);
            write_photo_meta(p, &meta)?;
            self.refresh_meta(p);
        }
        Ok(paths.len())
    }
}

/// Les colonnes d'une liste en CSV, dans l'ordre de [`csv`] (la ligne de commande les
/// écrit ainsi ; l'app, dans sa langue).
pub const CSV_COLUMNS: [&str; 14] = [
    "Fichier",
    "Date",
    "Boîtier",
    "Objectif",
    "ISO",
    "Ouverture",
    "Vitesse (s)",
    "Focale (mm)",
    "Note",
    "Drapeau",
    "Libellé",
    "Mots-clés",
    "Latitude",
    "Longitude",
];

/// Les mots du drapeau dans une liste en CSV : retenue, rejetée.
pub const CSV_FLAGS: [&str; 2] = ["retenue", "rejetée"];

/// Des photos en tableau CSV (RFC 4180 : virgules, guillemets doublés, fins de ligne
/// CRLF), une ligne d'en‑tête (`columns`), puis une ligne par photo ; les mots‑clés
/// séparés par « ; », les nombres avec un point, une valeur absente laissée vide ; le
/// drapeau écrit avec `flags` (retenue, rejetée).
pub fn csv(entries: &[&Entry], columns: &[&str; 14], flags: [&str; 2]) -> String {
    fn cell(out: &mut String, v: &str) {
        if v.contains([',', '"', '\n', '\r']) {
            out.push('"');
            out.push_str(&v.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(v);
        }
    }
    let mut out = String::new();
    let row = |out: &mut String, values: &[String]| {
        for (i, v) in values.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            cell(out, v);
        }
        out.push_str("\r\n");
    };
    row(&mut out, &columns.map(str::to_owned));
    let num = |v: Option<f32>| v.map(|v| format!("{v}")).unwrap_or_default();
    for e in entries {
        let flag = match e.flag() {
            Flag::Pick => flags[0],
            Flag::Reject => flags[1],
            Flag::None => "",
        };
        row(
            &mut out,
            &[
                e.path.display().to_string(),
                e.date.replace('T', " "),
                e.camera(),
                e.lens.clone(),
                e.iso.map(|i| i.to_string()).unwrap_or_default(),
                num(e.aperture),
                num(e.exposure_time),
                num(e.focal_length),
                e.rating.to_string(),
                flag.to_owned(),
                e.label.clone(),
                e.keywords.join("; "),
                e.gps.map(|g| format!("{:.6}", g[0])).unwrap_or_default(),
                e.gps.map(|g| format!("{:.6}", g[1])).unwrap_or_default(),
            ],
        );
    }
    out
}

/// Dans une liste de mots‑clés, `word` (majuscules et accents mis à part) devient `to`,
/// ou s'en va (`None`) ; un mot‑clé qui serait là deux fois ne l'est qu'une.
pub fn rename_keyword(keywords: &mut Vec<String>, word: &str, to: Option<&str>) {
    let mut out: Vec<String> = Vec::new();
    for k in keywords.iter() {
        let k = if query::fold(k) == query::fold(word) {
            match to {
                Some(t) => t.trim().to_owned(),
                None => continue,
            }
        } else {
            k.clone()
        };
        if !out.iter().any(|o| query::fold(o) == query::fold(&k)) {
            out.push(k);
        }
    }
    *keywords = out;
}

/// La note, le drapeau, le libellé et les mots‑clés d'une photo : ceux de son XMP, à
/// défaut ceux que le fichier porte en lui.
pub fn photo_meta(path: &Path) -> Meta {
    match std::fs::read_to_string(xmp::sidecar_path(path)) {
        Ok(t) => xmp::read_meta(&t),
        Err(_) => embedded_xmp(&copies::source(path))
            .map(|t| xmp::read_meta(&t))
            .unwrap_or_default(),
    }
}

/// Les réglages enregistrés d'une photo : ceux de son XMP, à défaut ceux qu'un RAW ou
/// un DNG porte en lui.
pub fn photo_settings(path: &Path) -> Option<luma_project::Settings> {
    match std::fs::read_to_string(xmp::sidecar_path(path)) {
        Ok(text) => xmp::read(&text).ok(),
        Err(_) => embedded_settings(&copies::source(path)),
    }
}

/// Écrit les métadonnées d'une photo dans son XMP, ses réglages gardés tels quels ;
/// sans réglages, un XMP de métadonnées seules (la photo garde son rendu par défaut).
pub fn write_photo_meta(path: &Path, meta: &Meta) -> Result<(), String> {
    let target = xmp::sidecar_path(path);
    let text = match photo_settings(path) {
        Some(settings) => xmp::write_with_meta(&settings, meta),
        None => xmp::write_meta(meta),
    };
    xmp::save(&target, &text).map_err(|e| format!("{} : {e}", target.display()))
}

/// Compte des valeurs, regroupées sans tenir compte des majuscules ni des accents ;
/// chaque groupe garde la première graphie rencontrée.
fn facet<S: AsRef<str>>(values: impl IntoIterator<Item = (S, usize)>) -> Vec<(String, usize)> {
    let mut groups: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for (v, n) in values {
        let v = v.as_ref();
        groups.entry(query::fold(v)).or_insert_with(|| (v.to_owned(), 0)).1 += n;
    }
    groups.into_values().collect()
}

/// Les valeurs distinctes, dans l'ordre où elles paraissent, avec leur nombre : sans
/// copie, pour que [`facet`] ne replie (minuscules, accents) chaque valeur qu'une
/// fois. Le premier venu d'un groupe reste celui qui le nomme, comme avant.
fn distinct<T: Eq + std::hash::Hash + Copy>(values: impl Iterator<Item = T>) -> Vec<(T, usize)> {
    let mut index: HashMap<T, usize> = HashMap::new();
    let mut order: Vec<(T, usize)> = Vec::new();
    for v in values {
        match index.get(&v) {
            Some(&i) => order[i].1 += 1,
            None => {
                index.insert(v, order.len());
                order.push((v, 1));
            }
        }
    }
    order
}

/// Le nom d'un boîtier, marque et modèle (le modèle seul s'il commence par la marque).
fn camera_name(make: &str, model: &str) -> String {
    if model.starts_with(make) {
        model.to_owned()
    } else {
        format!("{make} {model}").trim().to_owned()
    }
}

/// Les photos d'un dossier.
fn list(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for e in read.flatten() {
        let path = e.path();
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_dir() {
            // Les dossiers cachés (`.Trash`, paquets du système) ne sont pas parcourus.
            let hidden = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'));
            if recursive && !hidden {
                list(&path, recursive, out);
            }
        } else if luma_raw::is_photo(&path) {
            out.push(path);
        } else if let Some(copy) = copy_of_sidecar(&path) {
            out.push(copy);
        }
    }
}

/// La copie virtuelle dont ce fichier est le XMP (`IMG_0001.ARW.copie-2.xmp` →
/// `IMG_0001.ARW.copie-2`), si son original est là.
fn copy_of_sidecar(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    let copy = path.with_file_name(name.strip_suffix(".xmp")?);
    copies::number(&copy)?;
    let source = copies::source(&copy);
    (luma_raw::is_photo(&source) && source.is_file()).then_some(copy)
}

/// La photo est‑elle encore là ? Pour une copie virtuelle : son original et son XMP.
pub fn exists(path: &Path) -> bool {
    if copies::number(path).is_some() {
        copies::source(path).is_file() && copies::sidecar(path).is_file()
    } else {
        path.is_file()
    }
}

/// Une année de [`Catalog::dates`] : son nombre de photos et ses mois.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Year {
    pub year: String,
    pub count: usize,
    pub months: Vec<(String, usize)>,
}

/// Taille et date de modification d'un fichier ; (0, 0) s'il n'existe pas.
fn stamp(path: &Path) -> (u64, u64) {
    std::fs::metadata(path).map_or((0, 0), |m| {
        let modified = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        (m.len(), modified)
    })
}

fn read_meta(sidecar: &Path) -> Meta {
    std::fs::read_to_string(sidecar)
        .map(|t| xmp::read_meta(&t))
        .unwrap_or_default()
}

/// Ce que le catalogue retient d'une photo, lu tout de suite (sans passer par le
/// catalogue).
pub fn entry_of(path: &Path) -> Entry {
    let (size, modified) = stamp(&copies::source(path));
    let sidecar = stamp(&xmp::sidecar_path(path)).1;
    read_entry(path.to_path_buf(), size, modified, sidecar)
}

fn read_entry(path: PathBuf, size: u64, modified: u64, sidecar: u64) -> Entry {
    // Les pixels et l'EXIF d'une copie virtuelle sont ceux de son original.
    let info = luma_raw::info(copies::source(&path)).unwrap_or_default();
    let exif = info.exif;
    let mut e = Entry {
        size,
        modified,
        sidecar,
        date: exif
            .date
            .as_deref()
            .and_then(exif_date)
            .unwrap_or_else(|| unix_date(modified)),
        make: info.make,
        model: info.model,
        lens: exif.lens.unwrap_or_default().trim().to_owned(),
        iso: exif.iso,
        aperture: exif.aperture,
        exposure_time: exif.exposure_time,
        focal_length: exif.focal_length,
        gps: exif.gps.map(|g| [g.latitude, g.longitude]),
        ..Entry::default()
    };
    if sidecar != 0 {
        e.set_meta(read_meta(&xmp::sidecar_path(&path)));
    } else if let Some(text) = embedded_xmp(&copies::source(&path)) {
        // Sans XMP à côté : celui que le fichier porte en lui (une note, un libellé, des
        // mots‑clés posés par un autre logiciel ou par le boîtier).
        e.set_meta(xmp::read_meta(&text));
    }
    e.path = path;
    e
}

/// Où chercher le XMP qu'un fichier porte en lui : ses 64 premiers Ko (le paquet y est
/// presque toujours, dans un JPEG, un TIFF, un DNG, un PNG ou un RAW qui en a un : dans
/// tous ceux du corpus d'essai).
const EMBEDDED_XMP_SPAN: u64 = 64 << 10;

/// Les paquets XMP intégrés déjà cherchés, par fichier (sa taille et sa date) : l'app
/// en demande plusieurs fois par photo (note, réglages, vignette) ; lu une seule fois.
type XmpCache = std::sync::Mutex<HashMap<PathBuf, ((u64, u64), Option<String>)>>;
static EMBEDDED: std::sync::LazyLock<XmpCache> = std::sync::LazyLock::new(Default::default);

/// Le paquet XMP rangé dans le fichier lui‑même (`<x:xmpmeta …>…</x:xmpmeta>`), s'il y
/// en a un dans ses premiers octets.
pub fn embedded_xmp(path: &Path) -> Option<String> {
    let modified = stamp(path);
    {
        let cache = EMBEDDED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((m, xmp)) = cache.get(path)
            && *m == modified
        {
            return xmp.clone();
        }
    }
    let xmp = read_embedded_xmp(path);
    let mut cache = EMBEDDED.lock().unwrap_or_else(|e| e.into_inner());
    // Un plafond : une longue séance sur des milliers de photos ne garde pas tout.
    if cache.len() >= 8192 {
        cache.clear();
    }
    cache.insert(path.to_path_buf(), (modified, xmp.clone()));
    xmp
}

/// Les réglages de développement qu'un RAW ou un DNG porte en lui (développé par un
/// autre logiciel, qui les a rangés dans le fichier) : à prendre quand il n'a pas de XMP
/// à côté. Jamais ceux d'un JPEG ou d'une autre image déjà développée : ils sont déjà
/// dans ses pixels, les reprendre les doublerait.
pub fn embedded_settings(path: &Path) -> Option<luma_project::Settings> {
    if !luma_raw::is_raw(path) {
        return None;
    }
    xmp::read(&embedded_xmp(path)?).ok()
}

fn read_embedded_xmp(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut head = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(EMBEDDED_XMP_SPAN)
        .read_to_end(&mut head)
        .ok()?;
    let find = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).position(|w| w == needle);
    let start = find(&head, b"<x:xmpmeta")?;
    let end = start + find(&head[start..], b"</x:xmpmeta>")? + b"</x:xmpmeta>".len();
    String::from_utf8(head[start..end].to_vec()).ok()
}

/// Une date EXIF (`2026:10:26 14:03:00`) au format du catalogue.
pub fn exif_date(s: &str) -> Option<String> {
    let s = s.trim();
    let b = s.as_bytes();
    let digits = |r: std::ops::Range<usize>| b.get(r).is_some_and(|d| d.iter().all(u8::is_ascii_digit));
    if !(digits(0..4) && digits(5..7) && digits(8..10)) || s.starts_with("0000") {
        return None;
    }
    let time = if digits(11..13) && digits(14..16) && digits(17..19) {
        &s[11..19]
    } else {
        "00:00:00"
    };
    Some(format!(
        "{}-{}-{}T{}",
        &s[0..4],
        &s[5..7],
        &s[8..10],
        time.replace(['.', '-'], ":")
    ))
}

/// Une date en secondes depuis 1970 (UTC), au format du catalogue.
pub fn unix_date(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Du nombre de jours au calendrier grégorien (algorithme des jours civils).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("luma-library-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn png(path: &Path) {
        // La plus petite image PNG valide n'est pas nécessaire : seul le nom compte ici,
        // l'en‑tête d'une image développée n'étant pas lu.
        std::fs::write(path, b"\x89PNG\r\n\x1a\n").unwrap();
    }

    /// Le XMP porté par le fichier lui‑même (un JPEG noté ailleurs) : sa note, son
    /// libellé et ses mots‑clés entrent au catalogue ; un XMP à côté l'emporte.
    #[test]
    fn the_xmp_inside_a_file_is_read() {
        let dir = temp("xmp-integre");
        let packet = r#"<x:xmpmeta xmlns:x="x"><rdf:RDF xmlns:rdf="r"><rdf:Description xmlns:xmp="m" xmlns:dc="d" xmp:Rating="4" xmp:Label="Rouge"><dc:subject><rdf:Bag><rdf:li>mer</rdf:li><rdf:li>été</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF></x:xmpmeta>"#;
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x10];
        jpeg.extend_from_slice(packet.as_bytes());
        jpeg.extend_from_slice(&[0xFF, 0xD9]);
        let photo = dir.join("IMG_0001.jpg");
        std::fs::write(&photo, &jpeg).unwrap();
        assert!(embedded_xmp(&photo).is_some_and(|x| x.starts_with("<x:xmpmeta") && x.ends_with("</x:xmpmeta>")));
        let e = entry_of(&photo);
        assert_eq!((e.rating, e.label.as_str()), (4, "Rouge"));
        assert_eq!(e.keywords, ["mer", "été"]);
        // Un XMP à côté : c'est lui qui compte.
        let meta = Meta {
            rating: 2,
            ..Meta::default()
        };
        std::fs::write(
            xmp::sidecar_path(&photo),
            xmp::write_with_meta(&luma_project::Settings::default(), &meta),
        )
        .unwrap();
        assert_eq!(entry_of(&photo).rating, 2);
        // Un fichier sans paquet : rien.
        let plain = dir.join("b.jpg");
        std::fs::write(&plain, [0xFF, 0xD8, 0xFF, 0xD9]).unwrap();
        assert!(embedded_xmp(&plain).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Les réglages qu'un DNG porte en lui sont repris ; ceux d'un JPEG, déjà dans ses
    /// pixels, jamais. Un fichier changé est relu (le cache suit sa taille et sa date).
    #[test]
    fn settings_inside_a_raw_but_not_inside_a_jpeg() {
        let dir = temp("reglages-integres");
        let packet = |ev: &str| {
            format!(
                r#"<x:xmpmeta xmlns:x="x"><rdf:RDF xmlns:rdf="r"><rdf:Description xmlns:crs="c" crs:Exposure2012="{ev}" crs:Contrast2012="+20"/></rdf:RDF></x:xmpmeta>"#
            )
        };
        let dng = dir.join("IMG_0001.dng");
        let jpg = dir.join("IMG_0001.jpg");
        std::fs::write(&dng, packet("+1.50")).unwrap();
        std::fs::write(&jpg, packet("+1.50")).unwrap();
        let s = embedded_settings(&dng).expect("les réglages du DNG");
        assert!((s.exposure - 1.5).abs() < 1e-6 && (s.contrast - 20.0).abs() < 1e-6);
        assert!(embedded_settings(&jpg).is_none(), "déjà dans les pixels du JPEG");
        // Le fichier réécrit (une autre taille) : relu, pas pris dans le cache.
        std::fs::write(&dng, packet("-0.750")).unwrap();
        assert!((embedded_settings(&dng).unwrap().exposure + 0.75).abs() < 1e-6);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Une copie virtuelle entre au catalogue avec la date et la taille de son original,
    /// sa note à elle ; elle en sort quand son XMP disparaît.
    #[test]
    fn virtual_copies_are_in_the_catalog() {
        let dir = temp("copies");
        png(&dir.join("a.png"));
        let copy = luma_project::copies::path(&dir.join("a.png"), 2);
        let meta = Meta {
            rating: 4,
            ..Meta::default()
        };
        std::fs::write(
            luma_project::copies::sidecar(&copy),
            xmp::write_with_meta(&luma_project::Settings::default(), &meta),
        )
        .unwrap();
        // Le XMP d'une copie dont l'original n'est pas là : rien.
        std::fs::write(dir.join("absent.png.copie-3.xmp"), "").unwrap();
        let mut cat = Catalog::default();
        let u = cat.add_folder(&dir, false);
        assert_eq!((u.added, cat.len()), (2, 2));
        let (orig, c) = (cat.get(&dir.join("a.png")).unwrap(), cat.get(&copy).unwrap());
        assert_eq!((&c.date, c.size), (&orig.date, orig.size));
        assert_eq!((orig.rating, c.rating), (0, 4));
        assert!(cat.missing().is_empty() && cat.duplicates().is_empty());
        assert_eq!(entry_of(&copy).rating, 4);
        // Rien à relire ensuite.
        let u = cat.add_folder(&dir, false);
        assert_eq!((u.added, u.updated, u.unchanged, u.removed), (0, 0, 2, 0));
        // Le XMP de la copie supprimé : la copie sort du catalogue, l'original reste.
        std::fs::remove_file(luma_project::copies::sidecar(&copy)).unwrap();
        let u = cat.add_folder(&dir, false);
        assert_eq!((u.removed, cat.len()), (1, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Une photo renommée garde son entrée (note, date) et sa place dans une collection
    /// manuelle.
    #[test]
    fn renamed_photo_keeps_entry_and_collections() {
        let dir = temp("renommage");
        png(&dir.join("a.png"));
        let mut cat = Catalog::default();
        cat.add_folder(&dir, false);
        let mut c = Collection::manual("Choix");
        c.content = Content::Manual(vec![dir.join("a.png")]);
        cat.collections.push(c);
        let date = cat.get(&dir.join("a.png")).unwrap().date.clone();
        std::fs::rename(dir.join("a.png"), dir.join("b.png")).unwrap();
        cat.rename(&dir.join("a.png"), &dir.join("b.png"));
        assert!(cat.get(&dir.join("a.png")).is_none());
        let e = cat.get(&dir.join("b.png")).unwrap();
        assert_eq!((e.path.clone(), e.date.clone()), (dir.join("b.png"), date));
        assert_eq!(cat.collections[0].content, Content::Manual(vec![dir.join("b.png")]));
        // Rien à relire ensuite : le fichier renommé est à jour.
        let u = cat.add_folder(&dir, false);
        assert_eq!((u.added, u.removed, u.unchanged), (0, 0, 1));
        assert_eq!(entry_of(&dir.join("b.png")).path, dir.join("b.png"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dates() {
        assert_eq!(exif_date("2026:10:26 14:03:07").as_deref(), Some("2026-10-26T14:03:07"));
        assert_eq!(exif_date("2026:10:26").as_deref(), Some("2026-10-26T00:00:00"));
        assert_eq!(exif_date("0000:00:00 00:00:00"), None);
        assert_eq!(exif_date("hier"), None);
        assert_eq!(unix_date(0), "1970-01-01T00:00:00");
        assert_eq!(unix_date(951_782_400), "2000-02-29T00:00:00");
        assert_eq!(unix_date(1_793_017_387), "2026-10-26T12:23:07");
    }

    #[test]
    fn a_folder_is_indexed_then_updated_incrementally() {
        let dir = temp("maj");
        std::fs::create_dir_all(dir.join("jour 2")).unwrap();
        std::fs::create_dir_all(dir.join(".cache")).unwrap();
        for name in ["a.png", "b.png", "jour 2/c.png", ".cache/d.png"] {
            png(&dir.join(name));
        }
        std::fs::write(dir.join("notes.txt"), "pas une photo").unwrap();
        let meta = Meta {
            rating: 4,
            flag: Flag::Pick,
            label: "Rouge".into(),
            keywords: vec!["Mer".into(), "Été".into()],
            ..Meta::default()
        };
        std::fs::write(
            dir.join("a.xmp"),
            xmp::write_with_meta(&luma_project::Settings::default(), &meta),
        )
        .unwrap();

        let mut cat = Catalog::default();
        let first = cat.add_folder(&dir, true);
        assert_eq!((first.added, first.unchanged), (3, 0), "dossier caché et texte ignorés");
        let a = cat.get(&dir.join("a.png")).unwrap();
        assert_eq!((a.rating, a.flag(), a.label.as_str()), (4, Flag::Pick, "Rouge"));
        assert_eq!(a.keywords, ["Mer", "Été"]);
        assert_eq!(a.date.len(), 19, "date du fichier faute d'EXIF : {}", a.date);

        // Rien n'a changé : rien n'est relu.
        assert_eq!(cat.add_folder(&dir, true).unchanged, 3);

        // Une photo disparaît, une autre arrive, un XMP apparaît.
        std::fs::remove_file(dir.join("b.png")).unwrap();
        png(&dir.join("e.png"));
        std::fs::write(
            dir.join("jour 2/c.xmp"),
            xmp::write_with_meta(&luma_project::Settings::default(), &meta),
        )
        .unwrap();
        let next = cat.add_folder(&dir, true);
        assert_eq!((next.added, next.updated, next.unchanged, next.removed), (1, 1, 1, 1));
        assert_eq!(cat.get(&dir.join("jour 2/c.png")).unwrap().rating, 4);
        assert_eq!(cat.folders(), [dir.clone(), dir.join("jour 2")]);

        // Sans sous‑dossiers, ceux‑ci ne sont ni lus ni retirés.
        assert_eq!(cat.add_folder(&dir, false).removed, 0);
        assert_eq!(cat.len(), 3);

        // Enregistré puis relu : le même catalogue.
        let file = dir.join("index/catalogue.json");
        cat.save(&file).unwrap();
        cat.add_to_collection("Tirages", &[dir.join("a.png")]).unwrap();
        cat.set_collection(Collection::smart("Retenues", "drapeau:retenue").unwrap())
            .unwrap();
        cat.save(&file).unwrap();
        assert_eq!(Catalog::load(&file), cat);
        assert_eq!(Catalog::load(&file).collection_photos("Retenues").unwrap().len(), 2);
        // Un index d'avant les collections se relit tel quel.
        std::fs::write(&file, "{\"version\":1,\"entries\":[{\"path\":\"x.arw\",\"rating\":3}]}").unwrap();
        let old = Catalog::load(&file);
        assert_eq!((old.len(), old.collections().len()), (1, 0));
        // Le nouveau fichier n'existe pas encore : l'ancien, en JSON, à côté.
        assert_eq!(Catalog::load(&file.with_extension("lumacat")), old);
        assert!(Catalog::load(&dir.join("absent.json")).is_empty());
        std::fs::write(&file, "{\"version\":999,\"entries\":[]}").unwrap();
        assert!(Catalog::load(&file).is_empty(), "autre version : reconstruit");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sorted_by_capture_date() {
        let mut cat = Catalog::default();
        for (name, date) in [
            ("z.arw", "2024-01-02T10:00:00"),
            ("a.arw", "2025-06-01T08:00:00"),
            ("m.arw", "2024-01-02T09:59:59"),
        ] {
            let e = Entry {
                path: name.into(),
                date: date.into(),
                ..Entry::default()
            };
            cat.insert(e);
        }
        let order: Vec<_> = cat
            .by_date()
            .iter()
            .map(|e| e.path.to_str().unwrap().to_owned())
            .collect();
        assert_eq!(order, ["m.arw", "z.arw", "a.arw"]);
        let e = Entry {
            make: "Sony".into(),
            model: "ILCE-7M3".into(),
            ..Entry::default()
        };
        assert_eq!(e.camera(), "Sony ILCE-7M3");
    }

    #[test]
    fn search_and_facets() {
        let mut cat = Catalog::default();
        for (name, date, model, keywords) in [
            ("a.arw", "2024-06-02T10:00:00", "ILCE-7M3", vec!["Mer", "Été"]),
            ("b.arw", "2024-06-01T10:00:00", "ILCE-7M3", vec!["mer"]),
            ("c.arw", "2025-01-01T10:00:00", "ILCE-1", vec!["Montagne", "été"]),
        ] {
            let e = Entry {
                path: name.into(),
                date: date.into(),
                make: "Sony".into(),
                model: model.into(),
                keywords: keywords.into_iter().map(String::from).collect(),
                ..Entry::default()
            };
            cat.insert(e);
        }
        let found = |q: &str| -> Vec<String> {
            cat.search(&Query::parse(q).unwrap())
                .iter()
                .map(|e| e.path.display().to_string())
                .collect()
        };
        assert_eq!(found("mer"), ["b.arw", "a.arw"], "par date de prise de vue");
        assert_eq!(found("ete date:2025"), ["c.arw"]);
        assert_eq!(found(""), ["b.arw", "a.arw", "c.arw"]);
        assert_eq!(
            cat.keywords(),
            [("Été".to_owned(), 2), ("Mer".to_owned(), 2), ("Montagne".to_owned(), 1)]
        );
        assert_eq!(
            cat.cameras(),
            [("Sony ILCE-1".to_owned(), 1), ("Sony ILCE-7M3".to_owned(), 2)]
        );
        assert!(cat.lenses().is_empty());
    }

    /// Un catalogue illisible repart de zéro, mais l'ancien fichier reste dans les
    /// sauvegardes : l'enregistrement suivant ne le fait pas disparaître.
    #[test]
    fn an_unreadable_catalog_is_kept_in_the_backups() {
        let dir = std::env::temp_dir().join(format!("luma-illisible-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("catalogue.lumacat");
        std::fs::write(&file, b"pas un catalogue").unwrap();
        assert!(Catalog::load(&file).is_empty());
        let kept = backup::list(&file);
        assert_eq!(kept.len(), 1);
        assert_eq!(std::fs::read(&kept[0]).unwrap(), b"pas un catalogue");
        // Absent : vide, sans sauvegarde de plus.
        assert!(Catalog::load(&dir.join("aucun.lumacat")).is_empty());
        assert_eq!(backup::list(&file).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Relu bien des fois (l'app ouverte chaque jour, la ligne de commande), le même
    /// catalogue illisible n'est sauvegardé qu'une fois : les bonnes sauvegardes d'avant
    /// ne sont pas chassées par ses copies.
    #[test]
    fn an_unreadable_catalog_does_not_push_out_the_good_backups() {
        let dir = std::env::temp_dir().join(format!("luma-illisible-relu-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("catalogue.lumacat");
        std::fs::write(&file, b"un bon catalogue").unwrap();
        let good = backup::backup_at(&file, 1_000_000_000, backup::KEEP).unwrap();
        std::fs::write(&file, b"pas un catalogue").unwrap();
        for _ in 0..backup::KEEP + 2 {
            assert!(Catalog::load(&file).is_empty());
        }
        assert_eq!(backup::list(&file).len(), 2);
        assert_eq!(std::fs::read(&good).unwrap(), b"un bon catalogue");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Les photos par année et par mois : les plus récentes d'abord ; une date sans
    /// mois lisible, ou absente, n'entre pas.
    #[test]
    fn photos_by_year_and_month() {
        let mut cat = Catalog::default();
        for (name, date) in [
            ("a", "2024-06-15T10:00:00"),
            ("b", "2024-06-16T09:00:00"),
            ("c", "2024-01-02T08:00:00"),
            ("d", "2026-10-29T20:00:00"),
            ("e", ""),
            ("f", "2025-13-01T00:00:00"),
            ("g", "inconnue"),
        ] {
            cat.insert(Entry {
                path: PathBuf::from(format!("/photos/{name}.jpg")),
                date: date.into(),
                ..Entry::default()
            });
        }
        let year = |y: &str, n, m: &[(&str, usize)]| Year {
            year: y.into(),
            count: n,
            months: m.iter().map(|(a, b)| ((*a).to_owned(), *b)).collect(),
        };
        assert_eq!(
            cat.dates(),
            [year("2026", 1, &[("10", 1)]), year("2024", 3, &[("06", 2), ("01", 1)])]
        );
        assert!(Catalog::default().dates().is_empty());
    }

    /// Les listes de mots‑clés, de boîtiers et d'objectifs comptées sans copie sont
    /// celles d'avant : mêmes groupes (majuscules et accents mis à part), même nom pour
    /// chacun (le premier venu), mêmes nombres.
    #[test]
    fn facets_are_the_same_as_counted_one_by_one() {
        fn before(values: impl Iterator<Item = String>) -> Vec<(String, usize)> {
            let mut groups: BTreeMap<String, (String, usize)> = BTreeMap::new();
            for v in values {
                groups.entry(query::fold(&v)).or_insert_with(|| (v, 0)).1 += 1;
            }
            groups.into_values().collect()
        }
        let mut cat = Catalog::default();
        let words = ["Été", "été", "ete", "Mer", "mer", "coucher de soleil", "Œuvre"];
        let makes = [
            ("SONY", "ILCE-7M3"),
            ("Sony", "ILCE-7M3"),
            ("", "iPhone 15"),
            ("Canon", "Canon EOS R5"),
            ("", ""),
        ];
        for i in 0..400usize {
            cat.insert(Entry {
                path: PathBuf::from(format!("/p/{:03}/{i}.jpg", (i * 37) % 101)),
                make: makes[i % 5].0.into(),
                model: makes[i % 5].1.into(),
                lens: ["", "FE 24-70mm", "fe 24-70MM", "XF 35mm"][i % 4].into(),
                keywords: (0..i % 4).map(|k| words[(i + k) % 7].to_owned()).collect(),
                ..Entry::default()
            });
        }
        let e = || cat.entries.values();
        assert_eq!(cat.keywords(), before(e().flat_map(|e| e.keywords.iter().cloned())));
        assert_eq!(cat.cameras(), before(e().map(Entry::camera).filter(|c| !c.is_empty())));
        assert_eq!(
            cat.lenses(),
            before(e().map(|e| e.lens.clone()).filter(|l| !l.is_empty()))
        );
        assert_eq!(cat.cameras().len(), 3);
        // Les dossiers et leurs nombres : ceux du comptage photo par photo.
        let mut folders: BTreeMap<PathBuf, usize> = BTreeMap::new();
        for e in e() {
            if let Some(d) = e.path.parent() {
                *folders.entry(d.to_path_buf()).or_default() += 1;
            }
        }
        assert_eq!(cat.folder_counts(), folders.into_iter().collect::<Vec<_>>());
    }

    /// Un mot‑clé renommé ou retiré dans toutes ses photos, sur les fichiers : les XMP
    /// réécrits gardent la note et les réglages ; une photo sans le mot ne change pas ;
    /// pas de doublon quand le nouveau nom y était déjà.
    #[test]
    fn a_keyword_is_renamed_in_the_files() {
        let mut list = vec!["Mer".to_owned(), "Été".to_owned(), "ete".to_owned()];
        rename_keyword(&mut list, "été", Some("Océan"));
        assert_eq!(list, ["Mer", "Océan"]);
        rename_keyword(&mut list, "OCEAN", Some("mer"));
        assert_eq!(list, ["Mer"]);
        rename_keyword(&mut list, "mer", None);
        assert!(list.is_empty());

        let dir = std::env::temp_dir().join(format!("luma-mot-fichiers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.png"), dir.join("b.png"));
        for p in [&a, &b] {
            std::fs::write(p, b"\x89PNG\r\n\x1a\n").unwrap();
        }
        let settings = luma_project::Settings {
            exposure: 0.7,
            ..luma_project::Settings::default()
        };
        let meta = Meta {
            rating: 4,
            keywords: vec!["mer".into(), "Plage".into()],
            ..Meta::default()
        };
        xmp::save(&xmp::sidecar_path(&a), &xmp::write_with_meta(&settings, &meta)).unwrap();
        xmp::save(
            &xmp::sidecar_path(&b),
            &xmp::write_meta(&Meta {
                keywords: vec!["Montagne".into()],
                ..Meta::default()
            }),
        )
        .unwrap();
        let mut cat = Catalog::default();
        cat.add_folder(&dir, false);
        assert_eq!(cat.edit_keyword("MER", Some("Océan")).unwrap(), 1);
        let got = photo_meta(&a);
        assert_eq!(
            (got.rating, got.keywords.clone()),
            (4, vec!["Océan".to_owned(), "Plage".to_owned()])
        );
        assert_eq!(photo_settings(&a).unwrap().exposure, 0.7);
        assert_eq!(photo_meta(&b).keywords, ["Montagne"]);
        assert!(cat.keywords().iter().any(|(k, n)| k == "Océan" && *n == 1));
        assert_eq!(cat.edit_keyword("plage", None).unwrap(), 1);
        assert_eq!(photo_meta(&a).keywords, ["Océan"]);
        assert_eq!(cat.edit_keyword("absent", None).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Les libellés comptés comme les mots‑clés (majuscules mises à part), et les notes
    /// une à une.
    #[test]
    fn labels_and_ratings_are_counted() {
        let mut cat = Catalog::default();
        for (i, (label, rating)) in [("Rouge", 5), ("rouge", 3), ("Vert", 5), ("", 0), ("  ", 9)]
            .into_iter()
            .enumerate()
        {
            cat.insert(Entry {
                path: PathBuf::from(format!("/p/{i}.jpg")),
                label: label.into(),
                rating,
                ..Entry::default()
            });
        }
        assert_eq!(cat.labels(), [("Rouge".to_owned(), 2), ("Vert".to_owned(), 1)]);
        assert_eq!(cat.ratings(), [1, 0, 0, 1, 0, 3], "une note au‑delà de 5 compte pour 5");
    }

    /// La liste en CSV : en‑tête, une ligne par photo, guillemets là où il le faut
    /// (virgule, guillemet), valeurs absentes vides.
    #[test]
    fn photos_as_csv() {
        let a = Entry {
            path: PathBuf::from("/p/Mer, été/IMG_1.ARW"),
            date: "2026-06-01T10:00:00".into(),
            make: "SONY".into(),
            model: "ILCE-7M3".into(),
            lens: "FE 24-70mm".into(),
            iso: Some(100),
            aperture: Some(4.0),
            exposure_time: Some(0.004),
            focal_length: Some(35.0),
            rating: 4,
            flag: 1,
            label: "Rouge".into(),
            keywords: vec!["mer".into(), "dit \"bleu\"".into()],
            gps: Some([48.8584, 2.2945]),
            ..Entry::default()
        };
        let b = Entry {
            path: PathBuf::from("/p/b.jpg"),
            ..Entry::default()
        };
        let text = csv(&[&a, &b], &CSV_COLUMNS, CSV_FLAGS);
        let lines: Vec<&str> = text.split("\r\n").collect();
        assert_eq!(
            lines[0],
            "Fichier,Date,Boîtier,Objectif,ISO,Ouverture,Vitesse (s),Focale (mm),Note,Drapeau,Libellé,Mots-clés,Latitude,Longitude"
        );
        assert_eq!(
            lines[1],
            "\"/p/Mer, été/IMG_1.ARW\",2026-06-01 10:00:00,SONY ILCE-7M3,FE 24-70mm,100,4,0.004,35,4,retenue,Rouge,\"mer; dit \"\"bleu\"\"\",48.858400,2.294500"
        );
        assert_eq!(lines[2], "/p/b.jpg,,,,,,,,0,,,,,");
        assert_eq!(lines[3], "");
    }
}
