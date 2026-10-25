//! Les préréglages : des looks prêts à l'emploi, dont les rendus de pellicules.
//!
//! Un préréglage ne touche qu'aux réglages qui font son look (courbes, couleurs,
//! étalonnage, grain…) : l'exposition, la balance des blancs, le recadrage et les
//! masques de la photo restent ceux de l'utilisateur. Son intensité (0 à 100 %) fait un
//! fondu entre les réglages actuels et le look complet.
//!
//! Les pellicules sont des interprétations : leurs noms disent l'inspiration (couleurs,
//! contraste, grain caractéristiques), pas une copie des émulsions ni des marques.

use crate::{Profile, Settings, Wheel};

/// Les familles de préréglages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    ColorFilm,
    Cinema,
    BlackAndWhiteFilm,
    Instant,
    Creative,
}

impl Group {
    pub const ALL: [Group; 5] = [
        Group::ColorFilm,
        Group::Cinema,
        Group::BlackAndWhiteFilm,
        Group::Instant,
        Group::Creative,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Group::ColorFilm => "Pellicules couleur",
            Group::Cinema => "Cinéma",
            Group::BlackAndWhiteFilm => "Pellicules noir et blanc",
            Group::Instant => "Instantanés et vintage",
            Group::Creative => "Créatifs",
        }
    }
}

/// Un préréglage intégré.
pub struct Preset {
    pub name: &'static str,
    pub group: Group,
    pub description: &'static str,
    apply: fn(&mut Settings),
}

impl Preset {
    /// Les réglages `current` avec ce look, à `amount` (0 à 1).
    pub fn applied(&self, current: &Settings, amount: f32) -> Settings {
        let mut target = current.clone();
        reset_look(&mut target);
        (self.apply)(&mut target);
        blend(current, &target, amount)
    }
}

/// Remet à zéro ce qu'un look définit, pour que deux préréglages ne s'additionnent pas.
fn reset_look(s: &mut Settings) {
    let d = Settings::default();
    s.profile = d.profile;
    s.contrast = 0.0;
    s.highlights = d.highlights;
    s.shadows = d.shadows;
    s.whites = 0.0;
    s.blacks = 0.0;
    s.texture = 0.0;
    s.clarity = 0.0;
    s.vibrance = 0.0;
    s.saturation = 0.0;
    s.tone_curve = d.tone_curve;
    s.hsl = d.hsl;
    s.color_grading = d.color_grading;
    s.dehaze = 0.0;
    s.effects.vignette_amount = 0.0;
    s.effects.vignette_midpoint = d.effects.vignette_midpoint;
    s.effects.vignette_feather = d.effects.vignette_feather;
    s.effects.vignette_roundness = d.effects.vignette_roundness;
    s.effects.grain_amount = 0.0;
    s.effects.grain_size = d.effects.grain_size;
    s.effects.grain_roughness = d.effects.grain_roughness;
}

/// Le fondu entre deux réglages : chaque nombre est interpolé ; ce qui ne s'interpole
/// pas (profil, courbes de longueurs différentes) bascule à mi‑chemin.
pub fn blend(from: &Settings, to: &Settings, amount: f32) -> Settings {
    let amount = amount.clamp(0.0, 1.0);
    if amount >= 1.0 {
        return to.clone();
    }
    if amount <= 0.0 {
        return from.clone();
    }
    fn mix(a: &serde_json::Value, b: &serde_json::Value, t: f64) -> serde_json::Value {
        use serde_json::Value as V;
        match (a, b) {
            (V::Number(x), V::Number(y)) => {
                let (x, y) = (x.as_f64().unwrap_or(0.0), y.as_f64().unwrap_or(0.0));
                serde_json::Number::from_f64(x + (y - x) * t).map_or(b.clone(), V::Number)
            }
            (V::Object(x), V::Object(y)) => V::Object(
                y.iter()
                    .map(|(k, vb)| (k.clone(), x.get(k).map_or(vb.clone(), |va| mix(va, vb, t))))
                    .collect(),
            ),
            (V::Array(x), V::Array(y)) if x.len() == y.len() => {
                V::Array(x.iter().zip(y).map(|(p, q)| mix(p, q, t)).collect())
            }
            _ if t < 0.5 => a.clone(),
            _ => b.clone(),
        }
    }
    let (Ok(a), Ok(b)) = (serde_json::to_value(from), serde_json::to_value(to)) else {
        return to.clone();
    };
    serde_json::from_value(mix(&a, &b, f64::from(amount))).unwrap_or_else(|_| to.clone())
}

fn wheel(hue: f32, saturation: f32, luminance: f32) -> Wheel {
    Wheel {
        hue,
        saturation,
        luminance,
    }
}

// Les briques des looks.

/// Contraste, saturation, vibrance.
fn base(s: &mut Settings, contrast: f32, saturation: f32, vibrance: f32) {
    s.contrast = contrast;
    s.saturation = saturation;
    s.vibrance = vibrance;
}

/// Hautes lumières, ombres, blancs, noirs.
fn tones(s: &mut Settings, highlights: f32, shadows: f32, whites: f32, blacks: f32) {
    s.highlights = highlights;
    s.shadows = shadows;
    s.whites = whites;
    s.blacks = blacks;
}

/// Virage partagé : teinte et force des ombres, puis des hautes lumières.
fn split(s: &mut Settings, shadow_hue: f32, shadow_sat: f32, highlight_hue: f32, highlight_sat: f32) {
    s.color_grading.shadows = wheel(shadow_hue, shadow_sat, 0.0);
    s.color_grading.highlights = wheel(highlight_hue, highlight_sat, 0.0);
}

