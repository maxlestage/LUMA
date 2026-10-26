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
pub mod opcodes;
pub mod raster;

use std::path::Path;

use luma_color::{self as color, Mat3, TempTint, Xy};
use luma_core::{Error, Result, RgbImage};
use rayon::prelude::*;

/// Les extensions RAW reconnues dans un dossier.
pub const RAW_EXTENSIONS: [&str; 20] = [
    "arw", "cr2", "cr3", "crw", "nef", "nrw", "raf", "dng", "orf", "rw2", "pef", "srw", "3fr", "iiq", "erf", "kdc",
    "dcr", "mos", "mef", "rwl",
];

pub fn is_raw(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| RAW_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Une photo que LÜMA sait ouvrir : un RAW, ou une image déjà développée (JPEG, PNG,
/// TIFF, WebP… et, sur Mac, HEIC, AVIF, JPEG XL, PSD).
pub fn is_photo(path: &Path) -> bool {
    is_raw(path) || path.extension().and_then(|e| e.to_str()).is_some_and(raster::supported)
}

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
    /// L'auteur et le copyright inscrits par le boîtier.
    pub artist: Option<String>,
    pub copyright: Option<String>,
    pub gps: Option<Gps>,
}

/// La position de la prise de vue : degrés (nord et est positifs), altitude en mètres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gps {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
}

/// Une coordonnée EXIF (degrés, minutes, secondes) et son hémisphère (`N`, `S`, `E`, `O`).
fn gps_degrees(dms: &Option<[rawler::formats::tiff::Rational; 3]>, r: &Option<String>, negative: char) -> Option<f64> {
    let v = dms.as_ref()?;
    let part = |i: usize| (v[i].d != 0).then(|| f64::from(v[i].n) / f64::from(v[i].d));
    let deg = part(0)? + part(1).unwrap_or(0.0) / 60.0 + part(2).unwrap_or(0.0) / 3600.0;
    let neg = r.as_deref().is_some_and(|r| r.trim().starts_with(negative));
    Some(if neg { -deg } else { deg })
}

fn gps(g: &Option<rawler::exif::ExifGPS>) -> Option<Gps> {
    let g = g.as_ref()?;
    let latitude = gps_degrees(&g.gps_latitude, &g.gps_latitude_ref, 'S')?;
    let longitude = gps_degrees(&g.gps_longitude, &g.gps_longitude_ref, 'W')?;
    let altitude = g.gps_altitude.filter(|a| a.d != 0).map(|a| {
        let v = f64::from(a.n) / f64::from(a.d);
        if g.gps_altitude_ref == Some(1) { -v } else { v }
    });
    (latitude.is_finite() && longitude.is_finite() && (latitude, longitude) != (0.0, 0.0)).then_some(Gps {
        latitude,
        longitude,
        altitude,
    })
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
    /// Les corrections d'objectif d'un DNG (`OpcodeList3`), appliquées après le
    /// dématriçage, et l'image du capteur à laquelle elles se réfèrent.
    pub opcodes: Option<(Vec<opcodes::Opcode>, opcodes::Frame)>,
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

/// Les corrections d'objectif d'un DNG : `OpcodeList2` (sur la mosaïque) et
/// `OpcodeList3` (après le dématriçage). Vides pour les autres formats.
fn dng_opcodes(path: &Path) -> (Vec<opcodes::Opcode>, Vec<opcodes::Opcode>) {
    use rawler::formats::tiff::reader::TiffReader;
    let is_dng = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("dng"));
    let Some(tiff) = is_dng
        .then(|| std::fs::File::open(path).ok())
        .flatten()
        .and_then(|mut f| rawler::formats::tiff::GenericTiffReader::new(&mut f, 0, 0, None, &[]).ok())
    else {
        return Default::default();
    };
    let list = |tag: rawler::tags::DngTag| {
        tiff.root_ifd()
            .find_first_ifd_with_tag(tag)
            .and_then(|ifd| ifd.get_entry(tag))
            .map(|e| opcodes::parse(e.value.get_data()))
            .unwrap_or_default()
    };
    (
        list(rawler::tags::DngTag::OpcodeList2),
        list(rawler::tags::DngTag::OpcodeList3),
    )
}

