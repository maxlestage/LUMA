//! Les langues du site : le français (la langue du code, aux adresses d'origine),
//! l'anglais sous `/en/…` et l'espagnol sous `/es/…`, avec les mêmes adresses après le
//! préfixe.
//!
//! Les textes sont écrits en français dans le code ; [`t`] les traduit d'après deux
//! catalogues (`luma-i18n`) : `i18n/islands.txt`, les textes des îlots, compilé aussi
//! dans le bundle du navigateur pour que l'hydratation retrouve les mêmes textes, et
//! `i18n/pages.txt`, ceux des pages, côté serveur seulement.

use std::sync::OnceLock;

use luma_i18n::Catalog;
pub use luma_i18n::Lang;

/// Le catalogue des îlots (serveur et navigateur).
pub const ISLANDS: &str = include_str!("../i18n/islands.txt");

/// Le catalogue des pages (serveur).
#[cfg(not(target_arch = "wasm32"))]
pub const PAGES: &str = include_str!("../i18n/pages.txt");

/// Le texte des catalogues connus de cette cible, bout à bout.
#[cfg(not(target_arch = "wasm32"))]
pub fn source() -> String {
    format!("{ISLANDS}\n\n{PAGES}")
}

/// Le texte des catalogues connus de cette cible : celui des îlots.
#[cfg(target_arch = "wasm32")]
pub fn source() -> String {
    ISLANDS.to_owned()
}

/// Le catalogue, lu une fois.
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::parse(&source()))
}

/// Le texte français `fr` dans la langue `lang` (le français, sans traduction).
pub fn t(lang: Lang, fr: &str) -> &str {
    catalog().get(lang, fr)
}

/// La langue transmise à un îlot dans ses props (`"en"`) ; le français sinon.
pub fn from_props(props: &str) -> Lang {
    Lang::from_code(props).unwrap_or_default()
}

/// Le préfixe des adresses d'une langue : rien pour le français, `/en`, `/es`.
pub const fn prefix(lang: Lang) -> &'static str {
    match lang {
        Lang::Fr => "",
        Lang::En => "/en",
        Lang::Es => "/es",
    }
}

/// La locale Open Graph : `fr_FR`, `en_US`, `es_ES`.
pub const fn locale(lang: Lang) -> &'static str {
    match lang {
        Lang::Fr => "fr_FR",
        Lang::En => "en_US",
        Lang::Es => "es_ES",
    }
}

/// L'adresse d'une page du site dans `lang` : `/conditions/` → `/en/conditions/`.
/// Les adresses communes à toutes les langues (fichiers, abonnement, API) restent telles
/// quelles.
pub fn path(lang: Lang, page: &str) -> String {
    if neutral(page) {
        page.to_owned()
    } else {
        format!("{}{page}", prefix(lang))
    }
}

/// Une adresse commune à toutes les langues ?
pub fn neutral(page: &str) -> bool {
    [
        "/abonnement/",
        "/api/",
        "/assets/",
        "/download/",
        "/pkg/",
        "/js/",
        "/vendor/",
        "/photo",
        "/sitemap.xml",
        "/robots.txt",
        "/sw.js",
    ]
    .iter()
    .any(|p| page.starts_with(p))
}

/// La langue d'une adresse et l'adresse sans son préfixe : `/es/conditions/` →
/// (`Es`, `/conditions/`).
pub fn split(path: &str) -> (Lang, &str) {
    for lang in [Lang::En, Lang::Es] {
        let p = prefix(lang);
        if let Some(rest) = path.strip_prefix(p) {
            if rest.is_empty() {
                return (lang, "/");
            }
            if rest.starts_with('/') {
                return (lang, rest);
            }
        }
    }
    (Lang::Fr, path)
}

/// La langue d'une page sans préfixe demandée avec cet `Accept-Language` : celle du
/// visiteur si LÜMA la parle, l'anglais s'il en demande une autre, le français sans
/// en-tête.
pub fn negotiate(accept_language: Option<&str>) -> Lang {
    match accept_language.map(str::trim).filter(|h| !h.is_empty()) {
        None => Lang::Fr,
        Some(h) => Lang::negotiate(h).unwrap_or(Lang::En),
    }
}

