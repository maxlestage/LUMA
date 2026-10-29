//! Géométrie : orientation EXIF, recadrage et redressement.
//!
//! Appliqués en premier, sur l'image caméra : tout ce qui suit ne calcule que les
//! pixels gardés, et le vignetage se cale sur le cadre final.

use luma_core::RgbImage;
use luma_project::Crop;
use luma_raw::Orientation;
use rayon::prelude::*;

/// Oriente l'image pour l'affichage (sans copie si elle est déjà droite).
pub fn orient(img: RgbImage, o: Orientation) -> RgbImage {
    let (w, h) = (img.width, img.height);
    let swap = matches!(
        o,
        Orientation::Transpose | Orientation::Rotate90 | Orientation::Transverse | Orientation::Rotate270
    );
    if o == Orientation::Normal {
        return img;
    }
    let (ow, oh) = if swap { (h, w) } else { (w, h) };
    let source = |x: usize, y: usize| {
        let (sx, sy) = match o {
            Orientation::Normal => (x, y),
            Orientation::FlipHorizontal => (w - 1 - x, y),
            Orientation::Rotate180 => (w - 1 - x, h - 1 - y),
            Orientation::FlipVertical => (x, h - 1 - y),
            Orientation::Transpose => (y, x),
            // Rotation de 90° dans le sens horaire pour l'affichage.
            Orientation::Rotate90 => (y, h - 1 - x),
            Orientation::Transverse => (w - 1 - y, h - 1 - x),
            Orientation::Rotate270 => (w - 1 - y, x),
        };
        img.pixel(sx, sy)
    };
    if !swap {
        return RgbImage::from_fn(ow, oh, source);
    }
    // Un quart de tour lit la source en colonnes : par carreaux de 32 × 32, ses lignes
    // restent en cache. Les mêmes pixels, dans un autre ordre.
    const TILE: usize = 32;
    let mut out = RgbImage::new(ow, oh);
    out.data.par_chunks_mut(ow * TILE).enumerate().for_each(|(band, rows)| {
        let y0 = band * TILE;
        for x0 in (0..ow).step_by(TILE) {
            let x1 = (x0 + TILE).min(ow);
            for (dy, row) in rows.chunks_mut(ow).enumerate() {
                for x in x0..x1 {
                    row[x] = source(x, y0 + dy);
                }
            }
        }
    });
    out
}

/// La taille du recadrage dans une image `w × h`.
pub fn crop_size(w: usize, h: usize, c: &Crop) -> (usize, usize) {
    let cw = ((c.right - c.left) * w as f32).round().max(1.0) as usize;
    let ch = ((c.bottom - c.top) * h as f32).round().max(1.0) as usize;
    (cw, ch)
}

/// Recadre, et redresse de `c.angle` degrés autour du centre du cadre. `None` s'il n'y
/// a rien à faire (l'image source sert telle quelle, sans copie).
pub fn crop(img: &RgbImage, c: &Crop) -> Option<RgbImage> {
    if c.is_identity() {
        return None;
    }
    Some(crop_inner(img, c))
}