/// Les corrections d'objectif rangées par le boîtier dans un RAW qui n'est pas un DNG :
/// aujourd'hui celles des ARW de Sony et des RAF de Fujifilm.
fn maker_corrections(path: &Path) -> Vec<opcodes::Opcode> {
    use rawler::formats::tiff::reader::TiffReader;
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("raf")) {
        return fuji_corrections(path).into_iter().collect();
    }
    let is_arw = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("arw") || e.eq_ignore_ascii_case("sr2"));
    let Some(tiff) = is_arw
        .then(|| std::fs::File::open(path).ok())
        .flatten()
        .and_then(|mut f| rawler::formats::tiff::GenericTiffReader::new(&mut f, 0, 0, None, &[]).ok())
    else {
        return Vec::new();
    };
    let values = |tag: u16| -> Vec<i32> {
        tiff.root_ifd()
            .find_first_ifd_with_tag(tag)
            .and_then(|ifd| ifd.get_entry(tag))
            .map(|e| (0..e.value.count()).map(|i| e.value.force_i32(i)).collect())
            .unwrap_or_default()
    };
    opcodes::sony(&values(0x7037), &values(0x7035), &values(0x7032))
        .into_iter()
        .collect()
}

/// Les corrections d'un RAF de Fujifilm. Le RAF range ses données brutes dans un petit
/// TIFF (son adresse est à l'octet 0x64 de l'en‑tête), dont l'étiquette 0xF000 mène au
/// FujiIFD ; ses décalages se comptent depuis le début de ce TIFF. Seuls quelques Ko en
/// sont lus.
fn fuji_corrections(path: &Path) -> Option<opcodes::Opcode> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 0x68];
    file.read_exact(&mut head).ok()?;
    if !head.starts_with(b"FUJIFILMCCD-RAW") {
        return None;
    }
    let start = u32::from_be_bytes(head[0x64..0x68].try_into().ok()?);
    file.seek(SeekFrom::Start(u64::from(start))).ok()?;
    let mut tiff = Vec::new();
    file.take(64 << 10).read_to_end(&mut tiff).ok()?;
    let le = tiff.starts_with(b"II*\0");
    if !le && !tiff.starts_with(b"MM\0*") {
        return None;
    }
    let u16_at = |at: usize| -> Option<u16> {
        let b: [u8; 2] = tiff.get(at..at + 2)?.try_into().ok()?;
        Some(if le {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        })
    };
    let u32_at = |at: usize| -> Option<u32> {
        let b: [u8; 4] = tiff.get(at..at + 4)?.try_into().ok()?;
        Some(if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    // Les entrées d'un IFD : (étiquette, type, nombre, valeur ou décalage).
    let entries = |at: usize| -> Vec<(u16, u16, u32, u32)> {
        let n = u16_at(at).unwrap_or(0).min(512) as usize;
        (0..n)
            .filter_map(|k| {
                let e = at + 2 + 12 * k;
                Some((u16_at(e)?, u16_at(e + 2)?, u32_at(e + 4)?, u32_at(e + 8)?))
            })
            .collect()
    };
    let root = entries(u32_at(4)? as usize);
    let fuji_ifd = root.iter().find(|e| e.0 == 0xF000)?.3 as usize;
    let fuji = entries(fuji_ifd);
    // Une liste de rationnels signés (type 10).
    let rationals = |tag: u16| -> Vec<f64> {
        let Some(&(_, 10, count, offset)) = fuji.iter().find(|e| e.0 == tag) else {
            return Vec::new();
        };
        (0..count.min(128) as usize)
            .map_while(|i| {
                let at = offset as usize + 8 * i;
                let (n, d) = (u32_at(at)? as i32, u32_at(at + 4)? as i32);
                Some(if d == 0 { 0.0 } else { f64::from(n) / f64::from(d) })
            })
            .collect()
    };
    opcodes::fuji(
        &rationals(0xF00B),
        &rationals(0xF00F),
        &rationals(0xF010),
        fuji_crop(path),
    )
}

/// 1,25 quand le Fujifilm photographiait en mode recadré (étiquette CropMode 0x104D de sa
/// note du fabricant, valeur 2 ou 4), sinon 1.
fn fuji_crop(path: &Path) -> f64 {
    let Some(head) = read_head(path, 1 << 20) else {
        return 1.0;
    };
    let Some(base) = head.windows(12).position(|w| w == b"FUJIFILM\x0c\0\0\0") else {
        return 1.0;
    };
    let u16_at = |at: usize| head.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32_at = |at: usize| {
        head.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let Some(ifd) = u32_at(base + 8).map(|o| base + o as usize) else {
        return 1.0;
    };
    let n = u16_at(ifd).unwrap_or(0).min(512) as usize;
    for k in 0..n {
        let e = ifd + 2 + 12 * k;
        if u16_at(e) == Some(0x104D) {
            return if matches!(u16_at(e + 8), Some(2 | 4)) {
                1.25
            } else {
                1.0
            };
        }
    }
    1.0
}

/// Ce que dit l'en‑tête d'une photo, sans décoder ses pixels : de quoi la ranger dans
/// la photothèque.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Info {
    pub make: String,
    pub model: String,
    pub exif: Exif,
}

fn metadata(source: &rawler::rawsource::RawSource, params: &rawler::decoders::RawDecodeParams) -> Option<Info> {
    let m = rawler::get_decoder(source)
        .and_then(|d| d.raw_metadata(source, params))
        .ok()?;
    Some(Info {
        make: m.make.clone(),
        model: m.model.clone(),
        exif: Exif {
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
            artist: m.exif.artist.clone().filter(|s| !s.trim().is_empty()),
            copyright: m.exif.copyright.clone().filter(|s| !s.trim().is_empty()),
            gps: gps(&m.exif.gps),
        },
    })
}

/// Les données de prise de vue d'un RAW, lues dans son en‑tête (quelques millisecondes,
/// au lieu d'un décodage complet). Une image développée (JPEG, PNG…) ou un fichier
/// illisible donne `None`.
///
/// Seul le début du fichier est lu : 256 Ko suffisent à presque tous les RAW, 1 ou 4 Mo à
/// ceux qui rangent leur aperçu avant leurs métadonnées ; le fichier entier n'est lu
/// qu'en dernier recours, et pour le CR3. Sur un disque externe ou réseau, c'est ce qui coûte : un RAW
/// fait 25 à 60 Mo.
pub fn info(path: impl AsRef<Path>) -> Option<Info> {
    let path = path.as_ref();
    if raster::is_raster(path) {
        return None;
    }
    let params = rawler::decoders::RawDecodeParams::default();
    let size = std::fs::metadata(path).ok()?.len();
    // Le CR3 range une partie de ses métadonnées dans ses pistes, à la fin du fichier.
    let whole = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("cr3"));
    for n in [256 << 10, 1 << 20, 4 << 20] {
        if whole || size <= n as u64 {
            break;
        }
        let head = read_head(path, n)?;
        let source = rawler::rawsource::RawSource::new_from_shared_vec(std::sync::Arc::new(head)).with_path(path);
        // Un en‑tête coupé peut faire échouer le décodeur, jamais le tromper : ce qui
        // n'est pas dans le début du fichier ne se lit pas.
        if let Ok(Some(info)) = std::panic::catch_unwind(|| metadata(&source, &params)) {
            return Some(info);
        }
    }
    let source = rawler::rawsource::RawSource::new(path).ok()?;
    metadata(&source, &params)
}

