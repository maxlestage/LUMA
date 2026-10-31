//! Panneau Détail : réduction du bruit et accentuation.
//!
//! - La réduction du bruit travaille avant la tonalité, dans un espace « racine carrée »
//!   (luminance et deux différences de couleur) : la luminance par un filtre guidé qui
//!   garde les contours, la couleur par un flou plus large, guidé lui aussi pour ne pas
//!   baver sur les bords colorés.
//!   Au choix, la luminance passe par des ondelettes « à trous » : l'image est
//!   découpée en cinq échelles de détail, et chacune perd ce qui ne dépasse pas le bruit
//!   mesuré sur la plus fine (un Wiener local). Les contours, forts à toutes les
//!   échelles, restent ; le grain, faible et fin, s'en va.
//! - L'accentuation est un masque flou sur la luminance perçue, avec un plafond contre
//!   les halos (Détail) et un masque de contours (Masquage). Au choix, une
//!   déconvolution : Richardson‑Lucy avec un flou gaussien du Rayon, qui défait le flou
//!   au lieu d'en creuser l'écart ; même plafond, même Masquage.
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
    // luminance et deux différences de couleur, comme un YCbCr. Chaque pixel est
    // recalculé à la demande plutôt que gardé : ni copie encodée ni plan pleine taille
    // de couleur, seulement la luminance lissée et la couleur à demi‑résolution.
    let (w, h) = (img.width, img.height);
    let ycc = |p: [f32; 3]| {
        let e = p.map(|v| v.max(0.0).sqrt());
        let y = WR * e[0] + WG * e[1] + WB * e[2];
        (y, e[0] - y, e[2] - y)
    };
    let luma = do_lum.then(|| {
        let y = Plane {
            width: w,
            height: h,
            data: img.data.par_iter().map(|p| ycc(*p).0).collect(),
        };
        if d.luminance_nr_wavelets {
            return wavelet_denoise(&y, lum, d.luminance_nr_detail / 100.0, d.luminance_nr_contrast / 100.0);
        }
        // Le détail garde les petites structures : il baisse le seuil de lissage.
        let eps = (lum * lum * 0.0015 * (1.0 - 0.8 * d.luminance_nr_detail / 100.0)).max(1e-7);
        let smooth = y.guided(lum_sigma, eps);
        y.zip(&smooth, |orig, s| orig + (s - orig) * lum.sqrt())
    });
    let chroma = do_col.then(|| {
        let eps = 0.0001 + 0.001 * (1.0 - d.color_nr_detail / 100.0);
        let sigma = col_sigma * (0.6 + 0.8 * d.color_nr_smoothness / 100.0);
        // La couleur se lisse à demi‑résolution (comme un JPEG en 4:2:0, l'œil ne voit pas
        // la différence) ; petit rayon : flou simple, grand rayon : flou guidé.
        let (cr, cb) = half_chroma(img, &ycc);
        let smooth = |small: Plane| {
            if sigma < 5.0 {
                small.blur(sigma / 2.0)
            } else {
                small.guided(sigma / 2.0, eps)
            }
        };
        (smooth(cr), smooth(cb))
    });
    let columns = chroma.as_ref().map(|c| c.0.upsample_columns(w)).unwrap_or_default();
    let Some((cr, cb)) = &chroma else {
        // La luminance seule : chaque pixel garde sa couleur.
        let Some(l) = &luma else { return };
        img.data.par_chunks_mut(w).enumerate().for_each(|(row_y, row)| {
            for (p, &y) in row.iter_mut().zip(&l.data[row_y * w..(row_y + 1) * w]) {
                let (_, cr, cb) = ycc(*p);
                let (r, b) = (y + cr, y + cb);
                let g = (y - WR * r - WB * b) / WG;
                *p = [r.max(0.0).powi(2), g.max(0.0).powi(2), b.max(0.0).powi(2)];
            }
        });
        return;
    };
    // La couleur lissée : par ligne, la luminance (lissée, ou celle du pixel) puis le
    // retour, chacun en une boucle simple que le processeur mène par colonnes ; trois
    // tampons par fil, repris de ligne en ligne. Les mêmes calculs, dans le même ordre,
    // que pixel par pixel : les mêmes bits.
    img.data.par_chunks_mut(w).enumerate().for_each_init(
        || (vec![0.0f32; w], vec![0.0f32; w], vec![0.0f32; w]),
        |(ys, up_r, up_b), (row_y, row)| {
            cr.upsample_row(row_y, h, &columns, up_r);
            cb.upsample_row(row_y, h, &columns, up_b);
            match &luma {
                Some(l) => ys.copy_from_slice(&l.data[row_y * w..(row_y + 1) * w]),
                None => {
                    for (y, p) in ys.iter_mut().zip(row.iter()) {
                        let e = p.map(|v| v.max(0.0).sqrt());
                        *y = WR * e[0] + WG * e[1] + WB * e[2];
                    }
                }
            }
            for (((p, &y), &cr), &cb) in row.iter_mut().zip(ys.iter()).zip(up_r.iter()).zip(up_b.iter()) {
                let (r, b) = (y + cr, y + cb);
                let g = (y - WR * r - WB * b) / WG;
                *p = [r.max(0.0).powi(2), g.max(0.0).powi(2), b.max(0.0).powi(2)];
            }
        },
    );
}

