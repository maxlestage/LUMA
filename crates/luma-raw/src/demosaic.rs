//! Dématriçage et réduction des mosaïques de capteur.
//!
//! - Bayer : directionnel ([`bayer`]). Le vert d'abord, estimé en ligne et en colonne
//!   (moyenne des verts voisins corrigée par la courbure de la couleur du pixel,
//!   Hamilton‑Adams) et pondéré par la régularité de chaque direction : il suit les
//!   contours au lieu de les croiser, sans « fermeture éclair ». Puis rouge et bleu par
//!   les différences de couleur au vert, qui varient lentement. Malvar‑He‑Cutler (2004,
//!   filtres linéaires 5×5, une seule passe) reste disponible ([`bayer_mhc`]).
//! - X‑Trans : le vert d'abord (moyenne des verts voisins), repris dans le sens des
//!   contours (chaque voisin pondéré par la régularité de sa direction), puis rouge et
//!   bleu par interpolation des différences de couleur, pondérée par la distance.
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

/// Le dématriçage directionnel d'une mosaïque de Bayer (voir l'en‑tête du module).
pub fn bayer(raw: &[f32], w: usize, h: usize, cfa: &Cfa) -> RgbImage {
    if w < 5 || h < 5 {
        return bayer_mhc(raw, w, h, cfa);
    }
    // 1. Le vert partout.
    let mut green = vec![0.0f32; w * h];
    green.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let rows: [&[f32]; 5] = std::array::from_fn(|k| {
            let yy = reflect(y as isize + k as isize - 2, h);
            &raw[yy * w..(yy + 1) * w]
        });
        for (x, g) in row.iter_mut().enumerate() {
            let interior = x >= 2 && x + 2 < w;
            // Au bord, les voisins se lisent par réflexion ; ailleurs, directement.
            let at = |dx: isize, dy: isize| -> f32 {
                let r = rows[(dy + 2) as usize];
                if interior {
                    r[(x as isize + dx) as usize]
                } else {
                    r[reflect(x as isize + dx, w)]
                }
            };
            let c = at(0, 0);
            if cfa.color(x, y) == 1 {
                *g = c;
                continue;
            }
            // En ligne et en colonne : la moyenne des deux verts, corrigée par la courbure
            // de la couleur du pixel (rouge ou bleu, un pixel sur deux).
            let (gl, gr, gu, gd) = (at(-1, 0), at(1, 0), at(0, -1), at(0, 1));
            let curve_h = 2.0 * c - at(-2, 0) - at(2, 0);
            let curve_v = 2.0 * c - at(0, -2) - at(0, 2);
            let est_h = 0.5 * (gl + gr) + 0.25 * curve_h;
            let est_v = 0.5 * (gu + gd) + 0.25 * curve_v;
            // La régularité de chaque direction : l'écart des verts et la courbure.
            let grad_h = (gl - gr).abs() + curve_h.abs() + 1e-6;
            let grad_v = (gu - gd).abs() + curve_v.abs() + 1e-6;
            let (wh, wv) = (1.0 / (grad_h * grad_h), 1.0 / (grad_v * grad_v));
            *g = ((est_h * wh + est_v * wv) / (wh + wv)).max(0.0);
        }
    });
    // 2. Rouge et bleu, par les différences au vert (couleur − vert) de leurs voisins.
    let mut out = RgbImage::new(w, h);
    out.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let rows: [(&[f32], &[f32]); 3] = std::array::from_fn(|k| {
            let yy = reflect(y as isize + k as isize - 1, h);
            (&raw[yy * w..(yy + 1) * w], &green[yy * w..(yy + 1) * w])
        });
        for (x, px) in row.iter_mut().enumerate() {
            let interior = x >= 1 && x + 1 < w;
            let diff = |dx: isize, dy: isize| -> f32 {
                let (r, g) = rows[(dy + 1) as usize];
                let i = if interior {
                    (x as isize + dx) as usize
                } else {
                    reflect(x as isize + dx, w)
                };
                r[i] - g[i]
            };
            let (own, g) = (rows[1].0[x], rows[1].1[x]);
            let color = cfa.color(x, y);
            let rgb = if color == 1 {
                // Vert : rouge et bleu sont l'un dans la ligne, l'autre dans la colonne.
                let row_d = 0.5 * (diff(-1, 0) + diff(1, 0));
                let col_d = 0.5 * (diff(0, -1) + diff(0, 1));
                if cfa.color(x + 1, y) == 0 {
                    [g + row_d, g, g + col_d]
                } else {
                    [g + col_d, g, g + row_d]
                }
            } else {
                // Rouge ou bleu : l'autre couleur est aux quatre coins.
                let corners = 0.25 * (diff(-1, -1) + diff(1, -1) + diff(-1, 1) + diff(1, 1));
                if color == 0 {
                    [own, g, g + corners]
                } else {
                    [g + corners, g, own]
                }
            };
            *px = rgb.map(|v| v.max(0.0));
        }
    });
    out
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
    xtrans_with(raw, w, h, cfa, true)
}

