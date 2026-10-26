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
//!
//! Les boîtiers qui n'écrivent pas de DNG rangent les mêmes corrections dans leur RAW,
//! sous forme de courbes le long du rayon ([`Opcode::Spline`], voir [`sony`]).

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
    /// Les corrections d'un boîtier, en courbes définies par des nœuds le long du rayon
    /// (0 au centre de l'image, 1 au coin) : le rapport entre le rayon où l'objectif a
    /// posé un point et celui où il devrait être, pour le rouge, le vert et le bleu, et
    /// la part de lumière qui arrive (le vignetage). `zoom` agrandit l'image juste
    /// assez pour que la distorsion corrigée ne laisse aucun bord vide.
    Spline {
        knots: Vec<f64>,
        distortion: [Vec<f64>; 3],
        vignetting: Vec<f64>,
        zoom: f64,
    },
}

/// Une courbe linéaire par morceaux, constante au‑delà de ses nœuds.
fn spline(knots: &[f64], values: &[f64], r: f64) -> f64 {
    let n = knots.len().min(values.len());
    if n == 0 {
        return 1.0;
    }
    if r <= knots[0] {
        return values[0];
    }
    for i in 1..n {
        if r <= knots[i] {
            let t = (r - knots[i - 1]) / (knots[i] - knots[i - 1]).max(1e-12);
            return values[i - 1] + t * (values[i] - values[i - 1]);
        }
    }
    values[n - 1]
}

/// Le plus petit agrandissement (≥ 1) pour lequel le coin de l'image corrigée va encore
/// chercher ses pixels dans la photo : `r·d(r) ≤ 1` au rayon `r = 1/zoom`.
fn zoom_for(knots: &[f64], distortion: &[Vec<f64>; 3]) -> f64 {
    let reach = |z: f64| {
        let r = 1.0 / z;
        distortion.iter().map(|d| r * spline(knots, d, r)).fold(0.0, f64::max)
    };
    if reach(1.0) <= 1.0 {
        return 1.0;
    }
    let (mut lo, mut hi) = (1.0, 2.0);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if reach(mid) <= 1.0 { hi = mid } else { lo = mid }
    }
    hi
}

/// Seize nœuds réguliers, du centre (0) au coin (1).
fn regular_knots() -> Vec<f64> {
    (0..16).map(|i| f64::from(i) / 15.0).collect()
}

/// Les corrections enregistrées par un boîtier Olympus ou OM System dans son ORF
/// (ImageProcessing 0x150A : distorsion, 0x150C : aberration chromatique) : la distorsion
/// est `[k2, k4, k6, s]`, le rapport entre le rayon où l'objectif a posé un point et
/// celui où il devrait être valant `s·(1 + k2·ρ² + k4·ρ⁴ + k6·ρ⁶)` avec `ρ = s·r` ;
/// l'aberration chromatique, `[r0, r2, r4, b0, b2, b4]`, multiplie ce rapport par
/// `1 + r0 + r2·d² + r4·d⁴` pour le rouge (et de même pour le bleu), `d` étant le rayon
/// où tombe le vert. Pas de vignetage enregistré.
pub fn olympus(distortion: &[f64], ca: &[f64]) -> Option<Opcode> {
    let ([k2, k4, k6, s], ca) = (<[f64; 4]>::try_from(distortion).ok()?, ca);
    if (k2 == 0.0 && k4 == 0.0 && k6 == 0.0) || s <= 0.0 || !s.is_finite() {
        return None;
    }
    let ca: [f64; 6] = ca.try_into().unwrap_or([0.0; 6]);
    let knots = regular_knots();
    let green: Vec<f64> = knots
        .iter()
        .map(|r| {
            let rho2 = (r * s).powi(2);
            s * (1.0 + rho2 * (k2 + rho2 * (k4 + rho2 * k6)))
        })
        .collect();
    let tint = |c0: f64, c2: f64, c4: f64| -> Vec<f64> {
        knots
            .iter()
            .zip(&green)
            .map(|(r, g)| {
                let d2 = (g * r).powi(2);
                g * (1.0 + c0 + d2 * (c2 + d2 * c4))
            })
            .collect()
    };
    let distortion = [tint(ca[0], ca[1], ca[2]), green.clone(), tint(ca[3], ca[4], ca[5])];
    let zoom = zoom_for(&knots, &distortion);
    Some(Opcode::Spline {
        vignetting: vec![1.0; knots.len()],
        knots,
        distortion,
        zoom,
    })
}

