//! Les instruments de mesure de l'image affichée, ceux des étalonneurs : forme d'onde
//! (la luminance, colonne par colonne), parade RVB (la même, une couche à côté de
//! l'autre) et vectorscope (la teinte et la saturation, en Cb/Cr Rec. 709).
//!
//! Chaque instrument se calcule sur l'image encodée (8 bits, celle qu'on voit) et se
//! rend en une petite image RGBA, que l'interface affiche telle quelle ; les repères
//! (graduations, cibles des couleurs primaires) sont dessinés par-dessus par l'interface,
//! à partir de [`waveform_level`] et [`vectorscope_point`].

/// Un instrument.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    Histogram,
    Waveform,
    Parade,
    Vectorscope,
}

impl Scope {
    pub const ALL: [Scope; 4] = [Scope::Histogram, Scope::Waveform, Scope::Parade, Scope::Vectorscope];

    /// L'instrument suivant (touche Maj+J).
    pub fn next(self) -> Scope {
        let i = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// Un instrument rendu : `width × height` pixels RGBA, non prémultipliés.
#[derive(Clone, Debug, PartialEq)]
pub struct ScopeImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// La hauteur des formes d'onde (niveaux de 0 à 255 ramenés à 128 lignes).
pub const LEVELS: usize = 128;
/// La largeur de la forme d'onde, et celle de chaque couche de la parade.
pub const WAVE_COLUMNS: usize = 256;
pub const PARADE_COLUMNS: usize = 96;
/// L'écart entre deux couches de la parade.
pub const PARADE_GAP: usize = 6;
/// Le côté du vectorscope.
pub const VECTOR_SIDE: usize = 160;

/// Au plus ce nombre de pixels lus : au‑delà, un sur deux, un sur trois…
const MAX_SAMPLES: usize = 600_000;

#[inline]
fn luma(p: &[u8]) -> f32 {
    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
}

/// La ligne (0 en haut) d'un niveau de 0 à 1 dans une forme d'onde.
pub fn waveform_level(level: f32) -> f32 {
    1.0 - level.clamp(0.0, 1.0)
}

/// La place (0 à 1 sur chaque axe, le neutre au centre) d'une couleur encodée
/// (composantes de 0 à 1) dans le vectorscope.
pub fn vectorscope_point(rgb: [f32; 3]) -> [f32; 2] {
    let y = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    let cb = (rgb[2] - y) / 1.8556;
    let cr = (rgb[0] - y) / 1.5748;
    [0.5 + cb, 0.5 - cr]
}

/// Les pas de lecture : une ligne et une colonne sur `step`.
fn step(width: usize, height: usize) -> usize {
    let n = width * height;
    let mut s = 1;
    while n / (s * s) > MAX_SAMPLES {
        s += 1;
    }
    s
}

/// Une densité (nombre de pixels par case) ramenée à une intensité de 0 à 1 : une case
/// aussi remplie que la moyenne est déjà bien visible, une case pleine ne sature pas
/// tout de suite.
fn intensity(count: u32, mean: f32) -> f32 {
    if count == 0 {
        return 0.0;
    }
    1.0 - (-(count as f32) / (0.6 * mean.max(1e-3))).exp()
}

fn put(out: &mut [u8], i: usize, color: [f32; 3], v: f32) {
    let o = &mut out[i * 4..i * 4 + 4];
    for c in 0..3 {
        o[c] = (color[c] * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    o[3] = (v * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Les comptes d'une forme d'onde : pour chaque colonne, combien de pixels à chaque
/// niveau, la valeur lue par `value` (de 0 à 255).
fn wave_counts(
    rgba: &[u8],
    width: usize,
    height: usize,
    columns: usize,
    value: impl Fn(&[u8]) -> f32,
) -> (Vec<u32>, usize) {
    let mut counts = vec![0u32; columns * LEVELS];
    let s = step(width, height);
    let mut samples = 0;
    for y in (0..height).step_by(s) {
        let row = &rgba[y * width * 4..(y + 1) * width * 4];
        for x in (0..width).step_by(s) {
            let v = value(&row[x * 4..x * 4 + 4]);
            let level = ((v / 255.0) * (LEVELS - 1) as f32).round() as usize;
            let col = x * columns / width;
            counts[(LEVELS - 1 - level.min(LEVELS - 1)) * columns + col] += 1;
            samples += 1;
        }
    }
    (counts, samples)
}

/// Les comptes rendus en image, avec la couleur `color` ; `out` a `out_width` colonnes,
/// et la forme d'onde y commence à la colonne `left`.
fn paint_wave(
    counts: &[u32],
    samples: usize,
    columns: usize,
    color: [f32; 3],
    out: &mut [u8],
    out_width: usize,
    left: usize,
) {
    let mean = samples as f32 / (columns * LEVELS) as f32;
    for row in 0..LEVELS {
        for col in 0..columns {
            let v = intensity(counts[row * columns + col], mean);
            if v > 0.0 {
                put(out, row * out_width + left + col, color, v);
            }
        }
    }
}

/// L'instrument `scope` sur l'image RGBA `width × height` ; rien pour l'histogramme,
/// que l'interface dessine elle‑même.
pub fn render(scope: Scope, rgba: &[u8], width: usize, height: usize) -> Option<ScopeImage> {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return None;
    }
    match scope {
        Scope::Histogram => None,
        Scope::Waveform => {
            let (counts, n) = wave_counts(rgba, width, height, WAVE_COLUMNS, luma);
            let mut out = vec![0u8; WAVE_COLUMNS * LEVELS * 4];
            paint_wave(&counts, n, WAVE_COLUMNS, [0.78, 1.0, 0.86], &mut out, WAVE_COLUMNS, 0);
            Some(ScopeImage {
                width: WAVE_COLUMNS,
                height: LEVELS,
                rgba: out,
            })
        }
        Scope::Parade => {
            let w = PARADE_COLUMNS * 3 + PARADE_GAP * 2;
            let mut out = vec![0u8; w * LEVELS * 4];
            let colors = [[1.0, 0.36, 0.36], [0.38, 1.0, 0.5], [0.42, 0.62, 1.0]];
            for (c, color) in colors.iter().enumerate() {
                let (counts, n) = wave_counts(rgba, width, height, PARADE_COLUMNS, |p| f32::from(p[c]));
                let left = c * (PARADE_COLUMNS + PARADE_GAP);
                paint_wave(&counts, n, PARADE_COLUMNS, *color, &mut out, w, left);
            }
            Some(ScopeImage {
                width: w,
                height: LEVELS,
                rgba: out,
            })
        }
        Scope::Vectorscope => {
            let side = VECTOR_SIDE;
            let mut counts = vec![0u32; side * side];
            let s = step(width, height);
            let mut samples = 0usize;
            for y in (0..height).step_by(s) {
                let row = &rgba[y * width * 4..(y + 1) * width * 4];
                for x in (0..width).step_by(s) {
                    let p = &row[x * 4..x * 4 + 4];
                    let [u, v] = vectorscope_point([
                        f32::from(p[0]) / 255.0,
                        f32::from(p[1]) / 255.0,
                        f32::from(p[2]) / 255.0,
                    ]);
                    let cx = (u * (side - 1) as f32).round().clamp(0.0, (side - 1) as f32);
                    let cy = (v * (side - 1) as f32).round().clamp(0.0, (side - 1) as f32);
                    counts[cy as usize * side + cx as usize] += 1;
                    samples += 1;
                }
            }
            // Les cases occupées seulement : un ciel ou une peau ne se noie pas dans
            // tout le vide autour.
            let occupied = counts.iter().filter(|c| **c > 0).count().max(1);
            let mean = samples as f32 / occupied as f32;
            let mut out = vec![0u8; side * side * 4];
            for cy in 0..side {
                for cx in 0..side {
                    let v = intensity(counts[cy * side + cx], 0.25 * mean);
                    if v > 0.0 {
                        // La couleur de la case : celle de sa teinte, éclaircie au
                        // centre (le neutre est blanc).
                        let cb = cx as f32 / (side - 1) as f32 - 0.5;
                        let cr = 0.5 - cy as f32 / (side - 1) as f32;
                        let y = 0.6;
                        let r = y + 1.5748 * cr;
                        let b = y + 1.8556 * cb;
                        let g = (y - 0.2126 * r - 0.0722 * b) / 0.7152;
                        let color = [r, g, b].map(|c| (0.35 + 0.65 * c).clamp(0.0, 1.0));
                        put(&mut out, cy * side + cx, color, v);
                    }
                }
            }
            Some(ScopeImage {
                width: side,
                height: side,
                rgba: out,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: usize, height: usize, f: impl Fn(usize, usize) -> [u8; 3]) -> Vec<u8> {
        let mut out = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let p = f(x, y);
                out.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        out
    }

    fn lit_rows(s: &ScopeImage, left: usize, right: usize) -> Vec<usize> {
        (0..s.height)
            .filter(|y| (left..right).any(|x| s.rgba[(y * s.width + x) * 4 + 3] > 0))
            .collect()
    }

    /// Un gris uniforme : une seule ligne allumée dans la forme d'onde, à sa hauteur ;
    /// un dégradé horizontal monte de gauche à droite.
    #[test]
    fn waveform_follows_luminance() {
        let gray = image(300, 200, |_, _| [128, 128, 128]);
        let s = render(Scope::Waveform, &gray, 300, 200).unwrap();
        assert_eq!((s.width, s.height), (WAVE_COLUMNS, LEVELS));
        let rows = lit_rows(&s, 0, s.width);
        assert_eq!(rows.len(), 1);
        let expected = (waveform_level(128.0 / 255.0) * (LEVELS - 1) as f32).round() as usize;
        assert!(rows[0].abs_diff(expected) <= 1, "{rows:?} {expected}");

        let ramp = image(256, 64, |x, _| [x as u8; 3]);
        let s = render(Scope::Waveform, &ramp, 256, 64).unwrap();
        let left = lit_rows(&s, 0, 4);
        let right = lit_rows(&s, 252, 256);
        assert!(left.iter().all(|r| *r > 120), "{left:?}");
        assert!(right.iter().all(|r| *r < 8), "{right:?}");
    }

    /// Un rouge pur : la couche rouge de la parade est en haut, le vert et le bleu en
    /// bas, chacune dans sa colonne et de sa couleur.
    #[test]
    fn parade_separates_channels() {
        let red = image(120, 80, |_, _| [255, 0, 0]);
        let s = render(Scope::Parade, &red, 120, 80).unwrap();
        let band = |c: usize| {
            let l = c * (PARADE_COLUMNS + PARADE_GAP);
            lit_rows(&s, l, l + PARADE_COLUMNS)
        };
        assert_eq!(band(0), vec![0]);
        assert_eq!(band(1), vec![LEVELS - 1]);
        assert_eq!(band(2), vec![LEVELS - 1]);
        // Rien dans les écarts entre couches.
        let gap = PARADE_COLUMNS;
        assert!(lit_rows(&s, gap, gap + PARADE_GAP).is_empty());
        let p = &s.rgba[(PARADE_COLUMNS / 2) * 4..(PARADE_COLUMNS / 2) * 4 + 4];
        assert!(p[0] > p[1] && p[0] > p[2] && p[3] > 0);
    }

    /// Le gris tombe au centre du vectorscope ; le rouge, le vert et le bleu dans leurs
    /// directions habituelles (rouge en haut à gauche, bleu à droite, vert en bas à
    /// gauche), et plus loin du centre quand ils sont plus saturés.
    #[test]
    fn vectorscope_places_hues() {
        let center = |rgb: [u8; 3]| {
            let img = image(64, 64, |_, _| rgb);
            let s = render(Scope::Vectorscope, &img, 64, 64).unwrap();
            let lit: Vec<(usize, usize)> = (0..s.height)
                .flat_map(|y| (0..s.width).map(move |x| (x, y)))
                .filter(|(x, y)| s.rgba[(y * s.width + x) * 4 + 3] > 0)
                .collect();
            assert_eq!(lit.len(), 1, "{rgb:?}");
            let (x, y) = lit[0];
            let h = (VECTOR_SIDE - 1) as f32 / 2.0;
            (x as f32 - h, y as f32 - h)
        };
        let (gx, gy) = center([90, 90, 90]);
        assert!(gx.abs() <= 1.0 && gy.abs() <= 1.0);
        let (rx, ry) = center([255, 0, 0]);
        assert!(ry < -20.0 && rx < 0.0, "{rx} {ry}");
        let (bx, _) = center([0, 0, 255]);
        assert!(bx > 40.0);
        let (gx, gy) = center([0, 255, 0]);
        assert!(gx < 0.0 && gy > 20.0, "{gx} {gy}");
        let (px, py) = center([160, 110, 110]);
        assert!(px.hypot(py) < rx.hypot(ry));
        // La fonction des repères donne la même place.
        let [u, v] = vectorscope_point([1.0, 0.0, 0.0]);
        let h = (VECTOR_SIDE - 1) as f32;
        assert!((u * h - h / 2.0 - rx).abs() <= 1.0 && (v * h - h / 2.0 - ry).abs() <= 1.0);
    }

    /// Les grandes images sont lues une ligne et une colonne sur deux (ou plus) : le
    /// résultat garde sa forme.
    #[test]
    fn large_images_are_sampled() {
        assert_eq!(step(640, 480), 1);
        assert!(step(4000, 3000) >= 4);
        let w = 3000;
        let h = 2000;
        let img = image(w, h, |x, _| [(x * 255 / w) as u8; 3]);
        let s = render(Scope::Waveform, &img, w, h).unwrap();
        assert!(lit_rows(&s, 0, 4).iter().all(|r| *r > 120));
        assert!(render(Scope::Histogram, &img, w, h).is_none());
        assert!(render(Scope::Waveform, &img[..10], w, h).is_none());
        assert_eq!(Scope::Vectorscope.next(), Scope::Histogram);
    }
}
