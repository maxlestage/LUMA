//! La géométrie de l'alignement : homographies (8 degrés de liberté, pour les
//! panoramas et les photos à main levée), similitudes (rotation, échelle et
//! translation, pour le ciel étoilé), et l'estimation robuste par RANSAC, qui ignore
//! les mauvaises correspondances.

/// Un point (x, y), en pixels.
pub type Point = (f64, f64);

/// Une homographie : `p' ~ H · (x, y, 1)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Homography(pub [[f64; 3]; 3]);

impl Homography {
    pub const IDENTITY: Homography = Homography([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    /// Une similitude : rotation `angle` (radians), échelle `scale`, puis translation.
    pub fn similarity(scale: f64, angle: f64, tx: f64, ty: f64) -> Homography {
        let (s, c) = angle.sin_cos();
        Homography([[scale * c, -scale * s, tx], [scale * s, scale * c, ty], [0.0, 0.0, 1.0]])
    }

    pub fn translation(tx: f64, ty: f64) -> Homography {
        Homography([[1.0, 0.0, tx], [0.0, 1.0, ty], [0.0, 0.0, 1.0]])
    }

    pub fn scaling(s: f64) -> Homography {
        Homography([[s, 0.0, 0.0], [0.0, s, 0.0], [0.0, 0.0, 1.0]])
    }

    /// Le point transformé, `None` s'il part à l'infini ou derrière la caméra.
    #[inline]
    pub fn apply(&self, (x, y): Point) -> Option<Point> {
        let m = &self.0;
        let w = m[2][0] * x + m[2][1] * y + m[2][2];
        if w <= 1e-12 {
            return None;
        }
        Some((
            (m[0][0] * x + m[0][1] * y + m[0][2]) / w,
            (m[1][0] * x + m[1][1] * y + m[1][2]) / w,
        ))
    }

    /// Le vecteur homogène transformé (sans division).
    #[inline]
    pub fn apply3(&self, v: [f64; 3]) -> [f64; 3] {
        let m = &self.0;
        [
            m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
            m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
            m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
        ]
    }

    /// `self` puis `next`.
    pub fn then(&self, next: &Homography) -> Homography {
        let (a, b) = (&next.0, &self.0);
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        Homography(m).normalized()
    }

    pub fn inverse(&self) -> Option<Homography> {
        let m = &self.0;
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        if det.abs() < 1e-15 {
            return None;
        }
        let inv = [
            [
                (m[1][1] * m[2][2] - m[1][2] * m[2][1]) / det,
                (m[0][2] * m[2][1] - m[0][1] * m[2][2]) / det,
                (m[0][1] * m[1][2] - m[0][2] * m[1][1]) / det,
            ],
            [
                (m[1][2] * m[2][0] - m[1][0] * m[2][2]) / det,
                (m[0][0] * m[2][2] - m[0][2] * m[2][0]) / det,
                (m[0][2] * m[1][0] - m[0][0] * m[1][2]) / det,
            ],
            [
                (m[1][0] * m[2][1] - m[1][1] * m[2][0]) / det,
                (m[0][1] * m[2][0] - m[0][0] * m[2][1]) / det,
                (m[0][0] * m[1][1] - m[0][1] * m[1][0]) / det,
            ],
        ];
        Some(Homography(inv).normalized())
    }

    fn normalized(self) -> Homography {
        let k = self.0[2][2];
        if k.abs() < 1e-15 {
            return self;
        }
        Homography(self.0.map(|r| r.map(|v| v / k)))
    }

    /// La même transformation pour des images `s` fois plus grandes.
    pub fn rescaled(&self, s: f64) -> Homography {
        Homography::scaling(1.0 / s).then(self).then(&Homography::scaling(s))
    }
}

/// Résout `a · x = b` (élimination de Gauss avec pivot partiel).
#[allow(clippy::needless_range_loop)]
pub fn solve<const N: usize>(mut a: [[f64; N]; N], mut b: [f64; N]) -> Option<[f64; N]> {
    for col in 0..N {
        let pivot = (col..N).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-14 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..N {
            let f = a[row][col] / a[col][col];
            for k in col..N {
                a[row][k] -= f * a[col][k];
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; N];
    for i in (0..N).rev() {
        let s: f64 = (i + 1..N).map(|k| a[i][k] * x[k]).sum();
        x[i] = (b[i] - s) / a[i][i];
    }
    Some(x)
}

/// Le centre et l'échelle qui ramènent des points autour de l'origine, à une distance
/// moyenne de √2 (normalisation de Hartley : l'estimation reste bien conditionnée).
fn normalization(points: &[Point]) -> Homography {
    let n = points.len() as f64;
    let (cx, cy) = points.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
    let d = points
        .iter()
        .map(|p| ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt())
        .sum::<f64>()
        / n;
    let s = if d > 1e-12 { std::f64::consts::SQRT_2 / d } else { 1.0 };
    Homography([[s, 0.0, -s * cx], [0.0, s, -s * cy], [0.0, 0.0, 1.0]])
}

/// L'homographie qui envoie `src[i]` sur `dst[i]` (moindres carrés, h₃₃ = 1) ; au moins
/// quatre paires.
pub fn fit_homography(pairs: &[(Point, Point)]) -> Option<Homography> {
    if pairs.len() < 4 {
        return None;
    }
    let src: Vec<Point> = pairs.iter().map(|p| p.0).collect();
    let dst: Vec<Point> = pairs.iter().map(|p| p.1).collect();
    let (ns, nd) = (normalization(&src), normalization(&dst));
    // Équations normales du système 2N × 8.
    let mut ata = [[0.0; 8]; 8];
    let mut atb = [0.0; 8];
    for (s, d) in src.iter().zip(&dst) {
        let (x, y) = ns.apply(*s)?;
        let (u, v) = nd.apply(*d)?;
        let rows = [
            ([x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y], u),
            ([0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y], v),
        ];
        for (r, rhs) in rows {
            for i in 0..8 {
                atb[i] += r[i] * rhs;
                for j in 0..8 {
                    ata[i][j] += r[i] * r[j];
                }
            }
        }
    }
    let h = solve(ata, atb)?;
    let hn = Homography([[h[0], h[1], h[2]], [h[3], h[4], h[5]], [h[6], h[7], 1.0]]);
    Some(ns.then(&hn).then(&nd.inverse()?))
}

/// La similitude (rotation, échelle, translation) qui envoie `src[i]` sur `dst[i]` ;
/// au moins deux paires.
pub fn fit_similarity(pairs: &[(Point, Point)]) -> Option<Homography> {
    if pairs.len() < 2 {
        return None;
    }
    // x' = a·x − b·y + tx ; y' = b·x + a·y + ty
    let mut ata = [[0.0; 4]; 4];
    let mut atb = [0.0; 4];
    for &((x, y), (u, v)) in pairs {
        for (r, rhs) in [([x, -y, 1.0, 0.0], u), ([y, x, 0.0, 1.0], v)] {
            for i in 0..4 {
                atb[i] += r[i] * rhs;
                for j in 0..4 {
                    ata[i][j] += r[i] * r[j];
                }
            }
        }
    }
    let [a, b, tx, ty] = solve(ata, atb)?;
    Some(Homography([[a, -b, tx], [b, a, ty], [0.0, 0.0, 1.0]]))
}

/// Un générateur pseudo‑aléatoire minuscule (xorshift), reproductible.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.max(1))
    }

    pub fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// Le modèle à estimer par RANSAC.
#[derive(Clone, Copy, Debug)]
pub enum Model {
    Homography,
    Similarity,
}

/// Estimation robuste : des tirages de paires minimaux, le modèle qui explique le plus
/// de paires à moins de `threshold` pixels, puis un ajustement sur toutes celles‑là.
/// Rend le modèle et les indices des bonnes paires.
pub fn ransac(
    pairs: &[(Point, Point)],
    model: Model,
    threshold: f64,
    iterations: usize,
) -> Option<(Homography, Vec<usize>)> {
    let k = match model {
        Model::Homography => 4,
        Model::Similarity => 2,
    };
    if pairs.len() < k {
        return None;
    }
    let fit = |sample: &[(Point, Point)]| match model {
        Model::Homography => fit_homography(sample),
        Model::Similarity => fit_similarity(sample),
    };
    let inliers = |h: &Homography| -> Vec<usize> {
        pairs
            .iter()
            .enumerate()
            .filter(|(_, (s, d))| {
                h.apply(*s)
                    .is_some_and(|p| (p.0 - d.0).powi(2) + (p.1 - d.1).powi(2) < threshold * threshold)
            })
            .map(|(i, _)| i)
            .collect()
    };
    let mut rng = Rng::new(0x9E37_79B9 ^ pairs.len() as u64);
    let mut best: Vec<usize> = Vec::new();
    for _ in 0..iterations {
        let mut idx = Vec::with_capacity(k);
        while idx.len() < k {
            let i = rng.below(pairs.len());
            if !idx.contains(&i) {
                idx.push(i);
            }
        }
        let sample: Vec<(Point, Point)> = idx.iter().map(|&i| pairs[i]).collect();
        let Some(h) = fit(&sample) else { continue };
        let found = inliers(&h);
        if found.len() > best.len() {
            best = found;
            if best.len() * 10 >= pairs.len() * 9 {
                break;
            }
        }
    }
    if best.len() < k {
        return None;
    }
    // Deux ajustements sur les bonnes paires, qui peuvent en gagner quelques‑unes.
    let mut h = fit(&best.iter().map(|&i| pairs[i]).collect::<Vec<_>>())?;
    for _ in 0..2 {
        let more = inliers(&h);
        if more.len() < k {
            break;
        }
        h = fit(&more.iter().map(|&i| pairs[i]).collect::<Vec<_>>())?;
        best = more;
    }
    Some((h, best))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < 1e-6 && (a.1 - b.1).abs() < 1e-6
    }

    #[test]
    fn homography_is_recovered_despite_outliers() {
        let truth = Homography([[1.02, 0.05, 30.0], [-0.03, 0.98, -12.0], [1e-5, -2e-5, 1.0]]);
        let mut pairs = Vec::new();
        let mut rng = Rng::new(7);
        for i in 0..200 {
            let p = (rng.below(1000) as f64, rng.below(700) as f64);
            let q = if i % 4 == 0 {
                (rng.below(1000) as f64, rng.below(700) as f64)
            } else {
                truth.apply(p).unwrap()
            };
            pairs.push((p, q));
        }
        let (h, inliers) = ransac(&pairs, Model::Homography, 1.0, 500).unwrap();
        assert!(inliers.len() >= 150);
        for p in [(0.0, 0.0), (500.0, 350.0), (999.0, 699.0)] {
            assert!(
                close(h.apply(p).unwrap(), truth.apply(p).unwrap()) || {
                    let (a, b) = (h.apply(p).unwrap(), truth.apply(p).unwrap());
                    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
                }
            );
        }
    }

    #[test]
    fn similarity_inverse_and_composition() {
        let s = Homography::similarity(1.0, 0.01, 5.0, -3.0);
        let pairs: Vec<(Point, Point)> = [(10.0, 20.0), (400.0, 30.0), (200.0, 300.0)]
            .iter()
            .map(|&p| (p, s.apply(p).unwrap()))
            .collect();
        let f = fit_similarity(&pairs).unwrap();
        let back = f.then(&s.inverse().unwrap());
        assert!(close(back.apply((123.0, 45.0)).unwrap(), (123.0, 45.0)));
        let big = s.rescaled(4.0);
        let (x, y) = big.apply((400.0, 800.0)).unwrap();
        let (u, v) = s.apply((100.0, 200.0)).unwrap();
        assert!((x / 4.0 - u).abs() < 1e-9 && (y / 4.0 - v).abs() < 1e-9);
    }
}
