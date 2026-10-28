//! L'agrandissement d'une image, sans IA : un rééchantillonnage de Lanczos à trois lobes
//! (le plus net des filtres classiques), en lignes puis en colonnes, puis un garde‑fou
//! contre les halos : chaque pixel reste entre le plus sombre et le plus clair des
//! quatre pixels d'origine qui l'entourent. Les contours restent francs, sans liseré
//! clair ou sombre, et les aplats ne bougent pas.

use luma_core::RgbImage;
use rayon::prelude::*;

/// Le rayon du filtre, en pixels d'origine.
const LOBES: f32 = 3.0;

fn lanczos(x: f32) -> f32 {
    let x = x.abs();
    if x < 1e-6 {
        1.0
    } else if x >= LOBES {
        0.0
    } else {
        let px = std::f32::consts::PI * x;
        LOBES * px.sin() * (px / LOBES).sin() / (px * px)
    }
}

/// La position d'origine du centre du pixel de sortie `o`, pour `n` pixels d'origine
/// étirés sur `m`.
fn center(o: usize, n: usize, m: usize) -> f32 {
    (o as f32 + 0.5) * n as f32 / m as f32 - 0.5
}

/// Pour chaque pixel de sortie : les six pixels d'origine lus (bords prolongés, indices
/// calculés une fois pour toutes) et leurs six poids, de somme 1.
fn taps(n: usize, m: usize) -> Vec<([usize; 6], [f32; 6])> {
    (0..m)
        .map(|o| {
            let c = center(o, n, m);
            let first = c.floor() as isize - 2;
            let mut w: [f32; 6] = std::array::from_fn(|k| lanczos(c - (first + k as isize) as f32));
            let sum: f32 = w.iter().sum();
            w.iter_mut().for_each(|v| *v /= sum);
            (std::array::from_fn(|k| clamp_index(first + k as isize, n)), w)
        })
        .collect()
}

fn clamp_index(i: isize, n: usize) -> usize {
    i.clamp(0, n as isize - 1) as usize
}

/// Les deux pixels d'origine qui encadrent le centre du pixel de sortie `o`.
fn neighbours(o: usize, n: usize, m: usize) -> (usize, usize) {
    let f = center(o, n, m).floor() as isize;
    (clamp_index(f, n), clamp_index(f + 1, n))
}

/// `img` agrandie à `width × height` (au moins aussi grande que l'original sur chaque
/// côté ; sinon, utilisez une réduction par moyenne des surfaces).
pub fn enlarge(img: &RgbImage, width: usize, height: usize) -> RgbImage {
    let (sw, sh) = (img.width, img.height);
    let (width, height) = (width.max(1), height.max(1));
    if sw == 0 || sh == 0 || (width, height) == (sw, sh) {
        return img.clone();
    }
    // Les lignes d'abord, à la largeur voulue.
    let tx = taps(sw, width);
    let mut rows = RgbImage::new(width, sh);
    rows.data.par_chunks_mut(width).enumerate().for_each(|(y, out)| {
        let src = &img.data[y * sw..(y + 1) * sw];
        for (o, (idx, w)) in out.iter_mut().zip(&tx) {
            let mut acc = [0.0f32; 3];
            for (&i, wk) in idx.iter().zip(w) {
                let p = src[i];
                for c in 0..3 {
                    acc[c] += wk * p[c];
                }
            }
            *o = acc;
        }
    });
    // Puis les colonnes, et le garde‑fou sur les quatre voisins d'origine.
    let ty = taps(sh, height);
    let nx: Vec<(usize, usize)> = (0..width).map(|x| neighbours(x, sw, width)).collect();
    let mut out = RgbImage::new(width, height);
    out.data.par_chunks_mut(width).enumerate().for_each(|(y, line)| {
        let (idx, w) = &ty[y];
        // Ligne par ligne d'origine sur toute la largeur : le même ordre de sommes pour
        // chaque pixel qu'un pixel après l'autre, mais en boucles que le compilateur
        // vectorise.
        line.fill([0.0; 3]);
        for (&r, &wk) in idx.iter().zip(w) {
            let src = &rows.data[r * width..(r + 1) * width];
            for (o, p) in line.iter_mut().zip(src) {
                for c in 0..3 {
                    o[c] += wk * p[c];
                }
            }
        }
        let (y0, y1) = neighbours(y, sh, height);
        let (r0, r1) = (&img.data[y0 * sw..(y0 + 1) * sw], &img.data[y1 * sw..(y1 + 1) * sw]);
        for (o, &(x0, x1)) in line.iter_mut().zip(&nx) {
            let near = [r0[x0], r0[x1], r1[x0], r1[x1]];
            for c in 0..3 {
                let lo = near.iter().map(|p| p[c]).fold(f32::INFINITY, f32::min);
                let hi = near.iter().map(|p| p[c]).fold(f32::NEG_INFINITY, f32::max);
                // Quatre voisins NaN : pas de bornes (et `clamp` paniquerait).
                if lo <= hi {
                    o[c] = o[c].clamp(lo, hi);
                }
            }
        }
    });
    out
}

