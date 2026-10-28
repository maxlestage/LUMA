//! Les images du moteur : RGB en `f32` linéaire, et plans à un canal (luminance,
//! masques). Les opérations coûteuses sont parallélisées par lignes avec rayon.
//!
//! Le flou gaussien, utilisé partout (tonalité locale, clarté, netteté, bruit), est une
//! suite de trois flous boîte, en O(1) par pixel quel que soit le rayon ; les grands
//! rayons passent par une image réduite, puis agrandie.

use rayon::prelude::*;

/// Une image RGB, ligne par ligne, sans marge.
#[derive(Clone, Debug, PartialEq)]
pub struct RgbImage {
    pub width: usize,
    pub height: usize,
    pub data: Vec<[f32; 3]>,
}

/// Un plan à un seul canal.
#[derive(Clone, Debug, PartialEq)]
pub struct Plane {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl RgbImage {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![[0.0; 3]; width * height],
        }
    }

    pub fn from_fn(width: usize, height: usize, f: impl Fn(usize, usize) -> [f32; 3] + Sync) -> Self {
        let mut img = Self::new(width, height);
        img.data.par_chunks_mut(width.max(1)).enumerate().for_each(|(y, row)| {
            for (x, px) in row.iter_mut().enumerate() {
                *px = f(x, y);
            }
        });
        img
    }

    pub fn pixel(&self, x: usize, y: usize) -> [f32; 3] {
        self.data[y * self.width + x]
    }

    /// Réduction par moyenne des surfaces : chaque pixel de sortie est la moyenne
    /// exacte, pondérée, des pixels d'entrée qu'il recouvre. Sans crénelage ni moiré.
    pub fn resize_area(&self, width: usize, height: usize) -> RgbImage {
        if width == self.width && height == self.height {
            return self.clone();
        }
        let xs = spans(self.width, width);
        let ys = spans(self.height, height);
        let mut out = RgbImage::new(width, height);
        out.data.par_chunks_mut(width).enumerate().for_each(|(oy, row)| {
            let mut acc = vec![[0.0f32; 3]; width];
            let mut total_wy = 0.0;
            for &(iy, wy) in &ys[oy] {
                total_wy += wy;
                let src = &self.data[iy * self.width..(iy + 1) * self.width];
                for (ox, a) in acc.iter_mut().enumerate() {
                    for &(ix, wx) in &xs[ox] {
                        let p = src[ix];
                        let w = wx * wy;
                        a[0] += p[0] * w;
                        a[1] += p[1] * w;
                        a[2] += p[2] * w;
                    }
                }
            }
            for (ox, px) in row.iter_mut().enumerate() {
                let total_wx: f32 = xs[ox].iter().map(|s| s.1).sum();
                let norm = 1.0 / (total_wx * total_wy);
                *px = [acc[ox][0] * norm, acc[ox][1] * norm, acc[ox][2] * norm];
            }
        });
        out
    }

    /// La taille qui tient dans `max_side` en gardant les proportions.
    pub fn fit(width: usize, height: usize, max_side: usize) -> (usize, usize) {
        let long = width.max(height);
        if long <= max_side {
            return (width, height);
        }
        let s = max_side as f64 / long as f64;
        (
            ((width as f64 * s).round() as usize).max(1),
            ((height as f64 * s).round() as usize).max(1),
        )
    }

    /// Lecture bilinéaire, coordonnées en pixels (bords prolongés).
    pub fn sample(&self, x: f32, y: f32) -> [f32; 3] {
        let x = x.clamp(0.0, (self.width - 1) as f32);
        let y = y.clamp(0.0, (self.height - 1) as f32);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let p = |x, y| self.data[y * self.width + x];
        let (a, b, c, d) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
        let mut out = [0.0; 3];
        for i in 0..3 {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            out[i] = top + (bottom - top) * fy;
        }
        out
    }
}

