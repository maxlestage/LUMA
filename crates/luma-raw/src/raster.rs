//! Les images déjà développées, quel que soit leur format :
//!
//! - en Rust pur, partout : JPEG, PNG, TIFF, WebP, GIF, BMP, TGA, QOI, OpenEXR,
//!   Radiance HDR, PNM, ICO, DDS, farbfeld ([image]), JPEG XL ([jxl_oxide]), PSD
//!   ([psd]), JPEG 2000 ([hayro_jpeg2000]), SVG ([resvg]) ;
//! - par le décodeur du système quand l'app en fournit un ([`set_system_decoder`] :
//!   ImageIO d'Apple, sur Mac, iPhone et iPad) : HEIC et HEIF de l'iPhone, AVIF, et
//!   tout ce que le système lit. Il sert aussi de recours quand un décodeur en Rust
//!   refuse un fichier (PSD 16 bits, variante rare…).
//!
//! Une extension inconnue n'arrête rien : le contenu du fichier est essayé avec chaque
//! décodeur.
//!
//! Une telle image devient une [`RawPhoto`] linéaire dont la « caméra » est l'espace de
//! l'image (sRGB, Display P3, Adobe RGB, ProPhoto, Rec. 2020, lu dans son profil ICC) :
//! la balance des blancs, les couleurs et tous les outils marchent comme sur un RAW.
//! Ses valeurs sont celles de l'écran ([`RawPhoto::rendered`]) : le profil de rendu
//! « Original » la garde telle quelle tant qu'aucun réglage ne change. La transparence
//! est posée sur du blanc.

use std::path::Path;
use std::sync::OnceLock;

use image::ImageDecoder;
use luma_color::{self as color, Mat3, Primaries};
use luma_core::{Error, Result};

use crate::{Exif, Mosaic, Orientation, RawPhoto};

/// Les extensions lues partout, en Rust pur.
pub const EXTENSIONS: [&str; 31] = [
    "jpg", "jpeg", "jpe", "jfif", "png", "apng", "tif", "tiff", "webp", "gif", "bmp", "dib", "tga", "qoi", "exr",
    "hdr", "pnm", "ppm", "pgm", "pbm", "pam", "ico", "dds", "ff", "jxl", "psd", "psb", "jp2", "j2k", "j2c", "svg",
];

/// Les extensions confiées au décodeur du système (HEIC, AVIF…).
pub const SYSTEM_EXTENSIONS: [&str; 10] = [
    "heic", "heif", "hif", "heics", "avif", "avci", "icns", "astc", "ktx", "pvr",
];

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Une image développée (et non un RAW), d'après son extension.
pub fn is_raster(path: &Path) -> bool {
    let e = extension(path);
    EXTENSIONS.contains(&e.as_str()) || SYSTEM_EXTENSIONS.contains(&e.as_str()) || e == "svgz"
}

/// LÜMA sait‑il ouvrir cette extension, ici (sans compter les RAW) ?
pub fn supported(ext: &str) -> bool {
    let e = ext.to_ascii_lowercase();
    EXTENSIONS.contains(&e.as_str())
        || e == "svgz"
        || (SYSTEM_EXTENSIONS.contains(&e.as_str()) && SYSTEM.get().is_some())
}

// ----- Couleur ----------------------------------------------------------------------

/// Adobe RGB (1998).
const ADOBE_RGB: Primaries = Primaries {
    r: (0.64, 0.33),
    g: (0.21, 0.71),
    b: (0.15, 0.06),
    white: color::D65,
};

/// Les primaires d'une image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gamut {
    Srgb,
    DisplayP3,
    AdobeRgb,
    ProPhoto,
    Rec2020,
}

/// La courbe de transfert d'une image : comment ses valeurs codées deviennent de la
/// lumière.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transfer {
    Srgb,
    /// Gamma 2,2 (Adobe RGB).
    Gamma22,
    /// ProPhoto (ROMM) : gamma 1,8.
    Gamma18,
    /// Rec. 709 et Rec. 2020.
    Rec709,
    /// Déjà linéaire (OpenEXR, HDR, TIFF flottant, décodeur du système).
    Linear,
}

