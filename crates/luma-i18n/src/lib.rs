//! Les langues de LÜMA : français (la langue du code), anglais et espagnol.
//!
//! Un catalogue est un fichier texte, une entrée par paragraphe :
//!
//! ```text
//! # Un commentaire.
//! fr = Gérer mon abonnement
//! en = Manage my subscription
//! es = Gestionar mi suscripción
//! ```
//!
//! Le texte français, tel qu'écrit dans le code, est la clé : `catalog.get(lang, "Gérer
//! mon abonnement")`. Sans traduction, le texte français est rendu tel quel. Dans les
//! valeurs, `\n` est un retour à la ligne. Les textes à trous les écrivent `{nom}`, que
//! l'appelant remplace après la traduction.

use std::collections::HashMap;

/// Une langue de LÜMA.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Lang {
    #[default]
    Fr,
    En,
    Es,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::Fr, Lang::En, Lang::Es];

    /// Le code ISO 639‑1 : `fr`, `en`, `es`.
    pub const fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
            Lang::Es => "es",
        }
    }

    /// Le nom de la langue, dans la langue elle‑même.
    pub const fn name(self) -> &'static str {
        match self {
            Lang::Fr => "Français",
            Lang::En => "English",
            Lang::Es => "Español",
        }
    }

    /// La langue d'un code ou d'une locale (`en`, `en-US`, `es_ES.UTF-8`…) ; `None` si
    /// LÜMA ne la parle pas.
    pub fn from_code(code: &str) -> Option<Lang> {
        let c = code.trim().get(..2)?.to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == c)
    }

    /// La première langue parlée par LÜMA dans une liste de préférences
    /// (`Accept-Language`, `LANGUAGE`…) : `fr-CA,en;q=0.8`, `es:en`.
    pub fn negotiate(list: &str) -> Option<Lang> {
        list.split([',', ':'])
            .filter_map(|p| Lang::from_code(p.split(';').next().unwrap_or("")))
            .next()
    }

    fn index(self) -> Option<usize> {
        match self {
            Lang::Fr => None,
            Lang::En => Some(0),
            Lang::Es => Some(1),
        }
    }
}

/// Un catalogue de traductions.
#[derive(Default)]
pub struct Catalog {
    map: HashMap<String, [String; 2]>,
}

/// Une entrée mal formée d'un catalogue.
#[derive(Debug, PartialEq)]
pub struct Malformed {
    pub line: usize,
    pub reason: String,
}

fn unescape(s: &str) -> String {
    s.trim().replace("\\n", "\n")
}

impl Catalog {
    /// Lit un catalogue ; les entrées mal formées sont ignorées (voir [`Catalog::check`]).
    pub fn parse(src: &str) -> Catalog {
        Self::read(src).0
    }

    /// Lit un catalogue et dit ce qui ne va pas : entrée incomplète, clé en double…
    pub fn check(src: &str) -> Vec<Malformed> {
        Self::read(src).1
    }

    fn read(src: &str) -> (Catalog, Vec<Malformed>) {
        let mut map = HashMap::new();
        let mut errors = Vec::new();
        let mut entry: [Option<String>; 3] = Default::default();
        let mut start = 0;
        let mut flush =
            |entry: &mut [Option<String>; 3], start: usize, errors: &mut Vec<Malformed>| match std::mem::take(entry) {
                [None, None, None] => {}
                [Some(fr), Some(en), Some(es)] => {
                    if map.insert(fr.clone(), [en, es]).is_some() {
                        errors.push(Malformed {
                            line: start,
                            reason: format!("clé en double : {fr}"),
                        });
                    }
                }
                _ => errors.push(Malformed {
                    line: start,
                    reason: "il faut fr, en et es".into(),
                }),
            };
        for (i, line) in src.lines().enumerate() {
            let l = line.trim_start();
            if l.trim().is_empty() {
                flush(&mut entry, start, &mut errors);
                continue;
            }
            if l.starts_with('#') {
                continue;
            }
            let slot = match l.split_once('=') {
                Some(("fr ", v)) | Some(("fr", v)) => Some((0, v)),
                Some(("en ", v)) | Some(("en", v)) => Some((1, v)),
                Some(("es ", v)) | Some(("es", v)) => Some((2, v)),
                _ => None,
            };
            match slot {
                Some((k, v)) => {
                    if entry.iter().all(Option::is_none) {
                        start = i + 1;
                    }
                    if entry[k].replace(unescape(v)).is_some() {
                        errors.push(Malformed {
                            line: i + 1,
                            reason: "langue en double dans l'entrée".into(),
                        });
                    }
                }
                None => errors.push(Malformed {
                    line: i + 1,
                    reason: format!("ligne inconnue : {line}"),
                }),
            }
        }
        flush(&mut entry, start, &mut errors);
        (Catalog { map }, errors)
    }

