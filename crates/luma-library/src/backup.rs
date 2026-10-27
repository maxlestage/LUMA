//! Garder la photothèque en bon état : des sauvegardes du catalogue (une par semaine
//! dans l'app, à la demande en ligne de commande), et les photos manquantes ou
//! déplacées retrouvées.
//!
//! Les sauvegardes sont des copies du fichier du catalogue, dans un dossier
//! « Sauvegardes » à côté de lui, nommées par leur date : `Catalogue 2026-10-27
//! 081502.lumacat`. Les plus anciennes au‑delà des dernières sont effacées.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Catalog, Entry, unix_date};

/// Le nombre de sauvegardes gardées.
pub const KEEP: usize = 10;

/// Le dossier des sauvegardes d'un catalogue.
pub fn folder(catalog: &Path) -> PathBuf {
    catalog.parent().unwrap_or(Path::new(".")).join("Sauvegardes")
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Les sauvegardes d'un catalogue, de la plus récente à la plus ancienne.
pub fn list(catalog: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(folder(catalog)) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = read
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("Catalogue ") && n.ends_with(".lumacat"))
        })
        .collect();
    // Les noms portent la date : l'ordre alphabétique est l'ordre chronologique.
    v.sort();
    v.reverse();
    v
}

/// Copie le catalogue dans ses sauvegardes, à la date `secs` (secondes depuis 1970),
/// puis n'en garde que les `keep` dernières. Rend la sauvegarde écrite.
pub fn backup_at(catalog: &Path, secs: u64, keep: usize) -> std::io::Result<PathBuf> {
    let dir = folder(catalog);
    std::fs::create_dir_all(&dir)?;
    let stamp = unix_date(secs).replace('T', " ").replace(':', "");
    let mut to = dir.join(format!("Catalogue {stamp}.lumacat"));
    let mut n = 2;
    while to.exists() {
        to = dir.join(format!("Catalogue {stamp}-{n}.lumacat"));
        n += 1;
    }
    std::fs::copy(catalog, &to)?;
    for old in list(catalog).into_iter().skip(keep.max(1)) {
        let _ = std::fs::remove_file(old);
    }
    Ok(to)
}

/// Une sauvegarde maintenant.
pub fn backup(catalog: &Path) -> std::io::Result<PathBuf> {
    backup_at(catalog, now(), KEEP)
}

/// La dernière sauvegarde a‑t‑elle plus de `days` jours (ou n'y en a‑t‑il aucune) ?
/// Un catalogue qui n'existe pas encore n'a rien à sauvegarder.
pub fn due(catalog: &Path, days: u64) -> bool {
    if !catalog.is_file() {
        return false;
    }
    let newest = list(catalog).into_iter().next().and_then(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
    });
    newest.is_none_or(|t| now().saturating_sub(t) >= days * 86_400)
}

/// Remet une sauvegarde à la place du catalogue ; le catalogue d'avant est d'abord
/// sauvegardé lui aussi (rien n'est perdu). L'écriture passe par un fichier
/// temporaire renommé.
pub fn restore(backup: &Path, catalog: &Path) -> std::io::Result<()> {
    if catalog.is_file() {
        backup_at(catalog, now(), KEEP + 1)?;
    }
    let tmp = catalog.with_extension("tmp");
    std::fs::copy(backup, &tmp)?;
    std::fs::rename(&tmp, catalog)
}

impl Catalog {
    /// Les photos du catalogue dont le fichier n'est plus là (disque débranché,
    /// dossier déplacé ou renommé hors de LÜMA).
    pub fn missing(&self) -> Vec<&Entry> {
        self.entries().filter(|e| !e.path.is_file()).collect()
    }

    /// Un dossier déplacé : les photos manquantes rangées sous `from` sont cherchées au
    /// même endroit sous `to`, et celles qu'on y trouve y passent, avec leurs
    /// collections (voir [`Catalog::rename`]). Rend le nombre de photos retrouvées.
    pub fn relocate(&mut self, from: &Path, to: &Path) -> usize {
        let moves: Vec<(PathBuf, PathBuf)> = self
            .missing()
            .into_iter()
            .filter_map(|e| {
                let rest = e.path.strip_prefix(from).ok()?;
                let new = to.join(rest);
                new.is_file().then(|| (e.path.clone(), new))
            })
            .collect();
        for (a, b) in &moves {
            self.rename(a, b);
        }
        moves.len()
    }

    /// Sur iPhone et iPad, le dossier de l'app (`…/Containers/Data/Application/<id>`)
    /// change de nom quand l'app est mise à jour : les photos d'avant sont suivies
    /// jusqu'au dossier d'aujourd'hui, `home`, avec leurs notes et leurs collections.
    /// Celles d'un ancien dossier qu'on ne retrouve pas en sortent : ce dossier n'existe
    /// plus, elles ne reviendront pas (et la liste des dossiers n'en montre pas deux
    /// du même nom). Rend le nombre de photos suivies.
    pub fn follow_container(&mut self, home: &Path) -> usize {
        let Some(own) = container(home) else {
            return 0;
        };
        let mut moves = Vec::new();
        let mut gone = Vec::new();
        for e in self.missing() {
            let Some(old) = container(&e.path) else {
                continue;
            };
            if old == own {
                continue;
            }
            let new = home.join(e.path.strip_prefix(&old).unwrap_or(&e.path));
            if new.is_file() {
                moves.push((e.path.clone(), new));
            } else {
                gone.push(e.path.clone());
            }
        }
        for (a, b) in &moves {
            self.rename(a, b);
        }
        for p in gone {
            self.entries.remove(&p);
        }
        moves.len()
    }
}

