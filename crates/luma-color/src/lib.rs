//! Colorimétrie de LÜMA.
//!
//! - matrices 3×3 et espaces RGB définis par leurs primaires (sRGB, Display P3,
//!   Rec. 2020, ProPhoto) ;
//! - adaptation chromatique de Bradford ;
//! - température et teinte d'un blanc, sur la même échelle que la spécification DNG
//!   (méthode de Robertson, teinte en Δuv × −3000) : un « 5500 K, +10 » veut dire la
//!   même chose dans LÜMA que dans les autres logiciels qui lisent les DNG ;
//! - OKLab / OKLCh (Björn Ottosson), un espace perceptuel où travaillent la TSL et
//!   l'étalonnage des couleurs ;
//! - fonctions de transfert des sorties.
//!
//! L'espace de travail du moteur est le Rec. 2020 linéaire (blanc D65) : large, aux
//! primaires réelles, sans les couleurs imaginaires du ProPhoto.

pub mod icc;

pub type Mat3 = [[f32; 3]; 3];
pub type Vec3 = [f32; 3];

pub const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

pub fn mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

#[inline]
pub fn apply(m: &Mat3, v: Vec3) -> Vec3 {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

pub fn inverse(m: &Mat3) -> Option<Mat3> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-12 {
        return None;
    }
    let d = 1.0 / det;
    Some([
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * d,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * d,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * d,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * d,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * d,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * d,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * d,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * d,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * d,
        ],
    ])
}

pub fn diag(v: Vec3) -> Mat3 {
    [[v[0], 0.0, 0.0], [0.0, v[1], 0.0], [0.0, 0.0, v[2]]]
}

// ----- Blancs et espaces ------------------------------------------------------------------

/// Une chromaticité CIE xy.
pub type Xy = (f32, f32);

pub const D65: Xy = (0.3127, 0.3290);
pub const D50: Xy = (0.3457, 0.3585);
/// L'illuminant A (tungstène, 2856 K).
pub const A: Xy = (0.44757, 0.40745);

/// XYZ d'une chromaticité, à luminance Y = 1.
pub fn xy_to_xyz((x, y): Xy) -> Vec3 {
    [x / y, 1.0, (1.0 - x - y) / y]
}

pub fn xyz_to_xy(v: Vec3) -> Xy {
    let s = v[0] + v[1] + v[2];
    if s <= 0.0 { D65 } else { (v[0] / s, v[1] / s) }
}

/// Un espace RGB : ses primaires et son blanc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Primaries {
    pub r: Xy,
    pub g: Xy,
    pub b: Xy,
    pub white: Xy,
}

pub const SRGB: Primaries = Primaries {
    r: (0.64, 0.33),
    g: (0.30, 0.60),
    b: (0.15, 0.06),
    white: D65,
};
pub const DISPLAY_P3: Primaries = Primaries {
    r: (0.680, 0.320),
    g: (0.265, 0.690),
    b: (0.150, 0.060),
    white: D65,
};
pub const REC2020: Primaries = Primaries {
    r: (0.708, 0.292),
    g: (0.170, 0.797),
    b: (0.131, 0.046),
    white: D65,
};
pub const PROPHOTO: Primaries = Primaries {
    r: (0.7347, 0.2653),
    g: (0.1596, 0.8404),
    b: (0.0366, 0.0001),
    white: D50,
};

/// La matrice RGB → XYZ d'un espace (son blanc a Y = 1).
pub fn rgb_to_xyz(p: &Primaries) -> Mat3 {
    let (r, g, b) = (xy_to_xyz(p.r), xy_to_xyz(p.g), xy_to_xyz(p.b));
    let m = [[r[0], g[0], b[0]], [r[1], g[1], b[1]], [r[2], g[2], b[2]]];
    let s = apply(&inverse(&m).expect("primaires invalides"), xy_to_xyz(p.white));
    mul(&m, &diag(s))
}

/// L'adaptation chromatique de Bradford, d'un blanc à un autre (XYZ → XYZ).
pub fn bradford(from: Xy, to: Xy) -> Mat3 {
    const B: Mat3 = [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ];
    let (src, dst) = (apply(&B, xy_to_xyz(from)), apply(&B, xy_to_xyz(to)));
    let scale = diag([dst[0] / src[0], dst[1] / src[1], dst[2] / src[2]]);
    mul(&inverse(&B).expect("Bradford"), &mul(&scale, &B))
}

