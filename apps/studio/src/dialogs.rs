//! Choisir un fichier ou un dossier. Sur Mac (et Linux), les fenêtres du système ; sur
//! iPhone et iPad, l'app travaille dans son dossier Documents, que l'app Fichiers
//! remplit (Sur mon iPhone › LÜMA).

use std::path::PathBuf;

#[cfg(not(target_os = "ios"))]
use crate::i18n::tr;

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
        .set_title(tr("Ouvrir une photo"))
        .add_filter(tr("Photos"), &crate::app::photo_extensions())
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

/// Le dossier des films mesurés et des LUT importées (`.cube`), créé au besoin : sur Mac
/// `~/Library/Application Support/LÜMA/Films`, sur iPhone et iPad `Documents/Films`
/// (visible dans l'app Fichiers). `LUMA_FILMS` le remplace.
pub fn films_dir() -> Option<PathBuf> {
    let dir = if let Some(d) = std::env::var_os("LUMA_FILMS") {
        PathBuf::from(d)
    } else if cfg!(target_os = "ios") {
        documents()?.join("Films")
    } else if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support/LÜMA/Films")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".local/share")))?
            .join("luma/films")
    };
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Le dossier de LÜMA, au‑dessus de Films : celui de `licence.json`, de `langue.txt`…
pub fn app_dir() -> Option<PathBuf> {
    films_dir()?.parent().map(PathBuf::from)
}

/// Une LUT 3D à importer.
#[cfg(not(target_os = "ios"))]
pub fn pick_lut() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(tr("Importer une LUT 3D"))
        .add_filter(tr("LUT 3D"), &["cube", "CUBE"])
        .pick_file()
}

#[cfg(target_os = "ios")]
pub fn pick_lut() -> Option<PathBuf> {
    None
}

/// Le scan d'une pellicule, pour la mesurer.
#[cfg(not(target_os = "ios"))]
pub fn pick_scan() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(tr("Le scan de la même scène sur pellicule"))
        .add_filter(
            tr("Scan"),
            &[
                "jpg", "jpeg", "png", "tif", "tiff", "JPG", "JPEG", "PNG", "TIF", "TIFF",
            ],
        )
        .pick_file()
}

#[cfg(target_os = "ios")]
pub fn pick_scan() -> Option<PathBuf> {
    None
}

/// Le dossier de mes préréglages (`.xmp`), à côté du dossier Films : sur Mac
/// `~/Library/Application Support/LÜMA/Préréglages`, sur iPhone et iPad
/// `Documents/Préréglages`. `LUMA_PRESETS` le remplace.
pub fn presets_dir() -> Option<PathBuf> {
    let dir = match std::env::var_os("LUMA_PRESETS") {
        Some(d) => PathBuf::from(d),
        None => films_dir()?
            .parent()?
            .join(if cfg!(any(target_os = "macos", target_os = "ios")) {
                "Préréglages"
            } else {
                "presets"
            }),
    };
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Où enregistrer un préréglage exporté. Sur iPhone et iPad : `Documents/Préréglages
/// exportés`.
#[cfg(not(target_os = "ios"))]
pub fn save_preset(name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(tr("Exporter le préréglage"))
        .set_file_name(format!("{name}.xmp"))
        .add_filter(tr("Préréglage Lightroom"), &["xmp"])
        .save_file()
}

#[cfg(target_os = "ios")]
pub fn save_preset(name: &str) -> Option<PathBuf> {
    Some(export_folder()?.join(format!("{name}.xmp")))
}

/// Le dossier où exporter tous les préréglages.
#[cfg(not(target_os = "ios"))]
pub fn pick_export_folder() -> Option<PathBuf> {
    pick_folder(tr("Exporter les préréglages dans…"))
}

#[cfg(target_os = "ios")]
pub fn pick_export_folder() -> Option<PathBuf> {
    export_folder()
}

#[cfg(target_os = "ios")]
fn export_folder() -> Option<PathBuf> {
    let dir = documents()?.join("Préréglages exportés");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Des préréglages `.xmp` à importer (de Lightroom, Camera Raw ou LÜMA).
#[cfg(not(target_os = "ios"))]
pub fn pick_presets() -> Vec<PathBuf> {
    rfd::FileDialog::new()
        .set_title(tr("Importer des préréglages"))
        .add_filter(tr("Préréglages"), &["xmp", "XMP"])
        .pick_files()
        .unwrap_or_default()
}

#[cfg(target_os = "ios")]
pub fn pick_presets() -> Vec<PathBuf> {
    Vec::new()
}
