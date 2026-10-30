//! Les collections de la photothèque : des ensembles de photos nommés, rangés dans le
//! catalogue.
//!
//! - Une collection **manuelle** est une liste de photos, choisies une à une, quel que
//!   soit leur dossier. Une photo absente du disque n'y apparaît plus, mais y reste
//!   inscrite : elle revient si son disque est rebranché.
//! - Une collection **dynamique** est une règle, écrite comme une recherche
//!   ([`Query::parse`]) : « note:4 date:2024 boîtier:sony ». Ses photos sont celles
//!   qui y répondent à cet instant ; elle suit le catalogue sans qu'on y touche.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{Catalog, Entry, Query};

/// Le contenu d'une collection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Content {
    /// Les photos choisies, dans l'ordre où elles ont été ajoutées.
    Manual(Vec<PathBuf>),
    /// La règle, au format d'une recherche.
    Smart(String),
}

/// Une collection nommée.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    pub name: String,
    pub content: Content,
}

impl Collection {
    pub fn manual(name: &str) -> Collection {
        Collection {
            name: name.trim().to_owned(),
            content: Content::Manual(Vec::new()),
        }
    }

    /// Une collection dynamique ; la règle est vérifiée.
    pub fn smart(name: &str, rule: &str) -> Result<Collection, String> {
        Query::parse(rule)?;
        Ok(Collection {
            name: name.trim().to_owned(),
            content: Content::Smart(rule.trim().to_owned()),
        })
    }

    pub fn is_smart(&self) -> bool {
        matches!(self.content, Content::Smart(_))
    }
}

impl Catalog {
    /// Les collections, par ordre alphabétique.
    pub fn collections(&self) -> &[Collection] {
        &self.collections
    }

    pub fn collection(&self, name: &str) -> Option<&Collection> {
        self.collections.iter().find(|c| c.name == name)
    }

    /// Ajoute une collection, ou remplace celle du même nom.
    pub fn set_collection(&mut self, c: Collection) -> Result<(), String> {
        if c.name.is_empty() {
            return Err("une collection a besoin d'un nom".into());
        }
        if let Content::Smart(rule) = &c.content {
            Query::parse(rule)?;
        }
        match self.collections.iter_mut().find(|o| o.name == c.name) {
            Some(old) => *old = c,
            None => {
                self.collections.push(c);
                self.collections.sort_by_key(|c| crate::query::fold(&c.name));
            }
        }
        Ok(())
    }

    /// Renomme une collection, son contenu gardé, à sa place dans l'ordre alphabétique ;
    /// refusé si le nom est vide, déjà pris par une autre, ou si elle n'existe pas.
    pub fn rename_collection(&mut self, from: &str, to: &str) -> Result<(), String> {
        let to = to.trim();
        if to.is_empty() {
            return Err("une collection a besoin d'un nom".into());
        }
        if to != from && self.collection(to).is_some() {
            return Err(format!("une collection s'appelle déjà « {to} »"));
        }
        let c = self
            .collections
            .iter_mut()
            .find(|c| c.name == from)
            .ok_or_else(|| format!("pas de collection « {from} »"))?;
        c.name = to.to_owned();
        self.collections.sort_by_key(|c| crate::query::fold(&c.name));
        Ok(())
    }

    /// Retire une collection (les photos restent dans le catalogue) ; `false` si elle
    /// n'existait pas.
    pub fn remove_collection(&mut self, name: &str) -> bool {
        let before = self.collections.len();
        self.collections.retain(|c| c.name != name);
        self.collections.len() != before
    }

    /// Ajoute des photos à une collection manuelle, créée au besoin ; une photo déjà
    /// dedans n'est pas ajoutée deux fois. Renvoie le nombre de photos ajoutées.
    pub fn add_to_collection(&mut self, name: &str, photos: &[PathBuf]) -> Result<usize, String> {
        let name = name.trim();
        if self.collection(name).is_none() {
            self.set_collection(Collection::manual(name))?;
        }
        let c = self.collections.iter_mut().find(|c| c.name == name).expect("créée");
        let Content::Manual(list) = &mut c.content else {
            return Err(format!(
                "« {name} » est une collection dynamique : ses photos suivent sa règle"
            ));
        };
        let mut added = 0;
        for p in photos {
            if !list.contains(p) {
                list.push(p.clone());
                added += 1;
            }
        }
        Ok(added)
    }

    /// Retire des photos d'une collection manuelle. Renvoie le nombre retiré.
    pub fn remove_from_collection(&mut self, name: &str, photos: &[PathBuf]) -> usize {
        let Some(Content::Manual(list)) = self
            .collections
            .iter_mut()
            .find(|c| c.name == name)
            .map(|c| &mut c.content)
        else {
            return 0;
        };
        let before = list.len();
        list.retain(|p| !photos.contains(p));
        before - list.len()
    }

