//! Courbes de tonalité.
//!
//! - [`Filmic`] : le rendu de base, qui passe de la lumière de la scène (linéaire,
//!   sans limite haute) à l'écran (0 à 1). Forme de Lottes : un gris moyen à 18 % fixe,
//!   une pente réglée par le Contraste, une épaule réglée par les Blancs.
//! - [`Lut`] : une table 1D de 4096 entrées, où se composent la courbe paramétrique
//!   et les courbes à points du panneau Courbe des tonalités.

use luma_project::ToneCurve;

/// Le rendu filmique de base.
#[derive(Clone, Copy, Debug)]
pub struct Filmic {
    a: f32,
    d: f32,
    b: f32,
    c: f32,
    /// Plancher des noirs (curseur Noirs) : > 0 bouche, < 0 relève.
    black: f32,
}

impl Filmic {
    /// Gris moyen de la scène et sa place à l'écran (linéaire : ≈ 0,475 en sRGB).
    pub const MID_IN: f32 = 0.18;
    pub const MID_OUT: f32 = 0.19;

    /// `contrast`, `whites` et `blacks` : −100 à +100.
    pub fn new(contrast: f32, whites: f32, blacks: f32) -> Self {
        Self::with_look(contrast, whites, blacks, 1.0)
    }

    /// Avec le multiplicateur de contraste du profil de rendu.
    pub fn with_look(contrast: f32, whites: f32, blacks: f32, look: f32) -> Self {
        let a = 1.55 * look * (1.0 + contrast / 100.0 * 0.45);
        let d = 0.96;
        // La luminance de scène qui devient le blanc : le blanc du capteur, avec de la
        // marge pour une épaule douce. Les Blancs la rapprochent ou l'éloignent.
        let hdr_max = 1.2 * (-whites / 100.0 * 1.4).exp2();
        let (mi, mo) = (Self::MID_IN, Self::MID_OUT);
        let (ma, hm) = (mi.powf(a), hdr_max.powf(a));
        let (mad, hmd) = (ma.powf(d), hm.powf(d));
        let b = (-ma + hm * mo) / ((hmd - mad) * mo);
        let c = (hmd * ma - hm * mad * mo) / ((hmd - mad) * mo);
        Self {
            a,
            d,
            b,
            c,
            black: -blacks / 100.0 * 0.025,
        }
    }

    #[inline]
    pub fn apply(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        let xa = x.powf(self.a);
        let y = (xa / (xa.powf(self.d) * self.b + self.c)).min(1.0);
        if self.black > 0.0 {
            // Noirs bouchés : on retire un plancher, réparti sur les ombres.
            ((y - self.black) / (1.0 - self.black)).max(0.0)
        } else {
            // Noirs relevés : effet surtout dans les basses lumières.
            let lift = -self.black * (1.0 - y).powi(3);
            (y + lift).min(1.0)
        }
    }
}

/// La courbe filmique en table : l'entrée (0 à 4) est indexée par sa racine
/// quatrième, ce qui donne une résolution fine dans les ombres, là où elle compte.
pub struct FilmicTable(Vec<f32>);

impl FilmicTable {
    const SIZE: usize = 8192;
    const MAX: f32 = 4.0;

    #[inline]
    pub fn get(&self, x: f32) -> f32 {
        let u = (x.max(0.0) / Self::MAX).sqrt().sqrt() * Self::SIZE as f32;
        if u >= Self::SIZE as f32 {
            return 1.0;
        }
        let i = u as usize;
        let t = u - i as f32;
        self.0[i] + (self.0[i + 1] - self.0[i]) * t
    }
}

impl Filmic {
    pub fn table(&self) -> FilmicTable {
        FilmicTable(
            (0..=FilmicTable::SIZE)
                .map(|i| {
                    let u = i as f32 / FilmicTable::SIZE as f32;
                    self.apply(u.powi(4) * FilmicTable::MAX)
                })
                .collect(),
        )
    }
}

