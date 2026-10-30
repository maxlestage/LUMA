//! Garder la photothèque en bon état : des sauvegardes du catalogue (une par semaine
//! dans l'app, à la demande en ligne de commande), et les photos manquantes ou
//! déplacées retrouvées.
//!
//! Les sauvegardes sont des copies du fichier du catalogue, dans un dossier
//! « Sauvegardes » à côté de lui, nommées par leur date : `Catalogue 2026-10-27
//! 081502.lumacat`. Les plus anciennes au‑delà des dernières sont effacées.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Catalog, Entry, exists, unix_date};

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
    // La sauvegarde est copiée d'abord : sauvegarder le catalogue fait le tri des plus
    // anciennes, qui peut emporter celle qu'on restaure.
    let tmp = catalog.with_extension("tmp");
    std::fs::copy(backup, &tmp)?;
    if catalog.is_file()
        && let Err(e) = backup_at(catalog, now(), KEEP + 1)
    {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::rename(&tmp, catalog)
}

impl Catalog {
    /// Les photos du catalogue dont le fichier n'est plus là (disque débranché,
    /// dossier déplacé ou renommé hors de LÜMA).
    pub fn missing(&self) -> Vec<&Entry> {
        use rayon::prelude::*;
        // Sur tous les cœurs, dans l'ordre des chemins.
        let all: Vec<&Entry> = self.entries().collect();
        all.into_par_iter().filter(|e| !exists(&e.path)).collect()
    }

