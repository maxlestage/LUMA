//! Les composants de l'interface, sobres et précis : curseurs fins à repère
//! central (double‑clic : valeur par défaut), pistes colorées pour la balance des
//! blancs et le mélangeur, roues chromatiques de l'étalonnage, histogramme et instruments.

use egui::{
    Color32, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

/// Les couleurs de LÜMA : une encre profonde sous la photo, des îlots de verre fumé
/// qui flottent dessus, et l'anneau spectral de l'icône réservé à ce qui est actif.
pub mod theme {
    use egui::Color32;
    /// Le fond, sous la photo.
    pub const BACKGROUND: Color32 = Color32::from_rgb(0x06, 0x06, 0x09);
    /// Les îlots (verre fumé, presque opaque).
    // Du verre fumé : les lueurs du fond passent à travers les îlots.
    pub const PANEL: Color32 = Color32::from_rgba_premultiplied(0x10, 0x10, 0x16, 0xd6);
    pub const GLASS_EDGE: Color32 = Color32::from_rgba_premultiplied(0x2a, 0x2b, 0x36, 0x60);
    pub const SECTION: Color32 = Color32::from_rgb(0x20, 0x22, 0x2c);
    pub const TRACK: Color32 = Color32::from_rgb(0x3b, 0x3d, 0x4a);
    pub const TEXT: Color32 = Color32::from_rgb(0xec, 0xe8, 0xe1);
    pub const DIM: Color32 = Color32::from_rgb(0x8d, 0x8c, 0x98);
    /// L'ambre de LÜMA : poignées, valeurs actives.
    pub const ACCENT: Color32 = Color32::from_rgb(0xff, 0xb5, 0x47);
    pub const SPECTRUM: [Color32; 5] = [
        Color32::from_rgb(0xff, 0x6b, 0x3d),
        Color32::from_rgb(0xff, 0xcf, 0x4a),
        Color32::from_rgb(0x4a, 0xf2, 0xc8),
        Color32::from_rgb(0x4a, 0xa8, 0xff),
        Color32::from_rgb(0x8b, 0x7b, 0xff),
    ];
}

/// L'anneau spectral de LÜMA (la teinte fait le tour), de rayon `radius`.
pub fn spectral_ring(painter: &egui::Painter, center: Pos2, radius: f32, width: f32, alpha: f32) {
    let n = 72;
    let mut mesh = Mesh::default();
    for k in 0..=n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        let color = hue_color(a.to_degrees() + 200.0, 0.62, 1.0).gamma_multiply(alpha);
        let d = vec2(a.cos(), a.sin());
        mesh.colored_vertex(center + d * (radius - width / 2.0), color);
        mesh.colored_vertex(center + d * (radius + width / 2.0), color);
        if k > 0 {
            let i = (k * 2) as u32;
            mesh.add_triangle(i - 2, i - 1, i);
            mesh.add_triangle(i - 1, i, i + 1);
        }
    }
    painter.add(Shape::mesh(mesh));
}