    /// Le texte `fr`, dans la langue `lang` (ou en français, sans traduction).
    pub fn get<'a>(&'a self, lang: Lang, fr: &'a str) -> &'a str {
        match lang.index() {
            None => fr,
            Some(i) => self.map.get(fr).map_or(fr, |t| t[i].as_str()),
        }
    }

    /// A‑t‑il une traduction de `fr` ?
    pub fn has(&self, fr: &str) -> bool {
        self.map.contains_key(fr)
    }

    /// Les textes français du catalogue.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Pour les tests : les chaînes littérales passées à `func(` (ou `func!(`) dans un
/// source Rust, déséchappées, dans l'ordre (`literals(src, "tr")`). Ne voit que les
/// appels dont le premier argument (ou le dernier, `tr(lang, "…")`) est une chaîne.
pub fn literals(src: &str, func: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while let Some(p) = src[i..].find(func) {
        let at = i + p;
        i = at + func.len();
        // Un vrai appel : pas la fin d'un autre identifiant.
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        let mut j = i;
        if bytes.get(j) == Some(&b'!') {
            j += 1;
        }
        if bytes.get(j) != Some(&b'(') {
            continue;
        }
        j += 1;
        // Le premier argument chaîne avant la parenthèse fermante (on saute `lang, `).
        let mut depth = 0;
        while j < bytes.len() {
            match bytes[j] {
                b'"' => break,
                b'(' => depth += 1,
                b')' if depth == 0 => break,
                b')' => depth -= 1,
                _ => {}
            }
            j += 1;
        }
        if bytes.get(j) != Some(&b'"') {
            continue;
        }
        let mut s = String::new();
        let mut k = j + 1;
        let mut chars = src[k..].char_indices();
        while let Some((o, c)) = chars.next() {
            match c {
                '"' => {
                    k += o + 1;
                    break;
                }
                '\\' => match chars.next().map(|x| x.1) {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('\n') => {
                        // Continuation de ligne : les blancs du début sont sautés.
                        while chars.clone().next().is_some_and(|x| x.1.is_whitespace()) {
                            chars.next();
                        }
                    }
                    Some(c) => s.push(c),
                    None => {}
                },
                c => s.push(c),
            }
        }
        i = k.max(i);
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str =
        "# Essai\nfr = Bonjour\nen = Hello\nes = Hola\n\nfr = Deux\\nlignes\nen = Two\\nlines\nes = Dos\\nlíneas\n";

    #[test]
    fn a_catalog_translates_and_falls_back_to_french() {
        let c = Catalog::parse(SRC);
        assert!(Catalog::check(SRC).is_empty());
        assert_eq!(c.get(Lang::En, "Bonjour"), "Hello");
        assert_eq!(c.get(Lang::Es, "Bonjour"), "Hola");
        assert_eq!(c.get(Lang::Fr, "Bonjour"), "Bonjour");
        assert_eq!(c.get(Lang::En, "Deux\nlignes"), "Two\nlines");
        assert_eq!(c.get(Lang::En, "Inconnu"), "Inconnu");
    }

    #[test]
    fn malformed_entries_are_reported() {
        let e = Catalog::check("fr = A\nen = A\n\nfr = B\nen = B\nes = B\n\nfr = B\nen = C\nes = C\n");
        assert_eq!(e.len(), 2, "{e:?}");
    }

    #[test]
    fn languages_are_negotiated() {
        assert_eq!(Lang::negotiate("de-DE,es;q=0.8,en"), Some(Lang::Es));
        assert_eq!(Lang::from_code("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(Lang::negotiate("de"), None);
    }

    #[test]
    fn literals_are_found() {
        let src = r#"ui.label(tr("Bonjour")); t(lang, "L'\"art\""); attr("x"); format!("{}", tr("A \
            B"));"#;
        assert_eq!(literals(src, "tr"), ["Bonjour", "A B"]);
        assert_eq!(literals(src, "t"), ["L'\"art\""]);
    }
}
