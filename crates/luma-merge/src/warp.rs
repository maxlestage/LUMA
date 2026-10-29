//! La déformation des bords d'un panorama : plutôt que de rogner ses bords irréguliers,
//! on l'étire doucement pour qu'il remplisse tout son rectangle.
//!
//! Deux passes : chaque colonne étirée entre son premier et son dernier pixel couvert,
//! puis chaque ligne de même. Les bords suivis sont l'enveloppe intérieure des bords
//! réels (le plus contraignant sur une fenêtre, puis lissé), si bien qu'on ne lit jamais
//! hors des photos et que les lignes droites restent souples, sans vagues.

use luma_core::RgbImage;
use rayon::prelude::*;

/// Les bords lissés (début, fin) le long de `n` positions : `span(i)` donne la partie
/// couverte de la position `i` (`None` si rien), `reach` la largeur de la fenêtre.
fn envelope(n: usize, span: impl Fn(usize) -> Option<(f32, f32)>, reach: usize) -> Vec<(f32, f32)> {
    let raw: Vec<Option<(f32, f32)>> = (0..n).map(&span).collect();
    // Les positions sans rien prennent les bords de la plus proche qui en a.
    let known: Vec<usize> = (0..n).filter(|&i| raw[i].is_some()).collect();
    if known.is_empty() {
        return vec![(0.0, 0.0); n];
    }
    let filled: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            raw[i].unwrap_or_else(|| {
                let j = *known.iter().min_by_key(|&&k| k.abs_diff(i)).expect("au moins une");
                raw[j].expect("connue")
            })
        })
        .collect();
    // Le plus contraignant sur la fenêtre (le début le plus tardif, la fin la plus
    // précoce), puis une moyenne glissante, puis encore le réel : on reste dedans.
    let window = |i: usize| i.saturating_sub(reach)..(i + reach + 1).min(n);
    let tight: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            window(i).fold((f32::MIN, f32::MAX), |(a, b), j| {
                (a.max(filled[j].0), b.min(filled[j].1))
            })
        })
        .collect();
    (0..n)
        .map(|i| {
            let w = window(i);
            let k = w.len() as f32;
            let (a, b) = w.fold((0.0, 0.0), |(a, b), j| (a + tight[j].0, b + tight[j].1));
            ((a / k).max(filled[i].0), (b / k).min(filled[i].1))
        })
        .collect()
}

/// Un pixel lu entre deux voisins le long d'une colonne (ou d'une ligne).
fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|c| a[c] + (b[c] - a[c]) * t)
}

/// Étire chaque colonne de `img` entre ses bords couverts ; rend l'image et sa
/// couverture.
fn columns(img: &RgbImage, covered: &[bool]) -> (RgbImage, Vec<bool>) {
    let (w, h) = (img.width, img.height);
    let span = |x: usize| {
        let top = (0..h).find(|&y| covered[y * w + x])?;
        let bottom = (0..h).rev().find(|&y| covered[y * w + x])?;
        Some((top as f32, bottom as f32))
    };
    let edges = envelope(w, span, (w / 40).max(2));
    let mut out = RgbImage::new(w, h);
    let mut cover = vec![false; w * h];
    out.data
        .par_chunks_mut(w)
        .zip(cover.par_chunks_mut(w))
        .enumerate()
        .for_each(|(y, (row, cov))| {
            let t = y as f32 / (h - 1).max(1) as f32;
            for x in 0..w {
                let (a, b) = edges[x];
                if b < a {
                    continue;
                }
                let sy = a + (b - a) * t;
                let (y0, f) = (sy.floor() as usize, sy.fract());
                let y1 = (y0 + 1).min(h - 1);
                let (c0, c1) = (covered[y0 * w + x], covered[y1 * w + x]);
                if c0 || c1 {
                    let p0 = if c0 { img.data[y0 * w + x] } else { img.data[y1 * w + x] };
                    let p1 = if c1 { img.data[y1 * w + x] } else { p0 };
                    row[x] = lerp(p0, p1, f);
                    cov[x] = true;
                }
            }
        });
    (out, cover)
}

