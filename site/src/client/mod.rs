//! Le bundle du navigateur : hydrate les îlots (labo, crates) et lance les animations
//! de la page. Les scènes 3D (three.js) vivent dans `public/js/` et reçoivent le signal
//! de départ par l'événement `luma:intro`.

pub mod lab;
pub mod motion;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    paquets::hydrate(&crate::islands::ISLANDS);
    motion::init();
}

pub fn window() -> web_sys::Window {
    web_sys::window().expect("window")
}

pub fn document() -> web_sys::Document {
    window().document().expect("document")
}

pub fn root() -> web_sys::HtmlElement {
    document()
        .document_element()
        .expect("html")
        .unchecked_into()
}

/// La langue de la page (`<html lang="…">`).
pub fn lang() -> crate::i18n::Lang {
    root()
        .get_attribute("lang")
        .and_then(|l| crate::i18n::Lang::from_code(&l))
        .unwrap_or_default()
}

pub fn now() -> f64 {
    window().performance().map_or(0.0, |p| p.now())
}

pub fn matches(query: &str) -> bool {
    window()
        .match_media(query)
        .ok()
        .flatten()
        .is_some_and(|m| m.matches())
}

pub fn reduced_motion() -> bool {
    matches("(prefers-reduced-motion: reduce)")
}

pub fn fine_pointer() -> bool {
    matches("(hover: hover) and (pointer: fine)")
}

pub fn inner_width() -> f64 {
    window()
        .inner_width()
        .ok()
        .and_then(|w| w.as_f64())
        .unwrap_or(1280.0)
}

pub fn inner_height() -> f64 {
    window()
        .inner_height()
        .ok()
        .and_then(|h| h.as_f64())
        .unwrap_or(800.0)
}

pub fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// Ajoute un écouteur pour toute la vie de la page.
pub fn on<E: JsCast + 'static>(
    target: &web_sys::EventTarget,
    event: &str,
    f: impl FnMut(E) + 'static,
) {
    let mut f = f;
    let closure =
        Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| f(e.unchecked_into()));
    let _ = target.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
    closure.forget();
}

/// Comme [`on`], avec `{ passive: true }` (défilement, pointeur).
pub fn on_passive<E: JsCast + 'static>(
    target: &web_sys::EventTarget,
    event: &str,
    f: impl FnMut(E) + 'static,
) {
    let mut f = f;
    let closure =
        Closure::<dyn FnMut(web_sys::Event)>::new(move |e: web_sys::Event| f(e.unchecked_into()));
    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(true);
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(
        event,
        closure.as_ref().unchecked_ref(),
        &options,
    );
    closure.forget();
}

type FrameCallback = Closure<dyn FnMut(f64)>;

/// Appelle `f(horodatage)` à chaque image tant qu'il renvoie `true`.
pub fn request_frame(f: impl FnMut(f64) -> bool + 'static) {
    let f = Rc::new(RefCell::new(f));
    let slot: Rc<RefCell<Option<FrameCallback>>> = Rc::new(RefCell::new(None));
    let again = slot.clone();
    *slot.borrow_mut() = Some(Closure::new(move |now: f64| {
        if (f.borrow_mut())(now) {
            if let Some(cb) = again.borrow().as_ref() {
                let _ = window().request_animation_frame(cb.as_ref().unchecked_ref());
            }
        } else {
            again.borrow_mut().take();
        }
    }));
    if let Some(cb) = slot.borrow().as_ref() {
        let _ = window().request_animation_frame(cb.as_ref().unchecked_ref());
    }
}

/// Chaque élément qui correspond à `selector`.
pub fn all(selector: &str) -> Vec<web_sys::HtmlElement> {
    let Ok(list) = document().query_selector_all(selector) else {
        return Vec::new();
    };
    (0..list.length())
        .filter_map(|i| list.item(i)?.dyn_into().ok())
        .collect()
}

/// Le premier élément qui correspond à `selector`.
pub fn one(selector: &str) -> Option<web_sys::HtmlElement> {
    document()
        .query_selector(selector)
        .ok()
        .flatten()?
        .dyn_into()
        .ok()
}

/// Appelle `f` une fois après `ms` millisecondes.
pub fn later(ms: i32, f: impl FnOnce() + 'static) {
    let f = Closure::once_into_js(f);
    let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(f.unchecked_ref(), ms);
}

/// Observe `targets` et appelle `f` à chaque changement de visibilité.
pub fn observe(
    targets: &[web_sys::Element],
    root_margin: &str,
    threshold: f64,
    f: impl Fn(&web_sys::IntersectionObserverEntry, &web_sys::IntersectionObserver) + 'static,
) {
    let callback = Closure::<dyn Fn(js_sys::Array, web_sys::IntersectionObserver)>::new(
        move |entries: js_sys::Array, io: web_sys::IntersectionObserver| {
            for entry in entries.iter() {
                if let Ok(entry) = entry.dyn_into::<web_sys::IntersectionObserverEntry>() {
                    f(&entry, &io);
                }
            }
        },
    );
    let options = web_sys::IntersectionObserverInit::new();
    options.set_root_margin(root_margin);
    options.set_threshold(&JsValue::from_f64(threshold));
    if let Ok(io) =
        web_sys::IntersectionObserver::new_with_options(callback.as_ref().unchecked_ref(), &options)
    {
        for el in targets {
            io.observe(el);
        }
    }
    callback.forget();
}

pub fn set_var(el: &web_sys::HtmlElement, name: &str, value: &str) {
    let _ = el.style().set_property(name, value);
}
