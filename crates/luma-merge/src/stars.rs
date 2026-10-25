//! Les étoiles : détection (centroïdes au dixième de pixel), puis alignement d'une
//! pose sur l'autre. Le ciel tourne autour du pôle et se décale entre deux poses : une
//! similitude (rotation, translation) explique ce mouvement sur le champ d'un objectif.

use luma_core::{Plane, RgbImage};

use crate::geometry::{Homography, Model, Point, ransac};

/// Une étoile : position (pixels) et éclat (somme du signal au‑dessus du fond).
#[derive(Clone, Copy, Debug)]
pub struct Star {
    pub x: f64,
    pub y: f64,
    pub flux: f32,
}

/// Les étoiles d'une pose, les plus brillantes d'abord (au plus `limit`).
pub fn detect(img: &RgbImage, limit: usize) -> Vec<Star> {
    let l = Plane::from_rgb(img, |p| p[0] + p[1] + p[2]);
    let (w, h) = (l.width, l.height);
    if w < 32 || h < 32 {
        return Vec::new();
    }
    // Le fond du ciel (dégradés, pollution lumineuse) retiré par un grand flou.
    let background = l.blur(12.0);
    let residual: Vec<f32> = l.data.iter().zip(&background.data).map(|(a, b)| a - b).collect();
    // Le bruit : écart absolu médian.
    let mut sample: Vec<f32> = residual.iter().step_by(7).map(|v| v.abs()).collect();
    let mid = sample.len() / 2;
    let mad = *sample.select_nth_unstable_by(mid, f32::total_cmp).1;
    let threshold = (mad * 1.4826 * 6.0).max(1e-5);
    let mut stars = Vec::new();
    let margin = 8;
    for y in margin..h - margin {
        for x in margin..w - margin {
            let v = residual[y * w + x];
            if v < threshold {
                continue;
            }
            let mut is_max = true;
            'win: for dy in -2i32..=2 {
                for dx in -2i32..=2 {
                    if (dx, dy) == (0, 0) {
                        continue;
                    }
                    let n = residual[(y as i32 + dy) as usize * w + (x as i32 + dx) as usize];
                    if n > v || (n == v && (dy < 0 || (dy == 0 && dx < 0))) {
                        is_max = false;
                        break 'win;
                    }
                }
            }
            if !is_max {
                continue;
            }
            // Centroïde pondéré sur 7 × 7.
            let (mut sx, mut sy, mut sum) = (0.0f64, 0.0f64, 0.0f64);
            for dy in -3i32..=3 {
                for dx in -3i32..=3 {
                    let r = f64::from(residual[(y as i32 + dy) as usize * w + (x as i32 + dx) as usize].max(0.0));
                    sx += r * f64::from(dx);
                    sy += r * f64::from(dy);
                    sum += r;
                }
            }
            if sum > 0.0 {
                stars.push(Star {
                    x: x as f64 + sx / sum,
                    y: y as f64 + sy / sum,
                    flux: sum as f32,
                });
            }
        }
    }
    stars.sort_by(|a, b| b.flux.total_cmp(&a.flux));
    stars.truncate(limit);
    stars
}