impl RawPhoto {
    /// Décode et normalise un fichier RAW.
    pub fn open(path: impl AsRef<Path>) -> Result<RawPhoto> {
        let path = path.as_ref();
        if raster::is_raster(path) {
            return raster::open(path);
        }
        // Un RAW, ou à défaut une image d'après son contenu (extension inconnue).
        Self::open_raw(path).or_else(|e| raster::sniff(path).ok_or(e))
    }

    fn open_raw(path: &Path) -> Result<RawPhoto> {
        let source = rawler::rawsource::RawSource::new(path).map_err(Error::Io)?;
        let params = rawler::decoders::RawDecodeParams::default();
        let raw = rawler::decode(&source, &params).map_err(|e| Error::Decode(e.to_string()))?;
        let exif = metadata(&source, &params).map(|i| i.exif).unwrap_or_default();
        // L'image du capteur à laquelle se réfèrent les opcodes (la zone active), vue
        // depuis l'image recadrée de LÜMA.
        let active = raw
            .active_area
            .map_or((0, 0, raw.width, raw.height), |r| (r.p.x, r.p.y, r.d.w, r.d.h));
        let origin = raw.crop_area.or(raw.active_area).map_or((0, 0), |r| (r.p.x, r.p.y));
        let frame = opcodes::Frame {
            x: active.0 as f64 - origin.0 as f64,
            y: active.1 as f64 - origin.1 as f64,
            width: active.2 as f64,
            height: active.3 as f64,
        };
        let mut photo = Self::from_rawler(raw)?;
        photo.baseline_exposure = baseline_exposure(path);
        photo.exif = exif;
        let (before, mut after) = dng_opcodes(path);
        // Le vignetage d'`OpcodeList2` s'applique à la mosaïque, avant le dématriçage ;
        // sur un DNG déjà en RGB, il rejoint ceux d'après.
        if matches!(photo.mosaic, Mosaic::Linear) {
            after.splice(0..0, before);
        } else {
            opcodes::apply_to_mosaic(&before, &mut photo.data, photo.width, frame);
        }
        // Les corrections que le boîtier a rangées dans son RAW, autour du centre de
        // l'image.
        let own = maker_corrections(path);
        if !own.is_empty() && after.is_empty() {
            let image = opcodes::Frame {
                x: 0.0,
                y: 0.0,
                width: photo.width as f64,
                height: photo.height as f64,
            };
            photo.opcodes = Some((own, image));
        } else if !after.is_empty() {
            photo.opcodes = Some((after, frame));
        }
        Ok(photo)
    }

