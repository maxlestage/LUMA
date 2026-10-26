//! Les corrections d'objectif écrites dans un DNG par le boîtier ou le logiciel qui l'a
//! produit : les listes d'opcodes (`OpcodeList2`, `OpcodeList3`, spécification DNG).
//!
//! Deux opcodes portent les corrections d'objectif :
//!
//! - `WarpRectilinear` (1) : la distorsion, radiale (polynôme en r², r⁴, r⁶) et
//!   tangentielle, par plan de couleur, ce qui corrige aussi l'aberration chromatique
//!   latérale quand chaque plan a ses coefficients ;
//! - `FixVignetteRadial` (3) : le vignetage, un gain 1 + k₀r² + … + k₄r¹⁰.
//!
//! Les rayons sont normalisés par la plus grande distance entre le centre optique et un
//! coin de l'image ; les coordonnées sont celles de l'image entière du capteur (avant le
//! recadrage par défaut), d'où le [`Frame`] qui situe l'image de LÜMA dedans.
//!
//! Les autres opcodes (cartes de gain, pixels morts…) ne sont pas des corrections
//! d'objectif : ils sont ignorés, comme le permet leur drapeau « facultatif ».

use luma_core::RgbImage;
use rayon::prelude::*;

/// Un opcode de correction d'objectif.
#[derive(Clone, Debug, PartialEq)]
pub enum Opcode {
    /// La distorsion : pour chaque plan (un seul, ou rouge, vert, bleu), les
    /// coefficients radiaux `kr0..kr3` et tangentiels `kt0, kt1`, puis le centre
    /// optique (0 à 1 en largeur et en hauteur).
    WarpRectilinear { planes: Vec<[f64; 6]>, center: [f64; 2] },
    /// Le vignetage : `k0..k4` et le centre optique.
    FixVignetteRadial { k: [f64; 5], center: [f64; 2] },
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn f64_at(b: &[u8], at: usize) -> Option<f64> {
    let v = f64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?);
    v.is_finite().then_some(v)
}

/// Lit une liste d'opcodes (toujours en gros‑boutiste) ; ne garde que les corrections
/// d'objectif bien formées.
pub fn parse(bytes: &[u8]) -> Vec<Opcode> {
    let mut out = Vec::new();
    let Some(count) = u32_at(bytes, 0) else { return out };
    let mut at = 4;
    for _ in 0..count {
        let (Some(id), Some(size)) = (u32_at(bytes, at), u32_at(bytes, at + 12)) else {
            break;
        };
        let params = at + 16;
        let Some(end) = params.checked_add(size as usize).filter(|e| *e <= bytes.len()) else {
            break;
        };
        let p = &bytes[params..end];
        match id {
            1 => {
                if let Some(n) = u32_at(p, 0).filter(|n| (1..=4).contains(n)) {
                    let n = n as usize;
                    let read = |i: usize| f64_at(p, 4 + 8 * i);
                    let planes: Option<Vec<[f64; 6]>> = (0..n)
                        .map(|k| {
                            let mut c = [0.0; 6];
                            for (j, v) in c.iter_mut().enumerate() {
                                *v = read(k * 6 + j)?;
                            }
                            Some(c)
                        })
                        .collect();
                    if let (Some(planes), Some(cx), Some(cy)) = (planes, read(n * 6), read(n * 6 + 1)) {
                        out.push(Opcode::WarpRectilinear {
                            planes,
                            center: [cx, cy],
                        });
                    }
                }
            }
            3 => {
                let read = |i: usize| f64_at(p, 8 * i);
                let k: Option<Vec<f64>> = (0..5).map(read).collect();
                if let (Some(k), Some(cx), Some(cy)) = (k, read(5), read(6)) {
                    out.push(Opcode::FixVignetteRadial {
                        k: [k[0], k[1], k[2], k[3], k[4]],
                        center: [cx, cy],
                    });
                }
            }
            _ => {}
        }
        at = end;
    }
    out
}

/// L'image entière du capteur, à laquelle se réfèrent les opcodes, vue depuis l'image
/// de LÜMA (en pixels de celle‑ci) : son origine (négative si l'image est recadrée) et
/// sa taille.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Frame {
    /// La même image, à une autre échelle (aperçu réduit).
    pub fn scaled(self, s: f64) -> Frame {
        Frame {
            x: self.x * s,
            y: self.y * s,
            width: self.width * s,
            height: self.height * s,
        }
    }

    /// Le centre optique en pixels, et la distance de ce centre au coin le plus loin.
    fn center(&self, c: [f64; 2]) -> (f64, f64, f64) {
        let (cx, cy) = (self.x + c[0] * self.width, self.y + c[1] * self.height);
        let mut m: f64 = 0.0;
        for (x, y) in [
            (self.x, self.y),
            (self.x + self.width, self.y),
            (self.x, self.y + self.height),
            (self.x + self.width, self.y + self.height),
        ] {
            m = m.max(((x - cx).powi(2) + (y - cy).powi(2)).sqrt());
        }
        (cx, cy, m.max(1.0))
    }
}

