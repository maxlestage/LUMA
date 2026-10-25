//! Panneau Détail : réduction du bruit et accentuation.
//!
//! - La réduction du bruit travaille avant la tonalité, dans un espace « racine carrée »
//!   (luminance et deux différences de couleur) : la luminance par un filtre guidé qui
//!   garde les contours, la couleur par un flou plus large, guidé lui aussi pour ne pas
//!   baver sur les bords colorés.
//! - L'accentuation est un masque flou sur la luminance perçue, avec un plafond contre
//!   les halos (Détail) et un masque de contours (Masquage).
//!
//! Les rayons sont donnés en pixels de l'image pleine définition et ramenés à la
//! taille rendue (`scale`) : un aperçu réduit n'est pas sur‑traité.

use luma_color::luminance;
use luma_core::{Plane, RgbImage};
use luma_project::Detail;
use rayon::prelude::*;

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn noise_reduction(img: &mut RgbImage, d: &Detail, scale: f32) {
    let (lum, col) = (d.luminance_nr / 100.0, d.color_nr / 100.0);
    let lum_sigma = (1.0 + 2.5 * lum) * scale;
    let col_sigma = (2.0 + 10.0 * col) * scale;
    // En dessous d'un pixel de rayon (aperçus très réduits), la réduction elle‑même a
    // déjà moyenné le bruit : le lissage ne se verrait pas.
    let do_lum = lum > 0.0 && lum_sigma >= 0.8;
    let do_col = col > 0.0 && col_sigma >= 0.6;
    if !do_lum && !do_col {
        return;
    }
    // Encodage « racine carrée » : proche de la perception (le bruit y est à peu près
    // uniforme des ombres aux lumières) et bien moins coûteux qu'OKLab. Puis une
    // luminance et deux différences de couleur, comme un YCbCr.
    const WR: f32 = 0.2627;
    const WG: f32 = 0.6780;
    const WB: f32 = 0.0593;
    let (w, h) = (img.width, img.height);
    let enc: Vec<[f32; 3]> = img.data.par_iter().map(|p| p.map(|v| v.max(0.0).sqrt())).collect();
    let mut y = Plane {
        width: w,
        height: h,
        data: enc.par_iter().map(|e| WR * e[0] + WG * e[1] + WB * e[2]).collect(),
    };
    let mut cr = Plane {
        width: w,
        height: h,
        data: enc.par_iter().zip(&y.data).map(|(e, y)| e[0] - y).collect(),
    };
    let mut cb = Plane {
        width: w,
        height: h,
        data: enc.par_iter().zip(&y.data).map(|(e, y)| e[2] - y).collect(),
    };
    drop(enc);
    if do_lum {
        // Le détail garde les petites structures : il baisse le seuil de lissage.
        let eps = (lum * lum * 0.0015 * (1.0 - 0.8 * d.luminance_nr_detail / 100.0)).max(1e-7);
        let smooth = y.guided(lum_sigma, eps);
        y = y.zip(&smooth, |orig, s| orig + (s - orig) * lum.sqrt());
    }
    if do_col {
        let eps = 0.0001 + 0.001 * (1.0 - d.color_nr_detail / 100.0);
        let sigma = col_sigma * (0.6 + 0.8 * d.color_nr_smoothness / 100.0);
        // La couleur se lisse à demi‑résolution (comme un JPEG en 4:2:0, l'œil ne voit pas
        // la différence) ; petit rayon : flou simple, grand rayon : flou guidé.
        let half = |p: &Plane| {
            let small = p.downscale(2);
            let s = if sigma < 5.0 {
                small.blur(sigma / 2.0)
            } else {
                small.guided(sigma / 2.0, eps)
            };
            s.upsample(w, h)
        };
        cr = half(&cr);
        cb = half(&cb);
    }
    img.data.par_iter_mut().enumerate().for_each(|(i, p)| {
        let (y, r, b) = (y.data[i], y.data[i] + cr.data[i], y.data[i] + cb.data[i]);
        let g = (y - WR * r - WB * b) / WG;
        *p = [r.max(0.0).powi(2), g.max(0.0).powi(2), b.max(0.0).powi(2)];
    });
}

