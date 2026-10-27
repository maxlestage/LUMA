//! La correspondance des couleurs entre deux boîtiers : la même scène, photographiée par
//! l'un et par l'autre, donne la matrice qui rend au second les couleurs du premier.
//!
//! Les deux photos (développées sans look, en lumière linéaire de travail) sont alignées
//! comme pour une fusion, légèrement floutées (un décalage d'un pixel ne compte pas), puis
//! comparées point à point loin des contours et des saturations. L'exposition est
//! ramenée au même niveau ; la matrice a des lignes de somme 1 (le blanc reste neutre :
//! la balance des blancs reste celle de chaque photo), et ce qui ne se ressemble pas du
//! tout (ce qui a bougé entre les deux prises) pèse de moins en moins (moindres carrés
//! repondérés).

use luma_color as lc;
use luma_core::{Error, Result, RgbImage};
use rayon::prelude::*;

use crate::frames::sample;
use crate::geometry::{Model, ransac};
use crate::{analysis, features};

/// Le résultat d'une correspondance.
#[derive(Clone, Debug)]
pub struct Matched {
    /// La correction de la seconde photo (lignes : rouge, vert, bleu de sortie), dans
    /// l'espace de travail linéaire ; chaque ligne fait 1.
    pub matrix: lc::Mat3,
    /// Points comparés.
    pub samples: usize,
    /// L'écart moyen (ΔE 2000) entre les deux photos, avant et après.
    pub before: f32,
    pub after: f32,
}

const ANALYSIS: usize = 900;

/// Les couleurs appariées (référence, autre), l'exposition de l'autre ramenée à celle de
/// la référence.
fn pairs(reference: &RgbImage, other: &RgbImage) -> Result<Vec<([f32; 3], [f32; 3])>> {
    let a = analysis(reference, ANALYSIS);
    let b = analysis(other, ANALYSIS);
    let fa = features::detect(&features::luminance(&a), 2000);
    let fb = features::detect(&features::luminance(&b), 2000);
    let (h, inliers) = ransac(&features::matches(&fa, &fb), Model::Homography, 3.0, 2000)
        .ok_or_else(|| Error::Decode("les deux photos ne montrent pas la même scène".into()))?;
    if inliers.len() < 16 {
        return Err(Error::Decode(format!(
            "les deux photos ne montrent pas la même scène ({} points communs)",
            inliers.len()
        )));
    }
    let (a, b) = (blur(&a), blur(&b));
    let (w, hgt) = (a.width, a.height);
    let level = |p: [f32; 3]| p[0] + 2.0 * p[1] + p[2];
    let mut out: Vec<([f32; 3], [f32; 3])> = (1..hgt - 1)
        .into_par_iter()
        .flat_map_iter(|y| {
            let (a, b, h) = (&a, &b, &h);
            (1..w - 1).filter_map(move |x| {
                let p = a.data[y * w + x];
                // Loin des contours (relativement à la lumière du point).
                let l = level(p).max(1e-4);
                let gx = level(a.data[y * w + x + 1]) - level(a.data[y * w + x - 1]);
                let gy = level(a.data[(y + 1) * w + x]) - level(a.data[(y - 1) * w + x]);
                if (gx.abs() + gy.abs()) / l > 0.15 {
                    return None;
                }
                let (u, v) = h.apply((x as f64, y as f64))?;
                let q = sample(b, u, v)?;
                // Ni saturé ni noyé dans le bruit, des deux côtés.
                let ok = |p: [f32; 3]| {
                    let m = p[0].max(p[1]).max(p[2]);
                    p.iter().all(|v| v.is_finite() && *v >= 0.0) && (0.01..0.9).contains(&m)
                };
                (ok(p) && ok(q)).then_some((p, q))
            })
        })
        .collect();
    if out.len() < 500 {
        return Err(Error::Decode(format!(
            "trop peu de points comparables ({}) : une scène plus grande, moins de détails fins",
            out.len()
        )));
    }
    // Le même niveau d'exposition : la médiane des rapports de luminance.
    let mut ratios: Vec<f32> = out.iter().map(|(p, q)| level(*p) / level(*q).max(1e-6)).collect();
    let mid = ratios.len() / 2;
    let gain = *ratios.select_nth_unstable_by(mid, f32::total_cmp).1;
    for (_, q) in &mut out {
        *q = q.map(|v| v * gain);
    }
    Ok(out)
}

