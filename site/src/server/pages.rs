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

const LINUX_SVG: &str = r#"<svg class="linux-logo" viewBox="0 0 24 24" aria-hidden="true"><rect x="2.5" y="4" width="19" height="16" rx="3" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="M7 9.5l3 2.5-3 2.5M12.5 15h4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
/// Windows : une fenêtre en quatre carreaux (un pictogramme, pas le logo).
const WINDOWS_SVG: &str = r#"<svg class="windows-logo" viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M3 5.2l7.4-1v7.3H3zM11.6 4l9.4-1.3v8.8h-9.4zM3 12.6h7.4v7.3L3 18.8zM11.6 12.6H21v8.7l-9.4-1.3z"/></svg>"#;
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
                        .child(text(format!(" {}", t(lang, "Éditeur photo · Mac, iPhone, iPad, Linux et Windows")))),
                )
                .child(h1().class("hero-title").attr("aria-label", "LÜMA").child(title))
                .child(heading(p, t(lang, "La lumière, *développée*.")).class("hero-sub reveal"))
                .child(p().class("hero-lede reveal").text(t(lang,
                    "Ouvrez vos RAW, réglez la lumière et la couleur, posez un film, partagez. Un éditeur photo professionnel, rapide et précis, qui garde vos originaux intacts.",
                )))
                .child(
                    div()
                        .class("hero-cta reveal")
                        .child(a().class("btn magnetic").href("#tarifs").child(span().text(if downloads_open() {
                            t(lang, "Télécharger · un mois d'essai")
                        } else {
                            t(lang, "Voir les tarifs")
                        })))
                        .child(a().class("btn btn-ghost magnetic").href("#labo").child(span().text(t(lang, "Essayer en direct")))),
                ),
        )
        .child(
            div()
                .class("hero-meta reveal")
                .children(["RAW", t(lang, "275 films"), t(lang, "Masques"), "Mac · iPhone · iPad · Linux · Windows"].map(|s| span().text(s))),
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
                    .child(kicker(t(lang, "Le développement")))
                    .child(heading(
                        h2,
                        t(lang, "Vos réglages *s'empilent*. L'original ne bouge pas."),
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
            t(lang, "Essayez"),
            heading(h2, t(lang, "Bouge un curseur. *Rien* n'est détruit.")),
            Some(p().text(t(lang,
                "Bouge l'exposition, la balance des blancs, les hautes lumières, les ombres et le contraste : l'image change en direct, et l'original reste là. Glisse la barre pour comparer avant et après. Une démonstration du principe, directement dans ton navigateur.",
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

/// Linux et Windows : un écran 3D qui tourne lentement (`systems.js`), puis une
/// carte par système : ce qu'on télécharge, ce qu'il faut, et le bouton. Windows
/// n'affiche son téléchargement que quand ses fichiers sont là (voir [`windows_asset`]).
fn desktop(lang: Lang) -> Element {
    let spec = |items: [(&'static str, &'static str); 3]| {
        dl().class("desktop-spec").children(
            items
                .into_iter()
                .map(|(k, v)| div().child(dt().text(k)).child(dd().text(v))),
        )
    };
    let linux_action = if downloads_open() {
        div()
            .class("desktop-actions")
            .child(
                a().class("btn magnetic")
                    .href(download("LUMA-linux-x86_64.tar.gz"))
                    .attr("download", "")
                    .child(span().html(LINUX_SVG))
                    .child(span().text(t(lang, "Télécharger pour Linux"))),
            )
            .child(
                a().class("desktop-alt")
                    .href(download("LUMA-linux-amd64.deb"))
                    .attr("download", "")
                    .text(t(lang, "Paquet .deb (Debian, Ubuntu)")),
            )
    } else {
        div().class("desktop-actions").child(
            a().class("btn btn-ghost magnetic")
                .href("#tarifs")
                .child(span().text(t(lang, "Voir les tarifs"))),
        )
    };
    let windows = windows_open();
    let windows_action = match windows_primary() {
        Some(file) => div()
            .class("desktop-actions")
            .child(
                a().class("btn magnetic")
                    .href(download(file))
                    .attr("download", "")
                    .child(span().html(WINDOWS_SVG))
                    .child(span().text(t(lang, "Télécharger pour Windows"))),
            )
            .children(
                (file == WINDOWS_SETUP && windows_asset(WINDOWS_ZIP)).then(|| {
                    a().class("desktop-alt")
                        .href(download(WINDOWS_ZIP))
                        .attr("download", "")
                        .text(t(lang, "Archive .zip, sans installation"))
                }),
            ),
        None => div().class("desktop-actions").child(
            span()
                .class("btn btn-ghost btn-disabled")
                .attr("aria-disabled", "true")
                .child(span().html(WINDOWS_SVG))
                .child(span().text(t(lang, "Bientôt sur Windows"))),
        ),
    };
    let card = |id: &'static str, logo: &'static str, name: &str, ready: bool| {
        article()
            .class(if ready {
                "desktop-card reveal tilt"
            } else {
                "desktop-card desktop-soon reveal tilt"
            })
            .id(id)
            .child(
                p().class("desktop-os")
                    .child(span().class("kicker-mark").html(logo))
                    .child(text(format!(" {name}")))
                    .child(span().class("badge").text(if ready {
                        t(lang, "Disponible")
                    } else {
                        t(lang, "Bientôt")
                    })),
            )
    };
    section()
        .class("desktop")
        .id("bureau")
        .child(section_head(
            t(lang, "Linux et Windows"),
            heading(h2, t(lang, "Le même LÜMA, *sur votre PC*.")),
            Some(p().text(t(lang,
                "Le moteur du Mac, tel quel : les mêmes réglages, les mêmes films, le même rendu au pixel près. Linux est déjà là, Windows arrive, avec le même abonnement.",
            ))),
        ))
        .child(
            div()
                .class("desktop-stage")
                .id("desktop-stage")
                .attr("aria-hidden", "true")
                .child(canvas().id("systems-gl").class("systems-gl")),
        )
        .child(
            div()
                .class("desktop-grid")
                .child(
                    card("linux", LINUX_SVG, "Linux", true)
                        .child(heading(h3, t(lang, "Linux, *déjà là*.")))
                        .child(p().text(t(lang,
                            "Une archive .tar.gz qui s'installe sans mot de passe dans votre dossier personnel, avec son lanceur et ses icônes, ou un paquet .deb pour Debian et Ubuntu. Le même moteur que sur Mac, sur votre carte graphique.",
                        )))
                        .child(spec([
                            (t(lang, "Processeur"), t(lang, "x86_64 (64 bits)")),
                            (t(lang, "Système"), t(lang, "glibc 2.35 ou plus récente : Ubuntu 22.04, Debian 12, Fedora 36…")),
                            (t(lang, "Graphisme"), t(lang, "Un pilote Vulkan")),
                        ]))
                        .child(linux_action),
                )
                .child(
                    card("windows", WINDOWS_SVG, "Windows", windows)
                        .child(heading(h3, if windows {
                            t(lang, "Windows, *c'est parti*.")
                        } else {
                            t(lang, "Windows, *bientôt*.")
                        }))
                        .child(p().text(if windows {
                            t(lang, "Un installateur, ou une archive .zip à décompresser où vous voulez. Le même moteur que sur Mac et Linux, et le même abonnement.")
                        } else {
                            t(lang, "Le portage est en cours : un installateur et une archive .zip à décompresser, le même moteur que sur Mac et Linux, et le même abonnement. Le bouton apparaîtra ici dès la sortie.")
                        }))
                        .child(spec([
                            (t(lang, "Processeur"), t(lang, "x86_64 (64 bits)")),
                            (t(lang, "Système"), t(lang, "Windows 10 ou Windows 11")),
                            (t(lang, "Graphisme"), t(lang, "DirectX 12 ou Vulkan")),
                        ]))
                        .child(windows_action),
                ),
        )
        .child(p().class("desktop-note reveal").text(t(lang,
            "Un seul abonnement LÜMA Premium vaut pour Mac, Linux et Windows : la clé achetée sur ce site se saisit dans l'app, sur chacun de vos ordinateurs.",
        )))
}

fn platforms(lang: Lang) -> Element {
    section()
        .class("platforms")
        .child(section_head(
            t(lang, "Ordre de sortie"),
            heading(h2, t(lang, "Apple en premier. *Les autres suivent*, sur le même cœur.")),
            Some(p().text(t(lang,
                "Linux est déjà là, Windows arrive ; Android suivra, avec le même moteur : une photo aura le même rendu partout.",
            ))),
        ))
        .child(div().class("platform-grid").children(PLATFORMS.iter().map(
            |&(name, phase, detail, prime)| {
                // Windows passe « Disponible » dès que son téléchargement est là.
                let (phase, prime) = if name == "Windows" && windows_open() {
                    ("Disponible", true)
                } else {
                    (phase, prime)
                };
                div()
                    .class(if prime { "platform prime reveal tilt" } else { "platform reveal tilt" })
                    .child(span().class("badge").text(t(lang, phase)))
                    .child(h3().text(name))
                    .child(p().text(t(lang, detail)))
            },
        )))
}

/// Les quatre étapes du travail, de la photothèque au partage.
fn steps(lang: Lang) -> Element {
    section()
        .class("steps")
        .id("etapes")
        .child(section_head(
            t(lang, "Du déclencheur au partage"),
            heading(h2, t(lang, "Quatre étapes, *un seul outil*.")),
            Some(p().text(t(lang,
                "Tout ce qu'il faut pour passer d'une carte mémoire pleine à des photos prêtes à partager, sans changer de logiciel.",
            ))),
        ))
        .child(ol().class("step-grid").children(STEPS.iter().map(|&(n, title, body)| {
            li().class("step reveal tilt")
                .child(span().class("step-num").attr("aria-hidden", "true").text(n))
                .child(h3().text(t(lang, title)))
                .child(p().text(t(lang, body)))
        })))
}

/// Les films : une photo, puis le film qui se pose dessus en balayant l'image.
fn films(lang: Lang) -> Element {
    let photo = link("/photo.svg");
    section()
        .class("films")
        .id("films")
        .child(section_head(
            t(lang, "Films"),
            heading(h2, t(lang, "Deux cent soixante-quinze films. *Et les vôtres.*")),
            Some(p().text(t(lang,
                "Portra, Velvia, Kodachrome, CineStill, Tri‑X, Lomochrome, Aerochrome : des pellicules couleur, noir et blanc, cinéma et instantanées, rangées par familles et faciles à retrouver, à poser d'un geste et à doser. Et si vous scannez vos propres pellicules, LÜMA les mesure pour les reproduire.",
            ))),
        ))
        .child(div().class("film-grid").children(FILMS.iter().enumerate().map(
            |(k, &(name, family, filter))| {
                figure()
                    .class("film reveal")
                    .style(format!("--k:{k}"))
                    .child(
                        div()
                            .class("film-frame")
                            .attr("aria-hidden", "true")
                            .child(img().attr("src", photo.clone()).attr("alt", "").attr("loading", "lazy"))
                            .child(
                                img()
                                    .class("film-look")
                                    .attr("src", photo.clone())
                                    .attr("alt", "")
                                    .attr("loading", "lazy")
                                    .style(format!("filter:{filter}")),
                            ),
                    )
                    .child(figcaption().child(b().text(name)).child(span().text(t(lang, family))))
            },
        )))
        .child(p().class("films-note reveal").text(t(lang,
            "Aperçus indicatifs, sur une seule photo : dans LÜMA, chaque film est un vrai rendu, réglé sur vos images.",
        )))
}

/// La galerie : des photos développées avec LÜMA, chacune avec sa barre avant / après.
/// Les photos sont dessinées dans le navigateur (`showcase.js`, `drawPhoto`) : l'image
/// brute à gauche de la barre, la photo développée et son film à droite.
fn gallery(lang: Lang) -> Element {
    let items = GALLERY.iter().enumerate().map(|(k, &(film, seed, hue))| {
        let (name, family, filter) = FILMS[film];
        figure()
            .class("ba reveal")
            .style(format!("--k:{k}"))
            .attr("data-seed", seed.to_string())
            .attr("data-hue", hue.to_string())
            .child(
                div()
                    .class("ba-frame")
                    .style("--split:50%")
                    .child(canvas().class("ba-before").attr("aria-hidden", "true"))
                    .child(
                        canvas()
                            .class("ba-after")
                            .attr("aria-hidden", "true")
                            .style(format!("filter:{filter}")),
                    )
                    .child(
                        span()
                            .class("ba-line")
                            .attr("aria-hidden", "true")
                            .child(span()),
                    )
                    .child(
                        span()
                            .class("ba-tag ba-tag-l")
                            .attr("aria-hidden", "true")
                            .text(t(lang, "Avant")),
                    )
                    .child(
                        span()
                            .class("ba-tag ba-tag-r")
                            .attr("aria-hidden", "true")
                            .text(t(lang, "Après")),
                    )
                    .child(
                        input()
                            .class("ba-range")
                            .attr("type", "range")
                            .attr("min", "0")
                            .attr("max", "100")
                            .attr("value", "50")
                            .attr(
                                "aria-label",
                                fill(t(lang, "Avant / après : {film}"), &[("film", name)]),
                            ),
                    ),
            )
            .child(
                figcaption()
                    .child(b().text(name))
                    .child(span().text(t(lang, family))),
            )
    });
    section()
        .class("gallery")
        .id("galerie")
        .child(section_head(
            t(lang, "Galerie"),
            heading(h2, t(lang, "Avant, après. *Développées avec LÜMA.*")),
            Some(p().text(t(lang,
                "Glissez la barre de chaque photo : à gauche, l'image telle qu'elle sort du boîtier, terne et plate ; à droite, la même développée, avec son film.",
            ))),
        ))
        .child(div().class("ba-grid").children(items))
        .child(p().class("films-note reveal").text(t(lang,
            "Images d'illustration dessinées par le site ; les films y sont approchés.",
        )))
        .child(
            div()
                .class("share reveal")
                .child(heading(h3, t(lang, "Partagez vos photos *développées avec LÜMA*.")))
                .child(p().text(t(lang,
                    "Envoyez‑nous un avant / après, avec le film et les réglages qui vous plaisent : les plus belles images pourront rejoindre cette galerie, avec votre accord et votre nom.",
                )))
                .child(
                    a().class("btn btn-ghost magnetic")
                        .href(format!("mailto:{CONTACT_EMAIL}"))
                        .child(span().text(CONTACT_EMAIL)),
                ),
        )
}

/// Les nouveautés récentes : une carte par fonction, avec sa petite scène animée
/// (`showcase.js`, chargé à l'approche de la section).
fn recent(lang: Lang) -> Element {
    section()
        .class("recent")
        .id("recentes")
        .child(section_head(
            t(lang, "Nouveautés récentes"),
            heading(h2, t(lang, "Ce qui vient *d'arriver*.")),
            Some(p().text(t(lang,
                "Les dernières fonctions de LÜMA, faites et vérifiées par les tests. Sans IA : seulement la photo elle‑même.",
            ))),
        ))
        .child(div().class("recent-grid").children(RECENT.iter().map(|&(demo, tag, title, body)| {
            article()
                .class("recent-card reveal tilt")
                .child(
                    div()
                        .class("recent-demo")
                        .attr("aria-hidden", "true")
                        .child(canvas().attr("data-demo", demo).attr("width", "640").attr("height", "400")),
                )
                .child(
                    div()
                        .class("recent-copy")
                        .child(span().class("recent-tag").text(t(lang, tag)))
                        .child(h3().text(t(lang, title)))
                        .child(p().text(t(lang, body))),
                )
        })))
        .child(
            p().class("cap-more reveal").child(
                a().href(page_link(lang, "/nouveautes/"))
                    .text(t(lang, "Toutes les notes de version →")),
            ),
        )
}

/// Le comparatif : LÜMA à côté de Lightroom Classic, Capture One et darktable, sur des
/// informations publiques ; les prix des autres en modèle, sans montant.
fn compare(lang: Lang) -> Element {
    let luma: Vec<String> = COMPARE_LUMA
        .iter()
        .map(|s| {
            fill(
                t(lang, s),
                &[
                    ("month", t(lang, PLANS[0].1)),
                    ("year", t(lang, PLANS[1].1)),
                ],
            )
        })
        .collect();
    let head = thead().child(
        tr().child(
            th().attr("scope", "col")
                .child(span().class("sr-only").text(t(lang, "Critère"))),
        )
        .child(th().attr("scope", "col").class("cmp-luma").text("LÜMA"))
        .children(
            COMPARE_OTHERS
                .iter()
                .map(|(name, _)| th().attr("scope", "col").text(*name)),
        ),
    );
    let rows = COMPARE_ROWS.iter().enumerate().map(|(r, row)| {
        tr().child(th().attr("scope", "row").text(t(lang, row)))
            .child(
                td().class("cmp-luma")
                    .attr("data-label", "LÜMA")
                    .text(luma[r].clone()),
            )
            .children(
                COMPARE_OTHERS
                    .iter()
                    .map(|(name, cells)| td().attr("data-label", *name).text(t(lang, cells[r]))),
            )
    });
    section()
        .class("compare")
        .id("comparatif")
        .child(section_head(
            t(lang, "Comparatif"),
            heading(h2, t(lang, "LÜMA, *et les autres*.")),
            Some(p().text(t(lang,
                "Quatre éditeurs RAW, chacun avec ses choix : où ils tournent, comment ils se paient, et ce qu'ils savent faire hors ligne.",
            ))),
        ))
        .child(
            div()
                .class("cmp-wrap reveal")
                .child(table().class("cmp").child(head).child(tbody().children(rows))),
        )
        .child(p().class("films-note reveal").text(t(lang,
            "Informations publiques, octobre 2026 ; vérifiez auprès de chaque éditeur. Les noms cités sont des marques de leurs propriétaires.",
        )))
}

/// L'aide : les premiers pas, la documentation, les questions, et l'adresse où écrire.
fn help(lang: Lang) -> Element {
    section()
        .class("help")
        .id("aide")
        .child(section_head(
            t(lang, "Aide et contact"),
            heading(h2, t(lang, "Bien démarrer, *en quatre gestes*.")),
            None,
        ))
        .child(ol().class("help-steps").children(HELP_STEPS.iter().enumerate().map(|(n, &(title, body))| {
            li().class("help-step reveal")
                .child(span().class("help-num").attr("aria-hidden", "true").text(format!("{}", n + 1)))
                .child(h3().text(t(lang, title)))
                .child(p().text(t(lang, body)))
        })))
        .child(
            div()
                .class("help-grid")
                .child(
                    nav()
                        .class("help-links")
                        .attr("aria-label", t(lang, "Documentation"))
                        .children(HELP_LINKS.iter().map(|&(title, body, href)| {
                            a().class("help-link reveal")
                                .href(target(lang, href))
                                .child(b().text(t(lang, title)))
                                .child(span().text(t(lang, body)))
                        })),
                )
                .child(
                    div()
                        .class("help-contact reveal")
                        .id("contact")
                        .child(p().class("kicker").text(t(lang, "Contact")))
                        .child(heading(h3, t(lang, "Une question ? *Écrivez‑nous.*")))
                        .child(p().text(t(lang,
                            "Un RAW qui s'ouvre mal, une question sur l'abonnement, une idée : écrivez avec la version de LÜMA et, si possible, le fichier concerné.",
                        )))
                        .child(
                            a().class("btn magnetic")
                                .href(format!("mailto:{CONTACT_EMAIL}"))
                                .child(span().text(CONTACT_EMAIL)),
                        ),
                ),
        )
}

/// Vos photos restent chez vous.
fn privacy(lang: Lang) -> Element {
    section()
        .class("privacy")
        .id("vie-privee")
        .child(section_head(
            t(lang, "Vos photos"),
            heading(h2, t(lang, "Chez vous, *et nulle part ailleurs*.")),
            None,
        ))
        .child(
            div()
                .class("privacy-grid")
                .children(PRIVACY.iter().map(|&(title, body)| {
                    div()
                        .class("privacy-item reveal")
                        .child(span().class("privacy-mark").attr("aria-hidden", "true"))
                        .child(h3().text(t(lang, title)))
                        .child(p().text(t(lang, body)))
                })),
        )
}

/// Les questions fréquentes : chaque réponse se déplie (sans script).
fn faq(lang: Lang) -> Element {
    section()
        .class("faq")
        .id("questions")
        .child(section_head(
            t(lang, "Questions"),
            heading(h2, t(lang, "Vous vous *demandez*…")),
            None,
        ))
        .child(div().class("faq-list").children(FAQ.iter().map(|&(q, a)| {
            details()
                .class("faq-item reveal")
                .child(summary().text(t(lang, q)))
                .child(p().text(t(lang, a)))
        })))
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
                "Mesurés à chaque version sur un Mac à puce Apple, avec cinq vrais RAW : Sony, Canon, Nikon, Fujifilm et iPhone ProRAW. Sur iPhone, ce sont encore des objectifs.",
            ))),
        ))
        .child(div().class("perf-grid").children(cards))
        .child(
            div()
                .class("phone-gains reveal")
                .child(h3().text(t(lang, "Sur iPhone et iPad")))
                .child(ul().children(PHONE_GAINS.iter().map(|&(title, detail)| {
                    li().child(span().class("cap-check").attr("aria-hidden", "true").text("✔"))
                        .child(
                            span()
                                .child(strong().text(t(lang, title)))
                                .child(span().text(t(lang, detail))),
                        )
                })))
                .child(p().class("phone-gains-note").text(t(lang,
                    "Gains mesurés avec le même moteur sur Mac et sur Linux. Les temps sur iPhone et iPad remplaceront les objectifs dès les essais sur appareil.",
                ))),
        )
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

/// Une fonction de `docs/fonctions.md`, sans ce qui parle au développeur : les
/// parenthèses et les morceaux de phrase qui contiennent du code (`` `…` ``), les listes
/// de code, puis le code qui resterait.
fn plain(feature: &str) -> String {
    let has_code = |s: &str| s.contains('`');
    // Les parenthèses qui contiennent du code.
    let mut out = String::new();
    let mut rest = feature;
    while let Some(open) = rest.find('(') {
        let Some(len) = rest[open..].find(')') else {
            break;
        };
        let inner = &rest[open..open + len + 1];
        out.push_str(&rest[..open]);
        if !has_code(inner) {
            out.push_str(inner);
        }
        rest = &rest[open + len + 1..];
    }
    out.push_str(rest);
    // Les morceaux de phrase (après un « ; ») qui contiennent du code ; le premier
    // reste, son code sera retiré.
    let mut text = out
        .split(';')
        .enumerate()
        .filter(|(i, part)| *i == 0 || !has_code(part))
        .map(|(_, part)| part)
        .collect::<Vec<_>>()
        .join(";");
    // Le code qui reste, et les virgules qui le séparaient.
    while let Some(start) = text.find('`') {
        let Some(len) = text[start + 1..].find('`') else {
            break;
        };
        let mut end = start + len + 2;
        let tail = &text[end..];
        let skip = tail.len() - tail.trim_start_matches([',', ' ', '…']).len();
        end += skip;
        text.replace_range(start..end, "");
    }
    // Les espaces laissés devant une ponctuation, ou en double.
    let mut text = text
        .replace(" ,", ",")
        .replace(" :,", " :")
        .replace("  ", " ");
    for p in [" :", " ;", ","] {
        while text.trim_end().ends_with(p.trim()) {
            text = text.trim_end().trim_end_matches(p.trim()).to_owned();
        }
    }
    text.trim().to_owned()
}

/// Ce que LÜMA fait déjà : chaque fonction faite et testée de `docs/fonctions.md`,
/// cochée, domaine par domaine.
fn capabilities(lang: Lang) -> Element {
    let f = super::docs::features(lang);
    let groups = f.groups.iter().map(|(title, items)| {
        article()
            .class("cap-group reveal")
            .child(h3().text(title.clone()))
            // L'ordre de chaque coche : elles se posent une à une à l'écran.
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
                                .children(rich(&plain(feature))),
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
            "Un mois pour essayer, tout compris, sans carte bancaire. Vos photos n'attendent que vous.",
        )))
        .child(
            div()
                .class("hero-cta reveal")
                .child(a().class("btn magnetic").href("#tarifs").child(span().text(if downloads_open() {
                    t(lang, "Télécharger · un mois d'essai")
                } else {
                    t(lang, "Voir les tarifs")
                })))
                .child(a().class("btn btn-ghost magnetic").href("#questions").child(span().text(t(lang, "Questions fréquentes")))),
        )
}

/// Un prix en trois morceaux : ce qui précède le montant, le montant, ce qui le suit.
fn price_parts(price: &str) -> (&str, &str, &str) {
    let start = price.find(|c: char| c.is_ascii_digit()).unwrap_or(0);
    let end = price
        .rfind(|c: char| c.is_ascii_digit())
        .map_or(price.len(), |i| i + 1);
    (&price[..start], &price[start..end], &price[end..])
}

/// Tarifs et téléchargement : LÜMA Premium, un mois d'essai, puis au mois ou à l'an.
/// Les liens de paiement viennent de `LUMA_CHECKOUT_MONTHLY` et `LUMA_CHECKOUT_ANNUAL`
/// (liens de paiement Stripe) ; sans eux, les boutons attendent.
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
            heading(h2, t(lang, "Un mois pour *essayer*, puis à vous de choisir.")),
            Some(p().class("lede").text(t(lang,
                "Téléchargez LÜMA : tout est ouvert pendant l'essai. Ensuite, un abonnement au mois ou à l'an. Sur Mac, Linux et Windows, il s'achète uniquement ici, payé par Stripe ; votre clé de licence s'affiche aussitôt, et se saisit dans l'app. Sur iPhone et iPad (iOS et iPadOS), LÜMA Premium s'achète une fois pour toutes, uniquement dans l'App Store.",
            ))),
        ))
        .children(downloads_open().then(|| {
            // Une colonne par système : le bouton, puis ce qu'il faut savoir.
            div()
                .class("download reveal")
                .child(
                    div()
                        .class("download-option")
                        .child(
                            a().class("btn magnetic")
                                .href(download("LUMA-macos.zip"))
                                .attr("download", "")
                                .child(span().html(APPLE_SVG))
                                .child(span().text(t(lang, "Télécharger pour Mac"))),
                        )
                        .child(p().class("download-note").text(download_note(lang))),
                )
                .child(
                    div()
                        .class("download-option")
                        .child(
                            a().class("btn btn-ghost magnetic")
                                .href(download("LUMA-linux-x86_64.tar.gz"))
                                .attr("download", "")
                                .child(span().html(LINUX_SVG))
                                .child(span().text(t(lang, "Télécharger pour Linux"))),
                        )
                        .child(
                            p().class("download-note")
                                .text(t(lang, "Linux x86_64 (Ubuntu 22.04, Debian 12, Fedora 36 ou plus récent) : l'archive s'installe sans mot de passe avec install.sh ; il faut un pilote graphique Vulkan."))
                                .child(text(" "))
                                .child(
                                    a().href(download("LUMA-linux-amd64.deb"))
                                        .attr("download", "")
                                        .text(t(lang, "Paquet .deb (Debian, Ubuntu)")),
                                ),
                        ),
                )
                .child(windows_download(lang))
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
                        .child(p().class("footer-tagline").text(t(
                            lang,
                            "L'éditeur photo qui développe la lumière. Pour Mac, iPhone, iPad, Linux et Windows.",
                        )))
                        .child(
                            div()
                                .class("footer-apps")
                                .child(if downloads_open() {
                                    a().class("store")
                                        .href(download("LUMA-macos.zip"))
                                        .attr("download", "")
                                        .child(span().html(APPLE_SVG))
                                        .child(
                                            span()
                                                .child(small().text(t(lang, "Télécharger pour")))
                                                .child(strong().text("Mac")),
                                        )
                                } else {
                                    span()
                                        .class("store store-soon")
                                        .attr("aria-disabled", "true")
                                        .child(span().html(APPLE_SVG))
                                        .child(
                                            span()
                                                .child(small().text(t(lang, "Bientôt sur")))
                                                .child(strong().text("Mac")),
                                        )
                                })
                                .children(downloads_open().then(|| {
                                    a().class("store")
                                        .href(download("LUMA-linux-x86_64.tar.gz"))
                                        .attr("download", "")
                                        .child(span().html(LINUX_SVG))
                                        .child(
                                            span()
                                                .child(small().text(t(lang, "Télécharger pour")))
                                                .child(strong().text("Linux")),
                                        )
                                }))
                                .child(match windows_primary() {
                                    Some(file) => a()
                                        .class("store")
                                        .href(download(file))
                                        .attr("download", "")
                                        .child(span().html(WINDOWS_SVG))
                                        .child(
                                            span()
                                                .child(small().text(t(lang, "Télécharger pour")))
                                                .child(strong().text("Windows")),
                                        ),
                                    None => span()
                                        .class("store store-soon")
                                        .attr("aria-disabled", "true")
                                        .child(span().html(WINDOWS_SVG))
                                        .child(
                                            span()
                                                .child(small().text(t(lang, "Bientôt sur")))
                                                .child(strong().text("Windows")),
                                        ),
                                })
                                .child(
                                    span()
                                        .class("store store-soon")
                                        .attr("aria-disabled", "true")
                                        .child(span().html(APPLE_SVG))
                                        .child(
                                            span()
                                                .child(small().text(t(lang, "Bientôt sur")))
                                                .child(strong().text(t(lang, "l'App Store"))),
                                        ),
                                ),
                        ),
                )
                .child(
                    div()
                        .class("footer-cols")
                        .children(FOOTER.iter().map(|c| column(*c))),
                ),
        )
        // La signature, au centre : l'auteur, l'origine, puis le nom de l'app en grand.
        .child(
            div()
                .class("footer-signature")
                .child(
                    p().class("footer-signature-label")
                        .text(t(lang, "Conçu et développé par")),
                )
                .child(
                    p().class("footer-signature-name")
                        .text("Maxime Nathan Lestage"),
                )
                .child(heading(p, t(lang, "Fabriqué *en France*")).class("footer-signature-made"))
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
                .child(span().text(t(
                    lang,
                    "© 2026 Maxime Nathan Lestage. Tous droits réservés.",
                )))
                .child(span().text(t(lang, "Logiciel propriétaire")))
                .child(lang_switch(here, true))
                .child(
                    a().class("footer-made-with")
                        .href("https://github.com/maxlestage/Paquets")
                        .attr("target", "_blank")
                        .attr("rel", "noopener")
                        .text(t(lang, "Site fait avec Paquets ↗")),
                )
                .child(
                    a().href(target(lang, "#top"))
                        .text(t(lang, "Haut de page ↑")),
                ),
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
        Ok((claims, key)) => {
            let offered = claims.sub == super::offerts::SUB;
            let email = if claims.email.is_empty() { String::new() } else { format!(" ({})", claims.email) };
            let lede = if offered {
                fill(t(lang, "Votre accès LÜMA Premium offert est actif{email}."), &[("email", email.as_str())])
            } else {
                fill(
                    t(lang, "Votre abonnement LÜMA Premium {plan} est actif{email}."),
                    &[("plan", t(lang, claims.plan.name()).to_lowercase().as_str()), ("email", email.as_str())],
                )
            };
            div()
            .child(h1().text(t(lang, "Merci !")))
            .child(p().class("lede").text(lede))
            .child(section().id("cle")
                .child(h2().text(t(lang, "Votre clé de licence")))
                .child(p().text(if offered {
                    t(lang, "Copiez‑la, puis collez‑la dans LÜMA : la puce « Essai » en haut de l'app, champ « Clé de licence », Activer. Elle marche sur tous vos appareils (Mac, Linux, Windows), et se renouvelle seule tant que votre accès offert court.")
                } else {
                    t(lang, "Copiez‑la, puis collez‑la dans LÜMA : la puce « Essai » en haut de l'app, champ « Clé de licence », Activer. Elle marche sur tous vos appareils, et se renouvelle seule tant que l'abonnement court.")
                }))
                .child(textarea().class("licence-key").attr("readonly", "").attr("rows", "4").attr("aria-label", t(lang, "Clé de licence")).text(key))
                .child(p().class("todo").text(if offered {
                    t(lang, "Votre courriel et votre code redonnent votre clé à tout moment, sur cette page.")
                } else {
                    t(lang, "Gardez l'adresse de cette page (favori) : elle redonne votre clé à tout moment.")
                })))
            .child(section()
                .child(h2().text(t(lang, "Pas encore l'app ?")))
                .child(div().class("hero-cta")
                    .children(downloads_open().then(|| a().class("btn").href(download("LUMA-macos.zip")).attr("download", "").child(span().text(t(lang, "Télécharger pour Mac")))))
                    .children(downloads_open().then(|| a().class("btn btn-ghost").href(download("LUMA-linux-x86_64.tar.gz")).attr("download", "").child(span().text(t(lang, "Télécharger pour Linux")))))
                    .children(windows_primary().map(|file| a().class("btn btn-ghost").href(download(file)).attr("download", "").child(span().text(t(lang, "Télécharger pour Windows")))))
                    .child(portal.map(|u| a().class("btn btn-ghost").href(u).child(span().text(t(lang, "Gérer mon abonnement")))))))
        }

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

/// La page de l'accès offert : le courriel et le code reçus, pour obtenir sa clé.
/// `error` : le courriel tapé et ce qui n'a pas marché.
pub fn offert(site: &Site, origin: &str, here: Here, error: Option<(String, String)>) -> Page {
    let lang = here.lang;
    let (email, problem) = error.map_or((String::new(), None), |(e, why)| (e, Some(why)));
    let field = |name: &'static str,
                 kind: &'static str,
                 title: &'static str,
                 value: String,
                 auto: &'static str| {
        label()
            .class("offert-field")
            .child(span().text(title))
            .child(
                input()
                    .attr("type", kind)
                    .attr("name", name)
                    .attr("required", "")
                    .attr("autocomplete", auto)
                    .attr("value", value),
            )
    };
    let content = div()
        .child(h1().text(t(lang, "Accès offert")))
        .child(p().class("lede").text(t(lang, "On vous a offert LÜMA Premium ? Tapez le courriel et le code reçus : votre clé de licence s'affiche, prête à coller dans l'app.")))
        .children(problem.map(|why| {
            p().class("offert-error").attr("role", "alert").text(fill(
                t(lang, "Impossible d'afficher la clé : {error}."),
                &[("error", t(lang, &why))],
            ))
        }))
        .child(
            form()
                .class("offert-form")
                .attr("method", "post")
                .attr("action", page_link(lang, "/licence/offert/"))
                .child(field("email", "email", t(lang, "Courriel"), email, "email"))
                .child(field("code", "text", t(lang, "Code"), String::new(), "off"))
                .child(button().class("btn").attr("type", "submit").child(span().text(t(lang, "Obtenir ma clé")))),
        )
        .child(p().text(t(lang, "Le code vous est donné avec l'invitation. Pas de code ? LÜMA s'essaie gratuitement, puis s'achète sur ce site.")));
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
        &format!("{} — LÜMA", t(lang, "Accès offert")),
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
                .child(steps(lang))
                .child(pipeline(lang))
                .child(lab(lang))
                .child(films(lang))
                .child(gallery(lang))
                .child(recent(lang))
                .child(apple(lang))
                .child(desktop(lang))
                .child(capabilities(lang))
                .child(privacy(lang))
                .child(performance(lang))
                .child(platforms(lang))
                .child(compare(lang))
                .child(pricing(lang))
                .child(faq(lang))
                .child(help(lang))
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
    hosted_downloads() || release_downloads().is_some()
}

/// Les fichiers de l'app sont‑ils servis par le site lui‑même (`LUMA_DOWNLOADS=1`,
/// sous `/download/`) ?
fn hosted_downloads() -> bool {
    std::env::var("LUMA_DOWNLOADS").is_ok_and(|v| v == "1")
}

/// L'adresse des fichiers de la dernière Release GitHub
/// (`LUMA_RELEASE_DOWNLOADS`, par exemple
/// `https://github.com/<compte>/<dépôt>/releases/latest/download`), quand le site ne
/// sert pas les fichiers lui‑même : les boutons « Télécharger » y mènent.
fn release_downloads() -> Option<String> {
    std::env::var("LUMA_RELEASE_DOWNLOADS")
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_owned())
        .filter(|v| v.starts_with("https://"))
}

