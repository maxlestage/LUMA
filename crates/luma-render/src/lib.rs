//! Le moteur de rendu : une photo ouverte, ses images sources en cache, et le rendu des
//! réglages à la taille demandée.
//!
//! Ce qui rend un éditeur réactif, c'est de ne jamais refaire deux fois le même
//! travail. Le décodage et le dématriçage sont faits une fois par taille et gardés en
//! mémoire ; quand un curseur bouge, seul le développement est rejoué, à la taille de
//! l'écran. Pour un aperçu, la mosaïque est réduite sans dématriçage (un pixel par
//! bloc du capteur), ce qui est bien plus rapide et aussi net à cette taille.

pub mod export;

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

/// Le résultat des étapes de voisinage ([`luma_image::develop_base`]), gardé en cache :
/// tant que seuls le contraste, les courbes, les couleurs ou les effets changent, il
/// resert tel quel.
#[derive(Clone)]
pub struct Base {
    pub image: Arc<RgbImage>,
    /// Pixels rendus par pixel pleine définition.
    pub scale: f32,
    /// Change à chaque nouvelle base (pour savoir quand la renvoyer au GPU).
    pub id: u64,
}

/// Une photo ouverte dans le moteur.
pub struct Photo {
    /// Les dernières bases (la plus récente en tête) : taille, clé
    /// ([`luma_image::base_key`]) et image. Plusieurs : celle de la photo et celles des
    /// films de masques qui ont leurs propres hautes lumières, ombres ou clarté.
    bases: Mutex<Vec<(Size, Settings, Base)>>,
    raw: RawPhoto,
    /// Images caméra orientées, par taille. Bornées : au plus [`CACHE`] tailles.
    sources: Mutex<Vec<(Size, Arc<RgbImage>)>>,
    /// L'aberration chromatique latérale estimée (rouge, bleu), une fois par photo.
    ca: Mutex<Option<(f32, f32)>>,
    /// Les matrices caméra → travail déjà calculées, par balance des blancs.
    matrices: Mutex<HashMap<[u32; 3], luma_color::Mat3>>,
}

const CACHE: usize = 3;

/// Bases gardées : la photo et jusqu'à trois films de masques.
const BASES: usize = 4;

impl Photo {
    pub fn open(path: impl AsRef<Path>) -> Result<Photo> {
        Ok(Self::new(RawPhoto::open(path)?))
    }

