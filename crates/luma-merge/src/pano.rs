//! Les panoramas : des photos qui se chevauchent, assemblées en une seule.
//!
//! 1. Les coins de chaque photo sont appariés avec ceux des autres ; une homographie
//!    (RANSAC) relie chaque paire qui se recouvre.
//! 2. Les paires les plus solides forment un arbre ; la photo la plus centrale sert de
//!    référence, les autres s'y rattachent de proche en proche.
//! 3. La focale est estimée à partir des homographies : on peut alors projeter sur un
//!    cylindre ou une sphère (panoramas larges) ou rester en perspective (droites
//!    droites, pour l'architecture).
//! 4. Les écarts d'exposition entre photos sont compensés, puis les photos sont fondues
//!    l'une dans l'autre (poids décroissant vers leurs bords).

use std::collections::VecDeque;
use std::sync::Arc;

use luma_core::{Cancel, Error, Result, RgbImage};
use rayon::prelude::*;

use crate::frames::{Compact, Frames, sample};
use crate::geometry::{Homography, Model, Point, ransac};
use crate::{Merged, Progress, analysis, features};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    /// Les droites restent droites ; pour moins de 120° environ.
    Perspective,
    /// Les verticales restent droites ; le choix des panoramas horizontaux.
    Cylindrical,
    /// Pour les panoramas très larges et sur plusieurs rangées.
    Spherical,
}

impl Projection {
    pub fn name(self) -> &'static str {
        match self {
            Projection::Perspective => "Perspective",
            Projection::Cylindrical => "Cylindrique",
            Projection::Spherical => "Sphérique",
        }
    }
}

/// La focale (en pixels) d'après une homographie entre deux photos qui ne diffèrent
/// que par une rotation (formules de Szeliski), coordonnées centrées.
fn focal_from(h: &Homography) -> Option<f64> {
    let h = [
        h.0[0][0], h.0[0][1], h.0[0][2], h.0[1][0], h.0[1][1], h.0[1][2], h.0[2][0], h.0[2][1], h.0[2][2],
    ];
    let pick = |d1: f64, d2: f64, v1: f64, v2: f64| -> Option<f64> {
        let (v1, v2) = if v1 < v2 { (v2, v1) } else { (v1, v2) };
        if v1 > 0.0 && v2 > 0.0 {
            Some(if d1.abs() > d2.abs() { v1 } else { v2 }.sqrt())
        } else if v1 > 0.0 {
            Some(v1.sqrt())
        } else {
            None
        }
    };
    let (d1, d2) = (h[6] * h[7], (h[7] - h[6]) * (h[7] + h[6]));
    let f1 = pick(
        d1,
        d2,
        -(h[0] * h[1] + h[3] * h[4]) / d1,
        (h[0] * h[0] + h[3] * h[3] - h[1] * h[1] - h[4] * h[4]) / d2,
    );
    let (d1, d2) = (
        h[0] * h[3] + h[1] * h[4],
        h[0] * h[0] + h[1] * h[1] - h[3] * h[3] - h[4] * h[4],
    );
    let f0 = pick(d1, d2, -h[2] * h[5] / d1, (h[5] * h[5] - h[2] * h[2]) / d2);
    match (f0, f1) {
        (Some(a), Some(b)) if a.is_finite() && b.is_finite() => Some((a * b).sqrt()),
        _ => None,
    }
}

/// Une liaison entre deux photos : l'homographie de `a` vers `b` et les paires de points.
struct Link {
    a: usize,
    b: usize,
    h: Homography,
    pairs: Vec<(Point, Point)>,
}

