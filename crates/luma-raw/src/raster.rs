//! Les images déjà développées : JPEG, PNG, TIFF, WebP, GIF, BMP, TGA, QOI, OpenEXR,
//! Radiance HDR, PNM, ICO, décodées en Rust pur par [image] ; sur Mac, en plus, tout ce
//! que lit le système (HEIC et HEIF de l'iPhone, AVIF, JPEG XL, PSD, JPEG 2000…),
//! converti par `sips` en TIFF 16 bits.
//!
//! Une telle image devient une [`RawPhoto`] linéaire dont la « caméra » est l'espace de
//! l'image (sRGB, Display P3, Adobe RGB, ProPhoto, lu dans son profil ICC) : la balance
//! des blancs, les couleurs et tous les outils marchent comme sur un RAW. Ses valeurs
//! sont celles de l'écran ([`RawPhoto::rendered`]) : le profil de rendu « Original » la
//! garde telle quelle tant qu'aucun réglage ne change.

use std::path::Path;

use image::ImageDecoder;
use luma_color::{self as color, Mat3, Primaries};
use luma_core::{Error, Result};

use crate::{Exif, Mosaic, Orientation, RawPhoto};

/// Les extensions lues partout, sans aide du système.
pub const EXTENSIONS: [&str; 20] = [
    "jpg", "jpeg", "jpe", "jfif", "png", "tif", "tiff", "webp", "gif", "bmp", "tga", "qoi", "exr", "hdr", "pnm", "ppm",
    "pgm", "pbm", "pam", "ico",
];

/// Les extensions que seul le système sait lire (sur Mac).
pub const SYSTEM_EXTENSIONS: [&str; 8] = ["heic", "heif", "hif", "avif", "jxl", "psd", "jp2", "j2k"];

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Une image développée (et non un RAW), d'après son extension.
pub fn is_raster(path: &Path) -> bool {
    let e = extension(path);
    EXTENSIONS.contains(&e.as_str()) || SYSTEM_EXTENSIONS.contains(&e.as_str())
}

/// Ce système sait‑il lire cette extension (sans le RAW) ?
pub fn supported(ext: &str) -> bool {
    let e = ext.to_ascii_lowercase();
    EXTENSIONS.contains(&e.as_str()) || (cfg!(target_os = "macos") && SYSTEM_EXTENSIONS.contains(&e.as_str()))
}

/// Adobe RGB (1998).
const ADOBE_RGB: Primaries = Primaries {
    r: (0.64, 0.33),
    g: (0.21, 0.71),
    b: (0.15, 0.06),
    white: color::D65,
};

/// L'espace d'une image : ses primaires et sa courbe de transfert.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Space {
    Srgb,
    DisplayP3,
    AdobeRgb,
    ProPhoto,
    Rec2020,
    /// Déjà linéaire (OpenEXR, Radiance HDR), en primaires sRGB.
    LinearSrgb,
}

impl Space {
    fn primaries(self) -> Primaries {
        match self {
            Space::Srgb | Space::LinearSrgb => color::SRGB,
            Space::DisplayP3 => color::DISPLAY_P3,
            Space::AdobeRgb => ADOBE_RGB,
            Space::ProPhoto => color::PROPHOTO,
            Space::Rec2020 => color::REC2020,
        }
    }

    /// La valeur codée (0 à 1) en lumière linéaire.
    fn decode(self, v: f32) -> f32 {
        match self {
            Space::Srgb | Space::DisplayP3 => color::srgb_decode(v),
            Space::AdobeRgb => v.max(0.0).powf(563.0 / 256.0),
            Space::ProPhoto => {
                if v < 16.0 / 512.0 {
                    v / 16.0
                } else {
                    v.powf(1.8)
                }
            }
            Space::Rec2020 => {
                if v < 0.081 {
                    v / 4.5
                } else {
                    ((v + 0.099) / 1.099).powf(1.0 / 0.45)
                }
            }
            Space::LinearSrgb => v,
        }
    }

