//! Les retouches locales du panneau Masquage : la zone de chaque masque (dégradé
//! linéaire, dégradé radial, pinceau), restreinte à une plage de tons ou de couleurs
//! s'il en a une, puis ses curseurs appliqués en proportion.
//!
//! Elles agissent sur l'image linéaire de la scène, après la tonalité locale et avant
//! la courbe filmique, comme les curseurs du panneau Base : une exposition locale de
//! −1 assombrit d'un IL, exactement comme le curseur global.

use luma_color::{OkLab, Vec3, fast_atan2, luminance};
use luma_core::{Plane, RgbImage};
use luma_project::{Combine, Mask, Range, Shape};
use rayon::prelude::*;

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x < e0 { 0.0 } else { 1.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// La zone d'un masque sur une image de `w × h` pixels : 1 où il agit pleinement, 0
/// où il n'agit pas (inversion et intensité comprises).
pub fn weights(mask: &Mask, w: usize, h: usize) -> Plane {
    weights_guided(mask, w, h, None)
}

/// [`weights`], avec la clarté de la photo (`guide`, de même taille) pour un pinceau qui
/// suit les bords : chaque touche ne peint que les pixels d'une clarté proche de celle de
/// son centre, puis un filtre guidé lisse la zone sans traverser les contours.
fn weights_guided(mask: &Mask, w: usize, h: usize, guide: Option<&Plane>) -> Plane {
    let guide = guide.filter(|g| mask.edges && (g.width, g.height) == (w, h));
    let mut plane = shape_plane(&mask.shape, w, h, guide);
    // Les parties combinées, dans l'ordre : chacune ajoute, retire ou restreint.
    for part in &mask.parts {
        let mut p = shape_plane(&part.shape, w, h, guide);
        if part.invert {
            p.data.par_iter_mut().for_each(|v| *v = 1.0 - *v);
        }
        plane.data.par_iter_mut().zip(&p.data).for_each(|(v, &q)| {
            *v = match part.combine {
                Combine::Add => *v + q - *v * q,
                Combine::Subtract => *v * (1.0 - q),
                Combine::Intersect => *v * q,
            };
        });
    }
    let amount = (mask.amount / 100.0).clamp(0.0, 1.0);
    let invert = mask.invert;
    plane.data.par_iter_mut().for_each(|v| {
        let m = if invert { 1.0 - *v } else { *v };
        *v = m * amount;
    });
    plane
}

/// La zone d'une forme seule (0 à 1), sans inversion ni intensité ; `guide` : la clarté
/// de la photo, pour un pinceau qui suit les bords.
fn shape_plane(shape: &Shape, w: usize, h: usize, guide: Option<&Plane>) -> Plane {
    let (fw, fh) = (w as f32, h as f32);
    let mut plane = Plane::new(w, h);
    match shape {
        Shape::Full => plane.data.iter_mut().for_each(|v| *v = 1.0),
        Shape::Linear { start, end } => {
            // En pixels, pour que le fondu soit le même dans les deux directions.
            let (sx, sy) = (start.0 * fw, start.1 * fh);
            let (dx, dy) = (end.0 * fw - sx, end.1 * fh - sy);
            let len2 = (dx * dx + dy * dy).max(1e-6);
            plane.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
                let py = y as f32 + 0.5 - sy;
                for (x, v) in row.iter_mut().enumerate() {
                    let t = ((x as f32 + 0.5 - sx) * dx + py * dy) / len2;
                    *v = 1.0 - smoothstep(0.0, 1.0, t);
                }
            });
        }
        Shape::Radial {
            center,
            radius,
            angle,
            feather,
        } => {
            let (cx, cy) = (center.0 * fw, center.1 * fh);
            let (rx, ry) = ((radius.0 * fw).max(1.0), (radius.1 * fh).max(1.0));
            let (s, c) = (-angle.to_radians()).sin_cos();
            let inner = 1.0 - feather.clamp(0.0, 100.0) / 100.0;
            plane.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
                let py = y as f32 + 0.5 - cy;
                for (x, v) in row.iter_mut().enumerate() {
                    let px = x as f32 + 0.5 - cx;
                    let (u, q) = ((px * c - py * s) / rx, (px * s + py * c) / ry);
                    let r = (u * u + q * q).sqrt();
                    *v = 1.0 - smoothstep(inner, 1.0, r);
                }
            });
        }
        Shape::Brush { dabs } => {
            // Les touches dans l'ordre : chacune ajoute (ou gomme) selon son débit.
            for d in dabs {
                let r = (d.radius * fw).max(0.5);
                let inner = 1.0 - d.feather.clamp(0.0, 100.0) / 100.0;
                let flow = (d.flow / 100.0).clamp(0.0, 1.0);
                let (cx, cy) = (d.x * fw, d.y * fh);
                // La clarté sous le centre de la touche : celle qu'elle peint.
                let center = guide.map(|g| {
                    let (gx, gy) = ((cx as usize).min(w - 1), (cy as usize).min(h - 1));
                    g.data[gy * w + gx]
                });
                let x0 = (cx - r).floor().max(0.0) as usize;
                let x1 = ((cx + r).ceil().max(0.0) as usize).min(w);
                let y0 = (cy - r).floor().max(0.0) as usize;
                let y1 = ((cy + r).ceil().max(0.0) as usize).min(h);
                for y in y0..y1 {
                    let py = y as f32 + 0.5 - cy;
                    for x in x0..x1 {
                        let px = x as f32 + 0.5 - cx;
                        let mut a = (1.0 - smoothstep(inner, 1.0, (px * px + py * py).sqrt() / r)) * flow;
                        if let (Some(g), Some(c)) = (guide, center) {
                            // Un écart de clarté de 0,06 passe, au‑delà de 0,14 plus rien.
                            a *= 1.0 - smoothstep(0.06, 0.14, (g.data[y * w + x] - c).abs());
                        }
                        let v = &mut plane.data[y * w + x];
                        *v = if d.erase { *v * (1.0 - a) } else { *v + (1.0 - *v) * a };
                    }
                }
            }
        }
    }
    if let (Some(g), Shape::Brush { dabs }) = (guide, shape)
        && !dabs.is_empty()
    {
        // Lisser sans franchir les bords : le grain de la photo ne laisse pas de trous.
        let radius = dabs.iter().map(|d| d.radius).sum::<f32>() / dabs.len() as f32 * fw;
        let sigma = (radius * 0.15).clamp(1.0, 20.0);
        // Seulement autour des touches (marge de quatre rayons du filtre) : un trait
        // court ne fait pas filtrer toute l'image.
        let margin = sigma * 4.0;
        let (mut x0, mut y0, mut x1, mut y1) = (fw, fh, 0.0f32, 0.0f32);
        for d in dabs {
            let r = (d.radius * fw).max(0.5) + margin;
            x0 = x0.min(d.x * fw - r);
            y0 = y0.min(d.y * fh - r);
            x1 = x1.max(d.x * fw + r);
            y1 = y1.max(d.y * fh + r);
        }
        let (x0, y0) = (x0.max(0.0) as usize, y0.max(0.0) as usize);
        let (x1, y1) = (
            (x1.ceil().max(0.0) as usize).min(w),
            (y1.ceil().max(0.0) as usize).min(h),
        );
        if x1 > x0 + 1 && y1 > y0 + 1 {
            let (cw, ch) = (x1 - x0, y1 - y0);
            let crop = |p: &Plane| Plane {
                width: cw,
                height: ch,
                data: (y0..y1)
                    .flat_map(|y| p.data[y * w + x0..y * w + x1].iter().copied())
                    .collect(),
            };
            let smooth = crop(&plane).guided_by(&crop(g), sigma, 0.0016);
            for (k, row) in smooth.data.chunks(cw).enumerate() {
                for (v, s) in plane.data[(y0 + k) * w + x0..(y0 + k) * w + x1].iter_mut().zip(row) {
                    *v = s.clamp(0.0, 1.0);
                }
            }
        }
    }
    plane
}

