//! L'apparence de LÜMA : une encre profonde, des îlots de verre fumé aux coins très
//! arrondis, des textes chauds, l'ambre et le spectre pour ce qui est actif. La photo
//! reste la seule grande surface de couleur.

use egui::{Color32, CornerRadius, Stroke};

use crate::widgets::theme;

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = theme::PANEL;
    v.window_fill = theme::PANEL;
    v.extreme_bg_color = Color32::from_gray(0x14);
    v.faint_bg_color = theme::SECTION;
    v.override_text_color = Some(theme::TEXT);
    v.selection.bg_fill = Color32::from_rgb(0x3a, 0x2e, 0x1c);
    v.selection.stroke = Stroke::new(1.0, theme::ACCENT);
    v.widgets.noninteractive.bg_fill = theme::PANEL;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_gray(0x30));
    v.widgets.inactive.bg_fill = theme::SECTION;
    v.widgets.inactive.weak_bg_fill = theme::SECTION;
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2c, 0x2e, 0x3a);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(0x3a, 0x3c, 0x4a);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(9);
    }
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(12.0, 5.0);
    });
}
