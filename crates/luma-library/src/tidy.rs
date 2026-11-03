//! Ranger une photo à l'écart, sans rien effacer : la photo, son XMP et ceux de ses
//! copies virtuelles déplacés dans un sous‑dossier de son dossier (« Rejetées » dans
//! l'app et `luma tidy`).

use std::path::{Path, PathBuf};

/// Déplace `photo` dans le sous‑dossier `folder` de son dossier, avec son XMP et ceux
/// de ses copies virtuelles. Rien n'est écrasé : si une photo ou un XMP du même nom y
/// est déjà (celui d'une autre photo rangée avant, `IMG.ARW` puis `IMG.DNG`), la photo
/// reste où elle est et la réponse est `None`. Sinon, les chemins d'origine qui ont
/// quitté le dossier : la photo puis ses copies virtuelles.
pub fn move_aside(photo: &Path, folder: &str) -> Option<Vec<PathBuf>> {
    let (dir, name) = (photo.parent()?, photo.file_name()?);
    let target_dir = dir.join(folder);
    let target = target_dir.join(name);
    let copies = luma_project::copies::of(photo);
    // Les XMP suivent, sous leur nom à côté de la photo rangée.
    let mut sidecars = vec![luma_project::xmp::sidecar_path(photo)];
    sidecars.extend(copies.iter().map(|c| luma_project::copies::sidecar(c)));
    let sidecars: Vec<(PathBuf, PathBuf)> = sidecars
        .into_iter()
        .filter(|x| x.exists())
        .filter_map(|x| {
            let to = target_dir.join(x.file_name()?);
            Some((x, to))
        })
        .collect();
    if target.exists() || sidecars.iter().any(|(_, to)| to.exists()) {
        return None;
    }
    std::fs::create_dir_all(&target_dir).ok()?;
    std::fs::rename(photo, &target).ok()?;
    for (x, to) in sidecars {
        let _ = std::fs::rename(&x, &to);
    }
    let mut moved = vec![photo.to_path_buf()];
    moved.extend(copies);
    Some(moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La photo et son XMP rangés ; une seconde photo au même XMP (même nom sans
    /// l'extension) n'écrase rien et reste.
    #[test]
    fn a_photo_and_its_sidecar_move_aside_without_overwriting() {
        let dir = std::env::temp_dir().join(format!("luma-ranger-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (arw, dng) = (dir.join("IMG.ARW"), dir.join("IMG.DNG"));
        for p in [&arw, &dng] {
            std::fs::write(p, b"photo").unwrap();
        }
        let xmp = luma_project::xmp::sidecar_path(&arw);
        std::fs::write(&xmp, b"<x/>").unwrap();
        assert_eq!(move_aside(&arw, "Rejetées"), Some(vec![arw.clone()]));
        let aside = dir.join("Rejetées");
        assert!(aside.join("IMG.ARW").is_file() && !arw.exists());
        assert!(aside.join(xmp.file_name().unwrap()).is_file() && !xmp.exists());
        // Le DNG a le même XMP que l'ARW (s'il est nommé sans l'extension) : il reste.
        std::fs::write(luma_project::xmp::sidecar_path(&dng), b"<y/>").unwrap();
        if luma_project::xmp::sidecar_path(&dng).file_name() == xmp.file_name() {
            assert_eq!(move_aside(&dng, "Rejetées"), None);
            assert!(dng.is_file());
        }
        // Une photo du même nom déjà rangée : rien n'est écrasé.
        std::fs::write(&arw, b"autre").unwrap();
        assert_eq!(move_aside(&arw, "Rejetées"), None);
        assert_eq!(std::fs::read(aside.join("IMG.ARW")).unwrap(), b"photo");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
