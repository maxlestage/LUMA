//! Les photos à fusionner, chargées à la demande (une pose de 24 Mpx occupe près de
//! 300 Mo en flottants : on ne les garde pas toutes en mémoire quand on peut l'éviter),
//! et une forme compacte (16 bits) pour celles qu'il faut garder ensemble.

use std::path::PathBuf;
use std::sync::Arc;

use luma_core::{Result, RgbImage};
use luma_render::{Photo, Size};

/// Une suite de poses (couleurs caméra linéaires, orientées, toutes de même taille).
pub trait Frames: Sync {
    fn len(&self) -> usize;
    fn load(&self, index: usize) -> Result<Arc<RgbImage>>;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Des images déjà en mémoire (tests, petites fusions).
impl Frames for Vec<Arc<RgbImage>> {
    fn len(&self) -> usize {
        self.as_slice().len()
    }
    fn load(&self, index: usize) -> Result<Arc<RgbImage>> {
        Ok(self[index].clone())
    }
}

/// Des RAW sur le disque, relus à chaque passe.
pub struct RawFiles {
    pub paths: Vec<PathBuf>,
    pub size: Size,
}

impl Frames for RawFiles {
    fn len(&self) -> usize {
        self.paths.len()
    }
    fn load(&self, index: usize) -> Result<Arc<RgbImage>> {
        // Une photo ouverte pour l'occasion : son cache disparaît avec elle.
        let photo = Photo::open(&self.paths[index])?;
        Ok(photo.source(self.size, &mut luma_core::Timings::default()))
    }
}

/// Une image en 16 bits par couche (6 octets par pixel au lieu de 12).
pub struct Compact {
    pub width: usize,
    pub height: usize,
    data: Vec<[u16; 3]>,
}

impl Compact {
    pub fn new(img: &RgbImage) -> Compact {
        Compact {
            width: img.width,
            height: img.height,
            data: img
                .data
                .iter()
                .map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 65535.0).round() as u16))
                .collect(),
        }
    }

    #[inline]
    pub fn pixel(&self, x: usize, y: usize) -> [f32; 3] {
        self.data[y * self.width + x].map(|v| f32::from(v) / 65535.0)
    }

    /// Interpolation bilinéaire ; `None` hors de l'image.
    #[inline]
    pub fn sample(&self, x: f64, y: f64) -> Option<[f32; 3]> {
        sample_with(self.width, self.height, x, y, |x, y| self.pixel(x, y))
    }
}

/// Interpolation bilinéaire d'une image `w × h` lue par `px` ; `None` hors de l'image.
#[inline]
pub fn sample_with(w: usize, h: usize, x: f64, y: f64, px: impl Fn(usize, usize) -> [f32; 3]) -> Option<[f32; 3]> {
    if x < -0.5 || y < -0.5 || x > w as f64 - 0.5 || y > h as f64 - 0.5 {
        return None;
    }
    let x = x.clamp(0.0, (w - 1) as f64);
    let y = y.clamp(0.0, (h - 1) as f64);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
    let (a, b, c, d) = (px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1));
    let mut out = [0.0; 3];
    for i in 0..3 {
        let top = a[i] + (b[i] - a[i]) * fx;
        let bottom = c[i] + (d[i] - c[i]) * fx;
        out[i] = top + (bottom - top) * fy;
    }
    Some(out)
}

/// Interpolation bilinéaire dans une image en flottants ; `None` hors de l'image.
#[inline]
pub fn sample(img: &RgbImage, x: f64, y: f64) -> Option<[f32; 3]> {
    sample_with(img.width, img.height, x, y, |x, y| img.data[y * img.width + x])
}
