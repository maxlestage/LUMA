//! Panneaux Corrections de l'objectif et Transformation.
//!
//! Une seule passe géométrique, sur l'image caméra orientée et avant le recadrage :
//! pour chaque pixel de sortie, on remonte à sa position dans la photo d'origine
//! (perspective, échelle et proportions, puis distorsion de l'objectif, puis
//! aberration chromatique latérale couche par couche) et on y lit la couleur. Le
//! vignetage de l'objectif se corrige au même endroit, selon la distance au centre
//! optique de la photo d'origine.
//!
//! Les coordonnées sont normalisées par la demi‑diagonale : un coin est à 1 du centre.

use luma_color::luminance;
use luma_core::{Plane, RgbImage};
use luma_project::{Lens, Transform};
use rayon::prelude::*;

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// La focale supposée, en demi‑diagonales (environ un 35 mm sur plein format).
pub(crate) const FOCAL: f32 = 1.25;

type M3 = [[f32; 3]; 3];

fn mul(a: &M3, b: &M3) -> M3 {
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

/// La rotation de la caméra virtuelle, transposée (de la sortie vers l'origine).
fn rotation(t: &Transform) -> M3 {
    // Verticale négative élargit le haut (verticales qui
    // convergent quand on vise vers le haut).
    let (sx, cx) = (-t.vertical / 100.0 * 25f32.to_radians()).sin_cos();
    let (sy, cy) = (t.horizontal / 100.0 * 25f32.to_radians()).sin_cos();
    let (sz, cz) = t.rotate.to_radians().sin_cos();
    let rx = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
    let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
    let rz = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
    let r = mul(&rz, &mul(&ry, &rx));
    // Transposée = inverse d'une rotation.
    [
        [r[0][0], r[1][0], r[2][0]],
        [r[0][1], r[1][1], r[2][1]],
        [r[0][2], r[1][2], r[2][2]],
    ]
}

/// La correspondance d'un point de sortie (normalisé) vers la photo d'origine, avant
/// l'aberration chromatique : `None` s'il tombe derrière la caméra virtuelle.
pub struct Mapping {
    rt: M3,
    perspective: bool,
    sx: f32,
    sy: f32,
    k: f32,
    /// Le centre de la sortie reste le centre de la photo.
    center: (f32, f32),
}

impl Mapping {
    pub fn new(lens: &Lens, t: &Transform) -> Mapping {
        let s = t.scale / 100.0;
        let a = t.aspect / 100.0 * 0.25;
        let mut m = Mapping {
            rt: rotation(t),
            perspective: t.vertical != 0.0 || t.horizontal != 0.0 || t.rotate != 0.0,
            sx: s * a.exp2(),
            sy: s * (-a).exp2(),
            k: -0.15 * lens.distortion / 100.0,
            center: (0.0, 0.0),
        };
        if m.perspective {
            m.center = m.project(0.0, 0.0).unwrap_or((0.0, 0.0));
        }
        m
    }

    #[inline]
    fn project(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let m = &self.rt;
        let v = [
            m[0][0] * x + m[0][1] * y + m[0][2] * FOCAL,
            m[1][0] * x + m[1][1] * y + m[1][2] * FOCAL,
            m[2][0] * x + m[2][1] * y + m[2][2] * FOCAL,
        ];
        (v[2] > 1e-3).then(|| (v[0] / v[2] * FOCAL, v[1] / v[2] * FOCAL))
    }

    #[inline]
    pub fn map(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let (mut x, mut y) = (x / self.sx, y / self.sy);
        if self.perspective {
            let (px, py) = self.project(x, y)?;
            (x, y) = (px - self.center.0, py - self.center.1);
        }
        let d = 1.0 + self.k * (x * x + y * y);
        Some((x * d, y * d))
    }
}

/// Corrige la géométrie et le vignetage de l'objectif. `ca` : l'échelle relative des
/// couches rouge et bleue (voir [`estimate_ca`]), `(0, 0)` pour ne rien corriger.
/// `None` s'il n'y a rien à faire.
pub fn correct(src: &RgbImage, lens: &Lens, t: &Transform, ca: (f32, f32)) -> Option<RgbImage> {
    let geometry = lens.distortion != 0.0 || !t.is_identity() || ca != (0.0, 0.0);
    if !geometry && lens.vignetting == 0.0 {
        return None;
    }
    let (w, h) = (src.width, src.height);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let hd = (cx * cx + cy * cy).sqrt();
    let mapping = Mapping::new(lens, t);
    let v = lens.vignetting / 100.0;
    let mid = 0.15 + 0.6 * lens.vignetting_midpoint / 100.0;
    let mut out = RgbImage::new(w, h);
    out.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let ny = (y as f32 + 0.5 - cy) / hd;
        for (x, p) in row.iter_mut().enumerate() {
            let nx = (x as f32 + 0.5 - cx) / hd;
            let Some((sx, sy)) = (if geometry { mapping.map(nx, ny) } else { Some((nx, ny)) }) else {
                *p = [0.0; 3];
                continue;
            };
            let at = |k: f32| (sx * (1.0 + k) * hd + cx - 0.5, sy * (1.0 + k) * hd + cy - 0.5);
            let inside =
                |(px, py): (f32, f32)| px >= -0.5 && py >= -0.5 && px <= w as f32 - 0.5 && py <= h as f32 - 0.5;
            let g = at(0.0);
            if !inside(g) {
                *p = [0.0; 3];
                continue;
            }
            let mut c = src.sample(g.0, g.1);
            if ca != (0.0, 0.0) {
                let r = at(ca.0);
                let b = at(ca.1);
                c[0] = src.sample(r.0, r.1)[0];
                c[2] = src.sample(b.0, b.1)[2];
            }
            if v != 0.0 {
                let r = (sx * sx + sy * sy).sqrt();
                let gain = (v * 1.6 * smoothstep(mid, 1.25, r)).exp2();
                c = c.map(|x| x * gain);
            }
            *p = c;
        }
    });
    Some(out)
}

