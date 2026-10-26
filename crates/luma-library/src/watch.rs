//! Le dossier surveillé : les photos qui y arrivent (prise de vue en connexion, carte
//! copiée, export d'un autre poste) sont signalées dès qu'elles sont complètes.
//!
//! Par relevés successifs du dossier, sans service du système : portable, sans
//! dépendance, et sûr avec les dossiers réseau. Une photo n'est donnée qu'une fois sa
//! taille et sa date inchangées d'un relevé au suivant : un fichier encore en cours
//! d'écriture attend. Les fichiers cachés (« .IMG_0001.ARW », écrits puis renommés)
//! sont ignorés.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// La taille et la date d'un fichier.
type Signature = (u64, Option<SystemTime>);

#[derive(Debug)]
pub struct Watch {
    dir: PathBuf,
    /// Les photos déjà là ou déjà signalées.
    known: HashMap<PathBuf, Signature>,
    /// Les nouvelles, vues une fois, qui attendent d'être complètes.
    waiting: HashMap<PathBuf, Signature>,
}

fn photos(dir: &Path) -> Vec<(PathBuf, Signature)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| {
            let path = e.path();
            let meta = e.metadata().ok()?;
            (meta.is_file() && luma_raw::is_photo(&path)).then(|| (path, (meta.len(), meta.modified().ok())))
        })
        .collect()
}

impl Watch {
    /// Commence à surveiller `dir` : les photos déjà là ne sont pas nouvelles.
    pub fn new(dir: &Path) -> Self {
        Watch {
            dir: dir.to_path_buf(),
            known: photos(dir).into_iter().collect(),
            waiting: HashMap::new(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Relève le dossier : les photos arrivées et complètes depuis le relevé précédent,
    /// par nom. Une photo retirée puis remise compte de nouveau.
    pub fn poll(&mut self) -> Vec<PathBuf> {
        let now: HashMap<PathBuf, Signature> = photos(&self.dir).into_iter().collect();
        self.known.retain(|p, _| now.contains_key(p));
        self.waiting.retain(|p, _| now.contains_key(p));
        let mut ready = Vec::new();
        for (path, sig) in now {
            if self.known.contains_key(&path) {
                continue;
            }
            match self.waiting.get(&path) {
                Some(seen) if *seen == sig && sig.0 > 0 => {
                    self.waiting.remove(&path);
                    self.known.insert(path.clone(), sig);
                    ready.push(path);
                }
                _ => {
                    self.waiting.insert(path, sig);
                }
            }
        }
        ready.sort();
        ready
    }

    /// Des photos sont‑elles en cours d'arrivée ?
    pub fn is_receiving(&self) -> bool {
        !self.waiting.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("luma-watch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn new_photos_come_once_complete() {
        let d = dir("arrivees");
        std::fs::write(d.join("ancienne.ARW"), b"deja la").unwrap();
        let mut w = Watch::new(&d);
        assert!(w.poll().is_empty(), "les photos déjà là ne sont pas nouvelles");

        // Une photo en cours d'écriture : vue, mais pas encore donnée.
        let mut f = std::fs::File::create(d.join("IMG_0002.CR3")).unwrap();
        f.write_all(b"debut").unwrap();
        f.flush().unwrap();
        assert!(w.poll().is_empty());
        assert!(w.is_receiving());
        f.write_all(b" et la suite").unwrap();
        f.flush().unwrap();
        assert!(w.poll().is_empty(), "encore en train de grandir");
        drop(f);
        assert_eq!(w.poll(), [d.join("IMG_0002.CR3")]);
        assert!(!w.is_receiving());
        assert!(w.poll().is_empty(), "une seule fois");

        // Les autres fichiers, les fichiers cachés et les vides ne comptent pas.
        std::fs::write(d.join("notes.txt"), b"texte").unwrap();
        std::fs::write(d.join(".IMG_0003.NEF"), b"temporaire").unwrap();
        std::fs::write(d.join("IMG_0004.NEF"), b"").unwrap();
        std::fs::create_dir(d.join("sous.ARW")).unwrap();
        w.poll();
        assert!(w.poll().is_empty());

        // Deux arrivées en même temps : par nom ; une photo retirée puis remise revient.
        std::fs::write(d.join("b.DNG"), b"b").unwrap();
        std::fs::write(d.join("a.jpg"), b"a").unwrap();
        w.poll();
        assert_eq!(w.poll(), [d.join("a.jpg"), d.join("b.DNG")]);
        std::fs::remove_file(d.join("a.jpg")).unwrap();
        w.poll();
        std::fs::write(d.join("a.jpg"), b"a de nouveau").unwrap();
        w.poll();
        assert_eq!(w.poll(), [d.join("a.jpg")]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_folder_is_just_empty() {
        let mut w = Watch::new(Path::new("/nulle/part/luma"));
        assert!(w.poll().is_empty());
    }
}