/// Pour chaque pixel de sortie, les pixels d'entrée qu'il recouvre et leur poids.
fn spans(input: usize, output: usize) -> Vec<Vec<(usize, f32)>> {
    let scale = input as f64 / output as f64;
    (0..output)
        .map(|o| {
            let start = o as f64 * scale;
            let end = ((o + 1) as f64 * scale).min(input as f64);
            let mut v = Vec::new();
            let mut i = start.floor() as usize;
            while (i as f64) < end && i < input {
                let lo = start.max(i as f64);
                let hi = end.min((i + 1) as f64);
                if hi > lo {
                    v.push((i, (hi - lo) as f32));
                }
                i += 1;
            }
            if v.is_empty() {
                v.push(((start as usize).min(input - 1), 1.0));
            }
            v
        })
        .collect()
}

impl Plane {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; width * height],
        }
    }

    pub fn from_rgb(img: &RgbImage, f: impl Fn([f32; 3]) -> f32 + Sync) -> Self {
        Self {
            width: img.width,
            height: img.height,
            data: img.data.par_iter().map(|&p| f(p)).collect(),
        }
    }

    pub fn map(&self, f: impl Fn(f32) -> f32 + Sync) -> Self {
        Self {
            width: self.width,
            height: self.height,
            data: self.data.par_iter().map(|&v| f(v)).collect(),
        }
    }

    pub fn zip(&self, other: &Plane, f: impl Fn(f32, f32) -> f32 + Sync) -> Self {
        debug_assert_eq!(self.data.len(), other.data.len());
        Self {
            width: self.width,
            height: self.height,
            data: self.data.par_iter().zip(&other.data).map(|(&a, &b)| f(a, b)).collect(),
        }
    }

    /// Flou boîte horizontal de rayon `r`, bords prolongés. Le centre de la ligne est
    /// parcouru sans aucune vérification de bornes.
    #[cfg(test)]
    fn box_rows(&self, r: usize) -> Plane {
        if r == 0 {
            return self.clone();
        }
        let mut out = Plane::new(self.width, self.height);
        self.box_rows_into(r, &mut out);
        out
    }

    /// `box_rows`, écrit dans `out` (de même taille, `r` ≥ 1) : les passes successives
    /// d'un flou réutilisent deux tampons au lieu d'allouer chacune une image neuve (une
    /// mémoire neuve coûte une faute de page par page à la première écriture).
    fn box_rows_into(&self, r: usize, out: &mut Plane) {
        self.box_rows_map_into(r, out, |v| v);
    }

    /// `box_rows_into` de `f(v)` : chaque valeur transformée à la lecture (le carré du
    /// filtre guidé, sans plan intermédiaire).
    fn box_rows_map_into(&self, r: usize, out: &mut Plane, f: impl Fn(f32) -> f32 + Sync) {
        let w = self.width;
        let norm = 1.0 / (2 * r + 1) as f32;
        out.data
            .par_chunks_mut(w)
            .zip(self.data.par_chunks(w))
            .for_each(|(dst, src)| {
                let at = |i: isize| f(src[i.clamp(0, w as isize - 1) as usize]);
                let ri = r as isize;
                let mut sum: f32 = (-ri..=ri).map(at).sum();
                let lo = (r + 1).min(w);
                let hi = w.saturating_sub(r + 1).max(lo);
                for (x, d) in dst.iter_mut().enumerate().take(lo) {
                    *d = sum * norm;
                    sum += at(x as isize + ri + 1) - at(x as isize - ri);
                }
                for (x, d) in dst.iter_mut().enumerate().take(hi).skip(lo) {
                    *d = sum * norm;
                    sum += f(src[x + r + 1]) - f(src[x - r]);
                }
                for (x, d) in dst.iter_mut().enumerate().skip(hi) {
                    *d = sum * norm;
                    sum += at(x as isize + ri + 1) - at(x as isize - ri);
                }
            });
    }

    /// Flou boîte vertical de rayon `r`. Par blocs de lignes, chacun avec ses sommes de
    /// colonnes glissantes : lectures et écritures contiguës, sans transposition.
    #[cfg(test)]
    fn box_cols(&self, r: usize) -> Plane {
        if r == 0 {
            return self.clone();
        }
        let mut out = Plane::new(self.width, self.height);
        self.box_cols_into(r, &mut out);
        out
    }

    /// `box_cols`, écrit dans `out` (de même taille, `r` ≥ 1).
    fn box_cols_into(&self, r: usize, out: &mut Plane) {
        let (w, h) = (self.width, self.height);
        const ROWS: usize = 32;
        let norm = 1.0 / (2 * r + 1) as f32;
        let row = |y: isize| {
            let y = y.clamp(0, h as isize - 1) as usize;
            &self.data[y * w..(y + 1) * w]
        };
        out.data
            .par_chunks_mut(w * ROWS)
            .enumerate()
            .for_each(|(block, chunk)| {
                let y0 = (block * ROWS) as isize;
                let mut sum = vec![0.0f32; w];
                for k in -(r as isize)..=r as isize {
                    for (s, v) in sum.iter_mut().zip(row(y0 + k)) {
                        *s += v;
                    }
                }
                for (i, dst) in chunk.chunks_mut(w).enumerate() {
                    let y = y0 + i as isize;
                    for (d, s) in dst.iter_mut().zip(&sum) {
                        *d = s * norm;
                    }
                    let (add, sub) = (row(y + r as isize + 1), row(y - r as isize));
                    for ((s, a), b) in sum.iter_mut().zip(add).zip(sub) {
                        *s += a - b;
                    }
                }
            });
    }

    /// Le rayon des trois boîtes successives qui approchent une gaussienne d'écart type
    /// `sigma` (théorème central limite) ; le GPU s'en sert aussi.
    pub fn box_radius(sigma: f32) -> usize {
        let ideal = (12.0 * sigma * sigma / 3.0 + 1.0).sqrt();
        ((ideal - 1.0) / 2.0).round().max(1.0) as usize
    }

    /// Flou gaussien d'écart type `sigma` (en pixels de ce plan).
    pub fn blur(&self, sigma: f32) -> Plane {
        if sigma < 0.3 || self.width < 2 || self.height < 2 {
            return self.clone();
        }
        // Grand rayon : on floute une version réduite, le résultat est le même à l'œil
        // pour une fraction du coût.
        let factor = (sigma / 3.0).floor() as usize;
        let factor = factor.min(self.width / 8).min(self.height / 8);
        if factor >= 2 {
            let small = self.downscale(factor).blur(sigma / factor as f32);
            return small.upsample(self.width, self.height);
        }
        self.box_blur(Self::box_radius(sigma), |v| v)
    }

    /// Le flou de `f(v)` : les mêmes valeurs que `self.map(f).blur(sigma)` ; au petit
    /// rayon (pleine taille), `f` s'applique à la lecture de la première passe, sans plan
    /// intermédiaire.
    pub fn blur_map(&self, sigma: f32, f: impl Fn(f32) -> f32 + Sync) -> Plane {
        let factor = ((sigma / 3.0).floor() as usize)
            .min(self.width / 8)
            .min(self.height / 8);
        if sigma < 0.3 || self.width < 2 || self.height < 2 || factor >= 2 {
            return self.map(f).blur(sigma);
        }
        self.box_blur(Self::box_radius(sigma), f)
    }

    /// Trois flous boîte en ligne puis trois en colonne, de rayon `r`, sur `f(v)`. Six
    /// passes, deux tampons qui alternent.
    fn box_blur(&self, r: usize, f: impl Fn(f32) -> f32 + Sync) -> Plane {
        let (mut a, mut b) = (Plane::new(self.width, self.height), Plane::new(self.width, self.height));
        self.box_rows_map_into(r, &mut a, f);
        a.box_rows_into(r, &mut b);
        b.box_rows_into(r, &mut a);
        a.box_cols_into(r, &mut b);
        b.box_cols_into(r, &mut a);
        a.box_cols_into(r, &mut b);
        b
    }

    /// Réduction d'un facteur entier `k` : moyenne de chaque bloc `k × k` (les blocs du
    /// bord, incomplets, sont moyennés sur ce qu'ils contiennent).
    pub fn downscale(&self, k: usize) -> Plane {
        if k <= 1 {
            return self.clone();
        }
        let (w, h) = (self.width, self.height);
        let (ow, oh) = (w.div_ceil(k), h.div_ceil(k));
        let mut out = Plane::new(ow, oh);
        out.data.par_chunks_mut(ow).enumerate().for_each(|(oy, row)| {
            let (y0, y1) = (oy * k, ((oy + 1) * k).min(h));
            for y in y0..y1 {
                let src = &self.data[y * w..(y + 1) * w];
                for (ox, v) in row.iter_mut().enumerate() {
                    let (x0, x1) = (ox * k, ((ox + 1) * k).min(w));
                    let s = src[x0..x1].iter().sum::<f32>();
                    // La première ligne écrit sans lire : une mémoire neuve lue avant
                    // d'être écrite coûte deux fautes de page (la page de zéros, puis sa
                    // copie). `0.0 + s` garde exactement la somme d'avant.
                    *v = if y == y0 { 0.0 + s } else { *v + s };
                }
            }
            for (ox, v) in row.iter_mut().enumerate() {
                let n = (((ox + 1) * k).min(w) - ox * k) * (y1 - y0);
                *v /= n as f32;
            }
        });
        out
    }

    pub fn resize_area(&self, width: usize, height: usize) -> Plane {
        let xs = spans(self.width, width);
        let ys = spans(self.height, height);
        let mut out = Plane::new(width, height);
        out.data.par_chunks_mut(width).enumerate().for_each(|(oy, row)| {
            let mut acc = vec![0.0f32; width];
            let mut total_wy = 0.0;
            for &(iy, wy) in &ys[oy] {
                total_wy += wy;
                let src = &self.data[iy * self.width..(iy + 1) * self.width];
                for (ox, a) in acc.iter_mut().enumerate() {
                    for &(ix, wx) in &xs[ox] {
                        *a += src[ix] * wx * wy;
                    }
                }
            }
            for (ox, v) in row.iter_mut().enumerate() {
                let total_wx: f32 = xs[ox].iter().map(|s| s.1).sum();
                *v = acc[ox] / (total_wx * total_wy);
            }
        });
        out
    }

    /// Agrandissement bilinéaire (centres de pixels alignés).
    pub fn upsample(&self, width: usize, height: usize) -> Plane {
        let mut out = Plane::new(width, height);
        let columns = self.upsample_columns(width);
        out.data
            .par_chunks_mut(width)
            .enumerate()
            .for_each(|(y, row)| self.upsample_row(y, height, &columns, row));
        out
    }

    /// Pour chaque colonne d'un agrandissement à `width` : les deux colonnes voisines et
    /// le poids de la seconde (les mêmes pour toutes les lignes).
    pub fn upsample_columns(&self, width: usize) -> Vec<(usize, usize, f32)> {
        let sx = self.width as f32 / width as f32;
        (0..width)
            .map(|x| {
                let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (self.width - 1) as f32);
                let x0 = fx.floor() as usize;
                (x0, (x0 + 1).min(self.width - 1), fx - x0 as f32)
            })
            .collect()
    }

    /// La ligne `y` de l'agrandissement bilinéaire à `row.len() × height`, les colonnes
    /// données par `upsample_columns` (le calcul d'`upsample`, une ligne à la fois :
    /// sans garder l'image agrandie).
    pub fn upsample_row(&self, y: usize, height: usize, columns: &[(usize, usize, f32)], row: &mut [f32]) {
        let sy = self.height as f32 / height as f32;
        let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (self.height - 1) as f32);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(self.height - 1);
        let ty = fy - y0 as f32;
        let r0 = &self.data[y0 * self.width..(y0 + 1) * self.width];
        let r1 = &self.data[y1 * self.width..(y1 + 1) * self.width];
        for (v, &(x0, x1, tx)) in row.iter_mut().zip(columns) {
            let top = r0[x0] + (r0[x1] - r0[x0]) * tx;
            let bottom = r1[x0] + (r1[x1] - r1[x0]) * tx;
            *v = top + (bottom - top) * ty;
        }
    }

    /// Filtre guidé (He et al.) auto‑guidé : lisse en respectant les contours. `eps`
    /// fixe ce qui compte comme un contour (variance locale). Pour un grand rayon, les
    /// coefficients sont calculés sur une image réduite puis agrandis (filtre guidé
    /// rapide, He 2015) : même aspect, une fraction du coût.
    pub fn guided(&self, sigma: f32, eps: f32) -> Plane {
        let factor = ((sigma / 4.0).floor() as usize)
            .min(self.width / 8)
            .min(self.height / 8);
        if factor >= 2 {
            let (a, b) = self.downscale(factor).guided_coefficients(sigma / factor as f32, eps);
            self.apply_upsampled(&a, &b, self)
        } else {
            let (a, b) = self.guided_coefficients(sigma, eps);
            self.apply_coefficients(&a, &b, self)
        }
    }

    /// Filtre guidé croisé : ce plan lissé en suivant les contours de `guide` (de même
    /// taille). Une zone de masque y épouse les bords de la photo. Grand rayon : les
    /// coefficients sur des plans réduits, comme [`Plane::guided`].
    pub fn guided_by(&self, guide: &Plane, sigma: f32, eps: f32) -> Plane {
        let factor = ((sigma / 4.0).floor() as usize)
            .min(self.width / 8)
            .min(self.height / 8);
        if factor >= 2 {
            let (a, b) =
                self.downscale(factor)
                    .cross_coefficients(&guide.downscale(factor), sigma / factor as f32, eps);
            self.apply_upsampled(&a, &b, guide)
        } else {
            let (a, b) = self.cross_coefficients(guide, sigma, eps);
            self.apply_coefficients(&a, &b, guide)
        }
    }

    /// `a · guide + b`, pixel par pixel, en une passe.
    fn apply_coefficients(&self, a: &Plane, b: &Plane, guide: &Plane) -> Plane {
        let mut out = Plane::new(self.width, self.height);
        out.data
            .par_iter_mut()
            .zip(&a.data)
            .zip(&b.data)
            .zip(&guide.data)
            .for_each(|(((o, a), b), i)| *o = a * i + b);
        out
    }

    /// `a · guide + b`, les coefficients réduits `a` et `b` agrandis ligne par ligne (le
    /// calcul d'[`Plane::upsample`], sans garder les deux plans agrandis).
    fn apply_upsampled(&self, a: &Plane, b: &Plane, guide: &Plane) -> Plane {
        let (w, h) = (self.width, self.height);
        let mut out = Plane::new(w, h);
        let columns = a.upsample_columns(w);
        out.data.par_chunks_mut(w).enumerate().for_each_init(
            || (vec![0.0; w], vec![0.0; w]),
            |(ra, rb), (y, row)| {
                a.upsample_row(y, h, &columns, ra);
                b.upsample_row(y, h, &columns, rb);
                let g = &guide.data[y * w..(y + 1) * w];
                for (((o, a), b), i) in row.iter_mut().zip(ra.iter()).zip(rb.iter()).zip(g) {
                    *o = a * i + b;
                }
            },
        );
        out
    }

    fn cross_coefficients(&self, guide: &Plane, sigma: f32, eps: f32) -> (Plane, Plane) {
        let mean_i = guide.blur(sigma);
        let mean_p = self.blur(sigma);
        let corr_ip = guide.zip(self, |i, p| i * p).blur(sigma);
        let corr_ii = guide.blur_map(sigma, |v| v * v);
        // a et b ensemble, en une passe.
        let mut a = Plane::new(self.width, self.height);
        let mut b = Plane::new(self.width, self.height);
        a.data
            .par_iter_mut()
            .zip(b.data.par_iter_mut())
            .enumerate()
            .for_each(|(k, (a, b))| {
                let (mi, mp) = (mean_i.data[k], mean_p.data[k]);
                let var = (corr_ii.data[k] - mi * mi).max(0.0);
                *a = (corr_ip.data[k] - mi * mp) / (var + eps);
                *b = mp - *a * mi;
            });
        (a.blur(sigma), b.blur(sigma))
    }

    fn guided_coefficients(&self, sigma: f32, eps: f32) -> (Plane, Plane) {
        let mean = self.blur(sigma);
        let sq = self.blur_map(sigma, |v| v * v);
        let mut a = Plane::new(self.width, self.height);
        let mut b = Plane::new(self.width, self.height);
        a.data
            .par_iter_mut()
            .zip(b.data.par_iter_mut())
            .zip(&mean.data)
            .zip(&sq.data)
            .for_each(|(((a, b), m), s)| {
                let var = (s - m * m).max(0.0);
                *a = var / (var + eps);
                *b = m - *a * m;
            });
        (a.blur(sigma), b.blur(sigma))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // L'ancien filtre guidé (un plan neuf par étape), pour vérifier que la version
    // fusionnée donne exactement les mêmes valeurs.
    impl Plane {
        fn guided_reference(&self, sigma: f32, eps: f32) -> Plane {
            let factor = ((sigma / 4.0).floor() as usize)
                .min(self.width / 8)
                .min(self.height / 8);
            let (a, b) = if factor >= 2 {
                let (a, b) = self
                    .downscale(factor)
                    .guided_coefficients_reference(sigma / factor as f32, eps);
                (a.upsample(self.width, self.height), b.upsample(self.width, self.height))
            } else {
                self.guided_coefficients_reference(sigma, eps)
            };
            let mut out = a.zip(self, |a, i| a * i);
            out.data.par_iter_mut().zip(&b.data).for_each(|(o, b)| *o += b);
            out
        }

        fn guided_by_reference(&self, guide: &Plane, sigma: f32, eps: f32) -> Plane {
            let factor = ((sigma / 4.0).floor() as usize)
                .min(self.width / 8)
                .min(self.height / 8);
            let (a, b) = if factor >= 2 {
                let (a, b) = self.downscale(factor).cross_coefficients_reference(
                    &guide.downscale(factor),
                    sigma / factor as f32,
                    eps,
                );
                (a.upsample(self.width, self.height), b.upsample(self.width, self.height))
            } else {
                self.cross_coefficients_reference(guide, sigma, eps)
            };
            let mut out = a.zip(guide, |a, i| a * i);
            out.data.par_iter_mut().zip(&b.data).for_each(|(o, b)| *o += b);
            out
        }

        fn cross_coefficients_reference(&self, guide: &Plane, sigma: f32, eps: f32) -> (Plane, Plane) {
            let mean_i = guide.blur(sigma);
            let mean_p = self.blur(sigma);
            let corr_ip = guide.zip(self, |i, p| i * p).blur(sigma);
            let corr_ii = guide.map(|v| v * v).blur(sigma);
            let mut a = Plane::new(self.width, self.height);
            a.data.par_iter_mut().enumerate().for_each(|(k, a)| {
                let (mi, mp) = (mean_i.data[k], mean_p.data[k]);
                let var = (corr_ii.data[k] - mi * mi).max(0.0);
                *a = (corr_ip.data[k] - mi * mp) / (var + eps);
            });
            let b = mean_p.zip(&a.zip(&mean_i, |a, mi| a * mi), |mp, ami| mp - ami);
            (a.blur(sigma), b.blur(sigma))
        }

        fn guided_coefficients_reference(&self, sigma: f32, eps: f32) -> (Plane, Plane) {
            let mean = self.blur(sigma);
            let sq = self.map(|v| v * v).blur(sigma);
            let a = mean.zip(&sq, |m, s| {
                let var = (s - m * m).max(0.0);
                var / (var + eps)
            });
            let b = a.zip(&mean, |a, m| m - a * m);
            (a.blur(sigma), b.blur(sigma))
        }
    }

    #[test]
    fn fused_guided_filters_give_the_same_values() {
        for (w, h) in [(203, 151), (64, 64), (40, 23)] {
            let p = Plane {
                width: w,
                height: h,
                data: (0..w * h).map(|i| ((i * 7919) % 1000) as f32 / 1000.0).collect(),
            };
            let g = Plane {
                width: w,
                height: h,
                data: (0..w * h).map(|i| ((i % w) as f32 / w as f32).powi(2)).collect(),
            };
            let same = |a: &Plane, b: &Plane| a.data.iter().zip(&b.data).all(|(x, y)| x.to_bits() == y.to_bits());
            // Les petits rayons (pleine taille) et les grands (coefficients réduits).
            for sigma in [0.5, 2.0, 9.0, 24.0] {
                assert!(
                    same(&p.guided(sigma, 0.01), &p.guided_reference(sigma, 0.01)),
                    "{w}×{h} {sigma}"
                );
                assert!(
                    same(&p.guided_by(&g, sigma, 1e-4), &p.guided_by_reference(&g, sigma, 1e-4)),
                    "{w}×{h} {sigma}"
                );
            }
        }
    }

    #[test]
    fn blur_keeps_flat_images_flat() {
        let p = Plane {
            width: 64,
            height: 48,
            data: vec![0.5; 64 * 48],
        };
        for sigma in [1.0, 4.0, 20.0] {
            let b = p.blur(sigma);
            assert!(b.data.iter().all(|v| (v - 0.5).abs() < 1e-4), "sigma {sigma}");
        }
    }

    #[test]
    fn blur_preserves_the_mean_and_spreads_an_impulse() {
        let mut p = Plane::new(101, 101);
        p.data[50 * 101 + 50] = 1.0;
        let b = p.blur(3.0);
        let sum: f32 = b.data.iter().sum();

        assert!((sum - 1.0).abs() < 1e-3, "{sum}");
        assert!(b.data[50 * 101 + 50] < 0.05);
        assert!(b.data[50 * 101 + 53] > 0.0);
    }

    #[test]
    fn blur_of_a_mapped_plane_needs_no_intermediate_plane() {
        let p = Plane {
            width: 61,
            height: 43,
            data: (0..61 * 43).map(|i| ((i as f32) * 0.618).fract() - 0.3).collect(),
        };
        for sigma in [0.2, 0.8, 1.5, 2.9, 7.0] {
            let a = p.blur_map(sigma, |v| v * v);
            let b = p.map(|v| v * v).blur(sigma);
            assert!(
                a.data.iter().zip(&b.data).all(|(x, y)| x.to_bits() == y.to_bits()),
                "sigma {sigma}"
            );
        }
    }

    #[test]
    fn blur_in_two_buffers_matches_the_chained_passes() {
        let p = Plane {
            width: 53,
            height: 41,
            data: (0..53 * 41).map(|i| ((i as f32) * 0.618).fract()).collect(),
        };
        for sigma in [0.5, 1.0, 2.5, 5.0] {
            let ideal = (12.0 * sigma * sigma / 3.0 + 1.0_f32).sqrt();
            let r = ((ideal - 1.0) / 2.0).round().max(1.0) as usize;
            let chained = p
                .box_rows(r)
                .box_rows(r)
                .box_rows(r)
                .box_cols(r)
                .box_cols(r)
                .box_cols(r);
            let b = p.blur(sigma);
            assert!(
                b.data
                    .iter()
                    .zip(&chained.data)
                    .all(|(x, y)| x.to_bits() == y.to_bits()),
                "sigma {sigma}"
            );
        }
    }

    #[test]
    fn cross_guided_filter_follows_the_guide_edges() {
        // Un guide coupé net au milieu ; une zone qui déborde de trois pixels sur le
        // côté droit : le filtre la ramène vers le bord du guide.
        let (w, h) = (40, 20);
        let guide = Plane {
            width: w,
            height: h,
            data: (0..w * h).map(|i| if i % w < 20 { 0.2 } else { 0.8 }).collect(),
        };
        let zone = Plane {
            width: w,
            height: h,
            data: (0..w * h).map(|i| if i % w < 23 { 1.0 } else { 0.0 }).collect(),
        };
        let out = zone.guided_by(&guide, 4.0, 1e-4);
        let at = |x: usize| out.data[10 * w + x];
        assert!(at(10) > 0.95, "{}", at(10));
        assert!(at(21) < 0.5, "{}", at(21));
        // Sans bord dans le guide, rien ne la retient : un simple lissage.
        let flat = Plane {
            width: w,
            height: h,
            data: vec![0.5; w * h],
        };
        let soft = zone.guided_by(&flat, 4.0, 1e-4);
        assert!(soft.data[10 * w + 21] > 0.5);
    }

    #[test]
    fn vertical_and_horizontal_boxes_agree() {
        // Une image et sa transposée : le flou vertical de l'une est le flou horizontal
        // de l'autre.
        let (w, h) = (37, 70);
        let p = Plane {
            width: w,
            height: h,
            data: (0..w * h).map(|i| ((i * 7919) % 101) as f32).collect(),
        };
        let t = Plane {
            width: h,
            height: w,
            data: (0..w * h).map(|i| p.data[(i % h) * w + i / h]).collect(),
        };
        let (v, hz) = (p.box_cols(3), t.box_rows(3));
        for y in 0..h {
            for x in 0..w {
                assert!((v.data[y * w + x] - hz.data[x * h + y]).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn fast_guided_filter_matches_the_full_one_on_smooth_areas() {
        let p = Plane {
            width: 400,
            height: 300,
            data: (0..120_000).map(|i| ((i % 400) as f32 / 400.0).sin()).collect(),
        };
        let g = p.guided(24.0, 0.01);
        assert!((g.data[150 * 400 + 200] - p.data[150 * 400 + 200]).abs() < 0.02);
    }

    #[test]
    fn integer_downscale_averages_blocks() {
        let p = Plane {
            width: 5,
            height: 3,
            data: (0..15).map(|i| i as f32).collect(),
        };
        let d = p.downscale(2);
        assert_eq!((d.width, d.height), (3, 2));
        assert_eq!(d.data[0], (0.0 + 1.0 + 5.0 + 6.0) / 4.0);
        assert_eq!(d.data[2], (4.0 + 9.0) / 2.0);
        assert_eq!(d.data[5], 14.0);
    }

    #[test]
    fn downscale_keeps_the_exact_sums() {
        // Écrire la première ligne sans la lire ne change aucun bit : même ordre de
        // somme qu'une accumulation partant de zéro, zéros négatifs compris.
        let p = Plane {
            width: 7,
            height: 5,
            data: (0..35)
                .map(|i| if i % 6 == 0 { -0.0 } else { (i as f32 * 0.37).sin() })
                .collect(),
        };
        for k in [2, 3] {
            let d = p.downscale(k);
            for oy in 0..d.height {
                for ox in 0..d.width {
                    let mut v = 0.0f32;
                    let ys = oy * k..((oy + 1) * k).min(p.height);
                    let xs = ox * k..((ox + 1) * k).min(p.width);
                    for y in ys.clone() {
                        v += p.data[y * p.width + xs.start..y * p.width + xs.end].iter().sum::<f32>();
                    }
                    v /= (ys.len() * xs.len()) as f32;
                    assert_eq!(d.data[oy * d.width + ox].to_bits(), v.to_bits());
                }
            }
        }
    }

    #[test]
    fn area_resize_averages() {
        let img = RgbImage::from_fn(4, 2, |x, _| [x as f32, 0.0, 1.0]);
        let small = img.resize_area(2, 1);
        assert_eq!(small.data, vec![[0.5, 0.0, 1.0], [2.5, 0.0, 1.0]]);
        assert_eq!(RgbImage::fit(6000, 4000, 2048), (2048, 1365));
    }

    #[test]
    fn guided_filter_keeps_edges() {
        let p = Plane {
            width: 40,
            height: 10,
            data: (0..400).map(|i| if i % 40 < 20 { 0.0 } else { 1.0 }).collect(),
        };
        let g = p.guided(4.0, 0.01);
        assert!(g.data[5 * 40 + 18] < 0.1 && g.data[5 * 40 + 21] > 0.9);
    }
}
