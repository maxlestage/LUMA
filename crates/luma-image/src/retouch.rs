//! Tampon et correcteur ([`luma_project::Spot`]), appliqués juste après l'entrée, sur
//! l'image de travail linéaire : les réglages qui suivent (bruit, tonalité, masques)
//! voient la photo réparée.
//!
//! Le correcteur résout une équation de Laplace sur le disque : la source, plus une
//! membrane lisse qui la raccorde au bord de la zone. La membrane est interpolée depuis
//! 64 points du bord par les coordonnées de valeur moyenne (une solution approchée,
//! exacte pour une membrane affine, sans itérations) ; le résultat ne dépend pas de la
//! taille du rendu.

use luma_core::RgbImage;
use luma_project::{Spot, SpotKind};
use rayon::prelude::*;

/// Le nombre de points du bord pour la membrane du correcteur.
const RING: usize = 64;

/// Applique les retouches dans l'ordre : chacune voit les précédentes.
pub fn apply(img: &mut RgbImage, spots: &[Spot]) {
    for spot in spots {
        apply_one(img, spot);
    }
}

/// Le disque d'une retouche, en pixels : centre, rayon, décalage vers la source.
struct Disc {
    cx: f32,
    cy: f32,
    r: f32,
    dx: f32,
    dy: f32,
}

fn disc(spot: &Spot, w: usize, h: usize) -> Disc {
    let (w, h) = (w as f32, h as f32);
    Disc {
        cx: spot.center.0 * w,
        cy: spot.center.1 * h,
        r: (spot.radius * w).max(1.0),
        // Un décalage entier : la source n'est pas rééchantillonnée (pas de flou).
        dx: ((spot.source.0 - spot.center.0) * w).round(),
        dy: ((spot.source.1 - spot.center.1) * h).round(),
    }
}

/// La moyenne 3 × 3 autour de (x, y), bords prolongés.
fn average(img: &RgbImage, x: f32, y: f32) -> [f32; 3] {
    let (w, h) = (img.width as isize, img.height as isize);
    let (x, y) = (x.floor() as isize, y.floor() as isize);
    let mut sum = [0.0f32; 3];
    for j in -1..=1 {
        for i in -1..=1 {
            let px = (x + i).clamp(0, w - 1) as usize;
            let py = (y + j).clamp(0, h - 1) as usize;
            let p = img.data[py * img.width + px];
            for c in 0..3 {
                sum[c] += p[c];
            }
        }
    }
    sum.map(|v| v / 9.0)
}

fn apply_one(img: &mut RgbImage, spot: &Spot) {
    let (w, h) = (img.width, img.height);
    let opacity = (spot.opacity / 100.0).clamp(0.0, 1.0);
    if w == 0 || h == 0 || opacity <= 0.0 {
        return;
    }
    let d = disc(spot, w, h);
    let inner = d.r * (1.0 - (spot.feather / 100.0).clamp(0.0, 1.0));
    let x0 = (d.cx - d.r).floor().max(0.0) as usize;
    let y0 = (d.cy - d.r).floor().max(0.0) as usize;
    let x1 = ((d.cx + d.r).ceil() as usize + 1).min(w);
    let y1 = ((d.cy + d.r).ceil() as usize + 1).min(h);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    // Le bord : un polygone circonscrit au disque (le disque est tout entier dedans),
    // et l'écart entre la zone et la source sur chacun de ses sommets.
    let heal = spot.kind == SpotKind::Heal;
    let outer = d.r / (std::f32::consts::PI / RING as f32).cos();
    let ring: Vec<((f32, f32), [f32; 3])> = if heal {
        (0..RING)
            .map(|i| {
                let a = i as f32 / RING as f32 * std::f32::consts::TAU;
                let (s, c) = a.sin_cos();
                let (x, y) = (d.cx + outer * c, d.cy + outer * s);
                let target = average(img, x, y);
                let source = average(img, x + d.dx, y + d.dy);
                ((x, y), [0, 1, 2].map(|k| target[k] - source[k]))
            })
            .collect()
    } else {
        Vec::new()
    };
    let red_eye = spot.kind == SpotKind::RedEye;
    // La source lue avant d'écrire : elle peut chevaucher la zone. Les yeux rouges
    // partent du pixel lui‑même.
    let read = |x: usize, y: usize| {
        if red_eye {
            return pupil(img.data[y * w + x]);
        }
        let sx = (x as f32 + d.dx).clamp(0.0, (w - 1) as f32) as usize;
        let sy = (y as f32 + d.dy).clamp(0.0, (h - 1) as f32) as usize;
        img.data[sy * w + sx]
    };
    let bw = x1 - x0;
    let patch: Vec<[f32; 3]> = (y0..y1)
        .into_par_iter()
        .flat_map_iter(|y| {
            let ring = &ring;
            let d = &d;
            (x0..x1).map(move |x| {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let dist = ((px - d.cx).powi(2) + (py - d.cy).powi(2)).sqrt();
                if dist >= d.r {
                    return [f32::NAN; 3];
                }
                let mut v = read(x, y);
                if !ring.is_empty() {
                    let m = membrane(ring, px, py);
                    v = [0, 1, 2].map(|k| (v[k] + m[k]).max(0.0));
                }
                v
            })
        })
        .collect();
    img.data[y0 * w..y1 * w]
        .par_chunks_mut(w)
        .enumerate()
        .for_each(|(j, row)| {
            let py = (y0 + j) as f32 + 0.5;
            for x in x0..x1 {
                let v = patch[j * bw + x - x0];
                if v[0].is_nan() {
                    continue;
                }
                let px = x as f32 + 0.5;
                let dist = ((px - d.cx).powi(2) + (py - d.cy).powi(2)).sqrt();
                let t = if dist <= inner {
                    1.0
                } else {
                    let u = ((d.r - dist) / (d.r - inner)).clamp(0.0, 1.0);
                    u * u * (3.0 - 2.0 * u)
                };
                let a = t * opacity;
                let p = &mut row[x];
                for k in 0..3 {
                    p[k] += (v[k] - p[k]) * a;
                }
            }
        });
}

