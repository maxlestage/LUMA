//! Dématriçage et réduction des mosaïques de capteur.
//!
//! - Bayer : Malvar‑He‑Cutler (2004), des filtres linéaires 5×5 corrigés par le
//!   gradient : net, sans fausses couleurs marquées, et rapide (une seule passe).
//! - X‑Trans : le vert d'abord (moyenne des verts voisins), puis rouge et bleu par
//!   interpolation des différences de couleur, pondérée par la distance.
//! - Réduction : chaque bloc du motif devient un pixel, sans interpolation.
//!
//! Les bords sont prolongés par réflexion d'ordre 2, ce qui garde la parité du motif.

use luma_core::RgbImage;
use rayon::prelude::*;

use crate::Cfa;

#[inline]
fn reflect(i: isize, n: usize) -> usize {
    let n = n as isize;
    let i = if i < 0 {
        -i
    } else if i >= n {
        2 * (n - 1) - i
    } else {
        i
    };
    i.clamp(0, n - 1) as usize
}

pub fn bayer_mhc(raw: &[f32], w: usize, h: usize, cfa: &Cfa) -> RgbImage {
    let mut out = RgbImage::new(w, h);
    out.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let rows: [&[f32]; 5] = std::array::from_fn(|k| {
            let yy = reflect(y as isize + k as isize - 2, h);
            &raw[yy * w..(yy + 1) * w]
        });
        for (x, px) in row.iter_mut().enumerate() {
            let interior = x >= 2 && x + 2 < w;
            let at = |dx: isize, dy: isize| -> f32 {
                let r = rows[(dy + 2) as usize];
                if interior {
                    r[(x as isize + dx) as usize]
                } else {
                    r[reflect(x as isize + dx, w)]
                }
            };
            let c = at(0, 0);
            let color = cfa.color(x, y);
            let orth1 = at(-1, 0) + at(1, 0) + at(0, -1) + at(0, 1);
            let orth2 = at(-2, 0) + at(2, 0) + at(0, -2) + at(0, 2);
            let diag = at(-1, -1) + at(1, -1) + at(-1, 1) + at(1, 1);
            let rgb = if color == 1 {
                // Vert : les voisins de gauche et droite sont rouges ou bleus.
                let along_row = (5.0 * c + 4.0 * (at(-1, 0) + at(1, 0)) - (at(-2, 0) + at(2, 0)) - diag
                    + 0.5 * (at(0, -2) + at(0, 2)))
                    / 8.0;
                let along_col = (5.0 * c + 4.0 * (at(0, -1) + at(0, 1)) - (at(0, -2) + at(0, 2)) - diag
                    + 0.5 * (at(-2, 0) + at(2, 0)))
                    / 8.0;
                if cfa.color(x + 1, y) == 0 {
                    [along_row, c, along_col]
                } else {
                    [along_col, c, along_row]
                }
            } else {
                let g = (4.0 * c + 2.0 * orth1 - orth2) / 8.0;
                let other = (6.0 * c + 2.0 * diag - 1.5 * orth2) / 8.0;
                if color == 0 { [c, g, other] } else { [other, g, c] }
            };
            *px = [rgb[0].max(0.0), rgb[1].max(0.0), rgb[2].max(0.0)];
        }
    });
    out
}

/// Les voisins utiles d'une position du motif : (décalage x, décalage y, poids).
type Neighbors = Vec<(isize, isize, f32)>;

