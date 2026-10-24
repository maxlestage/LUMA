//! Les mouvements de la page : préchargement, révélations, barre de progression,
//! navigation, curseur, boutons magnétiques, inclinaison 3D, compteurs, manifeste mot à
//! mot, pipeline 3D, feuille de route horizontale et terminal. Sans WebAssembly, la page
//! reste complète et statique.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, MouseEvent};

use super::*;

/// Les fonctions rappelées à chaque image où la page a défilé.
type Scroller = Box<dyn Fn()>;

thread_local! {
    static SCROLLERS: RefCell<Vec<Scroller>> = RefCell::new(Vec::new());
}

fn on_scroll(f: impl Fn() + 'static) {
    SCROLLERS.with(|s| s.borrow_mut().push(Box::new(f)));
}

fn run_scrollers() {
    SCROLLERS.with(|s| s.borrow().iter().for_each(|f| f()));
}

/// Avancement (0 → 1) d'une section collante pendant qu'elle traverse l'écran.
fn sticky(el: &Element) -> f64 {
    let r = el.get_bounding_client_rect();
    clamp01(-r.top() / (r.height() - inner_height()).max(1.0))
}

pub fn init() {
    let reduced = reduced_motion();
    loader(reduced);
    reveals();
    chrome();
    if fine_pointer() && !reduced {
        let _ = root().class_list().add_1("has-cursor");
        cursor();
        magnetic();
        tilt();
    }
    counters(reduced);
    manifesto();
    pipeline();
    roadmap();
    terminal(reduced);

    // Une seule boucle de défilement pour toutes les sections collantes.
    let pending = Rc::new(Cell::new(false));
    let schedule = move |_: web_sys::Event| {
        if pending.replace(true) {
            return;
        }
        let pending = pending.clone();
        request_frame(move |_| {
            pending.set(false);
            run_scrollers();
            false
        });
    };
    on_passive(&window(), "scroll", schedule.clone());
    on_passive(&window(), "resize", schedule);
    run_scrollers();
}

// ----- Préchargement --------------------------------------------------------------------

/// Un diaphragme qui s'ouvre et un compteur, puis le rideau se lève et les scènes 3D
/// reçoivent `luma:intro`.
fn loader(reduced: bool) {
    let Some(el) = one(".loader") else {
        finish_loading();
        return;
    };
    let blades: Vec<Element> = {
        let list = el.query_selector_all(".blades path").ok();
        list.map(|l| {
            (0..l.length())
                .filter_map(|i| l.item(i)?.dyn_into().ok())
                .collect()
        })
        .unwrap_or_default()
    };
    let count = el.query_selector(".loader-count span").ok().flatten();
    let ready = Rc::new(Cell::new(document().ready_state() == "complete"));
    {
        let ready = ready.clone();
        on(&window(), "load", move |_: web_sys::Event| ready.set(true));
    }
    {
        let ready = ready.clone();
        on(&el, "click", move |_: web_sys::Event| ready.set(true));
    }
    let start = now();
    let duration = if reduced { 200.0 } else { 1400.0 };
    let n = blades.len().max(1) as f64;
    request_frame(move |time| {
        let t = clamp01((time - start) / duration);
        let shown = if ready.get() { t } else { t.min(0.86) };
        if let Some(count) = &count {
            count.set_text_content(Some(&format!("{}", (shown * 100.0).round())));
        }
        for (i, blade) in blades.iter().enumerate() {
            let angle = 360.0 / n * i as f64;
            let _ = blade.set_attribute(
                "transform",
                &format!("rotate({angle}) translate({:.2} {:.2})", t * 9.0, -t * 16.0),
            );
        }
        if (t >= 1.0 && ready.get()) || time - start > 4500.0 {
            finish_loading();
            return false;
        }
        true
    });
}

fn finish_loading() {
    let list = root().class_list();
    let _ = list.remove_1("loading");
    let _ = list.add_1("loaded");
    if let Ok(event) = web_sys::CustomEvent::new("luma:intro") {
        let _ = document().dispatch_event(&event);
    }
}

// ----- Révélations ----------------------------------------------------------------------

