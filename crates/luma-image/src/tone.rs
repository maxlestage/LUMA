//! Tonalité locale : Correction du voile, Hautes lumières, Ombres, Clarté, Texture.
//!
//! Tout se passe sur la luminance, en IL autour du gris moyen (log2(Y / 0,18)), puis
//! revient à l'image comme un gain par pixel : les couleurs gardent leur teinte.
//!
//! Hautes lumières et Ombres agissent sur une **base** lissée qui respecte les
//! contours (filtre guidé) : une zone claire est assombrie en bloc, son détail reste
//! intact, et les bords nets ne font pas de halo. Ils
//! s'adaptent à la photo : ils visent ses tons clairs et sombres à elle, quelle que
//! soit son exposition. Clarté et Texture amplifient l'écart
//! à une base floue, à grande échelle (Clarté) ou à petite échelle (Texture). Les
//! rayons sont proportionnels à la diagonale de l'image : un aperçu et un export
//! donnent le même résultat.

use luma_color::luminance;
use luma_core::{Plane, RgbImage};
use luma_project::Settings;
use rayon::prelude::*;

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Correction du voile : modèle de diffusion atmosphérique (canal sombre, He et al.).
/// Positif : retire le voile. Négatif : en ajoute.
pub fn dehaze(img: &mut RgbImage, amount: f32) {
    if amount == 0.0 {
        return;
    }
    let Some((dark, air)) = haze(img) else {
        return;
    };
    let a = amount / 100.0;
    img.data
        .par_iter_mut()
        .zip(&dark.data)
        .for_each(|(p, &d)| clear_haze(p, d, air, a));
}

/// Le voile de `img` : son canal sombre flouté, et la lumière du voile (les canaux
/// sombres les plus clairs, 99,5e centile). `None` pour une image vide.
pub fn haze(img: &RgbImage) -> Option<(Plane, f32)> {
    let diag = ((img.width * img.width + img.height * img.height) as f32).sqrt();
    let dark = Plane::from_rgb(img, |p| p[0].min(p[1]).min(p[2]).max(0.0)).blur(0.01 * diag);
    let mut sample: Vec<f32> = dark.data.iter().step_by(7).copied().collect();
    if sample.is_empty() {
        return None;
    }
    let k = ((sample.len() - 1) as f32 * 0.995) as usize;
    let air = *sample.select_nth_unstable_by(k, f32::total_cmp).1;
    Some((dark, air.max(0.05)))
}

/// Un pixel dont le canal sombre vaut `d`, le voile `air`, corrigé de `a` (−1 à 1).
#[inline]
pub fn clear_haze(p: &mut [f32; 3], d: f32, air: f32, a: f32) {
    if a > 0.0 {
        let t = (1.0 - 0.9 * a * (d / air)).max(0.2);
        for c in p.iter_mut() {
            *c = ((*c - air) / t + air).max(0.0);
        }
    } else {
        let h = -a * 0.45 * (1.0 - (d / air).min(1.0) * 0.5);
        for c in p.iter_mut() {
            *c = *c * (1.0 - h) + air * h;
        }
    }
}

/// Le rayon du flou de la Texture, pour une image de diagonale `diag`.
pub fn texture_sigma(diag: f32) -> f32 {
    (0.0012 * diag).max(1.0)
}

/// Les tons proposés par [`auto_tones`] : exposition (IL), contraste, hautes lumières
/// et ombres (−100 à 100), comme les curseurs du panneau Base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutoTones {
    pub exposure: f32,
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
}

