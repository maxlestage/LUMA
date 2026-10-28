//! L'import depuis une carte mémoire (ou tout dossier) : les photos sont copiées dans la
//! photothèque, rangées en sous‑dossiers d'après leur date de prise de vue, renommées
//! si on le veut, et copiées aussi sur un second disque. La carte n'est jamais
//! modifiée.
//!
//! Chaque copie est vérifiée : écrite à côté sous un nom provisoire, relue et comparée
//! octet pour octet à l'original, puis renommée. Une photo déjà importée (même nom, même
//! taille à sa place) est sautée ; un nom déjà pris par un autre fichier reçoit `-2`,
//! `-3`… Le RAW et le JPEG d'une même prise, et leur XMP, gardent un même nom.

use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::Entry;

/// Les cartes mémoire montées : les volumes qui ont un dossier `DCIM` (la norme des
/// appareils photo). Sur Mac `/Volumes/…`, ailleurs `/media/…` et `/run/media/…`.
pub fn cards() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/Volumes")];
    if let Some(user) = std::env::var_os("USER") {
        roots.push(Path::new("/media").join(&user));
        roots.push(Path::new("/run/media").join(&user));
    }
    roots.push(PathBuf::from("/media"));
    let mut out: Vec<PathBuf> = roots
        .iter()
        .filter_map(|r| std::fs::read_dir(r).ok())
        .flat_map(|d| d.flatten().map(|e| e.path()))
        .filter(|v| v.join("DCIM").is_dir())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Éjecte la carte montée en `mount`, une fois l'import fini : sur Mac par `diskutil` ;
/// sous Linux par `gio` (le bureau), sinon `umount`. Le message du système en cas
/// d'échec (une app qui lit encore la carte, un dossier qui n'est pas une carte).
pub fn eject(mount: &Path) -> Result<(), String> {
    let tries: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("diskutil", &["eject"])]
    } else if cfg!(target_os = "linux") {
        &[("gio", &["mount", "--eject"]), ("umount", &[])]
    } else {
        &[]
    };
    let mut last = String::from("éjection non prise en charge ici");
    for (program, args) in tries {
        match std::process::Command::new(program).args(*args).arg(mount).output() {
            Ok(out) if out.status.success() => return Ok(()),
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stderr).trim().to_owned();
                last = if text.is_empty() {
                    format!("{program} : échec ({})", out.status)
                } else {
                    text
                };
            }
            // Ce programme n'est pas là : le suivant.
            Err(_) => {}
        }
    }
    Err(last)
}

/// Les photos d'un dossier et de ses sous‑dossiers (les fichiers cachés exceptés).
pub fn photos(source: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![source.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if luma_raw::is_photo(&path) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Où et comment importer.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// Le dossier de la photothèque.
    pub dest: PathBuf,
    /// Les sous‑dossiers, d'après la date de prise de vue : `{année}`, `{mois}`,
    /// `{jour}` (par exemple « {année}/{année}-{mois}-{jour} ») ; vide : tout dans `dest`.
    pub folders: String,
    /// Le second disque, où chaque photo est copiée aussi (même rangement).
    pub backup: Option<PathBuf>,
    /// Seulement les photos de ces jours de prise de vue (`AAAA-MM-JJ`) ; vide : toutes.
    pub days: Vec<String>,
    /// Les photos laissées sur la carte, chacune avec les fichiers de sa prise (son
    /// JPEG, son RAW).
    pub skip: Vec<PathBuf>,
}

/// Une photo à copier.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub source: PathBuf,
    pub target: PathBuf,
    pub backup: Option<PathBuf>,
    /// Le XMP de la photo sur la carte, s'il y en a un.
    pub sidecar: Option<PathBuf>,
    pub size: u64,
}

/// Le plan de l'import : les photos à copier, celles déjà importées.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub items: Vec<Item>,
    pub skipped: Vec<PathBuf>,
}

impl Plan {
    pub fn bytes(&self) -> u64 {
        self.items.iter().map(|i| i.size).sum()
    }
}

/// Le sous‑dossier d'une photo prise à cette date (`AAAA-MM-JJ…`).
pub fn folder(template: &str, date: &str) -> PathBuf {
    let part = |r: std::ops::Range<usize>, default: &str| date.get(r).unwrap_or(default).to_owned();
    let text = template
        .replace("{année}", &part(0..4, "0000"))
        .replace("{annee}", &part(0..4, "0000"))
        .replace("{mois}", &part(5..7, "00"))
        .replace("{jour}", &part(8..10, "00"));
    text.split('/')
        .map(|s| s.trim().replace(['\\', ':'], "-"))
        .filter(|s| !s.is_empty() && s != "." && s != "..")
        .collect()
}