fn reveals() {
    let items = all(".reveal");
    // Décalage en cascade entre éléments voisins.
    for el in &items {
        if let Some(parent) = el.parent_element() {
            let children = parent.children();
            let mut index = 0;
            for i in 0..children.length() {
                let Some(child) = children.item(i) else {
                    continue;
                };
                if child == **el {
                    break;
                }
                if child.class_list().contains("reveal") {
                    index += 1;
                }
            }
            set_var(el, "--d", &format!("{}s", f64::from(index.min(6)) * 0.08));
        }
    }
    let height = inner_height();
    let mut later_ones = Vec::new();
    for el in items {
        let r = el.get_bounding_client_rect();
        if r.top() < height && r.bottom() > 0.0 {
            let _ = el.class_list().add_1("in");
        } else {
            later_ones.push(Element::from(el));
        }
    }
    observe(&later_ones, "0px 0px -8% 0px", 0.08, |entry, io| {
        if entry.is_intersecting() {
            let target = entry.target();
            let _ = target.class_list().add_1("in");
            io.unobserve(&target);
        }
    });
    // Seulement maintenant le CSS peut cacher ce qui n'est pas encore révélé : ce qui est
    // déjà à l'écran vient d'être marqué, rien ne clignote.
    let _ = root().class_list().add_1("hydrated");
}

// ----- Navigation et progression --------------------------------------------------------

fn chrome() {
    let bar = one(".progress");
    let nav = one(".nav");
    let last = Cell::new(window().scroll_y().unwrap_or(0.0));
    on_scroll(move || {
        let y = window().scroll_y().unwrap_or(0.0);
        let max = f64::from(
            document()
                .document_element()
                .map_or(0, |e| e.scroll_height()),
        ) - inner_height();
        if let Some(bar) = &bar {
            set_var(bar, "--p", &format!("{:.4}", y / max.max(1.0)));
        }
        if let Some(nav) = &nav
            && !root().class_list().contains("menu-open")
        {
            let _ = nav
                .class_list()
                .toggle_with_force("hide", y > last.get() && y > 400.0);
        }
        last.set(y);
    });

    // Le lien de la section affichée
    let links = all(".links a");
    let sections: Vec<Element> = links
        .iter()
        .filter_map(|a| {
            document()
                .query_selector(&a.get_attribute("href")?)
                .ok()
                .flatten()
        })
        .collect();
    let spy = links.clone();
    observe(&sections, "-45% 0px -50% 0px", 0.0, move |entry, _| {
        if entry.is_intersecting() {
            let id = format!("#{}", entry.target().id());
            for a in &spy {
                let _ = a
                    .class_list()
                    .toggle_with_force("active", a.get_attribute("href").as_deref() == Some(&id));
            }
        }
    });

    // Menu des téléphones
    if let Some(toggle) = one(".menu-toggle") {
        let button = toggle.clone();
        on(&toggle, "click", move |_: web_sys::Event| {
            let open = root().class_list().toggle("menu-open").unwrap_or(false);
            let _ = button.set_attribute("aria-expanded", &open.to_string());
        });
        for a in links {
            let toggle = toggle.clone();
            on(&a, "click", move |_: web_sys::Event| {
                let _ = root().class_list().remove_1("menu-open");
                let _ = toggle.set_attribute("aria-expanded", "false");
            });
        }
    }
}

// ----- Curseur, boutons magnétiques, inclinaison ----------------------------------------

fn cursor() {
    let (Some(ring), Some(dot)) = (one(".cursor"), one(".cursor-dot")) else {
        return;
    };
    let target = Rc::new(Cell::new((inner_width() / 2.0, inner_height() / 2.0)));
    {
        let target = target.clone();
        on_passive(&window(), "pointermove", move |e: MouseEvent| {
            let (x, y) = (f64::from(e.client_x()), f64::from(e.client_y()));
            target.set((x, y));
            let _ = dot
                .style()
                .set_property("transform", &format!("translate3d({x}px, {y}px, 0)"));
        });
    }
    let (mut rx, mut ry) = target.get();
    let style = ring.style();
    request_frame(move |_| {
        let (x, y) = target.get();
        rx += (x - rx) * 0.18;
        ry += (y - ry) * 0.18;
        let _ = style.set_property(
            "transform",
            &format!("translate3d({rx:.1}px, {ry:.1}px, 0)"),
        );
        true
    });
    on(&document(), "pointerover", move |e: web_sys::Event| {
        let hover = e
            .target()
            .and_then(|t| t.dyn_into::<Element>().ok())
            .and_then(|t| t.closest("a, button, input, .lab-split").ok().flatten())
            .is_some();
        let _ = ring.class_list().toggle_with_force("hover", hover);
    });
}