/// Pour chacune des 36 positions du motif X‑Trans : les verts du 3×3, puis les rouges
/// et les bleus du 5×5. Calculé une fois, au lieu de consulter le motif à chaque pixel.
fn xtrans_tables(cfa: &Cfa) -> Vec<[Neighbors; 3]> {
    let mut tables = Vec::with_capacity(cfa.width * cfa.height);
    for py in 0..cfa.height {
        for px in 0..cfa.width {
            let at = |dx: isize, dy: isize| {
                cfa.color(
                    (px as isize + dx).rem_euclid(cfa.width as isize) as usize,
                    (py as isize + dy).rem_euclid(cfa.height as isize) as usize,
                )
            };
            let mut green = Vec::new();
            for dy in -1..=1isize {
                for dx in -1..=1isize {
                    if (dx, dy) != (0, 0) && at(dx, dy) == 1 {
                        green.push((dx, dy, if dx == 0 || dy == 0 { 2.0 } else { 1.0 }));
                    }
                }
            }
            let mut by_color = [Vec::new(), Vec::new()];
            for dy in -2..=2isize {
                for dx in -2..=2isize {
                    let c = at(dx, dy);
                    if c != 1 {
                        by_color[c / 2].push((dx, dy, 1.0 / (1.0 + (dx * dx + dy * dy) as f32)));
                    }
                }
            }
            let [red, blue] = by_color;
            tables.push([green, red, blue]);
        }
    }
    tables
}

pub fn xtrans(raw: &[f32], w: usize, h: usize, cfa: &Cfa) -> RgbImage {
    let tables = xtrans_tables(cfa);
    let phase = |x: usize, y: usize| (y % cfa.height) * cfa.width + (x % cfa.width);
    // 1. Le vert partout.
    let mut green = vec![0.0f32; w * h];
    green.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, g) in row.iter_mut().enumerate() {
            let own = raw[y * w + x];
            if cfa.color(x, y) == 1 {
                *g = own;
                continue;
            }
            let interior = x >= 2 && y >= 2 && x + 2 < w && y + 2 < h;
            let (mut sum, mut n) = (0.0, 0.0);
            if interior {
                for &(dx, dy, wgt) in &tables[phase(x, y)][0] {
                    sum += raw[(y as isize + dy) as usize * w + (x as isize + dx) as usize] * wgt;
                    n += wgt;
                }
            } else {
                // Bords : la réflexion change la couleur du voisin, on relit le motif.
                for dy in -1..=1isize {
                    for dx in -1..=1isize {
                        let (xx, yy) = (reflect(x as isize + dx, w), reflect(y as isize + dy, h));
                        if (dx, dy) != (0, 0) && cfa.color(xx, yy) == 1 {
                            let wgt = if dx == 0 || dy == 0 { 2.0 } else { 1.0 };
                            sum += raw[yy * w + xx] * wgt;
                            n += wgt;
                        }
                    }
                }
            }
            *g = if n > 0.0 { sum / n } else { own };
        }
    });
    // 2. Rouge et bleu : différences de couleur au vert, voisinage 5×5.
    let mut out = RgbImage::new(w, h);
    out.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, px) in row.iter_mut().enumerate() {
            let own = cfa.color(x, y);
            let g = green[y * w + x];
            let interior = x >= 2 && y >= 2 && x + 2 < w && y + 2 < h;
            let mut rgb = [0.0f32, g, 0.0];
            for (slot, c) in [(1usize, 0usize), (2, 2)] {
                if own == c {
                    rgb[c] = raw[y * w + x];
                    continue;
                }
                let (mut sum, mut n) = (0.0, 0.0);
                if interior {
                    for &(dx, dy, wgt) in &tables[phase(x, y)][slot] {
                        let i = (y as isize + dy) as usize * w + (x as isize + dx) as usize;
                        sum += (raw[i] - green[i]) * wgt;
                        n += wgt;
                    }
                } else {
                    for dy in -2..=2isize {
                        for dx in -2..=2isize {
                            let (xx, yy) = (reflect(x as isize + dx, w), reflect(y as isize + dy, h));
                            if cfa.color(xx, yy) == c {
                                let wgt = 1.0 / (1.0 + (dx * dx + dy * dy) as f32);
                                sum += (raw[yy * w + xx] - green[yy * w + xx]) * wgt;
                                n += wgt;
                            }
                        }
                    }
                }
                rgb[c] = g + if n > 0.0 { sum / n } else { 0.0 };
            }
            *px = [rgb[0].max(0.0), rgb[1].max(0.0), rgb[2].max(0.0)];
        }
    });
    out
}

