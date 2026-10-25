//! La marque de LÜMA, dessinée par le code : nette à toutes les tailles, la même partout
//! (icônes de l'app sur Mac, iPhone et iPad, interface, site).
//!
//! - Le logo : un anneau de lumière en croissant (or et orange à gauche, rose et violet
//!   en bas, bleu à droite), qui rayonne dans le noir, et « LÜMA » en capitales fines et
//!   espacées.
//! - Le fond de l'app, « l'éclipse » : un noir profond éclairé par les lueurs du logo,
//!   un immense anneau en filigrane, un grain très fin.
//!
//! Les images sont rendues en RGBA 8 bits (alpha non prémultiplié).

/// Les couleurs de l'anneau, dans le sens des aiguilles d'une montre depuis midi :
/// (angle en degrés, couleur sRGB).
const STOPS: [(f32, [u8; 3]); 11] = [
    (0.0, [0x7a, 0x62, 0xff]),
    (40.0, [0x2f, 0x5c, 0xff]),
    (90.0, [0x4c, 0xc6, 0xff]),
    (135.0, [0x5a, 0x86, 0xff]),
    (165.0, [0x9d, 0x58, 0xff]),
    (195.0, [0xe8, 0x5c, 0xb0]),
    (228.0, [0xff, 0x78, 0x38]),
    (268.0, [0xff, 0xc4, 0x66]),
    (305.0, [0xf6, 0xcf, 0xae]),
    (335.0, [0xb0, 0x8c, 0xf0]),
    (360.0, [0x7a, 0x62, 0xff]),
];

/// La couleur de l'anneau à `degrees` (0 = midi, sens horaire), en sRGB 0 à 1.
pub fn ring_color(degrees: f32) -> [f32; 3] {
    let a = degrees.rem_euclid(360.0);
    let k = STOPS.iter().position(|s| s.0 >= a).unwrap_or(STOPS.len() - 1).max(1);
    let (a0, c0) = STOPS[k - 1];
    let (a1, c1) = STOPS[k];
    let t = ((a - a0) / (a1 - a0).max(1e-6)).clamp(0.0, 1.0);
    // Interpolation en lumière linéaire : pas de gris entre deux teintes vives.
    std::array::from_fn(|i| {
        let lin = |v: u8| (f32::from(v) / 255.0).powf(2.2);
        (lin(c0[i]) * (1.0 - t) + lin(c1[i]) * t).powf(1.0 / 2.2)
    })
}

/// L'éclat de l'anneau selon l'angle : le côté or et le côté bleu brillent le plus.
fn ring_energy(degrees: f32) -> f32 {
    let r = degrees.to_radians();
    0.78 + 0.22 * (2.0 * (r - 0.35)).sin().abs()
}

/// Le fond d'une image de logo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    /// Transparent : le logo seul, posé sur l'interface.
    None,
    /// Le carré arrondi noir des icônes de macOS (marges de la grille d'Apple).
    MacIcon,
    /// Tout le carré, noir : iPhone, iPad (le système arrondit), site.
    Full,
}

