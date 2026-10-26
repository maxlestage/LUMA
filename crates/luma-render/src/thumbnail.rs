//! Les vignettes enregistrées : le rendu 320 px d'une photo avec ses réglages, gardé
//! sans perte (QOI, rapide à relire) dans le dossier des doublons. Rouvrir un dossier
//! les affiche aussitôt, sans rien redévelopper ; elles se refont quand la photo, ses
//! réglages ou le moteur changent.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::{ImageEncoder, RgbImage};
use luma_project::Settings;

/// L'extension des vignettes, à côté des doublons (`.lumadup`).
pub const EXTENSION: &str = "lumavig";

/// Le fichier de la vignette d'une photo : son chemin, sa taille et sa date, ses
/// réglages (`None` : ceux par défaut), le côté demandé et la version du moteur
/// (FNV‑1a 64 bits).
pub fn path_for(source: &Path, settings: Option<&Settings>, side: u32, dir: &Path) -> Option<PathBuf> {
    let (len, time) = crate::duplicate::signature(source)?;
    let settings = settings.map_or_else(String::new, |s| serde_json::to_string(s).unwrap_or_default());
    let key = format!(
        "{}|{len}|{time}|{side}|{}|{settings}",
        source.display(),
        env!("CARGO_PKG_VERSION")
    );
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    Some(dir.join(format!("{h:016x}.{EXTENSION}")))
}

/// Relit une vignette enregistrée.
pub fn load(path: &Path) -> Option<RgbImage> {
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Qoi).ok()?;
    Some(img.into_rgb8())
}

/// Enregistre une vignette (QOI, sans perte), d'un coup : écrite à côté puis renommée,
/// une vignette à moitié écrite n'est jamais relue.
pub fn store(path: &Path, rgb: &[u8], width: u32, height: u32) -> std::io::Result<()> {
    let mut bytes = Vec::new();
    image::codecs::qoi::QoiEncoder::new(Cursor::new(&mut bytes))
        .write_image(rgb, width, height, image::ExtendedColorType::Rgb8)
        .map_err(std::io::Error::other)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thumbnail_follows_its_photo_and_settings() {
        let dir = std::env::temp_dir().join(format!("luma-vig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("photo.ARW");
        std::fs::write(&source, b"un RAW").unwrap();
        let s = Settings {
            exposure: 0.5,
            ..Settings::default()
        };
        let a = path_for(&source, None, 320, &dir).unwrap();
        let b = path_for(&source, Some(&s), 320, &dir).unwrap();
        assert_ne!(a, b, "d'autres réglages, une autre vignette");
        assert_eq!(b, path_for(&source, Some(&s), 320, &dir).unwrap());
        assert_ne!(b, path_for(&source, Some(&s), 640, &dir).unwrap());

        let (w, h) = (40u32, 30u32);
        let rgb: Vec<u8> = (0..w * h * 3).map(|i| (i % 251) as u8).collect();
        store(&b, &rgb, w, h).unwrap();
        let back = load(&b).unwrap();
        assert_eq!(back.dimensions(), (w, h));
        assert_eq!(back.as_raw(), &rgb, "sans perte");

        // La photo change : la vignette ne correspond plus.
        std::fs::write(&source, b"un autre RAW, plus long").unwrap();
        assert_ne!(b, path_for(&source, Some(&s), 320, &dir).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