/// Le gain de vignetage au rayon normalisé `r²`.
fn vignette_gain(k: &[f64; 5], r2: f64) -> f64 {
    1.0 + r2 * (k[0] + r2 * (k[1] + r2 * (k[2] + r2 * (k[3] + r2 * k[4]))))
}

/// Applique les opcodes, dans l'ordre, à une image RGB linéaire.
pub fn apply(ops: &[Opcode], img: &mut RgbImage, frame: Frame) {
    for op in ops {
        match op {
            Opcode::FixVignetteRadial { k, center } => vignette(img, frame, k, *center),
            Opcode::WarpRectilinear { planes, center } => warp(img, frame, planes, *center),
        }
    }
}

/// Le vignetage, sur une mosaïque (une valeur par photosite) : `OpcodeList2`, avant le
/// dématriçage.
pub fn apply_to_mosaic(ops: &[Opcode], data: &mut [f32], width: usize, frame: Frame) {
    for op in ops {
        if let Opcode::FixVignetteRadial { k, center } = op {
            let (cx, cy, m) = frame.center(*center);
            data.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
                let dy = (y as f64 + 0.5 - cy) / m;
                for (x, v) in row.iter_mut().enumerate() {
                    let dx = (x as f64 + 0.5 - cx) / m;
                    *v *= vignette_gain(k, dx * dx + dy * dy) as f32;
                }
            });
        }
    }
}

fn vignette(img: &mut RgbImage, frame: Frame, k: &[f64; 5], center: [f64; 2]) {
    let (cx, cy, m) = frame.center(center);
    let w = img.width;
    img.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let dy = (y as f64 + 0.5 - cy) / m;
        for (x, px) in row.iter_mut().enumerate() {
            let dx = (x as f64 + 0.5 - cx) / m;
            let g = vignette_gain(k, dx * dx + dy * dy) as f32;
            for c in px.iter_mut() {
                *c *= g;
            }
        }
    });
}