/// L'espace d'une image : ses primaires et sa courbe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Space {
    pub gamut: Gamut,
    pub transfer: Transfer,
}

impl Space {
    pub const SRGB: Space = Space {
        gamut: Gamut::Srgb,
        transfer: Transfer::Srgb,
    };
    pub const LINEAR_SRGB: Space = Space {
        gamut: Gamut::Srgb,
        transfer: Transfer::Linear,
    };
    pub const LINEAR_DISPLAY_P3: Space = Space {
        gamut: Gamut::DisplayP3,
        transfer: Transfer::Linear,
    };

    fn primaries(self) -> Primaries {
        match self.gamut {
            Gamut::Srgb => color::SRGB,
            Gamut::DisplayP3 => color::DISPLAY_P3,
            Gamut::AdobeRgb => ADOBE_RGB,
            Gamut::ProPhoto => color::PROPHOTO,
            Gamut::Rec2020 => color::REC2020,
        }
    }

    /// La valeur codée (0 à 1) en lumière linéaire.
    fn decode(self, v: f32) -> f32 {
        match self.transfer {
            Transfer::Srgb => color::srgb_decode(v),
            Transfer::Gamma22 => v.max(0.0).powf(563.0 / 256.0),
            Transfer::Gamma18 => {
                if v < 16.0 / 512.0 {
                    v / 16.0
                } else {
                    v.powf(1.8)
                }
            }
            Transfer::Rec709 => {
                if v < 0.081 {
                    v / 4.5
                } else {
                    ((v + 0.099) / 1.099).powf(1.0 / 0.45)
                }
            }
            Transfer::Linear => v,
        }
    }

    /// La température du blanc de l'espace.
    fn white_temperature(self) -> f32 {
        if self.gamut == Gamut::ProPhoto { 5003.0 } else { 6504.0 }
    }
}

/// L'espace décrit par un profil ICC, d'après son nom (balise `desc`, en ASCII ou en
/// UTF‑16) ; sRGB par défaut, comme le veut le web.
pub fn icc_space(icc: &[u8]) -> Space {
    let printable = |b: u8| b.is_ascii_graphic() || b == b' ';
    let ascii: String = icc
        .iter()
        .map(|&b| if printable(b) { b as char } else { ' ' })
        .collect();
    let utf16: String = icc
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| {
            if c[0] == 0 && printable(c[1]) {
                c[1] as char
            } else {
                ' '
            }
        })
        .collect();
    let text = format!("{ascii} {utf16}").to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| text.contains(w));
    let (gamut, transfer) = if has(&["display p3", "p3 d65", "dci-p3", "dci p3", "p3-d65"]) {
        (Gamut::DisplayP3, Transfer::Srgb)
    } else if has(&["adobe rgb", "adobergb", "compatible with adobe"]) {
        (Gamut::AdobeRgb, Transfer::Gamma22)
    } else if has(&["prophoto", "romm"]) {
        (Gamut::ProPhoto, Transfer::Gamma18)
    } else if has(&["2020", "2100"]) {
        (Gamut::Rec2020, Transfer::Rec709)
    } else {
        (Gamut::Srgb, Transfer::Srgb)
    };
    // Un profil « linéaire » (scRGB, sRGB linéaire, P3 linéaire…).
    let transfer = if has(&["linear", "scrgb", "lineal"]) {
        Transfer::Linear
    } else {
        transfer
    };
    Space { gamut, transfer }
}

// ----- Décodeur du système -------------------------------------------------------------

/// Une image décodée par le système : RGB linéaire (valeurs au‑delà de 1 permises),
/// transparence déjà posée sur du blanc.
pub struct Decoded {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
    pub space: Space,
    pub orientation: Orientation,
}

