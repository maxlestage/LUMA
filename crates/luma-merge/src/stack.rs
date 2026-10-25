//! Les empilements : plusieurs poses de la même scène, alignées puis combinées.
//!
//! - **Moyenne** : le bruit baisse (racine du nombre de poses) ; pour les poses longues
//!   simulées, l'eau soyeuse, les basses lumières.
//! - **Médiane** : ce qui bouge disparaît (passants, voitures).
//! - **Maximum** : les traces s'additionnent (filé d'étoiles, phares, éclairs).
//! - **Étoiles** : alignement sur les étoiles (le ciel tourne d'une pose à l'autre),
//!   puis moyenne avec rejet des valeurs aberrantes (avions, satellites, rayons
//!   cosmiques) : le ciel profond sort du bruit.

use std::sync::Arc;

use luma_core::{Cancel, Error, Result, RgbImage};
use rayon::prelude::*;

use crate::frames::{Compact, Frames, sample};
use crate::geometry::{Homography, Model, ransac};
use crate::{Merged, Progress, analysis, features, stars};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackMode {
    Mean,
    Median,
    Maximum,
    Stars,
}

impl StackMode {
    pub fn name(self) -> &'static str {
        match self {
            StackMode::Mean => "Moyenne",
            StackMode::Median => "Médiane",
            StackMode::Maximum => "Maximum",
            StackMode::Stars => "Étoiles",
        }
    }
}

/// Le passage des coordonnées pleine taille aux coordonnées d'analyse (facteur `s`),
/// centres de pixels compris.
fn to_analysis(s: f64) -> Homography {
    let o = (s - 1.0) / 2.0;
    Homography([[s, 0.0, o], [0.0, s, o], [0.0, 0.0, 1.0]])
}

/// Les transformations (référence → pose) de chaque pose, en pleine taille ; `None`
/// pour une pose qu'on n'a pas pu aligner.
fn align(
    frames: &dyn Frames,
    reference: usize,
    mode: StackMode,
    progress: &Progress,
    cancel: &Cancel,
) -> Result<(Vec<Option<Homography>>, usize, usize)> {
    let n = frames.len();
    let long_side = if mode == StackMode::Stars { 3000 } else { 1600 };
    let mut small = Vec::with_capacity(n);
    let mut full = (0, 0);
    for i in 0..n {
        cancel.check()?;
        progress(0.3 * i as f32 / n as f32, "Analyse des poses");
        let img = frames.load(i)?;
        if i == 0 {
            full = (img.width, img.height);
        } else if (img.width, img.height) != full {
            return Err(Error::Decode(format!(
                "la pose {} n'a pas la taille des autres ({} × {} au lieu de {} × {})",
                i + 1,
                img.width,
                img.height,
                full.0,
                full.1
            )));
        }
        small.push(analysis(&img, long_side));
    }
    let s = small[0].width as f64 / full.0 as f64;
    let a = to_analysis(s);
    let a_inv = a.inverse().expect("échelle");
    let to_full = |h: Homography| a.then(&h).then(&a_inv);
    let mut models: Vec<Option<Homography>> = vec![None; n];
    models[reference] = Some(Homography::IDENTITY);
    match mode {
        StackMode::Stars => {
            let detected: Vec<Vec<stars::Star>> = small.par_iter().map(|img| stars::detect(img, 400)).collect();
            if detected[reference].len() < 6 {
                return Err(Error::Decode("trop peu d'étoiles dans la pose de référence".into()));
            }
            // En partant de la référence, chaque pose part de l'estimation de sa voisine.
            let order: Vec<(usize, usize)> = (reference + 1..n)
                .map(|i| (i, i - 1))
                .chain((0..reference).rev().map(|i| (i, i + 1)))
                .collect();
            let mut small_models: Vec<Option<Homography>> = vec![None; n];
            small_models[reference] = Some(Homography::IDENTITY);
            for (k, (i, neighbour)) in order.into_iter().enumerate() {
                cancel.check()?;
                progress(0.3 + 0.1 * k as f32 / n as f32, "Alignement sur les étoiles");
                let guess = small_models[neighbour].unwrap_or(Homography::IDENTITY);
                small_models[i] = stars::align(&detected[reference], &detected[i], &guess).map(|(h, _)| h);
            }
            for (m, sm) in models.iter_mut().zip(small_models) {
                *m = sm.map(to_full);
            }
        }
        _ => {
            let feats: Vec<Vec<features::Feature>> = small
                .par_iter()
                .map(|img| features::detect(&features::luminance(img), 1500))
                .collect();
            for i in 0..n {
                if i == reference {
                    continue;
                }
                cancel.check()?;
                progress(0.3 + 0.1 * i as f32 / n as f32, "Alignement");
                let pairs = features::matches(&feats[reference], &feats[i]);
                models[i] = ransac(&pairs, Model::Homography, 2.0, 800)
                    .filter(|(_, inl)| inl.len() >= 15)
                    .map(|(h, _)| to_full(h));
            }
        }
    }
    Ok((models, full.0, full.1))
}

