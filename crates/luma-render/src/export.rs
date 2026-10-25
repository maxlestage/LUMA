//! Export des images rendues : JPEG, PNG et TIFF, avec le profil ICC de l'espace de
//! sortie incorporé (sans profil, un fichier Display P3 serait lu comme du sRGB).

use std::path::Path;

use image::{ExtendedColorType, ImageEncoder};
use luma_color::{OutputSpace, icc};
use luma_core::RgbImage;

/// Écrit l'image dans `path` : JPEG, PNG ou TIFF selon l'extension, 8 ou 16 bits (le
/// JPEG est toujours sur 8 bits), avec le profil ICC de l'espace de sortie.
pub fn write(path: &Path, img: &RgbImage, space: OutputSpace, quality: u8, bits: u8) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg")
        .to_ascii_lowercase();
    let file = std::fs::File::create(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let writer = std::io::BufWriter::new(file);
    let (w, h) = (img.width as u32, img.height as u32);
    let profile = icc::profile(space);
    let err = |e: image::ImageError| e.to_string();
    let sixteen = bits == 16 && ext != "jpg" && ext != "jpeg";
    let (bytes, kind) = if sixteen {
        let px = luma_image::encode16(img, space);
        (
            px.iter().flat_map(|v| v.to_ne_bytes()).collect::<Vec<u8>>(),
            ExtendedColorType::Rgb16,
        )
    } else {
        (luma_image::encode8(img, space), ExtendedColorType::Rgb8)
    };
    match ext.as_str() {
        "jpg" | "jpeg" => {
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, quality.clamp(1, 100));
            let _ = enc.set_icc_profile(profile);
            enc.write_image(&bytes, w, h, kind).map_err(err)
        }
        "png" => {
            let mut enc = image::codecs::png::PngEncoder::new(writer);
            let _ = enc.set_icc_profile(profile);
            enc.write_image(&bytes, w, h, kind).map_err(err)
        }
        "tif" | "tiff" => {
            let mut enc = image::codecs::tiff::TiffEncoder::new(writer);
            let _ = enc.set_icc_profile(profile);
            enc.write_image(&bytes, w, h, kind).map_err(err)
        }
        other => Err(format!("format de sortie inconnu « {other} » (jpg, png, tif)")),
    }
}