/// Une couleur de la roue chromatique (teinte en degrés, TSV pleine saturation).
pub fn hue_color(degrees: f32, saturation: f32, value: f32) -> Color32 {
    let h = degrees.rem_euclid(360.0) / 60.0;
    let c = value * saturation;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = value - c;
    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Ce qui est dessiné sous un curseur.
#[derive(Clone, Copy)]
pub enum Track {
    Plain,
    /// Dégradé de gauche à droite (température, teinte, couleurs du mélangeur).
    Gradient(&'static [Color32]),
}

/// Du noir au blanc, pour la plage de luminance d'un masque.
pub const GREYS: [Color32; 2] = [
    Color32::from_rgb(20, 20, 22),
    Color32::from_rgb(240, 240, 240),
];

/// Le tour des teintes (OKLab, de 0 à 360°), pour la plage de couleur d'un masque.
pub const HUE: [Color32; 13] = [
    Color32::from_rgb(230, 128, 161),
    Color32::from_rgb(235, 131, 115),
    Color32::from_rgb(223, 143, 72),
    Color32::from_rgb(196, 160, 50),
    Color32::from_rgb(154, 176, 75),
    Color32::from_rgb(98, 187, 120),
    Color32::from_rgb(6, 191, 168),
    Color32::from_rgb(0, 186, 209),
    Color32::from_rgb(75, 174, 237),
    Color32::from_rgb(133, 159, 246),
    Color32::from_rgb(177, 145, 234),
    Color32::from_rgb(210, 133, 203),
    Color32::from_rgb(230, 128, 161),
];

pub const TEMPERATURE: [Color32; 2] = [
    Color32::from_rgb(0x4a, 0x8c, 0xff),
    Color32::from_rgb(0xff, 0xc0, 0x3c),
];
pub const TINT: [Color32; 2] = [
    Color32::from_rgb(0x3c, 0xc8, 0x50),
    Color32::from_rgb(0xe0, 0x40, 0xfb),
];

/// Le résultat d'un curseur : `changed` à chaque mouvement, `committed` quand le geste
/// est fini (relâchement, saisie, double‑clic) : c'est là qu'une étape d'historique naît.
#[derive(Default, Clone, Copy)]
pub struct Change {
    pub changed: bool,
    pub committed: bool,
}

impl Change {
    pub fn merge(&mut self, other: Change) {
        self.changed |= other.changed;
        self.committed |= other.committed;
    }
}

/// Le format d'une valeur : virgule décimale (en français et en espagnol, point en
/// anglais), signe explicite.
pub fn format_value(v: f32, decimals: usize, signed: bool) -> String {
    let s = format!("{:.*}", decimals, v.abs());
    let s = if crate::i18n::decimal_comma() {
        s.replace('.', ",")
    } else {
        s
    };
    if signed && v > 0.0 {
        format!("+{s}")
    } else if v < 0.0 && s.chars().any(|c| c != '0' && c != ',' && c != '.') {
        format!("−{s}")
    } else {
        s
    }
}

pub struct SliderSpec {
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub decimals: usize,
    pub track: Track,
    /// Échelle logarithmique (la température, 2000 à 50 000 K).
    pub log: bool,
}

/// Un curseur de réglage : libellé, piste fine, valeur modifiable.
pub fn slider(ui: &mut Ui, spec: &SliderSpec, value: &mut f32) -> Change {
    let mut change = Change::default();
    // Au doigt, des rangées plus hautes : plus faciles à viser et à faire défiler.
    let height = if crate::touch() { 34.0 } else { 22.0 };
    let label_w = 112.0;
    let value_w = 52.0;
    let width = ui.available_width();
    let (row, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let label_rect = Rect::from_min_size(row.min, vec2(label_w, height));
    let track_rect = Rect::from_min_max(
        pos2(row.min.x + label_w + 6.0, row.min.y),
        pos2(row.max.x - value_w - 8.0, row.max.y),
    );
    let value_rect = Rect::from_min_max(
        pos2(row.max.x - value_w, row.min.y + 2.0),
        pos2(row.max.x, row.max.y - 2.0),
    );

    // Libellé : double‑clic pour revenir à la valeur par défaut.
    let label = ui.interact(
        label_rect,
        ui.scope_id().with((spec.label, "libellé")),
        Sense::click(),
    );
    let text_color = if label.hovered() {
        Color32::WHITE
    } else {
        theme::TEXT
    };
    ui.painter().text(
        pos2(label_rect.max.x, label_rect.center().y),
        egui::Align2::RIGHT_CENTER,
        crate::i18n::tr(spec.label),
        egui::FontId::proportional(12.5),
        text_color,
    );

    let to_t = |v: f32| -> f32 {
        if spec.log {
            (v.ln() - spec.min.ln()) / (spec.max.ln() - spec.min.ln())
        } else {
            (v - spec.min) / (spec.max - spec.min)
        }
    };
    let from_t = |t: f32| -> f32 {
        let t = t.clamp(0.0, 1.0);
        if spec.log {
            (spec.min.ln() + t * (spec.max.ln() - spec.min.ln())).exp()
        } else {
            spec.min + t * (spec.max - spec.min)
        }
    };

    let track = ui.interact(
        track_rect,
        ui.scope_id().with((spec.label, "piste")),
        Sense::click_and_drag(),
    );
    if crate::touch() {
        touch_track(
            ui,
            &track,
            track_rect,
            spec,
            value,
            &mut change,
            &to_t,
            &from_t,
        );
    } else if (track.dragged() || track.clicked())
        && let Some(p) = track.interact_pointer_pos()
    {
        // Majuscule : réglage fin, dix fois plus précis.
        let fine = ui.input(|i| i.modifiers.shift);
        let new = if fine && track.dragged() {
            let delta = track.drag_delta().x / track_rect.width() * 0.1;
            from_t(to_t(*value) + delta)
        } else {
            from_t((p.x - track_rect.min.x) / track_rect.width())
        };
        let step = 10f32.powi(-(spec.decimals as i32));
        let new = (new / step).round() * step;
        if new != *value {
            *value = new;
            change.changed = true;
        }
    }
    if !crate::touch() && (track.drag_stopped() || track.clicked()) {
        change.committed = true;
    }
    if label.double_clicked() || track.double_clicked() {
        *value = spec.default;
        change.changed = true;
        change.committed = true;
    }

    // Piste.
    let painter = ui.painter();
    let y = track_rect.center().y;
    let (x0, x1) = (track_rect.min.x + 4.0, track_rect.max.x - 4.0);
    match spec.track {
        Track::Plain => {
            painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.5, theme::TRACK));
        }
        Track::Gradient(colors) => {
            let mut mesh = Mesh::default();
            let n = colors.len().max(2) - 1;
            for (i, c) in colors.iter().enumerate() {
                let x = x0 + (x1 - x0) * i as f32 / n as f32;
                mesh.colored_vertex(pos2(x, y - 2.0), *c);
                mesh.colored_vertex(pos2(x, y + 2.0), *c);
                if i > 0 {
                    let k = (i * 2) as u32;
                    mesh.add_triangle(k - 2, k - 1, k);
                    mesh.add_triangle(k - 1, k, k + 1);
                }
            }
            painter.add(Shape::mesh(mesh));
        }
    }
    // Repère de la valeur par défaut (le centre des curseurs bipolaires).
    let tick = x0 + (x1 - x0) * to_t(spec.default).clamp(0.0, 1.0);
    painter.line_segment(
        [pos2(tick, y - 4.0), pos2(tick, y + 4.0)],
        Stroke::new(1.0, theme::DIM),
    );
    // Poignée : un triangle.
    let x = x0 + (x1 - x0) * to_t(*value).clamp(0.0, 1.0);
    let active = track.hovered() || track.dragged();
    let knob = if active {
        Color32::WHITE
    } else {
        Color32::from_gray(0xd8)
    };
    painter.add(Shape::convex_polygon(
        vec![
            pos2(x, y - 1.0),
            pos2(x + 5.5, y + 7.0),
            pos2(x - 5.5, y + 7.0),
        ],
        knob,
        Stroke::NONE,
    ));

    // Au doigt, la valeur est seulement affichée : un glissé dessus fait défiler le
    // panneau au lieu de la changer.
    if crate::touch() {
        ui.painter().rect_filled(value_rect, 6.0, theme::SECTION);
        ui.painter().text(
            value_rect.center(),
            egui::Align2::CENTER_CENTER,
            format_value(*value, spec.decimals, spec.min < 0.0),
            egui::FontId::proportional(13.0),
            theme::TEXT,
        );
        return change;
    }
    // Valeur : on peut la faire glisser ou la taper.
    let mut v = *value;
    let speed = (spec.max - spec.min) / 400.0;
    let drag = ui
        .put(
            value_rect,
            egui::DragValue::new(&mut v)
                .range(spec.min..=spec.max)
                .speed(speed)
                .fixed_decimals(spec.decimals)
                .custom_formatter(|n, _| format_value(n as f32, spec.decimals, spec.min < 0.0))
                .custom_parser(|s| {
                    s.replace(',', ".")
                        .replace('−', "-")
                        .trim_start_matches('+')
                        .parse()
                        .ok()
                }),
        )
        .accessible_name(crate::i18n::tr(spec.label));
    if v != *value {
        *value = v;
        change.changed = true;
    }
    if drag.drag_stopped() || drag.lost_focus() {
        change.committed = true;
    }
    change
}

/// Un curseur au doigt : le premier mouvement décide. Horizontal, il règle la valeur
/// en relatif (le doigt cache la poignée : pas de saut à l'endroit touché) ; vertical,
/// il fait défiler le panneau, comme partout sur iPhone. Un simple toucher ne change
/// rien (le double toucher remet la valeur par défaut, plus haut).
#[allow(clippy::too_many_arguments)]
fn touch_track(
    ui: &Ui,
    track: &egui::Response,
    track_rect: Rect,
    spec: &SliderSpec,
    value: &mut f32,
    change: &mut Change,
    to_t: &dyn Fn(f32) -> f32,
    from_t: &dyn Fn(f32) -> f32,
) {
    let axis_id = track.id.with("axe");
    if track.drag_started() {
        ui.data_mut(|d| d.remove::<bool>(axis_id));
    }
    if track.dragged() {
        let total = ui
            .input(|i| {
                i.pointer
                    .press_origin()
                    .zip(i.pointer.latest_pos())
                    .map(|(a, b)| b - a)
            })
            .unwrap_or_default();
        let decided = ui.data(|d| d.get_temp::<bool>(axis_id));
        // Ce qui s'applique cette image : tout le chemin fait au moment de décider,
        // puis le mouvement de l'image.
        let (horizontal, delta) = match decided {
            Some(h) => (Some(h), track.drag_delta()),
            None if total.length() > 8.0 => {
                let h = total.x.abs() > total.y.abs();
                ui.data_mut(|d| d.insert_temp(axis_id, h));
                (Some(h), total)
            }
            None => (None, Vec2::ZERO),
        };
        match horizontal {
            Some(true) => {
                let new = from_t(to_t(*value) + delta.x / track_rect.width());
                let step = 10f32.powi(-(spec.decimals as i32));
                let new = (new / step).round() * step;
                if new != *value {
                    *value = new;
                    change.changed = true;
                }
            }
            Some(false) => ui.scroll_with_delta(vec2(0.0, delta.y)),
            None => {}
        }
    }
    if track.drag_stopped() && ui.data(|d| d.get_temp::<bool>(axis_id)) == Some(true) {
        change.committed = true;
    }
}

/// Une roue chromatique : la direction donne la teinte, la distance au centre la
/// saturation (0 à 100).
pub fn color_wheel(
    ui: &mut Ui,
    id: &str,
    hue: &mut f32,
    saturation: &mut f32,
    diameter: f32,
) -> Change {
    let mut change = Change::default();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(diameter), Sense::click_and_drag());
    let response: Response = response.on_hover_text(crate::i18n::trf(
        "{zone} : teinte {hue}°, saturation {saturation}",
        &[
            ("zone", &id),
            ("hue", &format!("{hue:.0}")),
            ("saturation", &format!("{saturation:.0}")),
        ],
    ));
    let center = rect.center();
    let radius = diameter / 2.0 - 3.0;
    let mut mesh = Mesh::default();
    let steps = 72u32;
    mesh.colored_vertex(center, Color32::from_gray(0x8a));
    for i in 0..=steps {
        let deg = i as f32 / steps as f32 * 360.0;
        let a = deg.to_radians();
        mesh.colored_vertex(
            center + vec2(a.cos(), -a.sin()) * radius,
            hue_color(deg, 0.85, 0.9),
        );
        if i > 0 {
            mesh.add_triangle(0, i, i + 1);
        }
    }
    let painter = ui.painter();
    painter.add(Shape::mesh(mesh));
    painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::from_gray(0x10)));

    // Au doigt, la roue ne se règle qu'en partant de sa poignée (ou d'un toucher
    // franc dessus) : un glissé commencé ailleurs fait défiler le panneau.
    let a = hue.to_radians();
    let knob = center + vec2(a.cos(), -a.sin()) * radius * (*saturation / 100.0);
    let grabbed = !crate::touch()
        || ui
            .input(|i| i.pointer.press_origin())
            .is_some_and(|o| o.distance(knob) < 28.0);
    if crate::touch() && response.dragged() && !grabbed {
        ui.scroll_with_delta(vec2(0.0, response.drag_delta().y));
    } else if (response.dragged() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        let d = p - center;
        *hue = (-d.y).atan2(d.x).to_degrees().rem_euclid(360.0).round();
        *saturation = (d.length() / radius * 100.0).clamp(0.0, 100.0).round();
        change.changed = true;
    }
    if (response.drag_stopped() && grabbed) || response.clicked() {
        change.committed = true;
    }
    if response.double_clicked() {
        *saturation = 0.0;
        change.changed = true;
        change.committed = true;
    }
    let a = hue.to_radians();
    let knob = center + vec2(a.cos(), -a.sin()) * radius * (*saturation / 100.0);
    painter.circle(
        knob,
        5.0,
        hue_color(*hue, *saturation / 100.0, 1.0),
        Stroke::new(2.0, Color32::WHITE),
    );
    change
}

