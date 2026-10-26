//! Les taches de capteur : des poussières posées devant le capteur, qui assombrissent
//! d'un même facteur une petite tache floue, surtout visibles dans un ciel à petite
//! ouverture.
//!
//! Le travail se fait sur le logarithme de la luminance : une poussière y est un creux
//! de même profondeur quelle que soit la lumière de la scène (sauf dans les ombres
//! profondes, où elle ne se voit pas). La vue des taches montre
//! l'écart à un fond flouté, très amplifié ; la détection garde les creux petits, ronds
//! et nets, dans les zones calmes de la photo.

use luma_color::luminance;
use luma_core::{Plane, RgbImage};
use luma_project::{Spot, SpotKind};
use rayon::prelude::*;

/// Le flou du fond, en pixels, pour une image de `width` de large (16 px à 2048 px, au
/// moins 5) : bien plus large qu'une poussière, pour qu'il ne la suive pas.
fn sigma(width: usize) -> f32 {
    (16.0 * width as f32 / 2048.0).max(5.0)
}

/// Le logarithme de la luminance, décalé : dans les ombres profondes, où une poussière
/// ne se voit pas, le bruit n'y est pas amplifié.
fn log_luminance(img: &RgbImage) -> Plane {
    Plane::from_rgb(img, |p| (luminance(p).max(0.0) + 0.02).ln())
}

/// La vue des taches (RGBA) : en gris, l'écart de chaque pixel à son voisinage, amplifié
/// (un creux de 5 % est presque noir sur un fond gris moyen). Les contours de la photo y
/// sont aussi visibles ; les poussières y sortent comme des points sombres.
pub fn view(img: &RgbImage) -> Vec<u8> {
    let l = log_luminance(img);
    let background = l.blur(sigma(img.width));
    l.data
        .par_iter()
        .zip(&background.data)
        .flat_map_iter(|(&v, &b)| {
            let g = ((0.5 + 6.0 * (v - b)).clamp(0.0, 1.0) * 255.0).round() as u8;
            [g, g, g, 255]
        })
        .collect()
}