/// Bruit blanc d'écart type 1 : son écart type dans chaque échelle d'ondelettes
/// « à trous » (noyau B3, 1 4 6 4 1).
const WAVELET_NOISE: [f32; 5] = [0.889, 0.200, 0.086, 0.041, 0.020];

/// Débruite un plan par ondelettes « à trous » : `strength`, `detail` et `contrast` de
/// 0 à 1. Le bruit est mesuré sur l'échelle la plus fine (écart absolu médian, que les
/// contours, rares, ne faussent pas) ; chaque échelle perd en douceur ce dont
/// l'énergie locale reste proche de celle de ce bruit. Le détail ménage les deux échelles fines, le contraste les
/// grandes.
fn wavelet_denoise(y: &Plane, strength: f32, detail: f32, contrast: f32) -> Plane {
    let (w, h) = (y.width, y.height);
    // Quatre tampons pour toutes les échelles : l'image grossière, son lissage, la passe
    // des lignes, la somme. Les coefficients fins (grossière − lissée) et leur carré
    // se calculent à la lecture, l'atténuation et le cumul dans la passe des colonnes.
    let mut coarse = y.data.clone();
    let mut smooth = vec![0.0; w * h];
    let mut rows = vec![0.0; w * h];
    let mut out = Plane::new(w, h);
    let mut sigma = 0.0;
    for (level, noise) in WAVELET_NOISE.iter().enumerate() {
        let step = 1 << level;
        if 2 * step >= w.min(h) {
            break;
        }
        atrous_rows(|i| coarse[i], w, step, &mut rows);
        atrous_cols(&rows, w, h, step, &mut smooth, |_, o, v| *o = v);
        let (c, s) = (&coarse, &smooth);
        if level == 0 {
            sigma = median_abs(c.len(), |i| c[i] - s[i]) / 0.6745 / noise;
        }
        let keep = if level < 2 {
            1.0 - 0.7 * detail
        } else {
            1.0 - 0.8 * contrast
        };
        // Un Wiener local : là où l'énergie du voisinage (le carré des coefficients,
        // flouté à la taille de l'échelle) dépasse à peine celle du bruit, le
        // coefficient s'efface ; une texture ou un contour, bien plus énergiques, restent.
        let t = 3.0 * strength * keep * sigma * noise;
        if t > 0.0 {
            atrous_rows(
                |i| {
                    let f = c[i] - s[i];
                    f * f
                },
                w,
                step,
                &mut rows,
            );
            atrous_cols(&rows, w, h, step, &mut out.data, |i, o, e| {
                *o += (c[i] - s[i]) * ((e - t * t) / e.max(1e-12)).max(0.0);
            });
        } else {
            out.data.par_iter_mut().enumerate().for_each(|(i, o)| *o += c[i] - s[i]);
        }
        std::mem::swap(&mut coarse, &mut smooth);
    }
    out.data.par_iter_mut().zip(&coarse).for_each(|(o, c)| *o += c);
    out
}

/// Le lissage B3 (1 4 6 4 1, sur 16) à prises espacées de `step` pixels.
const B3: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];

fn b3(v: [f32; 5]) -> f32 {
    B3[0] * (v[0] + v[4]) + B3[1] * (v[1] + v[3]) + B3[2] * v[2]
}

/// L'indice `i` reflété dans `0..n`.
fn reflect(i: isize, n: usize) -> usize {
    let n = n as isize;
    let i = if i < 0 { -i } else { i };
    (if i >= n { 2 * (n - 1) - i } else { i }).clamp(0, n - 1) as usize
}

/// La passe des lignes du lissage B3 (bords reflétés) ; `at(i)` donne la valeur lue à
/// l'indice `i`.
fn atrous_rows(at: impl Fn(usize) -> f32 + Sync, w: usize, step: usize, dst: &mut [f32]) {
    dst.par_chunks_mut(w).enumerate().for_each(|(yy, row)| {
        let base = yy * w;
        for (x, o) in row.iter_mut().enumerate() {
            // L'intérieur, sans réflexion ; les bords la prennent.
            *o = if x >= 2 * step && x + 2 * step < w {
                b3(std::array::from_fn(|k| at(base + x + k * step - 2 * step)))
            } else {
                b3(std::array::from_fn(|k| {
                    at(base + reflect(x as isize + (k as isize - 2) * step as isize, w))
                }))
            };
        }
    });
}