    /// La température du blanc de l'espace.
    fn white_temperature(self) -> f32 {
        if self == Space::ProPhoto { 5003.0 } else { 6504.0 }
    }
}

/// L'espace décrit par un profil ICC, d'après son nom (balise `desc`, en ASCII ou en
/// UTF‑16) ; sRGB par défaut, comme le veut le web.
pub fn icc_space(icc: &[u8]) -> Space {
    let ascii: String = icc
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                ' '
            }
        })
        .collect();
    let utf16: String = icc
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| {
            if c[0] == 0 && (c[1].is_ascii_graphic() || c[1] == b' ') {
                c[1] as char
            } else {
                ' '
            }
        })
        .collect();
    let text = format!("{ascii} {utf16}").to_ascii_lowercase();
    if text.contains("display p3") || text.contains("p3 d65") || text.contains("dci-p3") || text.contains("dci p3") {
        Space::DisplayP3
    } else if text.contains("adobe rgb") || text.contains("adobergb") || text.contains("compatible with adobe") {
        Space::AdobeRgb
    } else if text.contains("prophoto") || text.contains("romm") {
        Space::ProPhoto
    } else if text.contains("2020") || text.contains("2100") {
        Space::Rec2020
    } else {
        Space::Srgb
    }
}

fn orientation(o: image::metadata::Orientation) -> Orientation {
    use image::metadata::Orientation as O;
    match o {
        O::NoTransforms => Orientation::Normal,
        O::FlipHorizontal => Orientation::FlipHorizontal,
        O::Rotate180 => Orientation::Rotate180,
        O::FlipVertical => Orientation::FlipVertical,
        O::Rotate90FlipH => Orientation::Transpose,
        O::Rotate90 => Orientation::Rotate90,
        O::Rotate270FlipH => Orientation::Transverse,
        O::Rotate270 => Orientation::Rotate270,
    }
}

fn decode_error(e: impl std::fmt::Display) -> Error {
    Error::Decode(e.to_string())
}

/// Ouvre une image développée.
pub fn open(path: &Path) -> Result<RawPhoto> {
    let ext = extension(path);
    if SYSTEM_EXTENSIONS.contains(&ext.as_str()) {
        return open_with_system(path);
    }
    let reader = image::ImageReader::open(path)
        .map_err(Error::Io)?
        .with_guessed_format()
        .map_err(Error::Io)?;
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let icc = decoder.icc_profile().ok().flatten();
    let turn = decoder.orientation().map_or(Orientation::Normal, orientation);
    let float = matches!(
        decoder.color_type(),
        image::ColorType::Rgb32F | image::ColorType::Rgba32F
    );
    let img = image::DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    let space = if float {
        Space::LinearSrgb
    } else {
        icc.as_deref().map_or(Space::Srgb, icc_space)
    };
    Ok(from_image(&img, space, turn))
}

/// Une image décodée, en photo linéaire.
pub fn from_image(img: &image::DynamicImage, space: Space, orientation: Orientation) -> RawPhoto {
    let rgb = img.to_rgb32f();
    let (width, height) = (rgb.width() as usize, rgb.height() as usize);
    // Le décodage de la courbe, en table pour les entrées entières (0 à 1).
    let table: Vec<f32> = (0..=4096).map(|i| space.decode(i as f32 / 4096.0)).collect();
    let decode = |v: f32| -> f32 {
        if space == Space::LinearSrgb {
            return v.max(0.0);
        }
        let x = v.clamp(0.0, 1.0) * 4096.0;
        let i = (x as usize).min(4095);
        table[i] + (table[i + 1] - table[i]) * (x - i as f32)
    };
    let data: Vec<f32> = rgb.into_raw().into_iter().map(decode).collect();
    let to_xyz = color::rgb_to_xyz(&space.primaries());
    let xyz_to_camera: Mat3 = color::inverse(&to_xyz).unwrap_or(color::IDENTITY);
    RawPhoto {
        make: String::new(),
        model: String::new(),
        width,
        height,
        mosaic: Mosaic::Linear,
        data,
        as_shot_neutral: [1.0, 1.0, 1.0],
        matrices: vec![(space.white_temperature(), xyz_to_camera)],
        orientation,
        baseline_exposure: 0.0,
        exif: Exif::default(),
        rendered: true,
    }
}