/// Les tons automatiques, lus sur l'histogramme de `img` (linéaire, sans réglage de
/// tons), sans IA. L'exposition amène la moyenne logarithmique de la luminance au gris
/// moyen, sans pousser le haut de l'histogramme bien au‑delà du blanc ; les hautes
/// lumières baissent si ce haut reste très clair, les ombres remontent si le bas reste
/// très sombre ; le contraste vise six IL entre les tons sombres et clairs.
pub fn auto_tones(img: &RgbImage) -> AutoTones {
    let every = (img.data.len() / 200_000).max(1);
    let mut lum: Vec<f32> = img
        .data
        .iter()
        .step_by(every)
        .map(|p| luminance(*p).max(1e-5))
        .collect();
    if lum.len() < 16 {
        return AutoTones {
            exposure: 0.0,
            contrast: 0.0,
            highlights: 0.0,
            shadows: 0.0,
        };
    }
    lum.sort_by(f32::total_cmp);
    let at = |q: f32| lum[((lum.len() - 1) as f32 * q) as usize];
    let key = (lum.iter().map(|v| f64::from(v.ln())).sum::<f64>() / lum.len() as f64).exp() as f32;
    // Le gris moyen ; jamais plus d'un IL au‑delà de ce qui amène le haut (99,5 %) au
    // double du blanc.
    let mut ev = (0.18 / key).log2();
    let room = (2.0 / at(0.995)).log2();
    if ev > room {
        ev = room.max(ev - 1.0).min(room + 1.0);
    }
    let ev = ev.clamp(-3.0, 3.0);
    let g = ev.exp2();
    let (lo, hi) = (at(0.05) * g, at(0.99) * g);
    let highlights = -(((hi - 0.6) / 1.4).clamp(0.0, 1.0) * 70.0);
    let shadows = ((0.03 / lo).log2() / 4.0).clamp(0.0, 1.0) * 50.0;
    let spread = (at(0.95) / at(0.05)).log2();
    let contrast = ((6.0 - spread) * 8.0).clamp(-30.0, 40.0);
    AutoTones {
        exposure: (ev * 20.0).round() / 20.0,
        contrast: contrast.round(),
        highlights: highlights.round(),
        shadows: shadows.round(),
    }
}

/// Hautes lumières, Ombres, Clarté et Texture, en un gain de luminance.
/// Les centiles 1, 50 et 99 de `sample` (ses valeurs de rang 1 %, 50 % et 99 %). La
/// médiane d'abord ; puis chacun des deux autres dans la moitié où il se trouve déjà,
/// au lieu de tout l'échantillon : les mêmes valeurs (une valeur de rang donné ne
/// dépend pas de l'ordre de la recherche), environ deux fois moins de travail.
fn centiles(sample: &mut [f32]) -> (f32, f32, f32) {
    let n = sample.len() - 1;
    let rank = |p: f32| (n as f32 * p) as usize;
    let (k_lo, k_mid, k_hi) = (rank(0.01), rank(0.5), rank(0.99));
    let (below, &mut mid, above) = sample.select_nth_unstable_by(k_mid, f32::total_cmp);
    let lo = if k_lo < k_mid {
        *below.select_nth_unstable_by(k_lo, f32::total_cmp).1
    } else {
        mid
    };
    let hi = if k_hi > k_mid {
        *above.select_nth_unstable_by(k_hi - k_mid - 1, f32::total_cmp).1
    } else {
        mid
    };
    (lo, mid, hi)
}