/// Un pixel par bloc `step × step`, moyenné sur une fenêtre `window × window` (qui
/// couvre une période entière du motif, donc les trois couleurs).
pub fn bin(raw: &[f32], w: usize, h: usize, cfa: &Cfa, step: usize, window: usize) -> RgbImage {
    let (ow, oh) = (w / step, h / step);
    let mut out = RgbImage::new(ow, oh);
    let margin = (window - step) / 2;
    out.data.par_chunks_mut(ow).enumerate().for_each(|(oy, row)| {
        let y0 = (oy * step).saturating_sub(margin);
        let y1 = (y0 + window).min(h);
        for (ox, px) in row.iter_mut().enumerate() {
            let x0 = (ox * step).saturating_sub(margin);
            let x1 = (x0 + window).min(w);
            let mut sum = [0.0f32; 3];
            let mut n = [0u32; 3];
            for y in y0..y1 {
                for x in x0..x1 {
                    let c = cfa.color(x, y);
                    sum[c] += raw[y * w + x];
                    n[c] += 1;
                }
            }
            *px = std::array::from_fn(|c| if n[c] > 0 { sum[c] / n[c] as f32 } else { 0.0 });
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une mosaïque Bayer qui échantillonne une image RGB.
    fn mosaic(img: &RgbImage, cfa: &Cfa) -> Vec<f32> {
        (0..img.height)
            .flat_map(|y| (0..img.width).map(move |x| (x, y)))
            .map(|(x, y)| img.pixel(x, y)[cfa.color(x, y)])
            .collect()
    }

    #[test]
    fn flat_colors_survive_demosaicing() {
        for pattern in ["RGGB", "BGGR", "GRBG", "GBRG"] {
            let cfa = Cfa::parse(pattern, 2, 2).unwrap();
            let img = RgbImage::from_fn(16, 12, |_, _| [0.6, 0.3, 0.1]);
            let out = bayer_mhc(&mosaic(&img, &cfa), 16, 12, &cfa);
            for p in &out.data {
                assert!(
                    (p[0] - 0.6).abs() < 1e-5 && (p[1] - 0.3).abs() < 1e-5 && (p[2] - 0.1).abs() < 1e-5,
                    "{pattern} {p:?}"
                );
            }
        }
    }

    #[test]
    fn gradients_are_reconstructed() {
        let cfa = Cfa::parse("RGGB", 2, 2).unwrap();
        let img = RgbImage::from_fn(32, 32, |x, y| [x as f32 / 32.0, 0.5, y as f32 / 32.0]);
        let out = bayer_mhc(&mosaic(&img, &cfa), 32, 32, &cfa);
        // Loin des bords, une rampe linéaire est reproduite exactement.
        for y in 4..28 {
            for x in 4..28 {
                let (a, b) = (out.pixel(x, y), img.pixel(x, y));
                assert!((0..3).all(|c| (a[c] - b[c]).abs() < 1e-4), "{x},{y}: {a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn xtrans_keeps_flat_colors_and_bins() {
        let cfa = Cfa::parse("GGRGGBGGBGGRBRGRBGGGBGGRGGRGGBRBGBRG", 6, 6).unwrap();
        let (w, h) = (24, 18);
        let raw: Vec<f32> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| [0.2, 0.5, 0.8][cfa.color(x, y)])
            .collect();
        for p in &xtrans(&raw, w, h, &cfa).data {
            assert!(
                (p[0] - 0.2).abs() < 1e-5 && (p[1] - 0.5).abs() < 1e-5 && (p[2] - 0.8).abs() < 1e-5,
                "{p:?}"
            );
        }
        let small = bin(&raw, w, h, &cfa, 3, 6);
        assert_eq!((small.width, small.height), (8, 6));
        assert!(
            small
                .data
                .iter()
                .all(|p| (p[0] - 0.2).abs() < 1e-6 && (p[2] - 0.8).abs() < 1e-6)
        );
    }
}