/// Ce qu'une plage garde d'un pixel (linéaire, espace de travail) : de 0 à 1.
#[inline]
pub fn range_factor(range: &Range, lab: &OkLab, p: Vec3) -> f32 {
    let [l, a, b] = lab.from_rgb(p.map(|v| v.max(0.0)));
    match *range {
        Range::Luminance { low, high, feather } => {
            let l = (l * 100.0).clamp(0.0, 100.0);
            let f = feather * 0.5;
            // Les bornes extrêmes gardent tout ce qui les dépasse (un blanc brûlé, un
            // noir bouché).
            let above = if low <= 0.0 { 1.0 } else { smoothstep(low - f, low, l) };
            let below = if high >= 100.0 {
                1.0
            } else {
                1.0 - smoothstep(high, high + f, l)
            };
            above * below
        }
        Range::Color { hue, width, feather } => {
            let chroma = a.hypot(b);
            let h = fast_atan2(b, a).to_degrees();
            let d = (h - hue).rem_euclid(360.0);
            let d = d.min(360.0 - d);
            let fade = 4.0 + feather * 0.6;
            // Les gris n'ont pas de teinte : ils ne sont pas pris.
            (1.0 - smoothstep(width, width + fade, d)) * smoothstep(0.012, 0.045, chroma)
        }
    }
}