/// La distorsion enregistrée par un boîtier Panasonic dans son RW2 (étiquette 0x0119 :
/// seize entiers de 16 bits). Active quand `v[7] & 0xF == 1` ; le modèle va du rayon
/// déformé `d` au rayon juste `u` : `u = d + e·(a·d³ + b·d⁵ + c·d⁷)`, avec `a = v[8]`,
/// `b = v[4]`, `c = v[11]` (en 32768ᵉ) et `e = 1/(1 + v[5]/32768)`. LÜMA a besoin de
/// l'inverse (d'où vient le point d'un rayon juste), trouvé par itérations.
pub fn panasonic(v: &[i16]) -> Option<Opcode> {
    let v: [i16; 16] = v.try_into().ok()?;
    if v[7] & 0x0F != 1 {
        return None;
    }
    let q = |i: usize| f64::from(v[i]) / 32768.0;
    let scale = 1.0 / (1.0 + q(5));
    let (a, b, c) = (q(8), q(4), q(11));
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let knots = regular_knots();
    let ratio: Vec<f64> = knots
        .iter()
        .map(|&r| {
            let mut d = r;
            for _ in 0..4 {
                let d2 = d * d;
                let f = 1.0 + scale * d2 * (a + d2 * (b + d2 * c));
                d = if f > 1e-6 { r / f } else { r };
            }
            if r > 0.0 { d / r } else { 1.0 }
        })
        .collect();
    let distortion = [ratio.clone(), ratio.clone(), ratio];
    let zoom = zoom_for(&knots, &distortion);
    Some(Opcode::Spline {
        vignetting: vec![1.0; knots.len()],
        knots,
        distortion,
        zoom,
    })
}

/// Les corrections enregistrées par un boîtier Fujifilm dans son RAF (étiquettes 0xF00B :
/// distorsion, 0xF00F : aberration chromatique, 0xF010 : vignetage, du FujiIFD), telles
/// que lues : une valeur d'en‑tête, puis les nœuds (9 sur les capteurs X‑Trans IV et V,
/// 11 avant), puis les valeurs, en pour cent pour la distorsion et le vignetage, en
/// rapport pour l'aberration chromatique (rouge puis bleu ; sans premier nœud avant le
/// X‑Trans IV). `crop` : 1,25 quand le boîtier photographiait en mode recadré, les
/// nœuds étant comptés sur l'image entière.
pub fn fuji(distortion: &[f64], ca: &[f64], vignetting: &[f64], crop: f64) -> Option<Opcode> {
    // (nombre de nœuds, début des valeurs de distorsion et de vignetage, du rouge, du bleu)
    let (n, values, red, blue, ca_first_knot) = match (distortion.len(), ca.len(), vignetting.len()) {
        (19, 29, 19) => (9, 10, 10, 19, true),
        (23, 31, 23) => (11, 12, 10, 20, false),
        _ => return None,
    };
    let knots: Vec<f64> = (0..n).map(|i| distortion[i + 1] * crop).collect();
    // Les trois listes disent les mêmes nœuds, sinon elles ne vont pas ensemble.
    let same = (0..n).all(|i| {
        let ca_knot = if ca_first_knot {
            Some(ca[i + 1])
        } else {
            (i > 0).then(|| ca[i])
        };
        vignetting[i + 1] == distortion[i + 1] && ca_knot.is_none_or(|k| k == distortion[i + 1])
    });
    if !same || knots.windows(2).any(|w| w[1] <= w[0]) {
        return None;
    }
    let green: Vec<f64> = (0..n).map(|i| distortion[values + i] / 100.0 + 1.0).collect();
    let tint = |start: usize| -> Vec<f64> {
        (0..n)
            .map(|i| {
                let c = if !ca_first_knot && i == 0 { 0.0 } else { ca[start + i] };
                green[i] * (c + 1.0)
            })
            .collect()
    };
    let distortion = [tint(red), green.clone(), tint(blue)];
    let vignetting: Vec<f64> = (0..n).map(|i| vignetting[values + i] / 100.0).collect();
    if vignetting.iter().any(|v| *v <= 0.0) {
        return None;
    }
    let zoom = zoom_for(&knots, &distortion);
    Some(Opcode::Spline {
        knots,
        distortion,
        vignetting,
        zoom,
    })
}