/// Transpose une image et sa couverture (pour faire les lignes comme des colonnes).
fn transpose(img: &RgbImage, covered: &[bool]) -> (RgbImage, Vec<bool>) {
    let (w, h) = (img.width, img.height);
    let out = RgbImage::from_fn(h, w, |x, y| img.data[x * w + y]);
    let cover = (0..w * h).map(|i| covered[(i % h) * w + i / h]).collect();
    (out, cover)
}

/// Le panorama `img` étiré pour remplir son rectangle (`covered` : ses pixels couverts).
/// Rend l'image et sa nouvelle couverture (presque tout, sauf là où une colonne ou une
/// ligne entière était vide).
pub fn boundary_warp(img: &RgbImage, covered: &[bool]) -> (RgbImage, Vec<bool>) {
    // Rien à étirer : une image vide, ou une couverture qui ne lui correspond pas.
    if img.data.is_empty() || covered.len() != img.data.len() {
        return (img.clone(), covered.to_vec());
    }
    let (cols, cover) = columns(img, covered);
    let (t, tc) = transpose(&cols, &cover);
    let (rows, rc) = columns(&t, &tc);
    transpose(&rows, &rc)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un « panorama » : une texture dont les bords sont ondulés et en biais, comme
    /// ceux de plusieurs vues assemblées.
    fn ragged() -> (RgbImage, Vec<bool>) {
        let (w, h) = (300, 120);
        let inside = |x: usize, y: usize| {
            let (fx, fy) = (x as f32, y as f32);
            let top = 8.0 + 6.0 * (fx * 0.05).sin() + fx * 0.02;
            let bottom = 112.0 - 5.0 * (fx * 0.07).cos() - (300.0 - fx) * 0.015;
            let left = 6.0 + fy * 0.05;
            let right = 292.0 - (120.0 - fy) * 0.04;
            fy >= top && fy <= bottom && fx >= left && fx <= right
        };
        let img = RgbImage::from_fn(w, h, |x, y| {
            if inside(x, y) {
                let v = 0.3 + 0.2 * ((x as f32 * 0.3).sin() * (y as f32 * 0.2).cos());
                [v, v * 0.9, v * 0.8]
            } else {
                [0.0; 3]
            }
        });
        let covered = (0..w * h).map(|i| inside(i % w, i / w)).collect();
        (img, covered)
    }

    #[test]
    fn a_ragged_panorama_fills_its_rectangle() {
        let (img, covered) = ragged();
        let before = covered.iter().filter(|c| **c).count() as f32 / covered.len() as f32;
        let (out, cover) = boundary_warp(&img, &covered);
        let after = cover.iter().filter(|c| **c).count() as f32 / cover.len() as f32;
        eprintln!("couverture : {:.1} % → {:.1} %", before * 100.0, after * 100.0);
        assert!(before < 0.9 && after > 0.995, "{before} → {after}");
        // Aucun pixel noir là où c'est couvert : on n'a jamais lu hors des photos.
        let black = out.data.iter().zip(&cover).filter(|(p, c)| **c && p[1] < 0.05).count();
        assert_eq!(black, 0);
        // Le rognage automatique n'a plus presque rien à enlever.
        let crop = crate::autocrop::auto_crop(&cover, out.width, out.height);
        assert!(
            crop.is_none_or(|c| (c.right - c.left) * (c.bottom - c.top) > 0.97),
            "{crop:?}"
        );
        // Une image déjà pleine ne bouge pas.
        let full = vec![true; img.width * img.height];
        let (same, _) = boundary_warp(&img, &full);
        let moved = same
            .data
            .iter()
            .zip(&img.data)
            .filter(|(a, b)| (a[1] - b[1]).abs() > 1e-5)
            .count();
        assert_eq!(moved, 0);
    }
}