/// Une table 1D sur [0, 1], interpolée linéairement.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut(Vec<f32>);

impl Lut {
    pub const SIZE: usize = 4096;

    pub fn identity() -> Self {
        Self::from_fn(|x| x)
    }

    pub fn from_fn(f: impl Fn(f32) -> f32) -> Self {
        Self((0..=Self::SIZE).map(|i| f(i as f32 / Self::SIZE as f32)).collect())
    }

    #[inline]
    pub fn get(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0) * Self::SIZE as f32;
        let i = (x as usize).min(Self::SIZE - 1);
        let t = x - i as f32;
        self.0[i] + (self.0[i + 1] - self.0[i]) * t
    }

    /// `self` puis `next`.
    pub fn then(&self, next: &Lut) -> Lut {
        Lut(self.0.iter().map(|&v| next.get(v)).collect())
    }

    pub fn is_identity(&self) -> bool {
        self.0
            .iter()
            .enumerate()
            .all(|(i, &v)| (v - i as f32 / Self::SIZE as f32).abs() < 1e-5)
    }
}

/// Interpolation cubique monotone (Fritsch–Carlson) : la courbe passe par chaque point
/// sans osciller ni s'inverser entre eux.
pub fn spline(points: &[(f32, f32)]) -> Lut {
    let pts: Vec<(f32, f32)> = points.iter().map(|&(x, y)| (x / 255.0, y / 255.0)).collect();
    if pts.len() < 2 {
        return Lut::identity();
    }
    let n = pts.len();
    let delta: Vec<f32> = (0..n - 1)
        .map(|i| (pts[i + 1].1 - pts[i].1) / (pts[i + 1].0 - pts[i].0).max(1e-6))
        .collect();
    let mut m = vec![0.0f32; n];
    m[0] = delta[0];
    m[n - 1] = delta[n - 2];
    for i in 1..n - 1 {
        m[i] = if delta[i - 1] * delta[i] <= 0.0 {
            0.0
        } else {
            (delta[i - 1] + delta[i]) / 2.0
        };
    }
    for i in 0..n - 1 {
        if delta[i] == 0.0 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
            continue;
        }
        let (a, b) = (m[i] / delta[i], m[i + 1] / delta[i]);
        let s = a * a + b * b;
        if s > 9.0 {
            let t = 3.0 / s.sqrt();
            m[i] = t * a * delta[i];
            m[i + 1] = t * b * delta[i];
        }
    }
    Lut::from_fn(|x| {
        if x <= pts[0].0 {
            return pts[0].1;
        }
        if x >= pts[n - 1].0 {
            return pts[n - 1].1;
        }
        let i = pts.windows(2).position(|w| x < w[1].0).unwrap_or(n - 2);
        let (x0, y0, x1, y1) = (pts[i].0, pts[i].1, pts[i + 1].0, pts[i + 1].1);
        let h = x1 - x0;
        let t = (x - x0) / h;
        let (t2, t3) = (t * t, t * t * t);
        let y = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
            + (t3 - 2.0 * t2 + t) * h * m[i]
            + (-2.0 * t3 + 3.0 * t2) * y1
            + (t3 - t2) * h * m[i + 1];
        y.clamp(0.0, 1.0)
    })
}