/// Les corrections enregistrées par un boîtier Sony dans son ARW (étiquettes 0x7037 :
/// distorsion, 0x7035 : aberration chromatique, rouge puis bleu, 0x7032 : vignetage ;
/// chacune commence par son nombre de valeurs). Des entiers, à l'échelle 2⁻¹⁴ pour la
/// distorsion, 2⁻²¹ pour l'aberration chromatique et 2⁻¹³ pour le vignetage, sur des
/// nœuds placés en (i + ½)/(n − 1) du rayon.
pub fn sony(distortion: &[i32], ca: &[i32], vignetting: &[i32]) -> Option<Opcode> {
    let n = usize::try_from(*distortion.first()?).ok()?;
    let ok = (2..=16).contains(&n)
        && distortion.len() > n
        && ca.first().is_some_and(|c| *c as usize == 2 * n && ca.len() > 2 * n)
        && vignetting
            .first()
            .is_some_and(|v| *v as usize == n && vignetting.len() > n);
    if !ok {
        return None;
    }
    let knots: Vec<f64> = (0..n).map(|i| (i as f64 + 0.5) / (n - 1) as f64).collect();
    let green: Vec<f64> = (0..n)
        .map(|i| f64::from(distortion[i + 1]) * 2f64.powi(-14) + 1.0)
        .collect();
    let tint = |offset: usize| -> Vec<f64> {
        (0..n)
            .map(|i| green[i] * (f64::from(ca[offset + i + 1]) * 2f64.powi(-21) + 1.0))
            .collect()
    };
    let distortion = [tint(0), green.clone(), tint(n)];
    let vignetting = (0..n)
        .map(|i| 2f64.powf(0.5 - 2f64.powf(f64::from(vignetting[i + 1]) * 2f64.powi(-13) - 1.0)))
        .collect();
    let zoom = zoom_for(&knots, &distortion);
    Some(Opcode::Spline {
        knots,
        distortion,
        vignetting,
        zoom,
    })
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
            Opcode::Spline {
                knots,
                distortion,
                vignetting,
                zoom,
            } => splines(img, frame, knots, distortion, vignetting, *zoom),
        }
    }
}

