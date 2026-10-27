//! Fluidité : les coups de pinceau (pousser, gonfler, dégonfler) deviennent un champ
//! de déplacement, puis la photo est relue à travers lui.
//!
//! Le champ est une grille (au plus 512 nœuds sur le grand côté) qui donne, pour chaque
//! point de l'image finale, l'endroit de la photo d'origine où le lire. Chaque touche du
//! pinceau compose sa déformation avec les précédentes : une zone poussée deux fois va
//! deux fois plus loin, et ne se déchire pas. Hors des zones touchées, les pixels ne
//! sont pas relus : ils restent exactement les mêmes.

use luma_core::RgbImage;
use luma_project::{Warp, WarpKind};
use rayon::prelude::*;

/// Le plus grand côté de la grille du champ.
const GRID: f32 = 512.0;
/// L'écart entre deux touches du pinceau, en fraction du rayon.
const SPACING: f32 = 0.25;

/// La douceur du pinceau : 1 au centre, 0 au bord, dérivée nulle aux deux.
fn falloff(t2: f32) -> f32 {
    if t2 >= 1.0 {
        0.0
    } else {
        let u = 1.0 - t2;
        u * u
    }
}

/// La grille : pour chaque nœud, la position (en pixels) où lire la photo.
struct Field {
    step: f32,
    gw: usize,
    gh: usize,
    at: Vec<[f32; 2]>,
}

impl Field {
    fn new(w: usize, h: usize) -> Field {
        let step = (w.max(h) as f32 / GRID).max(1.0);
        let gw = ((w - 1) as f32 / step).ceil() as usize + 1;
        let gh = ((h - 1) as f32 / step).ceil() as usize + 1;
        let at = (0..gw * gh)
            .map(|i| [(i % gw) as f32 * step, (i / gw) as f32 * step])
            .collect();
        Field { step, gw, gh, at }
    }

    /// La position lue pour le point `(x, y)` (en pixels), bilinéaire entre les nœuds de
    /// `nodes` (une grille de `gw` nœuds de large), bords prolongés.
    fn lookup(nodes: &[[f32; 2]], gw: usize, gh: usize, step: f32, x: f32, y: f32) -> [f32; 2] {
        let gx = (x / step).clamp(0.0, (gw - 1) as f32);
        let gy = (y / step).clamp(0.0, (gh - 1) as f32);
        let (x0, y0) = (gx.floor() as usize, gy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(gw - 1), (y0 + 1).min(gh - 1));
        let (fx, fy) = (gx - x0 as f32, gy - y0 as f32);
        let p = |x: usize, y: usize| nodes[y * gw + x];
        let (a, b, c, d) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            top + (bottom - top) * fy
        })
    }

    /// Une touche : chaque point `q` à moins de `r` pixels de `center` lit désormais ce
    /// que lisait `q − δ(q)`. Rend la boîte touchée (en pixels).
    fn dab(&mut self, center: [f32; 2], r: f32, delta: impl Fn([f32; 2], f32) -> [f32; 2]) -> [f32; 4] {
        let s = self.step;
        let (gw, gh) = (self.gw, self.gh);
        // Les nœuds touchés, et la marge qu'ils peuvent lire autour (le déplacement ne
        // dépasse jamais le rayon).
        let lo = |c: f32, m: f32| ((c - m) / s).floor().max(0.0) as usize;
        let hi = |c: f32, m: f32, n: usize| (((c + m) / s).ceil().max(0.0) as usize).min(n - 1);
        let (ax, ay) = (lo(center[0], 2.0 * r), lo(center[1], 2.0 * r));
        let (bx, by) = (hi(center[0], 2.0 * r, gw), hi(center[1], 2.0 * r, gh));
        if ax > bx || ay > by {
            return [0.0; 4];
        }
        // Une copie de la zone lue, pour que la touche ne relise pas ses propres nœuds.
        let (cw, ch) = (bx - ax + 1, by - ay + 1);
        let copy: Vec<[f32; 2]> = (0..ch)
            .flat_map(|j| self.at[(ay + j) * gw + ax..(ay + j) * gw + bx + 1].to_vec())
            .collect();
        let (tx, ty) = (lo(center[0], r), lo(center[1], r));
        let (ux, uy) = (hi(center[0], r, gw), hi(center[1], r, gh));
        for gy in ty..=uy {
            for gx in tx..=ux {
                let q = [gx as f32 * s, gy as f32 * s];
                let (dx, dy) = (q[0] - center[0], q[1] - center[1]);
                let f = falloff((dx * dx + dy * dy) / (r * r));
                if f <= 0.0 {
                    continue;
                }
                let d = delta(q, f);
                // La lecture dans la copie, décalée de son coin.
                let (px, py) = (q[0] - d[0] - ax as f32 * s, q[1] - d[1] - ay as f32 * s);
                self.at[gy * gw + gx] = Field::lookup(&copy, cw, ch, s, px, py);
            }
        }
        [
            center[0] - r - s,
            center[1] - r - s,
            center[0] + r + s,
            center[1] + r + s,
        ]
    }
}