/// La matrice de conversion entre deux espaces RGB linéaires (avec adaptation de blanc).
pub fn rgb_to_rgb(from: &Primaries, to: &Primaries) -> Mat3 {
    let to_xyz = rgb_to_xyz(from);
    let adapt = bradford(from.white, to.white);
    let from_xyz = inverse(&rgb_to_xyz(to)).expect("espace invalide");
    mul(&from_xyz, &mul(&adapt, &to_xyz))
}

/// L'espace de travail du moteur.
pub const WORKING: Primaries = REC2020;

/// Luminance Y d'une couleur de l'espace de travail.
#[inline]
pub fn luminance(c: Vec3) -> f32 {
    // Deuxième ligne de rgb_to_xyz(REC2020).
    0.2627 * c[0] + 0.6780 * c[1] + 0.0593 * c[2]
}

// ----- Température et teinte (échelle DNG) --------------------------------------------------

/// Isothermes de Robertson (Wyszecki & Stiles) : (micro‑réciproque de degré, u, v, pente).
const ROBERTSON: [(f64, f64, f64, f64); 31] = [
    (0.0, 0.18006, 0.26352, -0.24341),
    (10.0, 0.18066, 0.26589, -0.25479),
    (20.0, 0.18133, 0.26846, -0.26876),
    (30.0, 0.18208, 0.27119, -0.28539),
    (40.0, 0.18293, 0.27407, -0.30470),
    (50.0, 0.18388, 0.27709, -0.32675),
    (60.0, 0.18494, 0.28021, -0.35156),
    (70.0, 0.18611, 0.28342, -0.37915),
    (80.0, 0.18740, 0.28668, -0.40955),
    (90.0, 0.18880, 0.28997, -0.44278),
    (100.0, 0.19032, 0.29326, -0.47888),
    (125.0, 0.19462, 0.30141, -0.58204),
    (150.0, 0.19962, 0.30921, -0.70471),
    (175.0, 0.20525, 0.31647, -0.84901),
    (200.0, 0.21142, 0.32312, -1.0182),
    (225.0, 0.21807, 0.32909, -1.2168),
    (250.0, 0.22511, 0.33439, -1.4512),
    (275.0, 0.23247, 0.33904, -1.7298),
    (300.0, 0.24010, 0.34308, -2.0637),
    (325.0, 0.24702, 0.34655, -2.4681),
    (350.0, 0.25591, 0.34951, -2.9641),
    (375.0, 0.26400, 0.35200, -3.5814),
    (400.0, 0.27218, 0.35407, -4.3633),
    (425.0, 0.28039, 0.35577, -5.3762),
    (450.0, 0.28863, 0.35714, -6.7262),
    (475.0, 0.29685, 0.35823, -8.5955),
    (500.0, 0.30505, 0.35907, -11.324),
    (525.0, 0.31320, 0.35968, -15.628),
    (550.0, 0.32129, 0.36011, -23.325),
    (575.0, 0.32931, 0.36038, -40.770),
    (600.0, 0.33724, 0.36051, -116.45),
];

/// Échelle de la teinte : Δuv × −3000, comme la spécification DNG.
const TINT_SCALE: f64 = -3000.0;

/// Un blanc exprimé en température (kelvins) et teinte (vert − / magenta +).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TempTint {
    pub temperature: f32,
    pub tint: f32,
}

fn unit(dv: f64) -> (f64, f64) {
    let len = (1.0 + dv * dv).sqrt();
    (1.0 / len, dv / len)
}