/// Le plan : les photos (lues dans leurs en‑têtes, `entries`) dans l'ordre de prise de
/// vue, leur place, et leur nom donné par `name` (le nouveau nom sans extension, pour la
/// photo et le rang de sa prise ; `None` : son nom d'origine).
/// Le jour de prise de vue d'une photo (`AAAA-MM-JJ`).
fn day(e: &Entry) -> &str {
    e.date.get(..10).unwrap_or(&e.date)
}

/// Les jours de prise de vue des photos, dans l'ordre, et le nombre de photos de chacun :
/// de quoi n'en importer que certains.
pub fn days(entries: &[Entry]) -> Vec<(String, usize)> {
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    for e in entries {
        *count.entry(day(e).to_owned()).or_default() += 1;
    }
    count.into_iter().collect()
}

/// La prise d'un fichier : son dossier et son nom sans extension (le RAW et son JPEG
/// en ont une seule).
pub fn shot(path: &Path) -> (Option<PathBuf>, Option<std::ffi::OsString>) {
    (
        path.parent().map(Path::to_path_buf),
        path.file_stem().map(|s| s.to_os_string()),
    )
}

pub fn plan(entries: &[Entry], o: &Options, name: impl Fn(&Entry, usize) -> Option<String>) -> Plan {
    // Les prises laissées sur la carte, comparées en chemins absolus (`--skip` peut en
    // donner un relatif).
    let absolute = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let skipped: HashSet<_> = o.skip.iter().map(|p| shot(&absolute(p))).collect();
    let mut sorted: Vec<&Entry> = entries
        .iter()
        .filter(|e| o.days.is_empty() || o.days.iter().any(|d| d == day(e)))
        .filter(|e| skipped.is_empty() || !skipped.contains(&shot(&absolute(&e.path))))
        .collect();
    sorted.sort_by(|a, b| (&a.date, &a.path).cmp(&(&b.date, &b.path)));
    // Les prises : même dossier, même nom sans extension (le RAW et son JPEG).
    let shot = |e: &Entry| shot(&e.path);
    let mut shots: BTreeMap<(Option<PathBuf>, Option<std::ffi::OsString>), String> = BTreeMap::new();
    let mut taken: HashSet<PathBuf> = HashSet::new();
    let mut plan = Plan::default();
    for e in sorted {
        let key = shot(e);
        let n = shots.len() + 1;
        let stem = match shots.get(&key) {
            Some(s) => s.clone(),
            None => {
                let original = e
                    .path
                    .file_stem()
                    .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
                let s = name(e, n).filter(|s| !s.trim().is_empty()).unwrap_or(original);
                shots.insert(key, s.clone());
                s
            }
        };
        let ext = e
            .path
            .extension()
            .map_or_else(String::new, |x| format!(".{}", x.to_string_lossy()));
        let dir = o.dest.join(folder(&o.folders, &e.date));
        let size = std::fs::metadata(&e.path).map_or(e.size, |m| m.len());
        let mut target = dir.join(format!("{stem}{ext}"));
        // Déjà là, de la même taille : déjà importée.
        if std::fs::metadata(&target).is_ok_and(|m| m.len() == size) && !taken.contains(&target) {
            plan.skipped.push(e.path.clone());
            continue;
        }
        // Le second disque aussi : un fichier du même nom n'y est jamais remplacé.
        let on_backup = |t: &Path| {
            o.backup
                .as_ref()
                .is_some_and(|b| b.join(t.strip_prefix(&o.dest).unwrap_or(t)).exists())
        };
        let mut k = 2;
        while target.exists() || taken.contains(&target) || on_backup(&target) {
            target = dir.join(format!("{stem}-{k}{ext}"));
            k += 1;
        }
        taken.insert(target.clone());
        let backup = o.backup.as_ref().map(|b| {
            let rel = target.strip_prefix(&o.dest).unwrap_or(&target);
            b.join(rel)
        });
        let side = luma_project::xmp::sidecar_path(&e.path);
        plan.items.push(Item {
            source: e.path.clone(),
            target,
            backup,
            sidecar: side.is_file().then_some(side),
            size,
        });
    }
    plan
}