/// Remplace `{nom}` par `valeur` dans un texte traduit.
pub fn fill(text: &str, vars: &[(&str, &str)]) -> String {
    vars.iter().fold(text.to_owned(), |s, (k, v)| {
        s.replace(&format!("{{{k}}}"), v)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_carry_the_language() {
        assert_eq!(path(Lang::Fr, "/conditions/"), "/conditions/");
        assert_eq!(path(Lang::En, "/"), "/en/");
        assert_eq!(
            path(Lang::Es, "/docs/architecture/"),
            "/es/docs/architecture/"
        );
        assert_eq!(path(Lang::En, "/abonnement/"), "/abonnement/");
        assert_eq!(split("/en/docs/boitiers/"), (Lang::En, "/docs/boitiers/"));
        assert_eq!(split("/es"), (Lang::Es, "/"));
        assert_eq!(split("/ensemble/"), (Lang::Fr, "/ensemble/"));
        assert_eq!(split("/nouveautes/"), (Lang::Fr, "/nouveautes/"));
    }

    #[test]
    fn the_language_is_negotiated() {
        assert_eq!(negotiate(None), Lang::Fr);
        assert_eq!(negotiate(Some("")), Lang::Fr);
        assert_eq!(negotiate(Some("fr-FR,fr;q=0.9")), Lang::Fr);
        assert_eq!(negotiate(Some("es-MX,es;q=0.9,en;q=0.8")), Lang::Es);
        assert_eq!(negotiate(Some("de-DE,de;q=0.9")), Lang::En);
        assert_eq!(negotiate(Some("en-GB")), Lang::En);
        assert_eq!(from_props("es"), Lang::Es);
        assert_eq!(from_props(""), Lang::Fr);
    }
}

/// Le catalogue est complet : chaque texte du site a sa traduction, et les textes des
/// îlots sont dans le catalogue du navigateur.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod catalog_tests {
    use super::*;
    use crate::content::*;
    use crate::server::docs::{CAMERAS_INTRO_KEY, DOCS};

    /// Les textes identiques dans toutes les langues, sans entrée au catalogue.
    const SAME: [&str; 9] = [
        "1.0",
        "Alpha",
        "Apple",
        "Apple Pencil",
        "Display P3",
        "HDR / EDR",
        "Linux · Windows",
        "Metal",
        "RAW",
    ];

    const STRIPE_ERRORS: [&str; 12] = [
        "le paiement en ligne n'est pas encore ouvert",
        "le portail client n'est pas encore ouvert",
        "lien de paiement invalide",
        "le paiement n'est pas terminé",
        "ce paiement n'est pas un abonnement LÜMA",
        "l'abonnement n'est plus actif",
        "abonnement sans période",
        "abonnement sans identifiant",
        "indiquez votre courriel et votre code",
        "ce courriel et ce code ne correspondent à aucun accès offert",
        "cet accès offert a pris fin",
        "le serveur de licences n'est pas encore configuré",
    ];

    /// Les textes des îlots (rendus aussi dans le navigateur).
    fn island_texts() -> Vec<String> {
        let mut keys = luma_i18n::literals(include_str!("islands.rs"), "t");
        keys.extend(luma_i18n::literals(include_str!("client/lab.rs"), "t"));
        keys.extend(PRESETS.iter().map(|p| p.1.to_owned()));
        keys.extend(CONTROLS.iter().map(|c| c.1.to_owned()));
        keys.extend(PRIORITIES.iter().map(|p| p.1.to_owned()));
        keys.extend(CRATES.iter().map(|c| c.text.to_owned()));
        keys.push("IL".into());
        keys
    }

    /// Les textes des pages : les appels `t(…)` et les données de `content.rs`.
    fn page_texts() -> Vec<String> {
        let mut keys = Vec::new();
        for src in [
            include_str!("server/pages.rs"),
            include_str!("server/mod.rs"),
            include_str!("server/docs.rs"),
        ] {
            keys.extend(luma_i18n::literals(src, "t"));
        }
        let mut add = |s: &str| keys.push(s.to_owned());
        add(TITLE);
        add(DESCRIPTION);
        add(MANIFESTO);
        add(CAMERAS_INTRO_KEY);
        NAV.iter().for_each(|n| add(n.1));
        for (name, price, period, note, _) in PLANS {
            [name, price, period, note].into_iter().for_each(&mut add);
        }
        PREMIUM.into_iter().for_each(&mut add);
        MARQUEE.into_iter().for_each(&mut add);
        for s in &STATS {
            add(s.unit);
            add(s.text);
        }
        for (name, detail, tag) in PIPELINE {
            [name, detail, tag].into_iter().for_each(&mut add);
        }
        for (title, body) in APPLE {
            add(title);
            add(body);
        }
        for (_, phase, detail, _) in PLATFORMS {
            add(phase);
            add(detail);
        }
        for p in &PERF {
            add(p.name);
            add(p.unit);
            if let Some((note, _)) = p.note {
                add(note);
            }
        }
        for (title, detail) in PHONE_GAINS {
            add(title);
            add(detail);
        }
        for (_, title, body) in STEPS {
            add(title);
            add(body);
        }
        FILMS.iter().for_each(|f| add(f.1));
        for (title, body) in PRIVACY {
            add(title);
            add(body);
        }
        for (question, answer) in FAQ {
            add(question);
            add(answer);
        }
        for (_, tag, title, body) in RECENT {
            [tag, title, body].into_iter().for_each(&mut add);
        }
        COMPARE_ROWS.into_iter().for_each(&mut add);
        COMPARE_LUMA.into_iter().for_each(&mut add);
        for (_, cells) in COMPARE_OTHERS {
            cells.into_iter().for_each(&mut add);
        }
        for (title, body) in HELP_STEPS {
            add(title);
            add(body);
        }
        for (title, body, _) in HELP_LINKS {
            add(title);
            add(body);
        }
        for (title, links) in FOOTER {
            add(title);
            links.iter().for_each(|l| add(l.0));
        }
        for page in &LEGAL {
            add(page.title);
            add(page.lede);
            for (_, title, paras) in page.sections {
                add(title);
                for para in *paras {
                    add(para
                        .strip_prefix("[[")
                        .and_then(|s| s.strip_suffix("]]"))
                        .unwrap_or(para));
                }
            }
        }
        for doc in &DOCS {
            add(doc.title);
            add(doc.lede);
        }
        // Les erreurs que la page de la clé affiche (`server/mod.rs`, `server/stripe.rs`,
        // `server/offerts.rs`).
        for e in STRIPE_ERRORS {
            assert!(
                include_str!("server/stripe.rs").contains(e)
                    || include_str!("server/mod.rs").contains(e)
                    || include_str!("server/offerts.rs").contains(e),
                "{e}"
            );
            add(e);
        }
        keys
    }

    fn missing(catalog: &Catalog, keys: &[String]) -> Vec<String> {
        let mut out: Vec<String> = keys
            .iter()
            .filter(|k| !catalog.has(k) && !SAME.contains(&k.as_str()))
            .cloned()
            .collect();
        out.sort();
        out.dedup();
        out
    }

    fn placeholders(s: &str) -> Vec<&str> {
        let mut v: Vec<&str> = s
            .split('{')
            .skip(1)
            .filter_map(|p| p.split_once('}').map(|x| x.0))
            .collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn the_catalogs_are_well_formed() {
        assert_eq!(Catalog::check(ISLANDS), [], "i18n/islands.txt");
        assert_eq!(Catalog::check(PAGES), [], "i18n/pages.txt");
        assert_eq!(
            Catalog::check(&source()),
            [],
            "une clé dans les deux catalogues"
        );
        let c = catalog();
        for key in c.keys() {
            for lang in [Lang::En, Lang::Es] {
                let tr = c.get(lang, key);
                assert_eq!(
                    placeholders(key),
                    placeholders(tr),
                    "{} : {key}",
                    lang.code()
                );
                assert_eq!(
                    key.matches('*').count() % 2,
                    tr.matches('*').count() % 2,
                    "{} : marques * de {key}",
                    lang.code()
                );
                assert!(!tr.is_empty(), "{key}");
            }
        }
    }

    #[test]
    fn every_island_text_is_in_the_browser_catalog() {
        let islands = Catalog::parse(ISLANDS);
        let missing = missing(&islands, &island_texts());
        assert!(
            missing.is_empty(),
            "à traduire dans i18n/islands.txt :\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn every_page_text_is_translated() {
        let missing = missing(catalog(), &page_texts());
        assert!(
            missing.is_empty(),
            "à traduire dans i18n/pages.txt :\n{}",
            missing.join("\n")
        );
    }

    #[test]
    fn no_entry_is_left_unused() {
        let mut used = island_texts();
        used.extend(page_texts());
        let unused: Vec<&str> = catalog()
            .keys()
            .filter(|k| !used.iter().any(|u| u == k))
            .collect();
        assert!(unused.is_empty(), "entrées inutilisées : {unused:?}");
    }
}
