//! Mesurer une vraie pellicule : la même scène photographiée en numérique et sur film
//! (le scan du négatif inversé, ou de la diapositive). Les deux images sont alignées
//! (coins, RANSAC, homographie), puis chaque pixel apparié dit comment la pellicule
//! rend cette couleur. Le résultat est une LUT 3D : appliquée au rendu neutre de LÜMA,
//! elle donne le rendu de cette pellicule, sur n'importe quelle photo.
//!
//! L'ajustement est en deux temps :
//! - un modèle global (une matrice affine par couche, par moindres carrés), qui ne
//!   s'emballe pas loin des couleurs mesurées ;
//! - une correction locale sur la grille de la LUT, là où il y a des mesures, lissée et
//!   prolongée en douceur vers les couleurs voisines (moindres carrés régularisés).
//!
//! Les couleurs sont en sRGB encodé (0 à 1), l'espace des LUT créatives.

use luma_core::{Error, Result, RgbImage};
use luma_image::cube::Cube;
use rayon::prelude::*;

use crate::features;
use crate::frames::sample;
use crate::geometry::{Homography, Model, ransac};

/// Une paire mesurée : le rendu neutre numérique et le scan de la pellicule, en sRGB
/// encodé (0 à 1). `aligned` : les deux images sont déjà superposées (mêmes cadrages).
pub struct Pair {
    pub digital: RgbImage,
    pub film: RgbImage,
    pub aligned: bool,
}

/// Un scan (JPEG, PNG, TIFF 8 ou 16 bits), en sRGB encodé.
pub fn load_scan(path: &std::path::Path) -> Result<RgbImage> {
    let img = image::open(path)
        .map_err(|e| Error::Decode(format!("{} : {e}", path.display())))?
        .into_rgb32f();
    let (w, h) = (img.width() as usize, img.height() as usize);
    Ok(RgbImage {
        width: w,
        height: h,
        data: img
            .as_raw()
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| p.map(|v| v.clamp(0.0, 1.0)))
            .collect(),
    })
}

/// La référence numérique : un RAW développé avec les réglages neutres de LÜMA (ce sur
/// quoi la LUT s'appliquera), ou une image déjà développée.
pub fn load_reference(path: &std::path::Path) -> Result<RgbImage> {
    match luma_render::Photo::open(path) {
        Ok(photo) => reference(&photo),
        Err(_) => load_scan(path),
    }
}

/// Le rendu neutre d'une photo ouverte, en sRGB encodé : la référence numérique.
pub fn reference(photo: &luma_render::Photo) -> Result<RgbImage> {
    let r = photo.render(
        &luma_project::Settings::default(),
        luma_render::Size::Fit(1600),
        &luma_core::Cancel::new(),
    )?;
    let px = luma_image::encode8(&r.image, luma_color::OutputSpace::Srgb);
    Ok(RgbImage {
        width: r.image.width,
        height: r.image.height,
        data: px
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| p.map(|v| f32::from(v) / 255.0))
            .collect(),
    })
}

/// La paire du rendu du boîtier : le rendu neutre de `photo` et le JPEG que le boîtier a
/// rangé dans son RAW `path` (la même prise, ses couleurs à lui). Mesurée comme une
/// pellicule, elle donne une LUT qui rend le RAW comme le boîtier l'aurait fait.
pub fn camera_pair(photo: &luma_render::Photo, path: &std::path::Path) -> Result<Pair> {
    let jpeg = luma_raw::RawPhoto::embedded_preview(path, 1600)
        .ok_or_else(|| Error::Decode(format!("{} : pas de JPEG du boîtier", path.display())))?;
    let (w, h) = (jpeg.width() as usize, jpeg.height() as usize);
    Ok(Pair {
        digital: reference(photo)?,
        film: RgbImage {
            width: w,
            height: h,
            data: jpeg
                .as_raw()
                .as_chunks::<3>()
                .0
                .iter()
                .map(|p| p.map(|v| f32::from(v) / 255.0))
                .collect(),
        },
        // Le boîtier corrige parfois son objectif, ou recadre un peu : on aligne.
        aligned: false,
    })
}

