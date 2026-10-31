//! Les zones nettes d'un aperçu (le « focus peaking » des boîtiers), sans IA : là où la
//! luminance, à peine lissée pour que le bruit ne compte pas, change brusquement d'un
//! pixel à ses voisins (le laplacien). Le seuil est absolu : une photo floue n'a aucune
//! zone nette, une photo nette les a sur ses détails au point.

use rayon::prelude::*;

/// L'écart de luminance (0 à 255, laplacien d'un lissage 3 × 3) au‑delà duquel un
/// pixel compte comme net.
pub const THRESHOLD: f32 = 14.0;

/// La couleur des zones nettes : un vert vif, qu'on ne confond pas avec l'écrêtage
/// (rouge et bleu).
pub const COLOR: [u8; 3] = [40, 255, 90];

/// La luminance (0 à 255) d'un pixel 8 bits encodé.
#[inline]
fn luma(p: &[u8]) -> f32 {
    0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2])
}

/// Les pixels nets d'une image RGBA de `w × h` (8 bits) : `true` là où le détail est au
/// point. Les deux pixels du bord ne comptent jamais.
pub fn sharp(rgba: &[u8], w: usize, h: usize) -> Vec<bool> {
    let mut out = vec![false; w * h];
    if w < 5 || h < 5 || rgba.len() < w * h * 4 {
        return out;
    }
    // La luminance lissée 3 × 3 : le bruit d'un pixel ne fait pas une zone nette.
    let lum: Vec<f32> = rgba.as_chunks::<4>().0.par_iter().map(|p| luma(p)).collect();
    let mut smooth = vec![0.0f32; w * h];
    smooth
        .par_chunks_mut(w)
        .enumerate()
        .skip(1)
        .take(h - 2)
        .for_each(|(y, row)| {
            let (a, b, c) = (
                &lum[(y - 1) * w..y * w],
                &lum[y * w..(y + 1) * w],
                &lum[(y + 1) * w..(y + 2) * w],
            );
            for (x, v) in row.iter_mut().enumerate().take(w - 1).skip(1) {
                let s: f32 = (x - 1..=x + 1).map(|i| a[i] + b[i] + c[i]).sum();
                *v = s / 9.0;
            }
        });
    out.par_chunks_mut(w)
        .enumerate()
        .skip(2)
        .take(h - 4)
        .for_each(|(y, row)| {
            let (up, mid, down) = (
                &smooth[(y - 1) * w..y * w],
                &smooth[y * w..(y + 1) * w],
                &smooth[(y + 1) * w..(y + 2) * w],
            );
            for (x, v) in row.iter_mut().enumerate().take(w - 2).skip(2) {
                let lap = 4.0 * mid[x] - mid[x - 1] - mid[x + 1] - up[x] - down[x];
                *v = lap.abs() > THRESHOLD;
            }
        });
    out
}

/// Les zones nettes peintes par‑dessus l'image RGBA (voir [`sharp`]) ; le nombre de
/// pixels peints.
pub fn paint(rgba: &mut [u8], w: usize, h: usize) -> usize {
    let mask = sharp(rgba, w, h);
    let mut n = 0;
    for (p, &m) in rgba.as_chunks_mut::<4>().0.iter_mut().zip(&mask) {
        if m {
            p[..3].copy_from_slice(&COLOR);
            n += 1;
        }
    }
    n
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
