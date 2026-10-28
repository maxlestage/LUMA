//! Panneau Effets : vignetage après recadrage et grain.
//!
//! Le grain est un bruit de valeur calculé en coordonnées de l'image pleine
//! définition : l'aperçu et l'export ont le même grain, à la même place.

use luma_color::luminance;
use luma_core::RgbImage;
use luma_project::Effects;
use rayon::prelude::*;

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Un hachage 32 bits (entiers non signés seulement) : le même calcul exactement sur
/// le processeur et dans les shaders du GPU.
#[inline]
pub fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0 * 2.0 - 1.0
}

#[inline]
fn value_noise(x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (x - x0, y - y0);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let (xi, yi) = (x0 as i32, y0 as i32);
    let top = hash(xi, yi) + (hash(xi + 1, yi) - hash(xi, yi)) * sx;
    let bottom = hash(xi, yi + 1) + (hash(xi + 1, yi + 1) - hash(xi, yi + 1)) * sx;
    top + (bottom - top) * sy
}

/// Les grandeurs dérivées des curseurs (aussi utilisées par les shaders du GPU).
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub vignette: f32,
    pub circle: f32,
    pub power: f32,
    pub inner: f32,
    pub outer: f32,
    pub protect: f32,
    pub grain: f32,
    pub cell: f32,
    pub rough: f32,
}

/// `scale` : pixels rendus par pixel pleine définition.
pub fn params(e: &Effects, scale: f32) -> Params {
    let round = e.vignette_roundness / 100.0;
    let feather = e.vignette_feather / 100.0;
    let inner = (0.2 + 0.8 * e.vignette_midpoint / 100.0) * (1.0 - 0.7 * feather);
    Params {
        vignette: e.vignette_amount / 100.0,
        circle: round.max(0.0),
        power: 2.0 + (-round).max(0.0) * 3.0,
        inner,
        outer: inner + 0.15 + 0.9 * feather,
        protect: e.vignette_highlights / 100.0,
        grain: e.grain_amount / 100.0,
        cell: (1.0 + e.grain_size / 100.0 * 4.0) / scale.max(1e-3),
        rough: e.grain_roughness / 100.0,
    }
}

