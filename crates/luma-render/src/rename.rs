//! Le renommage par lot : des photos renommées d'après un modèle (les mêmes repères que
//! l'export : `{date}_{séquence}`, `{boîtier}-{nom}`…), avec tout ce qui les
//! accompagne.
//!
//! - Une photo et ses sœurs du même nom (`IMG_0001.ARW` et `IMG_0001.JPG`, prises
//!   ensemble) forment un groupe : elles gardent un même nom, et un seul numéro de
//!   séquence.
//! - Le XMP suit, sous ses deux formes (`IMG_0001.xmp` et `IMG_0001.ARW.xmp`), ainsi que
//!   les XMP des copies virtuelles (`IMG_0001.ARW.copie-2.xmp`).
//! - Un nom déjà pris par un autre fichier reçoit `-2`, `-3`… ; deux photos qui
//!   échangent leurs noms passent par des noms provisoires.
//! - Si un fichier ne peut pas être renommé, tout ce qui l'a déjà été revient à son nom
//!   d'origine : on n'est jamais laissé à mi‑chemin.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use luma_project::copies;

use crate::export::{NameFields, file_name};

/// Un fichier à renommer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Ce qu'un renommage fera : les photos (leurs chemins dans LÜMA, copies virtuelles
/// comprises) et tous les fichiers sur le disque.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub photos: Vec<Move>,
    pub files: Vec<Move>,
}

impl Plan {
    /// Le nouveau chemin d'une photo (elle‑même si elle ne change pas).
    pub fn renamed(&self, path: &Path) -> PathBuf {
        self.photos
            .iter()
            .find(|m| m.from == path)
            .map_or_else(|| path.to_path_buf(), |m| m.to.clone())
    }
}

/// Le nom d'un fichier sans son extension, et son extension.
fn split(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    (stem, ext)
}

fn with_ext(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    if ext.is_empty() {
        dir.join(stem)
    } else {
        dir.join(format!("{stem}.{ext}"))
    }
}

/// Les fichiers qui accompagnent une photo `dir/stem.ext` renommée en `new` : son XMP
/// long, ses copies virtuelles. (Le XMP court, partagé par le groupe, est traité à part.)
fn companions(photo: &Path, new: &Path) -> Vec<Move> {
    let mut out = Vec::new();
    let long = PathBuf::from(format!("{}.xmp", photo.display()));
    if long.is_file() {
        out.push(Move {
            from: long,
            to: PathBuf::from(format!("{}.xmp", new.display())),
        });
    }
    for copy in copies::of(photo) {
        if let Some(n) = copies::number(&copy) {
            out.push(Move {
                from: copies::sidecar(&copy),
                to: copies::sidecar(&copies::path(new, n)),
            });
        }
    }
    out
}

