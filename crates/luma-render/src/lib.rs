//! Le moteur de rendu : une photo ouverte, ses images sources en cache, et le rendu des
//! réglages à la taille demandée.
//!
//! Ce qui rend un éditeur réactif, c'est de ne jamais refaire deux fois le même
//! travail. Le décodage et le dématriçage sont faits une fois par taille et gardés en
//! mémoire ; quand un curseur bouge, seul le développement est rejoué, à la taille de
//! l'écran. Pour un aperçu, la mosaïque est réduite sans dématriçage (un pixel par
//! bloc du capteur), ce qui est bien plus rapide et aussi net à cette taille.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use luma_color::TempTint;
use luma_core::{Cancel, Result, RgbImage, Timings};
use luma_image::{Input, geometry};
use luma_project::{Settings, WhiteBalance};
use luma_raw::RawPhoto;

/// La taille d'un rendu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    /// Le plus grand côté de l'image entière (avant recadrage) tient dans ce nombre de
    /// pixels : aperçus, vignettes, exports réduits.
    Fit(usize),
    /// Pleine définition, dématriçage complet : export.
    Full,
}

/// Un rendu : l'image (Rec. 2020 linéaire, prête à encoder) et le temps de chaque étape.
pub struct Rendered {
    pub image: RgbImage,
    pub timings: Timings,
}

/// Une photo ouverte dans le moteur.
pub struct Photo {
    raw: RawPhoto,
    /// Images caméra orientées, par taille. Bornées : au plus [`CACHE`] tailles.
    sources: Mutex<Vec<(Size, Arc<RgbImage>)>>,
    /// Les matrices caméra → travail déjà calculées, par balance des blancs.
    matrices: Mutex<HashMap<[u32; 3], luma_color::Mat3>>,
}

const CACHE: usize = 3;

impl Photo {
    pub fn open(path: impl AsRef<Path>) -> Result<Photo> {
        Ok(Self::new(RawPhoto::open(path)?))
    }

    pub fn new(raw: RawPhoto) -> Photo {
        Photo {
            raw,
            sources: Mutex::new(Vec::new()),
            matrices: Mutex::new(HashMap::new()),
        }
    }

    pub fn raw(&self) -> &RawPhoto {
        &self.raw
    }

    /// La taille de l'image orientée, pleine définition.
    pub fn full_size(&self) -> (usize, usize) {
        let (w, h) = (self.raw.width, self.raw.height);
        use luma_raw::Orientation as O;
        match self.raw.orientation {
            O::Transpose | O::Rotate90 | O::Transverse | O::Rotate270 => (h, w),
            _ => (w, h),
        }
    }

    /// La balance du boîtier.
    pub fn as_shot(&self) -> TempTint {
        self.raw.as_shot()
    }

    /// L'image caméra orientée à la taille demandée (calculée une fois).
    pub fn source(&self, size: Size, t: &mut Timings) -> Arc<RgbImage> {
        if let Some((_, img)) = self.sources.lock().expect("cache").iter().find(|(s, _)| *s == size) {
            return img.clone();
        }
        let (fw, fh) = self.full_size();
        let img = match size {
            Size::Full => {
                let full = t.time("dématriçage", || self.raw.demosaic());
                t.time("orientation", || geometry::orient(full, self.raw.orientation))
            }
            Size::Fit(max) => {
                let (tw, th) = RgbImage::fit(fw, fh, max);
                let (bw, bh) = self.raw.binned_size();
                // La réduction sans dématriçage suffit tant qu'elle reste plus grande que
                // la cible ; au‑delà, on dématrice.
                let base = if bw.max(bh) >= tw.max(th) {
                    t.time("réduction capteur", || self.raw.binned())
                } else {
                    t.time("dématriçage", || self.raw.demosaic())
                };
                let oriented = t.time("orientation", || geometry::orient(base, self.raw.orientation));
                t.time("mise à l'échelle", || oriented.resize_area(tw, th))
            }
        };
        let img = Arc::new(img);
        let mut cache = self.sources.lock().expect("cache");
        if cache.len() >= CACHE {
            cache.remove(0);
        }
        cache.push((size, img.clone()));
        img
    }