pub fn local_tone(img: &mut RgbImage, s: &Settings) {
    let (hl, sh, clarity, texture) = (
        s.highlights / 100.0,
        s.shadows / 100.0,
        s.clarity / 100.0,
        s.texture / 100.0,
    );
    if hl == 0.0 && sh == 0.0 && clarity == 0.0 && texture == 0.0 {
        return;
    }
    let diag = ((img.width * img.width + img.height * img.height) as f32).sqrt();
    // Luminance en IL autour du gris moyen.
    let stops = Plane::from_rgb(img, |p| (luminance(p).max(1e-6) / 0.18).log2());

    // Les bases d'abord, puis une seule passe qui ajoute chaque terme dans l'ordre
    // (hautes lumières et ombres, clarté, texture) et applique le gain : ni copie de la
    // luminance, ni une passe par curseur.
    // Les trois bases à la fois (chacune déjà parallèle, mais les filtres guidés
    // travaillent sur des plans réduits qui n'occupent pas tous les cœurs).
    let tones = || {
        (hl != 0.0 || sh != 0.0).then(|| {
            // Base : lisse à l'intérieur des zones, nette sur les contours (écarts > ½ IL).
            let base = stops.guided(0.02 * diag, 0.25);
            // Ces curseurs s'adaptent à l'image : ils visent ses
            // parties claires et sombres à elle (centiles 1, 50 et 99 de la base).
            let mut sample: Vec<f32> = base.data.iter().step_by(5).copied().collect();
            let (lo, mid, hi) = centiles(&mut sample);
            let (span_hi, span_lo) = ((hi - mid).max(0.5), (mid - lo).max(0.5));
            // Transitions sur 1,5 fois l'écart ; l'amplitude est plafonnée pour que la
            // courbe reste croissante (un ton ne passe jamais devant un autre).
            let k_hi = (hl * 1.3).max(-0.85 * span_hi * 1.2 / 1.5);
            let k_sh = (sh * 2.2).min(0.85 * span_lo);
            (base, mid, span_hi, span_lo, k_hi, k_sh)
        })
    };
    let clarity_base = || (clarity != 0.0).then(|| stops.guided(0.008 * diag, 0.5));
    let texture_base = || (texture != 0.0).then(|| stops.blur(texture_sigma(diag)));
    let (tones, (clarity_base, texture_base)) = rayon::join(tones, || rayon::join(clarity_base, texture_base));
    // Les hautes lumières démarrent à la médiane (les tons moyens restent en place),
    // les ombres un peu avant (elles remontent aussi les tons sombres moyens).
    let (ramp_hi, ramp_sh) = (|u: f32| smoothstep(0.0, 1.2, u), |u: f32| smoothstep(-0.3, 1.2, u));
    // Huit pixels à la fois : chaque terme en colonne (que le processeur fait d'un
    // coup), dans l'ordre d'avant ; les mêmes bits.
    const N: usize = crate::LANES;
    img.data
        .par_chunks_mut(N)
        .zip(stops.data.par_chunks(N))
        .enumerate()
        .for_each(|(chunk, (px, ls))| {
            let start = chunk * N;
            let n = px.len();
            let mut v = [0.0f32; N];
            v[..n].copy_from_slice(ls);
            if let Some((base, mid, span_hi, span_lo, k_hi, k_sh)) = &tones {
                let bs = &base.data[start..start + n];
                for j in 0..n {
                    let b = bs[j];
                    let w_hi = ramp_hi((b - mid) / span_hi);
                    let w_sh = ramp_sh((mid - b) / span_lo);
                    v[j] += k_hi * w_hi + k_sh * w_sh;
                }
            }
            if let Some(base) = &clarity_base {
                // Les tons moyens surtout : ni les noirs ni les blancs ne s'emballent.
                let bs = &base.data[start..start + n];
                for j in 0..n {
                    let l = ls[j];
                    let mid = (-(l / 2.5).powi(2)).exp();
                    v[j] += clarity * 0.7 * (l - bs[j]) * mid;
                }
            }
            if let Some(base) = &texture_base {
                // Les ombres profondes, où vit le bruit, sont épargnées.
                let bs = &base.data[start..start + n];
                for j in 0..n {
                    let l = ls[j];
                    let w = smoothstep(-6.0, -3.0, l);
                    v[j] += texture * 0.9 * (l - bs[j]) * w;
                }
            }
            for j in 0..n {
                let g = (v[j] - ls[j]).clamp(-6.0, 6.0).exp2();
                let p = &mut px[j];
                p[0] *= g;
                p[1] *= g;
                p[2] *= g;
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les centiles cherchés dans les moitiés donnent les valeurs d'une recherche dans
    /// tout l'échantillon : petits et grands échantillons, valeurs répétées.
    #[test]
    fn centiles_by_halves_are_the_same() {
        for len in [1usize, 2, 3, 7, 100, 101, 5003] {
            let sample: Vec<f32> = (0..len).map(|i| ((i * 7919) % 97) as f32 / 9.0 - 3.0).collect();
            let mut full = sample.clone();
            let n = full.len() - 1;
            let mut q = |p: f32| *full.select_nth_unstable_by((n as f32 * p) as usize, f32::total_cmp).1;
            let expected = (q(0.01), q(0.5), q(0.99));
            let mut halves = sample.clone();
            assert_eq!(centiles(&mut halves), expected, "{len}");
        }
    }

    /// Moitié gauche sombre, moitié droite claire, avec un léger motif.
    fn split() -> RgbImage {
        RgbImage::from_fn(160, 80, |x, y| {
            let v = if x < 80 { 0.02 } else { 0.9 } * (1.0 + 0.05 * (((x + y) % 3) as f32 - 1.0));
            [v, v, v]
        })
    }

    fn mean(img: &RgbImage, x0: usize, x1: usize) -> f32 {
        let mut s = 0.0;
        for y in 0..img.height {
            for x in x0..x1 {
                s += img.pixel(x, y)[1];
            }
        }
        s / ((x1 - x0) * img.height) as f32
    }

    // L'ancienne Tonalité locale (une copie de la luminance, une passe par curseur),
    // pour vérifier que la passe unique donne exactement les mêmes pixels.
    fn local_tone_reference(img: &mut RgbImage, s: &Settings) {
        let (hl, sh, clarity, texture) = (
            s.highlights / 100.0,
            s.shadows / 100.0,
            s.clarity / 100.0,
            s.texture / 100.0,
        );
        if hl == 0.0 && sh == 0.0 && clarity == 0.0 && texture == 0.0 {
            return;
        }
        let diag = ((img.width * img.width + img.height * img.height) as f32).sqrt();
        // Luminance en IL autour du gris moyen.
        let stops = Plane::from_rgb(img, |p| (luminance(p).max(1e-6) / 0.18).log2());
        let mut out = stops.clone();

        if hl != 0.0 || sh != 0.0 {
            // Base : lisse à l'intérieur des zones, nette sur les contours (écarts > ½ IL).
            let base = stops.guided(0.02 * diag, 0.25);
            // Ces curseurs s'adaptent à l'image : ils visent ses
            // parties claires et sombres à elle (centiles 1, 50 et 99 de la base).
            let mut sample: Vec<f32> = base.data.iter().step_by(5).copied().collect();
            let n = sample.len() - 1;
            let mut q = |p: f32| *sample.select_nth_unstable_by((n as f32 * p) as usize, f32::total_cmp).1;
            let (lo, mid, hi) = (q(0.01), q(0.5), q(0.99));
            let (span_hi, span_lo) = ((hi - mid).max(0.5), (mid - lo).max(0.5));
            // Transitions sur 1,5 fois l'écart ; l'amplitude est plafonnée pour que la
            // courbe reste croissante (un ton ne passe jamais devant un autre).
            // Les hautes lumières démarrent à la médiane (les tons moyens restent en place),
            // les ombres un peu avant (elles remontent aussi les tons sombres moyens).
            let (ramp_hi, ramp_sh) = (|u: f32| smoothstep(0.0, 1.2, u), |u: f32| smoothstep(-0.3, 1.2, u));
            let k_hi = (hl * 1.3).max(-0.85 * span_hi * 1.2 / 1.5);
            let k_sh = (sh * 2.2).min(0.85 * span_lo);
            out.data.par_iter_mut().zip(&base.data).for_each(|(v, &b)| {
                let w_hi = ramp_hi((b - mid) / span_hi);
                let w_sh = ramp_sh((mid - b) / span_lo);
                *v += k_hi * w_hi + k_sh * w_sh;
            });
        }
        if clarity != 0.0 {
            let base = stops.guided(0.008 * diag, 0.5);
            out.data
                .par_iter_mut()
                .zip(stops.data.par_iter().zip(&base.data))
                .for_each(|(v, (&l, &b))| {
                    // Les tons moyens surtout : ni les noirs ni les blancs ne s'emballent.
                    let mid = (-(l / 2.5).powi(2)).exp();
                    *v += clarity * 0.7 * (l - b) * mid;
                });
        }
        if texture != 0.0 {
            let base = stops.blur(texture_sigma(diag));
            out.data
                .par_iter_mut()
                .zip(stops.data.par_iter().zip(&base.data))
                .for_each(|(v, (&l, &b))| {
                    // Les ombres profondes, où vit le bruit, sont épargnées.
                    let w = smoothstep(-6.0, -3.0, l);
                    *v += texture * 0.9 * (l - b) * w;
                });
        }
        img.data
            .par_iter_mut()
            .zip(out.data.par_iter().zip(&stops.data))
            .for_each(|(p, (&new, &old))| {
                let g = (new - old).clamp(-6.0, 6.0).exp2();
                p[0] *= g;
                p[1] *= g;
                p[2] *= g;
            });
    }

    #[test]
    fn one_pass_local_tone_gives_the_same_pixels() {
        let img = RgbImage::from_fn(181, 127, |x, y| {
            let v = 0.02 + 0.9 * (((x * 13 + y * 7) % 97) as f32 / 97.0).powi(2);
            [v, v * 0.8 + 0.05 * (y % 5) as f32, v * 0.6]
        });
        for (hl, sh, cl, tx) in [
            (-60.0, 40.0, 25.0, 15.0),
            (30.0, 0.0, 0.0, 0.0),
            (0.0, 0.0, -50.0, 0.0),
            (0.0, 0.0, 0.0, 80.0),
        ] {
            let s = Settings {
                highlights: hl,
                shadows: sh,
                clarity: cl,
                texture: tx,
                ..Settings::default()
            };
            let (mut a, mut b) = (img.clone(), img.clone());
            local_tone(&mut a, &s);
            local_tone_reference(&mut b, &s);
            let same = a
                .data
                .iter()
                .zip(&b.data)
                .all(|(p, q)| (0..3).all(|c| p[c].to_bits() == q[c].to_bits()));
            assert!(same, "{hl} {sh} {cl} {tx}");
        }
    }

    /// Une scène de tons répartis en douceur, de `lo` à `hi` (linéaire).
    fn ramp(lo: f32, hi: f32) -> RgbImage {
        RgbImage::from_fn(200, 100, |x, _| {
            let v = lo * (hi / lo).powf(x as f32 / 199.0);
            [v, v, v]
        })
    }

    #[test]
    fn auto_exposure_never_pushes_the_top_more_than_a_stop_past_double_white() {
        // Une nuit : presque tout très sombre, quelques lumières au blanc. Le gris moyen
        // demanderait près de six IL ; le haut (99,5 %) ne doit pas finir à plus d'un IL
        // au‑delà du double du blanc (soit quatre fois le blanc).
        let night = RgbImage::from_fn(200, 100, |x, y| {
            let v = if (x + y * 200) % 50 == 0 { 1.0 } else { 0.18 / 64.0 };
            [v, v, v]
        });
        let t = auto_tones(&night);
        assert!(t.exposure > 0.0 && t.exposure <= 2.0 + 0.05, "{t:?}");
    }

    #[test]
    fn auto_tones_follow_the_histogram() {
        // Sous‑exposée de deux IL : l'exposition remonte d'autant, environ.
        let dark = auto_tones(&ramp(0.18 / 4.0 / 8.0, 0.18 / 4.0 * 8.0));
        assert!((dark.exposure - 2.0).abs() < 0.15, "{dark:?}");
        // Surexposée : elle baisse.
        let bright = auto_tones(&ramp(0.18 * 2.0 / 8.0, 0.18 * 2.0 * 8.0));
        assert!((bright.exposure + 1.0).abs() < 0.15, "{bright:?}");
        // Bien exposée, six IL d'écart : presque rien.
        let fine = auto_tones(&ramp(0.18 / 8.0, 0.18 * 8.0));
        assert!(fine.exposure.abs() < 0.1 && fine.contrast.abs() <= 8.0, "{fine:?}");
        // Douze IL de la nuit au ciel : hautes lumières baissées, ombres remontées,
        // contraste réduit.
        let wide = auto_tones(&ramp(0.18 / 64.0, 0.18 * 64.0));
        assert!(
            wide.highlights < -20.0 && wide.shadows > 20.0 && wide.contrast < 0.0,
            "{wide:?}"
        );
        // Un brouillard de deux IL : contraste relevé, ni ombres ni hautes lumières.
        let flat = auto_tones(&ramp(0.18 / 2.0, 0.18 * 2.0));
        assert!(
            flat.contrast > 20.0 && flat.highlights == 0.0 && flat.shadows == 0.0,
            "{flat:?}"
        );
    }

    #[test]
    fn neutral_settings_do_nothing() {
        let mut img = split();
        local_tone(&mut img, &Settings::default());
        assert_eq!(img, split());
    }

    #[test]
    fn highlights_and_shadows_act_where_they_should() {
        // Un dégradé de −6 à +2 IL : les ombres relèvent la gauche, les hautes lumières
        // abaissent la droite, le milieu bouge peu.
        let ramp = || {
            RgbImage::from_fn(400, 40, |x, _| {
                let v = 0.18 * ((x as f32 / 400.0) * 8.0 - 6.0).exp2();
                [v, v, v]
            })
        };
        let orig = ramp();
        let mut sh = ramp();
        local_tone(
            &mut sh,
            &Settings {
                shadows: 100.0,
                ..Settings::default()
            },
        );
        let mut hl = ramp();
        local_tone(
            &mut hl,
            &Settings {
                highlights: -100.0,
                ..Settings::default()
            },
        );
        assert!(mean(&sh, 10, 60) > mean(&orig, 10, 60) * 2.0, "ombres relevées");
        assert!(
            mean(&hl, 360, 400) < mean(&orig, 360, 400) * 0.7,
            "hautes lumières ramenées"
        );
        assert!(mean(&hl, 10, 60) == mean(&orig, 10, 60), "les ombres ne bougent pas");
    }

    #[test]
    fn tone_order_is_preserved() {
        // Un dégradé : quels que soient les curseurs, il reste croissant.
        for (hl, sh) in [(-100.0, 100.0), (100.0, -100.0), (-100.0, -100.0)] {
            let s = Settings {
                highlights: hl,
                shadows: sh,
                ..Settings::default()
            };
            let mut img = RgbImage::from_fn(400, 20, |x, _| {
                let v = 0.18 * ((x as f32 / 400.0) * 10.0 - 6.0).exp2();
                [v, v, v]
            });
            local_tone(&mut img, &s);
            for x in 1..400 {
                assert!(
                    img.pixel(x, 10)[1] >= img.pixel(x - 1, 10)[1] * 0.999,
                    "{hl} {sh} x={x}"
                );
            }
        }
    }

    #[test]
    fn clarity_adds_local_contrast() {
        let s = Settings {
            clarity: 100.0,
            ..Settings::default()
        };
        // Diagonale 500 px : rayon de clarté ≈ 4 px, motif de 25 px.
        let mut img = RgbImage::from_fn(400, 300, |x, _| {
            let v = 0.18 * (1.0 + 0.3 * ((x as f32) / 4.0).sin());
            [v, v, v]
        });
        let before = img.clone();
        local_tone(&mut img, &s);
        let spread = |i: &RgbImage| {
            let r: Vec<f32> = (100..300).map(|x| i.pixel(x, 150)[1]).collect();
            r.iter().cloned().fold(f32::MIN, f32::max) - r.iter().cloned().fold(f32::MAX, f32::min)
        };
        assert!(spread(&img) > spread(&before) * 1.1);
    }

    #[test]
    fn dehaze_restores_contrast() {
        let mut img = RgbImage::from_fn(120, 80, |x, _| {
            let v = 0.6 + 0.1 * if x % 20 < 10 { 1.0 } else { -1.0 };
            [v, v, v * 1.02]
        });
        let before = img.clone();
        dehaze(&mut img, 80.0);
        let c = |i: &RgbImage| i.pixel(5, 40)[1] - i.pixel(15, 40)[1];
        assert!(c(&img) > c(&before));
    }
}
