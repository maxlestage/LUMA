//! La suppression : un disque de la photo rempli par des morceaux de la photo elle‑même
//! (synthèse par patchs, sans IA). Chaque pixel du disque prend la valeur que lui donnent
//! les morceaux de 7 × 7 pixels qui ressemblent le plus, autour de lui, à ce qui entoure
//! le disque ; ces correspondances se cherchent par PatchMatch (essais au hasard,
//! propagation entre voisins), d'abord sur une copie très réduite (la structure : les
//! lignes, les grandes masses), puis de plus en plus fine (le détail, le grain).
//!
//! Tout se passe dans une fenêtre de quelques rayons autour du disque : la source reste
//! proche, et le coût ne dépend que de la taille de la zone.

use luma_core::RgbImage;

/// Le demi‑côté des patchs.
const HALF: isize = 3;
/// Le poids des pixels encore estimés dans l'écart entre deux patchs.
const HOLE_WEIGHT: f32 = 0.35;

/// Un hachage pour des essais au hasard reproductibles.
fn hash(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Une image de travail avec son masque (vrai : à remplir).
struct Level {
    w: usize,
    h: usize,
    px: Vec<[f32; 3]>,
    hole: Vec<bool>,
}

impl Level {
    /// Divisée par deux : moyenne de chaque bloc, trou si l'un de ses pixels l'est.
    fn half(&self) -> Level {
        let (w, h) = (self.w.div_ceil(2), self.h.div_ceil(2));
        let mut px = vec![[0.0; 3]; w * h];
        let mut hole = vec![false; w * h];
        for y in 0..h {
            for x in 0..w {
                let mut s = [0.0; 3];
                let mut n = 0.0;
                for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    let (sx, sy) = (2 * x + dx, 2 * y + dy);
                    if sx < self.w && sy < self.h {
                        let i = sy * self.w + sx;
                        hole[y * w + x] |= self.hole[i];
                        if !self.hole[i] {
                            for (acc, v) in s.iter_mut().zip(self.px[i]) {
                                *acc += v;
                            }
                            n += 1.0;
                        }
                    }
                }
                if n > 0.0 {
                    px[y * w + x] = s.map(|v| v / n);
                }
            }
        }
        Level { w, h, px, hole }
    }

    /// Un patch source centré en (x, y) est utilisable : tout entier dans la fenêtre et
    /// hors du trou.
    fn source_ok(&self, x: isize, y: isize, ok: &[bool]) -> bool {
        x >= HALF
            && y >= HALF
            && x < self.w as isize - HALF
            && y < self.h as isize - HALF
            && ok[y as usize * self.w + x as usize]
    }

    /// L'écart (somme des carrés) entre le patch autour de `t` et celui autour de `s`,
    /// arrêté dès qu'il dépasse `best`.
    fn distance(&self, t: (isize, isize), s: (isize, isize), best: f32) -> f32 {
        let mut d = 0.0;
        for j in -HALF..=HALF {
            let ty = t.1 + j;
            if ty < 0 || ty >= self.h as isize {
                continue;
            }
            for i in -HALF..=HALF {
                let tx = t.0 + i;
                if tx < 0 || tx >= self.w as isize {
                    continue;
                }
                let t = ty as usize * self.w + tx as usize;
                let (a, b) = (self.px[t], self.px[(s.1 + j) as usize * self.w + (s.0 + i) as usize]);
                // Ce qui est connu compte plus que ce qui n'est encore qu'estimé (flou) :
                // la texture du bord guide le remplissage.
                let k = if self.hole[t] { HOLE_WEIGHT } else { 1.0 };
                d += k * ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2));
            }
            if d >= best {
                return d;
            }
        }
        d
    }
}

/// Les centres de patch utilisables comme source (aucun pixel du trou dans le patch).
fn sources(level: &Level) -> Vec<bool> {
    let (w, h) = (level.w, level.h);
    let mut ok = vec![false; w * h];
    for y in HALF as usize..h.saturating_sub(HALF as usize) {
        for x in HALF as usize..w.saturating_sub(HALF as usize) {
            ok[y * w + x] = (-HALF..=HALF).all(|j| {
                (-HALF..=HALF).all(|i| !level.hole[(y as isize + j) as usize * w + (x as isize + i) as usize])
            });
        }
    }
    ok
}

