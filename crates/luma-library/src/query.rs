//! Rechercher dans la photothèque : un texte libre et des filtres (note, drapeau,
//! libellé, date, ISO, focale, boîtier, objectif, mot‑clé).
//!
//! Une recherche s'écrit aussi en une ligne, mots et filtres mêlés :
//!
//! ```text
//! mer note:3 drapeau:retenue date:2024-06..2024-08 iso:..800 boîtier:sony
//! ```
//!
//! Les mots libres doivent tous se trouver dans le nom du fichier, le boîtier,
//! l'objectif, le libellé ou un mot‑clé ; majuscules et accents ne comptent pas
//! (« ete » trouve « Été »).

use luma_project::xmp::Flag;

use crate::Entry;

/// Une plage de valeurs, bornes comprises ; une borne absente est ouverte.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Range<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

impl<T: PartialOrd + Copy> Range<T> {
    pub fn is_open(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// Une valeur absente (photo sans EXIF) ne passe qu'une plage ouverte.
    fn contains(&self, v: Option<T>) -> bool {
        if self.is_open() {
            return true;
        }
        v.is_some_and(|v| self.min.is_none_or(|m| v >= m) && self.max.is_none_or(|m| v <= m))
    }
}

/// Une recherche. `Query::default()` trouve tout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// Les mots libres, déjà repliés (minuscules, sans accents).
    pub words: Vec<String>,
    /// Note minimale.
    pub min_rating: u8,
    pub flag: Option<Flag>,
    pub label: Option<String>,
    /// Dates `AAAA`, `AAAA-MM`, `AAAA-MM-JJ`… : depuis le début de la première,
    /// jusqu'à la fin de la seconde (`date:2024` : toute l'année).
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub iso: Range<u32>,
    /// En millimètres.
    pub focal_length: Range<f32>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub keyword: Option<String>,
}

/// Minuscules et sans accents, pour comparer des textes comme on les tape.
pub fn fold(s: &str) -> String {
    if s.is_ascii() {
        return s.to_ascii_lowercase();
    }
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' => 'i',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => 'u',
            'ý' | 'ÿ' => 'y',
            'ś' | 'š' => 's',
            'ź' | 'ż' | 'ž' => 'z',
            'ł' => 'l',
            'ř' => 'r',
            'ť' => 't',
            'ď' => 'd',
            c => c,
        })
        .collect()
}

fn range<T: std::str::FromStr + PartialOrd + Copy>(s: &str) -> Option<Range<T>> {
    let (a, b) = s.split_once("..").unwrap_or((s, s));
    let bound = |v: &str| -> Option<Option<T>> {
        let v = v.trim().trim_end_matches("mm");
        if v.is_empty() {
            Some(None)
        } else {
            v.parse().ok().map(Some)
        }
    };
    Some(Range {
        min: bound(a)?,
        max: bound(b)?,
    })
}

impl Query {
    /// Lit une recherche écrite en une ligne. Les filtres ont un nom français ou
    /// anglais : `note`/`rating`, `drapeau`/`flag` (`retenue`, `rejetée`, `aucun`),
    /// `libellé`/`label`, `date`, `iso`, `focale`/`focal`, `boîtier`/`camera`,
    /// `objectif`/`lens`, `mot`/`keyword`. Un filtre inconnu ou mal écrit est une
    /// erreur, qui le nomme.
    pub fn parse(text: &str) -> Result<Query, String> {
        let mut q = Query::default();
        for token in text.split_whitespace() {
            let Some((key, value)) = token.split_once(':').filter(|(k, _)| !k.is_empty()) else {
                q.words.push(fold(token));
                continue;
            };
            let bad = || format!("filtre « {token} » mal écrit");
            match fold(key).as_str() {
                "note" | "rating" => {
                    q.min_rating = value.trim_end_matches('+').parse::<u8>().map_err(|_| bad())?.min(5)
                }
                "drapeau" | "flag" => {
                    q.flag = Some(match fold(value).as_str() {
                        "retenue" | "retenues" | "pick" | "picked" => Flag::Pick,
                        "rejetee" | "rejetees" | "reject" | "rejected" => Flag::Reject,
                        "aucun" | "none" => Flag::None,
                        _ => return Err(bad()),
                    })
                }
                "libelle" | "label" => q.label = Some(fold(value)),
                "date" => {
                    let (a, b) = value.split_once("..").unwrap_or((value, value));
                    let ok = |d: &str| d.chars().all(|c| c.is_ascii_digit() || c == '-');
                    if !ok(a) || !ok(b) || (a.is_empty() && b.is_empty()) {
                        return Err(bad());
                    }
                    q.date_from = (!a.is_empty()).then(|| a.to_owned());
                    q.date_to = (!b.is_empty()).then(|| b.to_owned());
                }
                "iso" => q.iso = range(value).ok_or_else(bad)?,
                "focale" | "focal" => q.focal_length = range(value).ok_or_else(bad)?,
                "boitier" | "camera" => q.camera = Some(fold(value)),
                "objectif" | "lens" => q.lens = Some(fold(value)),
                "mot" | "keyword" => q.keyword = Some(fold(value)),
                _ => return Err(format!("filtre inconnu « {key} »")),
            }
        }
        Ok(q)
    }

