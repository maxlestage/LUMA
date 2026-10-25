//! Lecture des fichiers RAW.
//!
//! Le décodage des formats (ARW, CR2, CR3, NEF, RAF, DNG, ORF, RW2…) est fait par
//! [rawler], une bibliothèque en Rust pur. Tout ce qui décide de la qualité de l'image
//! est ici, dans LÜMA :
//!
//! - normalisation : niveaux de noir par position du motif, niveau de blanc, recadrage
//!   sur la zone utile du capteur ;
//! - couleur au sens DNG : les deux matrices du boîtier (illuminants A et D65) sont
//!   interpolées selon la température du blanc choisi ;
//! - dématriçage : Malvar‑He‑Cutler pour les capteurs Bayer, interpolation des
//!   différences de couleur pour les X‑Trans de Fujifilm ;
//! - réduction 2×2 (Bayer) ou 3×3 (X‑Trans) sans dématriçage, pour des aperçus
//!   instantanés.

pub mod cameras;
mod demosaic;
pub mod dng;
pub mod raster;

use std::path::Path;

use luma_color::{self as color, Mat3, TempTint, Xy};
use luma_core::{Error, Result, RgbImage};
use rayon::prelude::*;

/// Le motif de filtres colorés du capteur, recalé sur la zone recadrée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cfa {
    pub width: usize,
    pub height: usize,
    /// 0 = rouge, 1 = vert, 2 = bleu, ligne par ligne.
    pub colors: Vec<u8>,
}

impl Cfa {
    /// Un motif écrit comme « RGGB » (lignes concaténées).
    pub fn parse(pattern: &str, width: usize, height: usize) -> Option<Cfa> {
        let colors: Option<Vec<u8>> = pattern
            .chars()
            .map(|c| match c {
                'R' => Some(0),
                'G' => Some(1),
                'B' => Some(2),
                _ => None,
            })
            .collect();
        let colors = colors?;
        (colors.len() == width * height).then_some(Cfa { width, height, colors })
    }

    #[inline]
    pub fn color(&self, x: usize, y: usize) -> usize {
        usize::from(self.colors[(y % self.height) * self.width + (x % self.width)])
    }
}

/// La disposition des données du capteur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mosaic {
    /// Capteur Bayer (motif 2×2).
    Bayer(Cfa),
    /// Capteur X‑Trans de Fujifilm (motif 6×6).
    XTrans(Cfa),
    /// Données déjà en RGB linéaire (DNG linéaire, ProRAW de l'iPhone).
    Linear,
}

/// L'orientation à appliquer à l'image (étiquette EXIF).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Normal,
    FlipHorizontal,
    Rotate180,
    FlipVertical,
    Transpose,
    Rotate90,
    Transverse,
    Rotate270,
}

/// Les données de prise de vue.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exif {
    pub iso: Option<u32>,
    /// En secondes.
    pub exposure_time: Option<f32>,
    pub aperture: Option<f32>,
    /// En millimètres.
    pub focal_length: Option<f32>,
    pub lens: Option<String>,
    pub date: Option<String>,
}

/// Un RAW décodé et normalisé : 0 = noir, 1 = saturation du capteur.
#[derive(Clone, Debug)]
pub struct RawPhoto {
    pub make: String,
    pub model: String,
    pub width: usize,
    pub height: usize,
    pub mosaic: Mosaic,
    /// `width × height` valeurs (mosaïque) ou `width × height × 3` (linéaire).
    pub data: Vec<f32>,
    /// La balance des blancs du boîtier, en neutre caméra (rouge, vert, bleu ; vert = 1).
    pub as_shot_neutral: [f32; 3],
    /// Les matrices XYZ → caméra et la température de leur illuminant.
    pub matrices: Vec<(f32, Mat3)>,
    pub orientation: Orientation,
    /// Correction d'exposition prévue par le fichier (DNG `BaselineExposure`), en IL.
    pub baseline_exposure: f32,
    pub exif: Exif,
    /// Une image déjà développée (JPEG, PNG, TIFF…) : ses valeurs sont celles de
    /// l'écran, et non la lumière de la scène (voir [`raster`]).
    pub rendered: bool,
}

fn illuminant_temperature(i: rawler::imgop::xyz::Illuminant) -> f32 {
    use rawler::imgop::xyz::Illuminant as I;
    match i {
        I::A | I::Tungsten | I::IsoStudioTungsten => 2856.0,
        I::B => 4874.0,
        I::C => 6774.0,
        I::D50 => 5003.0,
        I::D55 | I::Daylight | I::Flash | I::FineWeather => 5503.0,
        I::D65 | I::CloudyWeather => 6504.0,
        I::D75 | I::Shade => 7504.0,
        I::Fluorescent | I::CoolWhiteFluorescent => 4150.0,
        I::DaylightFluorescent => 6430.0,
        I::DaylightWhiteFluorescent => 5000.0,
        I::WhiteFluorescent => 3450.0,
        I::Unknown => 5000.0,
    }
}