fn magnetic() {
    for el in all(".magnetic") {
        let inner: Option<HtmlElement> = el.first_element_child().and_then(|c| c.dyn_into().ok());
        let (target, label) = (el.clone(), inner.clone());
        on(&el, "pointermove", move |e: MouseEvent| {
            let r = target.get_bounding_client_rect();
            let dx = f64::from(e.client_x()) - (r.left() + r.width() / 2.0);
            let dy = f64::from(e.client_y()) - (r.top() + r.height() / 2.0);
            let _ = target.style().set_property(
                "transform",
                &format!("translate({:.1}px, {:.1}px)", dx * 0.25, dy * 0.35),
            );
            if let Some(label) = &label {
                let _ = label.style().set_property(
                    "transform",
                    &format!("translate({:.1}px, {:.1}px)", dx * 0.12, dy * 0.15),
                );
            }
        });
        let target = el.clone();
        on(&el, "pointerleave", move |_: web_sys::Event| {
            let _ = target.style().remove_property("transform");
            if let Some(label) = &inner {
                let _ = label.style().remove_property("transform");
            }
        });
    }
}

fn tilt() {
    for el in all(".tilt") {
        let target = el.clone();
        on(&el, "pointermove", move |e: MouseEvent| {
            let r = target.get_bounding_client_rect();
            let px = (f64::from(e.client_x()) - r.left()) / r.width();
            let py = (f64::from(e.client_y()) - r.top()) / r.height();
            set_var(&target, "--mx", &format!("{:.1}%", px * 100.0));
            set_var(&target, "--my", &format!("{:.1}%", py * 100.0));
            let _ = target.style().set_property(
                "transform",
                &format!(
                    "perspective(900px) rotateX({:.2}deg) rotateY({:.2}deg) translateZ(10px)",
                    (0.5 - py) * 10.0,
                    (px - 0.5) * 12.0
                ),
            );
        });
        let target = el.clone();
        on(&el, "pointerleave", move |_: web_sys::Event| {
            let _ = target.style().remove_property("transform");
        });
    }
}

// ----- Compteurs et manifeste ----------------------------------------------------------

/// Les nombres du HTML sont les vrais ; ils repartent de zéro pour monter à l'écran.
fn counters(reduced: bool) {
    if reduced {
        return;
    }
    let items = all("[data-count]");
    for el in &items {
        el.set_text_content(Some("0"));
    }
    let targets: Vec<Element> = items.into_iter().map(Element::from).collect();
    observe(&targets, "0px", 0.6, |entry, io| {
        if !entry.is_intersecting() {
            return;
        }
        let el = entry.target();
        io.unobserve(&el);
        let end: f64 = el
            .get_attribute("data-count")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0);
        let start = now();
        request_frame(move |time| {
            let t = clamp01((time - start) / 1600.0);
            let v = (end * (1.0 - (1.0 - t).powi(4))).round();
            el.set_text_content(Some(&format!("{v}")));
            t < 1.0
        });
    });
}

fn manifesto() {
    let Some(section) = one(".manifesto") else {
        return;
    };
    let words = all(".manifesto-text .w");
    on_scroll(move || {
        let p = sticky(&section);
        let lit = (clamp01(p * 1.25) * words.len() as f64).floor() as usize;
        for (i, w) in words.iter().enumerate() {
            let _ = w.class_list().toggle_with_force("on", i < lit);
        }
    });
}

// ----- Pipeline et feuille de route -----------------------------------------------------

