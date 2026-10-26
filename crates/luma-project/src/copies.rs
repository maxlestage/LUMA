//! Les copies virtuelles : plusieurs développements d'un même fichier, sans le
//! dupliquer.
//!
//! Une copie n'est qu'un jeu de réglages de plus, dans un XMP posé à côté de la
//! photo : `IMG_0001.ARW.copie-2.xmp`. Dans LÜMA, elle a pour chemin celui de la photo
//! suivi de `.copie-2` (un fichier qui n'existe pas) : partout où l'on lit les pixels,
//! ce chemin est ramené à l'original par [`source`] ; partout où l'on lit les réglages,
//! la note ou les mots‑clés, c'est le XMP de la copie qui compte.
//!
//! Supprimer une copie, c'est supprimer son XMP : l'original n'est jamais touché.

use std::path::{Path, PathBuf};

const MARK: &str = ".copie-";

/// Le numéro de la copie (2, 3…), ou `None` pour une photo ordinaire.
pub fn number(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    let (_, n) = name.rsplit_once(MARK)?;
    n.parse().ok().filter(|n| *n >= 2)
}

/// Le fichier dont la copie est faite (la photo elle‑même si ce n'est pas une copie).
pub fn source(path: &Path) -> PathBuf {
    match (number(path), path.file_name().and_then(|n| n.to_str())) {
        (Some(_), Some(name)) => {
            let (base, _) = name.rsplit_once(MARK).expect("numéro lu");
            path.with_file_name(base)
        }
        _ => path.to_path_buf(),
    }
}

/// Le chemin de la copie numéro `n` d'une photo.
pub fn path(photo: &Path, n: u32) -> PathBuf {
    let photo = source(photo);
    let name = photo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    photo.with_file_name(format!("{name}{MARK}{n}"))
}

/// Le XMP d'une copie, toujours au nom long (`IMG_0001.ARW.copie-2.xmp`).
pub fn sidecar(copy: &Path) -> PathBuf {
    PathBuf::from(format!("{}.xmp", copy.display()))
}

/// Les copies d'une photo qui ont un XMP, dans l'ordre de leur numéro.
pub fn of(photo: &Path) -> Vec<PathBuf> {
    let photo = source(photo);
    let (Some(dir), Some(name)) = (photo.parent(), photo.file_name().and_then(|n| n.to_str())) else {
        return Vec::new();
    };
    let prefix = format!("{name}{MARK}");
    let mut numbers: Vec<u32> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .filter_map(|e| {
                    let file = e.file_name();
                    let rest = file.to_str()?.strip_prefix(&prefix)?.strip_suffix(".xmp")?.to_owned();
                    rest.parse().ok().filter(|n| *n >= 2)
                })
                .collect()
        })
        .unwrap_or_default();
    numbers.sort_unstable();
    numbers.into_iter().map(|n| path(&photo, n)).collect()
}

/// Le chemin de la prochaine copie d'une photo : le premier numéro libre après les
/// copies existantes.
pub fn next(photo: &Path) -> PathBuf {
    let n = of(photo).iter().filter_map(|p| number(p)).max().map_or(2, |n| n + 1);
    path(photo, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let photo = Path::new("/photos/IMG_0001.ARW");
        let copy = path(photo, 2);
        assert_eq!(copy, Path::new("/photos/IMG_0001.ARW.copie-2"));
        assert_eq!(number(&copy), Some(2));
        assert_eq!(source(&copy), photo);
        assert_eq!(
            path(&copy, 3),
            Path::new("/photos/IMG_0001.ARW.copie-3"),
            "copie d'une copie"
        );
        assert_eq!(sidecar(&copy), Path::new("/photos/IMG_0001.ARW.copie-2.xmp"));
        assert_eq!(number(photo), None);
        assert_eq!(source(photo), photo);
        for not_a_copy in ["/a/b.copie-1", "/a/b.copie-x", "/a/b.copie-"] {
            assert_eq!(number(Path::new(not_a_copy)), None, "{not_a_copy}");
        }
    }

    #[test]
    fn copies_are_found_by_their_sidecar() {
        let dir = std::env::temp_dir().join(format!("luma-copies-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let photo = dir.join("IMG_0001.ARW");
        std::fs::write(&photo, b"").unwrap();
        assert!(of(&photo).is_empty());
        assert_eq!(next(&photo), path(&photo, 2));
        for n in [3, 2, 10] {
            std::fs::write(sidecar(&path(&photo, n)), "").unwrap();
        }
        // Les copies d'une autre photo, et un nom qui n'en est pas une, sont ignorés.
        std::fs::write(dir.join("IMG_0002.ARW.copie-2.xmp"), "").unwrap();
        std::fs::write(dir.join("IMG_0001.ARW.copie-1.xmp"), "").unwrap();
        assert_eq!(of(&photo), [path(&photo, 2), path(&photo, 3), path(&photo, 10)]);
        assert_eq!(next(&photo), path(&photo, 11));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