/// La passe des colonnes du lissage B3 (bords reflétés) ; `put(i, sortie, lissé)` écrit
/// le résultat de l'indice `i`.
fn atrous_cols(
    rows: &[f32],
    w: usize,
    h: usize,
    step: usize,
    dst: &mut [f32],
    put: impl Fn(usize, &mut f32, f32) + Sync,
) {
    dst.par_chunks_mut(w).enumerate().for_each(|(yy, row)| {
        let lines: [&[f32]; 5] = std::array::from_fn(|k| {
            let l = reflect(yy as isize + (k as isize - 2) * step as isize, h) * w;
            &rows[l..l + w]
        });
        for (x, o) in row.iter_mut().enumerate() {
            put(yy * w + x, o, b3(std::array::from_fn(|k| lines[k][x])));
        }
    });
}

/// La médiane des valeurs absolues des `n` valeurs `at(i)` (sur au plus 2¹⁸ valeurs
/// régulièrement prises).
fn median_abs(n: usize, at: impl Fn(usize) -> f32) -> f32 {
    let every = (n >> 18).max(1);
    let mut v: Vec<f32> = (0..n).step_by(every).map(|i| at(i).abs()).collect();
    if v.is_empty() {
        return 0.0;
    }
    let mid = v.len() / 2;
    *v.select_nth_unstable_by(mid, f32::total_cmp).1
}

const WR: f32 = 0.2627;
const WG: f32 = 0.6780;
const WB: f32 = 0.0593;

/// Les deux différences de couleur, réduites de moitié (moyenne de chaque bloc 2 × 2,
/// exactement comme `Plane::downscale(2)`), calculées directement depuis l'image.
fn half_chroma(img: &RgbImage, ycc: &(impl Fn([f32; 3]) -> (f32, f32, f32) + Sync)) -> (Plane, Plane) {
    let (w, h) = (img.width, img.height);
    let (ow, oh) = (w.div_ceil(2), h.div_ceil(2));
    let mut cr = Plane::new(ow, oh);
    let mut cb = Plane::new(ow, oh);
    // Par ligne de la source : les différences de couleur de chaque pixel dans deux
    // tampons, puis les paires additionnées, chaque boucle sans condition (le processeur
    // les mène par colonnes) ; les mêmes sommes dans le même ordre que pixel par pixel.
    let pairs = w / 2;
    cr.data
        .par_chunks_mut(ow)
        .zip(cb.data.par_chunks_mut(ow))
        .enumerate()
        .for_each_init(
            || (vec![0.0f32; w], vec![0.0f32; w]),
            |(er, eb), (oy, (row_r, row_b))| {
                let (y0, y1) = (oy * 2, (oy * 2 + 2).min(h));
                for y in y0..y1 {
                    let src = &img.data[y * w..(y + 1) * w];
                    for ((r, b), p) in er.iter_mut().zip(eb.iter_mut()).zip(src) {
                        let (_, r0, b0) = ycc(*p);
                        (*r, *b) = (r0, b0);
                    }
                    let (sr, sb) = (er.as_chunks::<2>().0, eb.as_chunks::<2>().0);
                    // Écrire sans lire sur la première ligne (voir `Plane::downscale`).
                    if y == y0 {
                        for ((r, b), (pr, pb)) in row_r.iter_mut().zip(row_b.iter_mut()).zip(sr.iter().zip(sb)) {
                            (*r, *b) = (0.0 + (pr[0] + pr[1]), 0.0 + (pb[0] + pb[1]));
                        }
                        if w % 2 == 1 {
                            (row_r[pairs], row_b[pairs]) = (0.0 + er[w - 1], 0.0 + eb[w - 1]);
                        }
                    } else {
                        for ((r, b), (pr, pb)) in row_r.iter_mut().zip(row_b.iter_mut()).zip(sr.iter().zip(sb)) {
                            (*r, *b) = (*r + (pr[0] + pr[1]), *b + (pb[0] + pb[1]));
                        }
                        if w % 2 == 1 {
                            (row_r[pairs], row_b[pairs]) = (row_r[pairs] + er[w - 1], row_b[pairs] + eb[w - 1]);
                        }
                    }
                }
                for (ox, (r, b)) in row_r.iter_mut().zip(row_b.iter_mut()).enumerate() {
                    let n = ((ox * 2 + 2).min(w) - ox * 2) * (y1 - y0);
                    *r /= n as f32;
                    *b /= n as f32;
                }
            },
        );
    (cr, cb)
}

/// Les paramètres de l'accentuation : force, rayon du flou (en pixels de l'image) et
/// borne de l'écart (le Détail). `None` quand elle ne s'applique pas : nulle, ou à moins
/// de 50 %, où elle ne se juge pas (elle ne se voit qu'à partir de l'affichage 1:1).
/// Le GPU s'en sert aussi.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sharpening {
    pub amount: f32,
    pub radius: f32,
    pub limit: f32,
}

