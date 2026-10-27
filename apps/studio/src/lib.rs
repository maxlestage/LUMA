//! LÜMA Studio : l'application de développement photo, sur le moteur de LÜMA.
//!
//! L'interface est dessinée par egui et affichée par wgpu (Metal sur Mac). Elle ne fait
//! aucun calcul d'image : le travail est confié à [`worker`], en arrière‑plan.

pub mod app;
pub mod crop;
pub mod dialogs;
pub mod i18n;
pub mod icons;
pub mod licence;
pub mod masking;
pub mod retouching;
pub mod style;
pub mod widgets;
pub mod worker;

pub use app::Studio;

/// Le journal de démarrage : sur la sortie d'erreur, et sur iPhone et iPad dans
/// `Documents/LÜMA-journal.txt` (lisible dans l'app Fichiers).
pub fn journal(message: &str) {
    eprintln!("LÜMA : {message}");
    if cfg!(target_os = "ios")
        && let Some(dir) = crate::dialogs::documents()
    {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("LÜMA-journal.txt"))
        {
            let _ = writeln!(f, "{message}");
        }
    }
}