/// Restreint une zone (`plane`) à la plage, lue sur `img` (linéaire, espace de travail),
/// qui peut être d'une autre taille : chaque point y prend le pixel correspondant.
pub fn restrict(plane: &mut Plane, range: &Range, img: &RgbImage) {
    let (w, h) = (plane.width, plane.height);
    if img.width == 0 || img.height == 0 || w == 0 {
        return;
    }
    let lab = OkLab::for_working();
    let (kx, ky) = (img.width as f32 / w as f32, img.height as f32 / h as f32);
    plane.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let sy = (((y as f32 + 0.5) * ky) as usize).min(img.height - 1);
        for (x, v) in row.iter_mut().enumerate() {
            if *v <= 0.0 {
                continue;
            }
            let sx = (((x as f32 + 0.5) * kx) as usize).min(img.width - 1);
            *v *= range_factor(range, &lab, img.data[sy * img.width + sx]);
        }
    });
}

/// La clarté perçue de `img` (linéaire, espace de travail) à la taille `w × h` : le guide
/// d'un pinceau qui suit les bords.
pub fn lightness_plane(img: &RgbImage, w: usize, h: usize) -> Plane {
    let mut guide = Plane::new(w, h);
    if img.width == 0 || img.height == 0 || w == 0 {
        return guide;
    }
    let (kx, ky) = (img.width as f32 / w as f32, img.height as f32 / h as f32);
    guide.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let sy = (((y as f32 + 0.5) * ky) as usize).min(img.height - 1);
        for (x, v) in row.iter_mut().enumerate() {
            let sx = (((x as f32 + 0.5) * kx) as usize).min(img.width - 1);
            *v = luma_color::fast_cbrt(luminance(img.data[sy * img.width + sx]).max(0.0)).min(1.0);
        }
    });
    guide
}

/// La zone d'un masque à la taille `w × h`, affinée selon la photo `img` (d'une taille
/// quelconque) : le pinceau qui suit les bords, puis la plage de tons ou de couleurs.
pub fn weights_for(mask: &Mask, w: usize, h: usize, img: &RgbImage) -> Plane {
    // Le guide un peu lissé : le grain de la photo n'est pas un bord.
    let brush = |s: &Shape| matches!(s, Shape::Brush { .. });
    let guide = (mask.edges && (brush(&mask.shape) || mask.parts.iter().any(|p| brush(&p.shape))))
        .then(|| lightness_plane(img, w, h).blur(1.5));
    let mut plane = weights_guided(mask, w, h, guide.as_ref());
    if let Some(r) = &mask.range {
        restrict(&mut plane, r, img);
    }
    plane
}

