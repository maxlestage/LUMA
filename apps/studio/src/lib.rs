//! LÜMA Studio : l'application de développement photo, sur le moteur de LÜMA.
//!
//! L'interface est dessinée par egui et affichée par wgpu (Metal sur Mac). Elle ne fait
//! aucun calcul d'image : le travail est confié à [`worker`], en arrière‑plan.

pub mod app;
pub mod crop;
pub mod dialogs;
pub mod i18n;
pub mod icons;
pub mod keys;
pub mod licence;
pub mod liquifying;
pub mod masking;
pub mod retouching;
pub mod style;
pub mod tiles;
pub mod widgets;
pub mod worker;
pub mod workspaces;

pub use app::Studio;

use std::sync::atomic::{AtomicBool, Ordering};

/// L'interface au doigt, forcée hors d'iOS (les essais rendent celle de l'iPhone).
static TOUCH_FORCED: AtomicBool = AtomicBool::new(false);

/// Un écran tactile sans souris (iPhone, iPad) : cibles plus grandes, appui long pour
/// choisir, glisser pour faire défiler.
pub fn touch() -> bool {
    cfg!(target_os = "ios") || TOUCH_FORCED.load(Ordering::Relaxed)
}

/// Rend l'interface de l'iPhone et de l'iPad sur un autre système (pour les essais),
/// avant de créer le [`Studio`].
pub fn force_touch(on: bool) {
    TOUCH_FORCED.store(on, Ordering::Relaxed);
}

/// L'écran de l'app, pour qu'un travail de fond le réveille quand il a fini (au lieu
/// de redessiner toutes les 100 ms pour voir s'il a fini).
static SCREEN: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();

/// Retient l'écran de l'app (une fois, au démarrage).
pub fn set_screen(ctx: &egui::Context) {
    let _ = SCREEN.set(ctx.clone());
}

/// Redessine l'écran : un travail de fond vient de finir.
pub fn wake() {
    if let Some(ctx) = SCREEN.get() {
        ctx.request_repaint();
    }
}

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
