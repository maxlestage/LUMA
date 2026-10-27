//! Le redressement automatique : l'inclinaison de la photo lue sur ses lignes droites
//! (l'horizon, les arêtes des bâtiments), par une transformée de Hough.
//!
//! Chaque pixel de contour ne vote que pour les droites proches de la direction de son
//! gradient (±3°), dans deux familles : les presque horizontales et les presque
//! verticales (±20°). Pour chaque famille, l'angle retenu est celui où les votes des
//! longues droites se concentrent le plus. Les verticales priment quand il y en a des
//! deux côtés (leur moyenne ôte l'effet d'une plongée) ; d'un seul côté, seulement si
//! l'horizon les confirme ; sinon l'horizon, s'il penche de 5° au plus. Dans le doute
//! (une photo de près, toute en lignes de fuite), pas d'angle.

use luma_color::luminance;
use luma_core::{Plane, RgbImage};
use rayon::prelude::*;

/// L'écart à l'horizontale ou à la verticale cherché, en degrés.
const RANGE: f32 = 20.0;
/// Le pas des angles essayés, en degrés.
const STEP: f32 = 0.1;
/// La tolérance autour de la direction du gradient, en degrés.
const SPREAD: f32 = 3.0;

/// L'inclinaison des lignes de `img` (non recadrée), en degrés, à mettre dans
/// `Crop::angle` pour la redresser ; `None` sans ligne assez longue.
pub fn angle(img: &RgbImage) -> Option<f32> {
    let (w, h) = RgbImage::fit(img.width, img.height, 1024);
    if w < 32 || h < 32 {
        return None;
    }
    let small = img.resize_area(w, h);
    let l = Plane::from_rgb(&small, |p| luminance(p).max(0.0).powf(1.0 / 2.2)).blur(1.0);
    let at = |x: usize, y: usize| l.data[y * w + x];
    // Les pixels de contour : le gradient (Sobel), assez fort.
    let edges: Vec<(f32, f32, f32, f32)> = (1..h - 1)
        .into_par_iter()
        .flat_map_iter(|y| {
            let at = &at;
            (1..w - 1).filter_map(move |x| {
                let gx = at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1)
                    - at(x - 1, y - 1)
                    - 2.0 * at(x - 1, y)
                    - at(x - 1, y + 1);
                let gy = at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1)
                    - at(x - 1, y - 1)
                    - 2.0 * at(x, y - 1)
                    - at(x + 1, y - 1);
                let m = (gx * gx + gy * gy).sqrt();
                (m > 0.12).then_some((x as f32, y as f32, gx, gy))
            })
        })
        .collect();
    let bins = (2.0 * RANGE / STEP).round() as usize + 1;
    let theta = |b: usize| (b as f32 * STEP - RANGE).to_radians();
    let diag = ((w * w + h * h) as f32).sqrt();
    let rhos = (2.0 * diag).ceil() as usize + 1;
    // Deux accumulateurs : droites presque horizontales, presque verticales.
    let mut horizontal = vec![0u32; bins * rhos];
    let mut vertical = vec![0u32; bins * rhos];
    for &(x, y, gx, gy) in &edges {
        // La direction de la ligne, perpendiculaire au gradient, ramenée dans [0, 180).
        let mut a = gy.atan2(gx).to_degrees() + 90.0;
        a = a.rem_euclid(180.0);
        let (tilt, vertical_family) = if (a - 90.0).abs() <= RANGE + SPREAD {
            (a - 90.0, true)
        } else if a <= RANGE + SPREAD {
            (a, false)
        } else if a >= 180.0 - RANGE - SPREAD {
            (a - 180.0, false)
        } else {
            continue;
        };
        let acc = if vertical_family {
            &mut vertical
        } else {
            &mut horizontal
        };
        let lo = (((tilt - SPREAD + RANGE) / STEP).floor().max(0.0)) as usize;
        let hi = (((tilt + SPREAD + RANGE) / STEP).ceil() as usize).min(bins - 1);
        for b in lo..=hi {
            let (s, c) = theta(b).sin_cos();
            // Horizontales : −x sin θ + y cos θ ; verticales : x cos θ + y sin θ.
            let rho = if vertical_family { x * c + y * s } else { -x * s + y * c };
            let r = (rho + diag).round() as usize;
            if r < rhos {
                acc[b * rhos + r] += 1;
            }
        }
    }
    // Une photo sans ligne assez longue (au moins 12 % du petit côté) n'a pas d'angle.
    let least = (0.12 * w.min(h) as f32) as u32;
    let longest = horizontal.iter().chain(&vertical).copied().max().unwrap_or(0);
    if longest < least {
        return None;
    }
    // Pour chaque angle, la concentration des votes des longues droites : la somme des
    // carrés des cases d'au moins `least` votes, sur une plage de distances. Au bon
    // angle, les pixels d'une droite tombent dans quelques cases ; à côté, ils
    // s'étalent ; les textures (branches, herbe) ne font que des cases faibles.
    let concentration = |acc: &[u32], rho: std::ops::Range<usize>| -> Vec<f64> {
        (0..bins)
            .map(|b| {
                acc[b * rhos + rho.start..b * rhos + rho.end]
                    .iter()
                    .filter(|&&v| v >= least)
                    .map(|&v| (v as f64).powi(2))
                    .sum()
            })
            .collect()
    };
    // Le meilleur angle d'un profil, affiné par la parabole des trois voisins ; rien au
    // bord de la plage (une ligne de fuite ou une texture), ni sans droite.
    let peak = |score: &[f64]| -> Option<(f32, f64)> {
        let best = (0..bins).max_by(|&a, &b| score[a].total_cmp(&score[b]))?;
        if score[best] <= 0.0 || best < 10 || best + 10 >= bins {
            return None;
        }
        let (l, c, r) = (score[best - 1], score[best], score[best + 1]);
        let d = l - 2.0 * c + r;
        let offset = if d < 0.0 { (0.5 * (l - r) / d) as f32 } else { 0.0 };
        Some(((best as f32 + offset.clamp(-0.5, 0.5)) * STEP - RANGE, c))
    };
    let horizon = peak(&concentration(&horizontal, 0..rhos));
    // Les verticales, à gauche et à droite du centre : en plongée ou en contre‑plongée,
    // elles convergent et penchent en sens contraires ; leur moyenne en ôte l'effet.
    let middle = (diag + w as f32 / 2.0).round() as usize;
    let left = peak(&concentration(&vertical, 0..middle));
    let right = peak(&concentration(&vertical, middle..rhos));
    match (left, right, horizon) {
        // Des verticales des deux côtés : leur moyenne dit l'inclinaison de l'appareil.
        (Some((a, _)), Some((b, _)), _) => Some(0.5 * (a + b)),
        // D'un seul côté, une plongée les ferait pencher : il faut que l'horizon dise
        // la même chose.
        (Some((v, _)), None, Some((h, _))) | (None, Some((v, _)), Some((h, _))) if (v - h).abs() <= 1.5 => Some(h),
        // Sinon l'horizon, s'il penche peu : une ligne très inclinée est une ligne de
        // fuite (une table, une étagère vue de près), pas l'horizon.
        (_, _, Some((h, _))) if h.abs() <= 5.0 => Some(h),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un bruit sans motif (un hachage, pas une formule linéaire qui ferait des rayures).
    fn noise(x: usize, y: usize) -> f32 {
        let mut v = (x as u32).wrapping_mul(0x9e37_79b9) ^ (y as u32).wrapping_mul(0x85eb_ca6b);
        v ^= v >> 15;
        v = v.wrapping_mul(0x2c1b_3c6d);
        v ^= v >> 12;
        (v % 1000) as f32 / 1000.0 - 0.5
    }

    /// Un paysage incliné de `tilt` degrés : un ciel clair au‑dessus de l'horizon, une
    /// mer sombre, deux arêtes verticales de bâtiment, un trait en biais à 30° (à
    /// ignorer), et un léger bruit.
    fn scene(w: usize, h: usize, tilt: f32) -> RgbImage {
        let (s, c) = tilt.to_radians().sin_cos();
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        RgbImage::from_fn(w, h, |x, y| {
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            // Coordonnées dans la scène redressée.
            let u = dx * c + dy * s;
            let v = -dx * s + dy * c;
            // Des bords doux, comme ceux d'une vraie optique (un pixel de fondu).
            let soft = |d: f32| (0.5 + d).clamp(0.0, 1.0);
            let mut l = 0.15 + 0.55 * soft(-v);
            let building = soft(0.04 * w as f32 - (u - 0.2 * w as f32).abs()) * soft(v + 0.3 * h as f32) * soft(-v);
            l += (0.3 - l) * building;
            let line = soft(1.5 - (u * 0.5 - v * 0.866 - 0.1 * w as f32).abs());
            l += (0.95 - l) * line;
            let n = noise(x, y) * 0.02;
            [l + n, l + n, l + n]
        })
    }

    #[test]
    fn finds_the_tilt_of_the_horizon_and_the_verticals() {
        for tilt in [2.5f32, -4.0, 0.0, 7.3] {
            let a = angle(&scene(1200, 800, tilt)).expect("des lignes");
            assert!((a - tilt).abs() < 0.2, "inclinaison {tilt} : trouvé {a}");
        }
    }

    #[test]
    fn straightening_with_the_angle_levels_the_photo() {
        let img = scene(1200, 800, 3.0);
        let a = angle(&img).unwrap();
        // Le cadre redressé de cet angle (assez petit pour ne pas sortir de la photo).
        let crop = luma_project::Crop {
            left: 0.15,
            top: 0.15,
            right: 0.85,
            bottom: 0.85,
            angle: a,
        };
        let level = crate::geometry::crop(&img, &crop).unwrap();
        let again = angle(&level).unwrap();
        assert!(again.abs() < 0.2, "après redressement : {again}");
    }

    #[test]
    fn no_line_no_angle() {
        let noise = RgbImage::from_fn(600, 400, |x, y| {
            let n = noise(x, y) * 0.05 + 0.4;
            [n, n, n]
        });
        assert_eq!(angle(&noise), None);
        assert_eq!(angle(&RgbImage::from_fn(10, 10, |_, _| [0.5; 3])), None);
    }
}
