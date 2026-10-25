//! Choisir un fichier ou un dossier. Sur Mac (et Linux), les fenêtres du système ; sur
//! iPhone et iPad, l'app travaille dans son dossier Documents, que l'app Fichiers
//! remplit (Sur mon iPhone › LÜMA).

use std::path::PathBuf;

/// Le dossier Documents de l'app sur iOS (créé au besoin).
pub fn documents() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("HOME")?).join("Documents");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Un RAW à ouvrir.
#[cfg(not(target_os = "ios"))]
pub fn pick_raw() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Ouvrir un RAW")
        .add_filter("RAW", &crate::app::RAW_EXTENSIONS)
        .pick_file()
}

/// Sur iOS : le dossier Documents, relu (de nouvelles photos ont pu y arriver).
#[cfg(target_os = "ios")]
pub fn pick_raw() -> Option<PathBuf> {
    documents()
}

/// Un dossier de destination ; `None` s'il n'y a pas de sélecteur (iOS).
#[cfg(not(target_os = "ios"))]
pub fn pick_folder(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).pick_folder()
}

#[cfg(target_os = "ios")]
pub fn pick_folder(_title: &str) -> Option<PathBuf> {
    None
}

/// Le système propose‑t‑il un sélecteur de dossier ?
pub const FOLDER_PICKER: bool = !cfg!(target_os = "ios");