pub fn sharpening(d: &Detail, scale: f32) -> Option<Sharpening> {
    let amount = d.sharpen_amount / 100.0 * 1.4;
    (amount > 0.0 && scale >= 0.5).then(|| Sharpening {
        amount,
        radius: (d.sharpen_radius * scale).max(0.5),
        limit: 0.015 + d.sharpen_detail / 100.0 * 0.12,
    })
}

/// Accentuation, sur l'image déjà rendue pour l'écran (linéaire, 0 à 1).
pub fn sharpen(img: &mut RgbImage, d: &Detail, scale: f32) {
    let Some(Sharpening { amount, radius, limit }) = sharpening(d, scale) else {
        return;
    };
    if img.data.is_empty() {
        return;
    }
    let perceived = Plane::from_rgb(img, |p| luminance(p).max(0.0).powf(1.0 / 2.2));
    let blurred = perceived.blur(radius);
    // La déconvolution : l'image nette estimée tient lieu de « v + écart », le Gain en
    // dose la part, le Détail double le plafond (elle rend du vrai détail, pas un halo).
    let (amount, limit, sharp) = if d.sharpen_deconvolution {
        let linear = Plane::from_rgb(img, |p| luminance(p).max(0.0));
        let sharp = richardson_lucy(&linear, radius);
        (amount / 1.4, limit * 2.0, Some(sharp.map(|v| v.powf(1.0 / 2.2))))
    } else {
        (amount, limit, None)
    };
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
        let diff = match &sharp {
            Some(s) => s.data[i] - v,
            None => v - blurred.data[i],
        }
        .clamp(-limit, limit);
        let m = mask.as_ref().map_or(1.0, |m| m.data[i]);
        let sharp = (v + amount * diff * m).max(0.0);
        let gain = if v > 1e-4 { (sharp / v).powf(2.2) } else { 1.0 };
        p.iter_mut().for_each(|c| *c *= gain);
    });
}

/// Les itérations de Richardson‑Lucy : assez pour défaire un flou d'un ou deux pixels,
/// pas assez pour que le bruit monte.
const RL_ITERATIONS: usize = 10;

/// Richardson‑Lucy sur `observed` (lumière linéaire, où le flou a eu lieu), pour un
/// flou gaussien `sigma` (symétrique : il est son propre transposé). Deux tampons
/// seulement, et chaque itération en quatre passes : le flou de l'estimation (lignes,
/// colonnes), puis celui du rapport observée / floutée, calculé à la volée dans la passe
/// des lignes, la correction appliquée dans celle des colonnes.
fn richardson_lucy(observed: &Plane, sigma: f32) -> Plane {
    const FLOOR: f32 = 1e-4;
    let (w, h) = (observed.width, observed.height);
    let kernel = gaussian_kernel(sigma);
    let mut u = observed.clone();
    let (mut rows, mut hu) = (vec![0.0; w * h], vec![0.0; w * h]);
    for _ in 0..RL_ITERATIONS {
        convolve_rows(&u.data, &mut rows, w, &kernel, |_, v| v);
        convolve_cols(&rows, &mut hu, w, h, &kernel, |_, _, c| c);
        let obs = &observed.data;
        convolve_rows(&hu, &mut rows, w, &kernel, |i, v| obs[i].max(0.0) / v.max(FLOOR));
        // La correction au carré (Richardson‑Lucy accéléré : le même chemin en deux fois
        // moins d'itérations), bornée pour qu'un pixel isolé ne s'emballe pas.
        convolve_cols(&rows, &mut u.data, w, h, &kernel, |_, u, c| {
            u.max(0.0) * (c * c).clamp(0.5, 2.0)
        });
    }
    u
}

/// Les poids d'une gaussienne d'écart type `sigma`, sur ±2,5 σ (au moins un pixel),
/// de somme 1.
fn gaussian_kernel(sigma: f32) -> Vec<f32> {
    let r = (2.5 * sigma).ceil().max(1.0) as isize;
    let k: Vec<f32> = (-r..=r)
        .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f32 = k.iter().sum();
    k.iter().map(|v| v / sum).collect()
}

