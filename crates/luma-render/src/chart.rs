//! La mesure des couleurs : la charte ColorChecker, CIE Lab et ΔE 2000. Pour les tests
//! colorimétriques et pour `luma colorchecker` (la fidélité d'un vrai boîtier, mesurée
//! sur la photo d'une vraie charte), jusqu'au profil du boîtier : la matrice qui ramène
//! ses couleurs sur celles de la charte ([`fit_profile`]).

use luma_color::{self as lc, Vec3};

/// La charte ColorChecker classique, en sRGB 8 bits (valeurs de référence publiées).
pub const CHART: [[u8; 3]; 24] = [
    [115, 82, 68],
    [194, 150, 130],
    [98, 122, 157],
    [87, 108, 67],
    [133, 128, 177],
    [103, 189, 170],
    [214, 126, 44],
    [80, 91, 166],
    [193, 90, 99],
    [94, 60, 108],
    [157, 188, 64],
    [224, 163, 46],
    [56, 61, 150],
    [70, 148, 73],
    [175, 54, 60],
    [231, 199, 31],
    [187, 86, 149],
    [8, 133, 161],
    [243, 243, 242],
    [200, 200, 200],
    [160, 160, 160],
    [122, 122, 121],
    [85, 85, 85],
    [52, 52, 52],
];

pub fn xyz_of_srgb(p: [u8; 3]) -> Vec3 {
    let lin = p.map(|v| lc::srgb_decode(f32::from(v) / 255.0));
    lc::apply(&lc::rgb_to_xyz(&lc::SRGB), lin)
}

/// CIE Lab sous D65.
pub fn lab(xyz: Vec3) -> Vec3 {
    let w = lc::xy_to_xyz(lc::D65);
    let f = |t: f32| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(xyz[0] / w[0]), f(xyz[1] / w[1]), f(xyz[2] / w[2]));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// ΔE 2000 (Sharma, Wu, Dalal, 2005).
