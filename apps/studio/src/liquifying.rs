//! La fluidité à l'écran : on glisse sur la photo pour pousser, ou on clique et glisse
//! pour gonfler ou dégonfler ; chaque geste devient un coup de pinceau.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Stroke, Ui};
use luma_project::{Warp, WarpKind};

use crate::widgets::Change;

/// L'écart minimal entre deux points enregistrés d'un même geste, en fraction du
/// rayon : assez fin pour suivre la main, sans remplir le XMP.
const MIN_STEP: f32 = 0.1;

/// Ajoute à `warp` le point `p` (fraction de l'image) s'il est assez loin du dernier.
pub fn extend(warp: &mut Warp, p: (f32, f32), aspect: f32) -> bool {
    let Some(&last) = warp.points.last() else {
        warp.points.push(p);
        return true;
    };
    // Les distances en fractions de la largeur (le rayon l'est aussi).
    let (dx, dy) = (p.0 - last.0, (p.1 - last.1) / aspect);
    if (dx * dx + dy * dy).sqrt() < MIN_STEP * warp.radius {
        return false;
    }
    warp.points.push(p);
    true
}

/// Le pinceau sur `rect` (la photo à l'écran) : un glissement ajoute un coup de `kind`
/// de rayon `radius` (fraction de la largeur) et de force `strength`.
pub fn edit(
    ui: &mut Ui,
    rect: Rect,
    warps: &mut Vec<Warp>,
    kind: WarpKind,
    radius: f32,
    strength: f32,
) -> Change {
    let mut change = Change::default();
    let to_norm = |p: Pos2| {
        (
            (p.x - rect.min.x) / rect.width(),
            (p.y - rect.min.y) / rect.height(),
        )
    };
    // La largeur rapportée à la hauteur : les fractions de hauteur en fractions de largeur.
    let aspect = rect.width() / rect.height().max(1.0);
    let id = ui.scope_id().with("fluidité");
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let drawing = id.with("geste");
    if response.drag_started()
        && let Some(p) = response.interact_pointer_pos()
    {
        warps.push(Warp::new(kind, to_norm(p), radius, strength));
        ui.data_mut(|d| d.insert_temp(drawing, true));
        // Gonfler et dégonfler agissent dès la pose.
        change.changed = kind != WarpKind::Push;
    }
    let active = ui.data(|d| d.get_temp::<bool>(drawing)).unwrap_or(false);
    if active
        && response.dragged()
        && let Some(p) = response.interact_pointer_pos()
        && let Some(w) = warps.last_mut()
        && extend(w, to_norm(p), aspect)
    {
        change.changed = true;
    }
    if active && response.drag_stopped() {
        ui.data_mut(|d| d.remove::<bool>(drawing));
        // Un simple appui pour pousser ne déplace rien : il n'est pas gardé.
        if warps
            .last()
            .is_some_and(|w| w.kind == WarpKind::Push && w.points.len() < 2)
        {
            warps.pop();
        } else {
            change.committed = true;
        }
    }
    // Un clic sans glisser : gonfler ou dégonfler sur place.
    if response.clicked()
        && kind != WarpKind::Push
        && let Some(p) = response.interact_pointer_pos()
    {
        warps.push(Warp::new(kind, to_norm(p), radius, strength));
        change.changed = true;
        change.committed = true;
    }
    if let Some(p) = response.hover_pos() {
        ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        let painter = ui.painter_at(rect);
        let r = radius * rect.width();
        painter.circle_stroke(p, r + 1.0, Stroke::new(2.5, Color32::from_black_alpha(90)));
        painter.circle_stroke(p, r, Stroke::new(1.0, Color32::from_white_alpha(190)));
        painter.circle_filled(p, 1.5, Color32::from_white_alpha(190));
    }
    change
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gesture_keeps_points_a_tenth_of_the_radius_apart() {
        let mut w = Warp::new(WarpKind::Push, (0.5, 0.5), 0.1, 50.0);
        // 0,005 : trop près du premier point (un dixième du rayon vaut 0,01).
        assert!(!extend(&mut w, (0.505, 0.5), 1.5));
        assert!(extend(&mut w, (0.52, 0.5), 1.5));
        // En hauteur, l'écart compte en fractions de largeur : 0,012 de hauteur sur une
        // photo 3:2 vaut 0,008 de largeur, trop peu.
        assert!(!extend(&mut w, (0.52, 0.512), 1.5));
        assert!(extend(&mut w, (0.52, 0.53), 1.5));
        assert_eq!(w.points.len(), 3);
    }
}
