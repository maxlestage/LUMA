//! Les zones nettes d'un aperçu (le « focus peaking » des boîtiers), sans IA : là où la
//! luminance, à peine lissée pour que le bruit ne compte pas, change brusquement d'un
//! pixel à ses voisins (le laplacien). Le seuil est absolu : une photo floue n'a aucune
//! zone nette, une photo nette les a sur ses détails au point.

use rayon::prelude::*;

/// L'écart de luminance (0 à 255, laplacien d'un lissage 3 × 3) au‑delà duquel un
/// pixel compte comme net : celui de la sensibilité moyenne, et de la mesure de la
/// part au point (« Retenir la plus nette »), pour que les photos se comparent.
pub const THRESHOLD: f32 = 14.0;

/// Les lignes d'une bande de [`sharp_above`], menée d'un coup sur un cœur.
const BAND: usize = 32;

/// Une photo dont la part au point est sous cette fraction de celle de la plus nette
/// d'un lot compte comme floue.
pub const BLURRED_FRACTION: f32 = 1.0 / 3.0;

/// La taille (grand côté, en pixels) de l'aperçu sur lequel se mesure la part au point :
/// la même pour toutes les photos, pour qu'elles se comparent.
pub const SHARE_SIDE: usize = 1600;

/// La part au point (0 à 1) d'une image RGBA de `w × h` : la part de ses pixels nets
/// (voir [`sharp`]).
pub fn share(rgba: &[u8], w: usize, h: usize) -> f32 {
    let n = sharp(rgba, w, h).iter().filter(|&&m| m).count();
    n as f32 / (w * h).max(1) as f32
}

/// La sensibilité des zones nettes : haute, les détails fins d'un objectif doux comptent
/// aussi ; basse, seuls les contours les plus francs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sensitivity {
    Low,
    #[default]
    Medium,
    High,
}

impl Sensitivity {
    pub const ALL: [Sensitivity; 3] = [Sensitivity::Low, Sensitivity::Medium, Sensitivity::High];

    /// Le seuil de cette sensibilité.
    pub fn threshold(self) -> f32 {
        match self {
            Sensitivity::Low => 22.0,
            Sensitivity::Medium => THRESHOLD,
            Sensitivity::High => 8.0,
        }
    }
}

/// La couleur des zones nettes par défaut : un vert vif, qu'on ne confond pas avec
/// l'écrêtage (rouge et bleu).
pub const COLOR: [u8; 3] = [40, 255, 90];

/// La couleur dont on peint les zones nettes : le vert par défaut ; le jaune, le rouge ou
/// le blanc quand le vert se perd dans la photo (un feuillage, une prairie).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Color {
    #[default]
    Green,
    Yellow,
    Red,
    White,
}

impl Color {
    pub const ALL: [Color; 4] = [Color::Green, Color::Yellow, Color::Red, Color::White];

    /// Ses valeurs RVB 8 bits.
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Color::Green => COLOR,
            Color::Yellow => [255, 230, 0],
            Color::Red => [255, 30, 30],
            Color::White => [255, 255, 255],
        }
    }
}

/// L'allure des zones nettes : leur sensibilité et leur couleur.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Look {
    pub sensitivity: Sensitivity,
    pub color: Color,
}

/// Les zones nettes de `source` (même taille, RGBA) peintes sur `dest` avec cette
/// allure ; le nombre de pixels peints.
pub fn paint_look(dest: &mut [u8], source: &[u8], w: usize, h: usize, look: Look) -> usize {
    let mask = sharp_above(source, w, h, look.sensitivity.threshold());
    paint_mask(dest, &mask, look.color)
}

/// Les pixels marqués de `mask` peints de cette couleur sur `dest` (RGBA, l'alpha gardé) ;
/// leur nombre.
pub fn paint_mask(dest: &mut [u8], mask: &[bool], color: Color) -> usize {
    let rgb = color.rgb();
    let mut n = 0;
    for (p, &m) in dest.as_chunks_mut::<4>().0.iter_mut().zip(mask) {
        if m {
            p[..3].copy_from_slice(&rgb);
            n += 1;
        }
    }
    n
}

/// La luminance (0 à 255) d'un pixel 8 bits encodé.
#[inline]
fn luma(p: &[u8]) -> f32 {
    0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2])
}

/// Les pixels nets d'une image RGBA de `w × h` (8 bits) : `true` là où le détail est au
/// point. Les deux pixels du bord ne comptent jamais.
pub fn sharp(rgba: &[u8], w: usize, h: usize) -> Vec<bool> {
    sharp_above(rgba, w, h, THRESHOLD)
}

