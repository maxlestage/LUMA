//! La mesure des couleurs : la charte ColorChecker, CIE Lab et ΔE 2000. Pour les tests
//! colorimétriques et pour `luma colorchecker` (la fidélité d'un vrai boîtier, mesurée
//! sur la photo d'une vraie charte).

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

/// La mesure d'une photo de charte recadrée au plus juste (6 × 4 plages, plage brune en
/// haut à gauche), développée en `img` (espace de travail linéaire). L'exposition est
/// ramenée sur la plage grise n° 22 (comme on le fait sur une vraie charte) ; rend le
/// ΔE 2000 de chaque plage, mesuré au centre (le tiers central de la plage).
pub fn measure_chart(img: &luma_core::RgbImage) -> [f32; 24] {
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
    let want = xyz_of_srgb(CHART[21])[1];
    let k = want / grey;
    std::array::from_fn(|i| {
        let got = lc::apply(&to_xyz, mean(i)).map(|v| v * k);
        delta_e2000(lab(xyz_of_srgb(CHART[i])), lab(got))
    })
}
