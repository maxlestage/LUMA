//! Les parties interactives du site. Chaque îlot est rendu par le serveur comme une vue
//! ordinaire puis, compilé en WebAssembly, s'hydrate dans le navigateur à partir du même
//! code.
//!
//! Les props d'un îlot sont le code de la langue de la page (`fr`, `en`, `es`) : le
//! serveur et le navigateur rendent ainsi les mêmes textes, et l'hydratation retrouve
//! exactement le HTML reçu.

use paquets::prelude::*;

use crate::content::{CONTROLS, CRATES, PRESETS, PRIORITIES, format_control};
use crate::i18n::{from_props, t};

/// Chaque îlot, enregistré sous le nom donné à [`island`].
pub const ISLANDS: [(&str, paquets::active::IslandFn); 2] = [("lab", lab), ("crates", crates)];

// ----- Labo ---------------------------------------------------------------------------

/// Le labo : l'aperçu (un canvas WebGL piloté par `client::lab`), la barre avant/après
/// et le panneau de réglages. Les réglages sont des états ; à chaque changement, ils
/// sont transmis au rendu, qui glisse vers eux en douceur.
pub fn lab(props: &str) -> Node {
    let lang = from_props(props);
    let start = PRESETS[0].2;
    let values: [State<f64>; 8] = std::array::from_fn(|i| use_state(start[i]));
    let preset = use_state(Some(0usize));

    effect(move || {
        let target: [f64; 8] = std::array::from_fn(|i| values[i].get());
        #[cfg(target_arch = "wasm32")]
        crate::client::lab::set_target(target);
        let _ = target;
    });

    let view = div()
        .class("lab-view")
        .child(
            canvas()
                .id("lab-gl")
                .attr("aria-label", t(lang, "Aperçu de la photo retouchée"))
                .on_mount(|_canvas| {
                    #[cfg(target_arch = "wasm32")]
                    crate::client::lab::start(_canvas);
                }),
        )
        .child(
            div()
                .class("lab-split")
                .attr("role", "slider")
                .attr("tabindex", "0")
                .attr("aria-label", t(lang, "Comparer avant et après"))
                .attr("aria-valuemin", "0")
                .attr("aria-valuemax", "100")
                .attr("aria-valuenow", "50")
                .child(span()),
        )
        .child(span().class("lab-tag lab-tag-l").text(t(lang, "Original")))
        .child(span().class("lab-tag lab-tag-r").text("LÜMA"))
        .child(
            span()
                .class("lab-fps")
                .text(format!("— {}", t(lang, "i/s"))),
        );

    let presets = div()
        .class("lab-presets")
        .children(
            PRESETS
                .iter()
                .enumerate()
                .map(move |(i, &(_, name, vals))| {
                    button()
                        .attr("type", "button")
                        .class_if("on", move || preset.get() == Some(i))
                        .attr_dyn("aria-pressed", move || {
                            (preset.get() == Some(i)).to_string()
                        })
                        .on_click(move |_| {
                            for (state, v) in values.iter().zip(vals) {
                                state.set(v);
                            }
                            preset.set(Some(i));
                        })
                        .text(t(lang, name))
                }),
        );

    let controls =
        CONTROLS
            .iter()
            .enumerate()
            .map(move |(i, &(key, name, min, max, track))| {
                let class = if track.is_empty() {
                    "ctl".to_owned()
                } else {
                    format!("ctl {track}")
                };
                label()
                    .class(class)
                    .child(span().text(t(lang, name)).child(
                        output().text_dyn(move || format_control(lang, key, values[i].get())),
                    ))
                    .child(
                        input()
                            .id(format!("lab-{key}"))
                            .attr("type", "range")
                            .attr("min", min.to_string())
                            .attr("max", max.to_string())
                            .attr("step", "0.01")
                            .bind_parsed(values[i])
                            .on("input", move |_| preset.set(None)),
                    )
            });

    div()
        .class("lab-grid reveal")
        .child(view)
        .child(
            div()
                .class("lab-panel")
                .child(presets)
                .children(controls)
                .child(p().class("lab-note").text(t(
                lang,
                "Paramètres enregistrés, pixels d'origine intacts : c'est la règle d'or de LÜMA.",
            ))),
        )
        .into()
}

// ----- Architecture -------------------------------------------------------------------

/// Les crates du workspace, filtrables par priorité.
pub fn crates(props: &str) -> Node {
    let lang = from_props(props);
    let filter = use_state("all");
    fragment([
        div()
            .class("chips reveal")
            .attr("role", "group")
            .attr("aria-label", t(lang, "Filtrer par priorité"))
            .children(PRIORITIES.iter().map(move |&(key, name)| {
                button()
                    .attr("type", "button")
                    .class_if("on", move || filter.get() == key)
                    .attr_dyn("aria-pressed", move || (filter.get() == key).to_string())
                    .on_click(move |_| filter.set(key))
                    .text(t(lang, name))
            })),
        div().class("crates").children(CRATES.iter().map(move |c| {
            let priority = c.priority;
            article()
                .class("crate reveal tilt")
                .attr("data-p", priority)
                .class_if("out", move || {
                    let f = filter.get();
                    f != "all" && f != priority
                })
                .child(
                    div()
                        .class("glyph")
                        .attr("aria-hidden", "true")
                        .text(c.glyph),
                )
                .child(span().class("prio").text(priority))
                .child(h3().text(c.name))
                .child(p().text(t(lang, c.text)))
        })),
    ])
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn the_lab_renders_its_controls_and_the_natural_preset() {
        let html = island("lab", "", lab).render();
        assert_eq!(html.matches(r#"type="range""#).count(), CONTROLS.len());
        assert!(html.contains("+0.15 IL"), "exposure of the natural preset");
        assert!(html.contains(r#"<canvas id="lab-gl""#));
    }

    #[test]
    fn the_lab_speaks_the_language_of_its_props() {
        let en = island("lab", "en", lab).render();
        assert!(en.contains(r#"data-props="en""#));
        assert!(en.contains("+0.15 EV") && en.contains("Exposure") && en.contains("Natural"));
        assert!(!en.contains("Exposition") && !en.contains("Naturel"));
        let es = island("crates", "es", crates).render();
        assert!(es.contains("Todo") && !es.contains(">Tout<"));
    }

    #[test]
    fn every_crate_is_rendered() {
        let html = island("crates", "", crates).render();
        assert_eq!(html.matches("<article").count(), CRATES.len());
        assert!(!html.contains(r#"class="crate reveal tilt out""#));
    }
}