    /// L'aperçu JPEG que le boîtier a enregistré dans le RAW, dans le bon sens, réduit
    /// à `max_side` pixels sur son grand côté : le plus petit des JPEG intégrés qui soit
    /// assez grand (sinon le plus grand, sans l'agrandir). Quelques millisecondes, sans décoder ni dématricer le capteur : de
    /// quoi montrer une vignette tout de suite, avant le vrai rendu. `None` si le fichier
    /// n'en a pas (ou n'est pas un RAW).
    pub fn embedded_preview(path: impl AsRef<Path>, max_side: u32) -> Option<image::RgbImage> {
        let path = path.as_ref();
        if raster::is_raster(path) {
            return None;
        }
        // Les aperçus sont presque toujours au début du fichier : les 8 premiers Mo
        // d'abord, le fichier entier seulement s'il le faut.
        let pick = |bytes: &[u8]| -> Option<image::DynamicImage> {
            let mut found = embedded_jpegs(bytes);
            found.sort_by_key(|j| u64::from(j.width) * u64::from(j.height));
            // Le plus petit assez grand ; à défaut, le plus grand qu'il y ait (un aperçu
            // de 1616 px pour un coup d'œil de 2048).
            let big_enough = found.iter().position(|j| j.width.max(j.height) >= max_side);
            let order: Vec<&EmbeddedJpeg> = match big_enough {
                Some(i) => found[i..].iter().collect(),
                None => found.iter().rev().collect(),
            };
            order
                .into_iter()
                .find_map(|j| decode_scaled(&bytes[j.offset..], max_side))
        };
        let head = read_head(path, 8 << 20)?;
        let (img, bytes) = match pick(&head) {
            Some(img) => (img, head),
            None => {
                let all = std::fs::read(path).ok()?;
                (pick(&all)?, all)
            }
        };
        let mut img = if img.width().max(img.height()) > max_side {
            img.thumbnail(max_side, max_side)
        } else {
            img
        };
        // Les aperçus sont enregistrés dans le sens du capteur : l'orientation de la photo
        // s'y applique. Lue dans l'en‑tête TIFF quand il y en a un (la plupart des RAW),
        // sinon par le décodeur.
        let code = tiff_orientation(&bytes).or_else(|| {
            let source = rawler::rawsource::RawSource::new(path).ok()?;
            let params = rawler::decoders::RawDecodeParams::default();
            rawler::get_decoder(&source)
                .and_then(|d| d.raw_metadata(&source, &params))
                .ok()
                .and_then(|m| m.exif.orientation)
        });
        if let Some(o) = code.and_then(|o| image::metadata::Orientation::from_exif(o.min(8) as u8)) {
            img.apply_orientation(o);
        }
        Some(img.to_rgb8())
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
            opcodes: None,
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
        let mut img = match &self.mosaic {
            Mosaic::Bayer(cfa) => demosaic::bayer_mhc(&self.data, self.width, self.height, cfa),
            Mosaic::XTrans(cfa) => demosaic::xtrans(&self.data, self.width, self.height, cfa),
            Mosaic::Linear => RgbImage {
                width: self.width,
                height: self.height,
                data: self.data.as_chunks::<3>().0.to_vec(),
            },
        };
        self.correct_lens(&mut img);
        img
    }