/// Le décodeur du système : `None` s'il ne connaît pas le fichier.
pub type SystemDecoder = fn(&Path) -> Option<std::result::Result<Decoded, String>>;

static SYSTEM: OnceLock<SystemDecoder> = OnceLock::new();

/// Donne au moteur le décodeur d'images du système (une fois, au démarrage de l'app).
pub fn set_system_decoder(decoder: SystemDecoder) {
    let _ = SYSTEM.set(decoder);
}

fn system(path: &Path) -> Option<Result<RawPhoto>> {
    let decoder = SYSTEM.get()?;
    Some(
        decoder(path)?
            .map(|d| photo(d.width, d.height, d.data, d.space, d.orientation))
            .map_err(Error::Decode),
    )
}

// ----- Ouverture -----------------------------------------------------------------------

fn decode_error(e: impl std::fmt::Display) -> Error {
    Error::Decode(e.to_string())
}

/// Ouvre une image développée, de n'importe quel format.
pub fn open(path: &Path) -> Result<RawPhoto> {
    let ext = extension(path);
    if SYSTEM_EXTENSIONS.contains(&ext.as_str()) {
        return system(path).unwrap_or_else(|| {
            Err(Error::Decode(format!(
                "aucun décodeur {} sur ce système",
                ext.to_uppercase()
            )))
        });
    }
    let first = match ext.as_str() {
        "jxl" => open_jxl(path),
        "psd" | "psb" => open_psd(path),
        "jp2" | "j2k" | "j2c" | "jpf" | "jpx" => open_jp2(path),
        "svg" | "svgz" => open_svg(path),
        _ => open_image(path),
    };
    match first {
        Ok(p) => Ok(p),
        // Un décodeur en Rust a refusé : le système, s'il sait, puis le contenu.
        Err(e) => system(path)
            .and_then(Result::ok)
            .map(Ok)
            .unwrap_or_else(|| sniff(path).ok_or(e)),
    }
}

/// D'après le contenu du fichier, quelle que soit son extension : chaque décodeur,
/// puis celui du système.
pub fn sniff(path: &Path) -> Option<RawPhoto> {
    let head = {
        use std::io::Read;
        let mut b = [0u8; 16];
        let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut b)).ok()?;
        b[..n].to_vec()
    };
    let tries: [fn(&Path) -> Result<RawPhoto>; 4] = [open_image, open_jxl, open_psd, open_jp2];
    let order: &[usize] = if head.starts_with(&[0xFF, 0x0A]) || head.get(4..8) == Some(b"JXL ") {
        &[1]
    } else if head.starts_with(b"8BPS") {
        &[2]
    } else if head.starts_with(&[0xFF, 0x4F, 0xFF, 0x51]) || head.get(4..8) == Some(b"jP  ") {
        &[3]
    } else {
        &[0]
    };
    order
        .iter()
        .find_map(|&i| tries[i](path).ok())
        .or_else(|| system(path).and_then(Result::ok))
        .or_else(|| {
            let text = std::fs::read(path).ok()?;
            let svg = text.windows(4).take(4096).any(|w| w == b"<svg");
            svg.then(|| open_svg(path).ok()).flatten()
        })
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

/// Par un décodeur du crate [image] (dont celui du JPEG XL).
fn from_decoder(mut decoder: impl ImageDecoder) -> Result<RawPhoto> {
    let icc = decoder.icc_profile().ok().flatten();
    let turn = decoder.orientation().map_or(Orientation::Normal, orientation);
    let float = matches!(
        decoder.color_type(),
        image::ColorType::Rgb32F | image::ColorType::Rgba32F
    );
    let img = image::DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    let space = match icc.as_deref() {
        Some(icc) => icc_space(icc),
        None if float => Space::LINEAR_SRGB,
        None => Space::SRGB,
    };
    Ok(from_image(&img, space, turn))
}