fn rational(r: &Option<rawler::formats::tiff::Rational>) -> Option<f32> {
    r.as_ref().filter(|r| r.d != 0).map(|r| r.n as f32 / r.d as f32)
}

/// `BaselineExposure` d'un DNG, lu dans son IFD racine.
fn baseline_exposure(path: &Path) -> f32 {
    use rawler::formats::tiff::reader::TiffReader;
    let is_dng = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("dng"));
    if !is_dng {
        return 0.0;
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return 0.0;
    };
    let Ok(tiff) = rawler::formats::tiff::GenericTiffReader::new(&mut file, 0, 0, None, &[]) else {
        return 0.0;
    };
    tiff.root_ifd()
        .get_entry(rawler::tags::DngTag::BaselineExposure)
        .map_or(0.0, |e| e.value.force_f32(0))
        .clamp(-10.0, 10.0)
}

impl RawPhoto {
    /// Décode et normalise un fichier RAW.
    pub fn open(path: impl AsRef<Path>) -> Result<RawPhoto> {
        let path = path.as_ref();
        if raster::is_raster(path) {
            return raster::open(path);
        }
        let source = rawler::rawsource::RawSource::new(path).map_err(Error::Io)?;
        let params = rawler::decoders::RawDecodeParams::default();
        let raw = rawler::decode(&source, &params).map_err(|e| Error::Decode(e.to_string()))?;
        let exif = rawler::get_decoder(&source)
            .and_then(|d| d.raw_metadata(&source, &params))
            .map(|m| Exif {
                iso: m.exif.iso_speed.or(m.exif.iso_speed_ratings.map(u32::from)),
                exposure_time: rational(&m.exif.exposure_time),
                aperture: rational(&m.exif.fnumber),
                focal_length: rational(&m.exif.focal_length),
                lens: m
                    .exif
                    .lens_model
                    .clone()
                    .or(m.lens.map(|l| format!("{} {}", l.lens_make, l.lens_model))),
                date: m.exif.date_time_original.clone(),
            })
            .unwrap_or_default();
        let mut photo = Self::from_rawler(raw)?;
        photo.baseline_exposure = baseline_exposure(path);
        photo.exif = exif;
        Ok(photo)
    }