/// L'aberration chromatique latérale : de combien les couches rouge et bleue sont plus
/// grandes ou plus petites que la verte (en proportion, autour de 0). Cherchée sur les
/// contours nets loin du centre, là où elle se voit.
pub fn estimate_ca(src: &RgbImage) -> (f32, f32) {
    let (w, h) = (src.width, src.height);
    if w < 64 || h < 64 {
        return (0.0, 0.0);
    }
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let hd = (cx * cx + cy * cy).sqrt();
    let green = Plane::from_rgb(src, |p| p[1].max(1e-6).ln());
    // Points d'échantillonnage : forts contours verts, rayon > 0,35.
    let mut points = Vec::new();
    let step = ((w * h) / 400_000).max(1);
    for y in (2..h - 2).step_by(step.min(h / 64).max(1)) {
        for x in (2..w - 2).step_by(step.min(w / 64).max(1)) {
            let (nx, ny) = ((x as f32 - cx) / hd, (y as f32 - cy) / hd);
            let r = (nx * nx + ny * ny).sqrt();
            if r < 0.35 {
                continue;
            }
            let gx = green.data[y * w + x + 1] - green.data[y * w + x - 1];
            let gy = green.data[(y + 1) * w + x] - green.data[(y - 1) * w + x];
            // Le contour doit être radial (la frange de l'aberration l'est).
            let radial = ((gx * nx + gy * ny) / r).abs();
            if radial > 0.25 {
                points.push((x, y));
            }
        }
    }
    if points.len() < 200 {
        return (0.0, 0.0);
    }
    if points.len() > 30_000 {
        let k = points.len() / 30_000 + 1;
        points = points.into_iter().step_by(k).collect();
    }
    let channel_ln = |c: usize, x: f32, y: f32| src.sample(x, y)[c].max(1e-6).ln();
    let best = |c: usize| -> f32 {
        let candidates: Vec<f32> = (-12..=12).map(|i| i as f32 * 0.00025).collect();
        // Les écarts de niveau entre couches sont retirés par la moyenne (en log).
        let errors: Vec<f32> = candidates
            .par_iter()
            .map(|&k| {
                let diffs: Vec<f32> = points
                    .iter()
                    .map(|&(x, y)| {
                        let (px, py) = (x as f32 - cx, y as f32 - cy);
                        channel_ln(c, cx + px * (1.0 + k), cy + py * (1.0 + k)) - green.data[y * w + x]
                    })
                    .collect();
                let mean = diffs.iter().sum::<f32>() / diffs.len() as f32;
                diffs.iter().map(|d| (d - mean).abs()).sum::<f32>()
            })
            .collect();
        let (i, _) = errors
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .expect("candidats");
        candidates[i]
    };
    (best(0), best(2))
}

