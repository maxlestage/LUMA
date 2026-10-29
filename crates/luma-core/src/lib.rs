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

/// Écrit `bytes` dans `path` d'un coup : dans un temporaire à côté (`nom.tmp`), renommé
/// ensuite. Une écriture interrompue (app arrêtée, disque plein) laisse l'ancien fichier
/// entier, jamais un fichier coupé ; le temporaire d'une écriture ratée est retiré.
pub fn save(path: &std::path::Path, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    let tmp = path.with_file_name(name);
    let written = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
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
    fn a_file_is_written_in_one_go() {
        let dir = std::env::temp_dir().join(format!("luma-ecrit-d-un-coup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Portra.cube");
        save(&path, "premier").unwrap();
        save(&path, b"second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names, [std::ffi::OsString::from("Portra.cube")]);
        // Une écriture qui échoue laisse l'ancien fichier entier, et rien d'autre.
        std::fs::create_dir_all(dir.join("Portra.cube.tmp/occupé")).unwrap();
        assert!(save(&path, "troisième").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_is_shared_between_clones() {
        let a = Cancel::new();
        let b = a.clone();
        assert!(a.check().is_ok());
        b.cancel();
        assert!(matches!(a.check(), Err(Error::Cancelled)));
    }
}
