//! Les empilements : plusieurs poses de la même scène, alignées puis combinées.
//!
//! - **Moyenne** : le bruit baisse (racine du nombre de poses) ; pour les poses longues
//!   simulées, l'eau soyeuse, les basses lumières.
//! - **Médiane** : ce qui bouge disparaît (passants, voitures).
//! - **Maximum** : les traces s'additionnent (filé d'étoiles, phares, éclairs).
//! - **Étoiles** : alignement sur les étoiles (le ciel tourne d'une pose à l'autre),
//!   puis moyenne avec rejet des valeurs aberrantes (avions, satellites, rayons
//!   cosmiques) : le ciel profond sort du bruit.
//! - **HDR** : des poses d'expositions différentes (bracketing), alignées sur des copies
//!   ramenées à la même luminosité ; le rapport d'exposition de chaque pose est mesuré
//!   sur l'image (pas lu dans l'EXIF), puis chaque pixel est la moyenne des poses où il
//!   n'est ni saturé ni noyé dans le bruit, les plus exposées comptant le plus. Ce qui a
//!   bougé entre les poses (feuillage, passants) est pris à la pose de référence seule :
//!   pas de fantômes.

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
    Hdr,
}

impl StackMode {
    pub fn name(self) -> &'static str {
        match self {
            StackMode::Mean => "Moyenne",
            StackMode::Median => "Médiane",
            StackMode::Maximum => "Maximum",
            StackMode::Stars => "Étoiles",
            StackMode::Hdr => "HDR",
        }
    }
}

/// Le passage des coordonnées pleine taille aux coordonnées d'analyse (facteur `s`),
/// centres de pixels compris.
fn to_analysis(s: f64) -> Homography {
    let o = (s - 1.0) / 2.0;
    Homography([[s, 0.0, o], [0.0, s, o], [0.0, 0.0, 1.0]])
}

/// Le résultat de l'alignement.
struct Aligned {
    /// Référence → pose, en pleine taille ; `None` pour une pose qu'on n'a pas pu aligner.
    models: Vec<Option<Homography>>,
    width: usize,
    height: usize,
    /// La pose de référence (en HDR, l'exposition médiane).
    reference: usize,
    /// Le gain qui ramène chaque pose à l'exposition de la référence (1 hors HDR).
    gains: Vec<f32>,
}

/// La luminance médiane d'une copie d'analyse (sans les pixels saturés).
fn median_level(img: &RgbImage) -> f32 {
    let mut v: Vec<f32> = img
        .data
        .iter()
        .step_by(7)
        .filter(|p| p[0].max(p[1]).max(p[2]) < 0.95)
        .map(|p| p[0] + 2.0 * p[1] + p[2])
        .collect();
    if v.is_empty() {
        return 4.0;
    }
    let mid = v.len() / 2;
    *v.select_nth_unstable_by(mid, f32::total_cmp).1
}

/// Le gain de `pose` vers `reference` (copies d'analyse, `model` : référence → pose) :
/// la médiane des rapports de luminance là où les deux sont bien exposées.
fn exposure_gain(reference: &RgbImage, pose: &RgbImage, model: &Homography) -> Option<f32> {
    let ok = |p: [f32; 3]| {
        let m = p[0].max(p[1]).max(p[2]);
        (0.03..0.85).contains(&m)
    };
    let mut ratios = Vec::new();
    for y in (0..reference.height).step_by(2) {
        for x in (0..reference.width).step_by(2) {
            let r = reference.data[y * reference.width + x];
            let Some((u, v)) = model.apply((x as f64, y as f64)) else {
                continue;
            };
            let Some(p) = sample(pose, u, v) else {
                continue;
            };
            if ok(r) && ok(p) {
                ratios.push((r[0] + 2.0 * r[1] + r[2]) / (p[0] + 2.0 * p[1] + p[2]));
            }
        }
    }
    if ratios.len() < 50 {
        return None;
    }
    let mid = ratios.len() / 2;
    Some(*ratios.select_nth_unstable_by(mid, f32::total_cmp).1)
}