/// Pas un pixel du trou, dans [`hole_index`].
const NONE: u32 = u32::MAX;

/// Pour chaque pixel du niveau, son rang dans `holes` (ou [`NONE`]) : un tableau plein,
/// lu en une instruction, au lieu d'une table de hachage interrogée des dizaines de
/// fois par pixel.
fn hole_index(len: usize, holes: &[usize]) -> Vec<u32> {
    let mut index = vec![NONE; len];
    for (k, &i) in holes.iter().enumerate() {
        index[i] = k as u32;
    }
    index
}

/// Les pixels du trou : chacun la moyenne des pixels que lui donnent les patchs qui le
/// recouvrent (ceux centrés sur un pixel du trou), selon leurs correspondances.
fn vote(level: &mut Level, nnf: &[(isize, isize)], holes: &[usize]) {
    let w = level.w as isize;
    let mut acc = vec![[0.0f32; 4]; holes.len()];
    let index = hole_index(level.w * level.h, holes);
    for (k, &p) in holes.iter().enumerate() {
        let (px, py) = ((p as isize) % w, (p as isize) / w);
        let (sx, sy) = nnf[k];
        for j in -HALF..=HALF {
            for i in -HALF..=HALF {
                let (qx, qy) = (px + i, py + j);
                if qx < 0 || qy < 0 || qx >= w || qy >= level.h as isize {
                    continue;
                }
                let m = index[(qy * w + qx) as usize];
                if m != NONE {
                    let v = level.px[((sy + j) * w + sx + i) as usize];
                    let a = &mut acc[m as usize];
                    for c in 0..3 {
                        a[c] += v[c];
                    }
                    a[3] += 1.0;
                }
            }
        }
    }
    for (k, &p) in holes.iter().enumerate() {
        let a = acc[k];
        if a[3] > 0.0 {
            level.px[p] = [a[0] / a[3], a[1] / a[3], a[2] / a[3]];
        }
    }
}

/// Quelques passes de PatchMatch : pour chaque pixel du trou, sa correspondance
/// s'améliore par celles de ses voisins (décalées d'un pixel) et par des essais au
/// hasard de plus en plus proches.
fn improve(level: &Level, ok: &[bool], holes: &[usize], nnf: &mut [(isize, isize)], pass: u64) {
    let w = level.w as isize;
    let index = hole_index(level.w * level.h, holes);
    let forward = pass.is_multiple_of(2);
    let order: Vec<usize> = if forward {
        (0..holes.len()).collect()
    } else {
        (0..holes.len()).rev().collect()
    };
    let step: isize = if forward { -1 } else { 1 };
    for k in order {
        let p = holes[k] as isize;
        let t = (p % w, p / w);
        let mut best = nnf[k];
        let mut best_d = level.distance(t, best, f32::MAX);
        // Propagation : le voisin déjà traité, décalé d'un pixel.
        for (nx, ny) in [(t.0 + step, t.1), (t.0, t.1 + step)] {
            if nx < 0 || ny < 0 || nx >= w || ny >= level.h as isize {
                continue;
            }
            let m = index[(ny * w + nx) as usize];
            if m != NONE {
                let m = m as usize;
                let c = (nnf[m].0 - (nx - t.0), nnf[m].1 - (ny - t.1));
                if level.source_ok(c.0, c.1, ok) {
                    let d = level.distance(t, c, best_d);
                    if d < best_d {
                        (best, best_d) = (c, d);
                    }
                }
            }
        }
        // Essais au hasard autour de la meilleure, dans un rayon qui se divise par deux.
        let mut radius = level.w.max(level.h) as isize;
        let mut n = 0u64;
        while radius >= 1 {
            let r = hash(pass.wrapping_mul(0x9e37_79b9) ^ (p as u64) << 8 ^ n);
            n += 1;
            let c = (
                best.0 + (r % (2 * radius as u64 + 1)) as isize - radius,
                best.1 + ((r >> 20) % (2 * radius as u64 + 1)) as isize - radius,
            );
            if level.source_ok(c.0, c.1, ok) {
                let d = level.distance(t, c, best_d);
                if d < best_d {
                    (best, best_d) = (c, d);
                }
            }
            radius /= 2;
        }
        nnf[k] = best;
    }
}