    /// Les doublons : les photos du même boîtier, prises au même instant, dont les
    /// fichiers ont la même taille et le même type (une copie dans un autre dossier,
    /// une carte importée deux fois). Le RAW et le JPEG d'une même prise ne sont pas des
    /// doublons. Chaque groupe est rangé par chemin ; les groupes, par date.
    pub fn duplicates(&self) -> Vec<Vec<&Entry>> {
        let mut groups: std::collections::BTreeMap<(&str, String, u64, String), Vec<&Entry>> =
            std::collections::BTreeMap::new();
        for e in self.entries() {
            // Sans date ni taille, rien ne permet de dire que deux photos sont la même ;
            // une copie virtuelle partage, par nature, le fichier de son original.
            if e.date.is_empty() || e.size == 0 || luma_project::copies::number(&e.path).is_some() {
                continue;
            }
            let kind = e
                .path
                .extension()
                .map(|x| x.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            groups
                .entry((e.date.as_str(), e.camera(), e.size, kind))
                .or_default()
                .push(e);
        }
        groups
            .into_values()
            .filter(|g| g.len() > 1)
            .map(|mut g| {
                g.sort_by(|a, b| a.path.cmp(&b.path));
                g
            })
            .collect()
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
                exists(&new).then(|| (e.path.clone(), new))
            })
            .collect();
        for (a, b) in &moves {
            self.rename(a, b);
        }
        moves.len()
    }

    /// Un dossier retrouvé, puis ses voisins : quand le dossier choisi porte le même nom
    /// que le dossier manquant (`…/Été/jour 1` retrouvé en `…/Archives/Été/jour 1`),
    /// c'est souvent tout le dossier parent qui a bougé ; les photos manquantes des
    /// dossiers voisins sont cherchées à leur tour, en remontant tant que les noms
    /// concordent. Rend le nombre de photos retrouvées.
    pub fn relocate_with_neighbours(&mut self, from: &Path, to: &Path) -> usize {
        let mut found = self.relocate(from, to);
        let (mut from, mut to) = (from, to);
        while from.file_name().is_some()
            && from.file_name() == to.file_name()
            && let (Some(a), Some(b)) = (from.parent(), to.parent())
        {
            if a.as_os_str().is_empty() || b.as_os_str().is_empty() {
                break;
            }
            found += self.relocate(a, b);
            (from, to) = (a, b);
        }
        found
    }

    /// Les dossiers des photos manquantes, avec leur nombre, rangés par chemin : ce que
    /// la fenêtre d'entretien propose de retrouver.
    pub fn missing_folders(&self) -> Vec<(PathBuf, usize)> {
        let mut folders: std::collections::BTreeMap<PathBuf, usize> = std::collections::BTreeMap::new();
        for e in self.missing() {
            let dir = e.path.parent().unwrap_or(Path::new("")).to_path_buf();
            *folders.entry(dir).or_default() += 1;
        }
        folders.into_iter().collect()
    }

    /// Retire du catalogue les photos manquantes rangées sous `dir` (un disque perdu,
    /// des photos effacées pour de bon), et leur place dans les collections manuelles.
    /// Les photos encore là ne bougent pas. Rend le nombre de photos retirées.
    pub fn forget_missing(&mut self, dir: &Path) -> usize {
        let gone: Vec<PathBuf> = self
            .missing()
            .into_iter()
            .filter(|e| e.path.starts_with(dir))
            .map(|e| e.path.clone())
            .collect();
        self.forget(&gone)
    }

    /// Retire du catalogue les photos du dossier `dir` et de ses sous‑dossiers (ils ne
    /// sont plus suivis), et leur place dans les collections manuelles ; les fichiers
    /// ne sont pas touchés. Rend le nombre de photos retirées.
    pub fn remove_folder(&mut self, dir: &Path) -> usize {
        let gone: Vec<PathBuf> = self
            .entries
            .keys()
            .filter(|p| p.starts_with(dir) && p.as_path() != dir)
            .cloned()
            .collect();
        self.forget(&gone)
    }

    /// Ces photos sortent du catalogue et des collections manuelles.
    fn forget(&mut self, gone: &[PathBuf]) -> usize {
        for p in gone {
            self.entries.remove(p);
        }
        let set: std::collections::HashSet<&PathBuf> = gone.iter().collect();
        for c in &mut self.collections {
            if let crate::Content::Manual(list) = &mut c.content {
                list.retain(|p| !set.contains(p));
            }
        }
        gone.len()
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
            if exists(&new) {
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

    #[test]
    fn duplicates_are_grouped() {
        let photo = |path: &str, date: &str, model: &str, size: u64| Entry {
            path: PathBuf::from(path),
            date: date.into(),
            make: "SONY".into(),
            model: model.into(),
            size,
            ..Entry::default()
        };
        let mut cat = Catalog::default();
        for e in [
            // Une carte importée deux fois.
            photo("/a/DSC0001.ARW", "2026-06-01T10:00:00", "ILCE-7M3", 24_000_000),
            photo(
                "/b/2026-06-01/DSC0001.ARW",
                "2026-06-01T10:00:00",
                "ILCE-7M3",
                24_000_000,
            ),
            photo("/c/copie.ARW", "2026-06-01T10:00:00", "ILCE-7M3", 24_000_000),
            // Le JPEG de la même prise : pas un doublon.
            photo("/a/DSC0001.JPG", "2026-06-01T10:00:00", "ILCE-7M3", 24_000_000),
            // Même instant, autre boîtier ; même boîtier, autre taille.
            photo("/a/autre.ARW", "2026-06-01T10:00:00", "ILCE-7RM4", 24_000_000),
            photo("/a/rafale.ARW", "2026-06-01T10:00:00", "ILCE-7M3", 23_900_000),
            // Un autre jour, deux copies.
            photo("/a/DSC0100.ARW", "2026-05-01T08:00:00", "ILCE-7M3", 25_000_000),
            photo("/z/DSC0100.ARW", "2026-05-01T08:00:00", "ILCE-7M3", 25_000_000),
        ] {
            cat.insert(e);
        }
        let groups: Vec<Vec<&str>> = cat
            .duplicates()
            .iter()
            .map(|g| g.iter().map(|e| e.path.to_str().unwrap()).collect())
            .collect();
        assert_eq!(
            groups,
            vec![
                vec!["/a/DSC0100.ARW", "/z/DSC0100.ARW"],
                vec!["/a/DSC0001.ARW", "/b/2026-06-01/DSC0001.ARW", "/c/copie.ARW"],
            ]
        );
        assert!(Catalog::default().duplicates().is_empty());
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

    /// Restaurer la plus ancienne quand toutes les places sont prises : le tri qui suit
    /// la sauvegarde du catalogue ne l'emporte pas avant qu'elle soit relue.
    #[test]
    fn the_oldest_backup_can_be_restored() {
        let dir = temp("plus-ancienne");
        let file = dir.join("catalogue.lumacat");
        std::fs::write(&file, b"ancien").unwrap();
        backup_at(&file, 1_000_000_000, KEEP + 1).unwrap();
        std::fs::write(&file, b"courant").unwrap();
        for day in 1..=KEEP as u64 {
            backup_at(&file, 1_000_000_000 + day * 86_400, KEEP + 1).unwrap();
        }
        let oldest = list(&file).pop().unwrap();
        assert_eq!(std::fs::read(&oldest).unwrap(), b"ancien");
        restore(&oldest, &file).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"ancien");
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

    /// Tout un dossier déplacé : un de ses sous‑dossiers retrouvé, les autres suivent ;
    /// les dossiers manquants se listent avec leur nombre, et les photos perdues pour
    /// de bon sortent du catalogue et des collections, sans toucher aux autres.
    #[test]
    fn neighbours_follow_and_lost_photos_are_forgotten() {
        let dir = temp("voisins");
        let old = dir.join("Voyage");
        for f in ["jour 1/a.png", "jour 2/b.png", "jour 2/c.png", "d.png"] {
            let p = old.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"\x89PNG\r\n\x1a\n").unwrap();
        }
        let keep = dir.join("Ailleurs/e.png");
        std::fs::create_dir_all(keep.parent().unwrap()).unwrap();
        std::fs::write(&keep, b"\x89PNG\r\n\x1a\n").unwrap();
        let mut cat = Catalog::default();
        cat.add_folder(&dir, true);
        let mut c = Collection::manual("Choix");
        c.content = Content::Manual(vec![old.join("jour 2/c.png"), keep.clone()]);
        cat.collections.push(c);

        let new = dir.join("Archives/Voyage");
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        std::fs::rename(&old, &new).unwrap();
        std::fs::remove_file(new.join("jour 2/c.png")).unwrap();
        assert_eq!(
            cat.missing_folders(),
            vec![(old.clone(), 1), (old.join("jour 1"), 1), (old.join("jour 2"), 2)]
        );
        // Le seul « jour 1 » retrouvé : « jour 2 » et la photo du dossier suivent.
        assert_eq!(
            cat.relocate_with_neighbours(&old.join("jour 1"), &new.join("jour 1")),
            3
        );
        assert_eq!(cat.missing_folders(), vec![(old.join("jour 2"), 1)]);
        // Un nom différent : rien d'autre n'est cherché.
        assert_eq!(
            cat.relocate_with_neighbours(&old.join("jour 2"), &dir.join("Ailleurs")),
            0
        );

        assert_eq!(cat.forget_missing(&old), 1);
        assert!(cat.missing().is_empty());
        assert_eq!(cat.len(), 4);
        assert_eq!(cat.collections()[0].content, Content::Manual(vec![keep.clone()]));
        assert_eq!(cat.forget_missing(&dir), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Un dossier retiré de la photothèque : ses photos et celles de ses sous‑dossiers
    /// sortent du catalogue et des collections ; un dossier au nom voisin et les autres
    /// restent, les fichiers aussi.
    #[test]
    fn a_folder_is_removed_but_its_files_stay() {
        let dir = temp("retire");
        for f in ["a.png", "b.png", "sous/c.png"] {
            let p = dir.join("Voyage").join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"\x89PNG\r\n\x1a\n").unwrap();
        }
        let other = dir.join("Voyage 2/d.png");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        std::fs::write(&other, b"\x89PNG\r\n\x1a\n").unwrap();
        let mut cat = Catalog::default();
        cat.add_folder(&dir, true);
        let mut c = Collection::manual("Choix");
        c.content = Content::Manual(vec![dir.join("Voyage/a.png"), other.clone()]);
        cat.collections.push(c);
        assert_eq!(cat.len(), 4);
        assert_eq!(cat.remove_folder(&dir.join("Voyage")), 3);
        assert_eq!(cat.len(), 1);
        assert!(cat.get(&other).is_some(), "« Voyage 2 » n'est pas sous « Voyage »");
        assert_eq!(cat.collections()[0].content, Content::Manual(vec![other.clone()]));
        assert!(dir.join("Voyage/a.png").is_file(), "le fichier reste");
        assert_eq!(cat.remove_folder(&dir.join("Absent")), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