    /// Les corrections d'objectif du fichier (DNG), à la taille de `img`.
    fn correct_lens(&self, img: &mut RgbImage) {
        if let Some((ops, frame)) = &self.opcodes {
            let scale = img.width as f64 / self.width.max(1) as f64;
            opcodes::apply(ops, img, frame.scaled(scale));
        }
    }

    /// L'image en RGB caméra, réduite sans dématriçage : chaque bloc du motif donne un
    /// pixel. Environ 20 fois plus rapide, pour les aperçus et les vignettes.
    pub fn binned(&self) -> RgbImage {
        match &self.mosaic {
            Mosaic::Bayer(cfa) => {
                let mut img = demosaic::bin(&self.data, self.width, self.height, cfa, 2, 2);
                self.correct_lens(&mut img);
                img
            }
            Mosaic::XTrans(cfa) => {
                let mut img = demosaic::bin(&self.data, self.width, self.height, cfa, 3, 6);
                self.correct_lens(&mut img);
                img
            }
            // Déjà corrigée par `demosaic`.
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

/// Un JPEG décodé directement à la plus petite échelle (1, 1/2, 1/4 ou 1/8) qui donne
/// encore au moins `side` pixels sur le grand côté : un aperçu de 13 Mpx en sort huit
/// fois plus petit, bien plus vite qu'un décodage complet.
fn decode_scaled(jpeg: &[u8], side: u32) -> Option<image::DynamicImage> {
    use jpeg_decoder::PixelFormat;
    let mut d = jpeg_decoder::Decoder::new(std::io::Cursor::new(jpeg));
    d.read_info().ok()?;
    let info = d.info()?;
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    // La taille demandée garde les proportions ; le décodeur choisit l'échelle juste
    // au‑dessus.
    let k = f64::from(side) / f64::from(w.max(h));
    let req = |v: u32| (f64::from(v) * k).ceil().clamp(1.0, 65_535.0) as u16;
    let (sw, sh) = d.scale(req(w), req(h)).ok()?;
    let px = d.decode().ok()?;
    let (sw, sh) = (u32::from(sw), u32::from(sh));
    let rgb: Vec<u8> = match d.info()?.pixel_format {
        PixelFormat::RGB24 => px,
        PixelFormat::L8 => px.iter().flat_map(|&v| [v, v, v]).collect(),
        PixelFormat::L16 => px.as_chunks::<2>().0.iter().flat_map(|v| [v[0], v[0], v[0]]).collect(),
        PixelFormat::CMYK32 => px
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                let k = 255 - u16::from(p[3]);
                [0, 1, 2].map(|c| ((255 - u16::from(p[c])) * k / 255) as u8)
            })
            .collect(),
    };
    image::RgbImage::from_raw(sw, sh, rgb).map(image::DynamicImage::ImageRgb8)
}

/// Les `n` premiers octets d'un fichier (moins s'il est plus court).
fn read_head(path: &Path, n: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut buf = Vec::with_capacity(n);
    std::fs::File::open(path)
        .ok()?
        .take(n as u64)
        .read_to_end(&mut buf)
        .ok()?;
    Some(buf)
}

/// L'orientation EXIF (étiquette 0x0112) du premier IFD d'un fichier TIFF (« II*\0 » ou
/// « MM\0* », l'en‑tête de la plupart des RAW).
fn tiff_orientation(b: &[u8]) -> Option<u16> {
    let le = match b.get(..4)? {
        [b'I', b'I', 42, 0] => true,
        [b'M', b'M', 0, 42] => false,
        _ => return None,
    };
    let u16_at = |at: usize| -> Option<u16> {
        let v = [*b.get(at)?, *b.get(at + 1)?];
        Some(if le {
            u16::from_le_bytes(v)
        } else {
            u16::from_be_bytes(v)
        })
    };
    let u32_at = |at: usize| -> Option<usize> {
        let v = [*b.get(at)?, *b.get(at + 1)?, *b.get(at + 2)?, *b.get(at + 3)?];
        Some(
            (if le {
                u32::from_le_bytes(v)
            } else {
                u32::from_be_bytes(v)
            }) as usize,
        )
    };
    let ifd = u32_at(4)?;
    let count = usize::from(u16_at(ifd)?);
    (0..count.min(512)).find_map(|k| {
        let e = ifd + 2 + k * 12;
        (u16_at(e)? == 0x0112).then(|| u16_at(e + 8)).flatten()
    })
}

/// Un JPEG enregistré dans un RAW : où il commence, et sa taille.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EmbeddedJpeg {
    offset: usize,
    width: u32,
    height: u32,
}