fn pipeline() {
    let (Some(section), Some(stage)) = (one("#pipeline"), one(".pipeline-stage")) else {
        return;
    };
    let layers = all(".pipeline-stage .layer");
    let steps = all(".pipeline-steps li");
    // Les fonds : la photo que le serveur génère en SVG, à chaque stade du développement.
    let site = one(".site");
    let attr = |name: &str| {
        site.as_ref()
            .and_then(|s| s.get_attribute(name))
            .unwrap_or_default()
    };
    let (photo, mono) = (attr("data-photo"), attr("data-photo-mono"));
    let backgrounds = [
        String::new(),
        format!("url({mono})"),
        format!("url({photo})"),
        format!(
            "radial-gradient(ellipse at 62% 30%, rgba(139,123,255,.75), rgba(139,123,255,.15) 45%, transparent 60%), url({photo})"
        ),
        format!("linear-gradient(120deg, rgba(255,255,255,.18), transparent 40%), url({photo})"),
    ];
    for (layer, bg) in layers.iter().zip(&backgrounds) {
        if !bg.is_empty() {
            let _ = layer.style().set_property("background-image", bg);
        }
    }
    on_scroll(move || {
        let p = sticky(&section);
        let spread = clamp01(p * 1.6);
        let gap = if inner_width() < 1000.0 { 46.0 } else { 92.0 } * spread;
        set_var(&stage, "--rx", &format!("{:.2}deg", 58.0 - p * 10.0));
        set_var(&stage, "--rz", &format!("{:.2}deg", -36.0 + p * 22.0));
        let active = ((clamp01((p - 0.12) / 0.8) * 5.0).floor() as usize).min(4);
        let started = p > 0.1;
        for (i, layer) in layers.iter().enumerate() {
            set_var(layer, "--z", &format!("{:.1}", (i as f64 - 2.0) * gap));
            let _ = layer
                .class_list()
                .toggle_with_force("active", started && i == active);
            set_var(
                layer,
                "--o",
                if started && i > active { "0.35" } else { "1" },
            );
        }
        for (i, step) in steps.iter().enumerate() {
            let _ = step
                .class_list()
                .toggle_with_force("on", started && i == active);
        }
    });
}

fn roadmap() {
    let (Some(section), Some(track), Some(line)) =
        (one("#roadmap"), one(".roadmap-track"), one(".roadmap-line"))
    else {
        return;
    };
    let phases = all(".phase");
    on_scroll(move || {
        let p = sticky(&section);
        let max = (f64::from(track.scroll_width()) - inner_width()).max(0.0);
        set_var(&track, "--tx", &format!("{:.1}px", -p * max));
        set_var(&line, "--p", &format!("{p:.4}"));
        let lit = (p * phases.len() as f64 + 0.6).floor() as usize;
        for (i, phase) in phases.iter().enumerate() {
            let _ = phase.class_list().toggle_with_force("lit", i < lit);
        }
    });
}

// ----- Terminal -----------------------------------------------------------------------

/// Le `deny.toml` est dans le HTML ; à l'écran, il est retapé ligne par ligne.
fn terminal(reduced: bool) {
    if reduced {
        return;
    }
    let Some(term) = one("#term") else { return };
    let lines: Vec<String> = term.inner_html().split('\n').map(str::to_owned).collect();
    term.set_inner_html("<span class=\"caret\"></span>");
    observe(
        &[Element::from(term.clone())],
        "0px",
        0.4,
        move |entry, io| {
            if !entry.is_intersecting() {
                return;
            }
            io.disconnect();
            type_line(term.clone(), Rc::new(lines.clone()), 0);
        },
    );
}

fn type_line(term: HtmlElement, lines: Rc<Vec<String>>, n: usize) {
    let done = n >= lines.len();
    let caret = "<span class=\"caret\"></span>";
    let shown = lines[..n.min(lines.len())].join("\n");
    term.set_inner_html(&format!("{shown}{}{caret}", if done { " " } else { "\n" }));
    if done {
        return;
    }
    let delay = if lines[n].is_empty() { 60 } else { 110 };
    later(delay, move || type_line(term, lines, n + 1));
}