/// Le dossier d'une app d'iPhone ou d'iPad qui contient `path`
/// (`…/Containers/Data/Application/<id>`), s'il y en a un.
fn container(path: &Path) -> Option<PathBuf> {
    let parts: Vec<_> = path.components().collect();
    let i = parts.windows(3).position(|w| {
        w[0].as_os_str() == "Containers" && w[1].as_os_str() == "Data" && w[2].as_os_str() == "Application"
    })?;
    parts.get(i + 3)?;
    Some(parts[..i + 4].iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Collection, Content};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("luma-sauvegarde-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Des sauvegardes datées, les dix dernières gardées ; une restauration remet
    /// l'ancien catalogue et garde celui qu'elle remplace.
    #[test]
    fn backups_are_dated_pruned_and_restored() {
        let dir = temp("rotation");
        let file = dir.join("catalogue.lumacat");
        assert!(!due(&file, 7));
        std::fs::write(&file, b"v1").unwrap();
        assert!(due(&file, 7));
        let first = backup_at(&file, 1_793_017_387, 3).unwrap();
        assert_eq!(first.file_name().unwrap(), "Catalogue 2026-10-26 122307.lumacat");
        // Deux sauvegardes dans la même seconde : la seconde reçoit -2.
        let again = backup_at(&file, 1_793_017_387, 3).unwrap();
        assert!(again.to_string_lossy().ends_with("122307-2.lumacat"));
        for day in 1..=4 {
            backup_at(&file, 1_793_017_387 + day * 86_400, 3).unwrap();
        }
        let all = list(&file);
        assert_eq!(all.len(), 3);
        assert!(all[0].to_string_lossy().contains("2026-10-30"));
        assert!(!first.exists());
        assert!(!due(&file, 7));

        std::fs::write(&file, b"v2").unwrap();
        restore(&all[2], &file).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"v1");
        // Le catalogue remplacé est dans les sauvegardes (datée de maintenant).
        assert!(list(&file).iter().any(|p| std::fs::read(p).unwrap() == b"v2"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Une mise à jour de l'app sur iPhone : son dossier change de nom ; les photos de
    /// Documents sont suivies, celles qui ont disparu sortent, le reste ne bouge pas.
    #[test]
    fn photos_follow_the_app_container() {
        let dir = temp("conteneur");
        let apps = dir.join("var/mobile/Containers/Data/Application");
        let (old, new) = (apps.join("AAAA-1111"), apps.join("BBBB-2222"));
        std::fs::create_dir_all(old.join("Documents")).unwrap();
        for f in ["a.png", "b.png"] {
            std::fs::write(old.join("Documents").join(f), b"\x89PNG\r\n\x1a\n").unwrap();
        }
        let elsewhere = dir.join("Photos");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("c.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        let mut cat = Catalog::default();
        cat.add_folder(&old.join("Documents"), false);
        cat.add_folder(&elsewhere, false);
        // La mise à jour : le dossier de l'app est renommé, une photo a disparu, une
        // photo d'ailleurs est débranchée (elle doit rester).
        std::fs::rename(&old, &new).unwrap();
        std::fs::remove_file(new.join("Documents/b.png")).unwrap();
        std::fs::rename(elsewhere.join("c.png"), dir.join("c.png")).unwrap();
        assert_eq!(cat.follow_container(&dir.join("ailleurs")), 0);
        assert_eq!(cat.follow_container(&new), 1);
        assert!(cat.get(&new.join("Documents/a.png")).is_some());
        assert!(cat.get(&old.join("Documents/b.png")).is_none());
        assert!(cat.get(&elsewhere.join("c.png")).is_some());
        assert_eq!(cat.len(), 2);
        assert_eq!(container(&new.join("Documents/a.png")), Some(new.clone()));
        assert_eq!(container(&apps), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Un dossier déplacé hors de LÜMA : ses photos manquent, puis sont retrouvées à
    /// leur nouvelle place, collections comprises ; celles qui n'y sont pas restent
    /// manquantes.
    #[test]
    fn moved_folder_is_found_again() {
        let dir = temp("deplace");
        let old = dir.join("Été");
        std::fs::create_dir_all(old.join("jour 1")).unwrap();
        for f in ["a.png", "jour 1/b.png", "c.png"] {
            std::fs::write(old.join(f), b"\x89PNG\r\n\x1a\n").unwrap();
        }
        let mut cat = Catalog::default();
        cat.add_folder(&old, true);
        let mut c = Collection::manual("Choix");
        c.content = Content::Manual(vec![old.join("jour 1/b.png")]);
        cat.collections.push(c);
        assert!(cat.missing().is_empty());

        let new = dir.join("Archives/Été 2026");
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        std::fs::rename(&old, &new).unwrap();
        std::fs::remove_file(new.join("c.png")).unwrap();
        assert_eq!(cat.missing().len(), 3);
        assert_eq!(cat.relocate(&old, &new), 2);
        assert_eq!(cat.missing().len(), 1);
        assert!(cat.get(&new.join("a.png")).is_some());
        assert_eq!(
            cat.collections()[0].content,
            Content::Manual(vec![new.join("jour 1/b.png")])
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
