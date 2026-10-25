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
    BlackAndWhiteFilm,
    Creative,
}

impl Group {
    pub fn name(self) -> &'static str {
        match self {
            Group::ColorFilm => "Pellicules couleur",
            Group::BlackAndWhiteFilm => "Pellicules noir et blanc",
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

/// Des noirs « délavés » et des blancs adoucis, comme un tirage argentique.
fn film_curve(s: &mut Settings, black: f32, white: f32) {
    s.tone_curve.point = vec![
        (0.0, black),
        (64.0, 62.0 + black * 0.3),
        (128.0, 128.0),
        (192.0, 194.0),
        (255.0, white),
    ];
}

fn grain(s: &mut Settings, amount: f32, size: f32, roughness: f32) {
    s.effects.grain_amount = amount;
    s.effects.grain_size = size;
    s.effects.grain_roughness = roughness;
}

// Ordre du mélangeur : rouge, orange, jaune, vert, bleu‑vert, bleu, violet, magenta.
const R: usize = 0;
const O: usize = 1;
const Y: usize = 2;
const G: usize = 3;
const A: usize = 4;
const B: usize = 5;

/// Les préréglages intégrés.
pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Portra 400",
        group: Group::ColorFilm,
        description: "Inspiré de Kodak Portra 400 : peaux chaudes, contraste doux, noirs délavés.",
        apply: |s| {
            s.contrast = -12.0;
            s.highlights = -15.0;
            s.saturation = -8.0;
            s.vibrance = 6.0;
            film_curve(s, 12.0, 248.0);
            s.hsl.saturation[O] = -6.0;
            s.hsl.luminance[O] = 6.0;
            s.hsl.hue[G] = 12.0;
            s.hsl.saturation[G] = -18.0;
            s.hsl.saturation[B] = -12.0;
            s.color_grading.shadows = wheel(200.0, 8.0, 0.0);
            s.color_grading.highlights = wheel(45.0, 12.0, 0.0);
            grain(s, 18.0, 25.0, 45.0);
        },
    },
    Preset {
        name: "Ektar 100",
        group: Group::ColorFilm,
        description: "Inspiré de Kodak Ektar 100 : couleurs franches, rouges et bleus profonds, grain très fin.",
        apply: |s| {
            s.contrast = 15.0;
            s.saturation = 12.0;
            s.vibrance = 10.0;
            s.hsl.saturation[R] = 10.0;
            s.hsl.saturation[B] = 15.0;
            s.hsl.luminance[B] = -10.0;
            film_curve(s, 4.0, 252.0);
            grain(s, 6.0, 15.0, 40.0);
        },
    },
    Preset {
        name: "Gold 200",
        group: Group::ColorFilm,
        description: "Inspiré de Kodak Gold 200 : lumière dorée, jaunes généreux, ciels adoucis.",
        apply: |s| {
            s.contrast = 5.0;
            s.vibrance = 8.0;
            s.hsl.saturation[Y] = 15.0;
            s.hsl.hue[B] = -10.0;
            s.hsl.saturation[B] = -15.0;
            film_curve(s, 10.0, 246.0);
            s.color_grading.global = wheel(50.0, 12.0, 0.0);
            s.color_grading.highlights = wheel(45.0, 18.0, 0.0);
            grain(s, 22.0, 30.0, 50.0);
        },
    },
    Preset {
        name: "Velvia 50",
        group: Group::ColorFilm,
        description: "Inspiré de Fujifilm Velvia 50 : diapositive de paysage, verts et bleus intenses, ombres denses.",
        apply: |s| {
            s.contrast = 25.0;
            s.saturation = 20.0;
            s.vibrance = 15.0;
            s.blacks = -10.0;
            s.hsl.hue[G] = -5.0;
            s.hsl.saturation[G] = 15.0;
            s.hsl.saturation[B] = 20.0;
            s.hsl.luminance[B] = -15.0;
            s.color_grading.shadows = wheel(230.0, 10.0, 0.0);
            grain(s, 5.0, 15.0, 30.0);
        },
    },
    Preset {
        name: "Provia 100F",
        group: Group::ColorFilm,
        description: "Inspiré de Fujifilm Provia 100F : diapositive fidèle, contraste net, couleurs propres.",
        apply: |s| {
            s.contrast = 10.0;
            s.saturation = 8.0;
            s.vibrance = 5.0;
            grain(s, 6.0, 15.0, 30.0);
        },
    },
    Preset {
        name: "Pro 400H",
        group: Group::ColorFilm,
        description: "Inspiré de Fujifilm Pro 400H : pastels lumineux, verts menthe, ombres froides.",
        apply: |s| {
            s.contrast = -15.0;
            s.saturation = -10.0;
            s.shadows = 10.0;
            film_curve(s, 16.0, 246.0);
            s.hsl.hue[G] = 15.0;
            s.hsl.saturation[G] = -10.0;
            s.hsl.hue[B] = -10.0;
            s.color_grading.shadows = wheel(160.0, 10.0, 0.0);
            s.color_grading.highlights = wheel(200.0, 5.0, 0.0);
            grain(s, 18.0, 25.0, 45.0);
        },
    },
    Preset {
        name: "Superia 400",
        group: Group::ColorFilm,
        description: "Inspiré de Fujifilm Superia 400 : pellicule du quotidien, ombres verdâtres, grain présent.",
        apply: |s| {
            s.contrast = 8.0;
            s.saturation = 4.0;
            film_curve(s, 8.0, 250.0);
            s.hsl.saturation[G] = 10.0;
            s.color_grading.shadows = wheel(140.0, 12.0, 0.0);
            s.color_grading.highlights = wheel(60.0, 6.0, 0.0);
            grain(s, 26.0, 30.0, 55.0);
        },
    },
    Preset {
        name: "CineStill 800T",
        group: Group::ColorFilm,
        description: "Inspiré de CineStill 800T : film de cinéma tungstène, nuits bleutées, lumières orangées.",
        apply: |s| {
            s.contrast = 10.0;
            s.highlights = -10.0;
            film_curve(s, 10.0, 245.0);
            s.hsl.hue[R] = 8.0;
            s.hsl.saturation[R] = 10.0;
            s.hsl.hue[A] = -10.0;
            s.color_grading.shadows = wheel(200.0, 25.0, 0.0);
            s.color_grading.highlights = wheel(30.0, 15.0, 0.0);
            grain(s, 30.0, 40.0, 50.0);
        },
    },
    Preset {
        name: "Kodachrome 64",
        group: Group::ColorFilm,
        description: "Inspiré de Kodachrome 64 : rouges riches, bleus profonds, contraste marqué.",
        apply: |s| {
            s.contrast = 20.0;
            s.saturation = 6.0;
            s.hsl.saturation[R] = 15.0;
            s.hsl.luminance[R] = -5.0;
            s.hsl.hue[Y] = -5.0;
            s.hsl.saturation[B] = 10.0;
            s.hsl.luminance[B] = -15.0;
            s.color_grading.highlights = wheel(40.0, 8.0, 0.0);
            grain(s, 10.0, 20.0, 40.0);
        },
    },
    Preset {
        name: "Ektachrome E100",
        group: Group::ColorFilm,
        description: "Inspiré de Kodak Ektachrome E100 : diapositive claire et froide, blancs purs.",
        apply: |s| {
            s.contrast = 15.0;
            s.saturation = 10.0;
            s.color_grading.shadows = wheel(220.0, 10.0, 0.0);
            s.hsl.saturation[B] = 10.0;
            grain(s, 6.0, 15.0, 30.0);
        },
    },
    Preset {
        name: "Tri-X 400",
        group: Group::BlackAndWhiteFilm,
        description: "Inspiré de Kodak Tri-X 400 : noir et blanc contrasté du reportage, grain franc.",
        apply: |s| {
            s.profile = Profile::Monochrome;
            s.contrast = 25.0;
            s.clarity = 10.0;
            s.hsl.luminance[R] = 5.0;
            s.hsl.luminance[B] = -10.0;
            s.tone_curve.point = vec![(0.0, 6.0), (64.0, 54.0), (128.0, 128.0), (192.0, 204.0), (255.0, 250.0)];
            grain(s, 35.0, 30.0, 60.0);
        },
    },
    Preset {
        name: "HP5 Plus",
        group: Group::BlackAndWhiteFilm,
        description: "Inspiré d'Ilford HP5 Plus : gris doux et nuancés, grain souple.",
        apply: |s| {
            s.profile = Profile::Monochrome;
            s.contrast = 8.0;
            film_curve(s, 10.0, 248.0);
            grain(s, 30.0, 30.0, 45.0);
        },
    },
    Preset {
        name: "Delta 100",
        group: Group::BlackAndWhiteFilm,
        description: "Inspiré d'Ilford Delta 100 : noir et blanc fin et précis, presque sans grain.",
        apply: |s| {
            s.profile = Profile::Monochrome;
            s.contrast = 6.0;
            s.texture = 8.0;
            grain(s, 8.0, 12.0, 30.0);
        },
    },
    Preset {
        name: "Acros 100",
        group: Group::BlackAndWhiteFilm,
        description: "Inspiré de Fujifilm Acros 100 : ciels sombres (filtre rouge léger), peaux claires.",
        apply: |s| {
            s.profile = Profile::Monochrome;
            s.contrast = 12.0;
            s.hsl.luminance[R] = 12.0;
            s.hsl.luminance[O] = 10.0;
            s.hsl.luminance[B] = -25.0;
            s.hsl.luminance[A] = -15.0;
            grain(s, 10.0, 15.0, 35.0);
        },
    },
    Preset {
        name: "Fondu mat",
        group: Group::Creative,
        description: "Noirs relevés, couleurs retenues : un rendu mat et calme.",
        apply: |s| {
            s.contrast = -10.0;
            s.saturation = -15.0;
            film_curve(s, 32.0, 236.0);
        },
    },
    Preset {
        name: "Sarcelle et orange",
        group: Group::Creative,
        description: "Le contraste de couleurs du cinéma : ombres sarcelle, peaux orangées.",
        apply: |s| {
            s.contrast = 12.0;
            s.vibrance = 10.0;
            s.color_grading.shadows = wheel(190.0, 30.0, 0.0);
            s.color_grading.highlights = wheel(35.0, 22.0, 0.0);
            s.hsl.hue[G] = 25.0;
            s.hsl.saturation[G] = -20.0;
        },
    },
    Preset {
        name: "Haute clé",
        group: Group::Creative,
        description: "Clair, aérien, ombres ouvertes.",
        apply: |s| {
            s.contrast = -20.0;
            s.highlights = -10.0;
            s.shadows = 40.0;
            s.whites = 15.0;
            s.saturation = -10.0;
        },
    },
    Preset {
        name: "Noir dramatique",
        group: Group::Creative,
        description: "Noir et blanc dense, clarté forte, ciels sombres.",
        apply: |s| {
            s.profile = Profile::Monochrome;
            s.contrast = 40.0;
            s.clarity = 30.0;
            s.blacks = -20.0;
            s.hsl.luminance[B] = -40.0;
            s.hsl.luminance[A] = -30.0;
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;

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
        let tri_x = PRESETS.iter().find(|p| p.name == "Tri-X 400").unwrap();
        let ektar = PRESETS.iter().find(|p| p.name == "Ektar 100").unwrap();
        let once = ektar.applied(&s, 1.0);
        let after_bw = ektar.applied(&tri_x.applied(&s, 1.0), 1.0);
        assert_eq!(once, after_bw);
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
