//! Les documents du dépôt (`docs/*.md`, `CHANGELOG.md`), servis comme pages du site : le
//! dépôt peut rester privé, la documentation publique vit ici. Le Markdown est rendu
//! par pulldown-cmark ; les liens entre documents deviennent des liens du site, et les
//! liens vers des fichiers du dépôt deviennent du simple texte.

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};

/// Un document : adresse, titre, chapeau, fichier d'origine (pour les liens) et texte.
pub struct Doc {
    pub path: &'static str,
    pub title: &'static str,
    pub lede: &'static str,
    file: &'static str,
    markdown: &'static str,
}

pub const DOCS: [Doc; 5] = [
    Doc {
        path: "/docs/boitiers/",
        title: "Boîtiers pris en charge",
        lede: "La matrice des boîtiers, générée depuis la base du décodeur et vérifiée à chaque version.",
        file: "boitiers.md",
        markdown: include_str!("../../../docs/boitiers.md"),
    },
    Doc {
        path: "/docs/parite-lightroom/",
        title: "Parité avec Lightroom",
        lede: "Le module Développement de Lightroom Classic, panneau par panneau : ce qui est fait, ce qui vient.",
        file: "parite-lightroom.md",
        markdown: include_str!("../../../docs/parite-lightroom.md"),
    },
    Doc {
        path: "/docs/architecture/",
        title: "Architecture",
        lede: "Comment LÜMA est construit : le moteur, le GPU, les apps, et les règles du 100 % Rust.",
        file: "architecture.md",
        markdown: include_str!("../../../docs/architecture.md"),
    },
    Doc {
        path: "/docs/feuille-de-route/",
        title: "Feuille de route",
        lede: "Les étapes, les jalons et les risques, avec où on en est.",
        file: "roadmap.md",
        markdown: include_str!("../../../docs/roadmap.md"),
    },
    Doc {
        path: "/nouveautes/",
        title: "Nouveautés",
        lede: "Ce qui change à chaque version de LÜMA.",
        file: "../CHANGELOG.md",
        markdown: include_str!("../../../CHANGELOG.md"),
    },
];

impl Doc {
    /// Le document en HTML, sans son titre de premier niveau (la page a le sien).
    pub fn html(&self, link: impl Fn(&str) -> String) -> String {
        let parser = Parser::new_ext(
            self.markdown,
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
    let name = file.trim_start_matches("./").trim_start_matches("docs/");
    DOCS.iter()
        .find(|d| {
            d.file == name || d.file.trim_start_matches("../") == name.trim_start_matches("../")
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
        let html = roadmap.html(|p| format!("/LUMA{p}"));
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
        let cams = DOCS[0].html(|p| p.to_owned());
        assert!(cams.contains("ILCE-7M3"));
    }
}