/// La zone d'un masque sur `img`, plage comprise (sans plage : [`weights`]).
pub fn weights_on(mask: &Mask, img: &RgbImage) -> Plane {
    weights_for(mask, img.width, img.height, img)
}

/// Applique les masques actifs à l'image (linéaire, espace de travail). Les plages sont
/// lues sur l'image d'avant les masques : un masque n'en déplace pas un autre.
pub fn apply(img: &mut RgbImage, masks: &[Mask]) {
    let active: Vec<&Mask> = masks.iter().filter(|m| m.is_active()).collect();
    if active.is_empty() {
        return;
    }
    let (w, h) = (img.width, img.height);
    let diag = ((w * w + h * h) as f32).sqrt();
    let stops = Plane::from_rgb(img, |p| (luminance(p).max(1e-6) / 0.18).log2());
    let before = active.iter().any(|m| m.range.is_some() || m.edges).then(|| img.clone());
    // La base de la clarté, comme pour la Clarté globale : une fois pour tous les masques.
    let clarity_base = active
        .iter()
        .any(|m| m.local.clarity != 0.0)
        .then(|| stops.guided(0.008 * diag, 0.5));
    for mask in active {
        let weights = match &before {
            Some(b) => weights_for(mask, w, h, b),
            None => weights(mask, w, h),
        };
        let l = &mask.local;
        let k = |v: f32| v / 100.0;
        let (contrast, hl, sh, wh, bl) = (k(l.contrast), k(l.highlights), k(l.shadows), k(l.whites), k(l.blacks));
        let (temp, tint, sat, clarity) = (k(l.temperature), k(l.tint), k(l.saturation), k(l.clarity));
        img.data
            .par_iter_mut()
            .enumerate()
            .zip(weights.data.par_iter().zip(stops.data.par_iter()))
            .for_each(|((i, p), (&m, &s))| {
                if m <= 0.0 {
                    return;
                }
                // Les tons, en IL : la même famille de courbes que le panneau Base.
                let mut d = l.exposure;
                d += contrast * 0.35 * s * (-(s / 5.0).powi(2)).exp();
                d += hl * 1.2 * smoothstep(-0.5, 2.5, s);
                d += sh * 1.5 * (1.0 - smoothstep(-4.0, -0.5, s));
                d += wh * 0.8 * smoothstep(1.0, 3.0, s);
                d += bl * 0.8 * (1.0 - smoothstep(-6.0, -3.0, s));
                if let Some(base) = &clarity_base {
                    let mid = (-(s / 2.5).powi(2)).exp();
                    d += clarity * 0.7 * (s - base.data[i]) * mid;
                }
                let g = (d * m).clamp(-6.0, 6.0).exp2();
                // Température (bleu ↔ jaune) et teinte (vert ↔ magenta).
                let gr = (temp * 0.35 * m).exp2();
                let gb = (-temp * 0.35 * m).exp2();
                let gg = (-tint * 0.25 * m).exp2();
                let mut c = [p[0] * g * gr, p[1] * g * gg, p[2] * g * gb];
                if sat != 0.0 {
                    let y = luminance(c);
                    let f = (1.0 + sat * m).max(0.0);
                    c = c.map(|v| y + (v - y) * f);
                }
                *p = c;
            });
        if l.moire > 0.0 || l.defringe > 0.0 {
            colour_fix(img, &weights, l.moire / 100.0, l.defringe / 100.0, diag);
        }
    }
}

