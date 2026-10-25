//! La page d'accueil et la page « introuvable », écrites en vues Active : du HTML fini
//! rendu par le serveur, lisible sans JavaScript, avec ses balises SEO.

use paquets::prelude::*;

use super::{Site, link};
use crate::content::*;
use crate::islands;

const FONTS: &str = "https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700;800&family=Instrument+Serif:ital@0;1&family=JetBrains+Mono:wght@400;500&display=swap";

/// Un titre de section : le mot en italique ressort en couleur.
fn heading(
    tag: fn() -> Element,
    before: &'static str,
    accent: &'static str,
    after: &'static str,
) -> Element {
    tag()
        .text(before)
        .child(em().text(accent))
        .child(text(after))
}

fn kicker(label: &'static str) -> Element {
    p().class("kicker").text(label)
}

fn section_head(label: &'static str, title: Element, lede: Option<Element>) -> Element {
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

fn navbar() -> Element {
    header()
        .class("nav")
        .child(
            a().class("brand")
                .href(link("/"))
                .attr("aria-label", "LÜMA, accueil")
                .child(span().class("brand-mark").html(BRAND_SVG))
                .child(span().text("LÜMA")),
        )
        .child(
            nav()
                .class("links")
                .attr("aria-label", "Sections")
                .children(
                    NAV.iter()
                        .map(|&(href, label)| a().href(target(href)).text(label)),
                ),
        )
        .child(
            a().class("btn btn-small magnetic")
                .href(target("#tarifs"))
                .child(span().text("Télécharger")),
        )
        .child(
            button()
                .class("menu-toggle")
                .attr("type", "button")
                .attr("aria-label", "Menu")
                .attr("aria-expanded", "false")
                .child(i())
                .child(i()),
        )
}

// ----- Sections -----------------------------------------------------------------------

fn hero() -> Element {
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
                        .child(text(" Éditeur photo · Mac, iPhone et iPad · 100 % Rust")),
                )
                .child(h1().class("hero-title").attr("aria-label", "LÜMA").child(title))
                .child(
                    p().class("hero-sub reveal")
                        .text("La lumière, ")
                        .child(em().text("développée"))
                        .child(text(" en Rust.")),
                )
                .child(p().class("hero-lede reveal").text(
                    "Un moteur RAW 32 bits flottants sur le GPU, 100 films vérifiés, et vos vraies pellicules mesurées sur un scan. Pas une copie de Lightroom : un concurrent qui se mesure à lui, chiffres à l'appui.",
                ))
                .child(
                    div()
                        .class("hero-cta reveal")
                        .child(a().class("btn magnetic").href("#tarifs").child(span().text("Télécharger · 14 jours d'essai")))
                        .child(a().class("btn btn-ghost magnetic").href("#labo").child(span().text("Voir le moteur en action"))),
                ),
        )
        .child(
            div()
                .class("hero-meta reveal")
                .children(["RAW", "32 bits f", "GPU", "100 films"].map(|t| span().text(t))),
        )
        .child(
            a().class("scroll-hint")
                .href("#manifeste")
                .attr("aria-label", "Défiler")
                .child(span()),
        )
}

fn marquee() -> Element {
    // Deux fois la même liste : la piste défile de la moitié de sa largeur, sans saut.
    let items = |hidden: bool| {
        MARQUEE.iter().flat_map(move |item| {
            let label = if hidden {
                span().attr("aria-hidden", "true").text(*item)
            } else {
                span().text(*item)
            };
            [label, b().attr("aria-hidden", "true").text("✦")]
        })
    };
    section()
        .class("marquee")
        .attr("aria-label", "Fonctions")
        .child(
            div()
                .class("marquee-track")
                .children(items(false))
                .children(items(true)),
        )
}

fn manifesto() -> Element {
    let words = MANIFESTO
        .split_whitespace()
        .map(|w| match w.strip_prefix('*') {
            Some(word) => span().class("w hl").text(word),
            None => span().class("w").text(w),
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

fn stats() -> Element {
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
            .child(small().text(s.unit));
        div()
            .class("stat reveal tilt")
            .child(num)
            .child(p().text(s.text))
    }))
}