/// L'histogramme RGB, avec les deux indicateurs d'écrêtage dans les coins :
/// allumés si des pixels sont bouchés ou brûlés. Un clic les active
/// à l'écran (touche J).
pub fn histogram(ui: &mut Ui, h: &[[u32; 256]; 3], clipping: &mut bool) -> Response {
    histogram_sized(ui, h, clipping, 96.0)
}

/// [`histogram`], à la hauteur `height`.
pub fn histogram_sized(
    ui: &mut Ui,
    h: &[[u32; 256]; 3],
    clipping: &mut bool,
    height: f32,
) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, Color32::from_gray(0x16));
    let total: u32 = h[1].iter().sum();
    if total > 0 {
        // Échelle : on ignore les pics des extrémités pour ne pas tout écraser.
        let peak = (0..3)
            .flat_map(|c| h[c][2..254].iter().copied())
            .max()
            .unwrap_or(1)
            .max(1) as f32;
        let colors = [
            Color32::from_rgba_unmultiplied(240, 70, 70, 110),
            Color32::from_rgba_unmultiplied(70, 220, 110, 110),
            Color32::from_rgba_unmultiplied(80, 140, 255, 110),
        ];
        for (c, color) in colors.iter().enumerate() {
            let mut mesh = Mesh::default();
            for (i, &count) in h[c].iter().enumerate() {
                let x = rect.min.x + rect.width() * i as f32 / 255.0;
                let v = (count as f32 / peak).sqrt().min(1.0);
                let top = rect.max.y - 4.0 - v * (rect.height() - 10.0);
                mesh.colored_vertex(pos2(x, rect.max.y - 2.0), *color);
                mesh.colored_vertex(pos2(x, top), *color);
                if i > 0 {
                    let k = (i * 2) as u32;
                    mesh.add_triangle(k - 2, k - 1, k);
                    mesh.add_triangle(k - 1, k, k + 1);
                }
            }
            painter.add(Shape::mesh(mesh));
        }
    }
    let shadows = (0..3).any(|c| h[c][0] as f32 > total as f32 * 0.002);
    // Les couches brûlées, chacune de sa couleur (rouge et vert : jaune ; les trois :
    // blanc), comme l'indicateur sur la photo.
    let burnt = [0, 1, 2].map(|c| h[c][255] as f32 > total as f32 * 0.002);
    let highlights = burnt.contains(&true);
    let burnt_color = if highlights {
        let [r, g, b] = burnt.map(|on| if on { 255 } else { 40 });
        Color32::from_rgb(r, g, b)
    } else {
        Color32::from_rgb(255, 60, 60)
    };
    let corner = |p: Pos2, lit: bool, color: Color32| {
        let fill = if lit || *clipping {
            color
        } else {
            Color32::from_gray(0x50)
        };
        painter.add(Shape::convex_polygon(
            vec![
                p,
                p + vec2(if p.x < rect.center().x { 10.0 } else { -10.0 }, 0.0),
                p + vec2(0.0, 10.0),
            ],
            fill,
            Stroke::NONE,
        ));
    };
    corner(
        rect.min + vec2(4.0, 4.0),
        shadows,
        Color32::from_rgb(60, 110, 255),
    );
    corner(
        pos2(rect.max.x - 4.0, rect.min.y + 4.0),
        highlights,
        burnt_color,
    );
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, Color32::from_gray(0x30)),
        StrokeKind::Inside,
    );
    if response.clicked() {
        *clipping = !*clipping;
    }
    response
}

