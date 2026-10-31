//! La distorsion de l'objectif, mesurée sur le JPEG que le boîtier a rangé dans le RAW :
//! le boîtier l'a déjà corrigée, pas le RAW. Les mêmes détails trouvés des deux côtés
//! donnent, par moindres carrés, le modèle du curseur Distorsion de LÜMA
//! (`source = sortie × (1 + k r²)`, `k = −0,15 × curseur / 100`) : de quoi corriger les
//! objectifs des marques dont LÜMA ne lit pas les corrections (Canon, Nikon, Pentax…).

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
    let fr = features::detect(&features::luminance(&r), 4000);
    let fj = features::detect(&features::luminance(&j), 4000);
    let norm = |img: &RgbImage| {
        let (cx, cy) = (img.width as f64 / 2.0, img.height as f64 / 2.0);
        let hd = (cx * cx + cy * cy).sqrt();
        move |(x, y): (f64, f64)| ((x - cx) / hd, (y - cy) / hd)
    };
    let (nj, nr) = (norm(&j), norm(&r));
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
    Ok((sol[1] / sol[0].powi(3)) as f32)
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
        for name in ["IMG_1361.DNG", "OM_System_OM-5_2_regular.ORF", "P1000951.RW2"] {
            let p = dir.join(name);
            let photo = luma_render::Photo::open(&p).unwrap();
            let d = distortion(&photo, &p, &luma_project::Settings::default()).unwrap();
            assert!(d.abs() < 4.0, "{name} : {d}");
        }
    }
}