/// Une source utilisable au hasard (ou la plus proche du coin s'il n'y en a pas).
fn random_source(level: &Level, ok: &[bool], seed: u64) -> Option<(isize, isize)> {
    for n in 0..256u64 {
        let r = hash(seed ^ n.wrapping_mul(0x5851_f42d));
        let (x, y) = ((r % level.w as u64) as isize, ((r >> 32) % level.h as u64) as isize);
        if level.source_ok(x, y, ok) {
            return Some((x, y));
        }
    }
    (0..level.w * level.h)
        .find(|&i| ok[i])
        .map(|i| ((i % level.w) as isize, (i / level.w) as isize))
}

/// Remplit le disque de centre (`cx`, `cy`) et de rayon `r` (en pixels de `img`) ; le
/// bord se fond sur `feather` (0 à 1 du rayon), à l'opacité `opacity` (0 à 1).
pub fn fill(img: &mut RgbImage, cx: f32, cy: f32, r: f32, feather: f32, opacity: f32) {
    fill_path(img, &[(cx, cy)], r, feather, opacity);
}

/// La distance du point (`x`, `y`) au tracé `path` (un point : sa distance au point).
fn distance_to(path: &[(f32, f32)], x: f32, y: f32) -> f32 {
    if path.len() == 1 {
        return ((x - path[0].0).powi(2) + (y - path[0].1).powi(2)).sqrt();
    }
    path.windows(2)
        .map(|s| {
            let ((ax, ay), (bx, by)) = (s[0], s[1]);
            let (dx, dy) = (bx - ax, by - ay);
            let len2 = dx * dx + dy * dy;
            let t = if len2 > 0.0 {
                (((x - ax) * dx + (y - ay) * dy) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ((x - ax - t * dx).powi(2) + (y - ay - t * dy).powi(2)).sqrt()
        })
        .fold(f32::INFINITY, f32::min)
}

/// Remplit le tracé `path` (des points en pixels de `img`, reliés), épais de `r` de
/// chaque côté : un fil, une rayure, un câble d'un seul geste. Un seul point : un disque.
pub fn fill_path(img: &mut RgbImage, path: &[(f32, f32)], r: f32, feather: f32, opacity: f32) {
    let (w, h) = (img.width, img.height);
    if w < 8 || h < 8 || r < 0.5 || opacity <= 0.0 || path.is_empty() {
        return;
    }
    // La fenêtre : trois rayons autour du tracé, de quoi trouver de la matière.
    let reach = 3.0 * r + 12.0;
    let (lo_x, hi_x) = path
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), p| (a.min(p.0), b.max(p.0)));
    let (lo_y, hi_y) = path
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), p| (a.min(p.1), b.max(p.1)));
    let x0 = (lo_x - reach).floor().max(0.0) as usize;
    let y0 = (lo_y - reach).floor().max(0.0) as usize;
    let x1 = ((hi_x + reach).ceil().max(0.0) as usize).min(w);
    let y1 = ((hi_y + reach).ceil().max(0.0) as usize).min(h);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let (ww, wh) = (x1 - x0, y1 - y0);
    if ww < 16 || wh < 16 {
        return;
    }
    // Le trou couvre la zone et un pixel de plus (le bord de l'objet).
    let mut base = Level {
        w: ww,
        h: wh,
        px: Vec::with_capacity(ww * wh),
        hole: Vec::with_capacity(ww * wh),
    };
    for y in y0..y1 {
        for x in x0..x1 {
            base.px.push(img.data[y * w + x]);
            let d = distance_to(path, x as f32 + 0.5, y as f32 + 0.5);
            base.hole.push(d < r + 1.0);
        }
    }
    // La pyramide, jusqu'à un trou d'une dizaine de pixels de rayon.
    let mut levels = vec![base];
    let mut radius = r;
    while radius > 10.0 && levels.last().is_some_and(|l| l.w >= 48 && l.h >= 48) {
        let next = levels.last().expect("au moins une").half();
        levels.push(next);
        radius /= 2.0;
    }
    let mut nnf: Vec<(isize, isize)> = Vec::new();
    let (mut prev_w, mut prev_h) = (0usize, 0usize);
    let mut prev_holes: Vec<usize> = Vec::new();
    let top = levels.len() - 1;
    for li in (0..levels.len()).rev() {
        let level = &mut levels[li];
        let ok = sources(level);
        if !ok.iter().any(|v| *v) {
            return;
        }
        let holes: Vec<usize> = (0..level.w * level.h).filter(|&i| level.hole[i]).collect();
        let seed = li as u64 * 0x1000_0000;
        // Les correspondances : au plus grossier au hasard, ensuite celles du niveau
        // au‑dessus, doublées.
        let coarse = hole_index(prev_w * prev_h, &prev_holes);
        nnf = holes
            .iter()
            .map(|&p| {
                let (x, y) = ((p % level.w) as isize, (p / level.w) as isize);
                let parent = (y as usize / 2) * prev_w + x as usize / 2;
                coarse
                    .get(parent)
                    .filter(|&&m| m != NONE)
                    .map(|&m| nnf[m as usize])
                    .map(|(sx, sy)| (sx * 2 + x % 2, sy * 2 + y % 2))
                    .filter(|&(sx, sy)| level.source_ok(sx, sy, &ok))
                    .or_else(|| random_source(level, &ok, seed ^ p as u64))
                    .expect("une source existe")
            })
            .collect();
        if li == top {
            // Le trou commence à la couleur moyenne de son entourage.
            let around: Vec<[f32; 3]> = (0..level.w * level.h)
                .filter(|&i| !level.hole[i])
                .map(|i| level.px[i])
                .collect();
            let n = around.len().max(1) as f32;
            let mean = around
                .iter()
                .fold([0.0; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]])
                .map(|v| v / n);
            for &p in &holes {
                level.px[p] = mean;
            }
        }
        vote(level, &nnf, &holes);
        let passes = if li == top { 8 } else { 4 };
        for pass in 0..passes {
            improve(level, &ok, &holes, &mut nnf, seed + pass);
            vote(level, &nnf, &holes);
        }
        if li == 0 {
            // À la taille finale, chaque pixel prend celui du centre de sa
            // correspondance : la moyenne des patchs qui se recouvrent lisserait le grain.
            let w = level.w as isize;
            for (k, &p) in holes.iter().enumerate() {
                let (sx, sy) = nnf[k];
                level.px[p] = level.px[(sy * w + sx) as usize];
            }
        }
        // Le niveau suivant (plus fin) part de cette image : ses trous prennent la
        // valeur du niveau courant (agrandie), puis les correspondances la précisent.
        if li > 0 {
            let (cw, coarse_px) = (level.w, level.px.clone());
            let fine = &mut levels[li - 1];
            for i in 0..fine.w * fine.h {
                if fine.hole[i] {
                    let (x, y) = (i % fine.w, i / fine.w);
                    fine.px[i] = coarse_px[(y / 2) * cw + x / 2];
                }
            }
        }
        prev_w = levels[li].w;
        prev_h = levels[li].h;
        prev_holes = holes;
    }
    // Le résultat, fondu sur le bord de la zone.
    let filled = &levels[0];
    let inner = r * (1.0 - feather.clamp(0.0, 1.0));
    for y in y0..y1 {
        for x in x0..x1 {
            let k = (y - y0) * ww + (x - x0);
            let d = distance_to(path, x as f32 + 0.5, y as f32 + 0.5);
            if d >= r {
                continue;
            }
            let t = if d <= inner {
                1.0
            } else {
                let u = ((r - d) / (r - inner)).clamp(0.0, 1.0);
                u * u * (3.0 - 2.0 * u)
            };
            let a = t * opacity.clamp(0.0, 1.0);
            let v = filled.px[k];
            let p = &mut img.data[y * w + x];
            for c in 0..3 {
                p[c] += (v[c] - p[c]) * a;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L'empreinte des pixels de trois suppressions (un grand disque, un petit, un
    /// tracé) : celle d'avant l'index plein ; une modification qui change le résultat la
    /// change, une accélération non.
    #[test]
    fn removal_gives_the_same_pixels_as_before() {
        let (w, h) = (520, 360);
        let img = RgbImage::from_fn(w, h, |x, y| {
            let s = if (x / 7 + y / 11) % 2 == 0 { 0.6 } else { 0.25 };
            [s, s * 0.8 + (x % 13) as f32 * 0.01, 0.3 + (y % 17) as f32 * 0.01]
        });
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for (path, r) in [
            (vec![(260.0, 180.0)], 30.0),
            (vec![(100.0, 60.0)], 8.0),
            (vec![(40.0, 40.0), (200.0, 120.0), (480.0, 300.0)], 5.0),
        ] {
            let mut a = img.clone();
            fill_path(&mut a, &path, r, 0.3, 1.0);
            for c in a.data.iter().flatten() {
                hash = (hash ^ u64::from(c.to_bits())).wrapping_mul(0x100_0000_01b3);
            }
        }
        assert_eq!(hash, 0xf6f4_e2fc_4c9a_b4ad);
    }

    /// Des rayures et un dégradé, et la même image avec un « objet » rouge dessus.
    fn scene() -> (RgbImage, RgbImage) {
        let clean = RgbImage::from_fn(200, 160, |x, y| {
            let stripe = if (x / 6) % 2 == 0 { 0.55 } else { 0.25 };
            [stripe, stripe * 0.9 + y as f32 * 0.0005, 0.3]
        });
        let mut spoiled = clean.clone();
        for y in 0..160 {
            for x in 0..200 {
                if (x as f32 - 100.0).powi(2) + (y as f32 - 80.0).powi(2) < 18.0f32.powi(2) {
                    spoiled.data[y * 200 + x] = [0.9, 0.05, 0.05];
                }
            }
        }
        (clean, spoiled)
    }

    fn error(a: &RgbImage, b: &RgbImage) -> f32 {
        let mut s = 0.0;
        let mut n = 0.0;
        for y in 55..105 {
            for x in 75..125 {
                let (p, q) = (a.data[y * 200 + x], b.data[y * 200 + x]);
                s += (0..3).map(|c| (p[c] - q[c]).powi(2)).sum::<f32>();
                n += 1.0;
            }
        }
        (s / n).sqrt()
    }

    #[test]
    fn an_object_is_removed_and_the_stripes_go_on() {
        let (clean, spoiled) = scene();
        let mut img = spoiled.clone();
        fill(&mut img, 100.0, 80.0, 20.0, 0.1, 1.0);
        // Plus rien de rouge.
        let red = img.data.iter().filter(|p| p[0] > 0.8 && p[1] < 0.2).count();
        assert_eq!(red, 0);
        // Les rayures continuent à travers la zone : bien plus proche de la photo
        // d'origine qu'un simple aplat de la couleur moyenne.
        let mut flat = spoiled.clone();
        let mean = [0.4, 0.36, 0.3];
        for y in 0..160 {
            for x in 0..200 {
                if (x as f32 - 100.0).powi(2) + (y as f32 - 80.0).powi(2) < 20.0f32.powi(2) {
                    flat.data[y * 200 + x] = mean;
                }
            }
        }
        let (e, ef) = (error(&img, &clean), error(&flat, &clean));
        eprintln!("écart à la photo d'origine : rempli {e:.4}, aplat {ef:.4}");
        assert!(e < ef * 0.5, "{e} / {ef}");
        // Hors du disque, rien ne bouge.
        let outside = img.data.iter().zip(&spoiled.data).enumerate().filter(|(i, (a, b))| {
            let (x, y) = ((i % 200) as f32, (i / 200) as f32);
            (x + 0.5 - 100.0).powi(2) + (y + 0.5 - 80.0).powi(2) >= 400.0 && a != b
        });
        assert_eq!(outside.count(), 0);
    }

    #[test]
    fn the_same_input_gives_the_same_fill() {
        let (_, spoiled) = scene();
        let (mut a, mut b) = (spoiled.clone(), spoiled);
        fill(&mut a, 100.0, 80.0, 20.0, 0.5, 1.0);
        fill(&mut b, 100.0, 80.0, 20.0, 0.5, 1.0);
        assert_eq!(a, b);
    }
}