pub fn delta_e2000(a: Vec3, b: Vec3) -> f32 {
    let (l1, a1, b1) = (f64::from(a[0]), f64::from(a[1]), f64::from(a[2]));
    let (l2, a2, b2) = (f64::from(b[0]), f64::from(b[1]), f64::from(b[2]));
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let cm = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cm.powi(7) / (cm.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = (a1 * (1.0 + g), a2 * (1.0 + g));
    let (c1p, c2p) = ((a1p * a1p + b1 * b1).sqrt(), (a2p * a2p + b2 * b2).sqrt());
    let hue = |b: f64, a: f64| {
        let h = b.atan2(a).to_degrees();
        if h < 0.0 { h + 360.0 } else { h }
    };
    let (h1p, h2p) = (hue(b1, a1p), hue(b2, a2p));
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let mut dh = h2p - h1p;
    if c1p * c2p == 0.0 {
        dh = 0.0;
    } else if dh > 180.0 {
        dh -= 360.0;
    } else if dh < -180.0 {
        dh += 360.0;
    }
    let dhh = 2.0 * (c1p * c2p).sqrt() * (dh.to_radians() / 2.0).sin();
    let lm = (l1 + l2) / 2.0;
    let cmp = (c1p + c2p) / 2.0;
    let hm = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (hm - 30.0).to_radians().cos()
        + 0.24 * (2.0 * hm).to_radians().cos()
        + 0.32 * (3.0 * hm + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hm - 63.0).to_radians().cos();
    let sl = 1.0 + 0.015 * (lm - 50.0).powi(2) / (20.0 + (lm - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cmp;
    let sh = 1.0 + 0.015 * cmp * t;
    let dtheta = 30.0 * (-((hm - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cmp.powi(7) / (cmp.powi(7) + 25f64.powi(7))).sqrt();
    let rt = -rc * (2.0 * dtheta).to_radians().sin();
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dhh / sh).powi(2) + rt * (dc / sc) * (dhh / sh)).sqrt() as f32
}

/// Les 24 plages d'une photo de charte recadrée au plus juste (6 × 4 plages, plage
/// brune en haut à gauche), développée en `img` (espace de travail linéaire) : la
/// moyenne du tiers central de chaque plage, l'exposition ramenée sur la plage grise
/// n° 22 (comme on le fait sur une vraie charte).
pub fn patches(img: &luma_core::RgbImage) -> [Vec3; 24] {
    let to_xyz = lc::rgb_to_xyz(&lc::WORKING);
    let (pw, ph) = (img.width as f32 / 6.0, img.height as f32 / 4.0);
    let mean = |k: usize| {
        let (cx, cy) = ((k % 6) as f32 * pw, (k / 6) as f32 * ph);
        let mut s = [0.0f32; 3];
        let mut n = 0.0f32;
        for y in (cy + ph / 3.0) as usize..(cy + 2.0 * ph / 3.0) as usize {
            for x in (cx + pw / 3.0) as usize..(cx + 2.0 * pw / 3.0) as usize {
                let p = img.data[y.min(img.height - 1) * img.width + x.min(img.width - 1)];
                for c in 0..3 {
                    s[c] += p[c];
                }
                n += 1.0;
            }
        }
        s.map(|v| v / n.max(1.0))
    };
    let grey = lc::apply(&to_xyz, mean(21))[1].max(1e-6);
    let k = xyz_of_srgb(CHART[21])[1] / grey;
    std::array::from_fn(|i| mean(i).map(|v| v * k))
}

/// Le ΔE 2000 de chaque plage (espace de travail linéaire) à sa référence.
fn errors(measured: &[Vec3; 24], m: &lc::Mat3) -> [f32; 24] {
    let to_xyz = lc::rgb_to_xyz(&lc::WORKING);
    std::array::from_fn(|i| {
        let got = lc::apply(&to_xyz, lc::apply(m, measured[i]));
        delta_e2000(lab(xyz_of_srgb(CHART[i])), lab(got))
    })
}

/// La mesure d'une photo de charte recadrée au plus juste (voir [`patches`]) : le ΔE
/// 2000 de chaque plage.
pub fn measure_chart(img: &luma_core::RgbImage) -> [f32; 24] {
    errors(&patches(img), &lc::diag([1.0; 3]))
}

/// Le profil d'un boîtier, mesuré sur une charte.
#[derive(Clone, Debug)]
pub struct Profile {
    /// La correction, dans l'espace de travail linéaire (lignes : rouge, vert, bleu de
    /// sortie) ; chaque ligne fait 1 : le blanc et la balance restent ceux de la photo.
    pub matrix: lc::Mat3,
    /// Le ΔE 2000 de chaque plage, avant et après.
    pub before: [f32; 24],
    pub after: [f32; 24],
}

/// Le profil qui ramène les couleurs de `img` (une charte, voir [`patches`]) sur celles
/// de la charte : une matrice 3 × 3 aux lignes de somme 1, d'abord par les moindres
/// carrés (en lumière linéaire), puis affinée pour le plus petit ΔE 2000 moyen
/// (descente coordonnée, pas divisé par deux quand rien n'améliore).
pub fn fit_profile(img: &luma_core::RgbImage) -> Profile {
    let measured = patches(img);
    let from_xyz = lc::inverse(&lc::rgb_to_xyz(&lc::WORKING)).expect("primaires");
    let target: [Vec3; 24] = std::array::from_fn(|i| lc::apply(&from_xyz, xyz_of_srgb(CHART[i])));
    // Chaque ligne s'écrit [a, b, 1 − a − b] : deux inconnues, des équations normales 2 × 2.
    let mut rows = [[0.0f32; 3]; 3];
    for (r, row) in rows.iter_mut().enumerate() {
        let (mut s11, mut s12, mut s22, mut t1, mut t2) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
        for (x, y) in measured.iter().zip(&target) {
            let (u, v) = (f64::from(x[0] - x[2]), f64::from(x[1] - x[2]));
            let rhs = f64::from(y[r] - x[2]);
            s11 += u * u;
            s12 += u * v;
            s22 += v * v;
            t1 += u * rhs;
            t2 += v * rhs;
        }
        let det = s11 * s22 - s12 * s12;
        let (a, b) = if det.abs() > 1e-12 {
            ((t1 * s22 - t2 * s12) / det, (s11 * t2 - s12 * t1) / det)
        } else {
            (if r == 0 { 1.0 } else { 0.0 }, if r == 1 { 1.0 } else { 0.0 })
        };
        *row = [a as f32, b as f32, (1.0 - a - b) as f32];
    }
    let build =
        |p: &[f32; 6]| -> lc::Mat3 { std::array::from_fn(|r| [p[2 * r], p[2 * r + 1], 1.0 - p[2 * r] - p[2 * r + 1]]) };
    let cost = |p: &[f32; 6]| errors(&measured, &build(p)).iter().sum::<f32>() / 24.0;
    let mut p = [rows[0][0], rows[0][1], rows[1][0], rows[1][1], rows[2][0], rows[2][1]];
    let mut best = cost(&p);
    let mut step = 0.05f32;
    for _ in 0..2000 {
        if step < 1e-5 {
            break;
        }
        let mut better = false;
        for k in 0..6 {
            for sign in [1.0, -1.0] {
                let mut q = p;
                q[k] += sign * step;
                let c = cost(&q);
                if c < best {
                    (p, best, better) = (q, c, true);
                }
            }
        }
        if !better {
            step /= 2.0;
        }
    }
    let matrix = build(&p);
    Profile {
        matrix,
        before: errors(&measured, &lc::diag([1.0; 3])),
        after: errors(&measured, &matrix),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une charte « photographiée » par un boîtier dont les couleurs dérivent (une
    /// matrice aux lignes de somme 1 : le gris reste gris).
    fn shot(drift: &lc::Mat3) -> luma_core::RgbImage {
        let from_xyz = lc::inverse(&lc::rgb_to_xyz(&lc::WORKING)).unwrap();
        luma_core::RgbImage::from_fn(240, 160, |x, y| {
            let k = (y / 40) * 6 + x / 40;
            lc::apply(drift, lc::apply(&from_xyz, xyz_of_srgb(CHART[k])))
        })
    }

    #[test]
    fn a_profile_brings_the_chart_back() {
        let drift = [[0.86, 0.18, -0.04], [0.06, 0.84, 0.10], [-0.03, 0.15, 0.88]];
        let p = fit_profile(&shot(&drift));
        let mean = |e: &[f32; 24]| e.iter().sum::<f32>() / 24.0;
        eprintln!("ΔE moyen : {:.2} → {:.2}", mean(&p.before), mean(&p.after));
        assert!(mean(&p.before) > 3.0);
        assert!(mean(&p.after) < 0.3, "{:?}", p.after);
        // Le blanc reste blanc : chaque ligne fait 1.
        for row in p.matrix {
            assert!((row.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        }
        // Sur une charte déjà juste, la correction est presque l'identité.
        let exact = fit_profile(&shot(&lc::diag([1.0; 3])));
        assert!(mean(&exact.after) < 0.1);
        for (r, row) in exact.matrix.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                assert!((v - if r == c { 1.0 } else { 0.0 }).abs() < 0.01, "{:?}", exact.matrix);
            }
        }
    }
}