/// Les poussières trouvées sur `img` (la base développée, avant les retouches à venir) :
/// des correcteurs posés dessus, leur source choisie, les plus marquées d'abord, au plus
/// `max`.
pub fn find(img: &RgbImage, max: usize) -> Vec<Spot> {
    let (w, h) = (img.width, img.height);
    if w < 16 || h < 16 {
        return Vec::new();
    }
    let s = sigma(w);
    let l = log_luminance(img);
    let fine = l.blur((s / 16.0).max(0.8));
    // Le fond, sans les creux : les pixels plus sombres que le premier fond y sont
    // remplacés par lui, puis on refloute. Il ne descend pas sous une poussière, dont
    // les voisins ne paraissent donc pas plus clairs que lui.
    let rough = l.blur(s);
    let background = l.zip(&rough, f32::max).blur(s);
    // Le creux, un peu lissé contre le bruit.
    let depth = background.zip(&fine, |b, v| b - v);
    // L'agitation du voisinage : une texture (feuillage, herbe) n'a pas de poussières
    // visibles, et ses creux ne doivent pas être pris pour elles. C'est la part des
    // pixels qui s'écartent nettement du fond, sur un large voisinage : une texture en a
    // beaucoup, une poussière isolée très peu. Elle déborde un peu (la plus forte de
    // neuf voisines), pour que le bord d'une texture compte aussi.
    let wide = 3.0 * s;
    let busy = fine
        .zip(&rough, |v, r| if (v - r).abs() > 0.03 { 1.0 } else { 0.0 })
        .blur(wide);
    let k = wide.round() as isize;
    let busy_near = |x: usize, y: usize| {
        let mut m = 0.0f32;
        for dy in [-k, 0, k] {
            for dx in [-k, 0, k] {
                let xx = (x as isize + dx).clamp(0, w as isize - 1) as usize;
                let yy = (y as isize + dy).clamp(0, h as isize - 1) as usize;
                m = m.max(busy.data[yy * w + xx]);
            }
        }
        m
    };
    // Un flou d'arrière-plan (bokeh) a des taches claires autant que sombres ; une
    // poussière n'a, autour d'elle, aucune tache claire d'ampleur comparable.
    let bright = fine.zip(&rough, |v, r| v - r);
    let reach = wide as isize;
    let bright_near = |cx: f32, cy: f32, peak: f32| {
        let (cx, cy) = (cx as isize, cy as isize);
        let ys = (cy - reach).max(0)..(cy + reach + 1).min(h as isize);
        ys.into_iter().any(|y| {
            let xs = (cx - reach).max(0)..(cx + reach + 1).min(w as isize);
            xs.into_iter()
                .any(|x| bright.data[y as usize * w + x as usize] > 0.6 * peak)
        })
    };
    let mask: Vec<bool> = depth.data.par_iter().map(|&d| d > 0.02).collect();
    // Les taches : composantes connexes du masque.
    let mut seen = vec![false; w * h];
    let mut found: Vec<(f32, Spot)> = Vec::new();
    let largest = std::f32::consts::PI * (1.5 * s).powi(2);
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !mask[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let (mut area, mut sum, mut sx, mut sy, mut peak) = (0usize, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            let d = depth.data[i];
            area += 1;
            sum += d;
            peak = peak.max(d);
            sx += d * (x as f32 + 0.5);
            sy += d * (y as f32 + 0.5);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            let mut visit = |j: usize| {
                if mask[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < w {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - w);
            }
            if y + 1 < h {
                visit(i + w);
            }
        }
        let (bw, bh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        // Trop grande, ou allongée (un fil, une ligne) : pas une poussière.
        if area as f32 > largest || bw.max(bh) > 2.5 * bw.min(bh) + 2.0 {
            continue;
        }
        let (cx, cy) = (sx / sum, sy / sum);
        if busy_near(cx as usize, cy as usize) >= 0.08 || bright_near(cx, cy, peak) {
            continue;
        }
        let r = 1.5 * (area as f32 / std::f32::consts::PI).sqrt() + 0.15 * s;
        // Près des bords, le fond flouté prolonge le bord : on n'y cherche pas.
        let margin = r.max(s);
        if cx < margin || cy < margin || cx + margin > w as f32 || cy + margin > h as f32 {
            continue;
        }
        let center = (cx / w as f32, cy / h as f32);
        let radius = r / w as f32;
        let mut spot = Spot::new(SpotKind::Heal, center, radius);
        if let Some(source) = crate::retouch::find_source(img, center, radius) {
            spot.source = source;
        }
        found.push((sum, spot));
    }
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    found.into_iter().take(max).map(|(_, s)| s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un ciel en dégradé, un léger bruit, une moitié droite très texturée.
    fn sky(w: usize, h: usize) -> RgbImage {
        let noise = |x: usize, y: usize, k: u32| {
            let mut v = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663) ^ k;
            v ^= v >> 13;
            v = v.wrapping_mul(0x5bd1_e995);
            v ^= v >> 15;
            (v % 1000) as f32 / 1000.0 - 0.5
        };
        let mut img = RgbImage {
            width: w,
            height: h,
            data: vec![[0.0; 3]; w * h],
        };
        for y in 0..h {
            for x in 0..w {
                let base = 0.25 + 0.2 * y as f32 / h as f32;
                let texture = if x > w * 2 / 3 {
                    0.4 * noise(x / 2, y / 2, 7)
                } else {
                    0.0
                };
                let v = base * (1.0 + 0.01 * noise(x, y, 1) + texture);
                img.data[y * w + x] = [v * 0.8, v * 0.9, v * 1.1];
            }
        }
        img
    }

    fn darken(img: &mut RgbImage, cx: f32, cy: f32, r: f32, depth: f32) {
        let w = img.width;
        for y in 0..img.height {
            for x in 0..w {
                let d2 = (x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2);
                let k = 1.0 - depth * (-d2 / (2.0 * r * r)).exp();
                let p = &mut img.data[y * w + x];
                *p = p.map(|v| v * k);
            }
        }
    }

    #[test]
    fn finds_the_dust_and_only_the_dust() {
        let (w, h) = (600, 400);
        let mut img = sky(w, h);
        let dust = [
            (60.0, 50.0, 2.0, 0.12),
            (150.0, 300.0, 3.0, 0.08),
            (300.0, 120.0, 1.5, 0.15),
            (330.0, 340.0, 2.5, 0.06),
        ];
        for &(x, y, r, d) in &dust {
            darken(&mut img, x, y, r, d);
        }
        // Un fil sombre dans le ciel, et des creux dans la texture : à laisser.
        for x in 100..260 {
            for y in 200..202 {
                let p = &mut img.data[y * w + x];
                *p = p.map(|v| v * 0.85);
            }
        }
        darken(&mut img, 500.0, 200.0, 2.0, 0.12);
        let spots = find(&img, 50);
        assert_eq!(spots.len(), dust.len(), "{spots:?}");
        for &(x, y, ..) in &dust {
            let near = spots
                .iter()
                .any(|s| (s.center.0 * w as f32 - x).abs() < 1.5 && (s.center.1 * h as f32 - y).abs() < 1.5);
            assert!(near, "poussière en ({x}, {y}) : {spots:?}");
        }
        // Réparées, les poussières ne se voient plus.
        let mut fixed = img.clone();
        crate::retouch::apply(&mut fixed, &spots);
        let clean = sky(w, h);
        for &(x, y, ..) in &dust {
            let i = y as usize * w + x as usize;
            let ratio = luminance(fixed.data[i]) / luminance(clean.data[i]);
            assert!((ratio - 1.0).abs() < 0.02, "({x}, {y}) : {ratio}");
        }
        assert!(find(&fixed, 50).is_empty());
        // Au plus `max`, les plus marquées d'abord (l'assombrissement total : la plus
        // large ici).
        let first = find(&img, 1);
        assert_eq!(first.len(), 1);
        assert!((first[0].center.0 * w as f32 - 150.0).abs() < 1.5, "{first:?}");
    }

    #[test]
    fn the_view_shows_dust_dark_on_grey() {
        let (w, h) = (300, 200);
        let mut img = sky(w, h);
        darken(&mut img, 50.0, 50.0, 2.0, 0.1);
        let v = view(&img);
        assert_eq!(v.len(), w * h * 4);
        let at = |x: usize, y: usize| v[(y * w + x) * 4];
        assert!(at(50, 50) < 70, "{}", at(50, 50));
        assert!((at(100, 150) as i32 - 128).abs() < 12, "{}", at(100, 150));
        assert!(
            find(
                &RgbImage {
                    width: 4,
                    height: 4,
                    data: vec![[0.5; 3]; 16]
                },
                5
            )
            .is_empty()
        );
    }
}