/// Le moiré et les franges d'un masque, sur l'image en cours : seules les couleurs
/// changent (l'écart de chaque canal à la luminance), la luminance reste au bit près
/// des calculs. Le moiré floute ces couleurs à l'échelle d'un motif fin ; la frange les
/// retire près des contours nets, où l'aberration chromatique laisse ses liserés.
fn colour_fix(img: &mut RgbImage, weights: &Plane, moire: f32, defringe: f32, diag: f32) {
    let (w, h) = (img.width, img.height);
    let y = Plane::from_rgb(img, luminance);
    let chroma: [Plane; 3] = std::array::from_fn(|k| Plane::from_rgb(img, |p| p[k] - luminance(p)));
    // À l'échelle d'un motif fin : 3 px à 2048 px de diagonale, 11 à 24 Mpx.
    let sigma = (0.0015 * diag).max(1.0);
    let smooth: Option<[Plane; 3]> = (moire > 0.0).then(|| std::array::from_fn(|k| chroma[k].blur(sigma)));
    // Les contours nets : la pente du logarithme de la luminance, étendue de quelques
    // pixels (les liserés débordent du contour).
    let edges = (defringe > 0.0).then(|| {
        let log = Plane::from_rgb(img, |p| luminance(p).max(1e-4).ln()).blur(0.7);
        let mut slope = Plane::new(w, h);
        slope.data.par_chunks_mut(w).enumerate().for_each(|(yy, row)| {
            for (x, v) in row.iter_mut().enumerate() {
                let at = |x: usize, y: usize| log.data[y.min(h - 1) * w + x.min(w - 1)];
                let gx = at(x + 1, yy) - at(x.saturating_sub(1), yy);
                let gy = at(x, yy + 1) - at(x, yy.saturating_sub(1));
                *v = smoothstep(0.15, 0.6, (gx * gx + gy * gy).sqrt());
            }
        });
        let reach = (0.0008 * diag).max(1.5);
        slope.blur(reach).map(|v| (v * 3.0).min(1.0))
    });
    img.data.par_iter_mut().enumerate().for_each(|(i, p)| {
        let m = weights.data[i];
        if m <= 0.0 {
            return;
        }
        let mut c = [chroma[0].data[i], chroma[1].data[i], chroma[2].data[i]];
        if let Some(s) = &smooth {
            let a = moire * m;
            for (k, v) in c.iter_mut().enumerate() {
                *v += (s[k].data[i] - *v) * a;
            }
        }
        if let Some(e) = &edges {
            let keep = 1.0 - defringe * m * e.data[i];
            c = c.map(|v| v * keep);
        }
        let l = y.data[i];
        *p = c.map(|v| (l + v).max(0.0));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moire_softens_colour_and_keeps_luminance() {
        // Un motif fin de rayures colorées, de même luminance : un moiré typique.
        let (w, h) = (200, 120);
        let mut img = RgbImage {
            width: w,
            height: h,
            data: vec![[0.0; 3]; w * h],
        };
        for y in 0..h {
            for x in 0..w {
                let (a, b) = ([0.30, 0.18, 0.10], [0.12, 0.20, 0.35]);
                // Même luminance : on ajuste le second.
                let la = luminance(a);
                let lb = luminance(b);
                let b = b.map(|v| v * la / lb);
                img.data[y * w + x] = if (x / 2) % 2 == 0 { a } else { b };
            }
        }
        let before = img.clone();
        let spread = |img: &RgbImage| {
            let v: Vec<f32> = img.data.iter().map(|p| p[0] - luminance(*p)).collect();
            let mean = v.iter().sum::<f32>() / v.len() as f32;
            (v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / v.len() as f32).sqrt()
        };
        let mut mask = Mask::full();
        mask.local.moire = 100.0;
        apply(&mut img, std::slice::from_ref(&mask));
        assert!(
            spread(&img) < 0.2 * spread(&before),
            "{} {}",
            spread(&img),
            spread(&before)
        );
        for (a, b) in img.data.iter().zip(&before.data) {
            assert!((luminance(*a) - luminance(*b)).abs() < 1e-5);
        }
        // À zéro, rien ne change.
        let mut same = before.clone();
        mask.local.moire = 0.0;
        apply(&mut same, &[mask]);
        assert_eq!(same, before);
    }

    #[test]
    fn defringe_clears_edges_and_keeps_flat_colour() {
        let (w, h) = (200, 120);
        let mut img = RgbImage {
            width: w,
            height: h,
            data: vec![[0.02; 3]; w * h],
        };
        for y in 0..h {
            for x in 0..w {
                // Un ciel blanc à droite d'un bord net, un liseré violet sur le bord, et
                // un aplat rouge loin de tout contour.
                if x >= 60 {
                    img.data[y * w + x] = [0.9; 3];
                }
                if (58..62).contains(&x) {
                    img.data[y * w + x] = [0.35, 0.05, 0.45];
                }
                if (130..190).contains(&x) && (30..90).contains(&y) {
                    img.data[y * w + x] = [0.6, 0.1, 0.1];
                }
            }
        }
        let chroma = |p: [f32; 3]| (p[0] - p[1]).abs() + (p[2] - p[1]).abs();
        let fringe = chroma(img.data[60 * w + 59]);
        let flat = chroma(img.data[60 * w + 160]);
        let mut mask = Mask::full();
        mask.local.defringe = 100.0;
        apply(&mut img, &[mask]);
        assert!(
            chroma(img.data[60 * w + 59]) < 0.3 * fringe,
            "liseré {:?}",
            img.data[60 * w + 59]
        );
        assert!((chroma(img.data[60 * w + 160]) - flat).abs() < 1e-4, "aplat rouge");
    }

    #[test]
    fn combined_masks_add_subtract_intersect() {
        let (w, h) = (100, 50);
        // Un dégradé radial à gauche, un autre à droite (sans fondu : des disques).
        let disc = |cx: f32| Shape::Radial {
            center: (cx, 0.5),
            radius: (0.15, 0.4),
            angle: 0.0,
            feather: 0.0,
        };
        let mut m = Mask::radial();
        m.shape = disc(0.3);
        let at = |m: &Mask, x: usize| weights(m, w, h).data[25 * w + x];
        assert_eq!((at(&m, 30), at(&m, 70)), (1.0, 0.0));
        // Ajouter : les deux disques.
        m.parts = vec![luma_project::Part {
            combine: Combine::Add,
            shape: disc(0.7),
            invert: false,
        }];
        assert_eq!((at(&m, 30), at(&m, 70), at(&m, 50)), (1.0, 1.0, 0.0));
        // Soustraire un disque qui recouvre la moitié droite du premier.
        m.parts = vec![luma_project::Part {
            combine: Combine::Subtract,
            shape: disc(0.45),
            invert: false,
        }];
        assert_eq!((at(&m, 20), at(&m, 40)), (1.0, 0.0));
        // Intersection : seule la partie commune reste.
        m.parts[0].combine = Combine::Intersect;
        assert_eq!((at(&m, 20), at(&m, 40), at(&m, 60)), (0.0, 1.0, 0.0));
        // Une partie inversée : soustraire « tout sauf le disque » revient à l'intersection.
        m.parts = vec![luma_project::Part {
            combine: Combine::Subtract,
            shape: disc(0.45),
            invert: true,
        }];
        assert_eq!((at(&m, 20), at(&m, 40), at(&m, 60)), (0.0, 1.0, 0.0));
        // L'inversion et l'intensité du masque s'appliquent au tout.
        m.invert = true;
        m.amount = 50.0;
        assert_eq!((at(&m, 20), at(&m, 40)), (0.5, 0.0));
    }

    #[test]
    fn an_edge_aware_brush_stops_at_the_edge() {
        // Une photo : sombre à gauche, claire à droite (bord net à x = 60) ; un trait de
        // pinceau centré à x = 55 qui déborde sur le côté clair.
        let (w, h) = (120, 60);
        let img = RgbImage::from_fn(w, h, |x, _| if x < 60 { [0.03; 3] } else { [0.5; 3] });
        let mut m = Mask::brush();
        if let Shape::Brush { dabs } = &mut m.shape {
            for k in 0..6 {
                dabs.push(luma_project::Dab {
                    x: 55.0 / w as f32,
                    y: (10.0 + 8.0 * k as f32) / h as f32,
                    radius: 12.0 / w as f32,
                    feather: 20.0,
                    flow: 100.0,
                    erase: false,
                });
            }
        }
        m.local.exposure = 1.0;
        let plain = weights_on(&m, &img);
        m.edges = true;
        let edged = weights_on(&m, &img);
        let at = |p: &Plane, x: usize| p.data[30 * w + x];
        // Du côté sombre, la zone reste ; du côté clair, elle recule nettement.
        assert!(at(&edged, 52) > 0.85, "{}", at(&edged, 52));
        assert!(
            at(&plain, 63) > 0.9 && at(&edged, 63) < 0.35,
            "{} {}",
            at(&plain, 63),
            at(&edged, 63)
        );
        assert!(edged.data.iter().all(|v| (0.0..=1.0).contains(v)));
        // Une photo granuleuse du même côté : pas de trous dans le trait.
        let grainy = RgbImage::from_fn(w, h, |x, y| {
            let n = (((x * 7919 + y * 104_729) % 97) as f32 / 97.0 - 0.5) * 0.02;
            if x < 60 { [0.03 + n; 3] } else { [0.5 + n; 3] }
        });
        let g = weights_on(&m, &grainy);
        let inner = (47..58).map(|x| g.data[30 * w + x]).fold(1.0f32, f32::min);
        assert!(inner > 0.8, "{inner}");
        // Les autres formes ne sont pas touchées par l'option.
        let mut radial = Mask::radial();
        radial.edges = true;
        assert_eq!(weights_on(&radial, &img).data, weights(&radial, w, h).data);
        // Appliqué : le côté clair, hors du trait corrigé, ne bouge presque pas.
        let mut out = img.clone();
        apply(&mut out, std::slice::from_ref(&m));
        assert!(out.data[30 * w + 63][1] < 0.5 * 1.35, "{:?}", out.data[30 * w + 63]);
        assert!(out.data[30 * w + 52][1] > 0.03 * 1.8, "{:?}", out.data[30 * w + 52]);
    }
    use luma_project::{Dab, Local};

    /// Une image de `n` pixels en dégradé de gris, du noir au blanc.
    fn greys(n: usize) -> RgbImage {
        let mut img = RgbImage::new(n, 1);
        for (i, p) in img.data.iter_mut().enumerate() {
            let v = (i as f32 / (n - 1) as f32).powf(2.4);
            *p = [v, v, v];
        }
        img
    }

    #[test]
    fn luminance_range_keeps_only_its_tones() {
        let mut m = Mask::full();
        m.range = Some(Range::Luminance {
            low: 60.0,
            high: 100.0,
            feather: 0.0,
        });
        let img = greys(101);
        let p = weights_on(&m, &img);
        assert!(p.data[5] == 0.0, "les ombres ne sont pas prises");
        assert!(p.data[100] > 0.99, "le blanc est pris");
        // Fondu : des valeurs entre 0 et 1 autour de la borne basse.
        m.range = Some(Range::Luminance {
            low: 60.0,
            high: 100.0,
            feather: 40.0,
        });
        let p = weights_on(&m, &img);
        assert!(p.data.iter().any(|&v| v > 0.05 && v < 0.95));
        // Les valeurs montent avec la luminosité.
        assert!(p.data.windows(2).all(|w| w[1] >= w[0] - 1e-6));
    }

    #[test]
    fn color_range_picks_a_hue_and_leaves_greys() {
        let lab = OkLab::for_working();
        let blue = lab.to_rgb([0.6, -0.03, -0.12]);
        let orange = lab.to_rgb([0.7, 0.08, 0.1]);
        let grey = [0.2, 0.2, 0.2];
        let hue = fast_atan2(-0.12, -0.03).to_degrees().rem_euclid(360.0);
        let r = Range::Color {
            hue,
            width: 20.0,
            feather: 0.0,
        };
        assert!(range_factor(&r, &lab, blue) > 0.99);
        assert!(range_factor(&r, &lab, orange) == 0.0);
        assert!(range_factor(&r, &lab, grey) == 0.0);
    }

    #[test]
    fn a_range_mask_only_touches_its_pixels() {
        // À gauche des gris sombres, à droite des gris clairs : +1 IL sur les clairs.
        let mut img = RgbImage::new(2, 1);
        img.data = vec![[0.02; 3], [0.6; 3]];
        let mut m = Mask::full();
        m.local.exposure = 1.0;
        m.range = Some(Range::luminance());
        apply(&mut img, &[m]);
        assert!((img.data[0][0] - 0.02).abs() < 1e-6, "le sombre ne bouge pas");
        assert!(
            (img.data[1][0] - 1.2).abs() < 1e-3,
            "le clair double : {}",
            img.data[1][0]
        );
    }

    #[test]
    fn linear_gradient_fades_from_start_to_end() {
        let m = Mask::linear(); // du haut (0,15) au milieu (0,5)
        let p = weights(&m, 10, 100);
        assert!(p.data[5 * 10] > 0.99, "plein effet au‑dessus du départ");
        assert!(p.data[90 * 10] < 0.01, "rien après l'arrivée");
        let mid = p.data[32 * 10];
        assert!(mid > 0.3 && mid < 0.7, "{mid}");
    }

    #[test]
    fn radial_inverted_and_amount() {
        let mut m = Mask::radial();
        let p = weights(&m, 100, 100);
        assert!(p.data[50 * 100 + 50] > 0.99 && p.data[0] < 0.01);
        m.invert = true;
        m.amount = 50.0;
        let p = weights(&m, 100, 100);
        assert!((p.data[0] - 0.5).abs() < 0.01 && p.data[50 * 100 + 50] < 0.01);
    }

    #[test]
    fn brush_paints_and_erases() {
        let mut m = Mask::brush();
        let dab = Dab {
            x: 0.5,
            y: 0.5,
            radius: 0.2,
            feather: 0.0,
            flow: 100.0,
            erase: false,
        };
        m.shape = Shape::Brush {
            dabs: vec![
                dab,
                Dab {
                    erase: true,
                    radius: 0.05,
                    ..dab
                },
            ],
        };
        let p = weights(&m, 100, 100);
        assert!(p.data[50 * 100 + 40] > 0.99, "peint");
        assert!(p.data[50 * 100 + 50] < 0.01, "gommé au centre");
        assert!(p.data[0] == 0.0, "rien ailleurs");
    }

    #[test]
    fn local_exposure_is_a_true_stop_inside_the_mask_only() {
        let mut img = RgbImage::from_fn(20, 20, |_, _| [0.18, 0.18, 0.18]);
        let mut m = Mask::radial();
        m.shape = Shape::Radial {
            center: (0.5, 0.5),
            radius: (0.2, 0.2),
            angle: 0.0,
            feather: 0.0,
        };
        m.local = Local {
            exposure: -1.0,
            ..Local::default()
        };
        apply(&mut img, &[m]);
        assert!((img.data[10 * 20 + 10][0] - 0.09).abs() < 1e-4);
        assert!((img.data[0][0] - 0.18).abs() < 1e-6);
    }

    #[test]
    fn temperature_warms_and_saturation_keeps_luminance() {
        let mut img = RgbImage::from_fn(4, 4, |_, _| [0.2, 0.2, 0.2]);
        let mut m = Mask::linear();
        m.shape = Shape::Linear {
            start: (0.0, 2.0),
            end: (0.0, 3.0),
        }; // partout
        m.local = Local {
            temperature: 50.0,
            ..Local::default()
        };
        apply(&mut img, std::slice::from_ref(&m));
        let p = img.data[0];
        assert!(p[0] > p[1] && p[1] > p[2], "plus chaud : {p:?}");
        let mut img = RgbImage::from_fn(4, 4, |_, _| [0.3, 0.2, 0.1]);
        let y0 = luminance(img.data[0]);
        m.local = Local {
            saturation: -100.0,
            ..Local::default()
        };
        apply(&mut img, &[m]);
        let p = img.data[0];
        assert!((p[0] - p[2]).abs() < 1e-5 && (luminance(p) - y0).abs() < 1e-5, "{p:?}");
    }
}