/// `scale` : pixels rendus par pixel pleine définition.
pub fn apply(img: &mut RgbImage, e: &Effects, scale: f32) {
    let Params {
        vignette,
        circle,
        power,
        inner,
        outer,
        protect,
        grain,
        cell,
        rough,
    } = params(e, scale);
    if vignette == 0.0 && grain == 0.0 {
        return;
    }
    let (w, h) = (img.width as f32, img.height as f32);
    let width = img.width;
    // La distance au centre se sépare : un terme par colonne, un par ligne, calculés une
    // fois (les mêmes valeurs qu'à chaque pixel).
    // Rondeur 0 : ellipse du cadre ; +100 : cercle ; −100 : presque rectangle.
    let term = |v: f32| if power == 2.0 { v * v } else { v.abs().powf(power) };
    let columns: Vec<f32> = (0..width)
        .map(|x| {
            let nx = (x as f32 + 0.5) / w * 2.0 - 1.0;
            term(if w < h {
                nx * (1.0 - circle + circle * w / h)
            } else {
                nx
            })
        })
        .collect();
    img.data.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        let ny = (y as f32 + 0.5) / h * 2.0 - 1.0;
        let ty = term(if w >= h {
            ny * (1.0 - circle + circle * h / w)
        } else {
            ny
        });
        for (x, p) in row.iter_mut().enumerate() {
            if vignette != 0.0 {
                let d = if power == 2.0 {
                    (columns[x] + ty).sqrt() * std::f32::consts::FRAC_1_SQRT_2
                } else {
                    (columns[x] + ty).powf(1.0 / power) / 2f32.sqrt().powf(2.0 / power)
                };
                let k = smoothstep(inner, outer, d);
                if vignette < 0.0 {
                    // Dans l'ovale intérieur, le gain vaut exactement 1 : rien à faire.
                    if k != 0.0 {
                        let bright = smoothstep(0.4, 1.0, luminance(*p));
                        let g = 1.0 + vignette * k * (1.0 - protect * bright);
                        p.iter_mut().for_each(|c| *c *= g);
                    }
                } else {
                    p.iter_mut().for_each(|c| *c += (1.0 - *c).max(0.0) * vignette * k);
                }
            }
            if grain != 0.0 {
                let (fx, fy) = (x as f32 / scale, y as f32 / scale);
                let coarse = value_noise(fx / cell, fy / cell);
                let fine = value_noise(fx / (cell * 0.45) + 17.3, fy / (cell * 0.45) - 9.1);
                let n = coarse * (1.0 - 0.5 * rough) + fine * 0.5 * rough;
                let y = luminance(*p).clamp(0.0, 1.0).sqrt();
                let strength = grain * 0.12 * (4.0 * y * (1.0 - y)).max(0.15);
                let g = 1.0 + n * strength;
                p.iter_mut().for_each(|c| *c = (*c * g).max(0.0));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // L'ancien calcul (la distance refaite à chaque pixel), pour vérifier que la
    // version par lignes et colonnes donne exactement les mêmes pixels.
    fn apply_reference(img: &mut RgbImage, e: &Effects, scale: f32) {
        let Params {
            vignette,
            circle,
            power,
            inner,
            outer,
            protect,
            grain,
            cell,
            rough,
        } = params(e, scale);
        if vignette == 0.0 && grain == 0.0 {
            return;
        }
        let (w, h) = (img.width as f32, img.height as f32);
        let width = img.width;
        img.data.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
            let ny = (y as f32 + 0.5) / h * 2.0 - 1.0;
            for (x, p) in row.iter_mut().enumerate() {
                if vignette != 0.0 {
                    let nx = (x as f32 + 0.5) / w * 2.0 - 1.0;
                    // Rondeur 0 : ellipse du cadre ; +100 : cercle ; −100 : presque rectangle.
                    let ny2 = if w >= h {
                        ny * (1.0 - circle + circle * h / w)
                    } else {
                        ny
                    };
                    let nx2 = if w < h {
                        nx * (1.0 - circle + circle * w / h)
                    } else {
                        nx
                    };
                    let d = if power == 2.0 {
                        (nx2 * nx2 + ny2 * ny2).sqrt() * std::f32::consts::FRAC_1_SQRT_2
                    } else {
                        (nx2.abs().powf(power) + ny2.abs().powf(power)).powf(1.0 / power)
                            / 2f32.sqrt().powf(2.0 / power)
                    };
                    let k = smoothstep(inner, outer, d);
                    if vignette < 0.0 {
                        let bright = smoothstep(0.4, 1.0, luminance(*p));
                        let g = 1.0 + vignette * k * (1.0 - protect * bright);
                        p.iter_mut().for_each(|c| *c *= g);
                    } else {
                        p.iter_mut().for_each(|c| *c += (1.0 - *c).max(0.0) * vignette * k);
                    }
                }
                if grain != 0.0 {
                    let (fx, fy) = (x as f32 / scale, y as f32 / scale);
                    let coarse = value_noise(fx / cell, fy / cell);
                    let fine = value_noise(fx / (cell * 0.45) + 17.3, fy / (cell * 0.45) - 9.1);
                    let n = coarse * (1.0 - 0.5 * rough) + fine * 0.5 * rough;
                    let y = luminance(*p).clamp(0.0, 1.0).sqrt();
                    let strength = grain * 0.12 * (4.0 * y * (1.0 - y)).max(0.15);
                    let g = 1.0 + n * strength;
                    p.iter_mut().for_each(|c| *c = (*c * g).max(0.0));
                }
            }
        });
    }

    #[test]
    fn separable_vignette_gives_the_same_pixels() {
        for (w, h) in [(121, 80), (64, 97)] {
            let img = RgbImage::from_fn(w, h, |x, y| {
                let v = 0.05 + 0.9 * (((x * 7 + y * 13) % 31) as f32 / 31.0);
                [v, v * 0.7, 1.0 - v]
            });
            for (amount, roundness, feather, grain) in [
                (-40.0, 0.0, 50.0, 0.0),
                (-70.0, -100.0, 10.0, 0.0),
                (-25.0, 60.0, 90.0, 30.0),
                (35.0, -40.0, 50.0, 0.0),
                (0.0, 0.0, 50.0, 40.0),
            ] {
                let e = Effects {
                    vignette_amount: amount,
                    vignette_roundness: roundness,
                    vignette_feather: feather,
                    vignette_highlights: 30.0,
                    grain_amount: grain,
                    ..Effects::default()
                };
                let (mut a, mut b) = (img.clone(), img.clone());
                apply(&mut a, &e, 0.5);
                apply_reference(&mut b, &e, 0.5);
                let same = a
                    .data
                    .iter()
                    .zip(&b.data)
                    .all(|(p, q)| (0..3).all(|c| p[c].to_bits() == q[c].to_bits()));
                assert!(same, "{w}×{h} {amount} {roundness} {feather} {grain}");
            }
        }
    }

    #[test]
    fn vignette_darkens_corners_not_the_center() {
        let mut img = RgbImage::from_fn(120, 80, |_, _| [0.4; 3]);
        apply(
            &mut img,
            &Effects {
                vignette_amount: -80.0,
                ..Effects::default()
            },
            1.0,
        );
        assert!(img.pixel(1, 1)[0] < 0.2);
        assert!((img.pixel(60, 40)[0] - 0.4).abs() < 1e-3);
    }

    #[test]
    fn grain_is_the_same_at_every_size() {
        let e = Effects {
            grain_amount: 60.0,
            ..Effects::default()
        };
        let mut big = RgbImage::from_fn(200, 200, |_, _| [0.2; 3]);
        let mut small = RgbImage::from_fn(100, 100, |_, _| [0.2; 3]);
        apply(&mut big, &e, 1.0);
        apply(&mut small, &e, 0.5);
        // Le même point de l'image a le même grain (à l'échantillonnage près).
        let (a, b) = (big.pixel(40, 60)[0], small.pixel(20, 30)[0]);
        assert!((a - b).abs() < 0.02, "{a} {b}");
        assert!(big.data.iter().any(|p| (p[0] - 0.2).abs() > 0.005));
    }
}
