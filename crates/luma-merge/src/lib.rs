//! Fusion de plusieurs photos en une : empilements (réduction du bruit, suppression de
//! ce qui bouge, filé, ciel étoilé) et panoramas ; et la mesure d'une vraie pellicule
//! (un scan comparé à la même scène en numérique, voir [`film`]).
//!
//! Tout se passe sur les couleurs caméra linéaires, avant tout développement : le
//! résultat est enregistré en DNG, un vrai RAW, qui se développe ensuite comme une
//! photo ordinaire. Le rognage automatique (le plus grand rectangle couvert) n'est
//! qu'un recadrage proposé dans les réglages de ce DNG : on peut toujours l'enlever.
//!
//! Aucune bibliothèque de vision : détection des coins et des étoiles, appariement,
//! RANSAC, homographies, projections et fondus sont écrits ici, en Rust.

pub mod autocrop;
pub mod features;
pub mod film;
pub mod frames;
pub mod geometry;
pub mod matching;
pub mod pano;
pub mod stack;
pub mod stars;
pub mod warp;

use std::path::{Path, PathBuf};

use luma_core::{Result, RgbImage};
use luma_project::{Crop, Settings};

pub use frames::{Frames, RawFiles};
pub use pano::{Projection, panorama};
pub use stack::{StackMode, stack};

/// L'avancement d'une fusion : fraction (0 à 1) et étape en cours.
pub type Progress<'a> = dyn Fn(f32, &str) + Sync + 'a;

/// Le résultat d'une fusion.
pub struct Merged {
    /// Couleurs caméra linéaires (1 = blanc du capteur).
    pub image: RgbImage,
    /// Les pixels couverts par toutes les photos (empilement) ou par au moins une
    /// (panorama).
    pub covered: Vec<bool>,
    /// Le recadrage proposé : le plus grand rectangle couvert (`None` : rien à rogner).
    pub crop: Option<Crop>,
    /// Le nombre de photos utilisées, et celles écartées (impossibles à aligner).
    pub used: usize,
    pub skipped: Vec<usize>,
    /// L'exposition (IL) à ajouter au développement : une fusion HDR est ramenée sous le
    /// blanc du capteur pour tenir dans le DNG, puis réexposée d'autant.
    pub exposure: f32,
}

impl Merged {
    pub fn new(image: RgbImage, covered: Vec<bool>, used: usize, skipped: Vec<usize>) -> Merged {
        let crop = autocrop::auto_crop(&covered, image.width, image.height);
        Merged {
            image,
            covered,
            crop,
            used,
            skipped,
            exposure: 0.0,
        }
    }

    /// Étire les bords irréguliers (d'un panorama) pour remplir le rectangle, au lieu de
    /// les rogner ; le rognage proposé devient ce qui reste à enlever (presque rien).
    pub fn warp_edges(&mut self) {
        let (image, covered) = warp::boundary_warp(&self.image, &self.covered);
        self.crop = autocrop::auto_crop(&covered, image.width, image.height);
        (self.image, self.covered) = (image, covered);
    }

    /// Ramène l'image sous 1 (le plus clair des pixels, à un dix‑millième près, devient
    /// le blanc du capteur) et note l'exposition qui la rend à son niveau.
    pub fn fit_exposure(&mut self) {
        let mut v: Vec<f32> = self
            .image
            .data
            .iter()
            .step_by(3)
            .map(|p| p[0].max(p[1]).max(p[2]))
            .collect();
        if v.is_empty() {
            return;
        }
        let at = ((v.len() as f64 * 0.9999) as usize).min(v.len() - 1);
        let top = *v.select_nth_unstable_by(at, f32::total_cmp).1;
        if top > 1.0 {
            let k = 1.0 / top;
            self.image
                .data
                .iter_mut()
                .for_each(|p| *p = p.map(|c| (c * k).min(1.0)));
            self.exposure = top.log2();
        }
    }

    /// Enregistre la fusion en DNG à `path`, avec la couleur de `template` (la photo de
    /// référence). Si `crop` est vrai, le rognage proposé est écrit comme recadrage dans
    /// le XMP à côté (modifiable, réversible) ; sinon toute l'image reste visible.
    pub fn save(&self, path: &Path, template: &luma_raw::RawPhoto, crop: bool) -> Result<PathBuf> {
        luma_raw::dng::write_linear_dng(
            path,
            self.image.width,
            self.image.height,
            &self.image.data,
            template,
            self.exposure,
            "LÜMA",
        )?;
        let xmp = path.with_extension("xmp");
        let settings = Settings {
            crop: if crop {
                self.crop.unwrap_or_default()
            } else {
                Crop::default()
            },
            ..Settings::default()
        };
        std::fs::write(&xmp, luma_project::xmp::write(&settings))
            .map_err(|e| luma_core::Error::Decode(format!("{} : {e}", xmp.display())))?;
        Ok(path.to_path_buf())
    }
}

/// Une version réduite pour l'analyse (grand côté au plus `long_side`).
pub fn analysis(img: &RgbImage, long_side: usize) -> RgbImage {
    let (w, h) = RgbImage::fit(img.width, img.height, long_side);
    if (w, h) == (img.width, img.height) {
        img.clone()
    } else {
        img.resize_area(w, h)
    }
}

/// Un nom de fichier libre à côté de `first` : `IMG_0001-Pano.dng`, `-Pano-2.dng`…
pub fn output_path(first: &Path, suffix: &str) -> PathBuf {
    let stem = first
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dir = first.parent().unwrap_or(Path::new("."));
    let mut k = 1;
    loop {
        let name = if k == 1 {
            format!("{stem}-{suffix}.dng")
        } else {
            format!("{stem}-{suffix}-{k}.dng")
        };
        let p = dir.join(name);
        if !p.exists() {
            return p;
        }
        k += 1;
    }
}
