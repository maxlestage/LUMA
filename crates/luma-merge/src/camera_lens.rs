//! La distorsion de l'objectif, mesurée sur le JPEG que le boîtier a rangé dans le RAW :
//! le boîtier l'a déjà corrigée, pas le RAW. Les mêmes détails trouvés des deux côtés
//! donnent, par moindres carrés, le modèle du curseur Distorsion de LÜMA
//! (`source = sortie × (1 + k r²)`, `k = −0,15 × curseur / 100`) : de quoi corriger les
//! objectifs des marques dont LÜMA ne lit pas les corrections (Canon, Nikon, Pentax…).
//! Le vignetage aussi : la courbe de tons du boîtier apprise au centre de l'image, puis
//! défaite partout, laisse le gain des bords.

use luma_core::{Error, Result, RgbImage};

use crate::features;

/// Un appariement : le point du JPEG, celui du rendu (normalisés).
type Match = ((f64, f64), (f64, f64));

/// Côté des images comparées : assez pour placer les détails au pixel près.
const SIDE: usize = 1600;

/// Le réglage Distorsion (−100 à 100) qui rend `photo`, développée avec `settings`,
/// comme le JPEG que le boîtier a rangé dans son RAW `path`. Proche de 0 quand les
/// deux ont la même géométrie (corrections déjà appliquées, objectif sans distorsion).
pub fn distortion(
    photo: &luma_render::Photo,
    path: &std::path::Path,
    settings: &luma_project::Settings,
) -> Result<f32> {
    let pair = crate::film::camera_pair(photo, path)?;
    let render = if *settings == luma_project::Settings::default() {
        pair.digital
    } else {
        crate::film::reference_with(photo, settings)?
    };
    let k = radial_k(&render, &pair.film)?;
    Ok((-k / 0.15 * 100.0).clamp(-100.0, 100.0))
}

/// Le `k` du modèle de LÜMA qui amène `jpeg` (la géométrie voulue) sur `render` : les
/// détails appariés, normalisés par la demi‑diagonale, ajustés sur
/// `source = a·j + b·j·r² + t` (linéaire en a, b, t), en écartant à chaque tour les
/// appariements trop loin du modèle ; `a` absorbe le léger recadrage du boîtier, d'où
/// `k = b / a³`.
pub fn radial_k(render: &RgbImage, jpeg: &RgbImage) -> Result<f32> {
    let r = crate::analysis(render, SIDE);
    let j = crate::analysis(jpeg, SIDE);
    let sol = fit(&r, &j)?;
    Ok((sol[1] / sol[0].powi(3)) as f32)
}

/// Le modèle `source = a·j + b·j·r² + t` (a, b, tx, ty) qui amène le JPEG `j` sur le
/// rendu `r` (déjà ramenés à la taille d'analyse), coordonnées normalisées par la
/// demi‑diagonale de chacun.
fn fit(r: &RgbImage, j: &RgbImage) -> Result<[f64; 4]> {
    let fr = features::detect(&features::luminance(r), 4000);
    let fj = features::detect(&features::luminance(j), 4000);
    let norm = |img: &RgbImage| {
        let (cx, cy) = (img.width as f64 / 2.0, img.height as f64 / 2.0);
        let hd = (cx * cx + cy * cy).sqrt();
        move |(x, y): (f64, f64)| ((x - cx) / hd, (y - cy) / hd)
    };
    let (nj, nr) = (norm(j), norm(r));
    let pts: Vec<Match> = features::matches(&fj, &fr)
        .into_iter()
        .map(|(a, b)| (nj(a), nr(b)))
        .collect();
    if pts.len() < 40 {
        return Err(Error::Decode(format!(
            "trop peu de détails communs avec le JPEG du boîtier ({})",
            pts.len()
        )));
    }
    let pixel = 2.0 / ((r.width * r.width + r.height * r.height) as f64).sqrt();
    let mut keep = vec![true; pts.len()];
    let mut sol = [1.0, 0.0, 0.0, 0.0];
    for _ in 0..6 {
        sol = solve(&pts, &keep).ok_or_else(|| Error::Decode("mesure de la distorsion impossible".into()))?;
        let residual = |((jx, jy), (sx, sy)): &Match| {
            let r2 = jx * jx + jy * jy;
            let dx = sol[0] * jx + sol[1] * jx * r2 + sol[2] - sx;
            let dy = sol[0] * jy + sol[1] * jy * r2 + sol[3] - sy;
            (dx * dx + dy * dy).sqrt()
        };
        let res: Vec<f64> = pts.iter().map(residual).collect();
        let mut kept: Vec<f64> = res.iter().zip(&keep).filter(|(_, k)| **k).map(|(r, _)| *r).collect();
        kept.sort_by(f64::total_cmp);
        let median = kept[kept.len() / 2];
        // Trois fois l'écart médian, au moins un pixel.
        for (k, r) in keep.iter_mut().zip(&res) {
            *k = *r < (3.0 * median).max(pixel);
        }
    }
    let kept = keep.iter().filter(|k| **k).count();
    if kept < 30 || !(0.5..2.0).contains(&sol[0]) {
        return Err(Error::Decode(format!(
            "le JPEG du boîtier et la photo ne se superposent pas ({kept} détails communs)"
        )));
    }
    Ok(sol)
}

