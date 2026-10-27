//! Le tampon et le correcteur à l'écran (touche Q) : un clic pose une retouche, sa
//! source choisie d'elle‑même ; on tire la zone ou sa source pour les déplacer.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, pos2};
use luma_project::{Spot, SpotKind};

use crate::widgets::Change;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Grip {
    Center(usize),
    Source(usize),
}

fn circle(center: Pos2, r: f32) -> Vec<Pos2> {
    (0..=64)
        .map(|i| {
            let t = i as f32 / 64.0 * std::f32::consts::TAU;
            center + egui::vec2(t.cos(), t.sin()) * r
        })
        .collect()
}

/// Une nouvelle retouche en `center` : la source cherchée sur `scene` (une copie réduite
/// de la photo), à côté sinon.
pub fn place(
    kind: SpotKind,
    center: (f32, f32),
    radius: f32,
    scene: Option<&luma_core::RgbImage>,
) -> Spot {
    let mut spot = Spot::new(kind, center, radius);
    // Les yeux rouges n'ont pas de source.
    if kind != SpotKind::RedEye
        && let Some(source) =
            scene.and_then(|s| luma_image::retouch::find_source(s, center, radius))
    {
        spot.source = source;
    }
    spot
}

/// Les retouches sur `rect` (la photo à l'écran). `placing` : l'outil actif, un clic hors
/// des retouches en pose une nouvelle de rayon `radius`.
pub fn edit(
    ui: &mut Ui,
    rect: Rect,
    spots: &mut Vec<Spot>,
    selected: &mut Option<usize>,
    placing: Option<SpotKind>,
    radius: f32,
    scene: Option<&luma_core::RgbImage>,
) -> Change {
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
    let id = ui.scope_id().with("retouche");
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let grip_id = id.with("poignée");
    let pick = |p: Pos2, spots: &[Spot], selected: Option<usize>| -> Option<Grip> {
        // La retouche choisie d'abord, puis les plus récentes.
        let order = selected
            .filter(|&i| i < spots.len())
            .into_iter()
            .chain((0..spots.len()).rev());
        for i in order {
            let s = &spots[i];
            let r = (s.radius * rect.width()).max(6.0);
            if Some(i) == selected
                && s.kind != SpotKind::RedEye
                && p.distance(to_screen(s.source)) < r
            {
                return Some(Grip::Source(i));
            }
            if p.distance(to_screen(s.center)) < r {
                return Some(Grip::Center(i));
            }
        }
        None
    };
    if response.drag_started()
        && let Some(p) = response.interact_pointer_pos()
        && let Some(g) = pick(p, spots, *selected)
    {
        let (Grip::Center(i) | Grip::Source(i)) = g;
        *selected = Some(i);
        ui.data_mut(|d| d.insert_temp(grip_id, g));
    }
    let grip: Option<Grip> = ui.data(|d| d.get_temp(grip_id));
    if let Some(g) = grip
        && response.dragged()
    {
        let delta = response.drag_delta();
        let (dx, dy) = (delta.x / rect.width(), delta.y / rect.height());
        match g {
            Grip::Center(i) if i < spots.len() => {
                spots[i].center.0 += dx;
                spots[i].center.1 += dy;
            }
            Grip::Source(i) if i < spots.len() => {
                spots[i].source.0 += dx;
                spots[i].source.1 += dy;
            }
            _ => {}
        }
        if delta != egui::Vec2::ZERO {
            change.changed = true;
        }
    }
    if response.drag_stopped() && grip.is_some() {
        ui.data_mut(|d| d.remove::<Grip>(grip_id));
        change.committed = true;
    }
    if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        match pick(p, spots, *selected) {
            Some(Grip::Center(i) | Grip::Source(i)) => *selected = Some(i),
            None => match placing {
                Some(kind) => {
                    spots.push(place(kind, to_norm(p), radius, scene));
                    *selected = Some(spots.len() - 1);
                    change.changed = true;
                    change.committed = true;
                }
                None => *selected = None,
            },
        }
    }
    if let Some(p) = response.hover_pos() {
        match grip.or_else(|| pick(p, spots, *selected)) {
            Some(_) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
            None if placing.is_some() => ui.ctx().set_cursor_icon(CursorIcon::Crosshair),
            None => {}
        }
    }

    let painter = ui.painter_at(rect);
    let white = Color32::WHITE;
    let dim = Color32::from_white_alpha(120);
    for (i, s) in spots.iter().enumerate() {
        let r = s.radius * rect.width();
        let (c, src) = (to_screen(s.center), to_screen(s.source));
        if Some(i) == *selected {
            painter.circle_stroke(c, r + 1.0, Stroke::new(3.0, Color32::from_black_alpha(110)));
            painter.circle_stroke(c, r, Stroke::new(1.5, white));
            // Les yeux rouges n'ont pas de source.
            let sourced = s.kind != SpotKind::RedEye;
            if sourced {
                painter.add(egui::Shape::dashed_line(
                    &circle(src, r),
                    Stroke::new(1.2, white),
                    5.0,
                    4.0,
                ));
            }
            let dir = (c - src).normalized();
            if sourced && (c - src).length() > 2.0 * r {
                painter.line_segment([src + dir * r, c - dir * r], Stroke::new(1.0, dim));
            }
            painter.circle_filled(c, 2.5, white);
        } else {
            painter.circle_stroke(c, r, Stroke::new(1.0, dim));
        }
    }
    if placing.is_some()
        && grip.is_none()
        && let Some(p) = response.hover_pos()
        && pick(p, spots, *selected).is_none()
    {
        painter.circle_stroke(
            p,
            radius * rect.width(),
            Stroke::new(1.0, Color32::from_white_alpha(170)),
        );
    }
    change
}