/// Le lien d'un fichier de l'app : sur le site s'il le sert, sinon dans la Release.
fn download(file: &str) -> String {
    download_href(file, hosted_downloads(), release_downloads().as_deref())
}

/// [`download`], d'après ce qui est servi : le site d'abord, la Release ensuite ;
/// sans l'un ni l'autre, l'adresse du site (les boutons n'apparaissent alors pas).
fn download_href(file: &str, hosted: bool, release: Option<&str>) -> String {
    match release {
        Some(base) if !hosted => format!("{base}/{file}"),
        _ => link(&format!("/download/{file}")),
    }
}

/// L'archive de LÜMA pour Windows, telle que la Release la publie.
pub const WINDOWS_ZIP: &str = "LUMA-windows-x86_64.zip";
/// L'installateur de LÜMA pour Windows.
pub const WINDOWS_SETUP: &str = "LUMA-windows-x86_64-setup.exe";

/// Ce fichier de Windows est‑il à télécharger ? Comme pour Mac et Linux, il faut que le
/// téléchargement soit ouvert (`LUMA_DOWNLOADS=1`) ; il faut en plus que la Release
/// l'ait apporté sous `public/download/` (Heroku, `heroku-build.sh`), ou, pour un export
/// statique où les fichiers sont copiés après le rendu, que `LUMA_WINDOWS_DOWNLOADS=1`
/// l'annonce ; ou que les boutons mènent à la Release GitHub (`LUMA_RELEASE_DOWNLOADS`),
/// qui a toujours les deux fichiers de Windows. Sinon, le site dit « Bientôt » : aucun
/// bouton vers une page introuvable.
pub fn windows_asset(name: &str) -> bool {
    downloads_open()
        && (release_downloads().is_some() && !hosted_downloads()
            || std::env::var("LUMA_WINDOWS_DOWNLOADS").is_ok_and(|v| v == "1")
            || super::site_dir()
                .join("public/download")
                .join(name)
                .is_file())
}

