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
    img.data.par_chunks_mut(w).enumerate().for_each(|(row_y, row)| {
        let (mut up_r, mut up_b) = (Vec::new(), Vec::new());
        if let Some((cr, cb)) = &chroma {
            up_r.resize(w, 0.0);
            up_b.resize(w, 0.0);
            cr.upsample_row(row_y, h, &columns, &mut up_r);
            cb.upsample_row(row_y, h, &columns, &mut up_b);
        }
        for (x, p) in row.iter_mut().enumerate() {
            let (y0, cr0, cb0) = ycc(*p);
            let y = luma.as_ref().map_or(y0, |l| l.data[row_y * w + x]);
            let (cr, cb) = if chroma.is_some() {
                (up_r[x], up_b[x])
            } else {
                (cr0, cb0)
            };
            let (r, b) = (y + cr, y + cb);
            let g = (y - WR * r - WB * b) / WG;
            *p = [r.max(0.0).powi(2), g.max(0.0).powi(2), b.max(0.0).powi(2)];
        }
    });
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
    let mut coarse = y.clone();
    let mut out = Plane::new(w, h);
    let mut sigma = 0.0;
    for (level, noise) in WAVELET_NOISE.iter().enumerate() {
        let step = 1 << level;
        if 2 * step >= w.min(h) {
            break;
        }
        let smooth = atrous(&coarse, step);
        let mut fine = coarse.zip(&smooth, |c, s| c - s);
        if level == 0 {
            sigma = median_abs(&fine.data) / 0.6745 / noise;
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
            let energy = atrous(&fine.zip(&fine, |a, b| a * b), step);
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

/// Un lissage B3 (1 4 6 4 1, sur 16) dont les prises sont espacées de `step` pixels,
/// en lignes puis en colonnes ; les bords se reflètent.
fn atrous(p: &Plane, step: usize) -> Plane {
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

/// La médiane des valeurs absolues (sur au plus 2¹⁸ valeurs régulièrement prises).
fn median_abs(data: &[f32]) -> f32 {
    let every = (data.len() >> 18).max(1);
    let mut v: Vec<f32> = data.iter().step_by(every).map(|x| x.abs()).collect();
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
    cr.data
        .par_chunks_mut(ow)
        .zip(cb.data.par_chunks_mut(ow))
        .enumerate()
        .for_each(|(oy, (row_r, row_b))| {
            let (y0, y1) = (oy * 2, (oy * 2 + 2).min(h));
            for y in y0..y1 {
                let src = &img.data[y * w..(y + 1) * w];
                for (ox, (r, b)) in row_r.iter_mut().zip(row_b.iter_mut()).enumerate() {
                    // La somme du bloc, dans l'ordre de `downscale` : la ligne d'abord.
                    let (_, r0, b0) = ycc(src[ox * 2]);
                    let (sr, sb) = match src.get(ox * 2 + 1) {
                        Some(&p) => {
                            let (_, r1, b1) = ycc(p);
                            (r0 + r1, b0 + b1)
                        }
                        None => (r0, b0),
                    };
                    // Écrire sans lire sur la première ligne (voir `Plane::downscale`).
                    if y == y0 {
                        (*r, *b) = (0.0 + sr, 0.0 + sb);
                    } else {
                        (*r, *b) = (*r + sr, *b + sb);
                    }
                }
            }
            for (ox, (r, b)) in row_r.iter_mut().zip(row_b.iter_mut()).enumerate() {
                let n = ((ox * 2 + 2).min(w) - ox * 2) * (y1 - y0);
                *r /= n as f32;
                *b /= n as f32;
            }
        });
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
    let perceived = Plane::from_rgb(img, |p| luminance(p).max(0.0).powf(1.0 / 2.2));
    let blurred = perceived.blur(radius);
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