fn pipeline() -> Element {
    let steps =
        ol().class("pipeline-steps")
            .children(PIPELINE.iter().map(|&(name, detail, _)| {
                li().child(b().text(name)).child(text(format!(" {detail}")))
            }));
    let layers = div().class("pipeline-stage").children(
        PIPELINE
            .iter()
            .enumerate()
            .map(|(n, &(_, _, tag))| div().class(format!("layer l{n}")).child(span().text(tag))),
    );
    section().class("pipeline").id("pipeline").child(
        div()
            .class("pipeline-sticky")
            .child(
                div()
                    .class("pipeline-copy")
                    .child(kicker("Le moteur"))
                    .child(heading(
                        h2,
                        "Un pipeline ",
                        "non destructif",
                        ", couche par couche.",
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

fn lab() -> Element {
    section()
        .class("lab")
        .id("labo")
        .child(section_head(
            "Le labo",
            heading(h2, "Bouge un curseur. ", "Rien", " n'est détruit."),
            Some(p().text(
                "Cette démo tourne sur le GPU de ton appareil, pilotée par du Rust compilé en WebAssembly : une scène générée en lumière linéaire (le soleil dépasse le blanc), puis exposition, balance des blancs, hautes lumières, ombres et contraste appliqués comme des paramètres. Glisse la barre pour comparer avec l'original. C'est une illustration du principe, pas encore le moteur LÜMA.",
            )),
        ))
        .child(island("lab", "", islands::lab))
}

fn apple() -> Element {
    let panels =
        div()
            .class("apple-panels")
            .children(APPLE.iter().map(|&(before, accent, body)| {
                article()
                    .class("apple-panel")
                    .child(heading(h2, before, accent, "."))
                    .child(p().text(body))
            }));
    section().class("apple").id("apple").child(
        div()
            .class("apple-sticky")
            .child(canvas().id("devices-gl").attr("aria-hidden", "true"))
            .child(
                div()
                    .class("apple-copy")
                    .child(
                        p().class("kicker")
                            .child(span().class("kicker-mark").html(APPLE_SVG))
                            .child(text(" macOS & iOS d'abord")),
                    )
                    .child(panels)
                    .child(div().class("apple-dots").children([i(), i(), i()])),
            ),
    )
}

fn platforms() -> Element {
    section()
        .class("platforms")
        .child(section_head(
            "Ordre de sortie",
            heading(h2, "Apple en premier. ", "Les autres suivent", ", sur le même cœur."),
            Some(p().text(
                "Windows, Linux et Android sont compilés en intégration continue dès le premier mois : le moteur ne doit rien supposer d'Apple. Leurs applications arrivent en phase 6.",
            )),
        ))
        .child(div().class("platform-grid").children(PLATFORMS.iter().map(
            |&(name, phase, detail, prime)| {
                div()
                    .class(if prime { "platform prime reveal tilt" } else { "platform reveal tilt" })
                    .child(span().class("badge").text(phase))
                    .child(h3().text(name))
                    .child(p().text(detail))
            },
        )))
}

fn architecture() -> Element {
    section()
        .class("arch")
        .id("architecture")
        .child(section_head(
            "Architecture",
            heading(h2, "Un workspace Cargo. ", "Une crate", ", une responsabilité."),
            Some(p().text(
                "Le moteur ne dépend jamais de la plateforme. Seules l'interface et les applications voient macOS ou iOS.",
            )),
        ))
        .child(island("crates", "", islands::crates))
}

fn performance() -> Element {
    let max = |a: u32, b: u32| f64::from(a.max(b));
    let cards = PERF.iter().map(|&(name, unit, mac, phone)| {
        let top = max(mac, phone);
        let row = |device: &'static str, v: u32| {
            [
                div().class("perf-row").child(span().text(device)).child(
                    b().text("< ")
                        .child(i().attr("data-count", v.to_string()).text(v.to_string()))
                        .child(text(format!(" {unit}"))),
                ),
                div()
                    .class("bar")
                    .child(span().style(format!("--v:{:.2}", f64::from(v) / top))),
            ]
        };
        div()
            .class("perf-card reveal")
            .child(h3().text(name))
            .children(row("Mac", mac))
            .children(row("iPhone", phone))
    });
    section()
        .class("perf")
        .id("performance")
        .child(section_head(
            "Performance",
            heading(h2, "Des objectifs ", "mesurables", "."),
            Some(p().text(
                "Ce sont des cibles d'ingénierie, pas des résultats. Référence : Mac Apple Silicon 16 Go et iPhone récent, cache chaud et cache froid mesurés séparément.",
            )),
        ))
        .child(div().class("perf-grid").children(cards))
        .child(
            div()
                .class("frame-budget reveal")
                .child(p().child(b().text("8,3 ms")).child(text(
                    " : le budget d'une image à 120 Hz. Les aperçus progressifs, les tuiles et l'annulation des calculs devenus inutiles servent à tenir ce budget pendant qu'un curseur bouge.",
                )))
                .child(
                    div()
                        .class("frames")
                        .attr("aria-hidden", "true")
                        .children((0..40).map(|k| i().style(format!("--k:{k}")))),
                ),
        )
}

fn roadmap() -> Element {
    let phases = ROADMAP.iter().map(|ph| {
        article()
            .class(if ph.later { "phase later" } else { "phase" })
            .child(span().class("phase-when").text(ph.when))
            .child(h3().text(ph.name))
            .child(ul().children(ph.items.iter().map(|item| li().text(*item))))
            .child(p().class("phase-goal").text(ph.goal))
    });
    section().class("roadmap").id("roadmap").child(
        div()
            .class("roadmap-sticky")
            .child(
                div()
                    .class("roadmap-head")
                    .child(kicker("Feuille de route"))
                    .child(heading(h2, "24 à 36 mois. ", "Apple", " à chaque étape.")),
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

fn rust() -> Element {
    let mut lines = Vec::<Node>::new();
    for (n, &(class, line)) in DENY.iter().enumerate() {
        if n > 0 {
            lines.push(text("\n"));
        }
        lines.push(deny_line(class, line));
    }
    section()
        .class("rust")
        .id("rust")
        .child(section_head(
            "100 % Rust, vérifiable",
            heading(h2, "Pas une promesse. ", "Un audit", "."),
            Some(
                p().text("Une crate sur crates.io n'est pas forcément pure Rust. LÜMA vérifie le graphe de dépendances réel à chaque build : aucune ")
                    .child(code().text("-sys"))
                    .child(text(", aucun "))
                    .child(code().text("build.rs"))
                    .child(text(" qui compile du C, aucun FFI vers une bibliothèque tierce. Les frameworks du système (Metal, AppKit, UIKit, PhotoKit) sont appelés depuis Rust.")),
            ),
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
                    RULES.iter().map(|&(name, body)| li().class("reveal").child(b().text(name)).child(text(format!(" {body}")))),
                )),
        )
}

fn cta() -> Element {
    section()
        .class("cta")
        .child(div().class("cta-orb").attr("aria-hidden", "true"))
        .child(h2().class("reveal").text("Développer").child(br()).child(em().text("la lumière")).child(text(".")))
        .child(p().class("reveal").text(
            "D'abord le meilleur moteur photo Rust possible, sur Mac et iPhone. Ensuite, tout le reste.",
        ))
        .child(
            div()
                .class("hero-cta reveal")
                .child(a().class("btn magnetic").href(format!("{REPO}/blob/master/docs/architecture.md")).child(span().text("Lire l'architecture")))
                .child(a().class("btn btn-ghost magnetic").href(format!("{REPO}/blob/master/docs/roadmap.md")).child(span().text("Feuille de route détaillée"))),
        )
}

/// Tarifs et téléchargement : LÜMA Premium, 14 jours d'essai, puis au mois ou à l'an.
/// Les liens de paiement viennent de `LUMA_CHECKOUT_MONTHLY` et `LUMA_CHECKOUT_ANNUAL`
/// (Lemon Squeezy) ; sans eux, les boutons attendent.
fn pricing() -> Element {
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
                .child(span().text("S'abonner")),
            None => span()
                .class("btn btn-ghost btn-disabled")
                .attr("aria-disabled", "true")
                .child(span().text("Bientôt disponible")),
        };
        div()
            .class(if best {
                "plan plan-best reveal"
            } else {
                "plan reveal"
            })
            .child(
                p().class("plan-name")
                    .text(name)
                    .child(best.then(|| span().class("plan-tag").text("Le plus avantageux"))),
            )
            .child(
                p().class("plan-price")
                    .text(price)
                    .child(span().text(format!(" {period}"))),
            )
            .child(p().class("plan-note").text(note))
            .child(button)
    };
    section()
        .class("pricing")
        .id("tarifs")
        .child(section_head(
            "LÜMA Premium",
            heading(h2, "Quatorze jours pour ", "essayer", ", puis à vous de choisir."),
            Some(p().class("lede").text(
                "Téléchargez LÜMA : tout est ouvert pendant l'essai. Ensuite, un abonnement au mois ou à l'an ; la clé de licence arrive par courriel, et se saisit dans l'app.",
            )),
        ))
        .child(
            div()
                .class("download reveal")
                .child(
                    a().class("btn magnetic")
                        .href(link("/download/LUMA-macos.zip"))
                        .attr("download", "")
                        .child(span().html(APPLE_SVG))
                        .child(span().text("Télécharger pour Mac")),
                )
                .child(p().class("download-note").text(
                    "Apple Silicon et Intel, macOS 11 ou plus récent. Premier lancement : clic droit sur LÜMA, puis Ouvrir.",
                )),
        )
        .child(
            div()
                .class("plans")
                .child(card(PLANS[0], false))
                .child(card(PLANS[1], true)),
        )
        .child(
            ul().class("premium-list reveal")
                .children(PREMIUM.iter().map(|f| li().text(*f))),
        )
}

/// L'adresse d'un lien du site : une section de l'accueil (`#…`), une page (`/…`), ou
/// une adresse externe telle quelle.
fn target(href: &str) -> String {
    if href.starts_with('#') {
        format!("{}{href}", link("/"))
    } else if href.starts_with('/') {
        link(href)
    } else {
        href.to_owned()
    }
}

fn site_footer() -> Element {
    type Column = (&'static str, &'static [(&'static str, &'static str)]);
    let column = |(title, links): Column| {
        div()
            .class("footer-col")
            .child(p().class("footer-title").text(title))
            .child(ul().children(links.iter().map(|&(label, href)| {
                let external = href.starts_with("http");
                let mut a = a().href(target(href)).text(label);
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
                                .href(link("/"))
                                .attr("aria-label", "LÜMA, accueil")
                                .child(span().class("brand-mark").html(BRAND_SVG))
                                .child(span().text("LÜMA")),
                        )
                        .child(p().class("footer-tagline").text(
                            "L'éditeur photo qui développe la lumière. Écrit en Rust, pour Mac, iPhone et iPad.",
                        ))
                        .child(
                            div()
                                .class("footer-apps")
                                .child(
                                    a().class("store")
                                        .href(link("/download/LUMA-macos.zip"))
                                        .attr("download", "")
                                        .child(span().html(APPLE_SVG))
                                        .child(span().child(small().text("Télécharger pour")).child(strong().text("Mac"))),
                                )
                                .child(
                                    span()
                                        .class("store store-soon")
                                        .attr("aria-disabled", "true")
                                        .child(span().html(APPLE_SVG))
                                        .child(span().child(small().text("Bientôt sur")).child(strong().text("l'App Store"))),
                                ),
                        ),
                )
                .child(div().class("footer-cols").children(FOOTER.iter().map(|c| column(*c)))),
        )
        .child(
            div()
                .class("footer-bottom")
                .child(span().text("© 2026 LÜMA. Tous droits réservés."))
                .child(span().text("Fait en France · 100 % Rust · Aucun traceur"))
                .child(a().href(target("#top")).text("Haut de page ↑")),
        )
}

/// Une page légale (mentions, confidentialité, conditions).
pub fn legal(site: &Site, origin: &str, page: &Legal) -> Page {
    let body = div()
        .class("site site--plain")
        .child(navbar())
        .child(
            html::main().child(
                article()
                    .class("legal")
                    .child(kicker("LÜMA"))
                    .child(h1().text(page.title))
                    .child(p().class("lede").text(page.lede))
                    .child(
                        nav()
                            .class("legal-toc")
                            .attr("aria-label", "Sommaire")
                            .children(
                                page.sections
                                    .iter()
                                    .map(|(id, title, _)| a().href(format!("#{id}")).text(*title)),
                            ),
                    )
                    .children(page.sections.iter().map(|(id, title, paras)| {
                        section()
                            .id(*id)
                            .child(h2().text(*title))
                            .children(paras.iter().map(|t| {
                                match t.strip_prefix("[[").and_then(|t| t.strip_suffix("]]")) {
                                    Some(todo) => {
                                        p().class("todo").text(format!("À compléter : {todo}"))
                                    }
                                    None => p().text(*t),
                                }
                            }))
                    })),
            ),
        )
        .child(site_footer());
    layout(site, origin, &format!("{} — LÜMA", page.title), body).seo(
        Seo::new()
            .site_name("LÜMA")
            .base_url(origin.to_owned())
            .canonical(page.path)
            .locale("fr_FR"),
    )
}

// ----- Pages --------------------------------------------------------------------------

/// Le `<head>` commun : thème sombre, icônes, polices, et le module des scènes 3D.
fn layout(site: &Site, origin: &str, title: &str, body: Element) -> Page {
    let head = format!(
        r##"<meta name="theme-color" content="#050507"/><meta name="color-scheme" content="dark"/><meta name="apple-mobile-web-app-capable" content="yes"/><meta name="apple-mobile-web-app-status-bar-style" content="black-translucent"/><meta name="apple-mobile-web-app-title" content="LÜMA"/><meta name="application-name" content="LÜMA"/><link rel="manifest" href="/manifest.webmanifest"/><script>if("serviceWorker"in navigator)addEventListener("load",()=>navigator.serviceWorker.register("/sw.js"))</script><link rel="icon" href="{icon}" type="image/svg+xml"/><link rel="apple-touch-icon" href="{touch}"/><link rel="preconnect" href="https://fonts.googleapis.com"/><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin=""/><script>document.documentElement.classList.add('js','loading')</script><script type="module" src="{scenes}"></script>"##,
        icon = link("/assets/favicon.svg"),
        touch = link("/assets/apple-touch-icon.png"),
        scenes = link(&format!("/js/scenes.js?v={}", site.versions.scenes)),
    );
    Page::new(title)
        .lang("fr")
        .description(DESCRIPTION)
        .seo(
            Seo::new()
                .site_name("LÜMA")
                .base_url(origin.to_owned())
                .canonical("/")
                .image("/assets/og-image.png")
                .og_type("website")
                .locale("fr_FR"),
        )
        .plain()
        .stylesheet(FONTS)
        .stylesheet(link(&format!("/site.css?v={}", site.versions.css)))
        .head(head)
        .body(body)
}

pub fn home(site: &Site, origin: &str) -> Page {
    let body = div()
        .class("site")
        .attr("data-base", super::base())
        .attr("data-photo", link("/photo.svg"))
        .attr("data-photo-mono", link("/photo-mono.svg"))
        .child(chrome())
        .child(navbar())
        .child(
            canvas()
                .id("hero-gl")
                .class("hero-gl")
                .attr("aria-hidden", "true"),
        )
        .child(
            html::main()
                .child(hero())
                .child(marquee())
                .child(manifesto())
                .child(stats())
                .child(pipeline())
                .child(lab())
                .child(apple())
                .child(platforms())
                .child(architecture())
                .child(performance())
                .child(roadmap())
                .child(rust())
                .child(pricing())
                .child(cta()),
        )
        .child(site_footer());
    layout(site, origin, TITLE, body)
}

pub fn not_found(site: &Site, origin: &str) -> Page {
    let body = div().class("site site--plain").child(navbar()).child(
        html::main().child(
            section()
                .class("cta")
                .child(div().class("cta-orb").attr("aria-hidden", "true"))
                .child(
                    h2().text("Hors ")
                        .child(em().text("champ"))
                        .child(text(".")),
                )
                .child(p().text("Cette page n'existe pas. La lumière est de l'autre côté."))
                .child(
                    div().class("hero-cta").child(
                        a().class("btn")
                            .href(link("/"))
                            .child(span().text("Retour à l'accueil")),
                    ),
                ),
        ),
    );
    let body = body.child(site_footer());
    layout(site, origin, "Page introuvable — LÜMA", body)
        .seo(Seo::new().noindex(true))
        .status(404)
}