/// Les deux fichiers sont‑ils identiques, octet pour octet ?
fn same(a: &Path, b: &Path) -> std::io::Result<bool> {
    let (mut fa, mut fb) = (std::fs::File::open(a)?, std::fs::File::open(b)?);
    if fa.metadata()?.len() != fb.metadata()?.len() {
        return Ok(false);
    }
    let (mut ba, mut bb) = (vec![0u8; 1 << 20], vec![0u8; 1 << 20]);
    loop {
        let n = fa.read(&mut ba)?;
        if n == 0 {
            return Ok(true);
        }
        fb.read_exact(&mut bb[..n])?;
        if ba[..n] != bb[..n] {
            return Ok(false);
        }
    }
}

/// Copie `from` en `to`, vérifiée : sous un nom provisoire, relue, puis renommée.
pub fn copy_verified(from: &Path, to: &Path) -> Result<(), String> {
    let err = |e: std::io::Error| format!("{} : {e}", to.display());
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    let tmp = to.with_file_name(format!(
        ".{}.luma-import",
        to.file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    ));
    std::fs::copy(from, &tmp).map_err(err)?;
    if !same(from, &tmp).map_err(err)? {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{} : la copie ne correspond pas à l'original", to.display()));
    }
    std::fs::rename(&tmp, to).map_err(err)
}

/// Le XMP d'une photo copiée : à côté d'elle, sous son nouveau nom, dans la forme de
/// l'original (`IMG.CR3.xmp` ou `IMG.xmp`). Si ce nom est déjà pris (le XMP d'une autre
/// photo du même nom), la forme longue, propre à la photo ; `None` si elle l'est aussi,
/// ou si le même XMP y est déjà (celui d'une prise, partagé par le RAW et son JPEG) :
/// un XMP déjà là n'est jamais écrasé.
fn sidecar_target(source: &Path, sidecar: &Path, photo: &Path) -> Option<PathBuf> {
    let long = PathBuf::from(format!("{}.xmp", photo.display()));
    let short = photo.with_extension("xmp");
    let first = if *sidecar == source.with_extension("xmp") {
        short
    } else {
        long.clone()
    };
    for p in [first, long] {
        if !p.exists() {
            return Some(p);
        }
        if same(sidecar, &p).unwrap_or(false) {
            return None;
        }
    }
    None
}

/// Ce que l'import a fait.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub copied: usize,
    pub bytes: u64,
}