    fn from_rawler(raw: rawler::RawImage) -> Result<RawPhoto> {
        use rawler::rawimage::RawPhotometricInterpretation as P;
        let cpp = raw.cpp;
        let area = raw.crop_area.or(raw.active_area);
        let (ox, oy, width, height) = area.map_or((0, 0, raw.width, raw.height), |r| {
            (r.p.x, r.p.y, r.d.w.min(raw.width), r.d.h.min(raw.height))
        });

        let mosaic = match &raw.photometric {
            P::Cfa(config) => {
                let pattern = &config.cfa;
                let mut colors = Vec::with_capacity(pattern.width * pattern.height);
                for y in 0..pattern.height {
                    for x in 0..pattern.width {
                        colors.push(pattern.color_at(y + oy, x + ox).min(2) as u8);
                    }
                }
                let cfa = Cfa {
                    width: pattern.width,
                    height: pattern.height,
                    colors,
                };
                if cpp != 1 {
                    return Err(Error::Decode("motif CFA avec plusieurs composantes".into()));
                }
                match (cfa.width, cfa.height) {
                    (2, 2) => Mosaic::Bayer(cfa),
                    (6, 6) => Mosaic::XTrans(cfa),
                    (w, h) => return Err(Error::Decode(format!("motif CFA {w}×{h} non pris en charge"))),
                }
            }
            P::LinearRaw if cpp == 3 => Mosaic::Linear,
            P::LinearRaw => return Err(Error::Decode(format!("DNG linéaire à {cpp} composantes"))),
            P::BlackIsZero => return Err(Error::Decode("image monochrome non prise en charge".into())),
        };

        // Niveaux : le noir dépend de la position dans le motif, en coordonnées capteur.
        let black = &raw.blacklevel;
        let black_levels = black.as_vec();
        let whites = raw.whitelevel.as_vec();
        let full_width = raw.width;
        let source = raw.data.as_f32();
        let mut data = vec![0.0f32; width * height * cpp];
        data.par_chunks_mut(width * cpp).enumerate().for_each(|(y, row)| {
            let sy = y + oy;
            for x in 0..width {
                let sx = x + ox;
                for c in 0..cpp {
                    let bi =
                        ((sy % black.height) * black.width + (sx % black.width)) * black.cpp + c.min(black.cpp - 1);
                    let b = black_levels.get(bi).copied().unwrap_or(0.0);
                    let w = whites.get(c).or(whites.first()).copied().unwrap_or(65535.0);
                    let v = source[(sy * full_width + sx) * cpp + c];
                    row[x * cpp + c] = ((v - b) / (w - b).max(1.0)).max(0.0);
                }
            }
        });

        let mut matrices: Vec<(f32, Mat3)> = raw
            .color_matrix
            .iter()
            .filter(|(_, m)| m.len() >= 9)
            .map(|(i, m)| {
                (
                    illuminant_temperature(*i),
                    [[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]],
                )
            })
            .collect();
        matrices.sort_by(|a, b| a.0.total_cmp(&b.0));
        matrices.dedup_by(|a, b| (a.0 - b.0).abs() < 1.0);
        if matrices.is_empty() {
            // Sans matrice : on considère la caméra comme du sRGB.
            let m = color::inverse(&color::rgb_to_xyz(&color::SRGB)).expect("sRGB");
            matrices.push((6504.0, m));
        }

        let wb = raw.wb_coeffs;
        let mut photo = RawPhoto {
            make: raw.clean_make.clone(),
            model: raw.clean_model.clone(),
            width,
            height,
            mosaic,
            data,
            as_shot_neutral: [1.0; 3],
            matrices,
            orientation: match raw.orientation {
                rawler::Orientation::HorizontalFlip => Orientation::FlipHorizontal,
                rawler::Orientation::Rotate180 => Orientation::Rotate180,
                rawler::Orientation::VerticalFlip => Orientation::FlipVertical,
                rawler::Orientation::Transpose => Orientation::Transpose,
                rawler::Orientation::Rotate90 => Orientation::Rotate90,
                rawler::Orientation::Transverse => Orientation::Transverse,
                rawler::Orientation::Rotate270 => Orientation::Rotate270,
                _ => Orientation::Normal,
            },
            baseline_exposure: 0.0,
            exif: Exif::default(),
            rendered: false,
        };
        photo.as_shot_neutral = if wb[0].is_finite() && wb[1] > 0.0 && wb[0] > 0.0 && wb[2] > 0.0 {
            [wb[1] / wb[0], 1.0, wb[1] / wb[2]]
        } else {
            // Sans balance enregistrée : lumière du jour.
            photo.neutral_for(TempTint {
                temperature: 5500.0,
                tint: 0.0,
            })
        };
        Ok(photo)
    }

    // ----- Couleur ----------------------------------------------------------------------