fn open_image(path: &Path) -> Result<RawPhoto> {
    let reader = image::ImageReader::open(path)
        .map_err(Error::Io)?
        .with_guessed_format()
        .map_err(Error::Io)?;
    from_decoder(reader.into_decoder().map_err(decode_error)?)
}

fn open_jxl(path: &Path) -> Result<RawPhoto> {
    let file = std::io::BufReader::new(std::fs::File::open(path).map_err(Error::Io)?);
    from_decoder(jxl_oxide::integration::JxlDecoder::new(file).map_err(decode_error)?)
}

fn open_psd(path: &Path) -> Result<RawPhoto> {
    let bytes = std::fs::read(path).map_err(Error::Io)?;
    let psd = psd::Psd::from_bytes(&bytes).map_err(decode_error)?;
    let img = image::RgbaImage::from_raw(psd.width(), psd.height(), psd.rgba())
        .ok_or_else(|| Error::Decode("PSD : taille incohérente".into()))?;
    Ok(from_image(&img.into(), Space::SRGB, Orientation::Normal))
}

fn open_jp2(path: &Path) -> Result<RawPhoto> {
    use hayro_jpeg2000::{ColorSpace, DecodeSettings, DecoderContext, Image};
    let bytes = std::fs::read(path).map_err(Error::Io)?;
    let image = Image::new(&bytes, &DecodeSettings::default()).map_err(decode_error)?;
    let (w, h) = (image.width(), image.height());
    let space = match image.color_space() {
        ColorSpace::Icc { profile, .. } => icc_space(profile),
        _ => Space::SRGB,
    };
    let channels = image.color_space().num_channels() as usize;
    let alpha = image.has_alpha();
    let mut ctx = DecoderContext::default();
    let data = image.decode(&mut ctx).map_err(decode_error)?.data_u8();
    let stride = channels + usize::from(alpha);
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for p in data.chunks_exact(stride) {
        let a = if alpha { p[stride - 1] } else { 255 };
        let rgb = match channels {
            1 | 2 => [p[0]; 3],
            4 => {
                // CMJN → RGB (sans profil).
                let k = 255 - u16::from(p[3]);
                [0, 1, 2].map(|c| ((255 - u16::from(p[c])) * k / 255) as u8)
            }
            _ => [p[0], p[1], p[2]],
        };
        rgba.extend([rgb[0], rgb[1], rgb[2], a]);
    }
    let img =
        image::RgbaImage::from_raw(w, h, rgba).ok_or_else(|| Error::Decode("JPEG 2000 : taille incohérente".into()))?;
    Ok(from_image(&img.into(), space, Orientation::Normal))
}

/// Un SVG, rendu à 4096 pixels sur son grand côté (c'est un dessin : il n'a pas de
/// définition propre).
fn open_svg(path: &Path) -> Result<RawPhoto> {
    let bytes = std::fs::read(path).map_err(Error::Io)?;
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_data(&bytes, &options).map_err(decode_error)?;
    let size = tree.size();
    let scale = 4096.0 / size.width().max(size.height()).max(1.0);
    let (w, h) = (
        (size.width() * scale).round().max(1.0) as u32,
        (size.height() * scale).round().max(1.0) as u32,
    );
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(w, h).ok_or_else(|| Error::Decode("SVG : taille impossible".into()))?;
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // Fond blanc opaque : les valeurs prémultipliées sont les vraies.
    let img = image::RgbaImage::from_raw(w, h, pixmap.take())
        .ok_or_else(|| Error::Decode("SVG : taille incohérente".into()))?;
    Ok(from_image(&img.into(), Space::SRGB, Orientation::Normal))
}

