//! Les pages du site, écrites en vues Active : du HTML fini rendu par le serveur,
//! lisible sans JavaScript, avec ses balises SEO, dans la langue de son adresse
//! (français, `/en/…`, `/es/…`).

use paquets::prelude::*;

use super::{Site, link};
use crate::content::*;
use crate::i18n::{self, Lang, fill, t};
use crate::islands;

const FONTS: &str = "https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700;800&family=Instrument+Serif:ital@0;1&family=JetBrains+Mono:wght@400;500&display=swap";

/// La page rendue : sa langue, et son adresse sans préfixe de langue (`/conditions/`),
/// pour les liens vers ses traductions.
#[derive(Clone, Copy)]
pub struct Here<'a> {
    pub lang: Lang,
    pub path: &'a str,
}

impl<'a> Here<'a> {
    pub fn new(lang: Lang, path: &'a str) -> Here<'a> {
        Here { lang, path }
    }
}

/// Une page du site dans `lang`, derrière le chemin de base.
fn page_link(lang: Lang, path: &str) -> String {
    link(&i18n::path(lang, path))
}

/// Un titre de section : le mot entre `*` ressort en couleur, en italique.
fn heading(tag: fn() -> Element, title: &str) -> Element {
    let mut parts = title.splitn(3, '*');
    let before = parts.next().unwrap_or_default().to_owned();
    let accent = parts.next();
    let after = parts.next().unwrap_or_default().to_owned();
    match accent {
        Some(accent) => tag()
            .text(before)
            .child(em().text(accent.to_owned()))
            .child(text(after)),
        None => tag().text(before),
    }
}

/// Un texte où `` `…` `` est du code et `**…**` du gras.
fn rich(s: &str) -> Vec<Node> {
    let mut out = Vec::<Node>::new();
    for (n, chunk) in s.split('`').enumerate() {
        if n % 2 == 1 {
            out.push(code().text(chunk.to_owned()).into());
            continue;
        }
        for (k, part) in chunk.split("**").enumerate() {
            if k % 2 == 1 {
                out.push(b().text(part.to_owned()).into());
            } else if !part.is_empty() {
                out.push(text(part.to_owned()));
            }
        }
    }
    out
}

fn kicker(label: &str) -> Element {
    p().class("kicker").text(label.to_owned())
}

fn section_head(label: &str, title: Element, lede: Option<Element>) -> Element {
    div()
        .class("section-head reveal")
        .child(kicker(label))
        .child(title)
        .child(lede)
}

/// La marque de LÜMA : l'anneau de lumière, dessiné par `luma-brand` (régénéré par
/// `cargo run -p luma-brand --example brand -- site/public/assets site`).
const BRAND_SVG: &str = include_str!("../../public/assets/marque.svg");

const IRIS_SVG: &str = r##"<svg class="loader-iris" viewBox="-50 -50 100 100"><defs><linearGradient id="spec" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff6b3d"/><stop offset=".35" stop-color="#ffcf4a"/><stop offset=".65" stop-color="#4af2c8"/><stop offset="1" stop-color="#8b7bff"/></linearGradient></defs><circle r="46" fill="none" stroke="url(#spec)" stroke-width="1.2"/><g class="blades"><path d="M0,-44 L38,-22 L12,-6 Z"/><path d="M0,-44 L38,-22 L12,-6 Z"/><path d="M0,-44 L38,-22 L12,-6 Z"/><path d="M0,-44 L38,-22 L12,-6 Z"/><path d="M0,-44 L38,-22 L12,-6 Z"/><path d="M0,-44 L38,-22 L12,-6 Z"/></g></svg>"##;

const APPLE_SVG: &str = r#"<svg class="apple-logo" viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M16.4 12.6c0-2.6 2.1-3.8 2.2-3.9-1.2-1.8-3.1-2-3.7-2-1.6-.2-3.1.9-3.9.9-.8 0-2-.9-3.4-.9-1.7 0-3.3 1-4.2 2.6-1.8 3.1-.5 7.7 1.3 10.2.9 1.2 1.9 2.6 3.2 2.6 1.3-.1 1.8-.8 3.3-.8 1.6 0 2 .8 3.4.8 1.4 0 2.3-1.3 3.1-2.5 1-1.4 1.4-2.8 1.4-2.9 0 0-2.7-1-2.7-4.1zM13.9 4.9c.7-.9 1.2-2 1-3.2-1 .1-2.3.7-3 1.6-.7.8-1.2 2-1.1 3.1 1.2.1 2.3-.6 3.1-1.5z"/></svg>"#;

// ----- Cadre --------------------------------------------------------------------------

fn chrome() -> Element {
    div()
        .child(
            div()
                .class("loader")
                .attr("aria-hidden", "true")
                .child(div().class("loader-art").html(IRIS_SVG))
                .child(div().class("loader-word").text("LÜMA"))
                .child(
                    div()
                        .class("loader-count")
                        .child(span().text("0"))
                        .child(text("%")),
                ),
        )
        .child(div().class("progress").attr("aria-hidden", "true"))
        .child(div().class("cursor").attr("aria-hidden", "true"))
        .child(div().class("cursor-dot").attr("aria-hidden", "true"))
        .child(div().class("grain").attr("aria-hidden", "true"))
}

/// Le sélecteur de langue : la même page dans chaque langue. `full` : les noms des
/// langues (pied de page) plutôt que leurs codes (barre de navigation).
fn lang_switch(here: Here, full: bool) -> Element {
    nav()
        .class(if full {
            "lang-switch lang-switch-full"
        } else {
            "lang-switch"
        })
        .attr("aria-label", t(here.lang, "Langue"))
        .children(Lang::ALL.map(|l| {
            let mut a = a()
                .href(page_link(l, here.path))
                .attr("hreflang", l.code())
                .attr("lang", l.code());
            if l == here.lang {
                a = a.attr("aria-current", "true");
            }
            if full {
                a.text(l.name())
            } else {
                a.attr("aria-label", l.name())
                    .text(l.code().to_ascii_uppercase())
            }
        }))
}

fn navbar(here: Here) -> Element {
    let lang = here.lang;
    header()
        .class("nav")
        .child(
            a().class("brand")
                .href(page_link(lang, "/"))
                .attr("aria-label", t(lang, "LÜMA, accueil"))
                .child(span().class("brand-mark").html(BRAND_SVG))
                .child(span().text("LÜMA")),
        )
        .child(
            nav()
                .class("links")
                .attr("aria-label", t(lang, "Sections"))
                .children(
                    NAV.iter()
                        .map(|&(href, label)| a().href(target(lang, href)).text(t(lang, label))),
                ),
        )
        .child(
            a().class("btn btn-small magnetic")
                .href(target(lang, "#tarifs"))
                .child(span().text(if downloads_open() {
                    t(lang, "Télécharger")
                } else {
                    t(lang, "Tarifs")
                })),
        )
        .child(lang_switch(here, false))
        .child(
            button()
                .class("menu-toggle")
                .attr("type", "button")
                .attr("aria-label", t(lang, "Menu"))
                .attr("aria-expanded", "false")
                .child(i())
                .child(i()),
        )
}

// ----- Sections -----------------------------------------------------------------------

fn hero(lang: Lang) -> Element {
    let title =
        span()
            .class("split")
            .attr("aria-hidden", "true")
            .children("LÜMA".chars().enumerate().map(|(n, c)| {
                span()
                    .class("ch")
                    .child(span().style(format!("--i:{n}")).text(c.to_string()))
            }));
    section()
        .class("hero")
        .id("top")
        .child(
            div()
                .class("hero-inner")
                .child(
                    p().class("eyebrow reveal")
                        .child(span().class("dot"))
                        .child(text(format!(" {}", t(lang, "Éditeur photo · Mac, iPhone et iPad · 100 % Rust")))),
                )
                .child(h1().class("hero-title").attr("aria-label", "LÜMA").child(title))
                .child(heading(p, t(lang, "La lumière, *développée* en Rust.")).class("hero-sub reveal"))
                .child(p().class("hero-lede reveal").text(t(lang,
                    "Un moteur RAW 32 bits flottants sur le GPU, 100 films vérifiés, et vos vraies pellicules mesurées sur un scan. Un éditeur photo professionnel, rapide et précis, chiffres à l'appui.",
                )))
                .child(
                    div()
                        .class("hero-cta reveal")
                        .child(a().class("btn magnetic").href("#tarifs").child(span().text(if downloads_open() {
                            t(lang, "Télécharger · 14 jours d'essai")
                        } else {
                            t(lang, "Voir les tarifs")
                        })))
                        .child(a().class("btn btn-ghost magnetic").href("#labo").child(span().text(t(lang, "Voir le moteur en action")))),
                ),
        )
        .child(
            div()
                .class("hero-meta reveal")
                .children(["RAW", t(lang, "32 bits f"), "GPU", t(lang, "100 films")].map(|s| span().text(s))),
        )
        .child(
            a().class("scroll-hint")
                .href("#manifeste")
                .attr("aria-label", t(lang, "Défiler"))
                .child(span()),
        )
}

fn marquee(lang: Lang) -> Element {
    // Deux fois la même liste : la piste défile de la moitié de sa largeur, sans saut.
    let items = |hidden: bool| {
        MARQUEE.iter().flat_map(move |item| {
            let label = if hidden {
                span().attr("aria-hidden", "true").text(t(lang, item))
            } else {
                span().text(t(lang, item))
            };
            [label, b().attr("aria-hidden", "true").text("✦")]
        })
    };
    section()
        .class("marquee")
        .attr("aria-label", t(lang, "Fonctions"))
        .child(
            div()
                .class("marquee-track")
                .children(items(false))
                .children(items(true)),
        )
}

fn manifesto(lang: Lang) -> Element {
    let words = t(lang, MANIFESTO)
        .split_whitespace()
        .map(|w| match w.strip_prefix('*') {
            Some(word) => span().class("w hl").text(word.to_owned()),
            None => span().class("w").text(w.to_owned()),
        });
    let mut text_el = p().class("manifesto-text");
    for (n, word) in words.enumerate() {
        if n > 0 {
            text_el = text_el.child(text(" "));
        }
        text_el = text_el.child(word);
    }
    section()
        .class("manifesto")
        .id("manifeste")
        .child(div().class("manifesto-sticky").child(text_el))
}

fn stats(lang: Lang) -> Element {
    section().class("stats").children(STATS.iter().map(|s| {
        let mut num = div().class("stat-num");
        if !s.prefix.is_empty() {
            num = num.child(small().text(s.prefix));
        }
        num = num
            .child(
                span()
                    .attr("data-count", s.value.to_string())
                    .text(s.value.to_string()),
            )
            .child(small().text(t(lang, s.unit)));
        div()
            .class("stat reveal tilt")
            .child(num)
            .child(p().text(t(lang, s.text)))
    }))
}

fn pipeline(lang: Lang) -> Element {
    let steps = ol()
        .class("pipeline-steps")
        .children(PIPELINE.iter().map(|&(name, detail, _)| {
            li().child(b().text(t(lang, name)))
                .child(text(format!(" {}", t(lang, detail))))
        }));
    let layers = div()
        .class("pipeline-stage")
        .children(PIPELINE.iter().enumerate().map(|(n, &(_, _, tag))| {
            div()
                .class(format!("layer l{n}"))
                .child(span().text(t(lang, tag)))
        }));
    section().class("pipeline").id("pipeline").child(
        div()
            .class("pipeline-sticky")
            .child(
                div()
                    .class("pipeline-copy")
                    .child(kicker(t(lang, "Le moteur")))
                    .child(heading(
                        h2,
                        t(lang, "Un pipeline *non destructif*, couche par couche."),
                    ))
                    .child(steps),
            )
            .child(
                div()
                    .class("pipeline-stage-wrap")
                    .attr("aria-hidden", "true")
                    .child(layers),
            ),
    )
}

fn lab(lang: Lang) -> Element {
    section()
        .class("lab")
        .id("labo")
        .child(section_head(
            t(lang, "Le labo"),
            heading(h2, t(lang, "Bouge un curseur. *Rien* n'est détruit.")),
            Some(p().text(t(lang,
                "Cette démo tourne sur le GPU de ton appareil, pilotée par du Rust compilé en WebAssembly : une scène générée en lumière linéaire (le soleil dépasse le blanc), puis exposition, balance des blancs, hautes lumières, ombres et contraste appliqués comme des paramètres. Glisse la barre pour comparer avec l'original. C'est une illustration du principe, pas encore le moteur LÜMA.",
            ))),
        ))
        // La langue voyage dans les props : le navigateur hydrate le même texte.
        .child(island("lab", lang.code(), islands::lab))
}

fn apple(lang: Lang) -> Element {
    let panels = div()
        .class("apple-panels")
        .children(APPLE.iter().map(|&(title, body)| {
            article()
                .class("apple-panel")
                .child(heading(h2, t(lang, title)))
                .child(p().text(t(lang, body)))
        }));
    section().class("apple").id("apple").child(
        div()
            .class("apple-sticky")
            .child(
                div()
                    .class("devices")
                    .id("devices")
                    .attr("aria-hidden", "true"),
            )
            .child(
                div()
                    .class("apple-copy")
                    .child(
                        p().class("kicker")
                            .child(span().class("kicker-mark").html(APPLE_SVG))
                            .child(text(format!(" {}", t(lang, "macOS, iOS & iPadOS d'abord")))),
                    )
                    .child(panels)
                    .child(div().class("apple-dots").children([i(), i(), i(), i()])),
            ),
    )
}

fn platforms(lang: Lang) -> Element {
    section()
        .class("platforms")
        .child(section_head(
            t(lang, "Ordre de sortie"),
            heading(h2, t(lang, "Apple en premier. *Les autres suivent*, sur le même cœur.")),
            Some(p().text(t(lang,
                "Windows, Linux et Android sont compilés en intégration continue dès le premier mois : le moteur ne doit rien supposer d'Apple. Leurs applications arrivent en phase 6.",
            ))),
        ))
        .child(div().class("platform-grid").children(PLATFORMS.iter().map(
            |&(name, phase, detail, prime)| {
                div()
                    .class(if prime { "platform prime reveal tilt" } else { "platform reveal tilt" })
                    .child(span().class("badge").text(t(lang, phase)))
                    .child(h3().text(name))
                    .child(p().text(t(lang, detail)))
            },
        )))
}

fn architecture(lang: Lang) -> Element {
    section()
        .class("arch")
        .id("architecture")
        .child(section_head(
            t(lang, "Architecture"),
            heading(h2, t(lang, "Un workspace Cargo. *Une crate*, une responsabilité.")),
            Some(p().text(t(lang,
                "Le moteur ne dépend jamais de la plateforme. Seules l'interface et les applications voient macOS, iOS ou iPadOS.",
            ))),
        ))
        .child(island("crates", lang.code(), islands::crates))
}

/// Un nombre à la manière de la langue : « 6,2 » en français et en espagnol, « 6.2 » en
/// anglais.
fn number(lang: Lang, v: f32, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    if lang == Lang::En {
        s
    } else {
        s.replace('.', ",")
    }
}

fn performance(lang: Lang) -> Element {
    let sep = if lang == Lang::En { "." } else { "," };
    let cards = PERF.iter().map(|p| {
        let unit = t(lang, p.unit);
        let measured = div()
            .class("perf-row")
            .child(span().text(t(lang, "Mac, mesuré")))
            .child(
                b().child(
                    i().attr("data-count", format!("{:.*}", p.decimals, p.mac))
                        .attr("data-decimals", p.decimals.to_string())
                        .attr("data-sep", sep)
                        .text(number(lang, p.mac, p.decimals)),
                )
                .child(text(format!(" {unit}"))),
            );
        let ratio = (p.mac / p.mac_target as f32).min(1.0);
        let target = fill(
            t(lang, "Objectif : moins de {v} {unit}."),
            &[("v", &p.mac_target.to_string()), ("unit", unit)],
        );
        let note = p
            .note
            .map(|(n, v)| fill(t(lang, n), &[("v", &number(lang, v, 0))]));
        div()
            .class("perf-card reveal")
            .child(h3().text(t(lang, p.name)))
            .child(measured)
            .child(
                div()
                    .class("bar")
                    .child(span().style(format!("--v:{ratio:.2}"))),
            )
            .child(p_note(match note {
                Some(n) => format!("{target} {n}"),
                None => target,
            }))
            .child(
                div()
                    .class("perf-row perf-phone")
                    .child(span().text(t(lang, "iPhone, objectif")))
                    .child(b().text(format!("< {} {unit}", p.phone_target))),
            )
    });
    let adjust = number(lang, PERF[0].mac, PERF[0].decimals);
    section()
        .class("perf")
        .id("performance")
        .child(section_head(
            t(lang, "Performance"),
            heading(h2, t(lang, "Des chiffres *mesurés*.")),
            Some(p().text(t(lang,
                "Mesurés à chaque version par l'intégration continue, sur un Mac Apple Silicon (machine virtuelle, GPU Metal) et cinq vrais RAW : Sony, Canon, Nikon, Fujifilm, iPhone ProRAW. Ce sont des médianes. Sur iPhone, ce sont encore des objectifs.",
            ))),
        ))
        .child(div().class("perf-grid").children(cards))
        .child(
            div()
                .class("frame-budget reveal")
                .child(p().children(rich(&fill(
                    t(lang,
                        "**8,3 ms** : le budget d'une image à 120 Hz. Un aperçu après réglage en prend **{v} ms** sur le GPU du Mac : l'image suit le curseur, sans retard.",
                    ),
                    &[("v", &adjust)],
                ))))
                .child(
                    div()
                        .class("frames")
                        .attr("aria-hidden", "true")
                        .children((0..40).map(|k| i().style(format!("--k:{k}")))),
                ),
        )
}

/// Ce que LÜMA fait déjà : chaque fonction faite et testée de `docs/fonctions.md`,
/// cochée, domaine par domaine.
fn capabilities(lang: Lang) -> Element {
    let f = super::docs::features(lang);
    let groups = f.groups.iter().map(|(title, items)| {
        article()
            .class("cap-group reveal")
            .child(h3().text(title.clone()))
            .child(
                ul().children(items.iter().enumerate().map(|(k, (panel, feature))| {
                    li().style(format!("--i:{}", k.min(24)))
                        .child(
                            span()
                                .class("cap-check")
                                .attr("aria-hidden", "true")
                                .text("✔"),
                        )
                        .child(
                            span()
                                .child(
                                    panel
                                        .as_ref()
                                        .map(|p| span().class("cap-panel").text(p.clone())),
                                )
                                .children(rich(feature)),
                        )
                })),
            )
    });
    section()
        .class("caps")
        .id("fonctions")
        .child(section_head(
            t(lang, "Fonctions"),
            heading(
                h2,
                &fill(t(lang, "*{n}* fonctions déjà là. Toutes testées."), &[("n", &f.done.to_string())]),
            ),
            Some(p().text(t(lang,
                "Chaque fonction cochée est faite et vérifiée par les tests à chaque version. La liste s'allonge chaque jour.",
            ))),
        ))
        // Sur téléphone, la liste est repliée : « Voir tout » la déplie (sans script).
        .child(
            input()
                .attr("type", "checkbox")
                .id("cap-all")
                .class("cap-toggle"),
        )
        .child(div().class("cap-grid").children(groups))
        .child(
            label()
                .attr("for", "cap-all")
                .class("cap-expand")
                .child(span().class("cap-open").text(fill(
                    t(lang, "Voir les {n} fonctions"),
                    &[("n", &f.done.to_string())],
                )))
                .child(span().class("cap-close").text(t(lang, "Réduire"))),
        )
        .child(
            p().class("cap-more reveal").child(
                a().href(page_link(lang, "/docs/fonctions/")).text(fill(
                    t(lang, "Et ce qui vient : {n} fonctions en préparation →"),
                    &[("n", &f.todo.to_string())],
                )),
            ),
        )
}

/// La ligne d'objectif d'une carte de performance.
fn p_note(text: String) -> Element {
    p().class("perf-note").text(text)
}

fn roadmap(lang: Lang) -> Element {
    let phases = ROADMAP.iter().map(|ph| {
        article()
            .class(if ph.later { "phase later" } else { "phase" })
            .child(span().class("phase-when").text(ph.when))
            .child(h3().text(t(lang, ph.name)))
            .child(ul().children(ph.items.iter().map(|item| li().text(t(lang, item)))))
            .child(p().class("phase-goal").text(t(lang, ph.goal)))
    });
    section().class("roadmap").id("roadmap").child(
        div()
            .class("roadmap-sticky")
            .child(
                div()
                    .class("roadmap-head")
                    .child(kicker(t(lang, "Feuille de route")))
                    .child(heading(
                        h2,
                        t(lang, "24 à 36 mois. *Apple* à chaque étape."),
                    )),
            )
            .child(div().class("roadmap-track").children(phases))
            .child(
                div()
                    .class("roadmap-line")
                    .attr("aria-hidden", "true")
                    .child(span()),
            ),
    )
}

/// Une ligne de `deny.toml` colorée : chaînes, invites, commentaires, sections.
fn deny_line(class: &str, line: &str) -> Node {
    let mut parts = Vec::<Node>::new();
    if let Some(rest) = line.strip_prefix("$ ") {
        parts.push(span().class("n").text("$").into());
        parts.push(text(format!(" {rest}")));
    } else {
        for (n, chunk) in line.split('"').enumerate() {
            if n % 2 == 1 {
                parts.push(span().class("s").text(format!("\"{chunk}\"")).into());
            } else if !chunk.is_empty() {
                parts.push(text(chunk.to_owned()));
            }
        }
    }
    if class.is_empty() {
        fragment(parts)
    } else {
        span().class(class.to_owned()).children(parts).into()
    }
}

fn rust(lang: Lang) -> Element {
    let mut lines = Vec::<Node>::new();
    for (n, &(class, line)) in DENY.iter().enumerate() {
        if n > 0 {
            lines.push(text("\n"));
        }
        // Les commentaires sont traduits ; le code reste du code.
        let line = if class == "c" { t(lang, line) } else { line };
        lines.push(deny_line(class, line));
    }
    section()
        .class("rust")
        .id("rust")
        .child(section_head(
            t(lang, "100 % Rust, vérifiable"),
            heading(h2, t(lang, "Pas une promesse. *Un audit*.")),
            Some(p().children(rich(t(lang,
                "Une crate sur crates.io n'est pas forcément pure Rust. LÜMA vérifie le graphe de dépendances réel à chaque build : aucune `-sys`, aucun `build.rs` qui compile du C, aucun FFI vers une bibliothèque tierce. Les frameworks du système (Metal, AppKit, UIKit, PhotoKit) sont appelés depuis Rust.",
            )))),
        ))
        .child(
            div()
                .class("rust-grid")
                .child(
                    div()
                        .class("terminal reveal")
                        .child(
                            div()
                                .class("terminal-bar")
                                .children([i(), i(), i()])
                                .child(span().text("deny.toml")),
                        )
                        .child(pre().child(code().id("term").children(lines))),
                )
                .child(ul().class("rules").children(
                    RULES.iter().map(|&(name, body)| li().class("reveal").child(b().text(t(lang, name))).child(text(format!(" {}", t(lang, body))))),
                )),
        )
}

fn cta(lang: Lang) -> Element {
    let title = t(lang, "Développer *la lumière*.");
    let mut parts = title.splitn(3, '*');
    let (before, accent, after) = (
        parts.next().unwrap_or_default().trim_end().to_owned(),
        parts.next().unwrap_or_default().to_owned(),
        parts.next().unwrap_or_default().to_owned(),
    );
    section()
        .class("cta")
        .child(div().class("cta-orb").attr("aria-hidden", "true"))
        .child(h2().class("reveal").text(before).child(br()).child(em().text(accent)).child(text(after)))
        .child(p().class("reveal").text(t(lang,
            "D'abord le meilleur moteur photo Rust possible, sur Mac et iPhone. Ensuite, tout le reste.",
        )))
        .child(
            div()
                .class("hero-cta reveal")
                .child(a().class("btn magnetic").href(page_link(lang, "/docs/architecture/")).child(span().text(t(lang, "Lire l'architecture"))))
                .child(a().class("btn btn-ghost magnetic").href(page_link(lang, "/docs/feuille-de-route/")).child(span().text(t(lang, "Feuille de route détaillée")))),
        )
}

/// Tarifs et téléchargement : LÜMA Premium, 14 jours d'essai, puis au mois ou à l'an.
/// Les liens de paiement viennent de `LUMA_CHECKOUT_MONTHLY` et `LUMA_CHECKOUT_ANNUAL`
/// (liens de paiement Stripe) ; sans eux, les boutons attendent.
/// Un prix en trois morceaux : ce qui précède le montant, le montant, ce qui le suit.
fn price_parts(price: &str) -> (&str, &str, &str) {
    let start = price.find(|c: char| c.is_ascii_digit()).unwrap_or(0);
    let end = price
        .rfind(|c: char| c.is_ascii_digit())
        .map_or(price.len(), |i| i + 1);
    (&price[..start], &price[start..end], &price[end..])
}

fn pricing(lang: Lang) -> Element {
    type Plan = (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    );
    let card = |(name, price, period, note, var): Plan, best: bool| {
        let url = std::env::var(var)
            .ok()
            .filter(|u| u.starts_with("https://"));
        let button = match url {
            Some(u) => a()
                .class(if best {
                    "btn magnetic"
                } else {
                    "btn btn-ghost magnetic"
                })
                .href(u)
                .child(span().text(t(lang, "S'abonner"))),
            None => span()
                .class("btn btn-ghost btn-disabled")
                .attr("aria-disabled", "true")
                .child(span().text(t(lang, "Bientôt disponible"))),
        };
        div()
            .class(if best {
                "plan plan-best reveal tilt"
            } else {
                "plan reveal tilt"
            })
            .child(
                p().class("plan-name")
                    .text(t(lang, name))
                    .child(best.then(|| {
                        span()
                            .class("plan-tag")
                            .text(t(lang, "Un seul paiement par an"))
                    })),
            )
            .child({
                // Le montant monte de zéro à l'écran, comme les compteurs ; le symbole
                // reste à sa place (« 19,99 € », « €19.99 »).
                let (before, amount, after) = price_parts(t(lang, price));
                p().class("plan-price")
                    .child(text(before))
                    .child(
                        i().attr("data-count", amount.replace(',', "."))
                            .attr("data-decimals", "2")
                            .attr("data-sep", if amount.contains(',') { "," } else { "." })
                            .text(amount),
                    )
                    .child(text(after))
                    .child(span().text(format!(" {}", t(lang, period))))
            })
            .child(p().class("plan-note").text(t(lang, note)))
            .child(button)
    };
    section()
        .class("pricing")
        .id("tarifs")
        .child(section_head(
            "LÜMA Premium",
            heading(h2, t(lang, "Quatorze jours pour *essayer*, puis à vous de choisir.")),
            Some(p().class("lede").text(t(lang,
                "Téléchargez LÜMA : tout est ouvert pendant l'essai. Ensuite, un abonnement au mois ou à l'an. Sur Mac, il s'achète uniquement ici, payé par Stripe ; votre clé de licence s'affiche aussitôt, et se saisit dans l'app. Sur iPhone et iPad (iOS et iPadOS), il s'achète uniquement dans l'App Store.",
            ))),
        ))
        .children(downloads_open().then(|| {
            div()
                .class("download reveal")
                .child(
                    a().class("btn magnetic")
                        .href(link("/download/LUMA-macos.zip"))
                        .attr("download", "")
                        .child(span().html(APPLE_SVG))
                        .child(span().text(t(lang, "Télécharger pour Mac"))),
                )
                .child(p().class("download-note").text(download_note(lang)))
        }))
        .child(
            div()
                .class("plans")
                .child(card(PLANS[0], false))
                .child(card(PLANS[1], true)),
        )
        .child(
            ul().class("premium-list reveal")
                .children(PREMIUM.iter().map(|f| li().text(t(lang, f)))),
        )
}

/// L'adresse d'un lien du site dans `lang` : une section de l'accueil (`#…`), une page
/// (`/…`), ou une adresse externe telle quelle.
fn target(lang: Lang, href: &str) -> String {
    if href.starts_with('#') {
        format!("{}{href}", page_link(lang, "/"))
    } else if href.starts_with('/') {
        page_link(lang, href)
    } else {
        href.to_owned()
    }
}

fn site_footer(here: Here) -> Element {
    let lang = here.lang;
    type Column = (&'static str, &'static [(&'static str, &'static str)]);
    let column = |(title, links): Column| {
        div()
            .class("footer-col")
            .child(p().class("footer-title").text(t(lang, title)))
            .child(ul().children(links.iter().map(|&(label, href)| {
                let external = href.starts_with("http");
                let mut a = a().href(target(lang, href)).text(t(lang, label));
                if external {
                    a = a.attr("target", "_blank").attr("rel", "noopener");
                }
                li().child(a)
            })))
    };
    footer()
        .class("footer")
        .child(
            div()
                .class("footer-top")
                .child(
                    div()
                        .class("footer-brand")
                        .child(
                            a().class("brand")
                                .href(page_link(lang, "/"))
                                .attr("aria-label", t(lang, "LÜMA, accueil"))
                                .child(span().class("brand-mark").html(BRAND_SVG))
                                .child(span().text("LÜMA")),
                        )
                        .child(p().class("footer-tagline").text(t(lang,
                            "L'éditeur photo qui développe la lumière. Écrit en Rust, pour Mac, iPhone et iPad.",
                        )))
                        .child(
                            div()
                                .class("footer-apps")
                                .child(if downloads_open() {
                                    a().class("store")
                                        .href(link("/download/LUMA-macos.zip"))
                                        .attr("download", "")
                                        .child(span().html(APPLE_SVG))
                                        .child(span().child(small().text(t(lang, "Télécharger pour"))).child(strong().text("Mac")))
                                } else {
                                    span()
                                        .class("store store-soon")
                                        .attr("aria-disabled", "true")
                                        .child(span().html(APPLE_SVG))
                                        .child(span().child(small().text(t(lang, "Bientôt sur"))).child(strong().text("Mac")))
                                })
                                .child(
                                    span()
                                        .class("store store-soon")
                                        .attr("aria-disabled", "true")
                                        .child(span().html(APPLE_SVG))
                                        .child(span().child(small().text(t(lang, "Bientôt sur"))).child(strong().text(t(lang, "l'App Store")))),
                                ),
                        ),
                )
                .child(div().class("footer-cols").children(FOOTER.iter().map(|c| column(*c)))),
        )
        // La signature, au centre : l'auteur, l'origine, puis le nom de l'app en grand.
        .child(
            div()
                .class("footer-signature")
                .child(p().class("footer-signature-label").text(t(lang, "Conçu et développé par")))
                .child(
                    p().class("footer-signature-name").text("Maxime Nathan Lestage"),
                )
                .child(heading(p, t(lang, "*Fabriqué en Rust*, en France")).class("footer-signature-made"))
                .child(
                    // Les lettres montent une à une quand le pied de page arrive, comme le titre
                    // de l'accueil.
                    p().class("footer-wordmark reveal")
                        .attr("aria-hidden", "true")
                        .children("LÜMA".chars().enumerate().map(|(n, c)| {
                            span()
                                .class("ch")
                                .child(span().style(format!("--i:{n}")).text(c.to_string()))
                        })),
                ),
        )
        .child(
            div()
                .class("footer-bottom")
                .child(span().text(t(lang, "© 2026 Maxime Nathan Lestage. Tous droits réservés.")))
                .child(span().text(t(lang, "Logiciel propriétaire")))
                .child(lang_switch(here, true))
                .child(
                    a().class("footer-made-with")
                        .href("https://github.com/maxlestage/Paquets")
                        .attr("target", "_blank")
                        .attr("rel", "noopener")
                        .text(t(lang, "Site fait avec Paquets ↗")),
                )
                .child(a().href(target(lang, "#top")).text(t(lang, "Haut de page ↑"))),
        )
}

/// La page de la clé de licence, après le paiement Stripe.
pub fn licence(
    site: &Site,
    origin: &str,
    here: Here,
    result: Result<(luma_licence::Claims, String), String>,
    portal: Option<String>,
) -> Page {
    let lang = here.lang;
    let content = match result {
        Ok((claims, key)) => div()
            .child(h1().text(t(lang, "Merci !")))
            .child(p().class("lede").text(fill(
                t(lang, "Votre abonnement LÜMA Premium {plan} est actif{email}."),
                &[
                    ("plan", t(lang, claims.plan.name()).to_lowercase().as_str()),
                    ("email", if claims.email.is_empty() { String::new() } else { format!(" ({})", claims.email) }.as_str()),
                ],
            )))
            .child(section().id("cle")
                .child(h2().text(t(lang, "Votre clé de licence")))
                .child(p().text(t(lang, "Copiez‑la, puis collez‑la dans LÜMA : la puce « Essai » en haut de l'app, champ « Clé de licence », Activer. Elle marche sur tous vos appareils, et se renouvelle seule tant que l'abonnement court.")))
                .child(textarea().class("licence-key").attr("readonly", "").attr("rows", "4").attr("aria-label", t(lang, "Clé de licence")).text(key))
                .child(p().class("todo").text(t(lang, "Gardez l'adresse de cette page (favori) : elle redonne votre clé à tout moment."))))
            .child(section()
                .child(h2().text(t(lang, "Pas encore l'app ?")))
                .child(div().class("hero-cta")
                    .children(downloads_open().then(|| a().class("btn").href(link("/download/LUMA-macos.zip")).attr("download", "").child(span().text(t(lang, "Télécharger pour Mac")))))
                    .child(portal.map(|u| a().class("btn btn-ghost").href(u).child(span().text(t(lang, "Gérer mon abonnement"))))))),
        Err(e) => div()
            .child(h1().text(t(lang, "Clé de licence")))
            .child(p().class("lede").text(fill(t(lang, "Impossible d'afficher la clé : {error}."), &[("error", t(lang, &e))])))
            .child(p().text(t(lang, "Si vous venez de payer, rechargez la page dans un instant. Sinon, écrivez‑nous (voir les mentions légales) avec le courriel de l'abonnement."))),
    };
    let body = div()
        .class("site site--plain")
        .child(navbar(here))
        .child(
            html::main().child(
                article()
                    .class("legal")
                    .child(kicker("LÜMA Premium"))
                    .child(content),
            ),
        )
        .child(site_footer(here));
    layout(
        site,
        origin,
        here,
        &format!("{} — LÜMA", t(lang, "Votre clé de licence")),
        t(lang, DESCRIPTION),
        body,
    )
    .seo(Seo::new().noindex(true))
}

/// Une page de documentation (un document du dépôt).
pub fn doc(site: &Site, origin: &str, lang: Lang, page: &super::docs::Doc) -> Page {
    let here = Here::new(lang, page.path);
    let note = page.untranslated(lang).then(|| {
        p().class("todo")
            .text(t(lang, "Ce tableau est généré en français."))
    });
    let body = div()
        .class("site site--plain")
        .child(navbar(here))
        .child(
            html::main().child(
                article()
                    .class("legal doc")
                    .child(kicker(t(lang, "Documentation")))
                    .child(h1().text(t(lang, page.title)))
                    .child(p().class("lede").text(t(lang, page.lede)))
                    .child(note)
                    .child(
                        div()
                            .class("doc-body")
                            .html(page.html(lang, |p| page_link(lang, p))),
                    ),
            ),
        )
        .child(site_footer(here));
    layout(
        site,
        origin,
        here,
        &format!("{} — LÜMA", t(lang, page.title)),
        t(lang, page.lede),
        body,
    )
}

/// Une page légale (mentions, confidentialité, conditions).
pub fn legal(site: &Site, origin: &str, lang: Lang, page: &Legal) -> Page {
    let here = Here::new(lang, page.path);
    // Hors du français, une traduction : la version française fait foi.
    let notice = (lang != Lang::Fr).then(|| {
        p().class("todo")
            .child(text(format!("{} ", t(lang, "Traduction fournie pour information : en cas de différence, la version française fait foi."))))
            .child(a().href(page_link(Lang::Fr, page.path)).attr("hreflang", "fr").attr("lang", "fr").text("Version française"))
    });
    let body = div()
        .class("site site--plain")
        .child(navbar(here))
        .child(
            html::main().child(
                article()
                    .class("legal")
                    .child(kicker("LÜMA"))
                    .child(h1().text(t(lang, page.title)))
                    .child(p().class("lede").text(t(lang, page.lede)))
                    .child(notice)
                    .child(
                        nav()
                            .class("legal-toc")
                            .attr("aria-label", t(lang, "Sommaire"))
                            .children(page.sections.iter().map(|(id, title, _)| {
                                a().href(format!("#{id}")).text(t(lang, title))
                            })),
                    )
                    .children(page.sections.iter().map(|(id, title, paras)| {
                        section().id(*id).child(h2().text(t(lang, title))).children(
                            paras.iter().map(|para| {
                                match para.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) {
                                    Some(todo) => p().class("todo").text(fill(
                                        t(lang, "À compléter : {todo}"),
                                        &[("todo", t(lang, todo))],
                                    )),
                                    None => p().text(t(lang, para)),
                                }
                            }),
                        )
                    })),
            ),
        )
        .child(site_footer(here));
    layout(
        site,
        origin,
        here,
        &format!("{} — LÜMA", t(lang, page.title)),
        t(lang, page.lede),
        body,
    )
}