/// Les JPEG d'aperçu cachés dans un fichier : chaque début de JPEG (FF D8 FF), suivi de
/// ses segments jusqu'à l'en‑tête d'image (SOF) qui donne la taille. Les JPEG sans
/// perte (SOF3, 7, 11, 15), qui portent les données du capteur, sont écartés.
fn embedded_jpegs(bytes: &[u8]) -> Vec<EmbeddedJpeg> {
    let be16 =
        |at: usize| -> Option<usize> { Some(usize::from(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]))) };
    let mut out = Vec::new();
    let mut i = 0;
    // On saute d'un octet 0xFF au suivant (recherche rapide), puis on vérifie FF D8 FF.
    while let Some(k) = bytes[i..].iter().position(|&b| b == 0xFF) {
        let start = i + k;
        i = start + 1;
        if bytes.get(start + 1) != Some(&0xD8) || bytes.get(start + 2) != Some(&0xFF) {
            continue;
        }
        i = start + 2;
        let mut j = start + 2;
        for _ in 0..64 {
            if bytes.get(j) != Some(&0xFF) {
                break;
            }
            let Some(&marker) = bytes.get(j + 1) else { break };
            let Some(len) = be16(j + 2) else { break };
            let sof = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
            if sof {
                let lossless = matches!(marker, 0xC3 | 0xC7 | 0xCB | 0xCF);
                if let (Some(h), Some(w)) = (be16(j + 5), be16(j + 7))
                    && !lossless
                    && w >= 16
                    && h >= 16
                {
                    out.push(EmbeddedJpeg {
                        offset: start,
                        width: w as u32,
                        height: h as u32,
                    });
                }
                break;
            }
            if len < 2 {
                break;
            }
            j += 2 + len;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_jpegs_are_found_with_their_size() {
        // Un faux RAW : des octets, un petit JPEG, encore des octets.
        let mut jpeg = Vec::new();
        image::RgbImage::from_pixel(64, 40, image::Rgb([200, 100, 50]))
            .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .unwrap();
        let mut file = vec![0u8; 1000];
        file.extend(&jpeg);
        file.extend([0xFF, 0xD8, 0xFF, 0x00, 0x12]);
        let found = embedded_jpegs(&file);
        assert_eq!(
            found,
            [EmbeddedJpeg {
                offset: 1000,
                width: 64,
                height: 40
            }]
        );
    }

    #[test]
    fn lens_corrections_of_a_dng_apply_at_every_size() {
        // Un capteur Bayer uniforme, et un vignetage à corriger : les coins s'éclaircissent,
        // à la même place dans l'image entière et dans l'aperçu réduit.
        let (w, h) = (64, 48);
        let mut photo = synthetic([1.0; 3]);
        photo.width = w;
        photo.height = h;
        photo.mosaic = Mosaic::Bayer(Cfa::parse("RGGB", 2, 2).unwrap());
        photo.data = vec![0.25; w * h];
        let flat_full = photo.demosaic();
        let flat_binned = photo.binned();
        let ops = vec![opcodes::Opcode::FixVignetteRadial {
            k: [1.0, 0.0, 0.0, 0.0, 0.0],
            center: [0.5, 0.5],
        }];
        let frame = opcodes::Frame {
            x: 0.0,
            y: 0.0,
            width: w as f64,
            height: h as f64,
        };
        photo.opcodes = Some((ops, frame));
        let full = photo.demosaic();
        let binned = photo.binned();
        let corner_gain = |a: &RgbImage, b: &RgbImage| a.data[0][1] / b.data[0][1];
        let center_gain = |a: &RgbImage, b: &RgbImage| {
            let i = (a.height / 2) * a.width + a.width / 2;
            a.data[i][1] / b.data[i][1]
        };
        assert!(
            corner_gain(&full, &flat_full) > 1.8,
            "{}",
            corner_gain(&full, &flat_full)
        );
        assert!(center_gain(&full, &flat_full) < 1.01);
        assert!(
            (corner_gain(&binned, &flat_binned) - corner_gain(&full, &flat_full)).abs() < 0.1,
            "aperçu {} et pleine taille {}",
            corner_gain(&binned, &flat_binned),
            corner_gain(&full, &flat_full)
        );
        // Sans opcodes, rien ne change.
        photo.opcodes = None;
        assert_eq!(photo.demosaic().data, flat_full.data);
    }

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
            opcodes: None,
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
