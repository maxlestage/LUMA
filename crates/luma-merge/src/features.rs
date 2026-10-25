//! Les points caractéristiques : les coins (détecteur de Harris), décrits par une
//! petite vignette normalisée (insensible à l'exposition), et appariés d'une photo à
//! l'autre (le meilleur candidat doit nettement battre le deuxième, et réciproquement).
//!
//! Les vignettes ne tournent pas avec l'image : l'appariement tolère une dizaine de
//! degrés de rotation, ce qui suffit pour un panorama ou une rafale à main levée.

use luma_core::{Plane, RgbImage};
use rayon::prelude::*;

use crate::geometry::Point;

/// Un coin et sa description (64 valeurs de moyenne nulle et de norme 1).
#[derive(Clone, Debug)]
pub struct Feature {
    pub x: f64,
    pub y: f64,
    pub descriptor: [f32; 64],
}

/// La luminance « perçue » d'une image caméra (racine carrée : le bruit et les écarts
/// d'exposition y pèsent moins).
pub fn luminance(img: &RgbImage) -> Plane {
    Plane::from_rgb(img, |p| ((p[0] + 2.0 * p[1] + p[2]) * 0.25).max(0.0).sqrt())
}

/// Les `count` coins les plus marqués, répartis sur toute l'image (grille de 8 × 8).
pub fn detect(l: &Plane, count: usize) -> Vec<Feature> {
    let (w, h) = (l.width, l.height);
    if w < 40 || h < 40 {
        return Vec::new();
    }
    let smooth = l.blur(1.0);
    // Gradients et tenseur de structure.
    let mut ixx = Plane::new(w, h);
    let mut iyy = Plane::new(w, h);
    let mut ixy = Plane::new(w, h);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            let gx = (smooth.data[i + 1] - smooth.data[i - 1]) * 0.5;
            let gy = (smooth.data[i + w] - smooth.data[i - w]) * 0.5;
            ixx.data[i] = gx * gx;
            iyy.data[i] = gy * gy;
            ixy.data[i] = gx * gy;
        }
    }
    let (ixx, iyy, ixy) = (ixx.blur(1.5), iyy.blur(1.5), ixy.blur(1.5));
    let response: Vec<f32> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (a, b, c) = (ixx.data[i], iyy.data[i], ixy.data[i]);
            a * b - c * c - 0.04 * (a + b) * (a + b)
        })
        .collect();
    // Maxima locaux (fenêtre 7 × 7), loin des bords pour que la vignette tienne.
    let margin = 14;
    let cells = 8;
    let mut buckets: Vec<Vec<(f32, usize, usize)>> = vec![Vec::new(); cells * cells];
    for y in margin..h.saturating_sub(margin) {
        for x in margin..w.saturating_sub(margin) {
            let r = response[y * w + x];
            if r <= 1e-7 {
                continue;
            }
            let mut is_max = true;
            'win: for dy in -3i32..=3 {
                for dx in -3i32..=3 {
                    if (dx, dy) != (0, 0) && response[(y as i32 + dy) as usize * w + (x as i32 + dx) as usize] >= r {
                        is_max = false;
                        break 'win;
                    }
                }
            }
            if is_max {
                let cell = (y * cells / h) * cells + x * cells / w;
                buckets[cell].push((r, x, y));
            }
        }
    }
    let per_cell = count.div_ceil(cells * cells).max(1);
    let blurred = l.blur(1.2);
    let mut points: Vec<(usize, usize)> = Vec::new();
    for mut b in buckets {
        b.sort_by(|a, c| c.0.total_cmp(&a.0));
        points.extend(b.into_iter().take(per_cell).map(|(_, x, y)| (x, y)));
    }
    points
        .par_iter()
        .filter_map(|&(x, y)| describe(&blurred, x as f64, y as f64))
        .collect()
}

/// La vignette 8 × 8 (pas de 2,5 pixels) autour d'un point, normalisée.
fn describe(l: &Plane, x: f64, y: f64) -> Option<Feature> {
    let mut d = [0.0f32; 64];
    for j in 0..8 {
        for i in 0..8 {
            let sx = x + (i as f64 - 3.5) * 2.5;
            let sy = y + (j as f64 - 3.5) * 2.5;
            d[j * 8 + i] = sample(l, sx, sy);
        }
    }
    let mean = d.iter().sum::<f32>() / 64.0;
    d.iter_mut().for_each(|v| *v -= mean);
    let norm = d.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm < 1e-4 {
        return None;
    }
    d.iter_mut().for_each(|v| *v /= norm);
    Some(Feature { x, y, descriptor: d })
}

fn sample(l: &Plane, x: f64, y: f64) -> f32 {
    let x = x.clamp(0.0, (l.width - 1) as f64);
    let y = y.clamp(0.0, (l.height - 1) as f64);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(l.width - 1), (y0 + 1).min(l.height - 1));
    let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
    let p = |x: usize, y: usize| l.data[y * l.width + x];
    let top = p(x0, y0) + (p(x1, y0) - p(x0, y0)) * fx;
    let bottom = p(x0, y1) + (p(x1, y1) - p(x0, y1)) * fx;
    top + (bottom - top) * fy
}

fn distance(a: &[f32; 64], b: &[f32; 64]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

/// Le meilleur et le deuxième voisin de chaque point de `a` dans `b`.
fn nearest(a: &[Feature], b: &[Feature]) -> Vec<Option<(usize, f32, f32)>> {
    a.par_iter()
        .map(|f| {
            let mut best = (usize::MAX, f32::MAX);
            let mut second = f32::MAX;
            for (j, g) in b.iter().enumerate() {
                let d = distance(&f.descriptor, &g.descriptor);
                if d < best.1 {
                    second = best.1;
                    best = (j, d);
                } else if d < second {
                    second = d;
                }
            }
            (best.0 != usize::MAX).then_some((best.0, best.1, second))
        })
        .collect()
}

/// Les paires (point de `a`, point de `b`) qui se ressemblent nettement plus qu'à tout
/// autre point, dans les deux sens.
pub fn matches(a: &[Feature], b: &[Feature]) -> Vec<(Point, Point)> {
    let ab = nearest(a, b);
    let ba = nearest(b, a);
    ab.iter()
        .enumerate()
        .filter_map(|(i, m)| {
            let (j, d1, d2) = (*m)?;
            let ratio_ok = d1 < 0.64 * d2;
            let mutual = ba[j].is_some_and(|(k, _, _)| k == i);
            (ratio_ok && mutual && d1 < 0.5).then(|| ((a[i].x, a[i].y), (b[j].x, b[j].y)))
        })
        .collect()
}