// ----- Pages --------------------------------------------------------------------------

/// Le `<head>` commun : thème sombre, icônes, polices, le module des scènes 3D, et les
/// balises SEO de la page dans sa langue (canonique, traductions, Open Graph).
fn layout(
    site: &Site,
    origin: &str,
    here: Here,
    title: &str,
    description: &str,
    body: Element,
) -> Page {
    let lang = here.lang;
    let head = format!(
        r##"<meta name="theme-color" content="#050507"/><meta name="color-scheme" content="dark"/><meta name="apple-mobile-web-app-capable" content="yes"/><meta name="apple-mobile-web-app-status-bar-style" content="black-translucent"/><meta name="apple-mobile-web-app-title" content="LÜMA"/><meta name="application-name" content="LÜMA"/><link rel="manifest" href="{manifest}"/><script>if("serviceWorker"in navigator)addEventListener("load",()=>navigator.serviceWorker.register("/sw.js"))</script><link rel="icon" href="{icon}" type="image/svg+xml"/><link rel="apple-touch-icon" href="{touch}"/><link rel="preconnect" href="https://fonts.googleapis.com"/><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin=""/><script>document.documentElement.classList.add('js','loading')</script><script type="module" src="{scenes}"></script>"##,
        manifest = link(&i18n::path(lang, "/manifest.webmanifest")),
        icon = link("/assets/favicon.svg"),
        touch = link("/assets/apple-touch-icon.png"),
        scenes = link(&format!("/js/scenes.js?v={}", site.versions.scenes)),
    );
    let mut seo = Seo::new()
        .site_name("LÜMA")
        .base_url(origin.to_owned())
        .canonical(i18n::path(lang, here.path))
        .image("/assets/og-image.png")
        .og_type("website")
        .locale(i18n::locale(lang));
    for l in Lang::ALL {
        seo = seo.alternate(l.code(), i18n::path(l, here.path));
    }
    seo = seo.alternate("x-default", here.path.to_owned());
    Page::new(title)
        .lang(lang.code())
        .description(description)
        .seo(seo)
        .plain()
        .stylesheet(FONTS)
        .stylesheet(link(&format!("/site.css?v={}", site.versions.css)))
        .head(head)
        .body(body)
}