/// [`sharp`] avec un autre seuil (voir [`Sensitivity`]).
pub fn sharp_above(rgba: &[u8], w: usize, h: usize, threshold: f32) -> Vec<bool> {
    let mut out = vec![false; w * h];
    if w < 5 || h < 5 || rgba.len() < w * h * 4 {
        return out;
    }
    // Par bandes de lignes, chacune sur un cœur avec ses petits tampons (la luminance et
    // la luminance lissée 3 × 3 de ses lignes et de leurs voisines), qui restent dans le
    // cache : trois images entières en mémoire coûtaient plus que le calcul. Les mêmes
    // opérations dans le même ordre, donc les mêmes pixels.
    out.par_chunks_mut(w * BAND).enumerate().for_each(|(i, band)| {
        let y0 = i * BAND;
        // Les lignes marquées de la bande (les deux du bord ne comptent jamais).
        let (ya, yb) = (y0.max(2), (y0 + band.len() / w).min(h - 2));
        if ya >= yb {
            return;
        }
        // La luminance des lignes ya − 2 à yb + 1 : le lissage du bruit d'un pixel.
        let first = ya - 2;
        let lum: Vec<f32> = rgba[first * w * 4..(yb + 2) * w * 4]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| luma(p))
            .collect();
        let line = |y: usize| &lum[(y - first) * w..(y - first + 1) * w];
        // Lissée, lignes ya − 1 à yb ; les sommes des trois lignes une fois par colonne.
        let top = ya - 1;
        let mut smooth = vec![0.0f32; (yb + 1 - top) * w];
        let mut col = vec![0.0f32; w];
        for (r, row) in (top..=yb).zip(smooth.chunks_exact_mut(w)) {
            let (a, b, c) = (line(r - 1), line(r), line(r + 1));
            for (i, v) in col.iter_mut().enumerate() {
                *v = a[i] + b[i] + c[i];
            }
            for (x, v) in row.iter_mut().enumerate().take(w - 1).skip(1) {
                *v = (col[x - 1] + col[x] + col[x + 1]) / 9.0;
            }
        }
        let smoothed = |y: usize| &smooth[(y - top) * w..(y - top + 1) * w];
        for y in ya..yb {
            let (up, mid, down) = (smoothed(y - 1), smoothed(y), smoothed(y + 1));
            let row = &mut band[(y - y0) * w..(y - y0 + 1) * w];
            for (x, v) in row.iter_mut().enumerate().take(w - 2).skip(2) {
                let lap = 4.0 * mid[x] - mid[x - 1] - mid[x + 1] - up[x] - down[x];
                *v = lap.abs() > threshold;
            }
        }
    });
    out
}

