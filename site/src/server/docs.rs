//! Les documents du dépôt (`docs/*.md`, `CHANGELOG.md`), servis comme pages du site : le
//! dépôt peut rester privé, la documentation publique vit ici. Le Markdown est rendu
//! par pulldown-cmark ; les liens entre documents deviennent des liens du site, et les
//! liens vers des fichiers du dépôt deviennent du simple texte.

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};

use crate::i18n::{Lang, fill, t};

/// Un document : adresse (sans préfixe de langue), titre et chapeau (en français,
/// traduits par le catalogue), fichier d'origine (pour les liens) et texte dans chaque
/// langue (français, anglais, espagnol).
pub struct Doc {
    pub path: &'static str,
    pub title: &'static str,
    pub lede: &'static str,
    file: &'static str,
    markdown: [&'static str; 3],
}

/// La matrice des boîtiers, générée (`luma cameras --markdown`), n'existe qu'en
/// français : ses titres de colonnes et son introduction sont traduits au rendu.
const CAMERAS: &str = include_str!("../../../docs/boitiers.md");

pub const DOCS: [Doc; 5] = [
    Doc {
        path: "/docs/boitiers/",
        title: "Boîtiers pris en charge",
        lede: "La matrice des boîtiers, générée depuis la base du décodeur et vérifiée à chaque version.",
        file: "boitiers.md",
        markdown: [CAMERAS, CAMERAS, CAMERAS],
    },
    Doc {
        path: "/docs/fonctions/",
        title: "Fonctions",
        lede: "Chaque fonction de LÜMA, domaine par domaine : ce qui est fait, ce qui vient.",
        file: "fonctions.md",
        markdown: [
            include_str!("../../../docs/fonctions.md"),
            include_str!("../../../docs/en/fonctions.md"),
            include_str!("../../../docs/es/fonctions.md"),
        ],
    },
    Doc {
        path: "/docs/architecture/",
        title: "Architecture",
        lede: "Comment LÜMA est construit : le moteur, le GPU, les apps, et les règles du 100 % Rust.",
        file: "architecture.md",
        markdown: [
            include_str!("../../../docs/architecture.md"),
            include_str!("../../../docs/en/architecture.md"),
            include_str!("../../../docs/es/architecture.md"),
        ],
    },
    Doc {
        path: "/docs/feuille-de-route/",
        title: "Feuille de route",
        lede: "Les étapes, les jalons et les risques, avec où on en est.",
        file: "roadmap.md",
        markdown: [
            include_str!("../../../docs/roadmap.md"),
            include_str!("../../../docs/en/roadmap.md"),
            include_str!("../../../docs/es/roadmap.md"),
        ],
    },
    Doc {
        path: "/nouveautes/",
        title: "Nouveautés",
        lede: "Ce qui change à chaque version de LÜMA.",
        file: "../CHANGELOG.md",
        markdown: [
            include_str!("../../../CHANGELOG.md"),
            include_str!("../../../docs/en/changelog.md"),
            include_str!("../../../docs/es/changelog.md"),
        ],
    },
];

/// L'introduction générée de la matrice des boîtiers, telle que `luma cameras` l'écrit.
const CAMERAS_INTRO: &str =
    "Généré par `luma cameras --markdown` depuis la base du décodeur ; la CI vérifie que ce
fichier est à jour. **Couleur** : matrice D65 connue (couleur étalonnée).
**Corpus** : un vrai RAW de ce boîtier est développé à chaque test
(`scripts/fetch-corpus.sh`). Les formats DNG (dont iPhone ProRAW) sont lus pour tous
les appareils, même absents de la liste.";

/// Les titres de colonnes de la matrice des boîtiers, en anglais et en espagnol.
const CAMERAS_HEADERS: [(&str, [&str; 2]); 2] = [
    (
        "| Marque | Boîtiers | Couleur étalonnée |",
        [
            "| Brand | Cameras | Calibrated color |",
            "| Marca | Cámaras | Color calibrado |",
        ],
    ),
    (
        "| Boîtier | Couleur | Corpus |",
        ["| Camera | Color | Corpus |", "| Cámara | Color | Corpus |"],
    ),
];

/// La matrice des boîtiers dans `lang` : l'introduction, le résumé et les titres de
/// colonnes traduits ; `false` si l'introduction générée a changé (elle reste alors en
/// français, et la page le dit).
fn cameras_in(lang: Lang, md: &str) -> (String, bool) {
    let i = match lang {
        Lang::Fr => return (md.to_owned(), true),
        Lang::En => 0,
        Lang::Es => 1,
    };
    let mut out = md.to_owned();
    for (fr, tr) in CAMERAS_HEADERS {
        out = out.replace(fr, tr[i]);
    }
    // « **735 boîtiers** de 20 marques ; 714 avec couleur étalonnée ; 4 vérifiés sur le
    // corpus. »
    let summary = out
        .lines()
        .find(|l| l.contains(" boîtiers** de "))
        .unwrap_or_default()
        .to_owned();
    let numbers: Vec<&str> = summary
        .split(|c: char| !c.is_ascii_digit())
        .filter(|n| !n.is_empty())
        .collect();
    if let [cameras, brands, color, corpus] = numbers[..] {
        let vars = [
            ("cameras", cameras),
            ("brands", brands),
            ("color", color),
            ("corpus", corpus),
        ];
        let line = fill(
            t(
                lang,
                "**{cameras} boîtiers** de {brands} marques ; {color} avec couleur étalonnée ; {corpus} vérifiés sur le corpus.",
            ),
            &vars,
        );
        out = out.replace(&summary, &line);
    }
    if out.contains(CAMERAS_INTRO) {
        (out.replace(CAMERAS_INTRO, t(lang, CAMERAS_INTRO_KEY)), true)
    } else {
        (out, false)
    }
}

/// La clé de catalogue de l'introduction (sur une ligne).
pub(crate) const CAMERAS_INTRO_KEY: &str = "Généré par `luma cameras --markdown` depuis la base du décodeur ; la CI vérifie que ce fichier est à jour. **Couleur** : matrice D65 connue (couleur étalonnée). **Corpus** : un vrai RAW de ce boîtier est développé à chaque test (`scripts/fetch-corpus.sh`). Les formats DNG (dont iPhone ProRAW) sont lus pour tous les appareils, même absents de la liste.";

impl Doc {
    /// Le Markdown du document dans `lang`.
    fn markdown(&self, lang: Lang) -> std::borrow::Cow<'static, str> {
        let md = self.markdown[Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0)];
        if self.file == "boitiers.md" && lang != Lang::Fr {
            return cameras_in(lang, md).0.into();
        }
        md.into()
    }

    /// Le document n'est-il pas traduit dans `lang` (rendu en français) ?
    pub fn untranslated(&self, lang: Lang) -> bool {
        lang != Lang::Fr && self.file == "boitiers.md" && !cameras_in(lang, CAMERAS).1
    }

    /// Le document en HTML dans `lang`, sans son titre de premier niveau (la page a le
    /// sien). `link` donne l'adresse d'une page du site (sans préfixe de langue) : les
    /// liens entre documents restent dans la langue de la page.
    pub fn html(&self, lang: Lang, link: impl Fn(&str) -> String) -> String {
        let markdown = self.markdown(lang);
        let parser = Parser::new_ext(
            &markdown,
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
        );
        let mut skip_h1 = false;
        let mut first_h1 = true;
        // Pile des liens ouverts : `true` si le lien est gardé.
        let mut links: Vec<bool> = Vec::new();
        let events = parser.filter_map(|e| match e {
            Event::Start(Tag::Heading {
                level: pulldown_cmark::HeadingLevel::H1,
                ..
            }) if first_h1 => {
                first_h1 = false;
                skip_h1 = true;
                None
            }
            Event::End(TagEnd::Heading(pulldown_cmark::HeadingLevel::H1)) if skip_h1 => {
                skip_h1 = false;
                None
            }
            _ if skip_h1 => None,
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => match resolve(&dest_url, &link) {
                Some(url) => {
                    links.push(true);
                    Some(Event::Start(Tag::Link {
                        link_type,
                        dest_url: CowStr::from(url),
                        title,
                        id,
                    }))
                }
                None => {
                    links.push(false);
                    None
                }
            },
            Event::End(TagEnd::Link) => links
                .pop()
                .unwrap_or(false)
                .then_some(Event::End(TagEnd::Link)),
            e => Some(e),
        });
        let mut out = String::new();
        pulldown_cmark::html::push_html(&mut out, events);
        // Les tableaux larges défilent dans leur cadre, pas la page.
        out.replace("<table>", "<div class=\"table\"><table>")
            .replace("</table>", "</table></div>")
    }
}

