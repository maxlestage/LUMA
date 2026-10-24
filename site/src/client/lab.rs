//! Le rendu du labo, en WebGL : le shader de `lab.frag` dessine la scène et la
//! développe. À gauche de la barre, l'original (réglages neutres) ; à droite, le rendu.
//! Les réglages arrivent de l'îlot `lab` par [`set_target`] et sont rejoints en douceur.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{
    Element, HtmlCanvasElement, HtmlElement, KeyboardEvent, PointerEvent, WebGlProgram,
    WebGlRenderingContext as Gl, WebGlUniformLocation,
};

use super::*;

const VERT: &str = include_str!("lab.vert");
const FRAG: &str = include_str!("lab.frag");
const UNIFORMS: [&str; 8] = [
    "uExposure",
    "uContrast",
    "uHighlights",
    "uShadows",
    "uTemp",
    "uTint",
    "uSat",
    "uVignette",
];

thread_local! {
    static TARGET: Cell<[f64; 8]> = const { Cell::new([0.0; 8]) };
}

/// Les réglages voulus, dans l'ordre de [`crate::content::CONTROLS`].
pub fn set_target(values: [f64; 8]) {
    TARGET.with(|t| t.set(values));
}

struct Lab {
    values: [f64; 8],
    split: f64,
    split_target: f64,
    dragging: bool,
    visible: bool,
    swept: bool,
    dirty: bool,
    size: (i32, i32),
}

fn compile(gl: &Gl, kind: u32, source: &str) -> Option<web_sys::WebGlShader> {
    let shader = gl.create_shader(kind)?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        == Some(true)
    {
        Some(shader)
    } else {
        web_sys::console::warn_1(&gl.get_shader_info_log(&shader).unwrap_or_default().into());
        None
    }
}

fn program(gl: &Gl) -> Option<WebGlProgram> {
    let program = gl.create_program()?;
    gl.attach_shader(&program, &compile(gl, Gl::VERTEX_SHADER, VERT)?);
    gl.attach_shader(&program, &compile(gl, Gl::FRAGMENT_SHADER, FRAG)?);
    gl.link_program(&program);
    (gl.get_program_parameter(&program, Gl::LINK_STATUS)
        .as_bool()
        == Some(true))
    .then_some(program)
}