/// Sur Mac : `sips` (l'outil d'images du système) convertit en TIFF 16 bits, que LÜMA
/// lit ensuite comme les autres ; le profil ICC et l'orientation suivent.
fn open_with_system(path: &Path) -> Result<RawPhoto> {
    if !cfg!(target_os = "macos") {
        return Err(Error::Decode(format!(
            "le format {} n'est lu que sur Mac ; exportez la photo en JPEG ou en TIFF",
            extension(path).to_uppercase()
        )));
    }
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tiff = std::env::temp_dir().join(format!("luma-{}-{n}.tiff", std::process::id()));
    let out = std::process::Command::new("sips")
        .args(["-s", "format", "tiff"])
        .arg(path)
        .arg("--out")
        .arg(&tiff)
        .output()
        .map_err(Error::Io)?;
    let result = if out.status.success() {
        open(&tiff)
    } else {
        Err(Error::Decode(String::from_utf8_lossy(&out.stderr).trim().to_owned()))
    };
    let _ = std::fs::remove_file(&tiff);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_srgb_png_becomes_linear_and_white_stays_white() {
        let mut img = image::RgbImage::new(4, 2);
        img.put_pixel(0, 0, image::Rgb([255, 255, 255]));
        img.put_pixel(1, 0, image::Rgb([119, 119, 119]));
        img.put_pixel(2, 0, image::Rgb([255, 0, 0]));
        let path = std::env::temp_dir().join(format!("luma-raster-{}.png", std::process::id()));
        img.save(&path).unwrap();
        let photo = RawPhoto::open(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(photo.rendered);
        assert_eq!((photo.width, photo.height), (4, 2));
        // Blanc → (1, 1, 1) en sortie de la matrice ; gris sRGB 119 → ≈ 18 % linéaire.
        let m = photo.camera_to_working(photo.as_shot_neutral);
        let white = color::apply(&m, [photo.data[0], photo.data[1], photo.data[2]]);
        assert!(white.iter().all(|v| (v - 1.0).abs() < 1e-3), "{white:?}");
        assert!((photo.data[3] - 0.184).abs() < 0.005, "{}", photo.data[3]);
        // La « Telle quelle » d'une image sRGB : le blanc D65.
        assert!((photo.as_shot().temperature - 6504.0).abs() < 60.0);
        // Rouge sRGB pur : dans l'espace de travail, du rouge sRGB (Rec. 2020 plus large).
        let red = color::apply(&m, [photo.data[6], photo.data[7], photo.data[8]]);
        let expect = color::apply(&color::rgb_to_rgb(&color::SRGB, &color::WORKING), [1.0, 0.0, 0.0]);
        for c in 0..3 {
            assert!((red[c] - expect[c]).abs() < 1e-3, "{red:?} {expect:?}");
        }
    }

    #[test]
    fn icc_names_give_the_space() {
        let p3 = b"....desc....Display P3....";
        assert_eq!(icc_space(p3), Space::DisplayP3);
        let utf16: Vec<u8> = "Adobe RGB (1998)".bytes().flat_map(|b| [0, b]).collect();
        assert_eq!(icc_space(&utf16), Space::AdobeRgb);
        assert_eq!(icc_space(b"sRGB IEC61966-2.1"), Space::Srgb);
        assert_eq!(icc_space(b"ProPhoto RGB"), Space::ProPhoto);
    }

    #[test]
    fn raster_extensions() {
        assert!(is_raster(Path::new("a/IMG_1.JPG")));
        assert!(is_raster(Path::new("b.heic")));
        assert!(!is_raster(Path::new("c.ARW")));
        assert!(!is_raster(Path::new("c.dng")));
        assert!(supported("WebP"));
    }
}
