//! Les retouches locales du panneau Masquage : la zone de chaque masque (dégradé
//! linéaire, dégradé radial, pinceau), puis ses curseurs appliqués en proportion.
//!
//! Elles agissent sur l'image linéaire de la scène, après la tonalité locale et avant
//! la courbe filmique, comme les curseurs du panneau Base : une exposition locale de
//! −1 assombrit d'un IL, exactement comme le curseur global.

use luma_color::luminance;
use luma_core::{Plane, RgbImage};
use luma_project::{Mask, Shape};
use rayon::prelude::*;

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x < e0 { 0.0 } else { 1.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// La zone d'un masque sur une image de `w × h` pixels : 1 où il agit pleinement, 0
/// où il n'agit pas (inversion et intensité comprises).
pub fn weights(mask: &Mask, w: usize, h: usize) -> Plane {
    let (fw, fh) = (w as f32, h as f32);
    let mut plane = Plane::new(w, h);
    match &mask.shape {
        Shape::Linear { start, end } => {
            // En pixels, pour que le fondu soit le même dans les deux directions.
            let (sx, sy) = (start.0 * fw, start.1 * fh);
            let (dx, dy) = (end.0 * fw - sx, end.1 * fh - sy);
            let len2 = (dx * dx + dy * dy).max(1e-6);
            plane.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
                let py = y as f32 + 0.5 - sy;
                for (x, v) in row.iter_mut().enumerate() {
                    let t = ((x as f32 + 0.5 - sx) * dx + py * dy) / len2;
                    *v = 1.0 - smoothstep(0.0, 1.0, t);
                }
            });
        }
        Shape::Radial {
            center,
            radius,
            angle,
            feather,
        } => {
            let (cx, cy) = (center.0 * fw, center.1 * fh);
            let (rx, ry) = ((radius.0 * fw).max(1.0), (radius.1 * fh).max(1.0));
            let (s, c) = (-angle.to_radians()).sin_cos();
            let inner = 1.0 - feather.clamp(0.0, 100.0) / 100.0;
            plane.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
                let py = y as f32 + 0.5 - cy;
                for (x, v) in row.iter_mut().enumerate() {
                    let px = x as f32 + 0.5 - cx;
                    let (u, q) = ((px * c - py * s) / rx, (px * s + py * c) / ry);
                    let r = (u * u + q * q).sqrt();
                    *v = 1.0 - smoothstep(inner, 1.0, r);
                }
            });
        }
        Shape::Brush { dabs } => {
            // Les touches dans l'ordre : chacune ajoute (ou gomme) selon son débit.
            for d in dabs {
                let r = (d.radius * fw).max(0.5);
                let inner = 1.0 - d.feather.clamp(0.0, 100.0) / 100.0;
                let flow = (d.flow / 100.0).clamp(0.0, 1.0);
                let (cx, cy) = (d.x * fw, d.y * fh);
                let x0 = (cx - r).floor().max(0.0) as usize;
                let x1 = ((cx + r).ceil().max(0.0) as usize).min(w);
                let y0 = (cy - r).floor().max(0.0) as usize;
                let y1 = ((cy + r).ceil().max(0.0) as usize).min(h);
                for y in y0..y1 {
                    let py = y as f32 + 0.5 - cy;
                    for x in x0..x1 {
                        let px = x as f32 + 0.5 - cx;
                        let a = (1.0 - smoothstep(inner, 1.0, (px * px + py * py).sqrt() / r)) * flow;
                        let v = &mut plane.data[y * w + x];
                        *v = if d.erase { *v * (1.0 - a) } else { *v + (1.0 - *v) * a };
                    }
                }
            }
        }
    }
    let amount = (mask.amount / 100.0).clamp(0.0, 1.0);
    let invert = mask.invert;
    plane.data.par_iter_mut().for_each(|v| {
        let m = if invert { 1.0 - *v } else { *v };
        *v = m * amount;
    });
    plane
}