/// Température et teinte d'une chromaticité.
pub fn xy_to_temp_tint((x, y): Xy) -> TempTint {
    let (x, y) = (f64::from(x), f64::from(y));
    let denom = 1.5 - x + 6.0 * y;
    let (u, v) = (2.0 * x / denom, 3.0 * y / denom);
    let (mut last_dt, mut last_du, mut last_dv) = (0.0, 0.0, 0.0);
    for i in 1..ROBERTSON.len() {
        let (du, dv) = unit(ROBERTSON[i].3);
        let (uu, vv) = (u - ROBERTSON[i].1, v - ROBERTSON[i].2);
        let mut dt = -uu * dv + vv * du;
        if dt <= 0.0 || i == ROBERTSON.len() - 1 {
            if dt > 0.0 {
                dt = 0.0;
            }
            dt = -dt;
            let f = if i == 1 { 0.0 } else { dt / (last_dt + dt) };
            let r = ROBERTSON[i - 1].0 * f + ROBERTSON[i].0 * (1.0 - f);
            let temperature = 1.0e6 / r.max(1e-6);
            let uu = u - (ROBERTSON[i - 1].1 * f + ROBERTSON[i].1 * (1.0 - f));
            let vv = v - (ROBERTSON[i - 1].2 * f + ROBERTSON[i].2 * (1.0 - f));
            let (mut du, mut dv) = (du * (1.0 - f) + last_du * f, dv * (1.0 - f) + last_dv * f);
            let len = (du * du + dv * dv).sqrt();
            du /= len;
            dv /= len;
            let tint = (uu * du + vv * dv) * TINT_SCALE;
            return TempTint {
                temperature: temperature as f32,
                tint: tint as f32,
            };
        }
        last_dt = dt;
        last_du = du;
        last_dv = dv;
    }
    unreachable!()
}

/// La chromaticité d'une température et d'une teinte.
pub fn temp_tint_to_xy(t: TempTint) -> Xy {
    let r = 1.0e6 / f64::from(t.temperature.clamp(1000.0, 1.0e6));
    let offset = f64::from(t.tint) / TINT_SCALE;
    let n = ROBERTSON.len();
    for i in 0..n - 1 {
        if r < ROBERTSON[i + 1].0 || i == n - 2 {
            let f = (ROBERTSON[i + 1].0 - r) / (ROBERTSON[i + 1].0 - ROBERTSON[i].0);
            let mut u = ROBERTSON[i].1 * f + ROBERTSON[i + 1].1 * (1.0 - f);
            let mut v = ROBERTSON[i].2 * f + ROBERTSON[i + 1].2 * (1.0 - f);
            let (u1, v1) = unit(ROBERTSON[i].3);
            let (u2, v2) = unit(ROBERTSON[i + 1].3);
            let (mut u3, mut v3) = (u1 * f + u2 * (1.0 - f), v1 * f + v2 * (1.0 - f));
            let len = (u3 * u3 + v3 * v3).sqrt();
            u3 /= len;
            v3 /= len;
            u += u3 * offset;
            v += v3 * offset;
            let d = u - 4.0 * v + 2.0;
            return ((1.5 * u / d) as f32, (v / d) as f32);
        }
    }
    unreachable!()
}

// ----- OKLab -----------------------------------------------------------------------------

/// `atan2` approché (erreur < 3e‑4 rad, soit 0,02°), pour les teintes calculées à chaque
/// pixel.
#[inline]
pub fn fast_atan2(y: f32, x: f32) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }
    let (ax, ay) = (x.abs(), y.abs());
    let a = ax.min(ay) / ax.max(ay);
    let s = a * a;
    let mut r = ((-0.046_496_475 * s + 0.159_314_22) * s - 0.327_622_76) * s * a + a;
    if ay > ax {
        r = FRAC_PI_2 - r;
    }
    if x < 0.0 {
        r = PI - r;
    }
    if y < 0.0 { -r } else { r }
}

/// Racine cubique rapide (estimation par les bits du flottant, puis deux itérations de
/// Newton) : erreur relative < 1e‑6, environ 4 fois plus rapide que `f32::cbrt`.
#[inline]
pub fn fast_cbrt(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    let mut y = f32::from_bits(x.to_bits() / 3 + 709_921_077);
    y = (2.0 * y + x / (y * y)) * (1.0 / 3.0);
    y = (2.0 * y + x / (y * y)) * (1.0 / 3.0);
    y
}

/// OKLab depuis XYZ (D65), matrices de Björn Ottosson.
const XYZ_TO_LMS: Mat3 = [
    [0.818_933, 0.361_866_74, -0.128_859_71],
    [0.032_984_544, 0.929_311_9, 0.036_145_64],
    [0.048_200_3, 0.264_366_27, 0.633_851_7],
];
const LMS_TO_LAB: Mat3 = [
    [0.210_454_26, 0.793_617_8, -0.004_072_047],
    [1.977_998_5, -2.428_592_2, 0.450_593_7],
    [0.025_904_037, 0.782_771_77, -0.808_675_77],
];