/// Convolution des lignes de `src` (bords prolongés) ; `pre(i, v)` transforme chaque
/// valeur lue à l'indice `i` avant de la pondérer.
fn convolve_rows(src: &[f32], dst: &mut [f32], w: usize, k: &[f32], pre: impl Fn(usize, f32) -> f32 + Sync) {
    let r = k.len() / 2;
    dst.par_chunks_mut(w).enumerate().for_each_init(
        || vec![0.0; w + 2 * r],
        |line, (y, out)| {
            let row = &src[y * w..(y + 1) * w];
            for (x, v) in line.iter_mut().enumerate() {
                let sx = x.saturating_sub(r).min(w - 1);
                *v = pre(y * w + sx, row[sx]);
            }
            // Huit pixels à la fois, chacun avec les mêmes produits ajoutés dans le même
            // ordre (depuis le zéro de `sum`) : les mêmes valeurs, en colonnes.
            let zero: f32 = std::iter::empty::<f32>().sum();
            let (blocks, tail) = out.as_chunks_mut::<8>();
            for (b, o) in blocks.iter_mut().enumerate() {
                let x = 8 * b;
                let mut acc = [zero; 8];
                for (j, kj) in k.iter().enumerate() {
                    let v = &line[x + j..x + j + 8];
                    for l in 0..8 {
                        acc[l] += kj * v[l];
                    }
                }
                *o = acc;
            }
            let rest = w - tail.len();
            for (x, o) in out.iter_mut().enumerate().skip(rest) {
                *o = k.iter().zip(&line[x..x + k.len()]).map(|(a, b)| a * b).sum();
            }
        },
    );
}