/// Une image décodée, en photo linéaire. La transparence est posée sur du blanc.
pub fn from_image(img: &image::DynamicImage, space: Space, orientation: Orientation) -> RawPhoto {
    let rgba = img.to_rgba32f();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let linear = space.transfer == Transfer::Linear;
    // Le décodage de la courbe, en table pour les valeurs codées (0 à 1).
    let table: Vec<f32> = (0..=4096).map(|i| space.decode(i as f32 / 4096.0)).collect();
    let decode = |v: f32| -> f32 {
        if linear {
            return v.max(0.0);
        }
        let x = v.clamp(0.0, 1.0) * 4096.0;
        let i = (x as usize).min(4095);
        table[i] + (table[i + 1] - table[i]) * (x - i as f32)
    };
    let mut data = Vec::with_capacity(width * height * 3);
    for p in rgba.into_raw().as_chunks::<4>().0 {
        let a = p[3].clamp(0.0, 1.0);
        for &c in &p[..3] {
            // Sur du blanc, dans les valeurs codées (comme le fait un écran).
            data.push(decode(c * a + (1.0 - a)));
        }
    }
    photo(width, height, data, space, orientation)
}

/// La photo d'une image linéaire dans l'espace `space`.
fn photo(width: usize, height: usize, data: Vec<f32>, space: Space, orientation: Orientation) -> RawPhoto {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("luma-raster-{}-{name}", std::process::id()))
    }

    #[test]
    fn a_srgb_png_becomes_linear_and_white_stays_white() {
        let mut img = image::RgbImage::new(4, 2);
        img.put_pixel(0, 0, image::Rgb([255, 255, 255]));
        img.put_pixel(1, 0, image::Rgb([119, 119, 119]));
        img.put_pixel(2, 0, image::Rgb([255, 0, 0]));
        let path = temp("a.png");
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
    fn transparency_lies_on_white() {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 0]));
        let photo = from_image(&img.into(), Space::SRGB, Orientation::Normal);
        assert!(photo.data.iter().all(|&v| (v - 1.0).abs() < 1e-4));
    }

    #[test]
    fn a_misnamed_file_is_opened_by_its_content() {
        let img = image::RgbImage::from_pixel(3, 3, image::Rgb([10, 200, 30]));
        let png = temp("b.png");
        img.save(&png).unwrap();
        let odd = temp("b.photo");
        std::fs::rename(&png, &odd).unwrap();
        let photo = RawPhoto::open(&odd).unwrap();
        let _ = std::fs::remove_file(&odd);
        assert!(photo.rendered && photo.width == 3);
    }

    #[test]
    fn a_svg_is_drawn_on_white() {
        let path = temp("c.svg");
        std::fs::write(
            &path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect x="0" y="0" width="10" height="10" fill="red"/></svg>"#,
        )
        .unwrap();
        let photo = RawPhoto::open(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!((photo.width, photo.height), (4096, 2048));
        let px = |x: usize, y: usize| {
            let o = (y * photo.width + x) * 3;
            [photo.data[o], photo.data[o + 1], photo.data[o + 2]]
        };
        assert!(px(100, 100)[0] > 0.99 && px(100, 100)[1] < 0.01, "rouge");
        assert!(px(4000, 100).iter().all(|&v| v > 0.99), "blanc");
    }

    #[test]
    fn icc_names_give_the_space() {
        assert_eq!(icc_space(b"....desc....Display P3....").gamut, Gamut::DisplayP3);
        let utf16: Vec<u8> = "Adobe RGB (1998)".bytes().flat_map(|b| [0, b]).collect();
        assert_eq!(icc_space(&utf16).gamut, Gamut::AdobeRgb);
        assert_eq!(icc_space(b"sRGB IEC61966-2.1"), Space::SRGB);
        assert_eq!(icc_space(b"ProPhoto RGB").transfer, Transfer::Gamma18);
        assert_eq!(icc_space(b"sRGB linear").transfer, Transfer::Linear);
    }

    #[test]
    fn raster_extensions() {
        assert!(is_raster(Path::new("a/IMG_1.JPG")));
        assert!(is_raster(Path::new("b.heic")));
        assert!(!is_raster(Path::new("c.ARW")));
        assert!(!is_raster(Path::new("c.dng")));
        assert!(supported("WebP") && supported("jxl") && supported("PSD"));
    }
}