/// Les touches d'un coup de pinceau : le chemin repris tous les `SPACING` rayons.
fn resample(points: &[[f32; 2]], spacing: f32) -> Vec<[f32; 2]> {
    let mut out = vec![points[0]];
    let mut last = points[0];
    for &p in &points[1..] {
        loop {
            let (dx, dy) = (p[0] - last[0], p[1] - last[1]);
            let len = (dx * dx + dy * dy).sqrt();
            if len < spacing {
                break;
            }
            last = [last[0] + dx / len * spacing, last[1] + dy / len * spacing];
            out.push(last);
        }
    }
    out
}

/// Déforme `img` (l'image recadrée) par les coups de pinceau `warps`, dans l'ordre.
pub fn apply(img: &mut RgbImage, warps: &[Warp]) {
    let (w, h) = (img.width, img.height);
    if warps.is_empty() || w < 2 || h < 2 {
        return;
    }
    let mut field = Field::new(w, h);
    let mut touched: Option<[f32; 4]> = None;
    for warp in warps.iter().filter(|w| !w.points.is_empty()) {
        let r = (warp.radius * w as f32).max(1.0);
        let k = (warp.strength / 100.0).clamp(0.0, 1.0);
        if k <= 0.0 {
            continue;
        }
        let path: Vec<[f32; 2]> = warp.points.iter().map(|&(x, y)| [x * w as f32, y * h as f32]).collect();
        let dabs = resample(&path, SPACING * r);
        let mut boxes = Vec::new();
        match warp.kind {
            // Chaque touche amène sous elle ce qui était sous la précédente.
            WarpKind::Push => {
                for pair in dabs.windows(2) {
                    let d = [(pair[1][0] - pair[0][0]) * k, (pair[1][1] - pair[0][1]) * k];
                    boxes.push(field.dab(pair[1], r, |_, f| [d[0] * f, d[1] * f]));
                }
            }
            // Le centre grandit (ou se resserre) : au plus deux fois au centre, réparti
            // sur les touches qui se recouvrent.
            WarpKind::Bloat | WarpKind::Pucker => {
                let sign = if warp.kind == WarpKind::Bloat { 1.0 } else { -1.0 };
                let a = sign * 0.5 * k / (dabs.len() as f32).min(1.0 / SPACING);
                for &c in &dabs {
                    boxes.push(field.dab(c, r, |q, f| [(q[0] - c[0]) * a * f, (q[1] - c[1]) * a * f]));
                }
            }
        }
        for b in boxes {
            touched = Some(match touched {
                None => b,
                Some(t) => [t[0].min(b[0]), t[1].min(b[1]), t[2].max(b[2]), t[3].max(b[3])],
            });
        }
    }
    let Some(t) = touched else {
        return;
    };
    // Seuls les pixels de la zone touchée sont relus.
    let x0 = t[0].floor().max(0.0) as usize;
    let x1 = (t[2].ceil().max(0.0) as usize).min(w - 1);
    let y0 = t[1].floor().max(0.0) as usize;
    let y1 = (t[3].ceil().max(0.0) as usize).min(h - 1);
    if x0 > x1 || y0 > y1 {
        return;
    }
    let source = img.clone();
    let f = &field;
    img.data
        .par_chunks_mut(w)
        .enumerate()
        .skip(y0)
        .take(y1 - y0 + 1)
        .for_each(|(y, row)| {
            for (x, o) in row.iter_mut().enumerate().take(x1 + 1).skip(x0) {
                let p = Field::lookup(&f.at, f.gw, f.gh, f.step, x as f32, y as f32);
                // Un nœud qui n'a pas bougé : le pixel reste le même.
                if (p[0] - x as f32).abs() < 1e-3 && (p[1] - y as f32).abs() < 1e-3 {
                    continue;
                }
                *o = source.sample(p[0], p[1]);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un damier de 8 pixels, et un point blanc en (60, 40).
    fn board(w: usize, h: usize) -> RgbImage {
        let mut img = RgbImage::new(w, h);
        for (i, p) in img.data.iter_mut().enumerate() {
            let (x, y) = (i % w, i / w);
            let v = if (x / 8 + y / 8) % 2 == 0 { 0.2 } else { 0.4 };
            *p = [v, v, v];
        }
        img
    }

    fn centroid_of_white(img: &RgbImage) -> (f32, f32, f32) {
        let (mut sx, mut sy, mut n) = (0.0, 0.0, 0.0);
        for (i, p) in img.data.iter().enumerate() {
            if p[1] > 0.6 {
                sx += (i % img.width) as f32;
                sy += (i / img.width) as f32;
                n += 1.0;
            }
        }
        (sx / n, sy / n, n)
    }

    fn with_dot(mut img: RgbImage, cx: usize, cy: usize, r: usize) -> RgbImage {
        let w = img.width;
        for (i, p) in img.data.iter_mut().enumerate() {
            let (dx, dy) = ((i % w) as isize - cx as isize, (i / w) as isize - cy as isize);
            if dx * dx + dy * dy <= (r * r) as isize {
                *p = [1.0, 1.0, 1.0];
            }
        }
        img
    }

    #[test]
    fn push_moves_what_is_under_the_brush() {
        let (w, h) = (200, 120);
        let mut img = with_dot(board(w, h), 60, 60, 6);
        let before = img.clone();
        // Un coup de 60 à 100 px vers la droite, à pleine force.
        let warp = Warp {
            points: vec![(0.3, 0.5), (0.5, 0.5)],
            ..Warp::new(WarpKind::Push, (0.3, 0.5), 0.15, 100.0)
        };
        apply(&mut img, &[warp]);
        let (x, y, n) = centroid_of_white(&img);
        assert!(x > 90.0 && (y - 60.0).abs() < 2.0, "point blanc en {x}, {y}");
        assert!(n > 60.0, "le point reste entier : {n} pixels");
        // Loin du pinceau, rien ne bouge, au bit près.
        for (i, (a, b)) in img.data.iter().zip(&before.data).enumerate() {
            let (px, py) = ((i % w) as f32, (i / w) as f32);
            if px > 160.0 || !(25.0..=95.0).contains(&py) {
                assert_eq!(a, b, "pixel {px}, {py} changé");
            }
        }
    }

    #[test]
    fn bloat_grows_and_pucker_shrinks() {
        let (w, h) = (160, 160);
        let dot = with_dot(board(w, h), 80, 80, 8);
        let (_, _, area) = centroid_of_white(&dot);
        let mut grown = dot.clone();
        apply(&mut grown, &[Warp::new(WarpKind::Bloat, (0.5, 0.5), 0.2, 100.0)]);
        let mut shrunk = dot.clone();
        apply(&mut shrunk, &[Warp::new(WarpKind::Pucker, (0.5, 0.5), 0.2, 100.0)]);
        let (gx, gy, g) = centroid_of_white(&grown);
        let (_, _, s) = centroid_of_white(&shrunk);
        assert!(g > 2.0 * area, "gonflé : {area} → {g} pixels");
        assert!(s < 0.7 * area, "dégonflé : {area} → {s} pixels");
        assert!((gx - 80.0).abs() < 1.0 && (gy - 80.0).abs() < 1.0);
        // Sans force, rien.
        let mut same = dot.clone();
        apply(&mut same, &[Warp::new(WarpKind::Bloat, (0.5, 0.5), 0.2, 0.0)]);
        assert_eq!(same, dot);
    }

    #[test]
    fn the_same_strokes_look_the_same_at_every_size() {
        // L'aperçu et l'export : la même déformation, à l'échelle près.
        let warp = Warp {
            points: vec![(0.4, 0.5), (0.55, 0.45)],
            ..Warp::new(WarpKind::Push, (0.4, 0.5), 0.1, 80.0)
        };
        let small = {
            let mut img = with_dot(board(200, 150), 80, 75, 6);
            apply(&mut img, std::slice::from_ref(&warp));
            centroid_of_white(&img)
        };
        let big = {
            let mut img = with_dot(board(800, 600), 320, 300, 24);
            apply(&mut img, &[warp]);
            centroid_of_white(&img)
        };
        assert!((small.0 * 4.0 - big.0).abs() < 6.0, "{small:?} / {big:?}");
        assert!((small.1 * 4.0 - big.1).abs() < 6.0, "{small:?} / {big:?}");
    }
}
