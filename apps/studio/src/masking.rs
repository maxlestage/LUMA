//! Les masques à l'écran : poignées des dégradés et pinceau.
//!
//! - dégradé linéaire : trois lignes (plein effet, milieu, plus d'effet) ; on tire le
//!   point de départ ou d'arrivée, ou la ligne du milieu pour tout déplacer ;
//! - dégradé radial : l'ellipse et son contour progressif ; on tire le centre, ou les
//!   poignées de droite et du bas pour les rayons ;
//! - pinceau : on peint en glissant ; Alt (Option) gomme.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use luma_project::{Dab, Mask, Shape};

use crate::widgets::Change;

/// Les réglages du pinceau (Taille en fraction de la largeur, Contour progressif,
/// Débit).
#[derive(Clone, Copy, Debug)]
pub struct Brush {
    pub radius: f32,
    pub feather: f32,
    pub flow: f32,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            radius: 0.04,
            feather: 50.0,
            flow: 100.0,
        }
    }
}

const ACCENT: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Grip {
    Start,
    End,
    Middle,
    Center,
    RadiusX,
    RadiusY,
    Paint,
}

fn handle(p: &egui::Painter, at: Pos2, filled: bool) {
    p.circle_filled(at, 6.0, Color32::from_black_alpha(140));
    p.circle_stroke(at, 6.0, Stroke::new(1.5, ACCENT));
    if filled {
        p.circle_filled(at, 3.0, ACCENT);
    }
}

