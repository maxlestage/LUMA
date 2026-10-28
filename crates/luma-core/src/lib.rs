//! Les types de base partagés par tout le moteur de LÜMA : les images en virgule
//! flottante, l'annulation des calculs devenus inutiles et la mesure du temps passé
//! dans chaque étape.

pub mod image;

pub use image::{Plane, RgbImage};

/// Le dossier de LÜMA sous Windows : `%APPDATA%\LÜMA` pour ce qui suit le compte
/// (catalogue, préréglages, films, licence), `%LOCALAPPDATA%\LÜMA` pour les caches
/// (`local`). `None` ailleurs que sur Windows, ou sans ces variables.
pub fn windows_dir(local: bool) -> Option<std::path::PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let var = if local { "LOCALAPPDATA" } else { "APPDATA" };
    Some(std::path::PathBuf::from(std::env::var_os(var)?).join("LÜMA"))
}

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Les erreurs du moteur.
#[derive(Debug)]
pub enum Error {
    /// Le fichier n'a pas pu être lu ou décodé.
    Decode(String),
    /// Le calcul a été annulé (un réglage a changé entre-temps).
    Cancelled,
    /// Entrée ou sortie.
    Io(std::io::Error),
    /// Un fichier de réglages (XMP, JSON) mal formé.
    Format(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Decode(e) => write!(f, "décodage impossible : {e}"),
            Error::Cancelled => write!(f, "calcul annulé"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Format(e) => write!(f, "format invalide : {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Un jeton d'annulation : l'interface l'annule quand l'utilisateur bouge à nouveau un
/// curseur, et le rendu en cours s'arrête à l'étape suivante au lieu de finir pour
/// rien.
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// `Err(Cancelled)` si le calcul a été annulé.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Le temps passé dans chaque étape d'un rendu, pour mesurer au lieu de supposer.
#[derive(Clone, Debug, Default)]
pub struct Timings {
    pub steps: Vec<(&'static str, Duration)>,
}

impl Timings {
    /// Mesure `f` sous le nom `name`.
    pub fn time<T>(&mut self, name: &'static str, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let out = f();
        self.steps.push((name, start.elapsed()));
        out
    }

    pub fn total(&self) -> Duration {
        self.steps.iter().map(|(_, d)| *d).sum()
    }
}

impl std::fmt::Display for Timings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, d) in &self.steps {
            writeln!(f, "  {name:<22} {:>8.1} ms", d.as_secs_f64() * 1000.0)?;
        }
        write!(f, "  {:<22} {:>8.1} ms", "total", self.total().as_secs_f64() * 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_is_shared_between_clones() {
        let a = Cancel::new();
        let b = a.clone();
        assert!(a.check().is_ok());
        b.cancel();
        assert!(matches!(a.check(), Err(Error::Cancelled)));
    }
}