/// Les corrections en courbes, en une passe : chaque pixel corrigé va chercher sa
/// valeur, couleur par couleur, là où l'objectif l'a posée, et lui rend la lumière que
/// l'objectif y a perdue.
///
/// Tout ce qui dépend du rayon est tabulé une fois, en fonction de son carré (pas de
/// racine ni de recherche par pixel) : le rapport de rayon du rouge, du vert et du bleu,
/// et le gain de lumière au point d'où vient le vert (le rouge et le bleu en viennent à
/// moins d'un millième de rayon près).
fn splines(img: &mut RgbImage, frame: Frame, knots: &[f64], distortion: &[Vec<f64>; 3], vignetting: &[f64], zoom: f64) {
    const N: usize = 1024;
    let (cx, cy, m) = frame.center([0.5, 0.5]);
    // Les rayons de l'image corrigée vont de 0 à 1 sur l'échelle agrandie.
    let table: Vec<[f32; 4]> = (0..=N + 1)
        .map(|i| {
            let r = (i as f64 / N as f64).sqrt();
            let d = distortion.each_ref().map(|d| spline(knots, d, r));
            let light = 1.0 / spline(knots, vignetting, r * zoom * d[1]).max(1e-4);
            [d[0] as f32, d[1] as f32, d[2] as f32, light as f32]
        })
        .collect();
    let (w, h) = (img.width, img.height);
    // L'image corrigée s'écrit dans une image neuve (des pages mises à zéro par le
    // système, remplies en parallèle) : pas de copie de la source, qui coûtait un tiers
    // du temps.
    let src = std::mem::take(&mut img.data);
    let mut out = vec![[0.0f32; 3]; w * h];
    let (cx, cy, unit) = (cx as f32, cy as f32, (m * zoom) as f32);
    let (mx, my) = ((w - 1) as f32, (h - 1) as f32);
    // L'écart au centre de chaque colonne, calculé une fois (la même formule, donc les
    // mêmes valeurs au bit près).
    let columns: Vec<f32> = (0..w).map(|x| (x as f32 + 0.5 - cx) / unit).collect();
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let dy = (y as f32 + 0.5 - cy) / unit;
        for (px, &dx) in row.iter_mut().zip(&columns) {
            let t = ((dx * dx + dy * dy) * N as f32).min(N as f32);
            let k = t as usize;
            let f = t - k as f32;
            let (a, b) = (table[k], table[k + 1]);
            let light = a[3] + (b[3] - a[3]) * f;
            for (c, v) in px.iter_mut().enumerate() {
                let d = a[c] + (b[c] - a[c]) * f;
                let sx = (cx + dx * d * unit - 0.5).clamp(0.0, mx);
                let sy = (cy + dy * d * unit - 0.5).clamp(0.0, my);
                let (x0, y0) = (sx as usize, sy as usize);
                let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
                let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
                let (p, q) = (src[y0 * w + x0][c], src[y0 * w + x1][c]);
                let top = p + (q - p) * fx;
                let (p, q) = (src[y1 * w + x0][c], src[y1 * w + x1][c]);
                let bottom = p + (q - p) * fx;
                *v = (top + (bottom - top) * fy) * light;
            }
        }
    });
    img.data = out;
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
    remap(img, |x, y, c| {
        let (dx, dy) = ((x - cx) / m, (y - cy) / m);
        let r2 = dx * dx + dy * dy;
        let k = planes[c.min(planes.len() - 1)];
        let f = k[0] + r2 * (k[1] + r2 * (k[2] + r2 * k[3]));
        let sx = dx * f + k[4] * 2.0 * dx * dy + k[5] * (r2 + 2.0 * dx * dx);
        let sy = dy * f + k[4] * (r2 + 2.0 * dy * dy) + k[5] * 2.0 * dx * dy;
        (cx + sx * m, cy + sy * m)
    });
}