/// Prépare le renommage de `photos` (dans l'ordre donné, qui fixe la séquence) selon
/// `template`. Les copies virtuelles suivent leur original : les donner seules ne fait
/// rien. Un `/` du modèle devient un tiret (le fichier reste dans son dossier).
pub fn plan(photos: &[(PathBuf, NameFields)], template: &str) -> Result<Plan, String> {
    crate::export::check_template(template)?;
    if template.trim().is_empty() {
        return Err("modèle vide".into());
    }
    // Les groupes : même dossier, même nom sans extension ; dans l'ordre d'arrivée.
    let mut groups: Vec<((PathBuf, String), Vec<PathBuf>, NameFields)> = Vec::new();
    for (path, fields) in photos {
        // Les copies virtuelles suivent leur original ; un XMP suit sa photo.
        let sidecar = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("xmp"));
        if copies::number(path).is_some() || sidecar {
            continue;
        }
        let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
        let (stem, _) = split(path);
        let key = (dir, stem);
        match groups.iter_mut().find(|g| g.0 == key) {
            Some(g) => {
                if !g.1.contains(path) {
                    g.1.push(path.clone());
                }
            }
            None => groups.push((key, vec![path.clone()], fields.clone())),
        }
    }

    // Tout ce qui part : ces fichiers-là peuvent être remplacés par un nouveau nom.
    let mut leaving: HashSet<PathBuf> = HashSet::new();
    for ((dir, stem), files, _) in &groups {
        for f in files {
            leaving.insert(f.clone());
            for c in companions(f, f) {
                leaving.insert(c.from);
            }
        }
        let short = with_ext(dir, stem, "xmp");
        if short.is_file() {
            leaving.insert(short);
        }
    }

    let mut out = Plan::default();
    let mut taken: HashSet<PathBuf> = HashSet::new();
    for (k, ((dir, stem), files, fields)) in groups.iter().enumerate() {
        let fields = NameFields {
            sequence: k + 1,
            ..fields.clone()
        };
        let mut base = file_name(template, &fields).replace('/', "-");
        if base.is_empty() {
            base = stem.clone();
        }
        let short = with_ext(dir, stem, "xmp");
        let has_short = short.is_file();
        // Tous les fichiers du groupe sous le nouveau nom `name`.
        let targets = |name: &str| -> Vec<Move> {
            let mut moves = Vec::new();
            for f in files {
                let (_, ext) = split(f);
                let to = with_ext(dir, name, &ext);
                moves.push(Move {
                    from: f.clone(),
                    to: to.clone(),
                });
                moves.extend(companions(f, &to));
            }
            if has_short {
                moves.push(Move {
                    from: short.clone(),
                    to: with_ext(dir, name, "xmp"),
                });
            }
            moves
        };
        let free = |moves: &[Move]| {
            moves
                .iter()
                .all(|m| !taken.contains(&m.to) && (m.to == m.from || leaving.contains(&m.to) || !m.to.exists()))
        };
        let mut name = base.clone();
        let mut n = 2;
        let mut moves = targets(&name);
        while !free(&moves) {
            name = format!("{base}-{n}");
            moves = targets(&name);
            n += 1;
        }
        for m in &moves {
            taken.insert(m.to.clone());
        }
        if name == *stem {
            continue;
        }
        for f in files {
            let (_, ext) = split(f);
            let to = with_ext(dir, &name, &ext);
            for copy in copies::of(f) {
                if let Some(n) = copies::number(&copy) {
                    out.photos.push(Move {
                        from: copy,
                        to: copies::path(&to, n),
                    });
                }
            }
            out.photos.push(Move { from: f.clone(), to });
        }
        out.files.extend(moves.into_iter().filter(|m| m.from != m.to));
    }
    // Les photos dans l'ordre où on les a données.
    let order: BTreeMap<&Path, usize> = photos.iter().enumerate().map(|(i, (p, _))| (p.as_path(), i)).collect();
    out.photos.sort_by_key(|m| {
        order.get(m.from.as_path()).copied().unwrap_or_else(|| {
            order
                .get(copies::source(&m.from).as_path())
                .copied()
                .unwrap_or(usize::MAX)
        })
    });
    Ok(out)
}

/// Le nom provisoire d'un fichier pendant le renommage.
fn provisional(path: &Path, k: usize) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".luma-renommage-{k}-{name}"))
}