/// L'adresse d'un lien de document : une page web telle quelle, un autre document vers
/// sa page, une ancre telle quelle ; un fichier du dépôt n'a pas d'adresse publique.
fn resolve(dest: &str, link: &impl Fn(&str) -> String) -> Option<String> {
    if dest.starts_with("http://")
        || dest.starts_with("https://")
        || dest.starts_with("mailto:")
        || dest.starts_with('#')
    {
        return Some(dest.to_owned());
    }
    let (file, anchor) = dest.split_once('#').map_or((dest, ""), |(f, a)| (f, a));
    // `roadmap.md`, `./docs/roadmap.md`, `../CHANGELOG.md`, `changelog.md` (traduit)…
    let mut name = file;
    loop {
        let rest = ["./", "../", "docs/", "en/", "es/"]
            .iter()
            .find_map(|p| name.strip_prefix(p));
        match rest {
            Some(rest) => name = rest,
            None => break,
        }
    }
    DOCS.iter()
        .find(|d| {
            let own = d.file.trim_start_matches("../");
            !name.contains('/') && own.eq_ignore_ascii_case(name)
        })
        .map(|d| {
            let page = link(d.path);
            if anchor.is_empty() {
                page
            } else {
                format!("{page}#{anchor}")
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_link_to_each_other_and_not_to_the_repository() {
        let roadmap = DOCS.iter().find(|d| d.file == "roadmap.md").unwrap();
        let html = roadmap.html(Lang::Fr, |p| format!("/LUMA{p}"));
        assert!(html.contains("<table>"));
        assert!(
            html.contains("href=\"/LUMA/docs/boitiers/\""),
            "lien vers la matrice des boîtiers"
        );
        assert!(!html.contains("github.com/maxlestage/LUMA/blob"));
        assert!(
            !html.contains(".md\""),
            "aucun lien vers un fichier Markdown"
        );
        assert!(!html.contains("<h1>"));
        let cams = DOCS[0].html(Lang::Fr, |p| p.to_owned());
        assert!(cams.contains("ILCE-7M3"));
    }

    #[test]
    fn translated_documents_link_within_their_language() {
        for lang in [Lang::En, Lang::Es] {
            let prefix = crate::i18n::prefix(lang);
            for doc in &DOCS {
                let html = doc.html(lang, |p| crate::i18n::path(lang, p));
                assert!(!html.contains(".md\""), "{} {}", lang.code(), doc.path);
                for href in html.split("href=\"").skip(1) {
                    let href = &href[..href.find('"').unwrap_or(0)];
                    if href.starts_with('/') {
                        assert!(href.starts_with(prefix), "{} : {href}", doc.path);
                    }
                }
            }
            let roadmap = DOCS.iter().find(|d| d.file == "roadmap.md").unwrap();
            let html = roadmap.html(lang, |p| crate::i18n::path(lang, p));
            assert!(html.contains(&format!("href=\"{prefix}/docs/boitiers/\"")));
        }
    }

    #[test]
    fn the_camera_matrix_is_translated_around_its_table() {
        let cams = &DOCS[0];
        assert!(
            !cams.untranslated(Lang::En),
            "l'introduction générée a changé"
        );
        let en = cams.html(Lang::En, |p| p.to_owned());
        assert!(en.contains("<th>Brand</th>") && en.contains("<th>Camera</th>"));
        assert!(!en.contains("Généré par") && !en.contains("boîtiers</strong> de"));
        assert!(en.contains("ILCE-7M3"));
        let es = cams.html(Lang::Es, |p| p.to_owned());
        assert!(es.contains("<th>Marca</th>") && !es.contains("Couleur étalonnée"));
    }
}