/// Applique les masques actifs à l'image (linéaire, espace de travail).
pub fn apply(img: &mut RgbImage, masks: &[Mask]) {
    let active: Vec<&Mask> = masks.iter().filter(|m| m.is_active()).collect();
    if active.is_empty() {
        return;
    }
    let (w, h) = (img.width, img.height);
    let diag = ((w * w + h * h) as f32).sqrt();
    let stops = Plane::from_rgb(img, |p| (luminance(p).max(1e-6) / 0.18).log2());
    // La base de la clarté, comme pour la Clarté globale : une fois pour tous les masques.
    let clarity_base = active
        .iter()
        .any(|m| m.local.clarity != 0.0)
        .then(|| stops.guided(0.008 * diag, 0.5));
    for mask in active {
        let weights = weights(mask, w, h);
        let l = &mask.local;
        let k = |v: f32| v / 100.0;
        let (contrast, hl, sh, wh, bl) = (k(l.contrast), k(l.highlights), k(l.shadows), k(l.whites), k(l.blacks));
        let (temp, tint, sat, clarity) = (k(l.temperature), k(l.tint), k(l.saturation), k(l.clarity));
        img.data
            .par_iter_mut()
            .enumerate()
            .zip(weights.data.par_iter().zip(stops.data.par_iter()))
            .for_each(|((i, p), (&m, &s))| {
                if m <= 0.0 {
                    return;
                }
                // Les tons, en IL : la même famille de courbes que le panneau Base.
                let mut d = l.exposure;
                d += contrast * 0.35 * s * (-(s / 5.0).powi(2)).exp();
                d += hl * 1.2 * smoothstep(-0.5, 2.5, s);
                d += sh * 1.5 * (1.0 - smoothstep(-4.0, -0.5, s));
                d += wh * 0.8 * smoothstep(1.0, 3.0, s);
                d += bl * 0.8 * (1.0 - smoothstep(-6.0, -3.0, s));
                if let Some(base) = &clarity_base {
                    let mid = (-(s / 2.5).powi(2)).exp();
                    d += clarity * 0.7 * (s - base.data[i]) * mid;
                }
                let g = (d * m).clamp(-6.0, 6.0).exp2();
                // Température (bleu ↔ jaune) et teinte (vert ↔ magenta).
                let gr = (temp * 0.35 * m).exp2();
                let gb = (-temp * 0.35 * m).exp2();
                let gg = (-tint * 0.25 * m).exp2();
                let mut c = [p[0] * g * gr, p[1] * g * gg, p[2] * g * gb];
                if sat != 0.0 {
                    let y = luminance(c);
                    let f = (1.0 + sat * m).max(0.0);
                    c = c.map(|v| y + (v - y) * f);
                }
                *p = c;
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luma_project::{Dab, Local};

    #[test]
    fn linear_gradient_fades_from_start_to_end() {
        let m = Mask::linear(); // du haut (0,15) au milieu (0,5)
        let p = weights(&m, 10, 100);
        assert!(p.data[5 * 10] > 0.99, "plein effet au‑dessus du départ");
        assert!(p.data[90 * 10] < 0.01, "rien après l'arrivée");
        let mid = p.data[32 * 10];
        assert!(mid > 0.3 && mid < 0.7, "{mid}");
    }

    #[test]
    fn radial_inverted_and_amount() {
        let mut m = Mask::radial();
        let p = weights(&m, 100, 100);
        assert!(p.data[50 * 100 + 50] > 0.99 && p.data[0] < 0.01);
        m.invert = true;
        m.amount = 50.0;
        let p = weights(&m, 100, 100);
        assert!((p.data[0] - 0.5).abs() < 0.01 && p.data[50 * 100 + 50] < 0.01);
    }

    #[test]
    fn brush_paints_and_erases() {
        let mut m = Mask::brush();
        let dab = Dab {
            x: 0.5,
            y: 0.5,
            radius: 0.2,
            feather: 0.0,
            flow: 100.0,
            erase: false,
        };
        m.shape = Shape::Brush {
            dabs: vec![
                dab,
                Dab {
                    erase: true,
                    radius: 0.05,
                    ..dab
                },
            ],
        };
        let p = weights(&m, 100, 100);
        assert!(p.data[50 * 100 + 40] > 0.99, "peint");
        assert!(p.data[50 * 100 + 50] < 0.01, "gommé au centre");
        assert!(p.data[0] == 0.0, "rien ailleurs");
    }

    #[test]
    fn local_exposure_is_a_true_stop_inside_the_mask_only() {
        let mut img = RgbImage::from_fn(20, 20, |_, _| [0.18, 0.18, 0.18]);
        let mut m = Mask::radial();
        m.shape = Shape::Radial {
            center: (0.5, 0.5),
            radius: (0.2, 0.2),
            angle: 0.0,
            feather: 0.0,
        };
        m.local = Local {
            exposure: -1.0,
            ..Local::default()
        };
        apply(&mut img, &[m]);
        assert!((img.data[10 * 20 + 10][0] - 0.09).abs() < 1e-4);
        assert!((img.data[0][0] - 0.18).abs() < 1e-6);
    }

    #[test]
    fn temperature_warms_and_saturation_keeps_luminance() {
        let mut img = RgbImage::from_fn(4, 4, |_, _| [0.2, 0.2, 0.2]);
        let mut m = Mask::linear();
        m.shape = Shape::Linear {
            start: (0.0, 2.0),
            end: (0.0, 3.0),
        }; // partout
        m.local = Local {
            temperature: 50.0,
            ..Local::default()
        };
        apply(&mut img, std::slice::from_ref(&m));
        let p = img.data[0];
        assert!(p[0] > p[1] && p[1] > p[2], "plus chaud : {p:?}");
        let mut img = RgbImage::from_fn(4, 4, |_, _| [0.3, 0.2, 0.1]);
        let y0 = luminance(img.data[0]);
        m.local = Local {
            saturation: -100.0,
            ..Local::default()
        };
        apply(&mut img, &[m]);
        let p = img.data[0];
        assert!((p[0] - p[2]).abs() < 1e-5 && (luminance(p) - y0).abs() < 1e-5, "{p:?}");
    }
}