/// Les zones nettes peintes par‑dessus l'image RGBA (voir [`sharp`]) ; le nombre de
/// pixels peints.
pub fn paint(rgba: &mut [u8], w: usize, h: usize) -> usize {
    let mask = sharp(rgba, w, h);
    paint_mask(rgba, &mask, Color::Green)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une image RGBA de `w × h` d'après une fonction de la luminance.
    fn image(w: usize, h: usize, f: impl Fn(usize, usize) -> u8) -> Vec<u8> {
        let mut v = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let l = f(x, y);
                v.extend_from_slice(&[l, l, l, 255]);
            }
        }
        v
    }

    /// Un damier net à gauche, le même flouté à droite : les zones nettes sont à
    /// gauche, aucune à droite.
    #[test]
    fn a_sharp_half_and_a_blurred_half() {
        let (w, h) = (160, 80);
        let checker = |x: usize, y: usize| {
            if (x / 6 + y / 6).is_multiple_of(2) {
                40.0f32
            } else {
                210.0
            }
        };
        // Le flou : la moyenne sur 13 × 13 pixels.
        let blurred = |x: usize, y: usize| {
            let mut s = 0.0;
            for dy in 0..13 {
                for dx in 0..13 {
                    s += checker(x + dx, y + dy);
                }
            }
            s / 169.0
        };
        let img = image(w, h, |x, y| {
            if x < w / 2 {
                checker(x, y) as u8
            } else {
                blurred(x, y) as u8
            }
        });
        let mask = sharp(&img, w, h);
        let count = |x0: usize, x1: usize| {
            (0..h)
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .filter(|&(x, y)| mask[y * w + x])
                .count()
        };
        let (left, right) = (count(4, w / 2 - 4), count(w / 2 + 4, w - 4));
        assert!(left > 1500, "{left}");
        assert_eq!(right, 0);
    }

    /// Un dégradé doux et une image unie : rien de net ; une petite image : rien, sans
    /// paniquer.
    #[test]
    fn smooth_images_have_no_sharp_zone() {
        let (w, h) = (120, 90);
        let ramp = image(w, h, |x, y| (x + y) as u8);
        assert!(!sharp(&ramp, w, h).contains(&true));
        let flat = image(w, h, |_, _| 128);
        assert!(!sharp(&flat, w, h).contains(&true));
        let tiny = image(3, 3, |x, _| (x * 100) as u8);
        assert_eq!(sharp(&tiny, 3, 3), vec![false; 9]);
    }

    /// Le bruit fin d'un pixel (±10 sur 255) ne fait pas de zone nette.
    #[test]
    fn fine_noise_is_not_sharp() {
        let (w, h) = (120, 90);
        let noisy = image(w, h, |x, y| (128 + ((x * 7919 + y * 104_729) % 21) as i32 - 10) as u8);
        let n = sharp(&noisy, w, h).iter().filter(|&&m| m).count();
        assert!(n < w * h / 100, "{n}");
    }

    /// Plus sensible, plus de zones nettes : la basse sensibilité en marque moins que la
    /// moyenne, la haute plus, et chacune contient la précédente.
    #[test]
    fn a_higher_sensitivity_marks_more() {
        let (w, h) = (160, 80);
        // Des marches de contraste croissant : chaque seuil en garde une de plus.
        let img = image(w, h, |x, y| {
            let step = (x / 40) as u8 + 1;
            if (x / 4 + y / 4) % 2 == 0 { 120 } else { 120 + 6 * step }
        });
        let count = |s: Sensitivity| sharp_above(&img, w, h, s.threshold());
        let [low, medium, high] = Sensitivity::ALL.map(count);
        let n = |m: &Vec<bool>| m.iter().filter(|&&v| v).count();
        assert!(
            n(&low) < n(&medium) && n(&medium) < n(&high),
            "{} {} {}",
            n(&low),
            n(&medium),
            n(&high)
        );
        assert!(low.iter().zip(&medium).all(|(a, b)| !a || *b));
        assert!(medium.iter().zip(&high).all(|(a, b)| !a || *b));
        assert_eq!(sharp(&img, w, h), medium);
    }

    /// Le calcul par bandes donne exactement les pixels du calcul d'origine (trois images
    /// entières : luminance, luminance lissée, laplacien), sur des tailles qui tombent
    /// juste ou non sur les bandes, jusqu'à la plus petite, à chaque sensibilité.
    #[test]
    fn bands_give_the_same_pixels_as_whole_images() {
        fn whole(rgba: &[u8], w: usize, h: usize, threshold: f32) -> Vec<bool> {
            let mut out = vec![false; w * h];
            let lum: Vec<f32> = rgba.as_chunks::<4>().0.iter().map(|p| luma(p)).collect();
            let mut smooth = vec![0.0f32; w * h];
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let s: f32 = (x - 1..=x + 1)
                        .map(|i| lum[(y - 1) * w + i] + lum[y * w + i] + lum[(y + 1) * w + i])
                        .sum();
                    smooth[y * w + x] = s / 9.0;
                }
            }
            for y in 2..h - 2 {
                for x in 2..w - 2 {
                    let m = |y: usize, x: usize| smooth[y * w + x];
                    let lap = 4.0 * m(y, x) - m(y, x - 1) - m(y, x + 1) - m(y - 1, x) - m(y + 1, x);
                    out[y * w + x] = lap.abs() > threshold;
                }
            }
            out
        }
        for (w, h) in [(5, 5), (7, 40), (64, BAND), (33, BAND + 1), (100, 3 * BAND + 5)] {
            let img = image(w, h, |x, y| ((x * 7919 + y * 104_729 + x * y * 31) % 251) as u8);
            for s in Sensitivity::ALL {
                let t = s.threshold();
                assert_eq!(sharp_above(&img, w, h, t), whole(&img, w, h, t), "{w} × {h}, {t}");
            }
        }
    }

    /// Chaque couleur peint les mêmes pixels (ceux de la sensibilité), de sa couleur ;
    /// les couleurs sont distinctes, la verte est celle par défaut.
    #[test]
    fn each_color_paints_the_same_pixels() {
        let (w, h) = (60, 40);
        let img = image(w, h, |x, y| if (x / 5 + y / 5).is_multiple_of(2) { 40 } else { 200 });
        let mask = sharp_above(&img, w, h, Sensitivity::High.threshold());
        let mut seen = Vec::new();
        for color in Color::ALL {
            let mut out = img.clone();
            let look = Look {
                sensitivity: Sensitivity::High,
                color,
            };
            let n = paint_look(&mut out, &img, w, h, look);
            assert_eq!(n, mask.iter().filter(|&&m| m).count());
            for (i, p) in out.as_chunks::<4>().0.iter().enumerate() {
                if mask[i] {
                    assert_eq!(p[..3], color.rgb());
                } else {
                    assert_eq!(p, &img[i * 4..i * 4 + 4]);
                }
            }
            assert!(!seen.contains(&color.rgb()));
            seen.push(color.rgb());
        }
        assert_eq!(Look::default().color.rgb(), COLOR);
        assert_eq!(Look::default().sensitivity, Sensitivity::Medium);
    }

    /// La peinture : les pixels nets en vert, les autres intacts, l'alpha gardé.
    #[test]
    fn painting_marks_only_the_sharp_pixels() {
        let (w, h) = (60, 40);
        let mut img = image(w, h, |x, _| if x < 30 { 30 } else { 220 });
        let before = img.clone();
        let mask = sharp(&img, w, h);
        let n = paint(&mut img, w, h);
        assert_eq!(n, mask.iter().filter(|&&m| m).count());
        assert!(n > 0);
        for (i, (p, q)) in img.as_chunks::<4>().0.iter().zip(before.as_chunks::<4>().0).enumerate() {
            if mask[i] {
                assert_eq!(&p[..3], &COLOR);
            } else {
                assert_eq!(p, q);
            }
            assert_eq!(p[3], 255);
        }
    }
}