/// Le logo dans un carré de `size` pixels, avec ou sans « LÜMA ».
pub fn logo(size: usize, backdrop: Backdrop, wordmark: bool) -> Vec<u8> {
    let n = size.max(1);
    let samples = if n <= 64 { 4 } else { 2 };
    let mut out = vec![0u8; n * n * 4];
    let px = 1.0 / n as f32;
    for y in 0..n {
        for x in 0..n {
            let mut acc = [0.0f32; 4];
            for sy in 0..samples {
                for sx in 0..samples {
                    let u = (x as f32 + (sx as f32 + 0.5) / samples as f32) * px - 0.5;
                    let v = (y as f32 + (sy as f32 + 0.5) / samples as f32) * px - 0.5;
                    let c = logo_at(u, v, backdrop, wordmark, px);
                    for k in 0..4 {
                        acc[k] += c[k];
                    }
                }
            }
            let s = (samples * samples) as f32;
            let a = acc[3] / s;
            let o = (y * n + x) * 4;
            for k in 0..3 {
                let v = if a > 0.0 { acc[k] / s / a } else { 0.0 };
                out[o + k] = (encode(v) * 255.0 + 0.5) as u8;
            }
            out[o + 3] = (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
    out
}

fn encode(linear: f32) -> f32 {
    linear.clamp(0.0, 1.0).powf(1.0 / 2.2)
}

fn linear(srgb: f32) -> f32 {
    srgb.clamp(0.0, 1.0).powf(2.2)
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// La lumière de l'anneau (linéaire, prémultipliée par sa couverture) au point (u, v),
/// pour un anneau de centre (0, 0) et de rayon extérieur `r`.
fn ring_light(u: f32, v: f32, r: f32, aa: f32) -> ([f32; 3], f32) {
    let d = (u * u + v * v).sqrt();
    let degrees = u.atan2(-v).to_degrees();
    let col = ring_color(degrees).map(linear);
    let e = ring_energy(degrees);
    // Le croissant : le disque sombre intérieur est décalé vers le haut à droite.
    let (ox, oy) = (0.10 * r, -0.09 * r);
    let inner_r = 0.82 * r;
    let di = ((u - ox).powi(2) + (v - oy).powi(2)).sqrt();
    let outside_outer = smoothstep(r - aa, r + aa, d);
    let inside_inner = 1.0 - smoothstep(inner_r - aa, inner_r + aa, di);
    let body = (1.0 - outside_outer) * (1.0 - inside_inner);
    // Plus clair contre le bord intérieur, comme un tube de lumière.
    // Où l'on est dans l'épaisseur, localement : 0 au bord intérieur, 1 au bord
    // extérieur ; le tube de lumière est plus clair un peu en dedans de son milieu.
    let frac = ((di - inner_r) / ((r - d) + (di - inner_r)).max(1e-6)).clamp(0.0, 1.0);
    let core = 0.5 + 1.05 * (-((frac - 0.4) / 0.38).powi(2)).exp();
    let mut light = col.map(|c| c * body * core * e);
    // Un cœur presque blanc dans la partie la plus épaisse (en bas à gauche).
    let thick = smoothstep(0.0, 1.0, (-(u * 0.75 + v * 0.66) / r).max(0.0));
    let white = body * (-((frac - 0.42) / 0.22).powi(2)).exp() * thick * 0.4;
    for c in &mut light {
        *c += white;
    }
    // Le halo : dehors, la lumière se diffuse dans le noir ; dedans, à peine.
    let out_glow = (-((d - r).max(0.0) / (0.15 * r)).powi(2)).exp() * smoothstep(0.86 * r, r, d);
    let in_glow = (-((inner_r - di).max(0.0) / (0.12 * r)).powi(2)).exp() * inside_inner;
    let glow = 0.42 * out_glow * (1.0 - body) + 0.12 * in_glow;
    for k in 0..3 {
        light[k] += col[k] * glow * e;
    }
    (light, body)
}

/// « LÜMA » : les traits des lettres (segments), dans une boîte de hauteur 1 (capitale),
/// et la largeur totale. Des capitales géométriques, fines et très espacées.
/// Les traits (segments), les points (du tréma) et la largeur de « LÜMA ».
type Strokes = (Vec<([f32; 2], [f32; 2])>, Vec<[f32; 2]>, f32);

fn wordmark_strokes() -> Strokes {
    let mut segs = Vec::new();
    let mut dots = Vec::new();
    let gap = 0.95;
    let mut x = 0.0;
    // L
    segs.push(([x, 0.0], [x, 1.0]));
    segs.push(([x, 1.0], [x + 0.6, 1.0]));
    x += 0.6 + gap;
    // Ü
    let (w, r) = (0.72, 0.36);
    segs.push(([x, 0.0], [x, 1.0 - r]));
    segs.push(([x + w, 0.0], [x + w, 1.0 - r]));
    let n = 18;
    for k in 0..n {
        let a0 = std::f32::consts::PI * k as f32 / n as f32;
        let a1 = std::f32::consts::PI * (k + 1) as f32 / n as f32;
        let p = |a: f32| [x + r - r * a.cos(), 1.0 - r + r * a.sin()];
        segs.push((p(a0), p(a1)));
    }
    dots.push([x + 0.2, -0.2]);
    dots.push([x + w - 0.2, -0.2]);
    x += w + gap;
    // M
    let w = 0.86;
    segs.push(([x, 1.0], [x, 0.0]));
    segs.push(([x, 0.0], [x + w / 2.0, 0.72]));
    segs.push(([x + w / 2.0, 0.72], [x + w, 0.0]));
    segs.push(([x + w, 0.0], [x + w, 1.0]));
    x += w + gap;
    // A
    let w = 0.84;
    segs.push(([x, 1.0], [x + w / 2.0, 0.0]));
    segs.push(([x + w / 2.0, 0.0], [x + w, 1.0]));
    segs.push(([x + w * 0.22, 0.66], [x + w * 0.78, 0.66]));
    x += w;
    (segs, dots, x)
}

/// La couverture de « LÜMA » au point (u, v) : capitales de hauteur `cap`, centrées en
/// (0, `cy`).
fn wordmark_at(u: f32, v: f32, cap: f32, cy: f32, aa: f32) -> f32 {
    let (segs, dots, width) = wordmark_strokes();
    let scale = cap;
    let (x0, y0) = (-width * scale / 2.0, cy - scale / 2.0);
    let (lx, ly) = ((u - x0) / scale, (v - y0) / scale);
    if lx < -0.3 || lx > width + 0.3 || ly < -0.45 || ly > 1.3 {
        return 0.0;
    }
    let half = 0.055;
    let mut d = f32::MAX;
    for (a, b) in &segs {
        let (pax, pay) = (lx - a[0], ly - a[1]);
        let (bax, bay) = (b[0] - a[0], b[1] - a[1]);
        let h = ((pax * bax + pay * bay) / (bax * bax + bay * bay).max(1e-9)).clamp(0.0, 1.0);
        d = d.min(((pax - bax * h).powi(2) + (pay - bay * h).powi(2)).sqrt() - half);
    }
    for p in &dots {
        d = d.min(((lx - p[0]).powi(2) + (ly - p[1]).powi(2)).sqrt() - 0.065);
    }
    let a = aa / scale;
    1.0 - smoothstep(-a, a, d)
}

/// Le logo au point (u, v) ∈ [−0,5 ; 0,5]² : couleur linéaire prémultipliée et alpha.
fn logo_at(u: f32, v: f32, backdrop: Backdrop, wordmark: bool, px: f32) -> [f32; 4] {
    let aa = px * 0.75;
    // Le fond.
    let (mut c, mut alpha) = match backdrop {
        Backdrop::None => ([0.0; 3], 0.0),
        Backdrop::Full => ([0.0002, 0.0002, 0.0003], 1.0),
        Backdrop::MacIcon => {
            // 824 px sur 1024, rayon ≈ 22,5 % (la grille des icônes de macOS).
            let half = 0.4023;
            let r = 0.185;
            let qx = (u.abs() - (half - r)).max(0.0);
            let qy = (v.abs() - (half - r)).max(0.0);
            let inside = 1.0 - smoothstep(r - aa, r + aa, (qx * qx + qy * qy).sqrt());
            ([0.0002 * inside, 0.0002 * inside, 0.0003 * inside], inside)
        }
    };
    let (ring_r, ring_cy, cap, word_cy) = match (backdrop, wordmark) {
        (Backdrop::MacIcon, true) => (0.2, -0.07, 0.078, 0.245),
        (Backdrop::MacIcon, false) => (0.25, 0.0, 0.0, 0.0),
        (_, true) => (0.25, -0.09, 0.095, 0.30),
        (_, false) => (0.33, 0.0, 0.0, 0.0),
    };
    let (light, body) = ring_light(u, v - ring_cy, ring_r, aa);
    let limit = if backdrop == Backdrop::MacIcon { alpha } else { 1.0 };
    for k in 0..3 {
        c[k] += light[k] * limit;
    }
    if backdrop == Backdrop::None {
        // Sans fond : l'anneau couvre, le halo éclaire en transparence.
        let glow = light.iter().copied().fold(0.0, f32::max).min(1.0);
        alpha = body.max(glow * 0.85);
    }
    if wordmark {
        let w = wordmark_at(u, v, cap, word_cy, aa) * 0.93;
        for v in &mut c {
            *v = *v * (1.0 - w) + w;
        }
        alpha = alpha.max(w);
    }
    [c[0], c[1], c[2], alpha]
}

/// Le fond de l'app, « l'éclipse » : `width` × `height` pixels RGBA (opaques). Il est
/// fait pour être étiré : rien n'y est net, sauf le grain, très fin.
pub fn backdrop(width: usize, height: usize) -> Vec<u8> {
    let (w, h) = (width.max(1), height.max(1));
    let mut out = vec![0u8; w * h * 4];
    let aspect = w as f32 / h as f32;
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5) / h as f32; // en hauteurs d'image
            let v = (y as f32 + 0.5) / h as f32;
            let mut c = [0.0003f32, 0.0003, 0.0005];
            // Les lueurs du logo : or en bas à gauche, bleu en haut à droite, un peu de
            // violet en bas au milieu.
            let blob = |cx: f32, cy: f32, r: f32| (-(((u - cx).powi(2) + (v - cy).powi(2)) / (r * r))).exp();
            let warm = blob(-0.05, 1.08, 0.55);
            let cool = blob(aspect + 0.05, -0.12, 0.70);
            let violet = blob(aspect * 0.55, 1.25, 0.55);
            let add = |c: &mut [f32; 3], col: [f32; 3], k: f32| {
                for i in 0..3 {
                    c[i] += linear(col[i]) * k;
                }
            };
            add(&mut c, ring_color(268.0), warm * 0.055);
            add(&mut c, ring_color(90.0), cool * 0.05);
            add(&mut c, ring_color(185.0), violet * 0.03);
            // L'anneau en filigrane : immense, à droite, à peine plus clair que le noir.
            let (cx, cy, r) = (aspect * 0.70, 0.52, 0.62);
            let d = ((u - cx).powi(2) + (v - cy).powi(2)).sqrt();
            let degrees = (u - cx).atan2(-(v - cy)).to_degrees();
            let ring = (-((d - r) / 0.014).powi(2)).exp() * 0.05 + (-((d - r) / 0.09).powi(2)).exp() * 0.012;
            add(&mut c, ring_color(degrees), ring);
            // Le grain : un bruit fixe, sous le seuil du visible, qui évite les marches.
            let n = hash(x as u32, y as u32) - 0.5;
            let o = (y * w + x) * 4;
            for i in 0..3 {
                out[o + i] = ((encode(c[i]) * 255.0 + n * 1.6).round()).clamp(0.0, 255.0) as u8;
            }
            out[o + 3] = 255;
        }
    }
    out
}

fn hash(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 16;
    (h & 0xffff) as f32 / 65535.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_is_warm_on_the_left_and_blue_on_the_right() {
        let left = ring_color(270.0);
        let right = ring_color(90.0);
        assert!(left[0] > left[2] + 0.4, "{left:?}");
        assert!(right[2] > right[0] + 0.4, "{right:?}");
        // Le tour est continu.
        let (a, b) = (ring_color(359.9), ring_color(0.0));
        assert!((0..3).all(|k| (a[k] - b[k]).abs() < 0.01));
    }

    #[test]
    fn the_logo_glows_in_its_ring_and_stays_dark_in_its_center() {
        let n = 128;
        let img = logo(n, Backdrop::Full, false);
        let px = |x: usize, y: usize| {
            let o = (y * n + x) * 4;
            [img[o], img[o + 1], img[o + 2], img[o + 3]]
        };
        let center = px(n / 2, n / 2);
        assert!(center[0] < 40 && center[2] < 60, "{center:?}");
        // Sur l'anneau, à gauche : or ; à droite : bleu.
        let left = px(n / 2 - (0.30 * n as f32) as usize, n / 2);
        let right = px(n / 2 + (0.315 * n as f32) as usize, n / 2);
        assert!(left[0] > 180 && left[0] > left[2], "{left:?}");
        assert!(right[2] > 180 && right[2] > right[0], "{right:?}");
        // L'icône de Mac est transparente dans les coins.
        let mac = logo(n, Backdrop::MacIcon, true);
        assert_eq!(mac[3], 0);
        // « LÜMA » est blanc sous l'anneau.
        let word = logo(256, Backdrop::Full, true);
        let lit = (0..256 * 256)
            .filter(|i| i / 256 > 170 && word[i * 4] > 200 && word[i * 4 + 2] > 200)
            .count();
        assert!(lit > 300, "{lit}");
    }

    #[test]
    fn the_backdrop_is_dark_with_a_warm_and_a_cool_side() {
        let (w, h) = (160, 100);
        let b = backdrop(w, h);
        let px = |x: usize, y: usize| {
            let o = (y * w + x) * 4;
            [b[o] as i32, b[o + 1] as i32, b[o + 2] as i32]
        };
        let (warm, cool, mid) = (px(4, h - 4), px(w - 4, 4), px(w / 3, h / 3));
        assert!(warm[0] > warm[2] && cool[2] > cool[0], "{warm:?} {cool:?}");
        assert!(mid.iter().all(|&v| v < 70), "{mid:?}");
    }
}