/// Le résultat d'une mesure.
pub struct Measured {
    pub cube: Cube,
    /// Pixels comparés.
    pub samples: usize,
    /// Écart moyen (OKLab × 100, perceptuel) entre le numérique et la pellicule, avant
    /// et après la LUT.
    pub before: f32,
    pub after: f32,
}

/// Côté de l'image d'analyse : assez pour bien aligner, assez petit pour que le grain
/// et un léger décalage ne comptent pas.
const ANALYSIS: usize = 900;

/// Les couleurs appariées d'une paire : (numérique, pellicule).
fn samples(pair: &Pair) -> Result<Vec<([f32; 3], [f32; 3])>> {
    // Une image vide (fichier abîmé) : rien à comparer, et pas de division par zéro.
    if pair.digital.data.is_empty() || pair.film.data.is_empty() {
        return Err(Error::Decode("une des deux images est vide".into()));
    }
    let digital = crate::analysis(&pair.digital, ANALYSIS);
    let film = crate::analysis(&pair.film, ANALYSIS);
    let h = if pair.aligned {
        Homography::scaling(film.width as f64 / digital.width as f64)
    } else {
        let fd = features::detect(&features::luminance(&digital), 2000);
        let ff = features::detect(&features::luminance(&film), 2000);
        let pairs = features::matches(&fd, &ff);
        let (h, inliers) = ransac(&pairs, Model::Homography, 3.0, 2000)
            .ok_or_else(|| Error::Decode("le scan et la photo ne se superposent pas".into()))?;
        if inliers.len() < 16 {
            return Err(Error::Decode(format!(
                "le scan et la photo ne se superposent pas ({} points communs)",
                inliers.len()
            )));
        }
        h
    };
    // Un léger flou des deux côtés : le grain et un alignement à un pixel près ne
    // faussent pas les couleurs.
    let digital = blur(&digital);
    let film = blur(&film);
    let (w, hgt) = (digital.width, digital.height);
    let gray = |p: [f32; 3]| (p[0] + p[1] + p[2]) / 3.0;
    let out: Vec<([f32; 3], [f32; 3])> = (1..hgt - 1)
        .into_par_iter()
        .flat_map_iter(|y| {
            let (digital, film, h) = (&digital, &film, &h);
            (1..w - 1).filter_map(move |x| {
                let d = digital.data[y * w + x];
                // Les contours : un décalage d'un pixel y changerait tout.
                let gx = gray(digital.data[y * w + x + 1]) - gray(digital.data[y * w + x - 1]);
                let gy = gray(digital.data[(y + 1) * w + x]) - gray(digital.data[(y - 1) * w + x]);
                if gx.abs() + gy.abs() > 0.06 {
                    return None;
                }
                let (fx, fy) = h.apply((x as f64, y as f64))?;
                let f = sample(film, fx, fy)?;
                let ok = |p: [f32; 3]| p.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v));
                (ok(d) && ok(f)).then_some((d, f))
            })
        })
        .collect();
    if out.len() < 500 {
        return Err(Error::Decode(format!(
            "trop peu de pixels comparables ({}) : une scène plus grande, moins de détails fins",
            out.len()
        )));
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

/// Le modèle global : pour chaque couche de la pellicule, une combinaison affine des
/// trois couches numériques (moindres carrés, un peu de rappel vers l'identité).
fn global_model(data: &[([f32; 3], [f32; 3])]) -> [[f64; 4]; 3] {
    let mut ata = [[0.0f64; 4]; 4];
    let mut atb = [[0.0f64; 4]; 3];
    for (d, f) in data {
        let x = [f64::from(d[0]), f64::from(d[1]), f64::from(d[2]), 1.0];
        for i in 0..4 {
            for j in 0..4 {
                ata[i][j] += x[i] * x[j];
            }
            for c in 0..3 {
                atb[c][i] += x[i] * f64::from(f[c]);
            }
        }
    }
    let ridge = 1e-3 * data.len() as f64;
    std::array::from_fn(|c| {
        let mut a = ata;
        let mut b = atb[c];
        for i in 0..3 {
            a[i][i] += ridge;
            // Le rappel tire vers « la couche c ne dépend que d'elle‑même ».
            if i == c {
                b[i] += ridge;
            }
        }
        crate::geometry::solve(a, b).unwrap_or_else(|| {
            let mut id = [0.0; 4];
            id[c] = 1.0;
            id
        })
    })
}

fn affine(m: &[[f64; 4]; 3], p: [f32; 3]) -> [f32; 3] {
    let x = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2]), 1.0];
    std::array::from_fn(|c| (0..4).map(|i| m[c][i] * x[i]).sum::<f64>() as f32)
}