    /// La matrice XYZ → caméra pour un blanc de température `t` (interpolation DNG en
    /// température inverse entre les deux illuminants).
    pub fn xyz_to_camera(&self, t: f32) -> Mat3 {
        let (first, last) = (self.matrices[0], self.matrices[self.matrices.len() - 1]);
        if self.matrices.len() == 1 || t <= first.0 {
            return first.1;
        }
        if t >= last.0 {
            return last.1;
        }
        let g = ((1.0 / t - 1.0 / last.0) / (1.0 / first.0 - 1.0 / last.0)).clamp(0.0, 1.0);
        let mut m = [[0.0; 3]; 3];
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                *v = g * first.1[i][j] + (1.0 - g) * last.1[i][j];
            }
        }
        m
    }

    /// La chromaticité d'un neutre caméra (itération de la spécification DNG : la
    /// matrice dépend du blanc, qui dépend de la matrice).
    pub fn neutral_to_xy(&self, neutral: [f32; 3]) -> Xy {
        let mut xy = color::D50;
        for _ in 0..30 {
            let t = color::xy_to_temp_tint(xy).temperature;
            let Some(inv) = color::inverse(&self.xyz_to_camera(t)) else {
                break;
            };
            let next = color::xyz_to_xy(color::apply(&inv, neutral));
            let done = (next.0 - xy.0).abs() + (next.1 - xy.1).abs() < 1e-7;
            xy = next;
            if done {
                break;
            }
        }
        xy
    }

    /// Le neutre caméra (vert = 1) d'un blanc donné en température et teinte.
    pub fn neutral_for(&self, white: TempTint) -> [f32; 3] {
        let xy = color::temp_tint_to_xy(white);
        let n = color::apply(&self.xyz_to_camera(white.temperature), color::xy_to_xyz(xy));
        [n[0] / n[1], 1.0, n[2] / n[1]]
    }

    /// La balance du boîtier, en température et teinte (« Telle quelle »).
    pub fn as_shot(&self) -> TempTint {
        color::xy_to_temp_tint(self.neutral_to_xy(self.as_shot_neutral))
    }

    /// La matrice caméra → espace de travail (Rec. 2020 linéaire) pour un neutre donné.
    /// Un pixel égal au neutre, avec un vert à 1, devient un blanc (1, 1, 1).
    pub fn camera_to_working(&self, neutral: [f32; 3]) -> Mat3 {
        let xy = self.neutral_to_xy(neutral);
        let t = color::xy_to_temp_tint(xy).temperature;
        let cam_to_xyz = color::inverse(&self.xyz_to_camera(t)).unwrap_or(color::IDENTITY);
        let adapt = color::bradford(xy, color::D65);
        let xyz_to_working = color::inverse(&color::rgb_to_xyz(&color::WORKING)).expect("espace de travail");
        let m = color::mul(&xyz_to_working, &color::mul(&adapt, &cam_to_xyz));
        // Mise à l'échelle : le neutre à vert = 1 vaut exactement 1 en sortie.
        let white = color::apply(&m, neutral);
        let s = 3.0 / (white[0] + white[1] + white[2]);
        m.map(|row| row.map(|v| v * s))
    }

    // ----- Images ------------------------------------------------------------------------

    /// L'image en RGB caméra, pleine définition (dématriçage complet).
    pub fn demosaic(&self) -> RgbImage {
        match &self.mosaic {
            Mosaic::Bayer(cfa) => demosaic::bayer_mhc(&self.data, self.width, self.height, cfa),
            Mosaic::XTrans(cfa) => demosaic::xtrans(&self.data, self.width, self.height, cfa),
            Mosaic::Linear => RgbImage {
                width: self.width,
                height: self.height,
                data: self.data.as_chunks::<3>().0.to_vec(),
            },
        }
    }

    /// L'image en RGB caméra, réduite sans dématriçage : chaque bloc du motif donne un
    /// pixel. Environ 20 fois plus rapide, pour les aperçus et les vignettes.
    pub fn binned(&self) -> RgbImage {
        match &self.mosaic {
            Mosaic::Bayer(cfa) => demosaic::bin(&self.data, self.width, self.height, cfa, 2, 2),
            Mosaic::XTrans(cfa) => demosaic::bin(&self.data, self.width, self.height, cfa, 3, 6),
            Mosaic::Linear => self.demosaic().resize_area(self.width / 2, self.height / 2),
        }
    }

    /// La taille de [`binned`](Self::binned).
    pub fn binned_size(&self) -> (usize, usize) {
        match &self.mosaic {
            Mosaic::XTrans(_) => (self.width / 3, self.height / 3),
            _ => (self.width / 2, self.height / 2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(neutral: [f32; 3]) -> RawPhoto {
        // Un boîtier fictif dont la matrice est celle du sRGB (A et D65 identiques).
        let m = color::inverse(&color::rgb_to_xyz(&color::SRGB)).unwrap();
        RawPhoto {
            make: "Test".into(),
            model: "Synthétique".into(),
            width: 4,
            height: 4,
            mosaic: Mosaic::Bayer(Cfa::parse("RGGB", 2, 2).unwrap()),
            data: vec![0.5; 16],
            as_shot_neutral: neutral,
            matrices: vec![(2856.0, m), (6504.0, m)],
            orientation: Orientation::Normal,
            baseline_exposure: 0.0,
            exif: Exif::default(),
            rendered: false,
        }
    }

    #[test]
    fn cfa_patterns() {
        let cfa = Cfa::parse("RGGB", 2, 2).unwrap();
        assert_eq!(
            (cfa.color(0, 0), cfa.color(1, 0), cfa.color(0, 1), cfa.color(1, 1)),
            (0, 1, 1, 2)
        );
        assert_eq!(cfa.color(2, 2), 0);
        assert!(Cfa::parse("RGXB", 2, 2).is_none());
    }

    #[test]
    fn neutral_maps_to_white() {
        let photo = synthetic([0.5, 1.0, 0.7]);
        let m = photo.camera_to_working(photo.as_shot_neutral);
        let w = color::apply(&m, photo.as_shot_neutral);
        assert!(w.iter().all(|v| (v - 1.0).abs() < 1e-3), "{w:?}");
    }

    #[test]
    fn temperature_round_trips_through_the_camera() {
        let photo = synthetic([1.0, 1.0, 1.0]);
        for temperature in [3000.0, 5500.0, 8000.0] {
            let n = photo.neutral_for(TempTint {
                temperature,
                tint: 10.0,
            });
            let back = color::xy_to_temp_tint(photo.neutral_to_xy(n));
            assert!((back.temperature - temperature).abs() < temperature * 0.01, "{back:?}");
            assert!((back.tint - 10.0).abs() < 1.0, "{back:?}");
        }
    }
}