/// La transformation qui envoie les étoiles de `reference` sur celles de `other`.
/// `guess` : une estimation de départ (la pose précédente), l'identité sinon.
pub fn align(reference: &[Star], other: &[Star], guess: &Homography) -> Option<(Homography, usize)> {
    if reference.len() < 6 || other.len() < 6 {
        return None;
    }
    // 1. Le décalage le plus fréquent entre les étoiles brillantes (vote sur 3 px).
    let bright_ref = &reference[..reference.len().min(60)];
    let bright_other = &other[..other.len().min(60)];
    let mut votes: std::collections::HashMap<(i64, i64), (usize, f64, f64)> = std::collections::HashMap::new();
    for r in bright_ref {
        let Some((px, py)) = guess.apply((r.x, r.y)) else {
            continue;
        };
        for o in bright_other {
            let (dx, dy) = (o.x - px, o.y - py);
            let key = ((dx / 3.0).round() as i64, (dy / 3.0).round() as i64);
            let e = votes.entry(key).or_insert((0, 0.0, 0.0));
            e.0 += 1;
            e.1 += dx;
            e.2 += dy;
        }
    }
    let (_, (count, sdx, sdy)) = votes.into_iter().max_by_key(|(_, v)| v.0)?;
    if count < 3 {
        return None;
    }
    let shift = Homography::translation(sdx / count as f64, sdy / count as f64);
    let mut model = guess.then(&shift);
    // 2. Paires au plus proche voisin, puis similitude robuste ; deux passes.
    let mut inliers = 0;
    for radius in [6.0, 2.5] {
        let pairs: Vec<(Point, Point)> = reference
            .iter()
            .filter_map(|r| {
                let p = model.apply((r.x, r.y))?;
                let (best, d2) = other
                    .iter()
                    .map(|o| (o, (o.x - p.0).powi(2) + (o.y - p.1).powi(2)))
                    .min_by(|a, b| a.1.total_cmp(&b.1))?;
                (d2 < radius * radius).then_some(((r.x, r.y), (best.x, best.y)))
            })
            .collect();
        let (h, found) = ransac(&pairs, Model::Similarity, 1.5, 300)?;
        model = h;
        inliers = found.len();
    }
    (inliers >= 6).then_some((model, inliers))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Rng;

    /// Un champ d'étoiles gaussiennes sur un fond bruité.
    pub fn starfield(w: usize, h: usize, stars: &[(f64, f64, f32)], seed: u64) -> RgbImage {
        let mut img = RgbImage::from_fn(w, h, |x, y| {
            let n = (noise(x, y, seed) - 0.5) * 0.004;
            [0.02 + n, 0.02 + n, 0.025 + n]
        });
        for &(sx, sy, f) in stars {
            let (x0, y0) = (sx.floor() as i64, sy.floor() as i64);
            for y in (y0 - 4).max(0)..(y0 + 5).min(h as i64) {
                for x in (x0 - 4).max(0)..(x0 + 5).min(w as i64) {
                    let d2 = (x as f64 - sx).powi(2) + (y as f64 - sy).powi(2);
                    let v = f * (-d2 / (2.0 * 1.2f64.powi(2))).exp() as f32;
                    let p = &mut img.data[y as usize * w + x as usize];
                    p.iter_mut().for_each(|c| *c += v);
                }
            }
        }
        img
    }

    /// Un bruit uniforme reproductible dans [0, 1).
    pub fn noise(x: usize, y: usize, seed: u64) -> f32 {
        let mut h =
            (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ seed;
        h ^= h >> 31;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 29;
        (h % 100_000) as f32 / 100_000.0
    }

    pub fn random_stars(n: usize, w: usize, h: usize, seed: u64) -> Vec<(f64, f64, f32)> {
        let mut rng = Rng::new(seed);
        (0..n)
            .map(|_| {
                (
                    20.0 + rng.below((w - 40) * 10) as f64 / 10.0,
                    20.0 + rng.below((h - 40) * 10) as f64 / 10.0,
                    0.05 + rng.below(1000) as f32 / 1000.0 * 0.6,
                )
            })
            .collect()
    }

    #[test]
    fn stars_are_found_at_subpixel_precision_and_aligned() {
        let (w, h) = (640, 420);
        let truth = random_stars(150, w, h, 3);
        let motion = Homography::similarity(1.0, 0.006, 7.3, -4.1);
        let moved: Vec<(f64, f64, f32)> = truth
            .iter()
            .map(|&(x, y, f)| {
                let (u, v) = motion.apply((x, y)).unwrap();
                (u, v, f)
            })
            .collect();
        let a = detect(&starfield(w, h, &truth, 1), 300);
        let b = detect(&starfield(w, h, &moved, 2), 300);
        assert!(a.len() > 100, "{} étoiles", a.len());
        let (m, inliers) = align(&a, &b, &Homography::IDENTITY).unwrap();
        assert!(inliers > 60, "{inliers}");
        for p in [(100.0, 100.0), (500.0, 300.0)] {
            let (x, y) = m.apply(p).unwrap();
            let (u, v) = motion.apply(p).unwrap();
            assert!(
                (x - u).abs() < 0.15 && (y - v).abs() < 0.15,
                "{:?} / {:?}",
                (x, y),
                (u, v)
            );
        }
    }
}
