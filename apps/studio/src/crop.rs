//! L'outil Recadrage (touche R) : un cadre posé sur toute la
//! photo, qu'on déplace en glissant à l'intérieur et qu'on redimensionne par ses coins
//! et ses côtés ; un guide aide à composer (tiers, nombre d'or, diagonales, triangles
//! d'or, spirale d'or, grille : O pour passer au suivant, Maj+O pour tourner la spirale
//! et les triangles) ; les proportions peuvent être verrouillées. Entrée valide, Échap
//! annule.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use luma_project::Crop;

use crate::widgets::Change;

/// Les proportions proposées : `None` = libre ; `Some(0.0)` = celles de la photo.
pub const ASPECTS: [(&str, Option<f32>); 7] = [
    ("Libre", None),
    ("Original", Some(0.0)),
    ("1 × 1", Some(1.0)),
    ("4 × 5", Some(4.0 / 5.0)),
    ("2 × 3", Some(2.0 / 3.0)),
    ("3 × 2", Some(3.0 / 2.0)),
    ("16 × 9", Some(16.0 / 9.0)),
];

/// Les guides de composition, dans l'ordre de la touche O.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Guide {
    None,
    #[default]
    Thirds,
    Golden,
    Diagonals,
    Triangles,
    Spiral,
    Grid,
}

impl Guide {
    pub const ALL: [Guide; 7] = [
        Guide::None,
        Guide::Thirds,
        Guide::Golden,
        Guide::Diagonals,
        Guide::Triangles,
        Guide::Spiral,
        Guide::Grid,
    ];

    /// Le nom (à traduire).
    pub fn name(self) -> &'static str {
        match self {
            Guide::None => "Aucun guide",
            Guide::Thirds => "Tiers",
            Guide::Golden => "Nombre d'or",
            Guide::Diagonals => "Diagonales",
            Guide::Triangles => "Triangles d'or",
            Guide::Spiral => "Spirale d'or",
            Guide::Grid => "Grille",
        }
    }

    /// Le guide suivant (touche O).
    pub fn next(self) -> Guide {
        let i = Guide::ALL.iter().position(|g| *g == self).unwrap_or(0);
        Guide::ALL[(i + 1) % Guide::ALL.len()]
    }
}

/// Le nombre d'or.
const PHI: f32 = 1.618_034;