/// Le téléchargement pour Windows est‑il là (l'installateur ou l'archive) ?
pub fn windows_open() -> bool {
    windows_primary().is_some()
}

/// Le fichier du bouton « Télécharger pour Windows » : l'installateur s'il est là,
/// l'archive sinon.
fn windows_primary() -> Option<&'static str> {
    [WINDOWS_SETUP, WINDOWS_ZIP]
        .into_iter()
        .find(|f| windows_asset(f))
}

/// La colonne Windows des téléchargements (tarifs) : le bouton et l'archive quand ils
/// sont là, « Bientôt » sinon.
fn windows_download(lang: Lang) -> Element {
    let note = p().class("download-note").text(t(
        lang,
        "Windows 10 ou 11, 64 bits (x86_64), avec DirectX 12 ou Vulkan.",
    ));
    match windows_primary() {
        Some(file) => div()
            .class("download-option")
            .child(
                a().class("btn btn-ghost magnetic")
                    .href(download(file))
                    .attr("download", "")
                    .child(span().html(WINDOWS_SVG))
                    .child(span().text(t(lang, "Télécharger pour Windows"))),
            )
            .child(if file == WINDOWS_SETUP && windows_asset(WINDOWS_ZIP) {
                note.child(text(" ")).child(
                    a().href(download(WINDOWS_ZIP))
                        .attr("download", "")
                        .text(t(lang, "Archive .zip, sans installation")),
                )
            } else {
                note
            }),
        None => div()
            .class("download-option")
            .child(
                span()
                    .class("btn btn-ghost btn-disabled")
                    .attr("aria-disabled", "true")
                    .child(span().html(WINDOWS_SVG))
                    .child(span().text(t(lang, "Bientôt sur Windows"))),
            )
            .child(note),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le lien d'un fichier de l'app : sur le site quand il le sert (même s'il y a une
    /// Release), dans la Release sinon.
    #[test]
    fn downloads_come_from_the_site_or_the_release() {
        let release = "https://github.com/compte/depot/releases/latest/download";
        assert_eq!(
            download_href("LUMA-macos.zip", false, Some(release)),
            format!("{release}/LUMA-macos.zip")
        );
        assert!(
            download_href("LUMA-macos.zip", true, Some(release))
                .ends_with("/download/LUMA-macos.zip")
        );
        assert!(
            !download_href("LUMA-macos.zip", true, Some(release)).starts_with("https://github.com")
        );
        assert!(download_href("LUMA-macos.zip", false, None).ends_with("/download/LUMA-macos.zip"));
    }

    use super::{WINDOWS_SETUP, WINDOWS_ZIP, plain};

    #[test]
    fn windows_files_match_the_release() {
        // Les noms que publie scripts/bundle-windows.ps1 (et la Release).
        assert_eq!(WINDOWS_ZIP, "LUMA-windows-x86_64.zip");
        assert_eq!(WINDOWS_SETUP, "LUMA-windows-x86_64-setup.exe");
    }

    #[test]
    fn features_lose_their_code() {
        assert_eq!(
            plain("Couleur, Neutre, Vif, Monochrome ; lecture de `crs:CameraProfile`"),
            "Couleur, Neutre, Vif, Monochrome"
        );
        assert_eq!(
            plain("DNG linéaire, Apple ProRAW (avec `BaselineExposure`)"),
            "DNG linéaire, Apple ProRAW"
        );
        assert_eq!(
            plain(
                "Modèles de nom des fichiers : `{nom}`, `{date}`, `{iso}`…, sous‑dossiers par date"
            ),
            "Modèles de nom des fichiers : sous‑dossiers par date"
        );
        assert_eq!(
            plain(
                "Modèles : `{nom}`, `{date}`…, sous‑dossiers\u{a0}; dans la fenêtre et `luma develop --name`"
            ),
            "Modèles : sous‑dossiers"
        );
        assert_eq!(
            plain("Grain (gain, taille, rugosité)"),
            "Grain (gain, taille, rugosité)"
        );
    }
}