    fn camera_to_working(&self, wb: &WhiteBalance) -> luma_color::Mat3 {
        let neutral = match *wb {
            WhiteBalance::AsShot => self.raw.as_shot_neutral,
            WhiteBalance::Custom { temperature, tint } => self.raw.neutral_for(TempTint { temperature, tint }),
        };
        let key = neutral.map(f32::to_bits);
        if let Some(m) = self.matrices.lock().expect("matrices").get(&key) {
            return *m;
        }
        let m = self.raw.camera_to_working(neutral);
        self.matrices.lock().expect("matrices").insert(key, m);
        m
    }

    /// Développe les réglages à la taille demandée. `cancel` arrête le calcul entre
    /// deux étapes si l'utilisateur a déjà changé d'avis.
    pub fn render(&self, settings: &Settings, size: Size, cancel: &Cancel) -> Result<Rendered> {
        let mut timings = Timings::default();
        let source = self.source(size, &mut timings);
        cancel.check()?;
        let cropped = timings.time("recadrage", || geometry::crop(&source, &settings.crop));
        let cam = cropped.as_ref().unwrap_or(&source);
        let input = Input {
            camera_to_working: self.camera_to_working(&settings.white_balance),
            exposure_bias: self.raw.baseline_exposure,
            scale: source.width as f32 / self.full_size().0 as f32,
        };
        let image = luma_image::develop(cam, &input, settings, cancel, &mut timings)?;
        Ok(Rendered { image, timings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use luma_raw::{Cfa, Exif, Mosaic, Orientation};

    fn photo() -> Photo {
        let m = luma_color::inverse(&luma_color::rgb_to_xyz(&luma_color::SRGB)).unwrap();
        let (w, h) = (64, 48);
        let cfa = Cfa::parse("RGGB", 2, 2).unwrap();
        let data = (0..w * h).map(|i| [0.2, 0.25, 0.15][cfa.color(i % w, i / w)]).collect();
        Photo::new(RawPhoto {
            make: "Test".into(),
            model: "Synthétique".into(),
            width: w,
            height: h,
            mosaic: Mosaic::Bayer(cfa),
            data,
            as_shot_neutral: [0.8, 1.0, 0.6],
            matrices: vec![(6504.0, m)],
            orientation: Orientation::Rotate90,
            baseline_exposure: 0.0,
            exif: Exif::default(),
        })
    }

    #[test]
    fn sources_are_cached_and_oriented() {
        let p = photo();
        assert_eq!(p.full_size(), (48, 64));
        let mut t = Timings::default();
        let a = p.source(Size::Fit(32), &mut t);
        assert_eq!((a.width, a.height), (24, 32));
        let steps = t.steps.len();
        let b = p.source(Size::Fit(32), &mut t);
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(t.steps.len(), steps, "rien n'est recalculé");
    }

    #[test]
    fn renders_at_every_size() {
        let p = photo();
        let s = Settings::default();
        let preview = p.render(&s, Size::Fit(32), &Cancel::new()).unwrap();
        assert_eq!((preview.image.width, preview.image.height), (24, 32));
        let full = p.render(&s, Size::Full, &Cancel::new()).unwrap();
        assert_eq!((full.image.width, full.image.height), (48, 64));
        // Même rendu au centre, quelle que soit la taille.
        let (a, b) = (preview.image.pixel(12, 16), full.image.pixel(24, 32));
        assert!((0..3).all(|c| (a[c] - b[c]).abs() < 0.01), "{a:?} {b:?}");
    }

    #[test]
    fn as_shot_neutral_renders_gray() {
        let p = photo();
        let flat = p.render(&Settings::default(), Size::Fit(32), &Cancel::new()).unwrap();
        // Les valeurs du capteur (0,2 ; 0,25 ; 0,15) sont proportionnelles à son neutre
        // (0,8 ; 1 ; 0,6) : la photo est un gris, elle doit sortir grise.
        let px = flat.image.pixel(10, 10);
        assert!((px[0] - px[1]).abs() < 0.005 && (px[1] - px[2]).abs() < 0.005, "{px:?}");
        // Une autre balance la colore.
        let warm = Settings {
            white_balance: WhiteBalance::Custom {
                temperature: 9000.0,
                tint: 0.0,
            },
            ..Settings::default()
        };
        let px = p
            .render(&warm, Size::Fit(32), &Cancel::new())
            .unwrap()
            .image
            .pixel(10, 10);
        assert!(px[0] > px[2] + 0.02, "plus chaud : {px:?}");
    }
}