/// Les traits du guide dans `frame` : des segments (la spirale, une ligne brisée fine).
/// `turn` (0 à 3) tourne la spirale et les triangles d'un quart de tour à chaque fois.
pub fn guide_lines(guide: Guide, frame: Rect, turn: u8) -> Vec<[Pos2; 2]> {
    let at = |u: f32, v: f32| {
        pos2(
            frame.min.x + u * frame.width(),
            frame.min.y + v * frame.height(),
        )
    };
    // Un quart de tour : miroir horizontal, puis vertical.
    let (fx, fy) = (turn & 1 == 1, turn & 2 == 2);
    let turned = |u: f32, v: f32| at(if fx { 1.0 - u } else { u }, if fy { 1.0 - v } else { v });
    let mut out = Vec::new();
    let mut lines = |ks: &[f32]| {
        for &k in ks {
            out.push([at(k, 0.0), at(k, 1.0)]);
            out.push([at(0.0, k), at(1.0, k)]);
        }
    };
    match guide {
        Guide::None => {}
        Guide::Thirds => lines(&[1.0 / 3.0, 2.0 / 3.0]),
        Guide::Golden => lines(&[1.0 - 1.0 / PHI, 1.0 / PHI]),
        Guide::Grid => lines(&(1..8).map(|k| k as f32 / 8.0).collect::<Vec<_>>()),
        Guide::Diagonals => {
            // De chaque coin, une ligne à 45° (à l'écran) jusqu'au côté opposé.
            let s = frame.width().min(frame.height());
            for (c, d) in [
                (frame.left_top(), vec2(1.0, 1.0)),
                (frame.right_top(), vec2(-1.0, 1.0)),
                (frame.left_bottom(), vec2(1.0, -1.0)),
                (frame.right_bottom(), vec2(-1.0, -1.0)),
            ] {
                out.push([c, c + d * s]);
            }
        }
        Guide::Triangles => {
            // Une diagonale, et de chacun des deux autres coins, sa perpendiculaire.
            let (a, b) = (turned(0.0, 0.0), turned(1.0, 1.0));
            out.push([a, b]);
            let d = (b - a).normalized();
            for c in [turned(1.0, 0.0), turned(0.0, 1.0)] {
                let foot = a + d * (c - a).dot(d);
                out.push([c, foot]);
            }
        }
        Guide::Spiral => {
            // Des quarts de cercle dans des carrés de plus en plus petits, construits dans
            // un rectangle d'or puis étirés au cadre.
            let (mut x0, mut y0, mut x1, mut y1) = (0.0f32, 0.0f32, PHI, 1.0f32);
            let map = |x: f32, y: f32| turned(x / PHI, y);
            for i in 0..10 {
                let (w, h) = (x1 - x0, y1 - y0);
                // Le carré se prend à gauche, en haut, à droite puis en bas, tour à tour.
                let (cx, cy, start, side) = match i % 4 {
                    0 => {
                        let s = h;
                        x0 += s;
                        (x0, y1, std::f32::consts::PI, s)
                    }
                    1 => {
                        let s = w;
                        y0 += s;
                        (x0, y0, std::f32::consts::FRAC_PI_2 * 3.0, s)
                    }
                    2 => {
                        let s = h;
                        x1 -= s;
                        (x1, y0, 0.0, s)
                    }
                    _ => {
                        let s = w;
                        y1 -= s;
                        (x1, y1, std::f32::consts::FRAC_PI_2, s)
                    }
                };
                let steps = 12;
                for k in 0..steps {
                    let a0 = start + std::f32::consts::FRAC_PI_2 * k as f32 / steps as f32;
                    let a1 = start + std::f32::consts::FRAC_PI_2 * (k + 1) as f32 / steps as f32;
                    out.push([
                        map(cx + side * a0.cos(), cy + side * a0.sin()),
                        map(cx + side * a1.cos(), cy + side * a1.sin()),
                    ]);
                }
            }
        }
    }
    out
}