    pub fn new(raw: RawPhoto) -> Photo {
        Photo {
            bases: Mutex::new(Vec::new()),
            ca: Mutex::new(None),
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
        let base = self.base(settings, size, cancel, &mut timings)?;
        let image = luma_image::finish_layers(
            &base.image,
            base.scale,
            settings,
            &mut |look, t| self.base(look, size, cancel, t).map(|b| b.image),
            cancel,
            &mut timings,
        )?;
        Ok(Rendered { image, timings })
    }

    /// La balance qui rend neutre une couleur caméra (pipette, balance automatique).
    pub fn white_balance_for(&self, camera: [f32; 3]) -> TempTint {
        let g = camera[1].max(1e-6);
        let neutral = [camera[0].max(1e-6) / g, 1.0, camera[2].max(1e-6) / g];
        let t = luma_color::xy_to_temp_tint(self.raw.neutral_to_xy(neutral));
        TempTint {
            temperature: t.temperature.clamp(2000.0, 50000.0).round(),
            tint: t.tint.clamp(-150.0, 150.0).round(),
        }
    }

    /// Balance automatique : la moyenne des tons proches du gris (ni sombres, ni
    /// écrêtés, ni très colorés), affinée trois fois à partir de la balance du boîtier.
    pub fn auto_white_balance(&self) -> TempTint {
        let src = self.source(Size::Fit(512), &mut Timings::default());
        let mut neutral = self.raw.as_shot_neutral;
        for _ in 0..3 {
            let mut sum = [0.0f64; 3];
            let mut n = 0usize;
            for p in &src.data {
                let top = p[0].max(p[1]).max(p[2]);
                if !(0.02..0.9).contains(&top) {
                    continue;
                }
                let (r, b) = (p[0] / neutral[0] / p[1].max(1e-6), p[2] / neutral[2] / p[1].max(1e-6));
                if r.ln().abs() < 0.35 && b.ln().abs() < 0.35 {
                    for c in 0..3 {
                        sum[c] += f64::from(p[c]);
                    }
                    n += 1;
                }
            }
            if n < 50 || sum[1] <= 0.0 {
                break;
            }
            neutral = [(sum[0] / sum[1]) as f32, 1.0, (sum[2] / sum[1]) as f32];
        }
        self.white_balance_for(neutral)
    }

    /// La couleur caméra moyenne autour d'un point de l'image affichée (fractions de
    /// l'image recadrée), pour la pipette. Le redressement et la perspective sont
    /// ignorés : le point est pris dans le cadre droit.
    pub fn camera_color_at(&self, x: f32, y: f32, crop: &luma_project::Crop) -> [f32; 3] {
        let src = self.source(Size::Fit(1024), &mut Timings::default());
        let sx = (crop.left + x.clamp(0.0, 1.0) * (crop.right - crop.left)) * src.width as f32;
        let sy = (crop.top + y.clamp(0.0, 1.0) * (crop.bottom - crop.top)) * src.height as f32;
        let mut sum = [0.0f32; 3];
        let mut n = 0.0;
        for dy in -3..=3 {
            for dx in -3..=3 {
                let p = src.sample(sx + dx as f32, sy + dy as f32);
                for c in 0..3 {
                    sum[c] += p[c];
                }
                n += 1.0;
            }
        }
        sum.map(|v| v / n)
    }

    /// L'aberration chromatique latérale de la photo, estimée une fois sur un aperçu
    /// (c'est une proportion : elle vaut pour toutes les tailles).
    pub fn chromatic_aberration(&self, timings: &mut Timings) -> (f32, f32) {
        if let Some(ca) = *self.ca.lock().expect("ca") {
            return ca;
        }
        let preview = self.source(Size::Fit(1600), timings);
        let ca = timings.time("aberration chromatique", || luma_image::lens::estimate_ca(&preview));
        *self.ca.lock().expect("ca") = Some(ca);
        ca
    }

    /// Les étapes de voisinage, en cache tant que [`luma_image::base_key`] ne change pas.
    pub fn base(&self, settings: &Settings, size: Size, cancel: &Cancel, timings: &mut Timings) -> Result<Base> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let key = luma_image::base_key(settings);
        {
            let mut bases = self.bases.lock().expect("bases");
            if let Some(i) = bases.iter().position(|(s, k, _)| *s == size && *k == key) {
                let hit = bases.remove(i);
                let b = hit.2.clone();
                bases.insert(0, hit);
                return Ok(b);
            }
        }
        let source = self.source(size, timings);
        cancel.check()?;
        let ca = if settings.lens.remove_ca {
            self.chromatic_aberration(timings)
        } else {
            (0.0, 0.0)
        };
        let corrected = timings.time("objectif", || {
            luma_image::lens::correct(&source, &settings.lens, &settings.transform, ca)
        });
        let source_ref = corrected.as_ref().unwrap_or(&source);
        cancel.check()?;
        let cropped = timings.time("recadrage", || geometry::crop(source_ref, &settings.crop));
        let cam = cropped.as_ref().unwrap_or(source_ref);
        let scale = source.width as f32 / self.full_size().0 as f32;
        let input = Input {
            camera_to_working: self.camera_to_working(&settings.white_balance),
            exposure_bias: self.raw.baseline_exposure,
            scale,
        };
        let image = luma_image::develop_base(cam, &input, settings, cancel, timings)?;
        let base = Base {
            image: Arc::new(image),
            scale,
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        };
        // Pleine définition (export) : trop gros pour être gardé ; les petits rendus
        // (vignettes, aperçus des préréglages) ne chassent pas la base de l'aperçu.
        if !matches!(size, Size::Full | Size::Fit(..=400)) {
            let mut bases = self.bases.lock().expect("bases");
            // Une seule taille d'aperçu à la fois : les bases d'une autre taille partent.
            bases.retain(|(s, _, _)| *s == size);
            bases.insert(0, (size, key, base.clone()));
            bases.truncate(BASES);
        }
        Ok(base)
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
    fn the_base_is_reused_when_only_colors_change() {
        let p = photo();
        let mut s = Settings::default();
        let mut t = Timings::default();
        let a = p.base(&s, Size::Fit(600), &Cancel::new(), &mut t).unwrap();
        s.contrast = 40.0;
        s.vibrance = 30.0;
        s.effects.vignette_amount = -20.0;
        let b = p.base(&s, Size::Fit(600), &Cancel::new(), &mut t).unwrap();
        assert_eq!(a.id, b.id);
        s.exposure = 0.5;
        let c = p.base(&s, Size::Fit(600), &Cancel::new(), &mut t).unwrap();
        assert_ne!(a.id, c.id);
        // Le rendu avec base en cache est le même qu'un développement complet.
        let cached = p.render(&s, Size::Fit(600), &Cancel::new()).unwrap().image;
        let source = p.source(Size::Fit(600), &mut t);
        let input = Input {
            camera_to_working: p.camera_to_working(&s.white_balance),
            exposure_bias: 0.0,
            scale: source.width as f32 / p.full_size().0 as f32,
        };
        let direct = luma_image::develop(&source, &input, &s, &Cancel::new(), &mut t).unwrap();
        assert_eq!(cached.data, direct.data);
    }

    #[test]
    fn mask_films_with_their_own_tones_keep_their_base_in_cache() {
        use luma_project::{Look, Mask};
        let p = photo();
        let mut s = Settings::default();
        let mut m = Mask::full();
        m.look = Some(Look {
            name: "Haute clé".into(),
            amount: 100.0,
        });
        s.masks.push(m);
        let size = Size::Fit(600);
        let layered = p.render(&s, size, &Cancel::new()).unwrap();
        // Le film est complet (ses ombres comprises), comme posé sur toute la photo.
        let global = luma_project::presets::find("Haute clé")
            .unwrap()
            .applied(&Settings::default(), 1.0);
        let direct = p.render(&global, size, &Cancel::new()).unwrap();
        assert_eq!(layered.image.data, direct.image.data);
        // La base de la photo et celle du film restent en cache : rien n'est redéveloppé.
        let again = p.render(&s, size, &Cancel::new()).unwrap();
        assert!(
            again.timings.steps.iter().all(|(name, _)| *name != "entrée"),
            "{:?}",
            again.timings.steps
        );
    }

    #[test]
    fn auto_and_eyedropper_white_balance_match_the_neutral() {
        // La photo d'essai est uniforme, de la couleur du neutre du boîtier : la balance
        // automatique et la pipette retrouvent la balance « telle quelle ».
        let p = photo();
        let shot = p.as_shot();
        let auto = p.auto_white_balance();
        assert!(
            (auto.temperature - shot.temperature).abs() <= 50.0,
            "{auto:?} / {shot:?}"
        );
        assert!((auto.tint - shot.tint).abs() <= 2.0);
        let picked = p.white_balance_for(p.camera_color_at(0.5, 0.5, &luma_project::Crop::default()));
        assert!((picked.temperature - shot.temperature).abs() <= 50.0, "{picked:?}");
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
