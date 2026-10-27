//! Les réglages par défaut par boîtier et par ISO : le point de départ d'une photo qui
//! n'a pas encore de réglages (pas de XMP), choisi d'après son boîtier et sa
//! sensibilité. La règle la plus précise l'emporte : le boîtier à partir de l'ISO le
//! plus proche en dessous de celui de la photo, puis le boîtier à tout ISO.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Settings;
use crate::paste::{Group, default_groups, paste};

/// Une règle : un boîtier (« Sony ILCE-7M3 »), à partir d'un ISO (ou à tout ISO), et
/// ses réglages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub camera: String,
    pub from_iso: Option<u32>,
    pub settings: Settings,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Defaults {
    pub rules: Vec<Rule>,
    /// Le film (préréglage) donné aux photos qui arrivent (dossier surveillé, boîtier
    /// relié), par son nom.
    pub import_preset: Option<String>,
}

impl Defaults {
    /// Lit le fichier ; vide s'il n'existe pas ou ne se lit pas.
    pub fn load(path: &Path) -> Defaults {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Écrit le fichier (à côté puis renommé : jamais à moitié écrit).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }

    /// Les réglages de départ d'une photo de ce boîtier à cet ISO, s'il y a une règle.
    pub fn for_photo(&self, camera: &str, iso: Option<u32>) -> Option<&Settings> {
        let mine = self.rules.iter().filter(|r| r.camera == camera);
        // À partir de l'ISO le plus haut qui ne dépasse pas celui de la photo…
        let by_iso = iso.and_then(|iso| {
            mine.clone()
                .filter(|r| r.from_iso.is_some_and(|f| f <= iso))
                .max_by_key(|r| r.from_iso)
        });
        // … sinon la règle du boîtier à tout ISO.
        by_iso
            .or_else(|| mine.clone().find(|r| r.from_iso.is_none()))
            .map(|r| &r.settings)
    }

    /// Fait des réglages d'une photo la règle de ce boîtier (à partir de cet ISO, ou à
    /// tout ISO) : seuls les groupes qui valent pour toutes ses photos (pas le
    /// recadrage, ni les masques, ni les retouches de taches).
    pub fn set(&mut self, camera: &str, from_iso: Option<u32>, settings: &Settings) {
        let base = Settings::default();
        let groups: Vec<Group> = default_groups();
        let mut kept = paste(&base, settings, &groups);
        kept.spots.clear();
        kept.liquify.clear();
        self.rules.retain(|r| !(r.camera == camera && r.from_iso == from_iso));
        self.rules.push(Rule {
            camera: camera.to_owned(),
            from_iso,
            settings: kept,
        });
        self.rules
            .sort_by(|a, b| (&a.camera, a.from_iso).cmp(&(&b.camera, b.from_iso)));
    }

    /// Retire les règles de ce boîtier ; rend leur nombre.
    pub fn clear(&mut self, camera: &str) -> usize {
        let before = self.rules.len();
        self.rules.retain(|r| r.camera != camera);
        before - self.rules.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(exposure: f32) -> Settings {
        Settings {
            exposure,
            ..Settings::default()
        }
    }

    #[test]
    fn the_most_precise_rule_wins() {
        let mut d = Defaults::default();
        d.set("Sony ILCE-7M3", None, &with(0.3));
        d.set("Sony ILCE-7M3", Some(1600), &with(0.6));
        d.set("Sony ILCE-7M3", Some(6400), &with(0.9));
        d.set("Nikon Z 6", None, &with(-0.5));
        let e = |d: &Defaults, camera: &str, iso| d.for_photo(camera, iso).map(|s| s.exposure);
        assert_eq!(e(&d, "Sony ILCE-7M3", Some(100)), Some(0.3));
        assert_eq!(e(&d, "Sony ILCE-7M3", Some(1600)), Some(0.6));
        assert_eq!(e(&d, "Sony ILCE-7M3", Some(3200)), Some(0.6));
        assert_eq!(e(&d, "Sony ILCE-7M3", Some(12800)), Some(0.9));
        assert_eq!(e(&d, "Sony ILCE-7M3", None), Some(0.3));
        assert_eq!(e(&d, "Nikon Z 6", Some(6400)), Some(-0.5));
        assert_eq!(e(&d, "Canon EOS R5", Some(100)), None);
        // La même règle remplacée, pas doublée ; puis retirée avec son boîtier.
        d.set("Sony ILCE-7M3", Some(1600), &with(0.7));
        assert_eq!(e(&d, "Sony ILCE-7M3", Some(1600)), Some(0.7));
        assert_eq!(d.clear("Sony ILCE-7M3"), 3);
        assert_eq!(e(&d, "Sony ILCE-7M3", Some(1600)), None);
    }

    #[test]
    fn crop_and_masks_stay_with_each_photo() {
        let mut s = with(1.0);
        s.crop.left = 0.3;
        s.masks.push(crate::Mask::linear());
        let mut d = Defaults::default();
        d.set("Sony ILCE-7M3", None, &s);
        let kept = d.for_photo("Sony ILCE-7M3", Some(100)).unwrap();
        assert_eq!(kept.exposure, 1.0);
        assert_eq!(kept.crop, Settings::default().crop);
        assert!(kept.masks.is_empty());
    }

    #[test]
    fn saved_and_read_back() {
        let path = std::env::temp_dir().join(format!("luma-defauts-{}.json", std::process::id()));
        let mut d = Defaults::default();
        d.set("Sony ILCE-7M3", Some(800), &with(0.4));
        d.save(&path).unwrap();
        assert_eq!(Defaults::load(&path), d);
        let _ = std::fs::remove_file(&path);
        assert_eq!(Defaults::load(&path), Defaults::default());
    }
}