/// Assemble les photos en panorama.
pub fn panorama(frames: &dyn Frames, projection: Projection, progress: &Progress, cancel: &Cancel) -> Result<Merged> {
    let n = frames.len();
    if n < 2 {
        return Err(Error::Decode("il faut au moins deux photos".into()));
    }
    // Les photos, gardées en 16 bits, et leurs versions d'analyse.
    let mut images = Vec::with_capacity(n);
    let mut small = Vec::with_capacity(n);
    for i in 0..n {
        cancel.check()?;
        progress(0.25 * i as f32 / n as f32, "Lecture des photos");
        let img = frames.load(i)?;
        small.push(analysis(&img, 1600));
        images.push(Compact::new(&img));
    }
    let (w, h) = (images[0].width, images[0].height);
    if images.iter().any(|im| (im.width, im.height) != (w, h)) {
        return Err(Error::Decode(
            "les photos d'un panorama doivent avoir la même taille (même boîtier, même orientation)".into(),
        ));
    }
    let s = small[0].width as f64 / w as f64;

    // 1. Liaisons entre toutes les paires de photos.
    progress(0.3, "Recherche des recouvrements");
    let feats: Vec<Vec<features::Feature>> = small
        .par_iter()
        .map(|img| features::detect(&features::luminance(img), 2000))
        .collect();
    let candidates: Vec<(usize, usize)> = (0..n).flat_map(|a| (a + 1..n).map(move |b| (a, b))).collect();
    let links: Vec<Link> = candidates
        .par_iter()
        .filter_map(|&(a, b)| {
            let pairs = features::matches(&feats[a], &feats[b]);
            let (h, inliers) = ransac(&pairs, Model::Homography, 3.0, 1500)?;
            (inliers.len() >= 20 && inliers.len() * 5 >= pairs.len()).then(|| Link {
                a,
                b,
                h,
                pairs: inliers.iter().map(|&i| pairs[i]).collect(),
            })
        })
        .collect();
    cancel.check()?;

    // 2. Arbre couvrant de poids maximal (Prim), puis la photo la plus centrale.
    let mut in_tree = vec![false; n];
    let mut tree: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n]; // (voisine, liaison)
    in_tree[0] = true;
    loop {
        let best = links
            .iter()
            .enumerate()
            .filter(|(_, l)| in_tree[l.a] != in_tree[l.b])
            .max_by_key(|(_, l)| l.pairs.len());
        let Some((k, l)) = best else { break };
        tree[l.a].push((l.b, k));
        tree[l.b].push((l.a, k));
        in_tree[l.a] = true;
        in_tree[l.b] = true;
    }
    let missing: Vec<String> = (0..n).filter(|&i| !in_tree[i]).map(|i| (i + 1).to_string()).collect();
    if !missing.is_empty() {
        return Err(Error::Decode(format!(
            "photo(s) {} sans recouvrement suffisant avec les autres (il faut environ 30 % de recouvrement)",
            missing.join(", ")
        )));
    }
    let distances = |from: usize| -> Vec<usize> {
        let mut d = vec![usize::MAX; n];
        d[from] = 0;
        let mut q = VecDeque::from([from]);
        while let Some(u) = q.pop_front() {
            for &(v, _) in &tree[u] {
                if d[v] == usize::MAX {
                    d[v] = d[u] + 1;
                    q.push_back(v);
                }
            }
        }
        d
    };
    let reference = (0..n)
        .min_by_key(|&i| distances(i).into_iter().max().unwrap_or(0))
        .unwrap_or(0);

    // 3. Homographies référence → photo (en pleine taille), par parcours de l'arbre.
    let a = Homography([[s, 0.0, (s - 1.0) / 2.0], [0.0, s, (s - 1.0) / 2.0], [0.0, 0.0, 1.0]]);
    let a_inv = a.inverse().expect("échelle");
    let full = |h: &Homography| a.then(h).then(&a_inv);
    let mut to_image: Vec<Option<Homography>> = vec![None; n];
    to_image[reference] = Some(Homography::IDENTITY);
    let mut gains = vec![1.0f32; n];
    let mut q = VecDeque::from([reference]);
    while let Some(u) = q.pop_front() {
        for &(v, k) in &tree[u] {
            if to_image[v].is_some() {
                continue;
            }
            let l = &links[k];
            // h_uv : coordonnées de u → coordonnées de v.
            let (h_uv, pairs_uv): (Homography, Vec<(Point, Point)>) = if l.a == u {
                (l.h, l.pairs.clone())
            } else {
                (
                    l.h.inverse()
                        .ok_or_else(|| Error::Decode("homographie dégénérée".into()))?,
                    l.pairs.iter().map(|&(p, q)| (q, p)).collect(),
                )
            };
            to_image[v] = Some(to_image[u].expect("parcours").then(&full(&h_uv)));
            // Compensation d'exposition : rapport médian des luminances appariées.
            let lum = |img: &RgbImage, p: Point| -> Option<f32> {
                let mut sum = 0.0;
                for dy in -2..=2 {
                    for dx in -2..=2 {
                        let c = sample(img, p.0 + f64::from(dx), p.1 + f64::from(dy))?;
                        sum += c[0] + 2.0 * c[1] + c[2];
                    }
                }
                Some(sum)
            };
            let mut ratios: Vec<f32> = pairs_uv
                .iter()
                .filter_map(|&(pu, pv)| {
                    let (lu, lv) = (lum(&small[u], pu)?, lum(&small[v], pv)?);
                    (lu > 0.02 && lv > 0.02 && lu < 90.0 && lv < 90.0).then(|| (lu / lv).ln())
                })
                .collect();
            if ratios.len() >= 5 {
                let mid = ratios.len() / 2;
                let r = *ratios.select_nth_unstable_by(mid, f32::total_cmp).1;
                gains[v] = gains[u] * r.clamp(-2.0, 2.0).exp();
            } else {
                gains[v] = gains[u];
            }
            q.push_back(v);
        }
    }
    let to_image: Vec<Homography> = to_image.into_iter().map(|h| h.expect("arbre complet")).collect();
    let from_image: Vec<Homography> = to_image
        .iter()
        .map(|h| h.inverse().ok_or_else(|| Error::Decode("homographie dégénérée".into())))
        .collect::<Result<_>>()?;

    // 4. Focale (coordonnées centrées), médiane des estimations.
    let (cx, cy) = ((w as f64 - 1.0) / 2.0, (h as f64 - 1.0) / 2.0);
    let center = Homography::translation(cx, cy);
    let uncenter = Homography::translation(-cx, -cy);
    let mut focals: Vec<f64> = to_image
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != reference)
        .filter_map(|(_, hm)| focal_from(&center.then(hm).then(&uncenter)))
        .filter(|f| *f > 0.2 * w.max(h) as f64 && *f < 20.0 * w.max(h) as f64)
        .collect();
    focals.sort_by(f64::total_cmp);
    let focal = focals.get(focals.len() / 2).copied().unwrap_or(1.2 * w.max(h) as f64);

    // 5. La correspondance sortie → photo, selon la projection.
    // Coordonnées « monde » : pour la perspective, le plan de la référence (pixels) ;
    // sinon des angles (radians) multipliés par la focale. Un rayon est exprimé dans
    // la caméra de la référence (x à droite, y en bas, z devant).
    let world_of_ray = |x: f64, y: f64, z: f64| -> Point {
        let theta = x.atan2(z);
        let r = (x * x + z * z).sqrt().max(1e-12);
        let v = if projection == Projection::Cylindrical {
            y / r
        } else {
            y.atan2(r)
        };
        (theta * focal, v * focal)
    };
    // Le rayon de la référence (homogène) qui correspond à un point du monde.
    let ref_of_world = |p: Point| -> [f64; 3] {
        match projection {
            Projection::Perspective => [p.0, p.1, 1.0],
            Projection::Cylindrical | Projection::Spherical => {
                let theta = p.0 / focal;
                let v = p.1 / focal;
                let (x, y, z) = if projection == Projection::Cylindrical {
                    (theta.sin(), v, theta.cos())
                } else {
                    (v.cos() * theta.sin(), v.sin(), v.cos() * theta.cos())
                };
                [focal * x + cx * z, focal * y + cy * z, z]
            }
        }
    };
    // Les bords de chaque photo dans le monde : l'étendue du panorama.
    let mut bounds = Vec::with_capacity(n);
    for fi in &from_image {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for k in 0..=40 {
            let t = k as f64 / 40.0;
            for p in [
                (t * (w - 1) as f64, 0.0),
                (t * (w - 1) as f64, (h - 1) as f64),
                (0.0, t * (h - 1) as f64),
                ((w - 1) as f64, t * (h - 1) as f64),
            ] {
                let v = fi.apply3([p.0, p.1, 1.0]);
                let q = match projection {
                    Projection::Perspective => {
                        if v[2] <= 1e-9 {
                            continue;
                        }
                        (v[0] / v[2], v[1] / v[2])
                    }
                    _ => world_of_ray((v[0] - cx * v[2]) / focal, (v[1] - cy * v[2]) / focal, v[2]),
                };
                b = (b.0.min(q.0), b.1.min(q.1), b.2.max(q.0), b.3.max(q.1));
            }
        }
        bounds.push(b);
    }
    let all = bounds.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, b| {
        (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
    });
    if ![all.0, all.1, all.2, all.3].iter().all(|v| v.is_finite()) {
        return Err(Error::Decode(
            "les photos n'ont pas pu être placées sur le panorama".into(),
        ));
    }
    let (mut ow, mut oh) = ((all.2 - all.0).ceil() as usize + 1, (all.3 - all.1).ceil() as usize + 1);
    // Avant toute réduction : réduit, un panorama démesuré passerait sous la limite.
    if projection == Projection::Perspective && (ow > 8 * w || oh > 8 * h) {
        return Err(Error::Decode(
            "trop large pour la perspective : choisissez la projection cylindrique ou sphérique".into(),
        ));
    }
    // Au plus 150 Mpx (et 40 000 px de côté) : au‑delà, le panorama est réduit.
    let max_px = 150_000_000f64;
    let mut scale = 1.0f64;
    if (ow as f64 * oh as f64) > max_px || ow > 40_000 || oh > 40_000 {
        scale = (max_px / (ow as f64 * oh as f64))
            .sqrt()
            .min(40_000.0 / ow as f64)
            .min(40_000.0 / oh as f64);
        // Un côté démesuré (une photo qui vise le zénith) ne réduit pas l'autre à rien.
        ow = ((ow as f64 * scale) as usize).max(1);
        oh = ((oh as f64 * scale) as usize).max(1);
    }
    let world = |x: usize, y: usize| -> Point { (all.0 + x as f64 / scale, all.1 + y as f64 / scale) };

    // 6. Fondu : chaque photo pèse d'autant plus qu'on est loin de ses bords.
    progress(0.5, "Assemblage");
    let mut data = vec![[0.0f32; 3]; ow * oh];
    let mut covered = vec![false; ow * oh];
    let rows_done = std::sync::atomic::AtomicUsize::new(0);
    let (hw, hh) = (w as f64 / 2.0, h as f64 / 2.0);
    data.par_chunks_mut(ow)
        .zip(covered.par_chunks_mut(ow))
        .enumerate()
        .for_each(|(y, (row, cov))| {
            if cancel.is_cancelled() {
                return;
            }
            let wy = world(0, y).1;
            let active: Vec<usize> = (0..n)
                .filter(|&i| wy >= bounds[i].1 - 1.0 && wy <= bounds[i].3 + 1.0)
                .collect();
            for x in 0..ow {
                let wp = world(x, y);
                let ray = ref_of_world(wp);
                let mut acc = [0.0f32; 3];
                let mut weight = 0.0f32;
                for &i in &active {
                    if wp.0 < bounds[i].0 - 1.0 || wp.0 > bounds[i].2 + 1.0 {
                        continue;
                    }
                    let v = to_image[i].apply3(ray);
                    if v[2] <= 1e-9 {
                        continue;
                    }
                    let (u, vv) = (v[0] / v[2], v[1] / v[2]);
                    let Some(c) = images[i].sample(u, vv) else { continue };
                    let wx = ((u + 0.5).min(w as f64 - 0.5 - u) / hw).clamp(0.0, 1.0);
                    let wyy = ((vv + 0.5).min(h as f64 - 0.5 - vv) / hh).clamp(0.0, 1.0);
                    let k = (wx * wyy) as f32 + 1e-6;
                    let g = gains[i];
                    for ch in 0..3 {
                        acc[ch] += k * c[ch] * g;
                    }
                    weight += k;
                }
                if weight > 0.0 {
                    row[x] = acc.map(|v| v / weight);
                    cov[x] = true;
                }
            }
            let done = rows_done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if done.is_multiple_of(64) {
                progress(0.5 + 0.5 * done as f32 / oh as f32, "Assemblage");
            }
        });
    cancel.check()?;
    // Les gains peuvent dépasser le blanc : la plus claire des photos garde sa
    // dynamique, on ramène l'ensemble sous 1 si besoin.
    let peak = gains.iter().copied().fold(0.0f32, f32::max);
    if peak > 1.0 {
        data.par_iter_mut().for_each(|p| *p = p.map(|v| v / peak));
    }
    progress(1.0, "Terminé");
    let image = RgbImage {
        width: ow,
        height: oh,
        data,
    };
    Ok(Merged::new(image, covered, n, Vec::new()))
}

/// Pratique pour les tests : des images en mémoire.
pub fn panorama_images(images: Vec<Arc<RgbImage>>, projection: Projection) -> Result<Merged> {
    panorama(&images, projection, &|_, _| {}, &Cancel::new())
}