fn crop_inner(img: &RgbImage, c: &Crop) -> RgbImage {
    let (w, h) = (img.width as f32, img.height as f32);
    let (cw, ch) = crop_size(img.width, img.height, c);
    let x0 = c.left * w;
    let y0 = c.top * h;
    if c.angle == 0.0 {
        let (x0, y0) = (x0.round() as usize, y0.round() as usize);
        let mut out = RgbImage::new(cw, ch);
        out.data.par_chunks_mut(cw).enumerate().for_each(|(y, row)| {
            let sy = (y0 + y).min(img.height - 1);
            for (x, p) in row.iter_mut().enumerate() {
                *p = img.pixel((x0 + x).min(img.width - 1), sy);
            }
        });
        return out;
    }
    let (cx, cy) = (x0 + cw as f32 / 2.0, y0 + ch as f32 / 2.0);
    let (s, co) = c.angle.to_radians().sin_cos();
    RgbImage::from_fn(cw, ch, |x, y| {
        let dx = x as f32 + 0.5 - cw as f32 / 2.0;
        let dy = y as f32 + 0.5 - ch as f32 / 2.0;
        img.sample(cx + dx * co - dy * s - 0.5, cy + dx * s + dy * co - 0.5)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les quarts de tour par carreaux donnent exactement l'image du calcul pixel par
    /// pixel, pour les huit orientations et des tailles qui ne tombent pas juste sur les
    /// carreaux.
    #[test]
    fn tiled_orientation_gives_the_same_pixels() {
        use Orientation as O;
        for (w, h) in [(1, 1), (5, 3), (70, 33), (129, 64)] {
            let img = RgbImage::from_fn(w, h, |x, y| [x as f32, y as f32, (x * 7 + y) as f32]);
            for o in [
                O::Normal,
                O::FlipHorizontal,
                O::Rotate180,
                O::FlipVertical,
                O::Transpose,
                O::Rotate90,
                O::Transverse,
                O::Rotate270,
            ] {
                let swap = matches!(o, O::Transpose | O::Rotate90 | O::Transverse | O::Rotate270);
                let (ow, oh) = if swap { (h, w) } else { (w, h) };
                let expected = RgbImage::from_fn(ow, oh, |x, y| {
                    let (sx, sy) = match o {
                        O::Normal => (x, y),
                        O::FlipHorizontal => (w - 1 - x, y),
                        O::Rotate180 => (w - 1 - x, h - 1 - y),
                        O::FlipVertical => (x, h - 1 - y),
                        O::Transpose => (y, x),
                        O::Rotate90 => (y, h - 1 - x),
                        O::Transverse => (w - 1 - y, h - 1 - x),
                        O::Rotate270 => (w - 1 - y, x),
                    };
                    img.pixel(sx, sy)
                });
                let got = orient(img.clone(), o);
                assert!(
                    got.data == expected.data && (got.width, got.height) == (ow, oh),
                    "{w}×{h} {o:?}"
                );
            }
        }
    }

    fn numbered() -> RgbImage {
        RgbImage::from_fn(3, 2, |x, y| [(y * 3 + x) as f32, 0.0, 0.0])
    }

    #[test]
    fn orientations() {
        let img = numbered();
        let r = orient(img.clone(), Orientation::Rotate90);
        assert_eq!((r.width, r.height), (2, 3));
        // Sens horaire : le coin bas‑gauche passe en haut à gauche.
        assert_eq!(r.pixel(0, 0)[0], 3.0);
        assert_eq!(r.pixel(1, 0)[0], 0.0);
        let full_turn = (0..4).fold(img.clone(), |acc, _| orient(acc, Orientation::Rotate90));
        assert_eq!(full_turn, img);
        assert_eq!(orient(r, Orientation::Rotate270), img);
        assert_eq!(
            orient(orient(img.clone(), Orientation::Transpose), Orientation::Transpose),
            img
        );
    }

    #[test]
    fn crops_without_rotation() {
        let img = RgbImage::from_fn(10, 10, |x, y| [x as f32, y as f32, 0.0]);
        let c = Crop {
            left: 0.2,
            top: 0.5,
            right: 0.6,
            bottom: 1.0,
            angle: 0.0,
        };
        let out = crop(&img, &c).unwrap();
        assert_eq!((out.width, out.height), (4, 5));
        assert_eq!(out.pixel(0, 0), [2.0, 5.0, 0.0]);
    }

    #[test]
    fn small_rotation_keeps_the_center() {
        let img = RgbImage::from_fn(41, 41, |x, y| [x as f32, y as f32, 0.0]);
        let c = Crop {
            left: 0.25,
            top: 0.25,
            right: 0.75,
            bottom: 0.75,
            angle: 5.0,
        };
        let out = crop(&img, &c).unwrap();
        let center = out.pixel(out.width / 2, out.height / 2);
        assert!(
            (center[0] - 20.5).abs() < 1.0 && (center[1] - 20.5).abs() < 1.0,
            "{center:?}"
        );
    }
}