/// Défrangeage : les franges violettes et vertes le long des contours très contrastés
/// perdent leur couleur. `scale` : pixels rendus par pixel pleine définition.
pub fn defringe(img: &mut RgbImage, lens: &Lens, scale: f32) {
    let (purple, green) = (lens.defringe_purple / 20.0, lens.defringe_green / 20.0);
    if purple <= 0.0 && green <= 0.0 {
        return;
    }
    let stops = Plane::from_rgb(img, |p| luminance(p).max(1e-6).log2());
    let blurred = stops.blur((2.0 * scale).max(0.6));
    img.data
        .par_iter_mut()
        .zip(stops.data.par_iter().zip(blurred.data.par_iter()))
        .for_each(|(p, (&s, &b))| {
            let edge = smoothstep(0.15, 0.7, (s - b).abs());
            if edge <= 0.0 {
                return;
            }
            let [r, g, bl] = *p;
            let top = r.max(g).max(bl).max(1e-6);
            // Violet : rouge et bleu au‑dessus du vert. Vert : le vert au‑dessus des deux.
            let violet = smoothstep(0.05, 0.35, (r.min(bl) - g) / top) * purple;
            let verdant = smoothstep(0.05, 0.35, (g - r.max(bl)) / top) * green;
            let k = (edge * violet.max(verdant)).min(1.0);
            if k > 0.0 {
                let y = luminance(*p);
                *p = p.map(|v| v + (y - v) * k);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_to_do_by_default() {
        let img = RgbImage::from_fn(8, 8, |_, _| [0.5; 3]);
        assert!(correct(&img, &Lens::default(), &Transform::default(), (0.0, 0.0)).is_none());
    }

    #[test]
    fn negative_vertical_widens_the_top_and_keeps_the_center() {
        let t = Transform {
            vertical: -50.0,
            ..Transform::default()
        };
        let m = Mapping::new(&Lens::default(), &t);
        // Largeur dans l'original de la ligne du haut et du bas de la sortie.
        let width = |y: f32| m.map(0.5, y).unwrap().0 - m.map(-0.5, y).unwrap().0;
        assert!(
            width(-0.5) < width(0.5),
            "le haut vient d'une zone plus étroite : il s'élargit"
        );
        let (cx, cy) = m.map(0.0, 0.0).unwrap();
        assert!(cx.abs() < 1e-5 && cy.abs() < 1e-5);
    }

    #[test]
    fn positive_distortion_samples_the_edges_closer_to_the_center() {
        let lens = Lens {
            distortion: 50.0,
            ..Lens::default()
        };
        let m = Mapping::new(&lens, &Transform::default());
        let (x, _) = m.map(0.8, 0.0).unwrap();
        assert!(x < 0.8 && x > 0.7);
        assert_eq!(m.map(0.0, 0.0), Some((0.0, 0.0)));
    }

    #[test]
    fn lens_vignetting_brightens_corners_only() {
        let img = RgbImage::from_fn(64, 48, |_, _| [0.2; 3]);
        let lens = Lens {
            vignetting: 60.0,
            ..Lens::default()
        };
        let out = correct(&img, &lens, &Transform::default(), (0.0, 0.0)).unwrap();
        assert!((out.pixel(32, 24)[0] - 0.2).abs() < 1e-4, "centre intact");
        assert!(out.pixel(0, 0)[0] > 0.25, "coin éclairci");
    }

    #[test]
    fn finds_a_synthetic_chromatic_aberration() {
        // Des anneaux concentriques ; le rouge est 0,1 % plus grand que le vert.
        let (w, h) = (600, 400);
        let ring = |x: f32, y: f32, k: f32| {
            let (dx, dy) = ((x - 300.0) / (1.0 + k), (y - 200.0) / (1.0 + k));
            // Lissés, comme une vraie photo l'est par l'objectif.
            0.45 + 0.35 * ((dx * dx + dy * dy).sqrt() / 3.0).sin()
        };
        let img = RgbImage::from_fn(w, h, |x, y| {
            let (x, y) = (x as f32, y as f32);
            [ring(x, y, 0.001), ring(x, y, 0.0), ring(x, y, -0.0015)]
        });
        let (r, b) = estimate_ca(&img);
        assert!((r - 0.001).abs() <= 0.00026, "rouge {r}");
        assert!((b + 0.0015).abs() <= 0.00026, "bleu {b}");
        // Et la correction réaligne les couches.
        let fixed = correct(&img, &Lens::default(), &Transform::default(), (r, b)).unwrap();
        let mismatch = |im: &RgbImage| {
            im.data
                .iter()
                .map(|p| ((p[0] - p[1]).abs() + (p[2] - p[1]).abs()) as f64)
                .sum::<f64>()
        };
        assert!(mismatch(&fixed) < mismatch(&img) * 0.5);
    }

    #[test]
    fn defringe_removes_purple_on_edges() {
        let mut img = RgbImage::from_fn(40, 10, |x, _| {
            if x < 19 {
                [0.02, 0.02, 0.02]
            } else if x < 21 {
                [0.4, 0.1, 0.5]
            } else {
                [0.9, 0.9, 0.9]
            }
        });
        let lens = Lens {
            defringe_purple: 20.0,
            ..Lens::default()
        };
        defringe(&mut img, &lens, 1.0);
        let p = img.pixel(19, 5);
        assert!((p[0] - p[1]).abs() < 0.12 && (p[2] - p[1]).abs() < 0.15, "{p:?}");
    }
}