/// Renomme les fichiers du plan : d'abord tous vers des noms provisoires, puis vers
/// leurs nouveaux noms. Au premier échec, tout revient comme avant.
pub fn apply(plan: &Plan) -> Result<(), String> {
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
    let undo = |done: &[(PathBuf, PathBuf)]| {
        for (from, to) in done.iter().rev() {
            let _ = std::fs::rename(to, from);
        }
    };
    let steps: Vec<(PathBuf, PathBuf, PathBuf)> = plan
        .files
        .iter()
        .enumerate()
        .map(|(k, m)| (m.from.clone(), provisional(&m.from, k), m.to.clone()))
        .collect();
    for (from, tmp, _) in &steps {
        if let Err(e) = std::fs::rename(from, tmp) {
            undo(&done);
            return Err(format!("{} : {e}", from.display()));
        }
        done.push((from.clone(), tmp.clone()));
    }
    for (_, tmp, to) in &steps {
        if to.exists() {
            undo(&done);
            return Err(format!("{} existe déjà", to.display()));
        }
        if let Err(e) = std::fs::rename(tmp, to) {
            undo(&done);
            return Err(format!("{} : {e}", to.display()));
        }
        done.push((tmp.clone(), to.clone()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("luma-renommage-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn touch(p: &Path, text: &str) {
        std::fs::write(p, text).unwrap();
    }

    fn fields(name: &str, date: [u32; 6]) -> NameFields {
        NameFields {
            name: name.into(),
            date: Some(date),
            make: "SONY".into(),
            model: "ILCE-7M3".into(),
            ..NameFields::default()
        }
    }

    fn names(d: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    /// Les photos prennent le nom du modèle, dans l'ordre donné ; la paire RAW + JPEG
    /// garde un même nom et un seul numéro ; les XMP (court, long) et les copies
    /// virtuelles suivent.
    #[test]
    fn renames_photos_and_companions() {
        let d = dir("compagnons");
        for f in ["A.ARW", "A.JPG", "B.CR3"] {
            touch(&d.join(f), f);
        }
        touch(&d.join("A.xmp"), "xmp de A");
        touch(&d.join("B.CR3.xmp"), "xmp de B");
        touch(&d.join("B.CR3.copie-2.xmp"), "copie de B");
        let day = [2026, 10, 27, 9, 30, 0];
        let photos = vec![
            (d.join("A.ARW"), fields("A", day)),
            (d.join("A.JPG"), fields("A", day)),
            (d.join("B.CR3"), fields("B", day)),
            (d.join("B.CR3.copie-2"), fields("B-2", day)),
        ];
        let mut given = photos.clone();
        given.push((d.join("A.xmp"), fields("A", day)));
        assert_eq!(
            plan(&given, "{date}_{séquence:3}").unwrap(),
            plan(&photos, "{date}_{séquence:3}").unwrap()
        );
        let p = plan(&photos, "{date}_{séquence:3}").unwrap();
        assert_eq!(p.renamed(&d.join("A.JPG")), d.join("2026-10-27_001.JPG"));
        assert_eq!(p.renamed(&d.join("B.CR3")), d.join("2026-10-27_002.CR3"));
        assert_eq!(
            p.renamed(&d.join("B.CR3.copie-2")),
            d.join("2026-10-27_002.CR3.copie-2")
        );
        assert_eq!(p.photos.len(), 4);
        apply(&p).unwrap();
        assert_eq!(
            names(&d),
            [
                "2026-10-27_001.ARW",
                "2026-10-27_001.JPG",
                "2026-10-27_001.xmp",
                "2026-10-27_002.CR3",
                "2026-10-27_002.CR3.copie-2.xmp",
                "2026-10-27_002.CR3.xmp",
            ]
        );
        let read = |n: &str| std::fs::read_to_string(d.join(n)).unwrap();
        assert_eq!(read("2026-10-27_001.xmp"), "xmp de A");
        assert_eq!(read("2026-10-27_002.CR3.copie-2.xmp"), "copie de B");
        assert_eq!(read("2026-10-27_001.JPG"), "A.JPG");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Un nom déjà pris par un fichier qui reste reçoit -2 ; deux photos qui échangent
    /// leurs noms y arrivent ; une photo qui garde son nom n'est pas touchée.
    #[test]
    fn collisions_and_swaps() {
        let d = dir("collisions");
        for f in ["X.ARW", "Y.ARW", "Z.ARW", "autre.ARW"] {
            touch(&d.join(f), f);
        }
        let swap = vec![
            (d.join("X.ARW"), fields("Y", [2026, 1, 1, 0, 0, 0])),
            (d.join("Y.ARW"), fields("X", [2026, 1, 1, 0, 0, 0])),
            (d.join("Z.ARW"), fields("Z", [2026, 1, 1, 0, 0, 0])),
        ];
        let p = plan(&swap, "{nom}").unwrap();
        assert_eq!(p.files.len(), 2);
        apply(&p).unwrap();
        let read = |n: &str| std::fs::read_to_string(d.join(n)).unwrap();
        assert_eq!(read("X.ARW"), "Y.ARW");
        assert_eq!(read("Y.ARW"), "X.ARW");
        assert_eq!(read("Z.ARW"), "Z.ARW");

        let taken = vec![(d.join("Z.ARW"), fields("autre", [2026, 1, 1, 0, 0, 0]))];
        let p = plan(&taken, "{nom}").unwrap();
        assert_eq!(p.renamed(&d.join("Z.ARW")), d.join("autre-2.ARW"));
        // Le même nom pour deux photos : -2 pour la seconde.
        let same = vec![
            (d.join("X.ARW"), fields("x", [2026, 1, 1, 0, 0, 0])),
            (d.join("Y.ARW"), fields("y", [2026, 1, 1, 0, 0, 0])),
        ];
        let p = plan(&same, "{boîtier}").unwrap();
        assert_eq!(p.renamed(&d.join("X.ARW")), d.join("SONY ILCE-7M3.ARW"));
        assert_eq!(p.renamed(&d.join("Y.ARW")), d.join("SONY ILCE-7M3-2.ARW"));
        assert!(plan(&same, "{inconnu}").is_err());
        assert!(plan(&same, "  ").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Un échec en route (un fichier disparu) : tout revient à son nom d'origine.
    #[test]
    fn failure_rolls_back() {
        let d = dir("echec");
        for f in ["P.ARW", "Q.ARW"] {
            touch(&d.join(f), f);
        }
        let photos = vec![
            (d.join("P.ARW"), fields("P", [2026, 1, 1, 0, 0, 0])),
            (d.join("Q.ARW"), fields("Q", [2026, 1, 1, 0, 0, 0])),
        ];
        let p = plan(&photos, "n{séquence}").unwrap();
        std::fs::remove_file(d.join("Q.ARW")).unwrap();
        assert!(apply(&p).is_err());
        assert_eq!(names(&d), ["P.ARW"]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