/// Les matrices entre l'espace de travail et OKLab, calculées une fois.
#[derive(Clone, Copy, Debug)]
pub struct OkLab {
    to_lms: Mat3,
    from_lms: Mat3,
    lab_to_lms: Mat3,
    /// Le léger écart du blanc (a, b par unité de L) dû à l'arrondi des matrices,
    /// retiré pour que tout gris soit exactement neutre.
    white: (f32, f32),
}

impl OkLab {
    pub fn for_working() -> Self {
        let to_lms = mul(&XYZ_TO_LMS, &rgb_to_xyz(&WORKING));
        let w = apply(&LMS_TO_LAB, apply(&to_lms, [1.0, 1.0, 1.0]).map(f32::cbrt));
        Self {
            to_lms,
            from_lms: inverse(&to_lms).expect("OKLab"),
            lab_to_lms: inverse(&LMS_TO_LAB).expect("OKLab"),
            white: (w[1] / w[0], w[2] / w[0]),
        }
    }

    #[inline]
    pub fn from_rgb(&self, c: Vec3) -> Vec3 {
        let lms = apply(&self.to_lms, c);
        let lab = apply(&LMS_TO_LAB, [fast_cbrt(lms[0]), fast_cbrt(lms[1]), fast_cbrt(lms[2])]);
        [lab[0], lab[1] - lab[0] * self.white.0, lab[2] - lab[0] * self.white.1]
    }

    #[inline]
    pub fn to_rgb(&self, lab: Vec3) -> Vec3 {
        let lab = [lab[0], lab[1] + lab[0] * self.white.0, lab[2] + lab[0] * self.white.1];
        let l = apply(&self.lab_to_lms, lab);
        apply(
            &self.from_lms,
            [l[0] * l[0] * l[0], l[1] * l[1] * l[1], l[2] * l[2] * l[2]],
        )
    }
}

// ----- Fonctions de transfert --------------------------------------------------------------

#[inline]
pub fn srgb_encode(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        12.92 * v.max(0.0)
    } else {
        1.055 * v.min(1.0).powf(1.0 / 2.4) - 0.055
    }
}

#[inline]
pub fn srgb_decode(v: f32) -> f32 {
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Une table de 4096 entrées pour encoder vite en sRGB (sorties 8 et 16 bits).
pub struct EncodeLut(Vec<f32>);

impl EncodeLut {
    pub const SIZE: usize = 4096;

    pub fn new(f: impl Fn(f32) -> f32) -> Self {
        Self((0..=Self::SIZE).map(|i| f(i as f32 / Self::SIZE as f32)).collect())
    }

    #[inline]
    pub fn get(&self, v: f32) -> f32 {
        let x = v.clamp(0.0, 1.0) * Self::SIZE as f32;
        let i = (x as usize).min(Self::SIZE - 1);
        let t = x - i as f32;
        self.0[i] + (self.0[i + 1] - self.0[i]) * t
    }
}

/// Les espaces de sortie.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputSpace {
    #[default]
    Srgb,
    DisplayP3,
    Rec2020,
    ProPhoto,
}

impl OutputSpace {
    pub fn primaries(self) -> Primaries {
        match self {
            OutputSpace::Srgb => SRGB,
            OutputSpace::DisplayP3 => DISPLAY_P3,
            OutputSpace::Rec2020 => REC2020,
            OutputSpace::ProPhoto => PROPHOTO,
        }
    }