/// Le dématriçage X‑Trans, le vert repris dans le sens des contours ou non (le calcul
/// d'avant, gardé pour les essais).
fn xtrans_with(raw: &[f32], w: usize, h: usize, cfa: &Cfa, directional: bool) -> RgbImage {
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
    // 1 bis. Le vert repris dans le sens des contours.
    let green = if directional {
        directional_green(raw, &green, w, h, cfa, &tables)
    } else {
        green
    };
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

/// Le vert X‑Trans repris dans le sens des contours : chaque vert voisin compte
/// d'autant moins que le premier vert (`g0`, la moyenne des voisins) varie dans sa
/// direction, de part et d'autre du pixel et un pas plus loin. Un contour n'est plus
/// moyenné en travers ; dans un aplat, les poids s'égalisent et le bruit reste moyenné.
/// Le seuil suit le niveau local, pour les ombres comme pour les hautes lumières.
fn directional_green(raw: &[f32], g0: &[f32], w: usize, h: usize, cfa: &Cfa, tables: &[[Neighbors; 3]]) -> Vec<f32> {
    let phase = |x: usize, y: usize| (y % cfa.height) * cfa.width + (x % cfa.width);
    let mut out = g0.to_vec();
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        if y < 2 || y + 2 >= h {
            return;
        }
        for (x, g) in row.iter_mut().enumerate() {
            if x < 2 || x + 2 >= w || cfa.color(x, y) == 1 {
                continue;
            }
            let at = |dx: isize, dy: isize| g0[(y as isize + dy) as usize * w + (x as isize + dx) as usize];
            let eps = 0.1 * (at(0, 0).max(0.0) + 0.002);
            let (mut sum, mut n) = (0.0f32, 0.0f32);
            for &(dx, dy, base) in &tables[phase(x, y)][0] {
                let grad = (at(dx, dy) - at(-dx, -dy)).abs() + (at(2 * dx, 2 * dy) - at(0, 0)).abs();
                let wgt = base / ((grad + eps) * (grad + eps));
                sum += raw[(y as isize + dy) as usize * w + (x as isize + dx) as usize] * wgt;
                n += wgt;
            }
            if n > 0.0 {
                *g = sum / n;
            }
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
    // La couleur de chaque photosite d'une fenêtre ne dépend que de la place de son coin
    // dans le motif : une table par place, calculée une fois (au lieu de deux restes de
    // division par photosite). Les mêmes sommes dans le même ordre : les mêmes bits.
    let (cw, ch) = (cfa.width, cfa.height);
    let tables: Vec<Vec<u8>> = (0..ch)
        .flat_map(|py| (0..cw).map(move |px| (px, py)))
        .map(|(px, py)| {
            (0..window)
                .flat_map(|dy| (0..window).map(move |dx| (dx, dy)))
                .map(|(dx, dy)| cfa.color(px + dx, py + dy) as u8)
                .collect()
        })
        .collect();
    if (step, window, cw, ch) == (2, 2, 2, 2) {
        // Bayer : chaque pixel, ses quatre photosites (jamais coupés par un bord), dans
        // l'ordre de la boucle générique.
        let t = &tables[0];
        let c = [
            usize::from(t[0]),
            usize::from(t[1]),
            usize::from(t[2]),
            usize::from(t[3]),
        ];
        out.data.par_chunks_mut(ow).enumerate().for_each(|(oy, row)| {
            let (top, bottom) = (
                &raw[2 * oy * w..2 * oy * w + w],
                &raw[(2 * oy + 1) * w..(2 * oy + 2) * w],
            );
            for ((px, t), b) in row
                .iter_mut()
                .zip(top.as_chunks::<2>().0)
                .zip(bottom.as_chunks::<2>().0)
            {
                let mut sum = [0.0f32; 3];
                let mut n = [0u32; 3];
                for (v, &k) in [t[0], t[1], b[0], b[1]].into_iter().zip(&c) {
                    sum[k] += v;
                    n[k] += 1;
                }
                *px = std::array::from_fn(|k| if n[k] > 0 { sum[k] / n[k] as f32 } else { 0.0 });
            }
        });
        return out;
    }
    out.data.par_chunks_mut(ow).enumerate().for_each(|(oy, row)| {
        let y0 = (oy * step).saturating_sub(margin);
        let y1 = (y0 + window).min(h);
        for (ox, px) in row.iter_mut().enumerate() {
            let x0 = (ox * step).saturating_sub(margin);
            let x1 = (x0 + window).min(w);
            let table = &tables[(y0 % ch) * cw + x0 % cw];
            let mut sum = [0.0f32; 3];
            let mut n = [0u32; 3];
            for y in y0..y1 {
                let colors = &table[(y - y0) * window..];
                let line = &raw[y * w + x0..y * w + x1];
                for (&v, &c) in line.iter().zip(colors) {
                    let c = usize::from(c);
                    sum[c] += v;
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

    /// La réduction d'avant, couleur par couleur pour chaque photosite : la table par
    /// place doit donner exactement les mêmes pixels.
    fn bin_reference(raw: &[f32], w: usize, h: usize, cfa: &Cfa, step: usize, window: usize) -> RgbImage {
        let (ow, oh) = (w / step, h / step);
        let margin = (window - step) / 2;
        RgbImage::from_fn(ow, oh, |ox, oy| {
            let y0 = (oy * step).saturating_sub(margin);
            let y1 = (y0 + window).min(h);
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
            std::array::from_fn(|c| if n[c] > 0 { sum[c] / n[c] as f32 } else { 0.0 })
        })
    }

    #[test]
    fn binning_by_tables_gives_the_same_pixels() {
        let xtrans = Cfa::parse("GGRGGBGGBGGRBRGRBGGGBGGRGGRGGBRBGBRG", 6, 6).unwrap();
        for (cfa, step, window) in [
            (Cfa::parse("RGGB", 2, 2).unwrap(), 2, 2),
            (Cfa::parse("GBRG", 2, 2).unwrap(), 2, 2),
            (xtrans, 3, 6),
        ] {
            for (w, h) in [(37, 23), (64, 48), (7, 6)] {
                let raw: Vec<f32> = (0..w * h).map(|i| ((i * 7919) % 1000) as f32 / 1000.0).collect();
                let a = bin(&raw, w, h, &cfa, step, window);
                let b = bin_reference(&raw, w, h, &cfa, step, window);
                assert!(
                    a.data == b.data && (a.width, a.height) == (b.width, b.height),
                    "{w}×{h} {step}"
                );
            }
        }
    }

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
    fn directional_keeps_flat_colors_and_ramps() {
        for pattern in ["RGGB", "BGGR", "GRBG", "GBRG"] {
            let cfa = Cfa::parse(pattern, 2, 2).unwrap();
            let img = RgbImage::from_fn(16, 12, |_, _| [0.6, 0.3, 0.1]);
            for p in &bayer(&mosaic(&img, &cfa), 16, 12, &cfa).data {
                assert!(
                    (p[0] - 0.6).abs() < 1e-5 && (p[1] - 0.3).abs() < 1e-5 && (p[2] - 0.1).abs() < 1e-5,
                    "{pattern} {p:?}"
                );
            }
            let ramp = RgbImage::from_fn(32, 32, |x, y| [x as f32 / 32.0, 0.5, y as f32 / 32.0]);
            let out = bayer(&mosaic(&ramp, &cfa), 32, 32, &cfa);
            for y in 4..28 {
                for x in 4..28 {
                    let (a, b) = (out.pixel(x, y), ramp.pixel(x, y));
                    assert!(
                        (0..3).all(|c| (a[c] - b[c]).abs() < 1e-4),
                        "{pattern} {x},{y}: {a:?} vs {b:?}"
                    );
                }
            }
        }
    }

    /// Des contours nets (verticaux, horizontaux, en biais) et des rayures fines : là où
    /// un dématriçage fait ses « fermetures éclair » et ses fausses couleurs.
    fn edges(w: usize, h: usize) -> RgbImage {
        RgbImage::from_fn(w, h, |x, y| {
            let (fx, fy) = (x as f32, y as f32);
            let mut v = if (x / 7) % 2 == 0 { 0.8 } else { 0.15 };
            if y > h / 3 {
                v = if ((fx + 0.6 * fy) / 9.0).floor() as i32 % 2 == 0 {
                    0.7
                } else {
                    0.1
                };
            }
            if y > 2 * h / 3 {
                v = if (y / 5) % 2 == 0 { 0.9 } else { 0.2 };
            }
            // Des aplats colorés, qui changent tous les 40 pixels.
            let tint = [[1.0, 0.6, 0.3], [0.4, 0.7, 1.0], [0.9, 0.9, 0.9]][(x / 40) % 3];
            [v * tint[0], v * tint[1], v * tint[2]]
        })
    }

    fn error(a: &RgbImage, b: &RgbImage) -> f64 {
        let (w, h) = (a.width, a.height);
        let mut sum = 0.0;
        let mut n = 0;
        for y in 3..h - 3 {
            for x in 3..w - 3 {
                for c in 0..3 {
                    sum += ((a.pixel(x, y)[c] - b.pixel(x, y)[c]) as f64).powi(2);
                    n += 1;
                }
            }
        }
        sum / n as f64
    }

    #[test]
    fn directional_beats_linear_filters_on_edges() {
        let cfa = Cfa::parse("RGGB", 2, 2).unwrap();
        let truth = edges(240, 180);
        let raw = mosaic(&truth, &cfa);
        let (fine, linear) = (bayer(&raw, 240, 180, &cfa), bayer_mhc(&raw, 240, 180, &cfa));
        let (e_fine, e_linear) = (error(&fine, &truth), error(&linear, &truth));
        let psnr = |e: f64| 10.0 * (1.0 / e).log10();
        assert!(
            e_fine < 0.7 * e_linear,
            "directionnel {:.2} dB, linéaire {:.2} dB",
            psnr(e_fine),
            psnr(e_linear)
        );
    }

    #[test]
    fn xtrans_follows_edges_without_hurting_flat_or_noisy_areas() {
        let cfa = Cfa::parse("GGRGGBGGBGGRBRGRBGGGBGGRGGRGGBRBGBRG", 6, 6).unwrap();
        let (w, h) = (240, 180);
        let psnr = |e: f64, peak: f64| 10.0 * (peak * peak / e).log10();
        let noise = |raw: &mut [f32], k: usize, amount: f32| {
            for (i, v) in raw.iter_mut().enumerate() {
                let n = ((i.wrapping_mul(k) as u32) >> 8) as f32 / 16_777_216.0 - 0.5;
                *v += amount * n;
            }
        };
        let both = |raw: &[f32], truth: &RgbImage, peak: f64| {
            (
                psnr(error(&xtrans(raw, w, h, &cfa), truth), peak),
                psnr(error(&xtrans_with(raw, w, h, &cfa, false), truth), peak),
            )
        };
        // Des contours nets : bien mieux.
        let edges = edges(w, h);
        let (new, old) = both(&mosaic(&edges, &cfa), &edges, 1.0);
        assert!(new > old + 2.5, "contours {new:.2} / {old:.2} dB");
        // Les mêmes, dix fois plus sombres et bruités : mieux aussi.
        let dark = RgbImage::from_fn(w, h, |x, y| edges.pixel(x, y).map(|v| v * 0.1));
        let mut raw = mosaic(&dark, &cfa);
        noise(&mut raw, 2_246_822_519, 0.004);
        let (new, old) = both(&raw, &dark, 0.1);
        assert!(new > old + 2.0, "sombre {new:.2} / {old:.2} dB");
        // Une image douce, propre puis bruitée : presque rien de perdu.
        let smooth = RgbImage::from_fn(w, h, |x, y| {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let t = 0.4 + 0.3 * (fx * 9.0).sin() * (fy * 7.0).cos();
            [t, 0.5 * t + 0.2 * fx, 0.3 + 0.2 * fy]
        });
        let mut raw = mosaic(&smooth, &cfa);
        let (new, old) = both(&raw, &smooth, 1.0);
        assert!(new > 60.0 && new > old - 1.0, "douce {new:.2} / {old:.2} dB");
        noise(&mut raw, 2_654_435_761, 0.03);
        let (new, old) = both(&raw, &smooth, 1.0);
        assert!(new > old - 0.3, "bruitée {new:.2} / {old:.2} dB");
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