/// Le réglage Vignetage (−100 à 100, Milieu à 50) qui rend `photo`, développée avec
/// `settings`, aussi claire vers les bords que le JPEG que le boîtier a rangé dans son
/// RAW `path` (le boîtier a corrigé le vignetage de son objectif) : le Vignetage de
/// `settings` plus ce qui manque. (Le Milieu reste fixe : ajusté avec, il s'échange
/// avec le Vignetage et la mesure saute d'un couple à l'autre.)
pub fn vignetting(
    photo: &luma_render::Photo,
    path: &std::path::Path,
    settings: &luma_project::Settings,
) -> Result<f32> {
    let pair = crate::film::camera_pair(photo, path)?;
    let render = if *settings == luma_project::Settings::default() {
        pair.digital
    } else {
        crate::film::reference_with(photo, settings)?
    };
    let start = settings.lens.vignetting;
    let first = vignetting_of(&render, &pair.film)?;
    // Le gain se mesure après la courbe de tons de LÜMA, le curseur agit avant : un
    // second rendu avec la valeur trouvée dit de combien elle porte vraiment, et la
    // droite entre les deux mesures donne celle qui ne laisse plus rien.
    let mut tried = settings.clone();
    tried.lens.vignetting = (start + first).clamp(-100.0, 100.0);
    tried.lens.vignetting_midpoint = MIDPOINT;
    let rest = vignetting_of(&crate::film::reference_with(photo, &tried)?, &pair.film)?;
    let reach = (first - rest) / first;
    let step = if first.abs() > 2.0 && (0.3..3.0).contains(&reach) {
        first / reach
    } else {
        first
    };
    Ok((start + step).clamp(-100.0, 100.0))
}

/// Le Milieu du vignetage mesuré.
pub const MIDPOINT: f32 = 50.0;