/// Un voile de couleur sur toute l'image.
fn tint(s: &mut Settings, hue: f32, saturation: f32) {
    s.color_grading.global = wheel(hue, saturation, 0.0);
}

/// Les tons moyens.
fn mids(s: &mut Settings, hue: f32, saturation: f32) {
    s.color_grading.midtones = wheel(hue, saturation, 0.0);
}

/// Une couleur du mélangeur : teinte, saturation, luminance.
fn band(s: &mut Settings, i: usize, hue: f32, saturation: f32, luminance: f32) {
    s.hsl.hue[i] = hue;
    s.hsl.saturation[i] = saturation;
    s.hsl.luminance[i] = luminance;
}

/// Des noirs « délavés » et des blancs adoucis, comme un tirage argentique.
fn fade(s: &mut Settings, black: f32, white: f32) {
    s.tone_curve.point = vec![
        (0.0, black),
        (64.0, 62.0 + black * 0.3),
        (128.0, 128.0),
        (192.0, 194.0),
        (255.0, white),
    ];
}

/// Une courbe en S (contraste de tirage) avec noirs et blancs donnés.
fn s_curve(s: &mut Settings, black: f32, depth: f32, white: f32) {
    s.tone_curve.point = vec![
        (0.0, black),
        (64.0, 64.0 - depth),
        (128.0, 128.0),
        (192.0, 192.0 + depth),
        (255.0, white),
    ];
}

/// Les courbes rouge, verte, bleue : (noir, blanc) de chaque couche, sur 0–255.
fn channels(s: &mut Settings, r: (f32, f32), g: (f32, f32), b: (f32, f32)) {
    let c = |(lo, hi): (f32, f32)| vec![(0.0, lo), (255.0, hi)];
    s.tone_curve.red = c(r);
    s.tone_curve.green = c(g);
    s.tone_curve.blue = c(b);
}

fn grain(s: &mut Settings, amount: f32, size: f32, roughness: f32) {
    s.effects.grain_amount = amount;
    s.effects.grain_size = size;
    s.effects.grain_roughness = roughness;
}

fn vignette(s: &mut Settings, amount: f32, midpoint: f32) {
    s.effects.vignette_amount = amount;
    s.effects.vignette_midpoint = midpoint;
}

fn mono(s: &mut Settings) {
    s.profile = Profile::Monochrome;
}

// Ordre du mélangeur : rouge, orange, jaune, vert, bleu‑vert, bleu, violet, magenta.
const R: usize = 0;
const O: usize = 1;
const Y: usize = 2;
const G: usize = 3;
const A: usize = 4;
const B: usize = 5;
const P: usize = 6;
const M: usize = 7;

macro_rules! preset {
    ($name:expr, $group:ident, $desc:expr, |$s:ident| $body:block) => {
        Preset {
            name: $name,
            group: Group::$group,
            description: $desc,
            apply: |$s: &mut Settings| $body,
        }
    };
}

