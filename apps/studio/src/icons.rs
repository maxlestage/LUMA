//! Les icônes de LÜMA, dessinées au trait (la police n'a pas de pictogrammes, et des
//! icônes vectorielles restent nettes à toutes les tailles d'écran).

use egui::{Color32, Painter, Pos2, Rect, Stroke, pos2, vec2};

/// Les icônes de l'interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Library,
    Light,
    Color,
    Detail,
    Masks,
    Films,
    Geometry,
    Merge,
    Export,
    Open,
    /// Masque radial : le cercle.
    Radial,
    /// Masque linéaire : les trois traits (plein effet, milieu, plus d'effet).
    Linear,
    Brush,
    Stack,
    Panorama,
    Star,
}

fn p(r: Rect, x: f32, y: f32) -> Pos2 {
    pos2(r.min.x + x * r.width(), r.min.y + y * r.height())
}

/// Dessine `icon` dans `rect` (carré conseillé), au trait de couleur `color`.
pub fn paint(painter: &Painter, icon: Icon, rect: Rect, color: Color32) {
    let w = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), vec2(w, w));
    let s = Stroke::new((w / 14.0).max(1.3), color);
    let c = r.center();
    match icon {
        Icon::Library => {
            for (x, y) in [(0.14, 0.14), (0.54, 0.14), (0.14, 0.54), (0.54, 0.54)] {
                painter.rect_stroke(
                    Rect::from_min_size(p(r, x, y), vec2(w * 0.32, w * 0.32)),
                    w * 0.06,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
        }
        Icon::Light => {
            painter.circle_stroke(c, w * 0.18, s);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let d = vec2(a.cos(), a.sin());
                painter.line_segment([c + d * w * 0.29, c + d * w * 0.42], s);
            }
        }
        Icon::Color => {
            for (dx, dy) in [(0.0, -0.11), (-0.11, 0.08), (0.11, 0.08)] {
                painter.circle_stroke(c + vec2(dx, dy) * w, w * 0.2, s);
            }
        }
        Icon::Detail => {
            painter.add(egui::Shape::closed_line(
                vec![
                    p(r, 0.5, 0.12),
                    p(r, 0.85, 0.5),
                    p(r, 0.5, 0.88),
                    p(r, 0.15, 0.5),
                ],
                s,
            ));
            painter.line_segment([p(r, 0.5, 0.12), p(r, 0.5, 0.88)], s);
        }
        Icon::Masks => {
            painter.circle_stroke(p(r, 0.36, 0.5), w * 0.24, s);
            for y in [0.3, 0.5, 0.7] {
                painter.line_segment([p(r, 0.66, y), p(r, 0.9, y)], s);
            }
        }
        Icon::Films => {
            painter.rect_stroke(
                Rect::from_min_max(p(r, 0.12, 0.2), p(r, 0.88, 0.8)),
                w * 0.05,
                s,
                egui::StrokeKind::Middle,
            );
            for k in 0..4 {
                let x = 0.22 + k as f32 * 0.19;
                for y in [0.27, 0.73] {
                    painter.circle_filled(p(r, x, y), w * 0.035, color);
                }
            }
            painter.line_segment([p(r, 0.12, 0.36), p(r, 0.88, 0.36)], s);
            painter.line_segment([p(r, 0.12, 0.64), p(r, 0.88, 0.64)], s);
        }
        Icon::Geometry => {
            painter.add(egui::Shape::line(
                vec![p(r, 0.28, 0.1), p(r, 0.28, 0.72), p(r, 0.9, 0.72)],
                s,
            ));
            painter.add(egui::Shape::line(
                vec![p(r, 0.1, 0.28), p(r, 0.72, 0.28), p(r, 0.72, 0.9)],
                s,
            ));
        }
        Icon::Merge | Icon::Stack => {
            for k in 0..3 {
                let o = k as f32 * 0.12;
                painter.rect_stroke(
                    Rect::from_min_size(p(r, 0.14 + o, 0.38 - o), vec2(w * 0.46, w * 0.46)),
                    w * 0.05,
                    s,
                    egui::StrokeKind::Middle,
                );
            }
        }
        Icon::Panorama => {
            painter.add(egui::Shape::closed_line(
                vec![
                    p(r, 0.08, 0.3),
                    p(r, 0.5, 0.38),
                    p(r, 0.92, 0.3),
                    p(r, 0.92, 0.7),
                    p(r, 0.5, 0.62),
                    p(r, 0.08, 0.7),
                ],
                s,
            ));
        }
        Icon::Star => {
            let pts: Vec<Pos2> = (0..10)
                .map(|k| {
                    let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
                    let rr = if k % 2 == 0 { 0.42 } else { 0.18 };
                    c + vec2(a.cos(), a.sin()) * w * rr
                })
                .collect();
            painter.add(egui::Shape::closed_line(pts, s));
        }
        Icon::Export => {
            painter.add(egui::Shape::line(
                vec![
                    p(r, 0.35, 0.3),
                    p(r, 0.2, 0.3),
                    p(r, 0.2, 0.85),
                    p(r, 0.8, 0.85),
                    p(r, 0.8, 0.3),
                    p(r, 0.65, 0.3),
                ],
                s,
            ));
            painter.line_segment([p(r, 0.5, 0.62), p(r, 0.5, 0.1)], s);
            painter.add(egui::Shape::line(
                vec![p(r, 0.36, 0.24), p(r, 0.5, 0.1), p(r, 0.64, 0.24)],
                s,
            ));
        }
        Icon::Open => {
            painter.add(egui::Shape::closed_line(
                vec![
                    p(r, 0.1, 0.25),
                    p(r, 0.4, 0.25),
                    p(r, 0.48, 0.35),
                    p(r, 0.9, 0.35),
                    p(r, 0.9, 0.8),
                    p(r, 0.1, 0.8),
                ],
                s,
            ));
        }
        Icon::Radial => {
            painter.circle_stroke(c, w * 0.38, s);
            painter.circle_stroke(
                c,
                w * 0.2,
                Stroke::new(s.width * 0.7, color.gamma_multiply(0.6)),
            );
            painter.circle_filled(c, w * 0.05, color);
        }
        Icon::Linear => {
            for (y, k) in [(0.22, 1.0), (0.5, 0.55), (0.78, 1.0)] {
                painter.line_segment(
                    [p(r, 0.1, y), p(r, 0.9, y)],
                    Stroke::new(s.width, color.gamma_multiply(k)),
                );
            }
        }
        Icon::Brush => {
            painter.line_segment(
                [p(r, 0.85, 0.12), p(r, 0.42, 0.55)],
                Stroke::new(s.width * 1.4, color),
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    p(r, 0.42, 0.5),
                    p(r, 0.52, 0.6),
                    p(r, 0.36, 0.86),
                    p(r, 0.12, 0.88),
                    p(r, 0.2, 0.66),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
}