/// Mesure la pellicule sur une ou plusieurs paires ; LUT de `size`³ couleurs.
pub fn measure(pairs: &[Pair], size: usize, title: &str) -> Result<Measured> {
    let mut data = Vec::new();
    for pair in pairs {
        data.extend(samples(pair)?);
    }
    fit(data, size, title)
}

/// Comme [`measure`], mais une paire qui ne se compare pas (images qui ne se
/// superposent pas, trop peu de pixels) est laissée de côté au lieu d'arrêter la
/// mesure : le rendu d'un boîtier sur plusieurs de ses photos. Rend aussi le nombre de
/// paires gardées ; l'erreur de la première paire si aucune ne se compare.
pub fn measure_tolerant(pairs: &[Pair], size: usize, title: &str) -> Result<(Measured, usize)> {
    let mut data = Vec::new();
    let (mut kept, mut first_error) = (0, None);
    for pair in pairs {
        match samples(pair) {
            Ok(s) => {
                data.extend(s);
                kept += 1;
            }
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    match first_error {
        Some(e) if kept == 0 => Err(e),
        _ => Ok((fit(data, size, title)?, kept)),
    }
}

/// La LUT ajustée sur les couleurs appariées `data`.
fn fit(data: Vec<([f32; 3], [f32; 3])>, size: usize, title: &str) -> Result<Measured> {
    let n = size.clamp(9, 65);
    if data.is_empty() {
        return Err(Error::Decode("aucune paire à mesurer".into()));
    }
    let global = global_model(&data);

    // Les écarts au modèle global, répartis sur les huit nœuds voisins de chaque mesure.
    let nodes = n * n * n;
    let m = (n - 1) as f32;
    let mut acc = vec![[0.0f64; 3]; nodes];
    let mut weight = vec![0.0f64; nodes];
    for (d, f) in &data {
        let g = affine(&global, *d);
        let e = [f[0] - g[0], f[1] - g[1], f[2] - g[2]];
        let t = d.map(|v| v.clamp(0.0, 1.0) * m);
        let i = t.map(|v| (v as usize).min(n - 2));
        let fr: [f32; 3] = std::array::from_fn(|c| t[c] - i[c] as f32);
        for corner in 0..8 {
            let o = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
            let w: f32 = (0..3).map(|c| if o[c] == 1 { fr[c] } else { 1.0 - fr[c] }).product();
            if w <= 0.0 {
                continue;
            }
            let k = (i[0] + o[0]) + (i[1] + o[1]) * n + (i[2] + o[2]) * n * n;
            for c in 0..3 {
                acc[k][c] += f64::from(w * e[c]);
            }
            weight[k] += f64::from(w);
        }
    }

    // Moindres carrés régularisés sur la grille : coller aux mesures là où il y en a
    // (confiance selon leur nombre), varier en douceur d'un nœud à l'autre, et revenir
    // lentement au modèle global loin de toute mesure.
    let measured: Vec<[f64; 3]> = acc
        .iter()
        .zip(&weight)
        .map(|(a, &w)| if w > 0.0 { a.map(|v| v / w) } else { [0.0; 3] })
        .collect();
    let confidence: Vec<f64> = weight.iter().map(|&w| w / (w + 3.0)).collect();
    let (smooth, pull) = (0.12, 0.002);
    let mut field = measured.clone();
    let index = |r: usize, g: usize, b: usize| r + g * n + b * n * n;
    for _ in 0..400 {
        field = (0..nodes)
            .into_par_iter()
            .map(|k| {
                let (r, g, b) = (k % n, k / n % n, k / (n * n));
                let mut sum = [0.0; 3];
                let mut count = 0.0;
                let mut add = |j: usize| {
                    for c in 0..3 {
                        sum[c] += field[j][c];
                    }
                    count += 1.0;
                };
                if r > 0 {
                    add(index(r - 1, g, b));
                }
                if r + 1 < n {
                    add(index(r + 1, g, b));
                }
                if g > 0 {
                    add(index(r, g - 1, b));
                }
                if g + 1 < n {
                    add(index(r, g + 1, b));
                }
                if b > 0 {
                    add(index(r, g, b - 1));
                }
                if b + 1 < n {
                    add(index(r, g, b + 1));
                }
                let cf = confidence[k];
                std::array::from_fn(|c| (cf * measured[k][c] + smooth * sum[c]) / (cf + smooth * count + pull))
            })
            .collect();
    }

    let mut cube = Cube::identity(n);
    cube.title = title.to_string();
    for (k, p) in cube.data.iter_mut().enumerate() {
        let g = affine(&global, *p);
        *p = std::array::from_fn(|c| (g[c] + field[k][c] as f32).clamp(0.0, 1.0));
    }

    // L'écart avant et après, en OKLab (sur les couleurs encodées ramenées au linéaire).
    let ok = luma_color::OkLab::for_working();
    let to_working = luma_color::rgb_to_rgb(&luma_color::SRGB, &luma_color::WORKING);
    let lab = |p: [f32; 3]| ok.from_rgb(luma_color::apply(&to_working, p.map(luma_color::srgb_decode)));
    let de = |a: [f32; 3], b: [f32; 3]| {
        let (a, b) = (lab(a), lab(b));
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() * 100.0
    };
    let step = (data.len() / 20000).max(1);
    let picked: Vec<&([f32; 3], [f32; 3])> = data.iter().step_by(step).collect();
    let before = picked.iter().map(|(d, f)| de(*d, *f)).sum::<f32>() / picked.len() as f32;
    let after = picked.iter().map(|(d, f)| de(cube.sample(*d), *f)).sum::<f32>() / picked.len() as f32;
    Ok(Measured {
        cube,
        samples: data.len(),
        before,
        after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une pellicule imaginaire : courbe en S, ombres vertes, hautes lumières chaudes.
    fn film_look(p: [f32; 3]) -> [f32; 3] {
        let s = |v: f32| (v * v * (3.0 - 2.0 * v)) * 0.85 + v * 0.15;
        let l = (p[0] + p[1] + p[2]) / 3.0;
        [
            (s(p[0]) + 0.04 * l).clamp(0.0, 1.0),
            (s(p[1]) + 0.03 * (1.0 - l)).clamp(0.0, 1.0),
            (s(p[2]) * 0.9 - 0.02 * l + 0.02).clamp(0.0, 1.0),
        ]
    }

    /// Une scène : des aplats de couleurs au hasard (beaucoup de coins pour l'alignement,
    /// des couleurs partout dans le cube) et des dégradés.
    fn scene(w: usize, h: usize) -> RgbImage {
        let mut rng = crate::geometry::Rng::new(7);
        let cells: Vec<[f32; 3]> = (0..24 * 18)
            .map(|_| std::array::from_fn(|_| rng.below(1000) as f32 / 999.0))
            .collect();
        RgbImage::from_fn(w, h, |x, y| {
            let (cx, cy) = (x * 24 / w, y * 18 / h);
            let c = cells[cy * 24 + cx];
            let t = (x % (w / 24)) as f32 / (w / 24) as f32;
            c.map(|v| (v * (0.8 + 0.2 * t)).clamp(0.0, 1.0))
        })
    }

    #[test]
    fn a_scanned_film_is_measured_into_a_lut_that_reproduces_it() {
        let digital = scene(1200, 900);
        // Le scan : cadré un peu autrement (tourné, décalé, agrandi), avec le rendu du film
        // et un peu de grain.
        let h = Homography::similarity(1.08, 0.03, -30.0, 22.0);
        let inv = h.inverse().unwrap();
        let film = RgbImage::from_fn(1250, 940, |x, y| {
            let (sx, sy) = inv.apply((x as f64, y as f64)).unwrap();
            let p = sample(&digital, sx, sy).unwrap_or([0.5; 3]);
            let grain = (((x * 7 + y * 13) % 17) as f32 / 17.0 - 0.5) * 0.01;
            film_look(p).map(|v| (v + grain).clamp(0.0, 1.0))
        });
        let measured = measure(
            &[Pair {
                digital: digital.clone(),
                film,
                aligned: false,
            }],
            33,
            "Essai",
        )
        .unwrap();
        assert!(measured.samples > 50_000, "{}", measured.samples);
        assert!(measured.before > 3.0, "{}", measured.before);
        assert!(measured.after < 1.0, "écart après la LUT : {}", measured.after);
        // Sur des couleurs que la mire n'avait pas exactement, la LUT reste juste.
        let mut worst = 0.0f32;
        for k in 0..400 {
            let p = [
                (k % 7) as f32 / 6.0 * 0.9 + 0.05,
                (k / 7 % 7) as f32 / 6.0 * 0.9 + 0.05,
                (k / 49 % 7) as f32 / 6.0 * 0.9 + 0.05,
            ];
            let (a, b) = (measured.cube.sample(p), film_look(p));
            worst = worst.max((0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max));
        }
        eprintln!(
            "{} pixels, écart {:.2} → {:.2}, pire {worst:.3}",
            measured.samples, measured.before, measured.after
        );
        assert!(worst < 0.06, "écart maximal {worst}");
    }

    #[test]
    fn unrelated_images_are_refused() {
        let a = scene(800, 600);
        let b = RgbImage::from_fn(800, 600, |x, y| [((x * y) % 255) as f32 / 255.0, 0.3, 0.6]);
        let r = measure(
            &[Pair {
                digital: a,
                film: b,
                aligned: false,
            }],
            17,
            "",
        );
        assert!(r.is_err());
    }

    /// Le rendu du boîtier, sur un RAW de chaque marque du corpus : la LUT mesurée sur
    /// son propre JPEG le reproduit (écart divisé par plus de cinq, sous 2) ; une
    /// image sans JPEG du boîtier est refusée.
    #[test]
    fn the_camera_look_is_measured_from_its_own_jpeg() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
        let raws: Vec<_> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| luma_raw::is_raw(p))
            .collect();
        if raws.is_empty() {
            eprintln!("corpus absent (scripts/fetch-corpus.sh) : essai ignoré");
            return;
        }
        for path in &raws {
            let photo = luma_render::Photo::open(path).unwrap();
            let m = measure(&[camera_pair(&photo, path).unwrap()], 33, "boîtier").unwrap();
            assert!(
                m.after < 2.0 && m.after * 5.0 < m.before,
                "{} : {} -> {}",
                path.display(),
                m.before,
                m.after
            );
        }
        // Toutes ensemble, avec une paire qui ne se superpose pas (le JPEG d'une autre
        // photo) : laissée de côté, les autres mesurées.
        let photos: Vec<_> = raws.iter().map(|p| luma_render::Photo::open(p).unwrap()).collect();
        let mut pairs: Vec<Pair> = photos
            .iter()
            .zip(&raws)
            .map(|(ph, p)| camera_pair(ph, p).unwrap())
            .collect();
        let stray = camera_pair(&photos[0], &raws[1]).unwrap();
        pairs.push(stray);
        let (m, kept) = measure_tolerant(&pairs, 17, "tous").unwrap();
        assert_eq!(kept, raws.len());
        assert!(m.after < m.before, "{} -> {}", m.before, m.after);
        assert!(
            measure(&pairs, 17, "tous").is_err(),
            "la mesure stricte refuse la paire"
        );
        assert!(measure_tolerant(&pairs[raws.len()..], 17, "seule").is_err());
        let png = std::env::temp_dir().join(format!("luma-sans-jpeg-{}.png", std::process::id()));
        image::RgbImage::from_pixel(64, 48, image::Rgb([90, 120, 150]))
            .save(&png)
            .unwrap();
        let photo = luma_render::Photo::open(&png).unwrap();
        assert!(camera_pair(&photo, &png).is_err());
        let _ = std::fs::remove_file(&png);
    }
}