/// Le plus petit cadre, en fraction de la photo.
const MIN: f32 = 0.05;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Handle {
    Move,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Handle {
    fn cursor(self) -> CursorIcon {
        match self {
            Handle::Move => CursorIcon::Grab,
            Handle::Left | Handle::Right => CursorIcon::ResizeHorizontal,
            Handle::Top | Handle::Bottom => CursorIcon::ResizeVertical,
            Handle::TopLeft | Handle::BottomRight => CursorIcon::ResizeNwSe,
            Handle::TopRight | Handle::BottomLeft => CursorIcon::ResizeNeSw,
        }
    }
    fn moves_left(self) -> bool {
        matches!(self, Handle::Left | Handle::TopLeft | Handle::BottomLeft)
    }
    fn moves_right(self) -> bool {
        matches!(self, Handle::Right | Handle::TopRight | Handle::BottomRight)
    }
    fn moves_top(self) -> bool {
        matches!(self, Handle::Top | Handle::TopLeft | Handle::TopRight)
    }
    fn moves_bottom(self) -> bool {
        matches!(
            self,
            Handle::Bottom | Handle::BottomLeft | Handle::BottomRight
        )
    }
}

/// Le rapport largeur / hauteur en fractions de la photo pour des proportions en
/// pixels `aspect`, sur une photo de proportions `image`.
fn norm_ratio(aspect: f32, image: f32) -> f32 {
    aspect / image
}

/// Ramène le cadre aux proportions demandées, autour de son centre, dans la photo.
pub fn constrain(c: &mut Crop, aspect: Option<f32>, image: f32) {
    let Some(a) = aspect else { return };
    let a = if a == 0.0 { image } else { a };
    let r = norm_ratio(a, image);
    let (cx, cy) = ((c.left + c.right) / 2.0, (c.top + c.bottom) / 2.0);
    let (mut w, mut h) = (c.right - c.left, c.bottom - c.top);
    // Garde la même surface, puis rentre dans la photo.
    let area = (w * h).max(MIN * MIN);
    w = (area * r).sqrt();
    h = w / r;
    let fit = (1.0 / w).min(1.0 / h).min(1.0);
    w *= fit;
    h *= fit;
    let cx = cx.clamp(w / 2.0, 1.0 - w / 2.0);
    let cy = cy.clamp(h / 2.0, 1.0 - h / 2.0);
    c.left = cx - w / 2.0;
    c.right = cx + w / 2.0;
    c.top = cy - h / 2.0;
    c.bottom = cy + h / 2.0;
}

/// Applique un déplacement `d` (en fractions de la photo) de la poignée `h`.
fn drag(c: &mut Crop, h: Handle, d: egui::Vec2, aspect: Option<f32>, image: f32) {
    if h == Handle::Move {
        let (w, ht) = (c.right - c.left, c.bottom - c.top);
        let l = (c.left + d.x).clamp(0.0, 1.0 - w);
        let t = (c.top + d.y).clamp(0.0, 1.0 - ht);
        c.left = l;
        c.right = l + w;
        c.top = t;
        c.bottom = t + ht;
        return;
    }
    if h.moves_left() {
        c.left = (c.left + d.x).clamp(0.0, c.right - MIN);
    }
    if h.moves_right() {
        c.right = (c.right + d.x).clamp(c.left + MIN, 1.0);
    }
    if h.moves_top() {
        c.top = (c.top + d.y).clamp(0.0, c.bottom - MIN);
    }
    if h.moves_bottom() {
        c.bottom = (c.bottom + d.y).clamp(c.top + MIN, 1.0);
    }
    let Some(a) = aspect else { return };
    let r = norm_ratio(if a == 0.0 { image } else { a }, image);
    // Proportions verrouillées : le côté tiré décide, l'autre suit, ancré au côté opposé.
    let horizontal = matches!(h, Handle::Left | Handle::Right)
        || (!matches!(h, Handle::Top | Handle::Bottom) && d.x.abs() >= d.y.abs() * r);
    let (mut w, mut ht) = (c.right - c.left, c.bottom - c.top);
    if horizontal {
        ht = w / r;
    } else {
        w = ht * r;
    }
    // Rester dans la photo.
    let max_w = if h.moves_left() {
        c.right
    } else if h.moves_right() {
        1.0 - c.left
    } else {
        1.0
    };
    let max_h = if h.moves_top() {
        c.bottom
    } else if h.moves_bottom() {
        1.0 - c.top
    } else {
        1.0
    };
    let fit = (max_w / w).min(max_h / ht).min(1.0);
    w *= fit;
    ht *= fit;
    if h.moves_left() {
        c.left = c.right - w;
    } else if h.moves_right() {
        c.right = c.left + w;
    } else {
        let cx = ((c.left + c.right) / 2.0).clamp(w / 2.0, 1.0 - w / 2.0);
        c.left = cx - w / 2.0;
        c.right = cx + w / 2.0;
    }
    if h.moves_top() {
        c.top = c.bottom - ht;
    } else if h.moves_bottom() {
        c.bottom = c.top + ht;
    } else {
        let cy = ((c.top + c.bottom) / 2.0).clamp(ht / 2.0, 1.0 - ht / 2.0);
        c.top = cy - ht / 2.0;
        c.bottom = cy + ht / 2.0;
    }
}

/// Le cadre et ses poignées sur `image_rect` (la photo entière à l'écran).
pub fn edit(
    ui: &mut Ui,
    image_rect: Rect,
    crop: &mut Crop,
    aspect: Option<f32>,
    guide: Guide,
    turn: u8,
) -> Change {
    let mut change = Change::default();
    let image = image_rect.width() / image_rect.height().max(1.0);
    let to_screen = |x: f32, y: f32| -> Pos2 {
        pos2(
            image_rect.min.x + x * image_rect.width(),
            image_rect.min.y + y * image_rect.height(),
        )
    };
    let frame = Rect::from_min_max(
        to_screen(crop.left, crop.top),
        to_screen(crop.right, crop.bottom),
    );

    let id = ui.scope_id().with("recadrage");
    let response = ui.interact(image_rect.expand(12.0), id, Sense::click_and_drag());
    let grab = 10.0;
    let pick = |p: Pos2| -> Option<Handle> {
        let near = |a: f32, b: f32| (a - b).abs() < grab;
        let inside_x = p.x > frame.min.x - grab && p.x < frame.max.x + grab;
        let inside_y = p.y > frame.min.y - grab && p.y < frame.max.y + grab;
        let (l, r, t, b) = (
            near(p.x, frame.min.x),
            near(p.x, frame.max.x),
            near(p.y, frame.min.y),
            near(p.y, frame.max.y),
        );
        Some(match (l, r, t, b) {
            (true, _, true, _) => Handle::TopLeft,
            (_, true, true, _) => Handle::TopRight,
            (true, _, _, true) => Handle::BottomLeft,
            (_, true, _, true) => Handle::BottomRight,
            (true, ..) if inside_y => Handle::Left,
            (_, true, ..) if inside_y => Handle::Right,
            (_, _, true, _) if inside_x => Handle::Top,
            (.., true) if inside_x => Handle::Bottom,
            _ if frame.contains(p) => Handle::Move,
            _ => return None,
        })
    };
    let active_id = id.with("poignée");
    if response.drag_started()
        && let Some(p) = response.interact_pointer_pos()
        && let Some(h) = pick(p)
    {
        ui.data_mut(|d| d.insert_temp(active_id, h));
    }
    let active: Option<Handle> = ui.data(|d| d.get_temp(active_id));
    if let Some(h) = active.or_else(|| response.hover_pos().and_then(pick)) {
        ui.ctx().set_cursor_icon(if active == Some(Handle::Move) {
            CursorIcon::Grabbing
        } else {
            h.cursor()
        });
    }
    if response.dragged()
        && let Some(h) = active
    {
        let d = response.drag_delta();
        let d = vec2(d.x / image_rect.width(), d.y / image_rect.height());
        let before = *crop;
        drag(crop, h, d, aspect, image);
        change.changed = *crop != before;
    }
    if response.drag_stopped() {
        ui.data_mut(|d| d.remove::<Handle>(active_id));
        change.committed = true;
    }

    // Dessin : l'extérieur assombri, le cadre, le guide, les poignées.
    let frame = Rect::from_min_max(
        to_screen(crop.left, crop.top),
        to_screen(crop.right, crop.bottom),
    );
    let p = ui.painter_at(image_rect.expand(14.0));
    let shade = Color32::from_black_alpha(150);
    p.rect_filled(
        Rect::from_min_max(image_rect.min, pos2(image_rect.max.x, frame.min.y)),
        0.0,
        shade,
    );
    p.rect_filled(
        Rect::from_min_max(pos2(image_rect.min.x, frame.max.y), image_rect.max),
        0.0,
        shade,
    );
    p.rect_filled(
        Rect::from_min_max(
            pos2(image_rect.min.x, frame.min.y),
            pos2(frame.min.x, frame.max.y),
        ),
        0.0,
        shade,
    );
    p.rect_filled(
        Rect::from_min_max(
            pos2(frame.max.x, frame.min.y),
            pos2(image_rect.max.x, frame.max.y),
        ),
        0.0,
        shade,
    );
    let thin = Stroke::new(1.0, Color32::from_white_alpha(110));
    for line in guide_lines(guide, frame, turn) {
        p.line_segment(line, thin);
    }
    p.rect_stroke(
        frame,
        0.0,
        Stroke::new(1.5, Color32::WHITE),
        egui::StrokeKind::Middle,
    );
    let corner = Stroke::new(4.0, Color32::WHITE);
    let len = 18.0f32.min(frame.width() / 3.0).min(frame.height() / 3.0);
    for (c, dx, dy) in [
        (frame.left_top(), 1.0, 1.0),
        (frame.right_top(), -1.0, 1.0),
        (frame.left_bottom(), 1.0, -1.0),
        (frame.right_bottom(), -1.0, -1.0),
    ] {
        p.line_segment([c, c + vec2(len * dx, 0.0)], corner);
        p.line_segment([c, c + vec2(0.0, len * dy)], corner);
    }
    change
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guides_stay_in_the_frame_and_the_spiral_is_continuous() {
        let frame = Rect::from_min_size(pos2(10.0, 20.0), vec2(600.0, 400.0));
        let inside = |p: Pos2| frame.expand(0.5).contains(p);
        for guide in Guide::ALL {
            for turn in 0..4 {
                let lines = guide_lines(guide, frame, turn);
                assert_eq!(lines.is_empty(), guide == Guide::None, "{guide:?}");
                assert!(
                    lines.iter().flatten().all(|p| inside(*p)),
                    "{guide:?} {turn}"
                );
            }
        }
        // Les tiers et le nombre d'or : deux traits par sens, aux bons endroits.
        let thirds = guide_lines(Guide::Thirds, frame, 0);
        assert_eq!(thirds.len(), 4);
        assert!((thirds[0][0].x - 210.0).abs() < 1e-3);
        let golden = guide_lines(Guide::Golden, frame, 0);
        assert!((golden[0][0].x - (10.0 + 600.0 * 0.381_966)).abs() < 0.1);
        // La spirale : chaque arc part du bout du précédent, et elle s'enroule vers son
        // œil, à 1 / (1 + 1/φ²) ≈ 72,4 % de la largeur et de la hauteur.
        let spiral = guide_lines(Guide::Spiral, frame, 0);
        for w in spiral.windows(2) {
            assert!(w[0][1].distance(w[1][0]) < 0.5, "{:?}", w);
        }
        let eye = spiral.last().unwrap()[1];
        let k = 1.0 / (1.0 + 1.0 / (PHI * PHI));
        let expected = pos2(10.0 + 600.0 * k, 20.0 + 400.0 * k);
        assert!(eye.distance(expected) < 6.0, "{eye:?} {expected:?}");
        // Un quart de tour la retourne.
        assert_ne!(guide_lines(Guide::Spiral, frame, 1)[0], spiral[0]);
        // Les triangles : la perpendiculaire tombe à angle droit sur la diagonale.
        let t = guide_lines(Guide::Triangles, frame, 0);
        let d = t[0][1] - t[0][0];
        let p = t[1][1] - t[1][0];
        assert!(d.dot(p).abs() < 1e-2 * d.length() * p.length());
        assert_eq!(Guide::Grid.next(), Guide::None);
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn moving_stays_inside_the_photo() {
        let mut c = Crop {
            left: 0.2,
            top: 0.2,
            right: 0.6,
            bottom: 0.6,
            angle: 0.0,
        };
        drag(&mut c, Handle::Move, vec2(1.0, -1.0), None, 1.5);
        assert!(
            close(c.right, 1.0) && close(c.left, 0.6) && close(c.top, 0.0) && close(c.bottom, 0.4)
        );
    }

    #[test]
    fn edges_never_cross() {
        let mut c = Crop::default();
        drag(&mut c, Handle::Left, vec2(2.0, 0.0), None, 1.5);
        assert!(close(c.left, 1.0 - MIN));
    }

    #[test]
    fn locked_ratio_is_kept_in_pixels() {
        // Photo 3:2, cadre 1:1 : en fractions, largeur = hauteur × 2/3.
        let mut c = Crop::default();
        constrain(&mut c, Some(1.0), 1.5);
        assert!(close((c.right - c.left) * 1.5, c.bottom - c.top), "{c:?}");
        assert!(c.left >= 0.0 && c.right <= 1.0 && c.top >= 0.0 && c.bottom <= 1.0);
        drag(
            &mut c,
            Handle::BottomRight,
            vec2(-0.1, -0.02),
            Some(1.0),
            1.5,
        );
        assert!(close((c.right - c.left) * 1.5, c.bottom - c.top), "{c:?}");
        // « Original » : les proportions de la photo, donc toute la photo possible.
        let mut c = Crop::default();
        constrain(&mut c, Some(0.0), 1.5);
        assert!(close(c.right - c.left, 1.0) && close(c.bottom - c.top, 1.0));
    }
}