/// Un instrument (forme d'onde, parade RVB, vectorscope) rendu par le moteur, avec ses
/// repères : les niveaux 0, 25, 50, 75 et 100 % ; pour le vectorscope, les cibles des
/// six couleurs primaires et secondaires à 75 % et la ligne des tons chair.
pub fn scope(
    ui: &mut Ui,
    scope: luma_image::scopes::Scope,
    texture: &egui::TextureHandle,
    height: f32,
) -> Response {
    use luma_image::scopes::{
        LEVELS, PARADE_COLUMNS, PARADE_GAP, Scope, vectorscope_point, waveform_level,
    };
    let width = ui.available_width();
    let vector = scope == Scope::Vectorscope;
    let h = if vector {
        width.min(height * 1.7)
    } else {
        height
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, Color32::from_gray(0x16));
    let grid = Stroke::new(1.0, Color32::from_gray(0x34));
    let image = if vector {
        Rect::from_center_size(rect.center(), Vec2::splat(h - 8.0))
    } else {
        rect.shrink2(vec2(6.0, 5.0))
    };
    if vector {
        let c = image.center();
        let r = image.width() / 2.0;
        painter.circle_stroke(c, r, grid);
        painter.circle_stroke(c, r * 0.5, Stroke::new(1.0, Color32::from_gray(0x26)));
        painter.line_segment([c - vec2(r, 0.0), c + vec2(r, 0.0)], grid);
        painter.line_segment([c - vec2(0.0, r), c + vec2(0.0, r)], grid);
        let at = |rgb: [f32; 3]| {
            let [u, v] = vectorscope_point(rgb);
            pos2(
                image.min.x + u * image.width(),
                image.min.y + v * image.height(),
            )
        };
        // Les tons chair : toutes les peaux tombent près de cette ligne.
        let skin = at([0.87, 0.67, 0.55]) - c;
        painter.line_segment(
            [c, c + skin.normalized() * r],
            Stroke::new(1.0, Color32::from_rgb(0x7a, 0x5a, 0x44)),
        );
        let targets = [
            (crate::i18n::tr("R|vectorscope"), [0.75, 0.0, 0.0]),
            (crate::i18n::tr("J|vectorscope"), [0.75, 0.75, 0.0]),
            (crate::i18n::tr("V|vectorscope"), [0.0, 0.75, 0.0]),
            (crate::i18n::tr("C|vectorscope"), [0.0, 0.75, 0.75]),
            (crate::i18n::tr("B|vectorscope"), [0.0, 0.0, 0.75]),
            (crate::i18n::tr("M|vectorscope"), [0.75, 0.0, 0.75]),
        ];
        for (name, rgb) in targets {
            let p = at(rgb);
            painter.rect_stroke(
                Rect::from_center_size(p, Vec2::splat(7.0)),
                1.0,
                grid,
                StrokeKind::Middle,
            );
            let out = (p - c).normalized() * 10.0;
            painter.text(
                p + out,
                egui::Align2::CENTER_CENTER,
                name,
                egui::FontId::proportional(9.0),
                Color32::from_gray(0x70),
            );
        }
    } else {
        for level in [0.0, 0.25, 0.5, 0.75, 1.0] {
            // Le milieu de la ligne de l'image qui porte ce niveau.
            let row = waveform_level(level) * (LEVELS - 1) as f32 + 0.5;
            let y = image.min.y + row / LEVELS as f32 * image.height();
            painter.line_segment([pos2(image.min.x, y), pos2(image.max.x, y)], grid);
        }
        if scope == Scope::Parade {
            let total = (PARADE_COLUMNS * 3 + PARADE_GAP * 2) as f32;
            for k in 1..3 {
                let x = image.min.x
                    + (k * (PARADE_COLUMNS + PARADE_GAP)) as f32 / total * image.width()
                    - PARADE_GAP as f32 / 2.0 / total * image.width();
                painter.line_segment([pos2(x, image.min.y), pos2(x, image.max.y)], grid);
            }
        }
    }
    painter.image(
        texture.id(),
        image,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0, Color32::from_gray(0x30)),
        StrokeKind::Inside,
    );
    response
}