/// Empile les poses. `align` : aligner (sinon elles sont prises telles quelles, pour
/// des poses sur trépied). La pose du milieu sert de référence.
pub fn stack(
    frames: &dyn Frames,
    mode: StackMode,
    align_frames: bool,
    progress: &Progress,
    cancel: &Cancel,
) -> Result<Merged> {
    let n = frames.len();
    if n < 2 {
        return Err(Error::Decode("il faut au moins deux photos".into()));
    }
    let reference = n / 2;
    let (models, w, h) = if align_frames || mode == StackMode::Stars {
        align(frames, reference, mode, progress, cancel)?
    } else {
        let first = frames.load(0)?;
        (vec![Some(Homography::IDENTITY); n], first.width, first.height)
    };
    let used: Vec<usize> = (0..n).filter(|&i| models[i].is_some()).collect();
    let skipped: Vec<usize> = (0..n).filter(|&i| models[i].is_none()).collect();
    if used.len() < 2 {
        return Err(Error::Decode("les poses n'ont pas pu être alignées entre elles".into()));
    }
    let px = w * h;
    let warp = |img: &RgbImage, m: &Homography, x: usize, y: usize| -> Option<[f32; 3]> {
        let (u, v) = m.apply((x as f64, y as f64))?;
        sample(img, u, v)
    };
    let step = |k: usize, label: &str| progress(0.4 + 0.6 * k as f32 / used.len() as f32, label);
    let mut count = vec![0u16; px];
    let image = match mode {
        StackMode::Mean | StackMode::Maximum => {
            let mut acc = vec![[0.0f32; 3]; px];
            for (k, &i) in used.iter().enumerate() {
                cancel.check()?;
                step(k, "Empilement");
                let img = frames.load(i)?;
                let m = models[i].expect("alignée");
                acc.par_chunks_mut(w)
                    .zip(count.par_chunks_mut(w))
                    .enumerate()
                    .for_each(|(y, (row, cnt))| {
                        for x in 0..w {
                            if let Some(p) = warp(&img, &m, x, y) {
                                let a = &mut row[x];
                                if mode == StackMode::Maximum {
                                    for c in 0..3 {
                                        a[c] = a[c].max(p[c]);
                                    }
                                } else {
                                    for c in 0..3 {
                                        a[c] += p[c];
                                    }
                                }
                                cnt[x] += 1;
                            }
                        }
                    });
            }
            if mode == StackMode::Mean {
                acc.par_iter_mut().zip(&count).for_each(|(a, &c)| {
                    if c > 0 {
                        *a = a.map(|v| v / f32::from(c));
                    }
                });
            }
            RgbImage {
                width: w,
                height: h,
                data: acc,
            }
        }
        StackMode::Median => {
            let budget = 6_000_000_000usize;
            if used.len() * px * 6 > budget {
                return Err(Error::Decode(format!(
                    "{} poses de {:.0} Mpx demandent trop de mémoire pour la médiane : réduisez la taille ou choisissez la moyenne",
                    used.len(),
                    px as f64 / 1e6
                )));
            }
            let mut warped = Vec::with_capacity(used.len());
            let mut covers: Vec<Vec<bool>> = Vec::with_capacity(used.len());
            for (k, &i) in used.iter().enumerate() {
                cancel.check()?;
                step(k, "Alignement des poses");
                let img = frames.load(i)?;
                let m = models[i].expect("alignée");
                // Hors de la pose : un rouge négatif, que le format compact ramènerait à 0 ;
                // la couverture est donc gardée à part.
                let out = RgbImage::from_fn(w, h, |x, y| warp(&img, &m, x, y).unwrap_or([-1.0; 3]));
                let cover: Vec<bool> = out.data.iter().map(|p| p[0] >= 0.0).collect();
                for (c, &v) in count.iter_mut().zip(&cover) {
                    *c += u16::from(v);
                }
                warped.push(Compact::new(&out));
                covers.push(cover);
            }
            let data: Vec<[f32; 3]> = (0..px)
                .into_par_iter()
                .map(|p| {
                    let (x, y) = (p % w, p / w);
                    let mut values: Vec<[f32; 3]> = warped
                        .iter()
                        .zip(&covers)
                        .filter(|(_, c)| c[p])
                        .map(|(f, _)| f.pixel(x, y))
                        .collect();
                    if values.is_empty() {
                        return [0.0; 3];
                    }
                    // Médiane selon la luminance : la couleur reste celle d'une vraie pose.
                    let mid = values.len() / 2;
                    values.select_nth_unstable_by(mid, |a, b| {
                        (a[0] + 2.0 * a[1] + a[2]).total_cmp(&(b[0] + 2.0 * b[1] + b[2]))
                    });
                    values[mid]
                })
                .collect();
            RgbImage {
                width: w,
                height: h,
                data,
            }
        }
        StackMode::Stars => {
            // Passe 1 : moyenne et écart type de la luminance de chaque pixel.
            let mut sum = vec![0.0f32; px];
            let mut sum2 = vec![0.0f32; px];
            for (k, &i) in used.iter().enumerate() {
                cancel.check()?;
                step(k / 2, "Statistiques du ciel");
                let img = frames.load(i)?;
                let m = models[i].expect("alignée");
                sum.par_chunks_mut(w)
                    .zip(sum2.par_chunks_mut(w))
                    .zip(count.par_chunks_mut(w))
                    .enumerate()
                    .for_each(|(y, ((s, s2), cnt))| {
                        for x in 0..w {
                            if let Some(p) = warp(&img, &m, x, y) {
                                let l = p[0] + p[1] + p[2];
                                s[x] += l;
                                s2[x] += l * l;
                                cnt[x] += 1;
                            }
                        }
                    });
            }
            let limits: Vec<(f32, f32)> = (0..px)
                .into_par_iter()
                .map(|p| {
                    let c = f32::from(count[p]).max(1.0);
                    let mean = sum[p] / c;
                    let sd = (sum2[p] / c - mean * mean).max(0.0).sqrt();
                    // Au moins un peu de marge : sur deux ou trois poses l'écart type ment.
                    let k = 2.5 * sd + 0.01 * mean + 1e-4;
                    (mean - k, mean + k)
                })
                .collect();
            drop((sum, sum2));
            // Passe 2 : moyenne des seules valeurs plausibles.
            let mut acc = vec![[0.0f32; 4]; px];
            for (k, &i) in used.iter().enumerate() {
                cancel.check()?;
                step(used.len() / 2 + k / 2, "Empilement des étoiles");
                let img = frames.load(i)?;
                let m = models[i].expect("alignée");
                acc.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
                    for (x, a) in row.iter_mut().enumerate() {
                        if let Some(p) = warp(&img, &m, x, y) {
                            let l = p[0] + p[1] + p[2];
                            let (lo, hi) = limits[y * w + x];
                            if l >= lo && l <= hi {
                                a[0] += p[0];
                                a[1] += p[1];
                                a[2] += p[2];
                                a[3] += 1.0;
                            }
                        }
                    }
                });
            }
            let data = acc
                .into_par_iter()
                .map(|a| {
                    if a[3] > 0.0 {
                        [a[0] / a[3], a[1] / a[3], a[2] / a[3]]
                    } else {
                        [0.0; 3]
                    }
                })
                .collect();
            RgbImage {
                width: w,
                height: h,
                data,
            }
        }
    };
    let total = used.len() as u16;
    let covered: Vec<bool> = count.iter().map(|&c| c >= total).collect();
    progress(1.0, "Terminé");
    Ok(Merged::new(image, covered, used.len(), skipped))
}

/// Pratique pour les tests : des images en mémoire.
pub fn stack_images(images: Vec<Arc<RgbImage>>, mode: StackMode, align_frames: bool) -> Result<Merged> {
    stack(&images, mode, align_frames, &|_, _| {}, &Cancel::new())
}