/// Démarre le rendu dans `canvas` (appelé par l'îlot une fois le canvas dans la page).
pub fn start(canvas: &Element) {
    let Ok(canvas) = canvas.clone().dyn_into::<HtmlCanvasElement>() else {
        return;
    };
    let Some(view) = canvas
        .parent_element()
        .and_then(|v| v.dyn_into::<HtmlElement>().ok())
    else {
        return;
    };
    let gl: Option<Gl> = canvas
        .get_context("webgl")
        .ok()
        .flatten()
        .and_then(|c| c.dyn_into().ok());
    let Some(gl) = gl else {
        let _ = view.class_list().add_1("no-webgl");
        return;
    };
    let Some(prog) = program(&gl) else {
        let _ = view.class_list().add_1("no-webgl");
        return;
    };
    gl.use_program(Some(&prog));
    let buffer = gl.create_buffer();
    gl.bind_buffer(Gl::ARRAY_BUFFER, buffer.as_ref());
    let triangle = js_sys::Float32Array::from(&[-1.0f32, -1.0, 3.0, -1.0, -1.0, 3.0][..]);
    gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &triangle, Gl::STATIC_DRAW);
    let attribute = gl.get_attrib_location(&prog, "p") as u32;
    gl.enable_vertex_attrib_array(attribute);
    gl.vertex_attrib_pointer_with_i32(attribute, 2, Gl::FLOAT, false, 0, 0);
    let uniform = |name: &str| gl.get_uniform_location(&prog, name);
    let (u_res, u_time, u_split, u_aspect) = (
        uniform("uRes"),
        uniform("uTime"),
        uniform("uSplit"),
        uniform("uAspect"),
    );
    let u_values: Vec<Option<WebGlUniformLocation>> = UNIFORMS.iter().map(|n| uniform(n)).collect();

    let reduced = reduced_motion();
    let state = Rc::new(RefCell::new(Lab {
        values: [0.0; 8],
        split: 0.5,
        split_target: 0.5,
        dragging: false,
        visible: false,
        swept: false,
        dirty: true,
        size: (0, 0),
    }));

    // Taille : pixels réels de l'écran, plafonnés à 2× (Retina) pour tenir la cadence.
    let resize = {
        let (canvas, gl, state) = (canvas.clone(), gl.clone(), state.clone());
        move || {
            let dpr = window().device_pixel_ratio().min(2.0);
            let w = (f64::from(canvas.client_width()) * dpr).round() as i32;
            let h = (f64::from(canvas.client_height()) * dpr).round() as i32;
            canvas.set_width(w.max(1) as u32);
            canvas.set_height(h.max(1) as u32);
            gl.viewport(0, 0, w, h);
            let mut s = state.borrow_mut();
            s.size = (w.max(1), h.max(1));
            s.dirty = true;
        }
    };
    resize();
    {
        let resize = resize.clone();
        on_passive(&window(), "resize", move |_: web_sys::Event| resize());
    }

    // Barre avant/après : glisser, cliquer dans l'image, flèches du clavier.
    let handle: Option<HtmlElement> = view
        .query_selector(".lab-split")
        .ok()
        .flatten()
        .and_then(|h| h.dyn_into().ok());
    let set_split = {
        let (state, handle) = (state.clone(), handle.clone());
        move |v: f64| {
            let v = v.clamp(0.02, 0.98);
            let mut s = state.borrow_mut();
            s.split_target = v;
            s.dirty = true;
            if let Some(h) = &handle {
                let _ = h.set_attribute("aria-valuenow", &format!("{}", (v * 100.0).round()));
            }
        }
    };
    let from_pointer = {
        let (view, set_split) = (view.clone(), set_split.clone());
        move |x: i32| {
            let r = view.get_bounding_client_rect();
            set_split((f64::from(x) - r.left()) / r.width());
        }
    };
    if let Some(handle) = &handle {
        let (st, f, h) = (state.clone(), from_pointer.clone(), handle.clone());
        on(handle, "pointerdown", move |e: PointerEvent| {
            st.borrow_mut().dragging = true;
            let _ = h.set_pointer_capture(e.pointer_id());
            f(e.client_x());
        });
        let (st, f) = (state.clone(), from_pointer.clone());
        on(handle, "pointermove", move |e: PointerEvent| {
            if st.borrow().dragging {
                f(e.client_x());
            }
        });
        for event in ["pointerup", "pointercancel"] {
            let st = state.clone();
            on(handle, event, move |_: web_sys::Event| {
                st.borrow_mut().dragging = false
            });
        }
        let (st, set) = (state.clone(), set_split.clone());
        on(handle, "keydown", move |e: KeyboardEvent| {
            let current = st.borrow().split_target;
            match e.key().as_str() {
                "ArrowLeft" => set(current - 0.05),
                "ArrowRight" => set(current + 0.05),
                _ => return,
            }
            e.prevent_default();
        });
    }
    {
        let (f, canvas_el) = (from_pointer.clone(), canvas.clone());
        on(&view, "click", move |e: web_sys::MouseEvent| {
            let on_canvas = e
                .target()
                .and_then(|t| t.dyn_into::<Element>().ok())
                .is_some_and(|t| t == **canvas_el);
            if on_canvas {
                f(e.client_x());
            }
        });
    }

    // Visible : on dessine. Première apparition : la barre balaie l'image.
    {
        let (st, set) = (state.clone(), set_split.clone());
        observe(
            &[Element::from(view.clone())],
            "0px",
            0.25,
            move |entry, _| {
                let visible = entry.is_intersecting();
                let first = {
                    let mut s = st.borrow_mut();
                    s.visible = visible;
                    s.dirty = true;
                    let first = visible && !s.swept;
                    if first {
                        s.swept = true;
                        s.split = 0.96;
                    }
                    first
                };
                if first {
                    set(0.5);
                }
            },
        );
    }

    let fps = view.query_selector(".lab-fps").ok().flatten();
    let started = now();
    let (mut frames, mut last_fps) = (0u32, started);
    request_frame(move |time| {
        let mut s = state.borrow_mut();
        if !s.visible {
            return true;
        }
        let target = TARGET.with(Cell::get);
        let ease = if reduced { 1.0 } else { 0.14 };
        let mut moving = false;
        for (v, t) in s.values.iter_mut().zip(target) {
            let d = t - *v;
            if d.abs() > 1e-4 {
                *v += d * ease;
                moving = true;
            } else {
                *v = t;
            }
        }
        let d = s.split_target - s.split;
        if d.abs() > 1e-4 {
            s.split += d * if s.dragging || reduced { 1.0 } else { 0.08 };
            moving = true;
        } else {
            s.split = s.split_target;
        }
        set_var(&view, "--split", &format!("{:.3}%", s.split * 100.0));
        // L'eau ondule : on redessine en continu, sauf si les animations sont réduites.
        if reduced && !moving && !s.dirty {
            return true;
        }
        s.dirty = false;
        let (w, h) = s.size;
        gl.uniform2f(u_res.as_ref(), w as f32, h as f32);
        gl.uniform1f(
            u_time.as_ref(),
            if reduced {
                0.0
            } else {
                ((time - started) / 1000.0) as f32
            },
        );
        gl.uniform1f(u_split.as_ref(), s.split as f32);
        gl.uniform1f(u_aspect.as_ref(), w as f32 / h as f32);
        for (location, v) in u_values.iter().zip(s.values) {
            gl.uniform1f(location.as_ref(), v as f32);
        }
        gl.draw_arrays(Gl::TRIANGLES, 0, 3);

        frames += 1;
        if time - last_fps > 500.0 {
            if let Some(fps) = &fps {
                let rate = (f64::from(frames) * 1000.0 / (time - last_fps)).round();
                fps.set_text_content(Some(&format!("{rate} i/s · GPU · Rust")));
            }
            frames = 0;
            last_fps = time;
        }
        true
    });
}