/// Une source pour une retouche en `center` de rayon `radius` (en fractions, comme
/// [`Spot`]) : parmi 16 directions et deux distances, le disque entièrement dans l'image
/// dont le bord ressemble le plus à celui de la zone (écarts au carré, moyennes ôtées :
/// le correcteur reprend la lumière du bord). Lu sur une copie réduite de la photo.
pub fn find_source(img: &RgbImage, center: (f32, f32), radius: f32) -> Option<(f32, f32)> {
    let (w, h) = (img.width as f32, img.height as f32);
    if img.width == 0 || img.height == 0 {
        return None;
    }
    let (cx, cy, r) = (center.0 * w, center.1 * h, (radius * w).max(1.0));
    let ring = |x: f32, y: f32| -> Vec<[f32; 3]> {
        (0..RING / 2)
            .map(|i| {
                let a = i as f32 / (RING / 2) as f32 * std::f32::consts::TAU;
                let (s, c) = a.sin_cos();
                average(img, x + r * c, y + r * s)
            })
            .collect()
    };
    let centered = |v: Vec<[f32; 3]>| {
        let n = v.len() as f32;
        let mean = v
            .iter()
            .fold([0.0f32; 3], |m, p| [m[0] + p[0] / n, m[1] + p[1] / n, m[2] + p[2] / n]);
        (
            v.into_iter()
                .map(|p| [p[0] - mean[0], p[1] - mean[1], p[2] - mean[2]])
                .collect::<Vec<_>>(),
            mean,
        )
    };
    let (target, mean) = centered(ring(cx, cy));
    let mut best: Option<(f32, (f32, f32))> = None;
    for distance in [2.5, 4.0] {
        for i in 0..16 {
            let a = i as f32 / 16.0 * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            let (x, y) = ((cx + distance * r * c).round(), (cy + distance * r * s).round());
            if x - r < 0.0 || y - r < 0.0 || x + r > w || y + r > h {
                continue;
            }
            let (other, m) = centered(ring(x, y));
            let shape: f32 = target
                .iter()
                .zip(&other)
                .map(|(p, q)| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>())
                .sum();
            // Une lumière proche départage, sans primer sur la texture.
            let light: f32 = (0..3).map(|k| (mean[k] - m[k]).powi(2)).sum::<f32>() * 0.1 * target.len() as f32;
            let score = shape + light + 1e-6 * distance;
            if best.is_none_or(|(b, _)| score < b) {
                best = Some((score, (x / w, y / h)));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Un pixel d'yeux rouges : d'autant plus ramené vers un sombre neutre que son rouge
/// domine franchement le vert et le bleu. En linéaire, une peau a un rouge d'environ
/// deux fois la moyenne du vert et du bleu, une pupille rouge de cinq à dix fois : seule
/// celle‑ci change. Le reflet blanc de l'œil, sans rouge dominant, reste.
pub fn pupil(p: [f32; 3]) -> [f32; 3] {
    let gb = 0.5 * (p[1] + p[2]);
    let ratio = p[0] / gb.max(1e-6);
    let smooth = |e0: f32, e1: f32, x: f32| {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    // Un rouge assez fort pour être vu (pas le bruit d'une ombre).
    let red = smooth(3.0, 5.0, ratio) * smooth(0.004, 0.02, p[0]);
    // La pupille : le rouge devient la moyenne du vert et du bleu, le tout un peu
    // assombri.
    let dark = [gb * 0.8, p[1] * 0.8, p[2] * 0.8];
    std::array::from_fn(|k| p[k] + (dark[k] - p[k]) * red)
}

/// La membrane en (x, y) : les écarts du bord interpolés par les coordonnées de valeur
/// moyenne du polygone `ring` (convexe, le point est dedans).
fn membrane(ring: &[((f32, f32), [f32; 3])], x: f32, y: f32) -> [f32; 3] {
    let n = ring.len();
    let s: Vec<(f32, f32, f32)> = ring
        .iter()
        .map(|((px, py), _)| {
            let (sx, sy) = (px - x, py - y);
            (sx, sy, (sx * sx + sy * sy).sqrt())
        })
        .collect();
    // tan(α/2) entre deux sommets voisins, vus depuis le point.
    let half_tan = |i: usize| {
        let (a, b) = (s[i], s[(i + 1) % n]);
        let cross = a.0 * b.1 - a.1 * b.0;
        let dot = a.0 * b.0 + a.1 * b.1;
        cross / (a.2 * b.2 + dot).max(1e-12)
    };
    let mut sum = [0.0f32; 3];
    let mut total = 0.0f32;
    for i in 0..n {
        if s[i].2 < 1e-4 {
            return ring[i].1;
        }
        let wi = (half_tan((i + n - 1) % n) + half_tan(i)) / s[i].2;
        total += wi;
        for (v, d) in sum.iter_mut().zip(ring[i].1) {
            *v += wi * d;
        }
    }
    if total.abs() < 1e-12 {
        return [0.0; 3];
    }
    sum.map(|v| v / total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(w: usize, h: usize) -> RgbImage {
        let mut img = RgbImage {
            width: w,
            height: h,
            data: vec![[0.0; 3]; w * h],
        };
        for y in 0..h {
            for x in 0..w {
                let v = 0.1 + 0.5 * x as f32 / w as f32 + 0.2 * y as f32 / h as f32;
                img.data[y * w + x] = [v, v * 0.8, v * 0.6];
            }
        }
        img
    }

    fn spot(kind: SpotKind, center: (f32, f32), source: (f32, f32)) -> Spot {
        Spot {
            kind,
            center,
            source,
            radius: 0.05,
            feather: 0.0,
            opacity: 100.0,
        }
    }

    #[test]
    fn clone_copies_the_source() {
        let (w, h) = (200, 100);
        let mut img = ramp(w, h);
        // Un carré blanc à la source.
        for y in 45..55 {
            for x in 145..155 {
                img.data[y * w + x] = [1.0; 3];
            }
        }
        let before = img.clone();
        apply(&mut img, &[spot(SpotKind::Clone, (0.25, 0.5), (0.75, 0.5))]);
        assert_eq!(img.data[50 * w + 50], [1.0; 3]);
        // Le reste de la zone : la rampe de la source, plus claire.
        assert_eq!(img.data[50 * w + 42], before.data[50 * w + 142]);
        // Hors du disque, rien ne change.
        assert_eq!(img.data[50 * w + 30], before.data[50 * w + 30]);
        assert_eq!(img.data[10 * w + 50], before.data[10 * w + 50]);
    }

    #[test]
    fn heal_keeps_the_light_of_the_area() {
        let (w, h) = (200, 100);
        let mut img = ramp(w, h);
        // Une poussière sombre au centre de la zone.
        for y in 48..52 {
            for x in 48..52 {
                img.data[y * w + x] = [0.0; 3];
            }
        }
        let clean = ramp(w, h);
        let mut cloned = img.clone();
        apply(&mut img, &[spot(SpotKind::Heal, (0.25, 0.5), (0.75, 0.5))]);
        apply(&mut cloned, &[spot(SpotKind::Clone, (0.25, 0.5), (0.75, 0.5))]);
        // La rampe est affine : la membrane la reconstitue presque exactement.
        let worst = (40..60)
            .flat_map(|y| (40..60).map(move |x| (x, y)))
            .map(|(x, y)| {
                let (a, b) = (img.data[y * w + x], clean.data[y * w + x]);
                (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0f32, f32::max)
            })
            .fold(0.0f32, f32::max);
        assert!(worst < 0.004, "écart {worst}");
        // Le tampon, lui, garde la lumière (plus claire) de la source.
        let c = cloned.data[50 * w + 50][0] - clean.data[50 * w + 50][0];
        assert!(c > 0.2, "{c}");
    }

    #[test]
    fn opacity_feather_and_order() {
        let (w, h) = (200, 100);
        let base = ramp(w, h);
        let mut img = base.clone();
        let mut s = spot(SpotKind::Clone, (0.25, 0.5), (0.75, 0.5));
        s.opacity = 0.0;
        apply(&mut img, std::slice::from_ref(&s));
        assert_eq!(img, base);
        // À 50 % : à mi-chemin entre la zone et la source.
        s.opacity = 50.0;
        apply(&mut img, std::slice::from_ref(&s));
        let mid = (base.data[50 * w + 50][0] + base.data[50 * w + 150][0]) / 2.0;
        assert!((img.data[50 * w + 50][0] - mid).abs() < 1e-5);
        // Le fondu : plein effet au centre, presque rien près du bord.
        let mut img = base.clone();
        s.opacity = 100.0;
        s.feather = 100.0;
        apply(&mut img, std::slice::from_ref(&s));
        let at = |x: usize| (img.data[50 * w + x][0] - base.data[50 * w + x][0]).abs();
        assert!(at(50) > 0.2 && at(41) < 0.02, "{} {}", at(50), at(41));
        // Hors de l'image ou vide : sans effet, sans panique.
        let mut img = base.clone();
        apply(&mut img, &[spot(SpotKind::Heal, (3.0, 3.0), (0.5, 0.5))]);
        apply(&mut img, &[spot(SpotKind::Heal, (0.0, 0.0), (-2.0, 5.0))]);
        apply(&mut img, &[]);
        assert_eq!(img.data[50 * w + 100], base.data[50 * w + 100]);
    }

    #[test]
    fn red_eye_darkens_only_the_red_pupil() {
        let (w, h) = (120, 80);
        let skin = [0.5, 0.3, 0.2];
        let mut img = RgbImage {
            width: w,
            height: h,
            data: vec![skin; w * h],
        };
        // La pupille rouge (rayon 6 px) et son reflet blanc.
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f32 - 60.0).powi(2) + (y as f32 - 40.0).powi(2)).sqrt();
                if d < 6.0 {
                    img.data[y * w + x] = [0.35, 0.03, 0.04];
                }
            }
        }
        img.data[38 * w + 58] = [0.9, 0.9, 0.9];
        // Un cercle généreux sur l'œil : il déborde sur la peau.
        let spot = Spot {
            feather: 20.0,
            ..Spot::new(SpotKind::RedEye, (0.5, 0.5), 0.1)
        };
        assert_eq!(spot.source, spot.center);
        apply(&mut img, &[spot]);
        let p = img.data[40 * w + 60];
        assert!(p[0] < 0.04 && (p[0] - p[1]).abs() < 0.01, "pupille {p:?}");
        assert_eq!(img.data[38 * w + 58], [0.9, 0.9, 0.9], "reflet");
        assert_eq!(img.data[40 * w + 69], skin, "peau dans le cercle");
        assert_eq!(img.data[5 * w + 5], skin, "hors du cercle");
        // Sur un pixel : la peau et le blanc ne bougent pas, un rouge franc si.
        assert_eq!(pupil(skin), skin);
        assert_eq!(pupil([0.9; 3]), [0.9; 3]);
        assert!(pupil([0.3, 0.02, 0.02])[0] < 0.02);
    }

    #[test]
    fn source_found_along_the_texture() {
        // Des rayures horizontales de 10 px : seule une source décalée en largeur a le
        // même bord.
        let (w, h) = (200, 200);
        let mut img = ramp(w, h);
        for y in 0..h {
            if (y / 5) % 2 == 0 {
                for x in 0..w {
                    img.data[y * w + x] = [0.9; 3];
                }
            }
        }
        let (cx, cy) = (0.5, 0.5);
        let (sx, sy) = find_source(&img, (cx, cy), 0.03).unwrap();
        assert!(((sy - cy) * h as f32).abs() < 0.5, "{sx} {sy}");
        assert!((sx - cx).abs() > 0.05);
        // Une zone trop grande pour l'image : pas de source.
        assert_eq!(find_source(&img, (0.5, 0.5), 0.3), None);
    }

    #[test]
    fn same_result_at_every_size() {
        // Le centre de la zone réparée, à deux tailles : mêmes valeurs à l'arrondi près.
        let mut values = Vec::new();
        for (w, h) in [(400, 200), (800, 400)] {
            let mut img = ramp(w, h);
            let (cx, cy) = (w / 4, h / 2);
            img.data[cy * w + cx] = [0.0; 3];
            apply(&mut img, &[spot(SpotKind::Heal, (0.25, 0.5), (0.7, 0.4))]);
            values.push(img.data[cy * w + cx + w / 50]);
        }
        for k in 0..3 {
            assert!((values[0][k] - values[1][k]).abs() < 0.01, "{values:?}");
        }
    }
}