/// La distorsion : chaque pixel corrigé va chercher sa valeur à l'endroit où l'objectif
/// l'a posé (interpolation bilinéaire).
fn warp(img: &mut RgbImage, frame: Frame, planes: &[[f64; 6]], center: [f64; 2]) {
    let (cx, cy, m) = frame.center(center);
    let (w, h) = (img.width, img.height);
    let src = img.data.clone();
    let sample = |x: f64, y: f64, c: usize| -> f32 {
        let x = x.clamp(0.0, (w - 1) as f64);
        let y = y.clamp(0.0, (h - 1) as f64);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
        let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
        let at = |x: usize, y: usize| src[y * w + x][c];
        let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * fx;
        let bottom = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * fx;
        top + (bottom - top) * fy
    };
    img.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let dy = (y as f64 + 0.5 - cy) / m;
        for (x, px) in row.iter_mut().enumerate() {
            let dx = (x as f64 + 0.5 - cx) / m;
            let r2 = dx * dx + dy * dy;
            for (c, v) in px.iter_mut().enumerate() {
                let k = planes[c.min(planes.len() - 1)];
                let f = k[0] + r2 * (k[1] + r2 * (k[2] + r2 * k[3]));
                let sx = dx * f + k[4] * 2.0 * dx * dy + k[5] * (r2 + 2.0 * dx * dx);
                let sy = dy * f + k[4] * (r2 + 2.0 * dy * dy) + k[5] * 2.0 * dx * dy;
                *v = sample(cx + sx * m - 0.5, cy + sy * m - 0.5, c);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une liste d'opcodes écrite comme dans un DNG.
    fn list(ops: &[(u32, u32, Vec<f64>)]) -> Vec<u8> {
        let mut b = (ops.len() as u32).to_be_bytes().to_vec();
        for (id, planes, values) in ops {
            let mut p = Vec::new();
            if *id == 1 {
                p.extend_from_slice(&planes.to_be_bytes());
            }
            for v in values {
                p.extend_from_slice(&v.to_be_bytes());
            }
            b.extend_from_slice(&id.to_be_bytes());
            b.extend_from_slice(&[1, 3, 0, 0]);
            b.extend_from_slice(&1u32.to_be_bytes());
            b.extend_from_slice(&(p.len() as u32).to_be_bytes());
            b.extend_from_slice(&p);
        }
        b
    }

    fn frame(w: usize, h: usize) -> Frame {
        Frame {
            x: 0.0,
            y: 0.0,
            width: w as f64,
            height: h as f64,
        }
    }

    #[test]
    fn lists_are_read() {
        let bytes = list(&[
            (9, 0, vec![1.0, 2.0]), // une carte de gain : ignorée
            (1, 1, vec![1.0, -0.05, 0.01, 0.0, 0.001, 0.0, 0.5, 0.5]),
            (3, 0, vec![0.4, 0.1, 0.0, 0.0, 0.0, 0.48, 0.52]),
        ]);
        assert_eq!(
            parse(&bytes),
            [
                Opcode::WarpRectilinear {
                    planes: vec![[1.0, -0.05, 0.01, 0.0, 0.001, 0.0]],
                    center: [0.5, 0.5]
                },
                Opcode::FixVignetteRadial {
                    k: [0.4, 0.1, 0.0, 0.0, 0.0],
                    center: [0.48, 0.52]
                },
            ]
        );
        // Tronquée ou vide : ce qui est entier est lu, sans jamais lire au‑delà.
        for cut in 0..bytes.len() {
            let _ = parse(&bytes[..cut]);
        }
        assert!(parse(&[]).is_empty());
    }

    #[test]
    fn vignetting_is_lifted_in_the_corners_only() {
        let (w, h) = (60, 40);
        let mut img = RgbImage::from_fn(w, h, |_, _| [0.5; 3]);
        let ops = [Opcode::FixVignetteRadial {
            k: [1.0, 0.0, 0.0, 0.0, 0.0],
            center: [0.5, 0.5],
        }];
        apply(&ops, &mut img, frame(w, h));
        let at = |x: usize, y: usize| img.data[y * w + x][0];
        assert!(
            (at(30, 20) - 0.5).abs() < 0.01,
            "le centre ne bouge pas : {}",
            at(30, 20)
        );
        // Au coin, r² ≈ 1 : gain 2.
        assert!((at(0, 0) - 1.0).abs() < 0.05, "coin : {}", at(0, 0));
        let mut mosaic = vec![0.5f32; w * h];
        apply_to_mosaic(&ops, &mut mosaic, w, frame(w, h));
        assert_eq!(mosaic[20 * w + 30], at(30, 20), "même gain sur la mosaïque");
    }

    #[test]
    fn identity_warp_changes_nothing_and_barrel_moves_points_outward() {
        let (w, h) = (80, 60);
        let original = RgbImage::from_fn(w, h, |x, y| [x as f32, y as f32, (x + y) as f32]);
        let mut img = original.clone();
        let identity = [Opcode::WarpRectilinear {
            planes: vec![[1.0, 0.0, 0.0, 0.0, 0.0, 0.0]],
            center: [0.5, 0.5],
        }];
        apply(&identity, &mut img, frame(w, h));
        for (a, b) in img.data.iter().zip(&original.data) {
            for c in 0..3 {
                assert!((a[c] - b[c]).abs() < 1e-3);
            }
        }
        // kr1 > 0 : chaque pixel prend sa valeur plus loin du centre.
        let mut img = original.clone();
        let barrel = [Opcode::WarpRectilinear {
            planes: vec![[1.0, 0.1, 0.0, 0.0, 0.0, 0.0]],
            center: [0.5, 0.5],
        }];
        apply(&barrel, &mut img, frame(w, h));
        let x = 60;
        assert!(img.data[30 * w + x][0] > x as f32, "{}", img.data[30 * w + x][0]);
        assert!((img.data[30 * w + 40][0] - 40.0).abs() < 0.6, "le centre reste");
        // Trois plans : chaque couleur a sa propre distorsion (aberration latérale).
        let mut img = original.clone();
        let ca = [Opcode::WarpRectilinear {
            planes: vec![
                [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                [1.01, 0.0, 0.0, 0.0, 0.0, 0.0],
            ],
            center: [0.5, 0.5],
        }];
        apply(&ca, &mut img, frame(w, h));
        let p = img.data[30 * w + 70];
        assert!((p[0] - 70.0).abs() < 1e-3 && (p[2] - original.data[30 * w + 70][2]).abs() > 0.1);
    }

    #[test]
    fn a_cropped_image_keeps_the_sensor_center() {
        // L'image de LÜMA est la moitié droite du capteur : le centre optique (au milieu
        // du capteur) est sur son bord gauche.
        let (w, h) = (50, 40);
        let mut img = RgbImage::from_fn(w, h, |_, _| [0.5; 3]);
        let ops = [Opcode::FixVignetteRadial {
            k: [1.0, 0.0, 0.0, 0.0, 0.0],
            center: [0.5, 0.5],
        }];
        let sensor = Frame {
            x: -50.0,
            y: 0.0,
            width: 100.0,
            height: 40.0,
        };
        apply(&ops, &mut img, sensor);
        assert!(
            img.data[20 * w][0] < img.data[20 * w + w - 1][0],
            "plus clair loin du centre"
        );
        assert!((img.data[20 * w][0] - 0.5).abs() < 0.01);
    }
}