/// Le vignetage de `render` par rapport à `jpeg` : chaque pixel du JPEG ramené sur le
/// rendu par le modèle de géométrie ; la courbe de tons du boîtier apprise au centre
/// (où le vignetage ne joue pas : la luminance du JPEG selon celle du rendu), puis
/// défaite partout ; ce qu'il reste, le gain du JPEG sur le rendu selon le rayon, ajusté
/// sur le modèle de LÜMA (`gain = 2^(1,6 · v · smoothstep(milieu, 1,25, r))`, Milieu à
/// [`MIDPOINT`]), par moindres carrés.
pub fn vignetting_of(render: &RgbImage, jpeg: &RgbImage) -> Result<f32> {
    let r = crate::analysis(render, SIDE);
    let j = crate::analysis(jpeg, SIDE);
    let sol = fit(&r, &j)?;
    let half = |img: &RgbImage| {
        let (cx, cy) = (img.width as f64 / 2.0, img.height as f64 / 2.0);
        (cx, cy, (cx * cx + cy * cy).sqrt())
    };
    let ((jcx, jcy, jhd), (rcx, rcy, rhd)) = (half(&j), half(&r));
    let lum = |p: [f32; 3]| p.iter().map(|&v| luma_color::srgb_decode(v)).sum::<f32>() / 3.0;
    // Les paires (rayon dans le rendu, log2 luminance du rendu, log2 luminance du JPEG),
    // loin des contours (un décalage d'un pixel y changerait tout) et des extrêmes.
    let mut samples: Vec<(f64, f64, f64)> = Vec::new();
    for y in (2..j.height - 2).step_by(3) {
        for x in (2..j.width - 2).step_by(3) {
            let p = j.pixel(x, y);
            let gx = lum(j.pixel(x + 2, y)) - lum(j.pixel(x - 2, y));
            let gy = lum(j.pixel(x, y + 2)) - lum(j.pixel(x, y - 2));
            let lj = lum(p);
            if gx.abs() + gy.abs() > 0.08 * lj.max(0.02) || p.iter().any(|&v| v > 0.97) || lj < 0.01 {
                continue;
            }
            let (nx, ny) = ((x as f64 - jcx) / jhd, (y as f64 - jcy) / jhd);
            let r2 = nx * nx + ny * ny;
            let (sx, sy) = (
                sol[0] * nx + sol[1] * nx * r2 + sol[2],
                sol[0] * ny + sol[1] * ny * r2 + sol[3],
            );
            let Some(q) =
                crate::frames::sample_with(r.width, r.height, sx * rhd + rcx - 0.5, sy * rhd + rcy - 0.5, |x, y| {
                    r.pixel(x, y)
                })
            else {
                continue;
            };
            let lr = lum(q);
            if lr < 0.002 || q.iter().any(|&v| v > 0.97) {
                continue;
            }
            samples.push(((sx * sx + sy * sy).sqrt(), f64::from(lr).log2(), f64::from(lj).log2()));
        }
    }
    // La courbe de tons au centre : la médiane du JPEG par tranche de luminance du rendu,
    // rendue croissante.
    const BINS: usize = 60;
    let (lo, hi) = (-10.0f64, 0.0f64);
    let bin = |l: f64| (((l - lo) / (hi - lo) * BINS as f64).floor() as isize).clamp(0, BINS as isize - 1) as usize;
    let mut by_bin: Vec<Vec<f64>> = vec![Vec::new(); BINS];
    for &(rad, lr, lj) in &samples {
        if rad < 0.3 {
            by_bin[bin(lr)].push(lj);
        }
    }
    let mut curve: Vec<(f64, f64)> = Vec::new();
    for (k, v) in by_bin.iter_mut().enumerate() {
        if v.len() >= 8 {
            v.sort_by(f64::total_cmp);
            let x = lo + (k as f64 + 0.5) / BINS as f64 * (hi - lo);
            let y = v[v.len() / 2];
            let y = curve.last().map_or(y, |&(_, last): &(f64, f64)| y.max(last + 1e-4));
            curve.push((x, y));
        }
    }
    if curve.len() < 4 {
        return Err(Error::Decode(
            "trop peu de tons communs au centre du JPEG du boîtier".into(),
        ));
    }
    // Le ton du rendu qui donne `lj` au centre (sans prolonger la courbe).
    let undo = |lj: f64| -> Option<f64> {
        let i = curve.windows(2).position(|w| lj >= w[0].1 && lj <= w[1].1)?;
        let (a, b) = (curve[i], curve[i + 1]);
        Some(a.0 + (lj - a.1) / (b.1 - a.1) * (b.0 - a.0))
    };
    // Le gain (log2) selon le rayon : la médiane par anneau.
    const RINGS: usize = 40;
    let mut rings: Vec<Vec<f64>> = vec![Vec::new(); RINGS];
    for &(rad, lr, lj) in &samples {
        if let Some(equivalent) = undo(lj) {
            let k = ((rad / 1.0) * RINGS as f64) as usize;
            if k < RINGS {
                rings[k].push(equivalent - lr);
            }
        }
    }
    let points: Vec<(f64, f64)> = rings
        .iter_mut()
        .enumerate()
        .filter(|(_, v)| v.len() >= 20)
        .map(|(k, v)| {
            v.sort_by(f64::total_cmp);
            ((k as f64 + 0.5) / RINGS as f64, v[v.len() / 2])
        })
        .collect();
    if points.iter().filter(|(rad, _)| *rad > 0.6).count() < 3 {
        return Err(Error::Decode(
            "trop peu de pixels vers les bords du JPEG du boîtier".into(),
        ));
    }
    // Le gain du centre (l'exposition) retiré, puis le Vignetage par moindres carrés.
    let central: Vec<f64> = points.iter().filter(|(rad, _)| *rad < 0.3).map(|(_, g)| *g).collect();
    let center = central.iter().sum::<f64>() / central.len().max(1) as f64;
    let smoothstep = |e0: f64, e1: f64, x: f64| {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let mid = 0.15 + 0.6 * f64::from(MIDPOINT) / 100.0;
    let (mut sg, mut ss) = (0.0, 0.0);
    for &(rad, g) in &points {
        let sm = 1.6 * smoothstep(mid, 1.25, rad);
        sg += (g - center) * sm;
        ss += sm * sm;
    }
    Ok(((sg / ss).clamp(-1.0, 1.0) * 100.0) as f32)
}

/// Les moindres carrés de `source = a·j + b·j·r² + t` sur les appariements gardés :
/// (a, b, tx, ty), ou `None` si le système est dégénéré.
fn solve(pts: &[Match], keep: &[bool]) -> Option<[f64; 4]> {
    let mut m = [[0.0f64; 5]; 4];
    for (((jx, jy), (sx, sy)), _) in pts.iter().zip(keep).filter(|(_, k)| **k) {
        let r2 = jx * jx + jy * jy;
        for (row, value) in [([*jx, jx * r2, 1.0, 0.0], *sx), ([*jy, jy * r2, 0.0, 1.0], *sy)] {
            for i in 0..4 {
                for j in 0..4 {
                    m[i][j] += row[i] * row[j];
                }
                m[i][4] += row[i] * value;
            }
        }
    }
    // Gauss‑Jordan avec pivot partiel.
    for i in 0..4 {
        let p = (i..4).max_by(|&a, &b| m[a][i].abs().total_cmp(&m[b][i].abs()))?;
        m.swap(i, p);
        if m[i][i].abs() < 1e-12 {
            return None;
        }
        let pivot = m[i][i];
        for v in &mut m[i] {
            *v /= pivot;
        }
        for r in 0..4 {
            if r != i {
                let (f, pivot_row) = (m[r][i], m[i]);
                for (v, p) in m[r].iter_mut().zip(pivot_row) {
                    *v -= f * p;
                }
            }
        }
    }
    Some([m[0][4], m[1][4], m[2][4], m[3][4]])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une distorsion connue, appliquée par LÜMA à une image, est retrouvée.
    #[test]
    fn a_known_distortion_is_found_again() {
        // Une image riche en détails : des taches à des places sans motif.
        let (w, h) = (1200usize, 800usize);
        let mut seed = 0x1234_5678u32;
        let mut img = RgbImage::new(w, h);
        for _ in 0..900 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let (x0, y0) = ((seed % w as u32) as usize, ((seed >> 12) % h as u32) as usize);
            let v = ((seed >> 24) as f32 / 255.0).max(0.2);
            for y in y0.saturating_sub(3)..(y0 + 4).min(h) {
                for x in x0.saturating_sub(3)..(x0 + 4).min(w) {
                    img.data[y * w + x] = [v, v * 0.8, v * 0.6];
                }
            }
        }
        for amount in [-40.0f32, 25.0] {
            // `img` joue le RAW ; le « JPEG du boîtier », le même corrigé de `amount`.
            let lens = luma_project::Lens {
                distortion: amount,
                ..luma_project::Lens::default()
            };
            let jpeg = luma_image::lens::correct(&img, &lens, &luma_project::Transform::default(), (0.0, 0.0)).unwrap();
            let k = radial_k(&img, &jpeg).unwrap();
            let found = -k / 0.15 * 100.0;
            assert!((found - amount).abs() < 3.0, "{amount} : {found}");
        }
        // Rien de commun : refusé.
        let flat = RgbImage::new(w, h);
        assert!(radial_k(&flat, &img).is_err());
    }

    /// Un vignetage connu, posé par LÜMA sur une image vue par un « boîtier » à la courbe
    /// de tons bien à lui, est retrouvé.
    #[test]
    fn a_known_vignetting_is_found_again() {
        // Des plages douces de toutes les luminances, et quelques détails pour aligner.
        let (w, h) = (1200usize, 800usize);
        let mut seed = 0x0BAD_5EEDu32;
        let mut img = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let v = 0.08 + 0.8 * (((x / 150) * 7 + (y / 100) * 3) % 10) as f32 / 10.0;
                img.data[y * w + x] = [v, v * 0.9, v * 0.8];
            }
        }
        for _ in 0..700 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let (x0, y0) = ((seed % w as u32) as usize, ((seed >> 12) % h as u32) as usize);
            let v = ((seed >> 24) as f32 / 255.0).max(0.1);
            for y in y0.saturating_sub(2)..(y0 + 3).min(h) {
                for x in x0.saturating_sub(2)..(x0 + 3).min(w) {
                    img.data[y * w + x] = [v, v, v];
                }
            }
        }
        let encode = |im: &RgbImage| RgbImage {
            width: im.width,
            height: im.height,
            data: im.data.iter().map(|p| p.map(luma_color::srgb_encode)).collect(),
        };
        for amount in [40.0f32, -25.0] {
            let lens = luma_project::Lens {
                vignetting: amount,
                vignetting_midpoint: MIDPOINT,
                ..luma_project::Lens::default()
            };
            // Le JPEG : la photo corrigée de ce vignetage, puis une courbe de tons.
            let corrected =
                luma_image::lens::correct(&img, &lens, &luma_project::Transform::default(), (0.0, 0.0)).unwrap();
            let toned = RgbImage {
                width: w,
                height: h,
                data: corrected
                    .data
                    .iter()
                    .map(|p| p.map(|v| (v * 1.3).min(1.0).powf(0.8)))
                    .collect(),
            };
            let v = vignetting_of(&encode(&img), &encode(&toned)).unwrap();
            assert!((v - amount).abs() < 8.0, "{amount} : {v}");
        }
        let v = vignetting_of(&encode(&img), &encode(&img)).unwrap();
        assert!(v.abs() < 3.0, "rien à corriger : {v}");
    }

    /// Sur le corpus : le NEF (objectif non corrigé par LÜMA) a une distorsion nette, et
    /// une fois corrigée de la valeur mesurée, il ne lui en reste presque plus ; les RAW
    /// dont LÜMA applique déjà les corrections (DNG, ORF, RW2) en ont à peine.
    #[test]
    fn the_camera_jpeg_gives_the_lens_distortion() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
        let nef = dir.join("DSC_0750.NEF");
        if !nef.exists() {
            eprintln!("corpus absent (scripts/fetch-corpus.sh) : essai ignoré");
            return;
        }
        let photo = luma_render::Photo::open(&nef).unwrap();
        let d = distortion(&photo, &nef, &luma_project::Settings::default()).unwrap();
        assert!(d < -15.0, "{d}");
        let corrected = luma_project::Settings {
            lens: luma_project::Lens {
                distortion: d,
                ..luma_project::Lens::default()
            },
            ..luma_project::Settings::default()
        };
        let rest = distortion(&photo, &nef, &corrected).unwrap();
        assert!(rest.abs() < 4.0, "{d} puis {rest}");
        // Son vignetage : net, et une fois corrigé de la valeur mesurée (après un
        // second rendu qui dit de combien le curseur porte), il n'en reste presque plus.
        let v = vignetting(&photo, &nef, &corrected).unwrap();
        assert!(v > 15.0, "{v}");
        let mut both = corrected.clone();
        both.lens.vignetting = v;
        both.lens.vignetting_midpoint = MIDPOINT;
        let jpeg = crate::film::camera_pair(&photo, &nef).unwrap().film;
        let left = vignetting_of(&crate::film::reference_with(&photo, &both).unwrap(), &jpeg).unwrap();
        assert!(left.abs() < 6.0, "{v} puis {left}");
        for name in ["IMG_1361.DNG", "OM_System_OM-5_2_regular.ORF", "P1000951.RW2"] {
            let p = dir.join(name);
            let photo = luma_render::Photo::open(&p).unwrap();
            let d = distortion(&photo, &p, &luma_project::Settings::default()).unwrap();
            assert!(d.abs() < 4.0, "{name} : {d}");
        }
    }
}