fn align(
    frames: &dyn Frames,
    reference: usize,
    mode: StackMode,
    progress: &Progress,
    cancel: &Cancel,
) -> Result<Aligned> {
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
    // En HDR, la référence est l'exposition médiane, et chaque copie est ramenée à sa
    // luminosité : les coins se retrouvent d'une pose à l'autre.
    let levels: Vec<f32> = small.iter().map(median_level).collect();
    let reference = if mode == StackMode::Hdr {
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| levels[a].total_cmp(&levels[b]));
        order[n / 2]
    } else {
        reference
    };
    let normalized: Vec<RgbImage> = if mode == StackMode::Hdr {
        small
            .iter()
            .zip(&levels)
            .map(|(img, &l)| {
                let k = levels[reference] / l.max(1e-4);
                RgbImage {
                    width: img.width,
                    height: img.height,
                    data: img.data.iter().map(|p| p.map(|v| (v * k).min(1.0))).collect(),
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let s = small[0].width as f64 / full.0 as f64;
    let a = to_analysis(s);
    let a_inv = a.inverse().expect("échelle");
    let to_full = |h: Homography| a.then(&h).then(&a_inv);
    let mut models: Vec<Option<Homography>> = vec![None; n];
    models[reference] = Some(Homography::IDENTITY);
    let mut small_models: Vec<Option<Homography>> = vec![None; n];
    small_models[reference] = Some(Homography::IDENTITY);
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
            for (k, (i, neighbour)) in order.into_iter().enumerate() {
                cancel.check()?;
                progress(0.3 + 0.1 * k as f32 / n as f32, "Alignement sur les étoiles");
                let guess = small_models[neighbour].unwrap_or(Homography::IDENTITY);
                small_models[i] = stars::align(&detected[reference], &detected[i], &guess).map(|(h, _)| h);
            }
            for (m, sm) in models.iter_mut().zip(&small_models) {
                *m = sm.map(to_full);
            }
        }
        _ => {
            let seen = if mode == StackMode::Hdr { &normalized } else { &small };
            let feats: Vec<Vec<features::Feature>> = seen
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
                small_models[i] = ransac(&pairs, Model::Homography, 2.0, 800)
                    .filter(|(_, inl)| inl.len() >= 15)
                    .map(|(h, _)| h);
            }
            for (m, sm) in models.iter_mut().zip(&small_models) {
                *m = sm.map(to_full);
            }
        }
    }
    // Les gains d'exposition, mesurés sur les copies alignées.
    let mut gains = vec![1.0; n];
    if mode == StackMode::Hdr {
        for i in 0..n {
            if let (Some(m), true) = (small_models[i], i != reference) {
                match exposure_gain(&small[reference], &small[i], &m) {
                    Some(g) => gains[i] = g,
                    None => models[i] = None,
                }
            }
        }
    }
    Ok(Aligned {
        models,
        width: full.0,
        height: full.1,
        reference,
        gains,
    })
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
    // Le HDR s'aligne toujours : il lui faut aussi les gains d'exposition.
    let Aligned {
        models,
        width: w,
        height: h,
        reference,
        gains,
    } = if align_frames || matches!(mode, StackMode::Stars | StackMode::Hdr) {
        align(frames, n / 2, mode, progress, cancel)?
    } else {
        let first = frames.load(0)?;
        Aligned {
            models: vec![Some(Homography::IDENTITY); n],
            width: first.width,
            height: first.height,
            reference: n / 2,
            gains: vec![1.0; n],
        }
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
        StackMode::Hdr => {
            let img = frames.load(reference)?;
            hdr(
                frames, &used, &models, &gains, reference, &img, &mut count, &step, cancel,
            )?
        }
    };
    let total = used.len() as u16;
    let covered: Vec<bool> = count.iter().map(|&c| c >= total).collect();
    progress(1.0, "Terminé");
    let mut merged = Merged::new(image, covered, used.len(), skipped);
    if mode == StackMode::Hdr {
        merged.fit_exposure();
    }
    Ok(merged)
}

/// La fusion HDR : chaque pose ramenée à l'exposition de la référence (`gains`), puis
/// moyennée avec un poids qui écarte ce qui sature, préfère les poses les plus exposées
/// (moins de bruit) et ignore ce qui ne ressemble pas à la référence là où elle est
/// fiable (ce qui a bougé). Là où toutes les poses saturent, la plus sombre reste.
#[allow(clippy::too_many_arguments)]
fn hdr(
    frames: &dyn Frames,
    used: &[usize],
    models: &[Option<Homography>],
    gains: &[f32],
    reference: usize,
    ref_img: &RgbImage,
    count: &mut [u16],
    step: &(dyn Fn(usize, &str) + Sync),
    cancel: &Cancel,
) -> Result<RgbImage> {
    let (w, h) = (ref_img.width, ref_img.height);
    // Le saturé ne compte pas ; il s'efface entre 0,8 et 0,97 du capteur.
    let fit = |m: f32| 1.0 - smoothstep(0.8, 0.97, m);
    let level = |p: [f32; 3]| p[0] + 2.0 * p[1] + p[2];
    let darkest = *used
        .iter()
        .max_by(|&&a, &&b| gains[a].total_cmp(&gains[b]))
        .expect("au moins deux poses");
    let mut acc = vec![[0.0f32; 4]; w * h];
    for (k, &i) in used.iter().enumerate() {
        cancel.check()?;
        step(k, "Fusion HDR");
        let img = if i == reference { None } else { Some(frames.load(i)?) };
        let img = img.as_deref().unwrap_or(ref_img);
        let m = models[i].expect("alignée");
        let g = gains[i];
        acc.par_chunks_mut(w)
            .zip(count.par_chunks_mut(w))
            .enumerate()
            .for_each(|(y, (row, cnt))| {
                for (x, a) in row.iter_mut().enumerate() {
                    let Some(p) = m.apply((x as f64, y as f64)).and_then(|(u, v)| sample(img, u, v)) else {
                        continue;
                    };
                    cnt[x] += 1;
                    let v = p.map(|c| c * g);
                    // Les poses les plus exposées (petit gain) ont le moins de bruit : le poids
                    // est l'inverse de la variance du bruit ramené (gain au carré).
                    let mut weight = fit(p[0].max(p[1]).max(p[2])) / (g * g);
                    if i != reference {
                        let r = ref_img.data[y * w + x];
                        let trust = fit(r[0].max(r[1]).max(r[2])) * smoothstep(0.005, 0.03, level(r));
                        if trust > 0.0 {
                            let d = ((level(v) + 0.02) / (level(r) + 0.02)).ln();
                            let alike = (-(d / 0.35).powi(2)).exp();
                            weight *= 1.0 - trust * (1.0 - alike);
                        }
                    }
                    if i == darkest {
                        weight = weight.max(1e-4);
                    }
                    for c in 0..3 {
                        a[c] += weight * v[c];
                    }
                    a[3] += weight;
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
    Ok(RgbImage {
        width: w,
        height: h,
        data,
    })
}

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Pratique pour les tests : des images en mémoire.
pub fn stack_images(images: Vec<Arc<RgbImage>>, mode: StackMode, align_frames: bool) -> Result<Merged> {
    stack(&images, mode, align_frames, &|_, _| {}, &Cancel::new())
}