/// Une étoile pleine (dessinée : la police n'a pas le glyphe).
pub fn paint_star(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    for k in 0..=10 {
        let r = if k % 2 == 0 { radius } else { radius * 0.45 };
        let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
        mesh.colored_vertex(center + vec2(a.cos(), a.sin()) * r, color);
        if k > 0 {
            mesh.add_triangle(0, k as u32, k as u32 + 1);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// La note en étoiles ; cliquer une étoile donne la note, recliquer la note la retire.
pub fn rating(ui: &mut Ui, value: u8, size: f32) -> Option<u8> {
    let (rect, response) = ui.allocate_exact_size(vec2(size * 5.4, size), Sense::click());
    let response = response.on_hover_text(crate::i18n::tr("Note (0 à 5)"));
    let hover = response
        .hover_pos()
        .map(|p| (((p.x - rect.min.x) / (size * 1.08)).floor() as u8 + 1).min(5));
    let shown = hover.unwrap_or(value);
    for k in 0..5u8 {
        let c = rect.min + vec2(size * 0.54 + k as f32 * size * 1.08, size * 0.5);
        let color = if k < shown {
            Color32::from_gray(0xe8)
        } else {
            Color32::from_gray(0x55)
        };
        paint_star(ui.painter(), c, size * 0.48, color);
    }
    let clicked = response.clicked();
    response
        .widget_info(|| egui::WidgetInfo::slider(true, f64::from(value), crate::i18n::tr("Note")));

    match hover {
        Some(n) if clicked => Some(if n == value { 0 } else { n }),
        _ => None,
    }
}

/// Le bouton d'une action principale (acheter, s'abonner) : toute la largeur, plein,
/// couleur d'accent, son texte au centre. `true` quand il est touché.
pub fn call_to_action(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let h = if crate::touch() { 48.0 } else { 38.0 };
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), sense);
    let resp = resp.on_hover_cursor(if enabled {
        egui::CursorIcon::PointingHand
    } else {
        egui::CursorIcon::Default
    });
    let fill = if !enabled {
        egui::Color32::from_rgb(0x2c, 0x2e, 0x3a)
    } else if resp.is_pointer_button_down_on() {
        theme::ACCENT.gamma_multiply(0.8)
    } else if resp.hovered() {
        theme::ACCENT.gamma_multiply(1.08)
    } else {
        theme::ACCENT
    };
    let p = ui.painter();
    p.rect_filled(rect, 12.0, fill);
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(16.0),
        if enabled {
            egui::Color32::from_rgb(0x1a, 0x12, 0x06)
        } else {
            theme::DIM
        },
    );
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    enabled && resp.clicked()
}