    /// Les photos d'une collection, par date de prise de vue ; seules celles du catalogue
    /// y figurent.
    pub fn collection_photos(&self, name: &str) -> Result<Vec<&Entry>, String> {
        let c = self
            .collection(name)
            .ok_or_else(|| format!("pas de collection « {name} »"))?;
        Ok(match &c.content {
            Content::Smart(rule) => self.search(&Query::parse(rule)?),
            Content::Manual(list) => {
                let mut v: Vec<&Entry> = list.iter().filter_map(|p| self.get(p)).collect();
                v.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.path.cmp(&b.path)));
                v
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        let mut cat = Catalog::default();
        for (name, date, rating) in [
            ("a.arw", "2024-06-02T10:00:00", 4),
            ("b.arw", "2024-06-01T10:00:00", 2),
            ("c.arw", "2025-01-01T10:00:00", 5),
        ] {
            cat.insert(Entry {
                path: name.into(),
                date: date.into(),
                rating,
                ..Entry::default()
            });
        }
        cat
    }

    fn names(v: Vec<&Entry>) -> Vec<String> {
        v.iter().map(|e| e.path.display().to_string()).collect()
    }

    #[test]
    fn manual_collections() {
        let mut cat = catalog();
        let photos: Vec<PathBuf> = ["c.arw", "b.arw", "c.arw", "absente.arw"].map(PathBuf::from).to_vec();
        assert_eq!(cat.add_to_collection("Tirages", &photos).unwrap(), 3, "sans doublon");
        assert_eq!(names(cat.collection_photos("Tirages").unwrap()), ["b.arw", "c.arw"]);
        assert_eq!(cat.remove_from_collection("Tirages", &[PathBuf::from("b.arw")]), 1);
        assert_eq!(names(cat.collection_photos("Tirages").unwrap()), ["c.arw"]);
        assert!(!cat.collection("Tirages").unwrap().is_smart());
    }

    /// Renommée, une collection garde ses photos et sa place alphabétique ; un nom vide,
    /// pris ou inconnu est refusé.
    #[test]
    fn a_collection_is_renamed() {
        let mut cat = catalog();
        cat.add_to_collection("Tirages", &[PathBuf::from("c.arw")]).unwrap();
        cat.set_collection(Collection::smart("Meilleures", "note:4").unwrap())
            .unwrap();
        cat.rename_collection("Tirages", " Albums ").unwrap();
        assert_eq!(names(cat.collection_photos("Albums").unwrap()), ["c.arw"]);
        assert!(cat.collection("Tirages").is_none());
        let order: Vec<&str> = cat.collections().iter().map(|c| c.name.as_str()).collect();
        assert_eq!(order, ["Albums", "Meilleures"]);
        assert!(
            cat.rename_collection("Albums", "meilleures").is_ok(),
            "le nom d'une autre, aux majuscules près"
        );
        assert_eq!(
            cat.rename_collection("meilleures", "Meilleures").unwrap_err(),
            "une collection s'appelle déjà « Meilleures »"
        );
        assert!(cat.rename_collection("meilleures", "  ").is_err());
        assert!(cat.rename_collection("Absente", "X").is_err());
    }

    #[test]
    fn smart_collections_follow_the_catalog() {
        let mut cat = catalog();
        cat.set_collection(Collection::smart("Meilleures", "note:4").unwrap())
            .unwrap();
        assert_eq!(names(cat.collection_photos("Meilleures").unwrap()), ["a.arw", "c.arw"]);
        cat.insert(Entry {
            path: "d.arw".into(),
            date: "2023-01-01T00:00:00".into(),
            rating: 5,
            ..Entry::default()
        });
        assert_eq!(
            names(cat.collection_photos("Meilleures").unwrap()),
            ["d.arw", "a.arw", "c.arw"]
        );
        assert_eq!(
            cat.add_to_collection("Meilleures", &[PathBuf::from("b.arw")])
                .unwrap_err(),
            "« Meilleures » est une collection dynamique : ses photos suivent sa règle"
        );
        // Remplacée sous le même nom.
        cat.set_collection(Collection::smart("Meilleures", "note:5 date:2025").unwrap())
            .unwrap();
        assert_eq!(names(cat.collection_photos("Meilleures").unwrap()), ["c.arw"]);
        assert_eq!(cat.collections().len(), 1);
    }

    #[test]
    fn names_rules_and_order() {
        let mut cat = catalog();
        assert_eq!(
            Collection::smart("X", "couleur:rouge").unwrap_err(),
            "filtre inconnu « couleur »"
        );
        assert!(cat.set_collection(Collection::manual("  ")).is_err());
        for n in ["Voyages", "été 2024", "Archives"] {
            cat.set_collection(Collection::manual(n)).unwrap();
        }
        let order: Vec<&str> = cat.collections().iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            order,
            ["Archives", "été 2024", "Voyages"],
            "ordre alphabétique sans accents"
        );
        assert!(cat.remove_collection("Voyages"));
        assert!(!cat.remove_collection("Voyages"));
        assert_eq!(
            cat.collection_photos("Voyages").unwrap_err(),
            "pas de collection « Voyages »"
        );
    }
}