/// Accentuation, sur l'image déjà rendue pour l'écran (linéaire, 0 à 1).
pub fn sharpen(img: &mut RgbImage, d: &Detail, scale: f32) {
    let amount = d.sharpen_amount / 100.0 * 1.4;
    // L'accentuation s'évalue à 100 % : très en dessous, on ne l'applique pas, comme
    // Lightroom qui ne la montre qu'à partir de l'affichage 1:1.
    if amount <= 0.0 || scale < 0.5 {
        return;
    }
    let radius = (d.sharpen_radius * scale).max(0.5);
    let perceived = Plane::from_rgb(img, |p| luminance(p).max(0.0).powf(1.0 / 2.2));
    let blurred = perceived.blur(radius);
    let limit = 0.015 + d.sharpen_detail / 100.0 * 0.12;
    let mask = if d.sharpen_masking > 0.0 {
        let th = d.sharpen_masking / 100.0 * 0.06;
        let edges = blurred.blur(1.0);
        let (w, h) = (edges.width, edges.height);
        let mut m = Plane::new(w, h);
        m.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            for (x, v) in row.iter_mut().enumerate() {
                let at = |x: usize, y: usize| edges.data[y.min(h - 1) * w + x.min(w - 1)];
                let gx = at(x + 1, y) - at(x.saturating_sub(1), y);
                let gy = at(x, y + 1) - at(x, y.saturating_sub(1));
                *v = smoothstep(th * 0.5, th, (gx * gx + gy * gy).sqrt());
            }
        });
        Some(m)
    } else {
        None
    };
    img.data.par_iter_mut().enumerate().for_each(|(i, p)| {
        let v = perceived.data[i];
        let diff = (v - blurred.data[i]).clamp(-limit, limit);
        let m = mask.as_ref().map_or(1.0, |m| m.data[i]);
        let sharp = (v + amount * diff * m).max(0.0);
        let gain = if v > 1e-4 { (sharp / v).powf(2.2) } else { 1.0 };
        p.iter_mut().for_each(|c| *c *= gain);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noisy() -> RgbImage {
        RgbImage::from_fn(96, 64, |x, y| {
            let n = (((x * 7919 + y * 104729) % 97) as f32 / 97.0 - 0.5) * 0.04;
            [0.2 + n, 0.2 - n * 0.5, 0.2 + n * 0.8]
        })
    }

    fn variance(img: &RgbImage, c: usize) -> f32 {
        let m: f32 = img.data.iter().map(|p| p[c]).sum::<f32>() / img.data.len() as f32;
        img.data.iter().map(|p| (p[c] - m).powi(2)).sum::<f32>() / img.data.len() as f32
    }

    #[test]
    fn noise_reduction_lowers_the_noise() {
        let mut img = noisy();
        let d = Detail {
            luminance_nr: 60.0,
            color_nr: 60.0,
            ..Detail::default()
        };
        noise_reduction(&mut img, &d, 1.0);
        assert!(variance(&img, 0) < variance(&noisy(), 0) * 0.5);
    }

    #[test]
    fn sharpening_raises_edge_contrast_and_spares_flat_areas() {
        let edge = RgbImage::from_fn(60, 20, |x, _| if x < 30 { [0.1; 3] } else { [0.5; 3] });
        let mut img = edge.clone();
        sharpen(
            &mut img,
            &Detail {
                sharpen_amount: 100.0,
                ..Detail::default()
            },
            1.0,
        );
        assert!(img.pixel(31, 10)[0] > edge.pixel(31, 10)[0]);
        assert!(img.pixel(28, 10)[0] < edge.pixel(28, 10)[0]);
        assert!((img.pixel(5, 10)[0] - 0.1).abs() < 1e-4);
        let mut small = edge.clone();
        sharpen(&mut small, &Detail::default(), 0.25);
        assert_eq!(small, edge, "pas d'accentuation sur un aperçu très réduit");
    }
}
