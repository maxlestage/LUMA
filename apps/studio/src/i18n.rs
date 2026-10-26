//! Les langues de l'app : français (la langue du code), anglais et espagnol.
//!
//! Chaque texte de l'interface est écrit en français dans le code et passe par [`tr`],
//! qui le cherche dans le catalogue `apps/studio/i18n.txt` (le texte français est la
//! clé). Les textes à trous s'écrivent `{nom}` et passent par [`trf`].
//!
//! La langue est choisie au démarrage (`main.rs`, voir [`startup`]) : `LUMA_LANG`, sinon
//! le choix fait dans l'app (`langue.txt`, à côté de `licence.json`), sinon celle du
//! système. Sans rien de tout cela (les tests), l'app parle français.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

pub use luma_i18n::{Catalog, Lang};

/// Le catalogue, tel qu'il est compilé dans l'app.
pub const SOURCE: &str = include_str!("../i18n.txt");

/// La langue en cours : son rang dans [`Lang::ALL`] (0, le français, par défaut).
static CURRENT: AtomicU8 = AtomicU8::new(0);

/// La langue de l'interface.
pub fn lang() -> Lang {
    Lang::ALL
        .get(usize::from(CURRENT.load(Ordering::Relaxed)))
        .copied()
        .unwrap_or_default()
}

/// Change la langue de l'interface (dès l'image suivante).
pub fn set_lang(lang: Lang) {
    let k = Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0);
    CURRENT.store(k as u8, Ordering::Relaxed);
}

/// Le catalogue, lu une fois.
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::parse(SOURCE))
}

/// Le texte français `fr`, dans la langue de l'interface (tel quel, sans traduction).
/// Un texte ambigu porte son contexte après `|`, qui ne s'affiche pas :
/// `tr("Teinte|mélangeur")` (Hue) n'est pas `tr("Teinte")` (Tint).
pub fn tr(fr: &str) -> &str {
    let text = catalog().get(lang(), fr);
    text.split_once('|').map_or(text, |(t, _)| t)
}

/// Un texte à trous : `trf("Essai · {n} j", &[("n", &5)])`.
pub fn trf(fr: &str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut text = tr(fr).to_owned();
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), &value.to_string());
    }
    text
}

/// Les nombres s'écrivent‑ils avec une virgule (français, espagnol) ?
pub fn decimal_comma() -> bool {
    lang() != Lang::En
}

/// Le fichier du choix fait dans l'app.
fn saved_path() -> Option<std::path::PathBuf> {
    Some(crate::dialogs::app_dir()?.join("langue.txt"))
}

/// La langue choisie dans l'app, si elle l'a été.
pub fn saved() -> Option<Lang> {
    Lang::from_code(&std::fs::read_to_string(saved_path()?).ok()?)
}

/// Choisit la langue dans l'app : elle s'applique tout de suite et reste au prochain
/// lancement.
pub fn choose(lang: Lang) {
    set_lang(lang);
    if let Some(p) = saved_path() {
        let _ = std::fs::write(p, lang.code());
    }
}

/// La langue au démarrage : `LUMA_LANG`, sinon le choix fait dans l'app, sinon celle
/// du système.
pub fn startup() -> Lang {
    if let Some(l) = std::env::var("LUMA_LANG")
        .ok()
        .and_then(|v| Lang::negotiate(&v))
    {
        return l;
    }
    saved().unwrap_or_else(system)
}

/// La langue du système : les variables de locale, puis sur Mac les préférences du
/// système (une app lancée du Finder ne reçoit pas `LANG`). Une langue que LÜMA ne
/// parle pas donne l'anglais.
pub fn system() -> Lang {
    detect(|k| std::env::var(k).ok(), apple_languages)
}

/// Le choix de [`system`], à partir des variables `env` et des langues du Mac `apple`.
pub fn detect(
    env: impl Fn(&str) -> Option<String>,
    apple: impl FnOnce() -> Option<String>,
) -> Lang {
    // L'ordre de gettext : LANGUAGE (une liste de préférences), puis LC_ALL,
    // LC_MESSAGES et LANG. « C » et « POSIX » ne disent rien de la langue.
    for var in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = env(var) else { continue };
        let value = value.trim();
        if value.is_empty() || value == "C" || value == "POSIX" || value.starts_with("C.") {
            continue;
        }
        return Lang::negotiate(value).unwrap_or(Lang::En);
    }
    apple()
        .and_then(|list| Lang::negotiate(&list))
        .unwrap_or(Lang::En)
}

/// Les langues préférées du Mac (`defaults read -g AppleLanguages`), en liste :
/// `fr-FR,en-US`.
fn apple_languages() -> Option<String> {
    // Sur iPhone et iPad, pas de `defaults` : le système le dit directement.
    if cfg!(target_os = "ios") {
        return luma_apple::preferred_languages();
    }
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        .ok()?;
    // La sortie est une liste à la NeXTSTEP : ( "fr-FR", "en-US" ).
    let text = String::from_utf8_lossy(&out.stdout);
    let list: String = text
        .chars()
        .filter(|c| !matches!(c, '(' | ')' | '"') && !c.is_whitespace())
        .collect();
    (!list.is_empty()).then_some(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(vars: &'a [(&str, &str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| vars.iter().find(|v| v.0 == k).map(|v| v.1.to_owned())
    }

    #[test]
    fn the_system_language_is_detected() {
        let none = || None;
        assert_eq!(detect(env(&[("LANG", "es_ES.UTF-8")]), none), Lang::Es);
        assert_eq!(detect(env(&[("LANG", "fr_FR.UTF-8")]), none), Lang::Fr);
        // Une langue que LÜMA ne parle pas : l'anglais.
        assert_eq!(detect(env(&[("LANG", "de_DE.UTF-8")]), none), Lang::En);
        assert_eq!(
            detect(env(&[("LANGUAGE", "de:es"), ("LANG", "fr_FR")]), none),
            Lang::Es
        );
        assert_eq!(
            detect(env(&[("LC_ALL", "C"), ("LANG", "fr_FR.UTF-8")]), none),
            Lang::Fr
        );
        // Sans locale (une app du Finder) : les préférences du Mac.
        assert_eq!(
            detect(env(&[]), || Some("de-DE,es-ES,en-US".into())),
            Lang::Es
        );
        assert_eq!(detect(env(&[("LANG", "C.UTF-8")]), none), Lang::En);
    }

    #[test]
    fn french_is_the_default() {
        // Aucun test de la bibliothèque ne change la langue : les captures restent en
        // français.
        assert_eq!(lang(), Lang::Fr);
        assert_eq!(tr("Exporter"), "Exporter");
        assert_eq!(tr("Teinte|mélangeur"), "Teinte");
        assert_eq!(trf("Essai · {n} j", &[("n", &3)]), "Essai · 3 j");
    }
}