pub fn home(site: &Site, origin: &str, lang: Lang) -> Page {
    let here = Here::new(lang, "/");
    let body = div()
        .class("site")
        .attr("data-base", super::base())
        .attr("data-photo", link("/photo.svg"))
        .attr("data-photo-mono", link("/photo-mono.svg"))
        .child(chrome())
        .child(navbar(here))
        .child(
            canvas()
                .id("hero-gl")
                .class("hero-gl")
                .attr("aria-hidden", "true"),
        )
        .child(
            html::main()
                .child(hero(lang))
                .child(marquee(lang))
                .child(manifesto(lang))
                .child(stats(lang))
                .child(pipeline(lang))
                .child(lab(lang))
                .child(apple(lang))
                .child(platforms(lang))
                .child(architecture(lang))
                .child(performance(lang))
                .child(capabilities(lang))
                .child(roadmap(lang))
                .child(rust(lang))
                .child(pricing(lang))
                .child(cta(lang)),
        )
        .child(site_footer(here));
    layout(
        site,
        origin,
        here,
        t(lang, TITLE),
        t(lang, DESCRIPTION),
        body,
    )
}

pub fn not_found(site: &Site, origin: &str, lang: Lang) -> Page {
    let here = Here::new(lang, "/");
    let body = div().class("site site--plain").child(navbar(here)).child(
        html::main().child(
            section()
                .class("cta")
                .child(div().class("cta-orb").attr("aria-hidden", "true"))
                .child(heading(h2, t(lang, "Hors *champ*.")))
                .child(p().text(t(
                    lang,
                    "Cette page n'existe pas. La lumière est de l'autre côté.",
                )))
                .child(
                    div().class("hero-cta").child(
                        a().class("btn")
                            .href(page_link(lang, "/"))
                            .child(span().text(t(lang, "Retour à l'accueil"))),
                    ),
                ),
        ),
    );
    let body = body.child(site_footer(here));
    layout(
        site,
        origin,
        here,
        &format!("{} — LÜMA", t(lang, "Page introuvable")),
        t(lang, DESCRIPTION),
        body,
    )
    .seo(Seo::new().noindex(true))
    .status(404)
}

/// La note sous le bouton de téléchargement. Une app notariée par Apple
/// (`LUMA_MAC_NOTARIZED=1`, posé par la CI quand les secrets de signature sont là)
/// s'ouvre d'un double clic ; sinon, la marche à suivre au premier lancement.
fn download_note(lang: Lang) -> String {
    let base = t(lang, "Apple Silicon et Intel, macOS 11 ou plus récent.");
    if std::env::var("LUMA_MAC_NOTARIZED").is_ok_and(|v| v == "1") {
        base.to_owned()
    } else {
        format!(
            "{base} {}",
            t(
                lang,
                "Premier lancement : si macOS refuse d'ouvrir LÜMA, Réglages Système → Confidentialité et sécurité → « Ouvrir quand même »."
            )
        )
    }
}

/// Le téléchargement de l'app est‑il ouvert (`LUMA_DOWNLOADS=1`) ? Fermé par défaut :
/// les boutons « Télécharger » disparaissent du site (barre, accueil, tarifs, pied de
/// page, page de la clé, raccourcis de la PWA) jusqu'à l'ouverture.
pub fn downloads_open() -> bool {
    std::env::var("LUMA_DOWNLOADS").is_ok_and(|v| v == "1")
}