/// Chaque pixel corrigé va chercher sa valeur à l'endroit où l'objectif l'a posé :
/// `source(x, y, couleur)`, au centre du pixel, donne ce point (interpolation
/// bilinéaire).
fn remap(img: &mut RgbImage, source: impl Fn(f64, f64, usize) -> (f64, f64) + Sync) {
    let (w, h) = (img.width, img.height);
    // Comme pour les courbes : une image neuve, pas de copie de la source.
    let src = std::mem::take(&mut img.data);
    let mut out = vec![[0.0f32; 3]; w * h];
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
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, px) in row.iter_mut().enumerate() {
            for (c, v) in px.iter_mut().enumerate() {
                let (sx, sy) = source(x as f64 + 0.5, y as f64 + 0.5, c);
                *v = sample(sx - 0.5, sy - 0.5, c);
            }
        }
    });
    img.data = out;
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
    fn fujifilm_lists_of_both_generations() {
        // X‑Trans IV et V : 9 nœuds partout.
        let knots: Vec<f64> = (1..=9).map(|i| f64::from(i) / 8.0).collect();
        let list = |values: &[f64]| [&[416.0][..], &knots, values].concat();
        let d = list(&[-1.0, -2.0, -3.0, -4.0, -5.0, -6.0, -7.0, -8.0, -9.0]);
        let ca = [list(&[-0.001; 9]), vec![0.002; 9], vec![416.0]].concat();
        let v = list(&[100.0, 99.0, 98.0, 97.0, 96.0, 95.0, 94.0, 93.0, 92.0]);
        let Some(Opcode::Spline {
            knots: k,
            distortion,
            vignetting,
            zoom,
        }) = fuji(&d, &ca, &v, 1.0)
        else {
            panic!()
        };
        assert_eq!(k, knots);
        assert!((distortion[1][8] - 0.91).abs() < 1e-9);
        assert!((distortion[0][0] - 0.99 * 0.999).abs() < 1e-9 && (distortion[2][0] - 0.99 * 1.002).abs() < 1e-9);
        assert!((vignetting[8] - 0.92).abs() < 1e-9);
        assert_eq!(zoom, 1.0);
        // En mode recadré, les nœuds se comptent sur l'image entière.
        let Some(Opcode::Spline { knots: k, .. }) = fuji(&d, &ca, &v, 1.25) else {
            panic!()
        };
        assert!((k[7] - 1.25).abs() < 1e-9);
        // Avant le X‑Trans IV : 11 nœuds, l'aberration chromatique sans le premier.
        let knots11: Vec<f64> = (0..11).map(|i| f64::from(i) / 10.0).collect();
        let d11 = [&[0.0][..], &knots11, &[-1.0; 11]].concat();
        let ca11 = [&[0.0][..], &knots11[1..], &[0.001; 10], &[-0.001; 10]].concat();
        let v11 = [&[0.0][..], &knots11, &[100.0; 11]].concat();
        let Some(Opcode::Spline { distortion, .. }) = fuji(&d11, &ca11, &v11, 1.0) else {
            panic!("{} {} {}", d11.len(), ca11.len(), v11.len())
        };
        assert_eq!(distortion[0][0], distortion[1][0], "pas d'aberration au centre");
        assert!(distortion[0][5] > distortion[1][5]);
        // Des nœuds qui ne vont pas ensemble : rien.
        let mut bad = v.clone();
        bad[3] += 0.01;
        assert!(fuji(&d, &ca, &bad, 1.0).is_none());
        assert!(fuji(&d[..10], &ca, &v, 1.0).is_none());
    }

    #[test]
    fn olympus_and_panasonic_without_correction() {
        assert!(olympus(&[0.0, 0.0, 0.0, 1.0], &[0.0; 6]).is_none(), "coefficients nuls");
        assert!(olympus(&[0.1, 0.0], &[]).is_none(), "liste incomplète");
        let Some(Opcode::Spline { distortion, .. }) = olympus(&[-0.1, 0.0, 0.0, 1.0], &[]) else {
            panic!()
        };
        assert!((distortion[1][15] - 0.9).abs() < 1e-9 && distortion[0] == distortion[1]);
        let mut v = [0i16; 16];
        v[8] = 3277; // a ≈ 0,1
        assert!(panasonic(&v).is_none(), "correction désactivée");
        v[7] = 1;
        let Some(Opcode::Spline { distortion, .. }) = panasonic(&v) else {
            panic!()
        };
        // u = d + 0,1·d³ ; au coin (u = 1), d ≈ 0,9217.
        assert!((distortion[1][15] - 0.9217).abs() < 1e-3, "{}", distortion[1][15]);
        assert!(panasonic(&v[..8]).is_none());
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
