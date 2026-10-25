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
pub mod style;
pub mod widgets;
pub mod worker;

pub use app::Studio;
