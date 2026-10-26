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

pub mod collections;
pub mod query;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use luma_project::xmp::{self, Flag, Meta};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

pub use collections::{Collection, Content};
pub use query::Query;

/// La version du format de l'index ; un index d'une autre version est reconstruit.
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
    pub rating: u8,
    /// 1 : retenue, -1 : rejetée.
    pub flag: i8,
    pub label: String,
    pub keywords: Vec<String>,
}

impl Entry {
    /// Le boîtier, marque comprise (« Sony ILCE-7M3 »).
    pub fn camera(&self) -> String {
        if self.model.starts_with(&self.make) {
            self.model.clone()
        } else {
            format!("{} {}", self.make, self.model).trim().to_owned()
        }
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

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    entries: Vec<Entry>,
    /// Absentes des catalogues d'avant les collections.
    #[serde(default)]
    collections: Vec<Collection>,
}

/// Le fichier du catalogue, dans le dossier de LÜMA : sur Mac
/// `~/Library/Application Support/LÜMA/Catalogue.json`, sur iPhone et iPad
/// `Library/Application Support/Catalogue.json` (hors de l'app Fichiers), ailleurs
/// `~/.local/share/luma/catalogue.json`. `LUMA_CATALOG` le remplace.
pub fn default_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LUMA_CATALOG") {
        return Some(PathBuf::from(p));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    Some(if cfg!(target_os = "ios") {
        home.join("Library/Application Support/Catalogue.json")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support/LÜMA/Catalogue.json")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map_or_else(|| home.join(".local/share"), PathBuf::from)
            .join("luma/catalogue.json")
    })
}

impl Catalog {
    /// Lit un catalogue ; vide s'il n'existe pas, est illisible ou d'une autre version.
    pub fn load(path: &Path) -> Catalog {
        let stored: Option<Stored> = std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let Some(stored) = stored.filter(|s| s.version == VERSION) else {
            return Catalog::default();
        };
        Catalog {
            entries: stored.entries.into_iter().map(|e| (e.path.clone(), e)).collect(),
            collections: stored.collections,
        }
    }

    /// Enregistre le catalogue. L'écriture passe par un fichier temporaire renommé : un
    /// arrêt en plein milieu laisse l'ancien index intact.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let stored = Stored {
            version: VERSION,
            entries: self.entries.values().cloned().collect(),
            collections: self.collections.clone(),
        };
        let bytes = serde_json::to_vec(&stored).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, bytes)?;
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

    /// Les mots‑clés du catalogue et leur nombre de photos, par ordre alphabétique
    /// sans tenir compte des majuscules ni des accents (« été » et « Été » ne font
    /// qu'un, rangé à « e »).
    pub fn keywords(&self) -> Vec<(String, usize)> {
        facet(self.entries.values().flat_map(|e| e.keywords.iter().cloned()))
    }

    /// Les boîtiers du catalogue et leur nombre de photos.
    pub fn cameras(&self) -> Vec<(String, usize)> {
        facet(self.entries.values().map(Entry::camera).filter(|c| !c.is_empty()))
    }

    /// Les objectifs du catalogue et leur nombre de photos.
    pub fn lenses(&self) -> Vec<(String, usize)> {
        facet(self.entries.values().map(|e| e.lens.clone()).filter(|l| !l.is_empty()))
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
            .filter(|p| !p.is_file())
            .cloned()
            .collect();
        for p in gone {
            self.entries.remove(&p);
            update.removed += 1;
        }

        let stamps: Vec<(PathBuf, u64, u64, u64)> = files
            .into_iter()
            .map(|p| {
                let (size, modified) = stamp(&p);
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

    /// Relit le XMP d'une photo (après un changement de note ou de mots‑clés dans l'app).
    pub fn refresh_meta(&mut self, path: &Path) {
        if let Some(e) = self.entries.get_mut(path) {
            let side = xmp::sidecar_path(path);
            e.sidecar = stamp(&side).1;
            e.set_meta(read_meta(&side));
        }
    }
}

/// Compte des valeurs, regroupées sans tenir compte des majuscules ni des accents ;
/// chaque groupe garde la première graphie rencontrée.
fn facet(values: impl Iterator<Item = String>) -> Vec<(String, usize)> {
    let mut groups: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for v in values {
        groups.entry(query::fold(&v)).or_insert_with(|| (v, 0)).1 += 1;
    }
    groups.into_values().collect()
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
        }
    }
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

fn read_entry(path: PathBuf, size: u64, modified: u64, sidecar: u64) -> Entry {
    let info = luma_raw::info(&path).unwrap_or_default();
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
        ..Entry::default()
    };
    if sidecar != 0 {
        e.set_meta(read_meta(&xmp::sidecar_path(&path)));
    }
    e.path = path;
    e
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
}
