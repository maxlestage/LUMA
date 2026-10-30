//! Rechercher dans la photothèque : un texte libre et des filtres (note, drapeau,
//! libellé, date, ISO, focale, boîtier, objectif, mot‑clé).
//!
//! Une recherche s'écrit aussi en une ligne, mots et filtres mêlés :
//!
//! ```text
//! mer note:3 drapeau:retenue date:2024-06..2024-08 iso:..800 boîtier:sony
//! ```
//!
//! La note se donne en minimum (`note:3`), en plage (`note:..2`, `note:2..4`) ou
//! exactement (`note:=0`, les photos pas encore notées) ; `sans:` trouve ce qui
//! manque encore : `sans:note`, `sans:mot` (aucun mot‑clé), `sans:lieu` (pas de
//! position), `sans:libellé`.
//!
//! Une date se donne aussi par rapport à aujourd'hui : `date:30j` (les trente derniers
//! jours, aujourd'hui compris), `date:aujourd'hui`, `date:hier` ; une collection
//! dynamique ainsi faite suit les jours.
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

/// Ce qu'une photo n'a pas encore, pour `sans:`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Rating,
    Keywords,
    Location,
    Label,
}

/// Une recherche. `Query::default()` trouve tout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// Les mots libres, déjà repliés (minuscules, sans accents).
    pub words: Vec<String>,
    /// Note minimale et maximale.
    pub min_rating: u8,
    pub max_rating: Option<u8>,
    /// Ce qui doit manquer (`sans:`).
    pub missing: Vec<Missing>,
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
    /// `sans`/`without` (`note`, `mot`, `lieu`, `libellé`),
    /// `libellé`/`label`, `date`, `iso`, `focale`/`focal`, `boîtier`/`camera`,
    /// `objectif`/`lens`, `mot`/`keyword`. Un filtre inconnu ou mal écrit est une
    /// erreur, qui le nomme.
    pub fn parse(text: &str) -> Result<Query, String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Query::parse_at(text, now)
    }

    /// [`Query::parse`], aujourd'hui étant le jour de `now` (secondes depuis 1970) pour
    /// les dates relatives.
    pub fn parse_at(text: &str, now: u64) -> Result<Query, String> {
        let mut q = Query::default();
        for token in tokens(text) {
            let token = token.as_str();
            let Some((key, value)) = token.split_once(':').filter(|(k, _)| !k.is_empty()) else {
                q.words.push(fold(token));
                continue;
            };
            let bad = || format!("filtre « {token} » mal écrit");
            match fold(key).as_str() {
                "note" | "rating" => {
                    let n = |v: &str| v.parse::<u8>().map(|n| n.min(5)).map_err(|_| bad());
                    if let Some(exact) = value.strip_prefix('=') {
                        let n = n(exact)?;
                        (q.min_rating, q.max_rating) = (n, Some(n));
                    } else if let Some((a, b)) = value.split_once("..") {
                        if a.is_empty() && b.is_empty() {
                            return Err(bad());
                        }
                        q.min_rating = if a.is_empty() { 0 } else { n(a)? };
                        q.max_rating = if b.is_empty() { None } else { Some(n(b)?) };
                    } else {
                        q.min_rating = n(value.trim_end_matches('+'))?;
                    }
                }
                "sans" | "without" => q
                    .missing
                    .push(match fold(value).replace(['-', '‑'], "").trim_end_matches('s') {
                        "note" | "rating" => Missing::Rating,
                        "mot" | "motcle" | "motscle" | "keyword" => Missing::Keywords,
                        "lieu" | "gps" | "position" | "location" => Missing::Location,
                        "libelle" | "label" => Missing::Label,
                        _ => return Err(bad()),
                    }),
                "drapeau" | "flag" => {
                    q.flag = Some(match fold(value).as_str() {
                        "retenue" | "retenues" | "pick" | "picked" => Flag::Pick,
                        "rejetee" | "rejetees" | "reject" | "rejected" => Flag::Reject,
                        "aucun" | "none" => Flag::None,
                        _ => return Err(bad()),
                    })
                }
                "libelle" | "label" => q.label = Some(fold(value)),
                "date" if let Some(from) = relative_day(value, now) => {
                    // Un seul jour (aujourd'hui, hier), ou du premier des n jours à
                    // aujourd'hui.
                    let to = if matches!(fold(value).as_str(), "hier" | "yesterday") {
                        from.clone()
                    } else {
                        day(now, 0)
                    };
                    (q.date_from, q.date_to) = (Some(from), Some(to));
                }
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
        if e.rating < self.min_rating
            || self.max_rating.is_some_and(|m| e.rating > m)
            || self.flag.is_some_and(|f| f != e.flag())
        {
            return false;
        }
        let lacks = |m: &Missing| match m {
            Missing::Rating => e.rating == 0,
            Missing::Keywords => e.keywords.is_empty(),
            Missing::Location => e.gps.is_none(),
            Missing::Label => e.label.trim().is_empty(),
        };
        if !self.missing.iter().all(lacks) {
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

/// Les morceaux d'une recherche, séparés par les espaces ; entre guillemets droits ou
/// typographiques, les espaces restent (`mot:"coucher de soleil"`), et les guillemets
/// partent.
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' | '“' | '”' | '«' | '»' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Un mot‑clé en filtre de recherche : entre guillemets s'il a des espaces.
pub fn keyword_filter(keyword: &str) -> String {
    filter("mot", keyword)
}

/// Le filtre `key:value`, la valeur entre guillemets si elle a des espaces.
pub fn filter(key: &str, value: &str) -> String {
    if value.chars().any(char::is_whitespace) {
        format!("{key}:\"{value}\"")
    } else {
        format!("{key}:{value}")
    }
}

/// Le jour `back` jours avant celui de `now` (secondes depuis 1970), `AAAA-MM-JJ`.
fn day(now: u64, back: u64) -> String {
    crate::unix_date(now.saturating_sub(back * 86_400))[..10].to_owned()
}

/// Le premier jour d'une date relative : `aujourd'hui`/`today`, `hier`/`yesterday`,
/// `30j`/`30d` (les 30 derniers jours, aujourd'hui compris) ; `None` pour une autre
/// valeur.
fn relative_day(value: &str, now: u64) -> Option<String> {
    let v = fold(value).replace(['\'', '’'], "");
    match v.as_str() {
        "aujourdhui" | "today" => return Some(day(now, 0)),
        "hier" | "yesterday" => return Some(day(now, 1)),
        _ => {}
    }
    let n: u64 = v.strip_suffix('j').or_else(|| v.strip_suffix('d'))?.parse().ok()?;
    (1..=36_500).contains(&n).then(|| day(now, n - 1))
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
            ("date:demain", "filtre « date:demain » mal écrit"),
        ] {
            assert_eq!(Query::parse(q).unwrap_err(), err);
        }
        // Un deux‑points en tête n'est pas un filtre.
        assert_eq!(Query::parse(":)").unwrap().words, [":)"]);
    }

    /// Les dates par rapport à aujourd'hui : les n derniers jours (aujourd'hui
    /// compris, à cheval sur un mois), aujourd'hui, hier ; une valeur hors d'usage est
    /// une erreur.
    #[test]
    fn dates_relative_to_today() {
        // Le 3 mars 2026 à 10 h (UTC).
        let now = 1_772_532_000;
        assert_eq!(crate::unix_date(now), "2026-03-03T10:00:00");
        let photo = |d: &str| Entry {
            path: "/p/a.jpg".into(),
            date: d.into(),
            ..Entry::default()
        };
        let found = |q: &str, d: &str| Query::parse_at(q, now).unwrap().matches(&photo(d));
        assert!(found("date:7j", "2026-02-25T00:00:00"));
        assert!(found("date:7j", "2026-03-03T23:59:59"));
        assert!(!found("date:7j", "2026-02-24T23:59:59"));
        assert!(found("date:7d", "2026-02-25T08:00:00"));
        assert!(found("date:aujourd'hui", "2026-03-03T06:00:00"));
        assert!(found("date:Aujourd’hui", "2026-03-03T06:00:00"));
        assert!(!found("date:today", "2026-03-02T23:00:00"));
        assert!(found("date:hier", "2026-03-02T23:00:00"));
        assert!(!found("date:yesterday", "2026-03-03T00:00:00"));
        assert!(found("date:1j", "2026-03-03T00:00:00"));
        for bad in ["date:0j", "date:demain", "date:7jours"] {
            assert!(Query::parse_at(bad, now).is_err(), "{bad}");
        }
        // Les dates écrites restent ce qu'elles étaient.
        assert_eq!(
            Query::parse_at("date:2024-06", now).unwrap().date_from.as_deref(),
            Some("2024-06")
        );
    }

    /// La note en plage ou exacte, et `sans:` : ce que les photos n'ont pas encore.
    #[test]
    fn ratings_in_a_range_and_what_is_missing() {
        let photo = |rating: u8, keywords: &[&str], gps: bool, label: &str| Entry {
            path: "/p/a.jpg".into(),
            rating,
            keywords: keywords.iter().map(|k| (*k).to_owned()).collect(),
            gps: gps.then_some([48.85, 2.35]),
            label: label.into(),
            ..Entry::default()
        };
        let found = |q: &str, e: &Entry| Query::parse(q).unwrap().matches(e);
        let bare = photo(0, &[], false, "");
        let done = photo(4, &["mer"], true, "Rouge");
        assert!(found("note:=0", &bare) && !found("note:=0", &done));
        assert!(found("note:..2", &bare) && !found("note:..2", &done));
        assert!(found("note:3..4", &done) && !found("note:3..4", &bare));
        assert!(found("note:4+", &done) && found("note:4", &done));
        for q in [
            "sans:note",
            "sans:mot",
            "sans:mots-clés",
            "sans:lieu",
            "without:location",
            "sans:libellé",
        ] {
            assert!(found(q, &bare), "{q}");
            assert!(!found(q, &done), "{q}");
        }
        // Plusieurs manques à la fois : tous.
        let half = photo(3, &[], true, "");
        assert!(found("sans:mot sans:libellé", &half));
        assert!(!found("sans:mot sans:lieu", &half));
        for bad in ["sans:objectif", "note:..", "note:=beaucoup", "note:a..3"] {
            assert!(Query::parse(bad).is_err(), "{bad}");
        }
    }

    /// Entre guillemets, un filtre garde ses espaces : un mot‑clé de plusieurs mots se
    /// retrouve, et un mot libre de même.
    #[test]
    fn quoted_values_keep_their_spaces() {
        let photo = Entry {
            path: "/p/plage.jpg".into(),
            keywords: vec!["Coucher de soleil".into(), "mer".into()],
            ..Entry::default()
        };
        let found = |q: &str| Query::parse(q).unwrap().matches(&photo);
        assert!(found("mot:\"coucher de soleil\""));
        assert!(found("mot:“Coucher de soleil” mer"));
        assert!(!found("mot:coucher"));
        assert!(found(&keyword_filter("Coucher de soleil")));
        assert_eq!(keyword_filter("mer"), "mot:mer");
        // Un boîtier et un objectif de plusieurs mots, retrouvés entiers.
        let shot = Entry {
            path: "/p/a.arw".into(),
            make: "SONY".into(),
            model: "ILCE-7M3".into(),
            lens: "FE 24-70mm F2.8 GM".into(),
            ..Entry::default()
        };
        let q = format!(
            "{} {}",
            filter("boîtier", &shot.camera()),
            filter("objectif", &shot.lens)
        );
        assert!(Query::parse(&q).unwrap().matches(&shot), "{q}");
        assert!(!Query::parse(&filter("objectif", "FE 70-200mm")).unwrap().matches(&shot));
        assert_eq!(tokens("  a  \"b c\" d"), ["a", "b c", "d"]);
    }
}