/// Une petite étiquette arrondie, couleur d'accent (« Une fois pour toutes »).
pub fn pill(ui: &mut egui::Ui, text: &str) {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::FontId::proportional(11.0),
        theme::ACCENT,
    );
    let (rect, _) =
        ui.allocate_exact_size(galley.size() + egui::vec2(16.0, 8.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(
        rect,
        rect.height() / 2.0,
        theme::ACCENT.gamma_multiply(0.16),
    );
    p.galley(rect.center() - galley.size() / 2.0, galley, theme::ACCENT);
}

/// Des onglets côte à côte, de même largeur, qui tiennent toujours dans la largeur
/// disponible (le texte rapetisse s'il le faut). `on` dit lesquels sont allumés ;
/// `tips` donne la bulle de chaque onglet (vide : aucune). Rend celui qui est touché.
pub fn segments(
    ui: &mut egui::Ui,
    labels: &[&str],
    on: &[bool],
    tips: &[&str],
    size: f32,
) -> Option<usize> {
    let n = labels.len().max(1) as f32;
    let h = if crate::touch() { 36.0 } else { 24.0 };
    let (row, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
    let w = row.width() / n;
    let mut hit = None;
    for (k, label) in labels.iter().enumerate() {
        let cell =
            egui::Rect::from_min_size(row.min + egui::vec2(w * k as f32, 0.0), egui::vec2(w, h));
        let id = ui.scope_id().with(("segment", k, *label));
        let mut resp = ui.interact(cell, id, egui::Sense::click());
        if let Some(tip) = tips.get(k).filter(|t| !t.is_empty()) {
            resp = resp.on_hover_text(*tip);
        }
        let lit = on.get(k).copied().unwrap_or(false);
        let p = ui.painter();
        if lit {
            p.rect_filled(
                cell.shrink(1.0),
                8.0,
                egui::Color32::from_rgb(0x3a, 0x2f, 0x1e),
            );
        } else if resp.hovered() {
            p.rect_filled(cell.shrink(1.0), 8.0, theme::SECTION);
        }
        let color = if lit { theme::TEXT } else { theme::DIM };
        let mut font = egui::FontId::proportional(size);
        let mut galley = p.layout_no_wrap((*label).to_owned(), font.clone(), color);
        if galley.size().x > w - 8.0 {
            font.size = (size * (w - 8.0) / galley.size().x).max(7.0);
            galley = p.layout_no_wrap((*label).to_owned(), font, color);
        }
        p.galley(cell.center() - galley.size() / 2.0, galley, color);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, lit, *label));
        if resp.clicked() {
            hit = Some(k);
        }
    }
    hit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_written_the_french_way() {
        assert_eq!(format_value(0.5, 2, true), "+0,50");
        assert_eq!(format_value(-12.0, 0, true), "−12");
        assert_eq!(format_value(0.0, 0, true), "0");
        assert_eq!(format_value(-0.001, 2, true), "0,00");
        assert_eq!(format_value(5500.0, 0, false), "5500");
    }

    #[test]
    fn hue_colors() {
        assert_eq!(hue_color(0.0, 1.0, 1.0), Color32::from_rgb(255, 0, 0));
        assert_eq!(hue_color(120.0, 1.0, 1.0), Color32::from_rgb(0, 255, 0));
        assert_eq!(hue_color(240.0, 0.0, 0.5), Color32::from_rgb(127, 127, 127));
    }
}