/// La courbe paramétrique : quatre régions (ombres, tons sombres, tons clairs, hautes
/// lumières) séparées par trois curseurs. Chaque réglage bombe la courbe dans sa
/// région, en débordant sur la moitié des régions voisines.
pub fn parametric(c: &ToneCurve) -> Lut {
    let (s1, s2, s3) = (
        c.shadow_split / 100.0,
        c.midtone_split / 100.0,
        c.highlight_split / 100.0,
    );
    let regions = [
        (c.shadows, 0.0, (s1 + s2) / 2.0),
        (c.darks, s1 / 2.0, (s2 + s3) / 2.0),
        (c.lights, (s1 + s2) / 2.0, (s3 + 1.0) / 2.0),
        (c.highlights, (s2 + s3) / 2.0, 1.0),
    ];
    if regions.iter().all(|r| r.0 == 0.0) {
        return Lut::identity();
    }
    let lut = Lut::from_fn(|x| {
        let mut y = x;
        for &(amount, lo, hi) in &regions {
            if amount != 0.0 && x > lo && x < hi {
                let t = (x - lo) / (hi - lo);
                let bump = (std::f32::consts::PI * t).sin().powi(2);
                y += amount / 100.0 * 0.16 * bump;
            }
        }
        y.clamp(0.0, 1.0)
    });
    // La courbe reste croissante.
    let mut values = lut.0;
    for i in 1..values.len() {
        values[i] = values[i].max(values[i - 1]);
    }
    Lut(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filmic_keeps_mid_gray_and_reaches_white() {
        let f = Filmic::new(0.0, 0.0, 0.0);
        assert!((f.apply(0.18) - Filmic::MID_OUT).abs() < 1e-3);
        assert!(f.apply(1.0) > 0.9 && f.apply(1.0) < 1.0, "{}", f.apply(1.0));
        assert_eq!(f.apply(1.25), 1.0);
        assert_eq!(f.apply(0.0), 0.0);
        // Croissante.
        let mut last = 0.0;
        for i in 1..200 {
            let y = f.apply(i as f32 / 100.0);
            assert!(y >= last);
            last = y;
        }
    }

    #[test]
    fn the_table_matches_the_curve() {
        let f = Filmic::new(20.0, -10.0, 5.0);
        let t = f.table();
        for x in [0.0005, 0.01, 0.05, 0.18, 0.5, 1.0, 1.3, 3.0] {
            assert!(
                (t.get(x) - f.apply(x)).abs() < 2e-3,
                "{x}: {} vs {}",
                t.get(x),
                f.apply(x)
            );
        }
        assert_eq!(t.get(10.0), 1.0);
    }

    #[test]
    fn contrast_pivots_on_mid_gray() {
        let (lo, hi) = (Filmic::new(-100.0, 0.0, 0.0), Filmic::new(100.0, 0.0, 0.0));
        assert!((hi.apply(0.18) - lo.apply(0.18)).abs() < 1e-3);
        assert!(hi.apply(0.05) < lo.apply(0.05));
        assert!(hi.apply(0.6) > lo.apply(0.6));
    }

    #[test]
    fn whites_and_blacks_move_the_ends() {
        let base = Filmic::new(0.0, 0.0, 0.0);
        assert!(Filmic::new(0.0, 50.0, 0.0).apply(0.8) > base.apply(0.8));
        assert!(Filmic::new(0.0, 0.0, -50.0).apply(0.01) < base.apply(0.01));
        assert!(Filmic::new(0.0, 0.0, 50.0).apply(0.01) > base.apply(0.01));
    }

    #[test]
    fn splines_pass_through_their_points_without_overshoot() {
        let lut = spline(&[(0.0, 0.0), (64.0, 40.0), (192.0, 220.0), (255.0, 255.0)]);
        assert!((lut.get(64.0 / 255.0) - 40.0 / 255.0).abs() < 1e-3);
        assert!((lut.get(192.0 / 255.0) - 220.0 / 255.0).abs() < 1e-3);
        let mut last = -1.0;
        for i in 0..=100 {
            let y = lut.get(i as f32 / 100.0);
            assert!(y >= last && (0.0..=1.0).contains(&y));
            last = y;
        }
        assert!(spline(&[(0.0, 0.0), (255.0, 255.0)]).is_identity());
    }

    #[test]
    fn parametric_regions() {
        let mut c = ToneCurve::default();
        assert!(parametric(&c).is_identity());
        c.shadows = 100.0;
        let lut = parametric(&c);
        assert!(lut.get(0.15) > 0.15);
        assert!((lut.get(0.9) - 0.9).abs() < 1e-6, "les hautes lumières ne bougent pas");
    }
}