/// Les poignées du masque sur `rect` (la photo à l'écran) ; modifie le masque.
pub fn edit(ui: &mut Ui, rect: Rect, mask: &mut Mask, brush: &Brush) -> Change {
    let mut change = Change::default();
    let to_screen = |(x, y): (f32, f32)| {
        pos2(
            rect.min.x + x * rect.width(),
            rect.min.y + y * rect.height(),
        )
    };
    let to_norm = |p: Pos2| {
        (
            (p.x - rect.min.x) / rect.width(),
            (p.y - rect.min.y) / rect.height(),
        )
    };
    let id = ui.scope_id().with("masque");
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let grip_id = id.with("poignée");
    let near = |a: Pos2, b: Pos2| a.distance(b) < 12.0;
    let pick = |p: Pos2, mask: &Mask| -> Option<Grip> {
        match &mask.shape {
            Shape::Linear { start, end } => {
                let (s, e) = (to_screen(*start), to_screen(*end));
                if near(p, s) {
                    Some(Grip::Start)
                } else if near(p, e) {
                    Some(Grip::End)
                } else if near(p, s + (e - s) * 0.5) {
                    Some(Grip::Middle)
                } else {
                    None
                }
            }
            Shape::Radial {
                center,
                radius,
                angle,
                ..
            } => {
                let c = to_screen(*center);
                let (s, co) = angle.to_radians().sin_cos();
                let rx = vec2(co, s) * radius.0 * rect.width();
                let ry = vec2(-s, co) * radius.1 * rect.height();
                if near(p, c + rx) {
                    Some(Grip::RadiusX)
                } else if near(p, c + ry) {
                    Some(Grip::RadiusY)
                } else if near(p, c) || (p - c).length() < 0.5 * rx.length().min(ry.length()) {
                    Some(Grip::Center)
                } else {
                    None
                }
            }
            Shape::Brush { .. } => Some(Grip::Paint),
            Shape::Full => None,
        }
    };
    if response.drag_started()
        && let Some(p) = response.interact_pointer_pos()
        && let Some(g) = pick(p, mask)
    {
        ui.data_mut(|d| d.insert_temp(grip_id, g));
    }
    let grip: Option<Grip> = ui.data(|d| d.get_temp(grip_id));
    if let Some(p) = response.hover_pos() {
        match grip.or_else(|| pick(p, mask)) {
            Some(Grip::Paint) => ui.ctx().set_cursor_icon(CursorIcon::Crosshair),
            Some(_) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
            None => {}
        }
    }
    let erase = ui.input(|i| i.modifiers.alt);
    if (response.dragged() || response.clicked())
        && let (Some(g), Some(p)) = (
            grip.or(response.clicked().then_some(Grip::Paint)),
            response.interact_pointer_pos(),
        )
    {
        let d = response.drag_delta();
        let dn = (d.x / rect.width(), d.y / rect.height());
        let n = to_norm(p);
        match (&mut mask.shape, g) {
            (Shape::Linear { start, .. }, Grip::Start) => *start = n,
            (Shape::Linear { end, .. }, Grip::End) => *end = n,
            (Shape::Linear { start, end }, Grip::Middle) => {
                *start = (start.0 + dn.0, start.1 + dn.1);
                *end = (end.0 + dn.0, end.1 + dn.1);
            }
            (Shape::Radial { center, .. }, Grip::Center) => {
                *center = (center.0 + dn.0, center.1 + dn.1)
            }
            (
                Shape::Radial {
                    center,
                    radius,
                    angle,
                    ..
                },
                Grip::RadiusX,
            ) => {
                // La poignée de droite règle le rayon et l'orientation.
                let c = to_screen(*center);
                let v = p - c;
                radius.0 = (v.length() / rect.width()).max(0.01);
                *angle = v.y.atan2(v.x).to_degrees();
            }
            (
                Shape::Radial {
                    center,
                    radius,
                    angle,
                    ..
                },
                Grip::RadiusY,
            ) => {
                let c = to_screen(*center);
                let (s, co) = angle.to_radians().sin_cos();
                let along = (p - c).dot(vec2(-s, co));
                radius.1 = (along.abs() / rect.height()).max(0.01);
            }
            (Shape::Brush { dabs }, Grip::Paint) => {
                // Une touche tous les quarts de rayon, le long du geste.
                let spacing = brush.radius * 0.25;
                let last = dabs.last().map(|d| (d.x, d.y));
                let far = last
                    .is_none_or(|(x, y)| to_screen((x, y)).distance(p) >= spacing * rect.width())
                    || response.drag_started()
                    || response.clicked();
                if far {
                    dabs.push(Dab {
                        x: n.0,
                        y: n.1,
                        radius: brush.radius,
                        feather: brush.feather,
                        flow: brush.flow,
                        erase,
                    });
                }
            }
            _ => {}
        }
        change.changed = true;
    }
    if response.drag_stopped() || response.clicked() {
        ui.data_mut(|d| d.remove::<Grip>(grip_id));
        change.committed = true;
    }

    // Dessin.
    let p = ui.painter_at(rect);
    let line = Stroke::new(1.2, Color32::from_white_alpha(200));
    match &mask.shape {
        Shape::Linear { start, end } => {
            let (s, e) = (to_screen(*start), to_screen(*end));
            let dir = (e - s).normalized();
            let perp = vec2(-dir.y, dir.x) * (rect.width() + rect.height());
            for (k, at) in [s, s + (e - s) * 0.5, e].into_iter().enumerate() {
                let stroke = if k == 1 {
                    Stroke::new(1.0, Color32::from_white_alpha(120))
                } else {
                    line
                };
                p.line_segment([at - perp, at + perp], stroke);
            }
            handle(&p, s, true);
            handle(&p, e, false);
            handle(&p, s + (e - s) * 0.5, false);
        }
        Shape::Radial {
            center,
            radius,
            angle,
            feather,
        } => {
            let c = to_screen(*center);
            let (s, co) = angle.to_radians().sin_cos();
            let (ax, ay) = (radius.0 * rect.width(), radius.1 * rect.height());
            let ellipse = |k: f32| -> Vec<Pos2> {
                (0..=72)
                    .map(|i| {
                        let t = i as f32 / 72.0 * std::f32::consts::TAU;
                        let (u, v) = (t.cos() * ax * k, t.sin() * ay * k);
                        c + vec2(u * co - v * s, u * s + v * co)
                    })
                    .collect()
            };
            p.add(egui::Shape::line(ellipse(1.0), line));
            let inner = 1.0 - feather / 100.0;
            if inner > 0.02 {
                p.add(egui::Shape::dashed_line(
                    &ellipse(inner),
                    Stroke::new(1.0, Color32::from_white_alpha(140)),
                    6.0,
                    5.0,
                ));
            }
            handle(&p, c, true);
            handle(&p, c + vec2(co, s) * ax, false);
            handle(&p, c + vec2(-s, co) * ay, false);
        }
        Shape::Full => {
            // Toute l'image : un cadre discret suffit.
            p.rect_stroke(
                rect.shrink(1.0),
                0.0,
                Stroke::new(1.5, Color32::from_white_alpha(90)),
                egui::StrokeKind::Inside,
            );
        }
        Shape::Brush { dabs } => {
            if let Some(last) = dabs.last() {
                handle(&p, to_screen((last.x, last.y)), true);
            }
            if let Some(h) = response.hover_pos() {
                let r = brush.radius * rect.width();
                p.circle_stroke(
                    h,
                    r,
                    Stroke::new(
                        1.2,
                        if erase {
                            Color32::from_rgb(0xff, 0x80, 0x80)
                        } else {
                            ACCENT
                        },
                    ),
                );
                p.circle_stroke(
                    h,
                    r * (1.0 - brush.feather / 100.0),
                    Stroke::new(1.0, Color32::from_white_alpha(110)),
                );
            }
        }
    }
    change
}

/// Le masque en rouge par‑dessus la photo (touche O) : une image de `w × h`. Sa plage
/// de luminance se lit sur `shown`, la copie réduite de l'image affichée (taille, RGBA).
pub fn overlay(mask: &Mask, w: usize, h: usize, shown: &([usize; 2], Vec<u8>)) -> egui::ColorImage {
    let mut weights = luma_image::local::weights(mask, w, h);
    let ([sw, sh], rgba) = shown;
    if let Some(r) = mask.range
        && *sw > 0
        && *sh > 0
        && rgba.len() >= sw * sh * 4
    {
        for (i, v) in weights.data.iter_mut().enumerate() {
            let (x, y) = (i % w * sw / w.max(1), i / w * sh / h.max(1));
            let p = &rgba[(y.min(sh - 1) * sw + x.min(sw - 1)) * 4..][..3];
            let lin = [0, 1, 2].map(|c| luma_color::srgb_decode(f32::from(p[c]) / 255.0));
            *v *= r.factor(luma_image::local::lightness(lin));
        }
    }
    let pixels = weights
        .data
        .iter()
        .map(|&v| Color32::from_rgba_unmultiplied(255, 40, 60, (v.clamp(0.0, 1.0) * 150.0) as u8))
        .collect();
    egui::ColorImage::new([w, h], pixels)
}