/// Convolution des colonnes de `src` (bords prolongés) ; `post(i, ancien, flou)` donne
/// la valeur écrite dans `dst` à l'indice `i`.
fn convolve_cols(
    src: &[f32],
    dst: &mut [f32],
    w: usize,
    h: usize,
    k: &[f32],
    post: impl Fn(usize, f32, f32) -> f32 + Sync,
) {
    let r = k.len() / 2;
    dst.par_chunks_mut(w).enumerate().for_each_init(
        || vec![0.0; w],
        |acc, (y, out)| {
            acc.iter_mut().for_each(|a| *a = 0.0);
            for (j, kj) in k.iter().enumerate() {
                let sy = (y + j).saturating_sub(r).min(h - 1);
                for (a, v) in acc.iter_mut().zip(&src[sy * w..(sy + 1) * w]) {
                    *a += kj * v;
                }
            }
            for (x, (o, a)) in out.iter_mut().zip(acc.iter()).enumerate() {
                *o = post(y * w + x, *o, *a);
            }
        },
    );
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

    /// La réduction du bruit telle qu'elle était écrite avant d'être fusionnée (plans
    /// pleine taille) : la version actuelle doit donner exactement les mêmes pixels.
    fn noise_reduction_reference(img: &mut RgbImage, d: &Detail, scale: f32) {
        let (lum, col) = (d.luminance_nr / 100.0, d.color_nr / 100.0);
        let lum_sigma = (1.0 + 2.5 * lum) * scale;
        let col_sigma = (2.0 + 10.0 * col) * scale;
        let do_lum = lum > 0.0 && lum_sigma >= 0.8;
        let do_col = col > 0.0 && col_sigma >= 0.6;
        if !do_lum && !do_col {
            return;
        }
        let (w, h) = (img.width, img.height);
        let enc: Vec<[f32; 3]> = img.data.iter().map(|p| p.map(|v| v.max(0.0).sqrt())).collect();
        let plane = |data| Plane {
            width: w,
            height: h,
            data,
        };
        let mut y = plane(enc.iter().map(|e| WR * e[0] + WG * e[1] + WB * e[2]).collect());
        let mut cr = plane(enc.iter().zip(&y.data).map(|(e, y)| e[0] - y).collect());
        let mut cb = plane(enc.iter().zip(&y.data).map(|(e, y)| e[2] - y).collect());
        if do_lum {
            let eps = (lum * lum * 0.0015 * (1.0 - 0.8 * d.luminance_nr_detail / 100.0)).max(1e-7);
            let smooth = y.guided(lum_sigma, eps);
            y = y.zip(&smooth, |orig, s| orig + (s - orig) * lum.sqrt());
        }
        if do_col {
            let eps = 0.0001 + 0.001 * (1.0 - d.color_nr_detail / 100.0);
            let sigma = col_sigma * (0.6 + 0.8 * d.color_nr_smoothness / 100.0);
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
        for (i, p) in img.data.iter_mut().enumerate() {
            let (y, r, b) = (y.data[i], y.data[i] + cr.data[i], y.data[i] + cb.data[i]);
            let g = (y - WR * r - WB * b) / WG;
            *p = [r.max(0.0).powi(2), g.max(0.0).powi(2), b.max(0.0).powi(2)];
        }
    }

    #[test]
    fn fused_noise_reduction_gives_the_same_pixels() {
        // Tailles impaires (blocs du bord incomplets), valeurs négatives, chaque
        // combinaison de réglages : luminance seule, couleur seule (petit et grand rayon),
        // les deux, et une échelle réduite.
        let img = RgbImage::from_fn(101, 67, |x, y| {
            let n = (((x * 7919 + y * 104_729) % 97) as f32 / 97.0 - 0.5) * 0.08;
            [
                0.15 + x as f32 * 0.004 + n,
                0.3 - n * 0.7,
                (0.05 + y as f32 * 0.006 - n).max(-0.01),
            ]
        });
        for (lum, col, smooth, scale) in [
            (40.0, 0.0, 50.0, 1.0),
            (0.0, 25.0, 50.0, 1.0),
            (0.0, 80.0, 100.0, 1.0),
            (60.0, 25.0, 50.0, 1.0),
            (30.0, 70.0, 20.0, 0.5),
            (0.0, 0.0, 50.0, 1.0),
        ] {
            let d = Detail {
                luminance_nr: lum,
                color_nr: col,
                color_nr_smoothness: smooth,
                ..Detail::default()
            };
            let (mut a, mut b) = (img.clone(), img.clone());
            noise_reduction(&mut a, &d, scale);
            noise_reduction_reference(&mut b, &d, scale);
            let same = a
                .data
                .iter()
                .zip(&b.data)
                .all(|(p, q)| (0..3).all(|c| p[c].to_bits() == q[c].to_bits()));
            assert!(same, "réglages {lum} {col} {smooth} {scale}");
        }
    }

    fn variance(img: &RgbImage, c: usize) -> f32 {
        let m: f32 = img.data.iter().map(|p| p[c]).sum::<f32>() / img.data.len() as f32;
        img.data.iter().map(|p| (p[c] - m).powi(2)).sum::<f32>() / img.data.len() as f32
    }

    /// Une scène nette (dégradé, disques aux bords francs, fines rayures) et la même
    /// avec un bruit gaussien de luminance.
    fn scene() -> (RgbImage, RgbImage) {
        let clean = RgbImage::from_fn(192, 160, |x, y| {
            let (fx, fy) = (x as f32, y as f32);
            let mut v = 0.05 + fx / 192.0 * 0.3;
            if (fx - 60.0).powi(2) + (fy - 60.0).powi(2) < 900.0 {
                v = 0.6;
            }
            if (fx - 140.0).powi(2) + (fy - 110.0).powi(2) < 600.0 {
                v = 0.02;
            }
            if y > 130 {
                v = 0.25 + 0.15 * (fx * 0.9).sin();
            }
            [v; 3]
        });
        let hash = |i: u64| {
            let mut z = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 40) as f32 / (1u64 << 24) as f32
        };
        let mut noisy = clean.clone();
        for (i, p) in noisy.data.iter_mut().enumerate() {
            let (u, v) = (hash(2 * i as u64).max(1e-7), hash(2 * i as u64 + 1));
            let n = (-2.0 * u.ln()).sqrt() * (std::f32::consts::TAU * v).cos();
            // Un bruit uniforme dans l'encodage « racine carrée », donc plus fort dans
            // les lumières en linéaire, comme celui d'un capteur.
            let e = p[0].sqrt() + n * 0.03;
            *p = [e.max(0.0).powi(2); 3];
        }
        (clean, noisy)
    }

    /// Le rapport signal sur bruit (dB), mesuré dans l'encodage « racine carrée ».
    fn psnr(a: &RgbImage, b: &RgbImage) -> f32 {
        let mse = a
            .data
            .iter()
            .zip(&b.data)
            .map(|(p, q)| (p[1].max(0.0).sqrt() - q[1].max(0.0).sqrt()).powi(2))
            .sum::<f32>()
            / a.data.len() as f32;
        -10.0 * mse.log10()
    }

    // L'ancienne version des ondelettes (un tampon neuf par passe), pour vérifier que la
    // version fusionnée donne exactement les mêmes pixels.
    fn wavelet_denoise_reference(y: &Plane, strength: f32, detail: f32, contrast: f32) -> Plane {
        let (w, h) = (y.width, y.height);
        let mut coarse = y.clone();
        let mut out = Plane::new(w, h);
        let mut sigma = 0.0;
        for (level, noise) in WAVELET_NOISE.iter().enumerate() {
            let step = 1 << level;
            if 2 * step >= w.min(h) {
                break;
            }
            let smooth = atrous_reference(&coarse, step);
            let mut fine = coarse.zip(&smooth, |c, s| c - s);
            if level == 0 {
                sigma = median_abs_reference(&fine.data) / 0.6745 / noise;
            }
            let keep = if level < 2 {
                1.0 - 0.7 * detail
            } else {
                1.0 - 0.8 * contrast
            };
            // Un Wiener local : là où l'énergie du voisinage (le carré des coefficients,
            // flouté à la taille de l'échelle) dépasse à peine celle du bruit, le
            // coefficient s'efface ; une texture ou un contour, bien plus énergiques, restent.
            let t = 3.0 * strength * keep * sigma * noise;
            if t > 0.0 {
                let energy = atrous_reference(&fine.zip(&fine, |a, b| a * b), step);
                fine.data.par_iter_mut().zip(&energy.data).for_each(|(v, e)| {
                    *v *= ((e - t * t) / e.max(1e-12)).max(0.0);
                });
            }
            out.data.par_iter_mut().zip(&fine.data).for_each(|(o, f)| *o += f);
            coarse = smooth;
        }
        out.data.par_iter_mut().zip(&coarse.data).for_each(|(o, c)| *o += c);
        out
    }

    fn atrous_reference(p: &Plane, step: usize) -> Plane {
        const K: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];
        let (w, h) = (p.width, p.height);
        let reflect = |i: isize, n: usize| -> usize {
            let n = n as isize;
            let i = if i < 0 { -i } else { i };
            (if i >= n { 2 * (n - 1) - i } else { i }).clamp(0, n - 1) as usize
        };
        let tap = |v: [f32; 5]| K[0] * (v[0] + v[4]) + K[1] * (v[1] + v[3]) + K[2] * v[2];
        let mut rows = Plane::new(w, h);
        rows.data.par_chunks_mut(w).enumerate().for_each(|(yy, row)| {
            let src = &p.data[yy * w..(yy + 1) * w];
            for (x, o) in row.iter_mut().enumerate() {
                // L'intérieur, sans réflexion ; les bords la prennent.
                *o = if x >= 2 * step && x + 2 * step < w {
                    tap(std::array::from_fn(|k| src[x + k * step - 2 * step]))
                } else {
                    tap(std::array::from_fn(|k| {
                        src[reflect(x as isize + (k as isize - 2) * step as isize, w)]
                    }))
                };
            }
        });
        let mut out = Plane::new(w, h);
        out.data.par_chunks_mut(w).enumerate().for_each(|(yy, row)| {
            let lines: [&[f32]; 5] = std::array::from_fn(|k| {
                let l = reflect(yy as isize + (k as isize - 2) * step as isize, h) * w;
                &rows.data[l..l + w]
            });
            for (x, o) in row.iter_mut().enumerate() {
                *o = tap(std::array::from_fn(|k| lines[k][x]));
            }
        });
        out
    }

    fn median_abs_reference(data: &[f32]) -> f32 {
        let every = (data.len() >> 18).max(1);
        let mut v: Vec<f32> = data.iter().step_by(every).map(|x| x.abs()).collect();
        if v.is_empty() {
            return 0.0;
        }
        let mid = v.len() / 2;
        *v.select_nth_unstable_by(mid, f32::total_cmp).1
    }

    #[test]
    fn fused_wavelets_give_the_same_pixels() {
        let (noisy, _) = scene();
        for (w, h) in [(noisy.width, noisy.height), (37, 23), (9, 9)] {
            let mut y = Plane::new(w, h);
            for (i, v) in y.data.iter_mut().enumerate() {
                *v = noisy.data[i % noisy.data.len()][1];
            }
            for (strength, detail, contrast) in [(0.5, 0.5, 0.3), (1.0, 0.0, 1.0), (0.0, 0.2, 0.2)] {
                let a = wavelet_denoise(&y, strength, detail, contrast);
                let b = wavelet_denoise_reference(&y, strength, detail, contrast);
                assert!(
                    a.data.iter().zip(&b.data).all(|(p, q)| p.to_bits() == q.to_bits()),
                    "{w}×{h}"
                );
            }
        }
    }

    #[test]
    fn wavelets_beat_the_guided_filter() {
        let (clean, noisy) = scene();
        let before = psnr(&clean, &noisy);
        let best = |wavelets: bool| {
            [30.0, 50.0, 70.0, 90.0]
                .iter()
                .map(|&lum| {
                    let mut img = noisy.clone();
                    let d = Detail {
                        luminance_nr: lum,
                        luminance_nr_wavelets: wavelets,
                        color_nr: 0.0,
                        ..Detail::default()
                    };
                    noise_reduction(&mut img, &d, 1.0);
                    psnr(&clean, &img)
                })
                .fold(f32::MIN, f32::max)
        };
        let (guided, wavelets) = (best(false), best(true));
        eprintln!("avant {before:.2} dB, guidé {guided:.2} dB, ondelettes {wavelets:.2} dB");
        assert!(wavelets > before + 5.0);
        assert!(wavelets > guided + 2.5);
    }

    #[test]
    fn wavelets_keep_edges_and_spare_a_clean_image() {
        let (clean, noisy) = scene();
        let d = Detail {
            luminance_nr: 60.0,
            luminance_nr_wavelets: true,
            color_nr: 0.0,
            ..Detail::default()
        };
        // Le bord du disque clair (x = 30) reste franc : à deux pixels de part et d'autre, l'écart
        // garde l'essentiel de sa hauteur.
        let mut img = noisy.clone();
        noise_reduction(&mut img, &d, 1.0);
        let at = |x: usize| img.data[60 * 192 + x][1];
        assert!(
            at(32) - at(28) > 0.8 * (0.6 - clean.data[60 * 192 + 28][1]),
            "{} {}",
            at(28),
            at(32)
        );
        // Sans bruit, presque rien ne bouge : le seuil suit le bruit mesuré.
        let mut same = clean.clone();
        noise_reduction(&mut same, &d, 1.0);
        assert!(psnr(&clean, &same) > 45.0, "{}", psnr(&clean, &same));
        // Désactivé, le chemin habituel ne change pas.
        let off = Detail {
            luminance_nr_wavelets: false,
            ..d.clone()
        };
        let (mut a, mut b) = (noisy.clone(), noisy.clone());
        noise_reduction(&mut a, &off, 1.0);
        noise_reduction_reference(&mut b, &off, 1.0);
        assert_eq!(a, b);
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

    /// Une mire nette (barres de plusieurs largeurs, disque, aplats) et la même floutée
    /// d'une gaussienne de 1,2 pixel, comme par un objectif un peu mou.
    fn soft_chart() -> (RgbImage, RgbImage) {
        let sharp = RgbImage::from_fn(160, 120, |x, y| {
            let v = if y < 40 {
                if (x / (3 + x / 40)) % 2 == 0 { 0.6 } else { 0.08 }
            } else if (x as f32 - 80.0).powi(2) + (y as f32 - 80.0).powi(2) < 600.0 {
                0.5
            } else {
                0.15
            };
            [v; 3]
        });
        let plane = Plane::from_rgb(&sharp, |p| p[1]).blur(1.2);
        let soft = RgbImage::from_fn(160, 120, |x, y| [plane.data[y * 160 + x]; 3]);
        (sharp, soft)
    }

    /// Le rapport signal sur bruit (dB) en luminance perçue, loin des bords de l'image.
    fn psnr_perceived(a: &RgbImage, b: &RgbImage) -> f32 {
        let mut sum = 0.0;
        let mut n = 0.0;
        for y in 6..a.height - 6 {
            for x in 6..a.width - 6 {
                let i = y * a.width + x;
                let g = |p: [f32; 3]| p[1].max(0.0).powf(1.0 / 2.2);
                sum += (g(a.data[i]) - g(b.data[i])).powi(2);
                n += 1.0;
            }
        }
        -10.0 * (sum / n).log10()
    }

    #[test]
    fn row_convolution_in_lanes_gives_the_same_values() {
        for w in [1, 7, 8, 13, 64, 101] {
            let h = 3;
            let src: Vec<f32> = (0..w * h)
                .map(|i| ((i * 37 % 101) as f32 / 50.0 - 0.7).powi(3))
                .collect();
            for sigma in [0.5, 0.8, 1.7, 3.0] {
                let k = gaussian_kernel(sigma);
                let r = k.len() / 2;
                let mut dst = vec![0.0; w * h];
                convolve_rows(&src, &mut dst, w, &k, |i, v| v * 1.5 + i as f32 * 1e-3);
                for y in 0..h {
                    for x in 0..w {
                        let at = |j: usize| {
                            let sx = (x + j).saturating_sub(r).min(w - 1);
                            src[y * w + sx] * 1.5 + (y * w + sx) as f32 * 1e-3
                        };
                        let want: f32 = k.iter().enumerate().map(|(j, kj)| kj * at(j)).sum();
                        assert_eq!(dst[y * w + x].to_bits(), want.to_bits(), "{w} {sigma} {x}");
                    }
                }
            }
        }
    }

    #[test]
    fn deconvolution_undoes_a_soft_lens_better_than_unsharp_masking() {
        let (sharp, soft) = soft_chart();
        let best = |deconvolution: bool| {
            [40.0, 70.0, 100.0, 130.0, 150.0]
                .iter()
                .map(|&amount| {
                    let mut img = soft.clone();
                    let d = Detail {
                        sharpen_amount: amount,
                        sharpen_radius: 1.2,
                        sharpen_detail: 100.0,
                        sharpen_deconvolution: deconvolution,
                        ..Detail::default()
                    };
                    sharpen(&mut img, &d, 1.0);
                    psnr_perceived(&sharp, &img)
                })
                .fold(f32::MIN, f32::max)
        };
        let before = psnr_perceived(&sharp, &soft);
        let (unsharp, deconvolved) = (best(false), best(true));
        eprintln!("flou {before:.2} dB, masque flou {unsharp:.2} dB, déconvolution {deconvolved:.2} dB");
        assert!(deconvolved > unsharp + 1.0);
        // Un aplat reste un aplat.
        let mut flat = RgbImage::from_fn(40, 40, |_, _| [0.3; 3]);
        let d = Detail {
            sharpen_amount: 150.0,
            sharpen_deconvolution: true,
            ..Detail::default()
        };
        sharpen(&mut flat, &d, 1.0);
        assert!(flat.data.iter().all(|p| (p[1] - 0.3).abs() < 1e-4));
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
