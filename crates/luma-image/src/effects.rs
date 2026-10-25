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

#[inline]
fn hash(x: i64, y: i64) -> f32 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h & 0xFFFF) as f32 / 65535.0 * 2.0 - 1.0
}

#[inline]
fn value_noise(x: f32, y: f32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (x - x0, y - y0);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let (xi, yi) = (x0 as i64, y0 as i64);
    let top = hash(xi, yi) + (hash(xi + 1, yi) - hash(xi, yi)) * sx;
    let bottom = hash(xi, yi + 1) + (hash(xi + 1, yi + 1) - hash(xi, yi + 1)) * sx;
    top + (bottom - top) * sy
}

/// `scale` : pixels rendus par pixel pleine définition.
pub fn apply(img: &mut RgbImage, e: &Effects, scale: f32) {
    let vignette = e.vignette_amount / 100.0;
    let grain = e.grain_amount / 100.0;
    if vignette == 0.0 && grain == 0.0 {
        return;
    }
    let (w, h) = (img.width as f32, img.height as f32);
    let round = e.vignette_roundness / 100.0;
    let circle = round.max(0.0);
    let power = 2.0 + (-round).max(0.0) * 3.0;
    let feather = e.vignette_feather / 100.0;
    let inner = (0.2 + 0.8 * e.vignette_midpoint / 100.0) * (1.0 - 0.7 * feather);
    let outer = inner + 0.15 + 0.9 * feather;
    let protect = e.vignette_highlights / 100.0;
    let cell = (1.0 + e.grain_size / 100.0 * 4.0) / scale.max(1e-3);
    let rough = e.grain_roughness / 100.0;
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
                    (nx2.abs().powf(power) + ny2.abs().powf(power)).powf(1.0 / power) / 2f32.sqrt().powf(2.0 / power)
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

#[cfg(test)]
mod tests {
    use super::*;

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