/// Copie les photos du plan (et leur XMP), sur la photothèque puis sur le second
/// disque ; `progress` reçoit le nombre de photos copiées. S'arrête à la première
/// erreur : les photos déjà copiées restent, la carte n'est jamais touchée.
pub fn run(plan: &Plan, progress: impl Fn(usize)) -> Result<Report, String> {
    let mut report = Report::default();
    for item in &plan.items {
        for to in std::iter::once(&item.target).chain(item.backup.as_ref()) {
            copy_verified(&item.source, to)?;
            if let Some(x) = &item.sidecar
                && let Some(to) = sidecar_target(&item.source, x, to)
            {
                copy_verified(x, &to)?;
            }
        }
        report.copied += 1;
        report.bytes += item.size;
        progress(report.copied);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("luma-import-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn entry(path: &Path, date: &str) -> Entry {
        Entry {
            path: path.to_path_buf(),
            date: date.into(),
            size: std::fs::metadata(path).map_or(0, |m| m.len()),
            ..Entry::default()
        }
    }

    #[test]
    fn only_the_chosen_days() {
        let root = std::env::temp_dir().join(format!("luma-import-jours-{}", std::process::id()));
        let entries = vec![
            entry(&root.join("IMG_0001.CR3"), "2026-10-26T09:00:00"),
            entry(&root.join("IMG_0001.JPG"), "2026-10-26T09:00:00"),
            entry(&root.join("IMG_0002.CR3"), "2026-10-27T12:00:00"),
            entry(&root.join("IMG_0003.CR3"), "2026-10-28T08:30:00"),
        ];
        assert_eq!(
            days(&entries),
            [
                ("2026-10-26".to_owned(), 2),
                ("2026-10-27".to_owned(), 1),
                ("2026-10-28".to_owned(), 1)
            ]
        );
        let mut o = Options {
            dest: root.join("Photos"),
            folders: String::new(),
            backup: None,
            days: Vec::new(),
            skip: Vec::new(),
        };
        assert_eq!(plan(&entries, &o, |_, _| None).items.len(), 4, "tous les jours");
        o.days = vec!["2026-10-26".into(), "2026-10-28".into()];
        let names: Vec<String> = plan(&entries, &o, |_, _| None)
            .items
            .iter()
            .map(|i| i.source.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["IMG_0001.CR3", "IMG_0001.JPG", "IMG_0003.CR3"]);
    }

    #[test]
    fn a_folder_that_is_not_a_card_is_not_ejected() {
        let dir = std::env::temp_dir().join(format!("luma-ejecter-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("DCIM")).unwrap();
        let e = eject(&dir).unwrap_err();
        assert!(!e.is_empty());
        assert!(dir.join("DCIM").is_dir(), "le dossier reste");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn photos_left_on_the_card() {
        let root = std::env::temp_dir().join(format!("luma-import-laisser-{}", std::process::id()));
        let entries = vec![
            entry(&root.join("IMG_0001.CR3"), "2026-10-26T09:00:00"),
            entry(&root.join("IMG_0001.JPG"), "2026-10-26T09:00:00"),
            entry(&root.join("IMG_0002.CR3"), "2026-10-26T09:01:00"),
            entry(&root.join("IMG_0003.CR3"), "2026-10-27T08:30:00"),
        ];
        let o = Options {
            dest: root.join("Photos"),
            folders: String::new(),
            backup: None,
            days: vec!["2026-10-26".into()],
            // Le JPEG désigne sa prise : le RAW reste aussi sur la carte.
            skip: vec![root.join("IMG_0001.JPG")],
        };
        let names: Vec<String> = plan(&entries, &o, |_, _| None)
            .items
            .iter()
            .map(|i| i.source.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["IMG_0002.CR3"]);
    }

    #[test]
    fn folders_from_the_date() {
        assert_eq!(
            folder("{année}/{année}-{mois}-{jour}", "2026-10-27T10:00:00"),
            Path::new("2026/2026-10-27")
        );
        assert_eq!(folder("", "2026-10-27T10:00:00"), PathBuf::new());
        assert_eq!(
            folder("../{annee}/./x:y", "2024-01-02"),
            Path::new("2024/x-y"),
            "jamais hors de la photothèque"
        );
    }

    #[test]
    fn a_card_imported_twice_and_to_a_second_disk() {
        let root = dir("carte");
        let card = root.join("CARTE/DCIM/100CANON");
        std::fs::create_dir_all(&card).unwrap();
        std::fs::write(card.join("IMG_0001.CR3"), b"raw un").unwrap();
        std::fs::write(card.join("IMG_0001.JPG"), b"jpeg un").unwrap();
        std::fs::write(card.join("IMG_0001.xmp"), b"<x:xmpmeta/>").unwrap();
        std::fs::write(card.join("IMG_0002.CR3"), b"raw deux, plus long").unwrap();
        std::fs::write(card.join(".cache.CR3"), b"cache").unwrap();
        std::fs::write(card.join("notes.txt"), b"texte").unwrap();
        let found = photos(&root.join("CARTE"));
        assert_eq!(found.len(), 3, "{found:?}");
        let entries = vec![
            entry(&card.join("IMG_0002.CR3"), "2026-10-27T12:00:00"),
            entry(&card.join("IMG_0001.CR3"), "2026-10-26T09:00:00"),
            entry(&card.join("IMG_0001.JPG"), "2026-10-26T09:00:00"),
        ];
        let o = Options {
            dest: root.join("Photos"),
            folders: "{année}/{année}-{mois}-{jour}".into(),
            backup: Some(root.join("Second")),
            days: Vec::new(),
            skip: Vec::new(),
        };
        // Renommées dans l'ordre de prise de vue ; le RAW et son JPEG ensemble.
        let name = |e: &Entry, n: usize| Some(format!("{}_{n:03}", &e.date[..10]));
        let p = plan(&entries, &o, name);
        let targets: Vec<String> = p
            .items
            .iter()
            .map(|i| i.target.strip_prefix(&o.dest).unwrap().display().to_string())
            .collect();
        assert_eq!(
            targets,
            [
                "2026/2026-10-26/2026-10-26_001.CR3",
                "2026/2026-10-26/2026-10-26_001.JPG",
                "2026/2026-10-27/2026-10-27_002.CR3",
            ]
        );
        assert!(p.items[0].sidecar.is_some() && p.items[2].sidecar.is_none());
        let seen = std::cell::Cell::new(0);
        let r = run(&p, |n| seen.set(n)).unwrap();
        assert_eq!((r.copied, seen.get()), (3, 3));
        for base in [&o.dest, o.backup.as_ref().unwrap()] {
            let raw = base.join("2026/2026-10-26/2026-10-26_001.CR3");
            assert_eq!(std::fs::read(&raw).unwrap(), b"raw un");
            assert!(raw.with_extension("xmp").is_file(), "le XMP suit");
        }
        // La carte n'a pas bougé.
        assert!(card.join("IMG_0001.CR3").is_file() && card.join("IMG_0002.CR3").is_file());
        // Une seconde fois : rien de nouveau.
        let again = plan(&entries, &o, name);
        assert!(again.items.is_empty() && again.skipped.len() == 3, "{again:?}");
        // Une autre photo du même nom : elle ne remplace rien.
        std::fs::write(card.join("IMG_0002.CR3"), b"une autre photo").unwrap();
        let other = plan(
            &[entry(&card.join("IMG_0002.CR3"), "2026-10-27T12:00:00")],
            &o,
            |_, _| None,
        );
        assert!(other.items[0].target.ends_with("2026/2026-10-27/IMG_0002.CR3"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Les XMP d'une prise (forme longue, un par photo) arrivent chacun sous son nom ; un
    /// XMP de la photothèque, et un fichier du second disque, ne sont jamais remplacés.
    #[test]
    fn sidecars_and_the_second_disk_are_never_overwritten() {
        let root = dir("xmp");
        let card = root.join("CARTE");
        std::fs::create_dir_all(&card).unwrap();
        std::fs::write(card.join("IMG_1.CR3"), b"raw").unwrap();
        std::fs::write(card.join("IMG_1.JPG"), b"jpeg").unwrap();
        std::fs::write(card.join("IMG_1.CR3.xmp"), b"reglages du raw").unwrap();
        std::fs::write(card.join("IMG_1.JPG.xmp"), b"reglages du jpeg").unwrap();
        let o = Options {
            dest: root.join("Photos"),
            folders: String::new(),
            backup: Some(root.join("Second")),
            days: Vec::new(),
            skip: Vec::new(),
        };
        std::fs::create_dir_all(&o.dest).unwrap();
        std::fs::write(o.dest.join("IMG_1.xmp"), b"reglages de la phototheque").unwrap();
        std::fs::create_dir_all(o.backup.as_ref().unwrap()).unwrap();
        std::fs::write(root.join("Second/IMG_1.CR3"), b"autre photo du second disque").unwrap();
        let entries = [
            entry(&card.join("IMG_1.CR3"), "2026-10-26T09:00:00"),
            entry(&card.join("IMG_1.JPG"), "2026-10-26T09:00:00"),
        ];
        let p = plan(&entries, &o, |_, _| None);
        assert!(p.items[0].target.ends_with("IMG_1-2.CR3"), "{:?}", p.items[0].target);
        run(&p, |_| {}).unwrap();
        let read = |p: &str| std::fs::read(o.dest.join(p)).unwrap();
        assert_eq!(read("IMG_1.xmp"), b"reglages de la phototheque");
        assert_eq!(read("IMG_1-2.CR3.xmp"), b"reglages du raw");
        assert_eq!(read("IMG_1.JPG.xmp"), b"reglages du jpeg");
        assert_eq!(
            std::fs::read(root.join("Second/IMG_1.CR3")).unwrap(),
            b"autre photo du second disque"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_name_already_taken_gets_a_number() {
        let root = dir("pris");
        std::fs::create_dir_all(root.join("Photos")).unwrap();
        std::fs::write(root.join("Photos/A.NEF"), b"autre contenu, autre taille").unwrap();
        std::fs::write(root.join("A.NEF"), b"nouvelle").unwrap();
        let o = Options {
            dest: root.join("Photos"),
            folders: String::new(),
            backup: None,
            days: Vec::new(),
            skip: Vec::new(),
        };
        let p = plan(&[entry(&root.join("A.NEF"), "2026-01-01T00:00:00")], &o, |_, _| None);
        assert_eq!(p.items[0].target, root.join("Photos/A-2.NEF"));
        run(&p, |_| {}).unwrap();
        assert_eq!(
            std::fs::read(root.join("Photos/A.NEF")).unwrap(),
            b"autre contenu, autre taille"
        );
        // Aucun fichier provisoire ne reste.
        let left: Vec<_> = std::fs::read_dir(root.join("Photos")).unwrap().flatten().collect();
        assert_eq!(left.len(), 2);
        let _ = std::fs::remove_dir_all(&root);
    }
}