/// Un flou 3 × 3.
fn blur(img: &RgbImage) -> RgbImage {
    let (w, h) = (img.width, img.height);
    RgbImage::from_fn(w, h, |x, y| {
        let mut s = [0.0; 3];
        let mut n = 0.0;
        for yy in y.saturating_sub(1)..(y + 2).min(h) {
            for xx in x.saturating_sub(1)..(x + 2).min(w) {
                let p = img.data[yy * w + xx];
                for c in 0..3 {
                    s[c] += p[c];
                }
                n += 1.0;
            }
        }
        s.map(|v| v / n)
    })
}

/// La matrice aux lignes de somme 1 qui envoie au mieux `q` sur `p` (moindres carrés
/// pondérés par `weights`) : chaque ligne est [a, b, 1 − a − b].
fn solve(data: &[([f32; 3], [f32; 3])], weights: &[f32]) -> lc::Mat3 {
    std::array::from_fn(|r| {
        let (mut s11, mut s12, mut s22, mut t1, mut t2) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
        for ((p, q), &wt) in data.iter().zip(weights) {
            let wt = f64::from(wt);
            let (u, v) = (f64::from(q[0] - q[2]), f64::from(q[1] - q[2]));
            let rhs = f64::from(p[r] - q[2]);
            s11 += wt * u * u;
            s12 += wt * u * v;
            s22 += wt * v * v;
            t1 += wt * u * rhs;
            t2 += wt * v * rhs;
        }
        let det = s11 * s22 - s12 * s12;
        if det.abs() < 1e-12 {
            let mut id = [0.0; 3];
            id[r] = 1.0;
            return id;
        }
        let (a, b) = ((t1 * s22 - t2 * s12) / det, (s11 * t2 - s12 * t1) / det);
        [a as f32, b as f32, (1.0 - a - b) as f32]
    })
}

/// L'écart moyen (ΔE 2000) entre `p` et `m · q`, sur au plus 4000 points.
fn mean_error(data: &[([f32; 3], [f32; 3])], m: &lc::Mat3) -> f32 {
    use luma_render::chart::{delta_e2000, lab};
    let to_xyz = lc::rgb_to_xyz(&lc::WORKING);
    // La même échelle que la charte : le gris moyen des points vers 18 %.
    let mean = data.iter().map(|(p, _)| lc::apply(&to_xyz, *p)[1]).sum::<f32>() / data.len() as f32;
    let k = 0.18 / mean.max(1e-6);
    let step = (data.len() / 4000).max(1);
    let (sum, n) = data.iter().step_by(step).fold((0.0, 0.0), |(s, n), (p, q)| {
        let a = lab(lc::apply(&to_xyz, *p).map(|v| v * k));
        let b = lab(lc::apply(&to_xyz, lc::apply(m, *q)).map(|v| v * k));
        (s + delta_e2000(a, b), n + 1.0)
    });
    sum / n
}

/// La correction qui rend à `other` les couleurs de `reference` (la même scène, prise
/// par deux boîtiers ; développées sans look, en lumière linéaire de travail).
pub fn match_colors(reference: &RgbImage, other: &RgbImage) -> Result<Matched> {
    let data = pairs(reference, other)?;
    let mut weights = vec![1.0f32; data.len()];
    let mut m = solve(&data, &weights);
    // Trois passes repondérées : un point qui s'écarte beaucoup (ce qui a bougé, un reflet)
    // pèse de moins en moins (poids de Cauchy, à l'échelle de l'écart médian).
    for _ in 0..3 {
        let res: Vec<f32> = data
            .iter()
            .map(|(p, q)| {
                let f = lc::apply(&m, *q);
                let l = (p[0] + 2.0 * p[1] + p[2]).max(1e-4);
                ((0..3).map(|c| (f[c] - p[c]).powi(2)).sum::<f32>()).sqrt() / l
            })
            .collect();
        let mut sorted = res.clone();
        let mid = sorted.len() / 2;
        let scale = sorted.select_nth_unstable_by(mid, f32::total_cmp).1.max(1e-6) * 2.0;
        for (w, r) in weights.iter_mut().zip(&res) {
            *w = 1.0 / (1.0 + (r / scale).powi(2));
        }
        m = solve(&data, &weights);
    }
    Ok(Matched {
        matrix: m,
        samples: data.len(),
        before: mean_error(&data, &lc::diag([1.0; 3])),
        after: mean_error(&data, &m),
    })
}