    /// Cette photo répond‑elle à la recherche ?
    pub fn matches(&self, e: &Entry) -> bool {
        if e.rating < self.min_rating || self.flag.is_some_and(|f| f != e.flag()) {
            return false;
        }
        // Un libellé de couleur se retrouve dans toutes les langues (« libellé:rouge »
        // trouve « Red ») ; un autre libellé, à l'identique.
        if let Some(l) = &self.label {
            let same = match luma_project::xmp::label_color(l) {
                Some(c) => luma_project::xmp::label_color(&e.label) == Some(c),
                None => fold(&e.label) == *l,
            };
            if !same {
                return false;
            }
        }
        if self.date_from.as_ref().is_some_and(|d| e.date.as_str() < d.as_str()) {
            return false;
        }
        // La fin d'une date partielle : « 2024-06 » garde tout juin.
        if self
            .date_to
            .as_ref()
            .is_some_and(|d| e.date.get(..d.len()).unwrap_or(&e.date) > d.as_str())
        {
            return false;
        }
        if !self.iso.contains(e.iso) || !self.focal_length.contains(e.focal_length) {
            return false;
        }
        if self.camera.as_ref().is_some_and(|c| !e.camera_contains(c)) {
            return false;
        }
        if self.lens.as_ref().is_some_and(|l| !contains(&e.lens, l)) {
            return false;
        }
        if self
            .keyword
            .as_ref()
            .is_some_and(|k| !e.keywords.iter().any(|w| fold(w) == *k))
        {
            return false;
        }
        let name = e.path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
        self.words.iter().all(|w| {
            contains(&name, w)
                || e.camera_contains(w)
                || contains(&e.lens, w)
                || contains(&e.label, w)
                || e.keywords.iter().any(|k| contains(k, w))
        })
    }
}

/// `text` contient‑il `folded` (déjà replié), majuscules et accents mis à part ? Sans
/// allocation pour un texte ASCII, le cas courant (noms de fichiers, boîtiers).
pub fn contains(text: &str, folded: &str) -> bool {
    if folded.is_empty() {
        return true;
    }
    if text.is_ascii() {
        let (t, n) = (text.as_bytes(), folded.as_bytes());
        t.len() >= n.len() && t.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
    } else {
        fold(text).contains(folded)
    }
}

impl Entry {
    /// Le boîtier contient‑il `folded` ? Marque et modèle se lisent « marque modèle ».
    fn camera_contains(&self, folded: &str) -> bool {
        contains(&self.model, folded)
            || contains(&self.make, folded)
            || (folded.contains(' ') && contains(&self.camera(), folded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo(name: &str, date: &str) -> Entry {
        Entry {
            path: format!("/photos/{name}").into(),
            date: date.into(),
            make: "Sony".into(),
            model: "ILCE-7M3".into(),
            lens: "FE 24-70mm F2.8 GM".into(),
            iso: Some(400),
            focal_length: Some(35.0),
            ..Entry::default()
        }
    }

    #[test]
    fn folding() {
        assert_eq!(fold("Été à Ł'ÎLE"), "ete a l'ile");
    }

    #[test]
    fn everything_matches_the_empty_query() {
        assert!(Query::parse("  ").unwrap().matches(&Entry::default()));
    }

    #[test]
    fn free_words_and_filters() {
        let mut e = photo("DSC0001.ARW", "2024-06-15T10:00:00");
        e.rating = 4;
        e.flag = 1;
        e.label = "Rouge".into();
        e.keywords = vec!["Été".into(), "Mer".into()];
        let yes = [
            "",
            "ete",
            "MER sony",
            "dsc0001",
            "24-70",
            "note:4",
            "rating:3",
            "drapeau:retenue",
            "flag:pick",
            "libellé:rouge",
            "label:red",
            "libellé:rojo",
            "date:2024",
            "date:2024-06",
            "date:2024-06-15",
            "date:2024-01..2024-06",
            "date:2024-06..",
            "date:..2024-06-15",
            "iso:400",
            "iso:100..800",
            "iso:..400",
            "focale:24..70",
            "focal:35mm",
            "boîtier:sony",
            "camera:ilce-7m3",
            "objectif:gm",
            "mot:ete",
            "keyword:mer",
            "mer note:3 drapeau:retenue date:2024-06..2024-08 iso:..800 boitier:sony",
        ];
        for q in yes {
            assert!(Query::parse(q).unwrap().matches(&e), "« {q} » devrait trouver la photo");
        }
        let no = [
            "montagne",
            "mer montagne",
            "note:5",
            "drapeau:rejetée",
            "drapeau:aucun",
            "libellé:vert",
            "label:blue",
            "date:2023",
            "date:2024-07..",
            "date:..2024-05",
            "date:2024-06-16",
            "iso:800..",
            "iso:100..200",
            "focale:50..",
            "boîtier:canon",
            "objectif:85",
            "mot:me",
        ];
        for q in no {
            assert!(
                !Query::parse(q).unwrap().matches(&e),
                "« {q} » ne devrait pas trouver la photo"
            );
        }
    }

    #[test]
    fn photos_without_exif_only_pass_open_ranges() {
        let mut e = photo("scan.png", "2024-06-15T10:00:00");
        e.iso = None;
        e.focal_length = None;
        assert!(Query::parse("iso:..").unwrap().matches(&e));
        assert!(!Query::parse("iso:..800").unwrap().matches(&e));
        assert!(!Query::parse("focale:24..70").unwrap().matches(&e));
    }

    #[test]
    fn mistakes_are_named() {
        for (q, err) in [
            ("couleur:rouge", "filtre inconnu « couleur »"),
            ("note:beaucoup", "filtre « note:beaucoup » mal écrit"),
            ("drapeau:vert", "filtre « drapeau:vert » mal écrit"),
            ("iso:cent", "filtre « iso:cent » mal écrit"),
            ("date:hier", "filtre « date:hier » mal écrit"),
        ] {
            assert_eq!(Query::parse(q).unwrap_err(), err);
        }
        // Un deux‑points en tête n'est pas un filtre.
        assert_eq!(Query::parse(":)").unwrap().words, [":)"]);
    }
}
