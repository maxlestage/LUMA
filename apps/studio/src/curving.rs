//! Les courbes à points à l'écran : luminosité, rouge, vert, bleu. Un clic sur la
//! courbe ajoute un point, on le glisse pour le régler, un double‑clic le retire ; les
//! deux points des extrémités restent à leur place en largeur. Un glissement qui ne
//! part pas d'un point fait défiler le panneau : au doigt, sur iPhone et iPad, la
//! courbe ne retient pas le défilement.

use egui::{Color32, CursorIcon, Pos2, Sense, Stroke, Ui, pos2, vec2};
use luma_image::curve::Lut;

use crate::i18n::tr;
use crate::widgets::Change;

/// L'écart minimal, en niveaux (0 à 255), entre deux points voisins.
const GAP: f32 = 4.0;
/// La distance, en points d'écran, à laquelle un point se saisit.
const GRAB: f32 = 9.0;

/// Le point le plus proche de `x` à moins de `GAP` niveaux, sinon un nouveau point
/// `(x, y)` inséré à sa place dans l'ordre : son indice.
pub fn insert(points: &mut Vec<(f32, f32)>, x: f32, y: f32) -> usize {
    let (x, y) = (x.clamp(0.0, 255.0), y.clamp(0.0, 255.0));
    if let Some(i) = points.iter().position(|p| (p.0 - x).abs() < GAP) {
        return i;
    }
    let i = points.iter().position(|p| p.0 > x).unwrap_or(points.len());
    points.insert(i, (x, y));
    i
}

/// Déplace le point `i` vers `(x, y)` : les extrémités gardent leur abscisse, les
/// autres restent entre leurs voisins, à `GAP` niveaux au moins.
pub fn move_point(points: &mut [(f32, f32)], i: usize, x: f32, y: f32) {
    let n = points.len();
    if i >= n {
        return;
    }
    let x = if i == 0 || i == n - 1 {
        points[i].0
    } else {
        x.clamp(points[i - 1].0 + GAP, points[i + 1].0 - GAP)
    };
    points[i] = (x, y.clamp(0.0, 255.0));
}

/// Retire le point `i`, sauf les extrémités : `true` s'il l'a été.
pub fn remove(points: &mut Vec<(f32, f32)>, i: usize) -> bool {
    if i == 0 || i + 1 >= points.len() {
        return false;
    }
    points.remove(i);
    true
}

/// Une courbe à deux points ou moins, ou des extrémités hors des bords : la diagonale,
/// pour que l'éditeur ait toujours ses deux extrémités.
pub fn sanitize(points: &mut Vec<(f32, f32)>) {
    if points.len() < 2 || points[0].0 > 0.0 || points[points.len() - 1].0 < 255.0 {
        points.retain(|p| p.0 > 0.0 && p.0 < 255.0);
        points.insert(0, (0.0, 0.0));
        points.push((255.0, 255.0));
    }
}