/// Les 72 préréglages intégrés.
pub const PRESETS: &[Preset] = &[
    // ----- Pellicules couleur -----
    preset!(
        "Portra 160",
        ColorFilm,
        "Inspiré de Kodak Portra 160 : peaux naturelles, couleurs douces et fines.",
        |s| {
            base(s, -8.0, -10.0, 4.0);
            tones(s, -10.0, 5.0, 0.0, 0.0);
            fade(s, 8.0, 250.0);
            band(s, O, 0.0, -4.0, 6.0);
            band(s, G, 10.0, -15.0, 0.0);
            split(s, 210.0, 5.0, 45.0, 8.0);
            grain(s, 10.0, 18.0, 35.0);
        }
    ),
    preset!(
        "Portra 400",
        ColorFilm,
        "Inspiré de Kodak Portra 400 : peaux chaudes, contraste doux, noirs délavés.",
        |s| {
            base(s, -12.0, -8.0, 6.0);
            tones(s, -15.0, 0.0, 0.0, 0.0);
            fade(s, 12.0, 248.0);
            band(s, O, 0.0, -6.0, 6.0);
            band(s, G, 12.0, -18.0, 0.0);
            band(s, B, 0.0, -12.0, 0.0);
            split(s, 200.0, 8.0, 45.0, 12.0);
            grain(s, 18.0, 25.0, 45.0);
        }
    ),
    preset!(
        "Portra 800",
        ColorFilm,
        "Inspiré de Kodak Portra 800 : plus chaud et plus dense, grain visible, lumière tardive.",
        |s| {
            base(s, -4.0, -4.0, 8.0);
            tones(s, -12.0, 6.0, 0.0, -4.0);
            fade(s, 14.0, 246.0);
            band(s, O, 0.0, 0.0, 4.0);
            band(s, G, 15.0, -15.0, 0.0);
            split(s, 190.0, 10.0, 40.0, 16.0);
            grain(s, 28.0, 32.0, 55.0);
        }
    ),
    preset!(
        "Ektar 100",
        ColorFilm,
        "Inspiré de Kodak Ektar 100 : couleurs franches, rouges et bleus profonds, grain très fin.",
        |s| {
            base(s, 15.0, 12.0, 10.0);
            band(s, R, 0.0, 10.0, 0.0);
            band(s, B, 0.0, 15.0, -10.0);
            fade(s, 4.0, 252.0);
            grain(s, 6.0, 15.0, 40.0);
        }
    ),
    preset!(
        "Gold 200",
        ColorFilm,
        "Inspiré de Kodak Gold 200 : lumière dorée, jaunes généreux, ciels adoucis.",
        |s| {
            base(s, 5.0, 0.0, 8.0);
            band(s, Y, 0.0, 15.0, 0.0);
            band(s, B, -10.0, -15.0, 0.0);
            fade(s, 10.0, 246.0);
            tint(s, 50.0, 12.0);
            split(s, 0.0, 0.0, 45.0, 18.0);
            grain(s, 22.0, 30.0, 50.0);
        }
    ),
    preset!(
        "ColorPlus 200",
        ColorFilm,
        "Inspiré de Kodak ColorPlus 200 : pellicule bon marché, chaleur et contraste modeste.",
        |s| {
            base(s, 2.0, 4.0, 6.0);
            band(s, Y, -4.0, 10.0, 0.0);
            band(s, G, 8.0, -8.0, 0.0);
            fade(s, 12.0, 244.0);
            tint(s, 40.0, 10.0);
            grain(s, 26.0, 32.0, 55.0);
        }
    ),
    preset!(
        "UltraMax 400",
        ColorFilm,
        "Inspiré de Kodak UltraMax 400 : saturé, chaud, contrasté, la pellicule des vacances.",
        |s| {
            base(s, 14.0, 12.0, 6.0);
            band(s, R, 0.0, 8.0, 0.0);
            band(s, Y, 0.0, 12.0, 0.0);
            split(s, 220.0, 6.0, 40.0, 14.0);
            fade(s, 6.0, 250.0);
            grain(s, 26.0, 30.0, 55.0);
        }
    ),
    preset!(
        "Fujicolor C200",
        ColorFilm,
        "Inspiré de Fujicolor C200 : verts frais, ombres froides, rendu clair.",
        |s| {
            base(s, 4.0, 2.0, 6.0);
            band(s, G, -6.0, 10.0, 4.0);
            band(s, B, -5.0, 6.0, 0.0);
            split(s, 170.0, 10.0, 60.0, 6.0);
            fade(s, 10.0, 248.0);
            grain(s, 20.0, 28.0, 50.0);
        }
    ),
    preset!(
        "Superia 400",
        ColorFilm,
        "Inspiré de Fujifilm Superia 400 : pellicule du quotidien, ombres verdâtres, grain présent.",
        |s| {
            base(s, 8.0, 4.0, 0.0);
            fade(s, 8.0, 250.0);
            band(s, G, 0.0, 10.0, 0.0);
            split(s, 140.0, 12.0, 60.0, 6.0);
            grain(s, 26.0, 30.0, 55.0);
        }
    ),
    preset!(
        "Superia X-TRA 800",
        ColorFilm,
        "Inspiré de Fujifilm Superia X-TRA 800 : nuit et intérieur, cyan dans les ombres, gros grain.",
        |s| {
            base(s, 10.0, 6.0, 4.0);
            split(s, 175.0, 16.0, 50.0, 8.0);
            fade(s, 14.0, 246.0);
            grain(s, 36.0, 40.0, 60.0);
        }
    ),
    preset!(
        "Pro 400H",
        ColorFilm,
        "Inspiré de Fujifilm Pro 400H : pastels lumineux, verts menthe, ombres froides.",
        |s| {
            base(s, -15.0, -10.0, 0.0);
            tones(s, 0.0, 10.0, 0.0, 0.0);
            fade(s, 16.0, 246.0);
            band(s, G, 15.0, -10.0, 0.0);
            band(s, B, -10.0, 0.0, 0.0);
            split(s, 160.0, 10.0, 200.0, 5.0);
            grain(s, 18.0, 25.0, 45.0);
        }
    ),
    preset!(
        "Velvia 50",
        ColorFilm,
        "Inspiré de Fujifilm Velvia 50 : diapositive de paysage, verts et bleus intenses, ombres denses.",
        |s| {
            base(s, 25.0, 20.0, 15.0);
            tones(s, 0.0, 0.0, 0.0, -10.0);
            band(s, G, -5.0, 15.0, 0.0);
            band(s, B, 0.0, 20.0, -15.0);
            split(s, 230.0, 10.0, 0.0, 0.0);
            grain(s, 5.0, 15.0, 30.0);
        }
    ),
    preset!(
        "Velvia 100",
        ColorFilm,
        "Inspiré de Fujifilm Velvia 100 : saturé mais plus neutre que la 50, magentas tenus.",
        |s| {
            base(s, 20.0, 15.0, 12.0);
            band(s, G, 0.0, 10.0, 0.0);
            band(s, M, 0.0, -10.0, 0.0);
            band(s, B, 0.0, 12.0, -8.0);
            grain(s, 5.0, 15.0, 30.0);
        }
    ),
    preset!(
        "Provia 100F",
        ColorFilm,
        "Inspiré de Fujifilm Provia 100F : diapositive fidèle, contraste net, couleurs propres.",
        |s| {
            base(s, 10.0, 8.0, 5.0);
            grain(s, 6.0, 15.0, 30.0);
        }
    ),
    preset!(
        "Astia 100F",
        ColorFilm,
        "Inspiré de Fujifilm Astia 100F : diapositive douce pour le portrait, peaux délicates.",
        |s| {
            base(s, -2.0, 0.0, 4.0);
            tones(s, -10.0, 6.0, 0.0, 0.0);
            band(s, O, 0.0, -8.0, 6.0);
            band(s, B, 0.0, 6.0, 0.0);
            grain(s, 5.0, 15.0, 30.0);
        }
    ),
    preset!(
        "Kodachrome 64",
        ColorFilm,
        "Inspiré de Kodachrome 64 : rouges riches, bleus profonds, contraste marqué.",
        |s| {
            base(s, 20.0, 6.0, 0.0);
            band(s, R, 0.0, 15.0, -5.0);
            band(s, Y, -5.0, 0.0, 0.0);
            band(s, B, 0.0, 10.0, -15.0);
            split(s, 0.0, 0.0, 40.0, 8.0);
            grain(s, 10.0, 20.0, 40.0);
        }
    ),
    preset!(
        "Kodachrome 25",
        ColorFilm,
        "Inspiré de Kodachrome 25 : la plus fine, couleurs denses et chaudes d'autrefois.",
        |s| {
            base(s, 22.0, 4.0, 0.0);
            tones(s, -8.0, 0.0, 0.0, -6.0);
            band(s, R, 0.0, 12.0, -6.0);
            band(s, G, -8.0, -5.0, -5.0);
            band(s, B, 0.0, 6.0, -18.0);
            tint(s, 35.0, 6.0);
            grain(s, 6.0, 12.0, 30.0);
        }
    ),
    preset!(
        "Ektachrome E100",
        ColorFilm,
        "Inspiré de Kodak Ektachrome E100 : diapositive claire et froide, blancs purs.",
        |s| {
            base(s, 15.0, 10.0, 0.0);
            band(s, B, 0.0, 10.0, 0.0);
            split(s, 220.0, 10.0, 0.0, 0.0);
            grain(s, 6.0, 15.0, 30.0);
        }
    ),
    preset!(
        "Elite Chrome",
        ColorFilm,
        "Inspiré de Kodak Elite Chrome : diapositive grand public, ciels cyan, contraste vif.",
        |s| {
            base(s, 16.0, 12.0, 4.0);
            band(s, B, -8.0, 12.0, -6.0);
            band(s, A, 0.0, 10.0, 0.0);
            grain(s, 8.0, 18.0, 35.0);
        }
    ),
    preset!(
        "Agfa Vista 200",
        ColorFilm,
        "Inspiré d'Agfa Vista 200 : rouges chauds, ciels très bleus, ombres un peu froides.",
        |s| {
            base(s, 10.0, 10.0, 6.0);
            band(s, R, 0.0, 15.0, 0.0);
            band(s, B, 0.0, 15.0, -6.0);
            split(s, 210.0, 10.0, 30.0, 10.0);
            fade(s, 8.0, 250.0);
            grain(s, 22.0, 28.0, 50.0);
        }
    ),
    preset!(
        "Agfa Ultra 100",
        ColorFilm,
        "Inspiré d'Agfa Ultra 100 : saturation extrême, couleurs presque irréelles.",
        |s| {
            base(s, 18.0, 28.0, 10.0);
            band(s, R, 0.0, 10.0, 0.0);
            band(s, G, 0.0, 10.0, 0.0);
            band(s, B, 0.0, 15.0, -10.0);
            grain(s, 10.0, 18.0, 35.0);
        }
    ),
    preset!(
        "Lomography 800",
        ColorFilm,
        "Inspiré de Lomography Color 800 : rouges électriques, ombres sales, gros grain.",
        |s| {
            base(s, 18.0, 16.0, 0.0);
            band(s, R, 4.0, 15.0, 0.0);
            split(s, 160.0, 14.0, 30.0, 12.0);
            fade(s, 14.0, 244.0);
            vignette(s, -15.0, 40.0);
            grain(s, 38.0, 40.0, 65.0);
        }
    ),
    // ----- Cinéma -----
    preset!(
        "CineStill 800T",
        Cinema,
        "Inspiré de CineStill 800T : film de cinéma tungstène, nuits bleutées, lumières orangées.",
        |s| {
            base(s, 10.0, 0.0, 0.0);
            tones(s, -10.0, 0.0, 0.0, 0.0);
            fade(s, 10.0, 245.0);
            band(s, R, 8.0, 10.0, 0.0);
            band(s, A, -10.0, 0.0, 0.0);
            split(s, 200.0, 25.0, 30.0, 15.0);
            grain(s, 30.0, 40.0, 50.0);
        }
    ),
    preset!(
        "CineStill 400D",
        Cinema,
        "Inspiré de CineStill 400D : film lumière du jour, chaud et doux, ombres teintées de cyan.",
        |s| {
            base(s, -4.0, 4.0, 8.0);
            tones(s, -12.0, 6.0, 0.0, 0.0);
            fade(s, 10.0, 246.0);
            split(s, 185.0, 12.0, 38.0, 14.0);
            band(s, G, 10.0, -10.0, 0.0);
            grain(s, 24.0, 32.0, 50.0);
        }
    ),
    preset!(
        "CineStill 50D",
        Cinema,
        "Inspiré de CineStill 50D : fin et lumineux, ciels cyan, peaux claires.",
        |s| {
            base(s, 6.0, 6.0, 8.0);
            band(s, B, -8.0, 8.0, 0.0);
            band(s, O, 0.0, -4.0, 6.0);
            split(s, 190.0, 8.0, 40.0, 8.0);
            grain(s, 8.0, 15.0, 35.0);
        }
    ),
    preset!(
        "Vision3 50D",
        Cinema,
        "Inspiré de Kodak Vision3 50D : négatif cinéma de jour, très fin, couleurs justes et douces.",
        |s| {
            base(s, -6.0, -2.0, 4.0);
            tones(s, -10.0, 4.0, 0.0, 0.0);
            s_curve(s, 6.0, 4.0, 250.0);
            split(s, 200.0, 6.0, 40.0, 8.0);
            grain(s, 6.0, 14.0, 30.0);
        }
    ),
    preset!(
        "Vision3 250D",
        Cinema,
        "Inspiré de Kodak Vision3 250D : le négatif des longs métrages en extérieur, large et naturel.",
        |s| {
            base(s, -4.0, 0.0, 6.0);
            tones(s, -12.0, 6.0, 0.0, 0.0);
            s_curve(s, 8.0, 6.0, 248.0);
            split(s, 195.0, 10.0, 38.0, 10.0);
            grain(s, 14.0, 22.0, 40.0);
        }
    ),
    preset!(
        "Vision3 200T",
        Cinema,
        "Inspiré de Kodak Vision3 200T : tungstène fin, intérieurs ambrés, ombres sarcelle.",
        |s| {
            base(s, 6.0, 0.0, 4.0);
            s_curve(s, 8.0, 6.0, 248.0);
            split(s, 190.0, 18.0, 35.0, 14.0);
            band(s, O, 0.0, 6.0, 0.0);
            grain(s, 16.0, 24.0, 40.0);
        }
    ),
    preset!(
        "Vision3 500T",
        Cinema,
        "Inspiré de Kodak Vision3 500T : le négatif des nuits de cinéma, noirs profonds, néons.",
        |s| {
            base(s, 12.0, 4.0, 6.0);
            tones(s, -14.0, 4.0, 0.0, -6.0);
            s_curve(s, 6.0, 8.0, 248.0);
            split(s, 205.0, 22.0, 30.0, 16.0);
            mids(s, 30.0, 6.0);
            grain(s, 26.0, 34.0, 50.0);
        }
    ),
    preset!(
        "Kodak 2383",
        Cinema,
        "Inspiré du film de tirage Kodak 2383 : le rendu des salles, contraste riche, sarcelle et ambre.",
        |s| {
            base(s, 22.0, 6.0, 0.0);
            tones(s, -10.0, 0.0, 0.0, -10.0);
            s_curve(s, 4.0, 10.0, 246.0);
            split(s, 190.0, 18.0, 38.0, 16.0);
            band(s, O, 0.0, 8.0, 0.0);
            band(s, G, 15.0, -12.0, 0.0);
            grain(s, 10.0, 20.0, 40.0);
        }
    ),
    preset!(
        "Kodak 2393",
        Cinema,
        "Inspiré du film de tirage Kodak 2393 : plus neutre et plus doux que le 2383.",
        |s| {
            base(s, 14.0, 2.0, 0.0);
            s_curve(s, 6.0, 7.0, 248.0);
            split(s, 200.0, 10.0, 40.0, 10.0);
            grain(s, 8.0, 18.0, 35.0);
        }
    ),
    preset!(
        "Fuji 3513",
        Cinema,
        "Inspiré du film de tirage Fujifilm 3513 : verts denses, ombres froides, peaux rosées.",
        |s| {
            base(s, 18.0, 4.0, 0.0);
            s_curve(s, 4.0, 8.0, 248.0);
            band(s, G, -6.0, 8.0, -6.0);
            band(s, O, -4.0, 0.0, 4.0);
            split(s, 175.0, 14.0, 20.0, 8.0);
            grain(s, 8.0, 18.0, 35.0);
        }
    ),
    preset!(
        "Eterna 250D",
        Cinema,
        "Inspiré de Fujifilm Eterna : couleurs retenues, contraste bas, élégance du cinéma japonais.",
        |s| {
            base(s, -18.0, -22.0, 0.0);
            tones(s, -10.0, 12.0, 0.0, 0.0);
            fade(s, 10.0, 244.0);
            band(s, G, 8.0, -15.0, 0.0);
            split(s, 180.0, 8.0, 50.0, 4.0);
            grain(s, 12.0, 20.0, 40.0);
        }
    ),
    preset!(
        "Eterna Vivid 500",
        Cinema,
        "Inspiré de Fujifilm Eterna Vivid 500 : la variante vive d'Eterna, verts et bleus francs.",
        |s| {
            base(s, -2.0, 4.0, 10.0);
            tones(s, -8.0, 8.0, 0.0, 0.0);
            band(s, G, 0.0, 6.0, 0.0);
            band(s, B, 0.0, 8.0, 0.0);
            split(s, 190.0, 8.0, 45.0, 6.0);
            grain(s, 20.0, 28.0, 45.0);
        }
    ),
    preset!(
        "Technicolor 2 bandes",
        Cinema,
        "Le Technicolor des années 1920 : rouges et cyans seuls, pas de vrai bleu ni de vert.",
        |s| {
            base(s, 14.0, -10.0, 0.0);
            channels(s, (6.0, 255.0), (10.0, 236.0), (20.0, 200.0));
            band(s, B, -40.0, -40.0, 0.0);
            band(s, G, 40.0, -30.0, 0.0);
            band(s, R, 0.0, 15.0, 0.0);
            grain(s, 16.0, 24.0, 45.0);
        }
    ),
    preset!(
        "Technicolor 3 bandes",
        Cinema,
        "Le Technicolor des années 1940 : couleurs saturées et lumineuses, rouges éclatants.",
        |s| {
            base(s, 18.0, 25.0, 10.0);
            band(s, R, 0.0, 18.0, 0.0);
            band(s, Y, 0.0, 10.0, 6.0);
            band(s, B, 0.0, 15.0, -6.0);
            s_curve(s, 4.0, 6.0, 250.0);
            grain(s, 10.0, 20.0, 40.0);
        }
    ),
    preset!(
        "Blanchiment sauté",
        Cinema,
        "Le traitement sans blanchiment : argent gardé, couleurs éteintes, contraste dur.",
        |s| {
            base(s, 40.0, -55.0, -10.0);
            tones(s, -10.0, -10.0, 0.0, -15.0);
            s.clarity = 20.0;
            s_curve(s, 0.0, 12.0, 255.0);
            grain(s, 18.0, 25.0, 55.0);
        }
    ),
    preset!(
        "Traitement croisé",
        Cinema,
        "Une diapositive développée comme un négatif : jaunes et verts, ombres bleues, contraste fort.",
        |s| {
            base(s, 25.0, 15.0, 0.0);
            channels(s, (0.0, 255.0), (10.0, 250.0), (40.0, 190.0));
            band(s, Y, 0.0, 15.0, 0.0);
            split(s, 220.0, 20.0, 70.0, 20.0);
            grain(s, 18.0, 25.0, 50.0);
        }
    ),
    preset!(
        "Nuit américaine",
        Cinema,
        "Tourner de jour pour faire la nuit : sous‑exposé, bleu, peu saturé.",
        |s| {
            base(s, 10.0, -35.0, 0.0);
            tones(s, -40.0, -20.0, -30.0, -10.0);
            tint(s, 220.0, 35.0);
            // Assombrie par la courbe : l'exposition de la photo reste à l'utilisateur.
            s.tone_curve.point = vec![(0.0, 0.0), (128.0, 70.0), (255.0, 170.0)];
            grain(s, 14.0, 22.0, 45.0);
        }
    ),
    preset!(
        "Blockbuster",
        Cinema,
        "Le contraste de couleurs des grands films d'action : ombres sarcelle, peaux orangées.",
        |s| {
            base(s, 20.0, 0.0, 10.0);
            split(s, 185.0, 35.0, 35.0, 25.0);
            band(s, G, 30.0, -25.0, 0.0);
            band(s, B, -10.0, 10.0, 0.0);
            s.clarity = 10.0;
            grain(s, 6.0, 15.0, 30.0);
        }
    ),
    preset!(
        "Néo-noir",
        Cinema,
        "Nuits de néon : magentas et cyans, noirs profonds.",
        |s| {
            base(s, 18.0, 6.0, 15.0);
            tones(s, -10.0, -5.0, 0.0, -10.0);
            split(s, 200.0, 30.0, 320.0, 25.0);
            band(s, P, 0.0, 15.0, 0.0);
            band(s, M, 0.0, 15.0, 0.0);
            grain(s, 16.0, 24.0, 45.0);
        }
    ),
    preset!(
        "Désert",
        Cinema,
        "Lumière écrasante des films de désert : orange brûlé, ciels pâles, contraste dur.",
        |s| {
            base(s, 22.0, 6.0, 0.0);
            tint(s, 32.0, 25.0);
            band(s, B, -10.0, -30.0, 10.0);
            band(s, O, 0.0, 15.0, 0.0);
            s.clarity = 15.0;
            grain(s, 14.0, 22.0, 50.0);
        }
    ),
    preset!(
        "Code vert",
        Cinema,
        "Le voile vert des thrillers numériques de 1999 : froid, contrasté, désaturé.",
        |s| {
            base(s, 20.0, -25.0, 0.0);
            tint(s, 130.0, 22.0);
            split(s, 150.0, 15.0, 90.0, 8.0);
            grain(s, 8.0, 18.0, 35.0);
        }
    ),
    preset!(
        "Super 8",
        Cinema,
        "Inspiré de l'Ektachrome 100D en Super 8 : chaud, contrasté, gros grain, coins sombres.",
        |s| {
            base(s, 20.0, 15.0, 0.0);
            tint(s, 40.0, 12.0);
            fade(s, 12.0, 240.0);
            vignette(s, -30.0, 35.0);
            grain(s, 45.0, 55.0, 70.0);
        }
    ),
    // ----- Noir et blanc -----
    preset!(
        "Tri-X 400",
        BlackAndWhiteFilm,
        "Inspiré de Kodak Tri-X 400 : noir et blanc contrasté du reportage, grain franc.",
        |s| {
            mono(s);
            base(s, 25.0, 0.0, 0.0);
            s.clarity = 10.0;
            band(s, R, 0.0, 0.0, 5.0);
            band(s, B, 0.0, 0.0, -10.0);
            s.tone_curve.point = vec![(0.0, 6.0), (64.0, 54.0), (128.0, 128.0), (192.0, 204.0), (255.0, 250.0)];
            grain(s, 35.0, 30.0, 60.0);
        }
    ),
    preset!(
        "T-Max 400",
        BlackAndWhiteFilm,
        "Inspiré de Kodak T-Max 400 : grain tabulaire fin, gris très nuancés.",
        |s| {
            mono(s);
            base(s, 12.0, 0.0, 0.0);
            s.texture = 6.0;
            grain(s, 16.0, 18.0, 35.0);
        }
    ),
    preset!(
        "HP5 Plus",
        BlackAndWhiteFilm,
        "Inspiré d'Ilford HP5 Plus : gris doux et nuancés, grain souple.",
        |s| {
            mono(s);
            base(s, 8.0, 0.0, 0.0);
            fade(s, 10.0, 248.0);
            grain(s, 30.0, 30.0, 45.0);
        }
    ),
    preset!(
        "FP4 Plus",
        BlackAndWhiteFilm,
        "Inspiré d'Ilford FP4 Plus : classique et lumineux, grain discret.",
        |s| {
            mono(s);
            base(s, 10.0, 0.0, 0.0);
            tones(s, -8.0, 6.0, 0.0, 0.0);
            grain(s, 12.0, 16.0, 35.0);
        }
    ),
    preset!(
        "Delta 100",
        BlackAndWhiteFilm,
        "Inspiré d'Ilford Delta 100 : noir et blanc fin et précis, presque sans grain.",
        |s| {
            mono(s);
            base(s, 6.0, 0.0, 0.0);
            s.texture = 8.0;
            grain(s, 8.0, 12.0, 30.0);
        }
    ),
    preset!(
        "Delta 3200",
        BlackAndWhiteFilm,
        "Inspiré d'Ilford Delta 3200 : la nuit en noir et blanc, grain énorme, noirs bouchés.",
        |s| {
            mono(s);
            base(s, 30.0, 0.0, 0.0);
            tones(s, 0.0, 0.0, 0.0, -15.0);
            grain(s, 60.0, 50.0, 70.0);
        }
    ),
    preset!(
        "Pan F Plus 50",
        BlackAndWhiteFilm,
        "Inspiré d'Ilford Pan F Plus 50 : extrêmement fin, contraste élevé, noirs denses.",
        |s| {
            mono(s);
            base(s, 22.0, 0.0, 0.0);
            s_curve(s, 0.0, 8.0, 255.0);
            grain(s, 4.0, 10.0, 25.0);
        }
    ),
    preset!(
        "Acros 100",
        BlackAndWhiteFilm,
        "Inspiré de Fujifilm Acros 100 : ciels sombres (filtre rouge léger), peaux claires.",
        |s| {
            mono(s);
            base(s, 12.0, 0.0, 0.0);
            band(s, R, 0.0, 0.0, 12.0);
            band(s, O, 0.0, 0.0, 10.0);
            band(s, B, 0.0, 0.0, -25.0);
            band(s, A, 0.0, 0.0, -15.0);
            grain(s, 10.0, 15.0, 35.0);
        }
    ),
    preset!(
        "Neopan 1600",
        BlackAndWhiteFilm,
        "Inspiré de Fujifilm Neopan 1600 : rues de nuit, contraste fort, grain serré.",
        |s| {
            mono(s);
            base(s, 28.0, 0.0, 0.0);
            s.clarity = 12.0;
            grain(s, 45.0, 35.0, 60.0);
        }
    ),
    preset!(
        "Double-X 5222",
        BlackAndWhiteFilm,
        "Inspiré du Kodak Double-X 5222, le noir et blanc du cinéma : gris profonds, éclat argentique.",
        |s| {
            mono(s);
            base(s, 18.0, 0.0, 0.0);
            tones(s, -10.0, 0.0, 0.0, -8.0);
            s_curve(s, 4.0, 8.0, 250.0);
            grain(s, 26.0, 28.0, 50.0);
        }
    ),
    preset!(
        "Fomapan 400",
        BlackAndWhiteFilm,
        "Inspiré de Fomapan 400 : rendu d'Europe de l'Est, ombres ouvertes, grain irrégulier.",
        |s| {
            mono(s);
            base(s, 4.0, 0.0, 0.0);
            tones(s, 0.0, 15.0, 0.0, 0.0);
            fade(s, 14.0, 246.0);
            grain(s, 38.0, 40.0, 75.0);
        }
    ),
    preset!(
        "Infrarouge",
        BlackAndWhiteFilm,
        "Inspiré du film infrarouge : feuillages blancs, ciels presque noirs, halo léger.",
        |s| {
            mono(s);
            base(s, 25.0, 0.0, 0.0);
            band(s, G, 0.0, 0.0, 70.0);
            band(s, Y, 0.0, 0.0, 50.0);
            band(s, B, 0.0, 0.0, -60.0);
            band(s, A, 0.0, 0.0, -40.0);
            grain(s, 25.0, 30.0, 55.0);
        }
    ),
    // ----- Instantanés et vintage -----
    preset!(
        "Polaroid 600",
        Instant,
        "L'instantané carré des années 80 : chaud, délavé, ombres vert‑cyan.",
        |s| {
            base(s, -10.0, -10.0, 0.0);
            fade(s, 30.0, 236.0);
            split(s, 165.0, 15.0, 45.0, 15.0);
            vignette(s, -15.0, 45.0);
            grain(s, 10.0, 25.0, 40.0);
        }
    ),
    preset!(
        "Polaroid SX-70",
        Instant,
        "Le Polaroid d'origine : tons crème, contraste bas, couleurs rêveuses.",
        |s| {
            base(s, -18.0, -20.0, 0.0);
            fade(s, 36.0, 232.0);
            tint(s, 45.0, 15.0);
            split(s, 140.0, 10.0, 40.0, 10.0);
            vignette(s, -20.0, 40.0);
        }
    ),
    preset!(
        "Polaroid noir et blanc",
        Instant,
        "L'instantané noir et blanc : gris laiteux, noirs adoucis.",
        |s| {
            mono(s);
            base(s, -6.0, 0.0, 0.0);
            fade(s, 28.0, 238.0);
            vignette(s, -18.0, 45.0);
        }
    ),
    preset!(
        "Instax",
        Instant,
        "L'instantané d'aujourd'hui : clair, frais, légère dominante cyan.",
        |s| {
            base(s, 6.0, 4.0, 6.0);
            tones(s, -10.0, 10.0, 5.0, 0.0);
            fade(s, 14.0, 246.0);
            split(s, 185.0, 10.0, 60.0, 5.0);
        }
    ),
    preset!(
        "Appareil jetable",
        Instant,
        "Le jetable au flash : couleurs criardes, centre éclatant, coins sombres.",
        |s| {
            base(s, 20.0, 14.0, 0.0);
            tint(s, 60.0, 8.0);
            vignette(s, -35.0, 30.0);
            grain(s, 30.0, 35.0, 60.0);
        }
    ),
    preset!(
        "Lomo LC-A",
        Instant,
        "L'appareil russe culte : couleurs saturées, vignetage fort, contraste poussé.",
        |s| {
            base(s, 25.0, 22.0, 0.0);
            split(s, 220.0, 15.0, 50.0, 10.0);
            vignette(s, -55.0, 25.0);
            grain(s, 25.0, 30.0, 55.0);
        }
    ),
    preset!(
        "Holga",
        Instant,
        "L'appareil en plastique : flou des bords, vignetage énorme, couleurs imprévues.",
        |s| {
            base(s, 18.0, 8.0, 0.0);
            split(s, 170.0, 15.0, 40.0, 15.0);
            vignette(s, -70.0, 20.0);
            fade(s, 12.0, 240.0);
            grain(s, 35.0, 45.0, 65.0);
        }
    ),
    preset!(
        "Pellicule périmée",
        Instant,
        "Un film oublié dans un tiroir : dominante magenta, noirs délavés, grain lourd.",
        |s| {
            base(s, -6.0, -15.0, 0.0);
            fade(s, 34.0, 230.0);
            tint(s, 330.0, 15.0);
            split(s, 170.0, 10.0, 40.0, 10.0);
            grain(s, 45.0, 45.0, 70.0);
        }
    ),
    preset!(
        "Années 70",
        Instant,
        "Les tirages qui ont jauni : orange chaud, bleus éteints, contraste fatigué.",
        |s| {
            base(s, -8.0, -10.0, 0.0);
            fade(s, 22.0, 236.0);
            tint(s, 38.0, 20.0);
            band(s, B, 0.0, -35.0, 0.0);
            grain(s, 20.0, 30.0, 50.0);
        }
    ),
    preset!("Sépia", Instant, "Le virage sépia des tirages anciens.", |s| {
        base(s, 6.0, -100.0, 0.0);
        fade(s, 14.0, 242.0);
        tint(s, 35.0, 30.0);
        grain(s, 18.0, 25.0, 45.0);
    }),
    preset!(
        "Cyanotype",
        Instant,
        "Le tirage au bleu de Prusse : monochrome cyan profond.",
        |s| {
            base(s, 12.0, -100.0, 0.0);
            tint(s, 205.0, 55.0);
            fade(s, 10.0, 244.0);
            grain(s, 14.0, 22.0, 45.0);
        }
    ),
    preset!(
        "Daguerréotype",
        Instant,
        "La plaque argentée des premières photos : gris métallique, bords sombres.",
        |s| {
            mono(s);
            base(s, -10.0, 0.0, 0.0);
            fade(s, 26.0, 228.0);
            vignette(s, -50.0, 30.0);
            grain(s, 30.0, 40.0, 70.0);
        }
    ),
    // ----- Créatifs -----
    preset!(
        "Fondu mat",
        Creative,
        "Noirs relevés, couleurs retenues : un rendu mat et calme.",
        |s| {
            base(s, -10.0, -15.0, 0.0);
            fade(s, 32.0, 236.0);
        }
    ),
    preset!(
        "Sarcelle et orange",
        Creative,
        "Le contraste de couleurs du cinéma, en plus discret.",
        |s| {
            base(s, 12.0, 0.0, 10.0);
            split(s, 190.0, 30.0, 35.0, 22.0);
            band(s, G, 25.0, -20.0, 0.0);
        }
    ),
    preset!("Haute clé", Creative, "Clair, aérien, ombres ouvertes.", |s| {
        base(s, -20.0, -10.0, 0.0);
        tones(s, -10.0, 40.0, 15.0, 0.0);
    }),
    preset!(
        "Noir dramatique",
        Creative,
        "Noir et blanc dense, clarté forte, ciels sombres.",
        |s| {
            mono(s);
            base(s, 40.0, 0.0, 0.0);
            s.clarity = 30.0;
            tones(s, 0.0, 0.0, 0.0, -20.0);
            band(s, B, 0.0, 0.0, -40.0);
            band(s, A, 0.0, 0.0, -30.0);
        }
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seventy_two_presets_with_unique_names_in_every_family() {
        assert_eq!(PRESETS.len(), 72);
        let mut names: Vec<&str> = PRESETS.iter().map(|p| p.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 72, "noms en double");
        for g in Group::ALL {
            assert!(PRESETS.iter().any(|p| p.group == g), "{}", g.name());
        }
        assert!(PRESETS.iter().filter(|p| p.group == Group::Cinema).count() >= 20);
    }

    #[test]
    fn presets_keep_the_photo_settings_and_blend() {
        let mut current = Settings {
            exposure: 0.7,
            ..Settings::default()
        };
        current.crop.left = 0.1;
        current.masks.push(crate::Mask::radial());
        let portra = PRESETS.iter().find(|p| p.name == "Portra 400").unwrap();
        let full = portra.applied(&current, 1.0);
        assert_eq!(full.exposure, 0.7);
        assert_eq!(full.crop.left, 0.1);
        assert_eq!(full.masks.len(), 1);
        assert_eq!(full.contrast, -12.0);
        let half = portra.applied(&current, 0.5);
        assert!((half.contrast + 6.0).abs() < 1e-4);
        assert!((half.effects.grain_amount - 9.0).abs() < 1e-4);
        assert_eq!(portra.applied(&current, 0.0), current);
    }

    #[test]
    fn presets_replace_each_other_instead_of_adding_up() {
        let s = Settings::default();
        for a in PRESETS {
            let direct = PRESETS[0].applied(&s, 1.0);
            let after = PRESETS[0].applied(&a.applied(&s, 1.0), 1.0);
            assert_eq!(after, direct, "{}", a.name);
        }
        let tri_x = PRESETS.iter().find(|p| p.name == "Tri-X 400").unwrap();
        assert_eq!(tri_x.applied(&s, 1.0).profile, Profile::Monochrome);
    }

    #[test]
    fn every_preset_stays_in_range() {
        for p in PRESETS {
            let s = p.applied(&Settings::default(), 1.0);
            assert_eq!(s.clone().clamped(), s, "{}", p.name);
        }
    }
}
