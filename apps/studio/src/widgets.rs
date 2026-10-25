//! Les composants de l'interface, dans l'esprit de Lightroom : curseurs fins à repère
//! central (double‑clic : valeur par défaut), pistes colorées pour la balance des
//! blancs et le mélangeur, roues chromatiques de l'étalonnage, histogramme.

use egui::{
    Color32, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

/// Les couleurs de l'interface : gris neutres (la photo est la seule couleur à l'écran)
/// et l'accent spectral de LÜMA, réservé à ce qui est actif.
pub mod theme {
    use egui::Color32;
    pub const BACKGROUND: Color32 = Color32::from_rgb(0x11, 0x11, 0x13);
    pub const PANEL: Color32 = Color32::from_rgb(0x1d, 0x1d, 0x20);
    pub const SECTION: Color32 = Color32::from_rgb(0x26, 0x26, 0x2a);
    pub const TRACK: Color32 = Color32::from_rgb(0x52, 0x52, 0x58);
    pub const TEXT: Color32 = Color32::from_rgb(0xc9, 0xc9, 0xcf);
    pub const DIM: Color32 = Color32::from_rgb(0x82, 0x82, 0x8a);
    pub const ACCENT: Color32 = Color32::from_rgb(0xff, 0xcf, 0x4a);
    pub const SPECTRUM: [Color32; 5] = [
        Color32::from_rgb(0xff, 0x6b, 0x3d),
        Color32::from_rgb(0xff, 0xcf, 0x4a),
        Color32::from_rgb(0x4a, 0xf2, 0xc8),
        Color32::from_rgb(0x4a, 0xa8, 0xff),
        Color32::from_rgb(0x8b, 0x7b, 0xff),
    ];
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

/// Le format d'une valeur, à la française (virgule décimale, signe explicite).
pub fn format_value(v: f32, decimals: usize, signed: bool) -> String {
    let s = format!("{:.*}", decimals, v.abs()).replace('.', ",");
    if signed && v > 0.0 {
        format!("+{s}")
    } else if v < 0.0 && s.chars().any(|c| c != '0' && c != ',') {
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

/// Un curseur à la Lightroom : libellé, piste fine, valeur modifiable.
pub fn slider(ui: &mut Ui, spec: &SliderSpec, value: &mut f32) -> Change {
    let mut change = Change::default();
    let height = 22.0;
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
        spec.label,
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
    if (track.dragged() || track.clicked())
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
    if track.drag_stopped() || track.clicked() {
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
    // Poignée : un triangle, comme dans Lightroom.
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
        .accessible_name(spec.label);
    if v != *value {
        *value = v;
        change.changed = true;
    }
    if drag.drag_stopped() || drag.lost_focus() {
        change.committed = true;
    }
    change
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
    let response: Response = response.on_hover_text(format!(
        "{id} : teinte {:.0}°, saturation {:.0}",
        hue, saturation
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

    if (response.dragged() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        let d = p - center;
        *hue = (-d.y).atan2(d.x).to_degrees().rem_euclid(360.0).round();
        *saturation = (d.length() / radius * 100.0).clamp(0.0, 100.0).round();
        change.changed = true;
    }
    if response.drag_stopped() || response.clicked() {
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

/// L'histogramme RGB, avec les deux indicateurs d'écrêtage dans les coins (comme
/// Lightroom) : allumés si des pixels sont bouchés ou brûlés. Un clic les active
/// à l'écran (touche J).
pub fn histogram(ui: &mut Ui, h: &[[u32; 256]; 3], clipping: &mut bool) -> Response {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 96.0), Sense::click());
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
    let highlights = (0..3).any(|c| h[c][255] as f32 > total as f32 * 0.002);
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
        Color32::from_rgb(255, 60, 60),
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