/// L'éditeur de la courbe `points`, tracée en `color` ; `under` : une courbe déjà
/// appliquée avant (la paramétrique), montrée en pâle sous la courbe à points. La
/// courbe n'est réécrite qu'à un changement : une courbe lue sans ses extrémités
/// garde son rendu tant qu'on n'y touche pas.
pub fn editor(
    ui: &mut Ui,
    id: &str,
    curve_points: &mut Vec<(f32, f32)>,
    color: Color32,
    under: Option<&Lut>,
) -> Change {
    let mut change = Change::default();
    let mut points = curve_points.clone();
    sanitize(&mut points);
    let points = &mut points;
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, w * 0.75), Sense::hover());
    let to_screen = |(x, y): (f32, f32)| {
        pos2(
            rect.min.x + x / 255.0 * rect.width(),
            rect.max.y - y / 255.0 * rect.height(),
        )
    };
    let to_curve = |p: Pos2| {
        (
            ((p.x - rect.min.x) / rect.width() * 255.0).clamp(0.0, 255.0),
            ((rect.max.y - p.y) / rect.height() * 255.0).clamp(0.0, 255.0),
        )
    };
    let nearest = |p: Pos2, points: &[(f32, f32)]| {
        points
            .iter()
            .enumerate()
            .map(|(i, &q)| (i, to_screen(q).distance(p)))
            .filter(|&(_, d)| d < GRAB)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    };
    let held = ui.scope_id().with(id).with("point tenu");
    // Le glissement n'est pris que s'il part d'un point (ou en tient déjà un) ;
    // sinon il revient au panneau, qui défile.
    let pointer = ui.input(|i| i.pointer.press_origin().or(i.pointer.hover_pos()));
    let grab = ui.data(|d| d.get_temp::<usize>(held)).is_some()
        || pointer.is_some_and(|p| rect.contains(p) && nearest(p, points).is_some());
    let sense = if grab {
        Sense::click_and_drag()
    } else {
        Sense::click()
    };
    let response = ui
        .interact(rect, ui.scope_id().with(id), sense)
        .accessible_name(tr("Courbe à points"));
    if response.drag_started()
        && let Some(i) = ui
            .input(|i| i.pointer.press_origin())
            .and_then(|p| nearest(p, points))
    {
        ui.data_mut(|d| d.insert_temp(held, i));
    }
    if response.dragged()
        && let Some(i) = ui.data(|d| d.get_temp::<usize>(held))
        && let Some(p) = response.interact_pointer_pos()
    {
        let (x, y) = to_curve(p);
        let before = points.get(i).copied();
        move_point(points, i, x, y);
        change.changed |= points.get(i).copied() != before;
    }
    if response.drag_stopped() && ui.data(|d| d.get_temp::<usize>(held)).is_some() {
        ui.data_mut(|d| d.remove::<usize>(held));
        change.committed = true;
    }
    if response.double_clicked()
        && let Some(p) = response.interact_pointer_pos()
        && let Some(i) = nearest(p, points)
        && remove(points, i)
    {
        change.changed = true;
        change.committed = true;
    } else if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
        && nearest(p, points).is_none()
    {
        let (x, y) = to_curve(p);
        let n = points.len();
        insert(points, x, y);
        if points.len() != n {
            change.changed = true;
            change.committed = true;
        }
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, Color32::from_gray(0x16));
    let grid = Stroke::new(1.0, Color32::from_gray(0x2a));
    for k in 1..4 {
        let x = rect.min.x + rect.width() * k as f32 / 4.0;
        let y = rect.min.y + rect.height() * k as f32 / 4.0;
        painter.line_segment([pos2(x, rect.min.y), pos2(x, rect.max.y)], grid);
        painter.line_segment([pos2(rect.min.x, y), pos2(rect.max.x, y)], grid);
    }
    painter.line_segment(
        [rect.left_bottom(), rect.right_top()],
        Stroke::new(1.0, Color32::from_gray(0x34)),
    );
    let curve = |values: &dyn Fn(f32) -> f32| -> Vec<Pos2> {
        (0..=128)
            .map(|i| {
                let x = i as f32 / 128.0;
                to_screen((x * 255.0, values(x) * 255.0))
            })
            .collect()
    };
    if let Some(under) = under.filter(|l| !l.is_identity()) {
        painter.add(egui::Shape::line(
            curve(&|x| under.get(x)),
            Stroke::new(1.0, Color32::from_gray(0x70)),
        ));
    }
    let lut = luma_image::curve::spline(points);
    painter.add(egui::Shape::line(
        curve(&|x| lut.get(x)),
        Stroke::new(1.5, color),
    ));
    let hover = response.hover_pos().and_then(|p| nearest(p, points));
    for (i, &q) in points.iter().enumerate() {
        let c = to_screen(q);
        let r = if hover == Some(i) { 5.0 } else { 3.5 };
        painter.circle_filled(c, r + 1.0, Color32::from_black_alpha(160));
        painter.circle_filled(c, r, color);
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(if hover.is_some() {
            CursorIcon::Grab
        } else {
            CursorIcon::Crosshair
        });
    }
    if change.changed {
        *curve_points = points.clone();
    }
    change
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> Vec<(f32, f32)> {
        vec![(0.0, 0.0), (255.0, 255.0)]
    }

    #[test]
    fn points_are_inserted_in_order_and_not_twice() {
        let mut p = line();
        assert_eq!(insert(&mut p, 128.0, 150.0), 1);
        assert_eq!(insert(&mut p, 64.0, 50.0), 1);
        assert_eq!(
            p,
            vec![(0.0, 0.0), (64.0, 50.0), (128.0, 150.0), (255.0, 255.0)]
        );
        // Trop près d'un point : c'est lui qu'on saisit, rien n'est ajouté.
        assert_eq!(insert(&mut p, 130.0, 10.0), 2);
        assert_eq!(p.len(), 4);
        // Hors de la courbe : ramené dans les bornes.
        assert_eq!(insert(&mut p, 200.0, 300.0), 3);
        assert_eq!(p[3], (200.0, 255.0));
    }

    #[test]
    fn points_stay_between_their_neighbours_and_ends_stay_put() {
        let mut p = vec![(0.0, 0.0), (64.0, 50.0), (128.0, 150.0), (255.0, 255.0)];
        move_point(&mut p, 1, 200.0, 80.0);
        assert_eq!(p[1], (124.0, 80.0));
        move_point(&mut p, 0, 40.0, 30.0);
        assert_eq!(p[0], (0.0, 30.0));
        move_point(&mut p, 3, 100.0, -20.0);
        assert_eq!(p[3], (255.0, 0.0));
        move_point(&mut p, 9, 1.0, 1.0);
        assert_eq!(p.len(), 4);
    }

    #[test]
    fn only_middle_points_are_removed() {
        let mut p = vec![(0.0, 0.0), (64.0, 50.0), (255.0, 255.0)];
        assert!(!remove(&mut p, 0));
        assert!(!remove(&mut p, 2));
        assert!(remove(&mut p, 1));
        assert_eq!(p, line());
    }

    #[test]
    fn a_curve_without_its_ends_gets_them_back() {
        let mut empty = Vec::new();
        sanitize(&mut empty);
        assert_eq!(empty, line());
        let mut middle = vec![(10.0, 20.0), (128.0, 140.0)];
        sanitize(&mut middle);
        assert_eq!(
            middle,
            vec![(0.0, 0.0), (10.0, 20.0), (128.0, 140.0), (255.0, 255.0)]
        );
        // Une courbe complète ne bouge pas.
        let mut full = vec![(0.0, 10.0), (128.0, 140.0), (255.0, 250.0)];
        sanitize(&mut full);
        assert_eq!(full, vec![(0.0, 10.0), (128.0, 140.0), (255.0, 250.0)]);
    }
}