    /// La fonction de transfert de l'espace (Display P3 reprend celle du sRGB).
    pub fn encode(self, v: f32) -> f32 {
        match self {
            OutputSpace::Srgb | OutputSpace::DisplayP3 => srgb_encode(v),
            OutputSpace::Rec2020 => {
                let v = v.clamp(0.0, 1.0);
                if v < 0.018_053_97 {
                    4.5 * v
                } else {
                    1.099_296_8 * v.powf(0.45) - 0.099_296_8
                }
            }
            OutputSpace::ProPhoto => {
                let v = v.clamp(0.0, 1.0);
                if v < 1.0 / 512.0 { 16.0 * v } else { v.powf(1.0 / 1.8) }
            }
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "srgb" => Some(OutputSpace::Srgb),
            "p3" | "display-p3" | "displayp3" => Some(OutputSpace::DisplayP3),
            "rec2020" | "bt2020" => Some(OutputSpace::Rec2020),
            "prophoto" => Some(OutputSpace::ProPhoto),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn srgb_matrix_matches_the_standard() {
        let m = rgb_to_xyz(&SRGB);
        assert!(close(m[0][0], 0.4124, 1e-3) && close(m[1][1], 0.7152, 1e-3) && close(m[2][2], 0.9505, 1e-3));
        // La luminance codée en dur correspond bien à l'espace de travail.
        let w = rgb_to_xyz(&WORKING);
        assert!(close(w[1][0], 0.2627, 1e-3) && close(w[1][1], 0.6780, 1e-3) && close(w[1][2], 0.0593, 1e-3));
    }

    #[test]
    fn matrices_invert() {
        let m = rgb_to_xyz(&DISPLAY_P3);
        let i = mul(&m, &inverse(&m).unwrap());
        for (r, row) in i.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                assert!(close(*v, if r == c { 1.0 } else { 0.0 }, 1e-5));
            }
        }
    }

    #[test]
    fn white_maps_to_white_between_spaces() {
        for to in [SRGB, DISPLAY_P3, PROPHOTO] {
            let w = apply(&rgb_to_rgb(&WORKING, &to), [1.0, 1.0, 1.0]);
            assert!(w.iter().all(|v| close(*v, 1.0, 1e-4)), "{w:?}");
        }
    }

    #[test]
    fn temperature_matches_known_illuminants() {
        let d65 = xy_to_temp_tint(D65);
        assert!(close(d65.temperature, 6504.0, 15.0), "{d65:?}");
        let a = xy_to_temp_tint(A);
        assert!(close(a.temperature, 2856.0, 10.0) && close(a.tint, 0.0, 2.0), "{a:?}");
    }

    #[test]
    fn temperature_round_trips() {
        for temperature in [2000.0, 3200.0, 5000.0, 5500.0, 7500.0, 12000.0] {
            for tint in [-50.0, 0.0, 30.0] {
                let back = xy_to_temp_tint(temp_tint_to_xy(TempTint { temperature, tint }));
                assert!(
                    close(back.temperature, temperature, temperature * 0.002),
                    "{temperature} → {back:?}"
                );
                assert!(close(back.tint, tint, 0.2), "{tint} → {back:?}");
            }
        }
    }

    #[test]
    fn oklab_round_trips_and_white_is_neutral() {
        let ok = OkLab::for_working();
        let white = ok.from_rgb([1.0, 1.0, 1.0]);
        assert!(
            close(white[0], 1.0, 1e-3) && white[1].abs() < 1e-6 && white[2].abs() < 1e-6,
            "{white:?}"
        );
        let gray = ok.from_rgb([0.2, 0.2, 0.2]);
        assert!(gray[1].abs() < 1e-6 && gray[2].abs() < 1e-6, "{gray:?}");
        let c = [0.3, 0.1, 0.05];
        let back = ok.to_rgb(ok.from_rgb(c));
        assert!(back.iter().zip(c).all(|(a, b)| close(*a, b, 1e-4)), "{back:?}");
    }

    #[test]
    fn fast_atan2_is_accurate() {
        for i in 0..360 {
            let t = (i as f32).to_radians();
            let (y, x) = (t.sin() * 0.3, t.cos() * 0.3);
            assert!((fast_atan2(y, x) - y.atan2(x)).abs() < 3e-4, "{i}°");
        }
    }

    #[test]
    fn fast_cbrt_is_accurate() {
        for x in [1e-8f32, 1e-4, 0.018, 0.5, 1.0, 3.7, 250.0] {
            assert!((fast_cbrt(x) - x.cbrt()).abs() <= x.cbrt() * 2e-6, "{x}");
        }
        assert_eq!(fast_cbrt(-1.0), 0.0);
    }

    #[test]
    fn srgb_transfer_round_trips() {
        for v in [0.0, 0.001, 0.18, 0.5, 1.0] {
            assert!(close(srgb_decode(srgb_encode(v)), v, 1e-5));
        }
        let lut = EncodeLut::new(srgb_encode);
        assert!(close(lut.get(0.18), srgb_encode(0.18), 1e-4));
    }
}