/// Bilinéaire, aux mêmes centres de pixels : la référence que l'agrandissement doit
/// battre.
#[cfg(test)]
fn bilinear(img: &RgbImage, width: usize, height: usize) -> RgbImage {
    let mut out = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            out.data[y * width + x] = img.sample(center(x, img.width, width), center(y, img.height, height));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une scène détaillée : des rayures de fréquences diverses, un disque net, un dégradé.
    fn scene(w: usize, h: usize) -> RgbImage {
        let mut img = RgbImage::new(w, h);
        for (i, p) in img.data.iter_mut().enumerate() {
            let (x, y) = ((i % w) as f32 / w as f32, (i / w) as f32 / h as f32);
            let stripes = 0.5 + 0.4 * (x * 40.0 + (y * 25.0).sin() * 3.0).sin();
            let disk = ((x - 0.6).powi(2) + (y - 0.4).powi(2) < 0.04) as u8 as f32;
            let v = 0.6 * stripes + 0.3 * disk + 0.1 * y;
            *p = [v, v * 0.8 + 0.1 * x, 0.3 + 0.2 * disk];
        }
        img
    }

    fn psnr(a: &RgbImage, b: &RgbImage) -> f32 {
        let mse: f32 = a
            .data
            .iter()
            .zip(&b.data)
            .flat_map(|(p, q)| (0..3).map(move |c| (p[c] - q[c]).powi(2)))
            .sum::<f32>()
            / (3 * a.data.len()) as f32;
        10.0 * (1.0 / mse).log10()
    }

    /// L'agrandissement tel qu'il était avant les indices précalculés et la somme des
    /// colonnes sur toute la ligne, pixel après pixel : la référence du résultat exact.
    fn reference(img: &RgbImage, width: usize, height: usize) -> RgbImage {
        let (sw, sh) = (img.width, img.height);
        let weights = |n: usize, m: usize| -> Vec<(isize, [f32; 6])> {
            (0..m)
                .map(|o| {
                    let c = center(o, n, m);
                    let first = c.floor() as isize - 2;
                    let mut w: [f32; 6] = std::array::from_fn(|k| lanczos(c - (first + k as isize) as f32));
                    let sum: f32 = w.iter().sum();
                    w.iter_mut().for_each(|v| *v /= sum);
                    (first, w)
                })
                .collect()
        };
        let (tx, ty) = (weights(sw, width), weights(sh, height));
        let mut rows = RgbImage::new(width, sh);
        for y in 0..sh {
            for (o, (first, w)) in tx.iter().enumerate() {
                let mut acc = [0.0f32; 3];
                for (k, wk) in w.iter().enumerate() {
                    let p = img.data[y * sw + clamp_index(first + k as isize, sw)];
                    for c in 0..3 {
                        acc[c] += wk * p[c];
                    }
                }
                rows.data[y * width + o] = acc;
            }
        }
        let mut out = RgbImage::new(width, height);
        for (y, (first, w)) in ty.iter().enumerate() {
            let cy = center(y, sh, height);
            let (y0, y1) = (
                clamp_index(cy.floor() as isize, sh),
                clamp_index(cy.floor() as isize + 1, sh),
            );
            for x in 0..width {
                let mut acc = [0.0f32; 3];
                for (k, wk) in w.iter().enumerate() {
                    let p = rows.data[clamp_index(first + k as isize, sh) * width + x];
                    for c in 0..3 {
                        acc[c] += wk * p[c];
                    }
                }
                let cx = center(x, sw, width);
                let (x0, x1) = (
                    clamp_index(cx.floor() as isize, sw),
                    clamp_index(cx.floor() as isize + 1, sw),
                );
                let near = [
                    img.data[y0 * sw + x0],
                    img.data[y0 * sw + x1],
                    img.data[y1 * sw + x0],
                    img.data[y1 * sw + x1],
                ];
                for c in 0..3 {
                    let lo = near.iter().map(|p| p[c]).fold(f32::INFINITY, f32::min);
                    let hi = near.iter().map(|p| p[c]).fold(f32::NEG_INFINITY, f32::max);
                    if lo <= hi {
                        acc[c] = acc[c].clamp(lo, hi);
                    }
                }
                out.data[y * width + x] = acc;
            }
        }
        out
    }

    #[test]
    fn enlarging_gives_exactly_the_same_pixels_as_before() {
        let mut odd = scene(13, 9);
        odd.data[40] = [f32::NAN; 3];
        for (img, w, h) in [
            (scene(40, 30), 97, 71),
            (scene(5, 4), 50, 9),
            (odd, 29, 30),
            (scene(64, 48), 128, 96),
        ] {
            let (a, b) = (enlarge(&img, w, h), reference(&img, w, h));
            let bits = |i: &RgbImage| i.data.iter().flat_map(|p| p.map(f32::to_bits)).collect::<Vec<_>>();
            assert_eq!(bits(&a), bits(&b), "{w}×{h}");
        }
    }

    #[test]
    fn enlarging_beats_bilinear() {
        let truth = scene(400, 300);
        let small = truth.resize_area(200, 150);
        let ours = enlarge(&small, 400, 300);
        let plain = bilinear(&small, 400, 300);
        let (a, b) = (psnr(&ours, &truth), psnr(&plain, &truth));
        assert!(a > b + 1.0, "Lanczos {a:.2} dB, bilinéaire {b:.2} dB");
    }

    #[test]
    fn a_hard_edge_gets_no_halo_and_flat_areas_stay() {
        // Un bord noir et blanc : ni liseré plus clair que le blanc, ni plus sombre que
        // le noir.
        let (w, h) = (40, 10);
        let mut img = RgbImage::new(w, h);
        for (i, p) in img.data.iter_mut().enumerate() {
            let v = if i % w < 20 { 0.1 } else { 0.9 };
            *p = [v, v, v];
        }
        let big = enlarge(&img, 3 * w, 3 * h);
        assert!(big.data.iter().all(|p| p[0] >= 0.1 - 1e-6 && p[0] <= 0.9 + 1e-6));
        // Loin du bord, les aplats gardent leur valeur exacte.
        assert!((big.data[3 * w * 15 + 10][0] - 0.1).abs() < 1e-6);
        assert!((big.data[3 * w * 15 + 110][0] - 0.9).abs() < 1e-6);
        // Le bord reste franc : la transition tient en quelques pixels agrandis.
        let row = &big.data[3 * w * 15..3 * w * 16];
        let soft = row.iter().filter(|p| p[0] > 0.15 && p[0] < 0.85).count();
        assert!(soft <= 3, "{soft} pixels de transition");
    }

    #[test]
    fn pixels_that_are_not_numbers_do_not_panic() {
        // Une zone de NaN (un calcul amont qui a divisé zéro par zéro) : le garde‑fou
        // n'a plus de bornes, l'agrandissement ne doit pas paniquer pour autant.
        let mut img = scene(12, 10);
        for y in 3..7 {
            for x in 4..8 {
                img.data[y * 12 + x] = [f32::NAN; 3];
            }
        }
        let big = enlarge(&img, 30, 25);
        assert_eq!((big.width, big.height), (30, 25));
        assert!(big.data[0].iter().all(|v| v.is_finite()));
    }

    #[test]
    fn same_size_is_a_copy_and_sizes_are_kept() {
        let img = scene(31, 17);
        assert_eq!(enlarge(&img, 31, 17), img);
        let big = enlarge(&img, 77, 40);
        assert_eq!((big.width, big.height), (77, 40));
    }
}
