//! Export des images rendues : JPEG, PNG et TIFF, avec le profil ICC de l'espace de
//! sortie incorporé (sans profil, un fichier Display P3 serait lu comme du sRGB).
//!
//! Les préréglages d'export (format, qualité, profondeur, espace, taille, dossier) sont
//! partagés par l'app et la ligne de commande : quelques préréglages de base, et ceux
//! de l'utilisateur, enregistrés dans un fichier JSON du dossier de LÜMA.

use std::path::{Path, PathBuf};

use image::{ExtendedColorType, ImageEncoder};
use luma_color::{OutputSpace, icc};
use luma_core::{Cancel, RgbImage};
use luma_project::Settings;
use serde::{Deserialize, Serialize};

use crate::metadata::insert_jpeg_xmp;
pub use crate::metadata::{Embedded, MetadataOptions, PhotoMetadata};
pub use crate::watermark::{Logo, Position, Watermark};
use crate::{Photo, Size};

/// Écrit l'image dans `path` : JPEG, PNG ou TIFF selon l'extension, 8 ou 16 bits (le
/// JPEG est toujours sur 8 bits), avec le profil ICC de l'espace de sortie.
pub fn write(path: &Path, img: &RgbImage, space: OutputSpace, quality: u8, bits: u8) -> Result<(), String> {
    write_with(path, img, space, quality, bits, &Embedded::default())
}

/// `write`, avec les métadonnées : l'EXIF dans les trois formats, le paquet XMP dans
/// les JPEG et les TIFF.
pub fn write_with(
    path: &Path,
    img: &RgbImage,
    space: OutputSpace,
    quality: u8,
    bits: u8,
    meta: &Embedded,
) -> Result<(), String> {
    write_file(path, img, space, quality, bits, meta, false)
}

/// `write_with` ; `web` : un JPEG allégé pour le web ([`crate::jpeg::encode_web`]).
/// Le fichier s'écrit d'un coup : dans un temporaire caché à côté, renommé une fois
/// complet. Un export coupé (disque plein, app arrêtée) ne laisse jamais un fichier
/// abîmé sous le nom voulu, ni n'abîme l'export d'avant du même nom.
pub fn write_file(
    path: &Path,
    img: &RgbImage,
    space: OutputSpace,
    quality: u8,
    bits: u8,
    meta: &Embedded,
    web: bool,
) -> Result<(), String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("jpg");
    let name = path.file_name().map_or_else(Default::default, |n| n.to_string_lossy());
    let tmp = path.with_file_name(format!(".{name}.partiel.{ext}"));
    let written = write_at(&tmp, img, space, quality, bits, meta, web)
        .and_then(|()| std::fs::rename(&tmp, path).map_err(|e| format!("{} : {e}", path.display())));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    // Les messages nomment le fichier voulu, pas le temporaire.
    written.map_err(|e| e.replace(&tmp.display().to_string(), &path.display().to_string()))
}

/// L'écriture elle‑même, dans `path` (JPEG, PNG ou TIFF selon son extension).
fn write_at(
    path: &Path,
    img: &RgbImage,
    space: OutputSpace,
    quality: u8,
    bits: u8,
    meta: &Embedded,
    web: bool,
) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg")
        .to_ascii_lowercase();
    let (w, h) = (img.width as u32, img.height as u32);
    let profile = icc::profile(space);
    let err = |e: image::ImageError| e.to_string();
    let sixteen = bits == 16 && ext != "jpg" && ext != "jpeg";
    let jpeg = matches!(ext.as_str(), "jpg" | "jpeg");
    // L'image en octets, sauf pour le JPEG ordinaire, converti bande par bande ; s'il a
    // un poids maximal, converti une fois pour tous ses codages.
    let (bytes, kind) = if sixteen {
        (luma_image::encode16_bytes(img, space), ExtendedColorType::Rgb16)
    } else if jpeg && !web && meta.max_bytes.is_none() {
        (Vec::new(), ExtendedColorType::Rgb8)
    } else {
        (luma_image::encode8(img, space), ExtendedColorType::Rgb8)
    };
    let create = || std::fs::File::create(path).map_err(|e| format!("{} : {e}", path.display()));
    match ext.as_str() {
        "jpg" | "jpeg" => {
            // Sur tous les cœurs, octet pour octet le JPEG d'avant ; en mémoire d'abord :
            // le paquet XMP s'insère après les premiers segments.
            let enc = luma_image::Encoder8::new(space);
            let encode = |quality: u8| -> Result<Vec<u8>, String> {
                let mut out = if web {
                    crate::jpeg::encode_web(&bytes, img.width, img.height, quality, &meta.exif, &profile)?
                } else if !bytes.is_empty() {
                    crate::jpeg::encode(&bytes, img.width, img.height, quality, &meta.exif, &profile)?
                } else {
                    let w = img.width;
                    let lines = |y: usize, x0: usize, dst: &mut [u8]| {
                        let start = y * w + x0;
                        enc.pixels(&img.data[start..start + dst.len() / 3], dst);
                    };
                    crate::jpeg::encode_with(&lines, img.width, img.height, quality, &meta.exif, &profile)?
                };
                if let Some(dpi) = meta.dpi {
                    crate::jpeg::set_density(&mut out, dpi);
                }
                if let Some(x) = &meta.xmp {
                    insert_jpeg_xmp(&mut out, x)?;
                }
                Ok(out)
            };
            // L'échantillon du poids maximal : une bande de huit lignes sur soixante‑quatre
            // (une ligne de blocs sur huit), sans métadonnées.
            let (width, height) = (img.width, img.height);
            let sample_height = height.div_ceil(64) * 8;
            let source_row = |y: usize| ((y / 8) * 64 + y % 8).min(height - 1);
            let sample = |quality: u8| -> Result<usize, String> {
                let out = if web {
                    let rows: Vec<u8> = (0..sample_height)
                        .flat_map(|y| {
                            let y = source_row(y);
                            bytes[y * width * 3..(y + 1) * width * 3].iter().copied()
                        })
                        .collect();
                    crate::jpeg::encode_web(&rows, width, sample_height, quality, &[], &[])?
                } else {
                    let lines = |y: usize, x0: usize, dst: &mut [u8]| {
                        let start = source_row(y) * width + x0;
                        enc.pixels(&img.data[start..start + dst.len() / 3], dst);
                    };
                    crate::jpeg::encode_with(&lines, width, sample_height, quality, &[], &[])?
                };
                Ok(out.len())
            };
            let out = match meta.max_bytes {
                Some(max) if width > 0 && height > 0 => under_weight(quality, max, encode, sample)?.0,
                _ => encode(quality)?,
            };
            std::fs::write(path, out).map_err(|e| format!("{} : {e}", path.display()))
        }
        "png" => {
            let Some(dpi) = meta.dpi else {
                let mut enc = image::codecs::png::PngEncoder::new(std::io::BufWriter::new(create()?));
                let _ = enc.set_icc_profile(profile);
                if !meta.exif.is_empty() {
                    let _ = enc.set_exif_metadata(meta.exif.clone());
                }
                return enc.write_image(&bytes, w, h, kind).map_err(err);
            };
            // Un tirage : le PNG en mémoire, puis sa résolution (bloc pHYs) juste après
            // l'en‑tête.
            let mut png = Vec::new();
            let mut enc = image::codecs::png::PngEncoder::new(&mut png);
            let _ = enc.set_icc_profile(profile);
            if !meta.exif.is_empty() {
                let _ = enc.set_exif_metadata(meta.exif.clone());
            }
            enc.write_image(&bytes, w, h, kind).map_err(err)?;
            insert_png_density(&mut png, dpi);
            std::fs::write(path, png).map_err(|e| format!("{} : {e}", path.display()))
        }
        "tif" | "tiff" => {
            // Écrit par LÜMA : une bande RVB non compressée, avec ICC, EXIF et XMP ; les
            // pixels à la suite de l'en‑tête, sans copie (déjà petit‑boutistes ici).
            let pixels = if sixteen && cfg!(target_endian = "big") {
                bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .flat_map(|b| u16::from_ne_bytes(*b).to_le_bytes())
                    .collect()
            } else {
                bytes
            };
            let header = crate::metadata::tiff_header(img.width, img.height, sixteen, pixels.len(), &profile, meta);
            let io = |e: std::io::Error| format!("{} : {e}", path.display());
            let mut f = create()?;
            std::io::Write::write_all(&mut f, &header).map_err(io)?;
            std::io::Write::write_all(&mut f, &pixels).map_err(io)
        }
        other => Err(format!("format de sortie inconnu « {other} » (jpg, png, tif)")),
    }
}

/// Le JPEG de la plus haute qualité, `quality` au plus, qui tient en `max` octets, et
/// cette qualité ; si même la qualité 1 est trop lourde, celui de qualité 1.
///
/// `sample(q)` : le poids d'un échantillon de l'image (une bande de blocs sur huit) à
/// la qualité `q`, huit fois moins cher qu'un codage entier. Mis à l'échelle du dernier
/// vrai codage, il dit quelle qualité essayer ensuite ; seuls les vrais codages
/// décident : la qualité rendue tient, celle juste au‑dessus non — le choix d'une
/// dichotomie sur les vrais poids, en trois codages le plus souvent au lieu de huit.
/// Trois essais de suite du même côté : le milieu, pour qu'un échantillon trompeur ne
/// coûte jamais beaucoup plus qu'une dichotomie.
fn under_weight(
    quality: u8,
    max: usize,
    encode: impl Fn(u8) -> Result<Vec<u8>, String>,
    sample: impl Fn(u8) -> Result<usize, String>,
) -> Result<(Vec<u8>, u8), String> {
    let quality = quality.clamp(1, 100);
    let first = encode(quality)?;
    if first.len() <= max || quality == 1 {
        return Ok((first, quality));
    }
    let mut samples = [None; 101];
    let mut sample_at = |q: u8| -> Result<usize, String> {
        if let Some(n) = samples[usize::from(q)] {
            return Ok(n);
        }
        let n = sample(q)?.max(1);
        samples[usize::from(q)] = Some(n);
        Ok(n)
    };
    // Entre une qualité qui tient (`lo`, ou 1 pas encore codée) et une trop lourde
    // (`hi`) ; `scale` : le vrai poids rapporté à l'échantillon, au dernier codage.
    let (mut lo, mut hi, mut best) = (1u8, quality, None);
    let mut scale = first.len() as f64 / sample_at(quality)? as f64;
    let mut streak = (true, 0u8);
    while hi - lo > 1 {
        let next = if streak.1 >= 3 {
            lo + (hi - lo) / 2
        } else {
            // La plus haute qualité entre les bornes dont l'échantillon, à l'échelle,
            // tient.
            let (mut a, mut b) = (lo, hi);
            while b - a > 1 {
                let mid = a + (b - a) / 2;
                if sample_at(mid)? as f64 * scale <= max as f64 {
                    a = mid;
                } else {
                    b = mid;
                }
            }
            a.max(lo + 1)
        };
        let out = encode(next)?;
        let fits = out.len() <= max;
        scale = out.len() as f64 / sample_at(next)? as f64;
        streak = if streak.0 == fits {
            (fits, streak.1 + 1)
        } else {
            (fits, 1)
        };
        if fits {
            (lo, best) = (next, Some(out));
        } else {
            hi = next;
        }
    }
    match best {
        Some(out) => Ok((out, lo)),
        None => Ok((encode(1)?, 1)),
    }
}

/// Le CRC‑32 des blocs PNG (polynôme 0xEDB88320).
fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

/// Ajoute la résolution `dpi` (bloc pHYs, en points par mètre) à un PNG, juste après
/// son en‑tête ; ce qui n'est pas un PNG ne bouge pas.
fn insert_png_density(png: &mut Vec<u8>, dpi: u32) {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if png.len() < 33 || png[..8] != SIGNATURE || &png[12..16] != b"IHDR" {
        return;
    }
    let per_metre = (f64::from(dpi) / 0.0254).round() as u32;
    let mut body = b"pHYs".to_vec();
    body.extend_from_slice(&per_metre.to_be_bytes());
    body.extend_from_slice(&per_metre.to_be_bytes());
    body.push(1);
    let mut chunk = 9u32.to_be_bytes().to_vec();
    chunk.extend_from_slice(&body);
    chunk.extend_from_slice(&crc32(&body).to_be_bytes());
    // La signature (8), puis l'en‑tête : longueur, nom, 13 octets, CRC.
    png.splice(33..33, chunk);
}

/// Le format de fichier de l'export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Jpeg,
    Png,
    Tiff,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::Tiff => "tif",
        }
    }

    pub fn parse(s: &str) -> Option<Format> {
        match s.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Some(Format::Jpeg),
            "png" => Some(Format::Png),
            "tif" | "tiff" => Some(Format::Tiff),
            _ => None,
        }
    }
}

/// Les réglages d'un export.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub format: Format,
    /// Qualité JPEG, 1 à 100.
    pub quality: u8,
    /// 8 ou 16 bits (PNG, TIFF).
    pub bits: u8,
    pub space: OutputSpace,
    /// La taille de l'image recadrée ; `None` : pleine définition.
    pub resize: Option<Resize>,
    /// `None` : un dossier `LÜMA/` à côté de chaque RAW.
    pub folder: Option<PathBuf>,
    /// Le modèle du nom des fichiers (voir [`file_name`]) ; `None` : le nom de l'original.
    pub name: Option<String>,
    /// L'accentuation de sortie, après le redimensionnement ; `None` : aucune.
    pub sharpen: Option<OutputSharpen>,
    /// Le filigrane texte, posé en dernier ; `None` : aucun.
    pub watermark: Option<Watermark>,
    /// Le logo en filigrane (PNG) ; `None` : aucun.
    pub logo: Option<Logo>,
    /// Les métadonnées recopiées et ajoutées.
    pub metadata: MetadataOptions,
    /// Un JPEG allégé pour le web (chrominance 4:2:0, tables de Huffman de l'image).
    pub web: bool,
    /// Agrandir l'image quand la taille demandée dépasse la pleine définition (jusqu'à
    /// quatre fois chaque côté, 200 Mpx au plus), au lieu de s'arrêter à sa taille.
    pub enlarge: bool,
    /// Le poids maximal d'un JPEG, en kilo‑octets (1 000 octets) : la qualité descend
    /// jusqu'à ce qu'il tienne ; `None` : la qualité demandée.
    pub max_kb: Option<u32>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: Format::Jpeg,
            quality: 92,
            bits: 8,
            space: OutputSpace::Srgb,
            resize: None,
            folder: None,
            name: None,
            sharpen: None,
            watermark: None,
            logo: None,
            metadata: MetadataOptions::default(),
            web: false,
            enlarge: false,
            max_kb: None,
        }
    }
}

/// Le support d'une image exportée : l'accentuation de sortie dépend de la façon dont il
/// rend les détails (l'encre s'étale sur un papier mat).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Medium {
    Screen,
    Matte,
    Glossy,
}

/// L'intensité de l'accentuation de sortie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    Low,
    Standard,
    High,
}

/// L'accentuation de sortie : appliquée à l'image à sa taille finale, pour compenser ce
/// que l'écran ou le papier adoucit. Elle s'ajoute à celle du panneau Détail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputSharpen {
    pub medium: Medium,
    pub strength: Strength,
}

impl OutputSharpen {
    /// Les réglages d'accentuation pour une image de ce grand côté. Écran : un rayon d'un
    /// demi‑pixel ; papier : un rayon plus large qui grandit avec l'image (une grande
    /// image s'imprime grande, vue de plus loin), plus fort sur papier mat.
    pub fn detail(self, long_edge: usize) -> luma_project::Detail {
        let k = match self.strength {
            Strength::Low => 0.6,
            Strength::Standard => 1.0,
            Strength::High => 1.5,
        };
        let print = (long_edge as f32 / 3000.0).clamp(1.0, 2.0);
        let (radius, amount) = match self.medium {
            Medium::Screen => (0.5, 45.0),
            Medium::Glossy => (0.9 * print, 60.0),
            Medium::Matte => (1.3 * print, 85.0),
        };
        luma_project::Detail {
            sharpen_amount: amount * k,
            sharpen_radius: radius,
            sharpen_detail: 50.0,
            // Les aplats (ciel, peau) ne prennent pas de grain.
            sharpen_masking: 15.0,
            ..luma_project::Detail::default()
        }
    }

    /// Accentue l'image (linéaire, à sa taille finale).
    pub fn apply(self, img: &mut RgbImage) {
        let d = self.detail(img.width.max(img.height));
        luma_image::detail::sharpen(img, &d, 1.0);
    }

    /// Lit `screen`, `matte:high`, `brillant:faible`… (support, puis intensité ;
    /// normale par défaut), en français ou en anglais.
    pub fn parse(s: &str) -> Option<OutputSharpen> {
        let s = s.trim().to_lowercase();
        let (m, k) = s.split_once(':').unwrap_or((&s, "standard"));
        let medium = match m.trim() {
            "screen" | "écran" | "ecran" => Medium::Screen,
            "matte" | "mat" => Medium::Matte,
            "glossy" | "brillant" => Medium::Glossy,
            _ => return None,
        };
        let strength = match k.trim() {
            "low" | "faible" => Strength::Low,
            "standard" | "normal" | "normale" => Strength::Standard,
            "high" | "forte" | "fort" => Strength::High,
            _ => return None,
        };
        Some(OutputSharpen { medium, strength })
    }

    /// L'écriture que `parse` relit.
    pub fn id(self) -> String {
        let m = match self.medium {
            Medium::Screen => "screen",
            Medium::Matte => "matte",
            Medium::Glossy => "glossy",
        };
        let k = match self.strength {
            Strength::Low => "low",
            Strength::Standard => "standard",
            Strength::High => "high",
        };
        format!("{m}:{k}")
    }

    /// En clair : « accentuation écran, normale ».
    pub fn describe(self) -> String {
        let m = match self.medium {
            Medium::Screen => "écran",
            Medium::Matte => "papier mat",
            Medium::Glossy => "papier brillant",
        };
        let k = match self.strength {
            Strength::Low => "faible",
            Strength::Standard => "normale",
            Strength::High => "forte",
        };
        format!("accentuation {m}, {k}")
    }
}

/// La taille d'un export, sans changer ses proportions (et sans agrandir l'image, sauf
/// avec [`Resize::fit`] qui le permet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resize {
    /// Le grand côté, en pixels.
    LongEdge(usize),
    /// Le petit côté, en pixels.
    ShortEdge(usize),
    /// L'image tient dans ce cadre (largeur × hauteur, en pixels).
    Fit { width: usize, height: usize },
    /// Le nombre de pixels, en millions.
    Megapixels(f32),
    /// Un tirage : l'image tient dans ce cadre (en centimètres, le grand côté de la photo
    /// sur le grand côté du cadre) à `dpi` points par pouce, inscrits dans le fichier.
    Print { width_cm: f32, height_cm: f32, dpi: u32 },
}

impl Resize {
    /// Les dimensions d'une image `width × height` après redimensionnement : jamais plus
    /// grandes, proportions gardées.
    pub fn apply(self, width: usize, height: usize) -> (usize, usize) {
        self.fit(width, height, false)
    }

    /// Comme [`Resize::apply`] ; avec `enlarge`, une taille demandée plus grande que
    /// l'image l'agrandit, jusqu'à quatre fois chaque côté et 200 Mpx.
    pub fn fit(self, width: usize, height: usize, enlarge: bool) -> (usize, usize) {
        let (w, h) = (width.max(1) as f64, height.max(1) as f64);
        let scale = match self {
            Resize::LongEdge(n) => n as f64 / w.max(h),
            Resize::ShortEdge(n) => n as f64 / w.min(h),
            Resize::Fit { width: fw, height: fh } => (fw as f64 / w).min(fh as f64 / h),
            Resize::Megapixels(m) => (f64::from(m) * 1e6 / (w * h)).sqrt(),
            Resize::Print { .. } => {
                let (bw, bh) = self.print_pixels(width >= height);
                (bw as f64 / w).min(bh as f64 / h)
            }
        };
        if scale.is_nan() || scale == 1.0 || (scale > 1.0 && !enlarge) {
            return (width, height);
        }
        if scale > 1.0 {
            let most = 4.0f64.min((200e6 / (w * h)).sqrt()).max(1.0);
            if scale > most {
                // Arrondi vers le bas : le plafond n'est jamais dépassé.
                return (
                    ((w * most).floor() as usize).max(1),
                    ((h * most).floor() as usize).max(1),
                );
            }
        }
        let out = |v: f64| ((v * scale).round() as usize).max(1);
        // Le côté visé est tenu exactement (l'arrondi de l'autre suit).
        match self {
            Resize::LongEdge(n) if width >= height => (n.max(1), out(h)),
            Resize::LongEdge(n) => (out(w), n.max(1)),
            Resize::ShortEdge(n) if width <= height => (n.max(1), out(h)),
            Resize::ShortEdge(n) => (out(w), n.max(1)),
            _ => (out(w), out(h)),
        }
    }

    /// Le cadre d'un tirage en pixels, posé dans le sens de la photo (`landscape` : plus
    /// large que haute). `(0, 0)` pour une autre taille.
    pub fn print_pixels(self, landscape: bool) -> (usize, usize) {
        let Resize::Print {
            width_cm,
            height_cm,
            dpi,
        } = self
        else {
            return (0, 0);
        };
        let px = |cm: f32| ((f64::from(cm) / 2.54 * f64::from(dpi)).round() as usize).max(1);
        let (long, short) = (px(width_cm.max(height_cm)), px(width_cm.min(height_cm)));
        if landscape { (long, short) } else { (short, long) }
    }

    /// Les points par pouce à inscrire dans le fichier (un tirage seulement).
    pub fn dpi(self) -> Option<u32> {
        match self {
            Resize::Print { dpi, .. } => Some(dpi),
            _ => None,
        }
    }

    /// Lit une taille : `2048` ou `long:2048` (grand côté), `short:1080` (petit côté),
    /// `1920x1080` (cadre), `12mp` (mégapixels), `30x40cm@300` (tirage, 300 ppp par
    /// défaut). `None` si la valeur est vide ou nulle.
    pub fn parse(s: &str) -> Option<Resize> {
        let s = s.trim().to_lowercase();
        let num = |v: &str| v.trim().parse::<usize>().ok().filter(|&n| n > 0);
        if s.contains("cm") {
            let (size, dpi) = match s.split_once('@') {
                Some((a, b)) => (
                    a,
                    b.trim()
                        .trim_end_matches("ppp")
                        .trim_end_matches("dpi")
                        .trim()
                        .parse::<u32>()
                        .ok()?,
                ),
                None => (s.as_str(), 300),
            };
            let (a, b) = size.trim().trim_end_matches("cm").split_once(['x', '×'])?;
            let cm = |v: &str| {
                v.trim()
                    .trim_end_matches("cm")
                    .trim()
                    .replace(',', ".")
                    .parse::<f32>()
                    .ok()
                    .filter(|v| v.is_finite() && *v > 0.0 && *v <= 500.0)
            };
            return (1..=2400).contains(&dpi).then_some(Resize::Print {
                width_cm: cm(a)?,
                height_cm: cm(b)?,
                dpi,
            });
        }
        if let Some(v) = s.strip_suffix("mpx").or_else(|| s.strip_suffix("mp")) {
            let m: f32 = v.trim().replace(',', ".").parse().ok()?;
            return (m > 0.0 && m.is_finite()).then_some(Resize::Megapixels(m));
        }
        if let Some((a, b)) = s.split_once(['x', '×']) {
            return Some(Resize::Fit {
                width: num(a)?,
                height: num(b)?,
            });
        }
        if let Some(v) = s.strip_prefix("short:").or_else(|| s.strip_prefix("petit:")) {
            return num(v).map(Resize::ShortEdge);
        }
        num(s
            .strip_prefix("long:")
            .or_else(|| s.strip_prefix("grand:"))
            .unwrap_or(&s))
        .map(Resize::LongEdge)
    }

    /// L'écriture que `parse` relit.
    pub fn id(self) -> String {
        match self {
            Resize::LongEdge(n) => format!("long:{n}"),
            Resize::ShortEdge(n) => format!("short:{n}"),
            Resize::Fit { width, height } => format!("{width}x{height}"),
            Resize::Megapixels(m) => format!("{m}mp"),
            Resize::Print {
                width_cm,
                height_cm,
                dpi,
            } => format!("{width_cm}x{height_cm}cm@{dpi}"),
        }
    }

    /// En clair : « 2048 px », « petit côté 1080 px », « 1920 × 1080 px », « 12 Mpx ».
    pub fn describe(self) -> String {
        match self {
            Resize::LongEdge(n) => format!("{n} px"),
            Resize::ShortEdge(n) => format!("petit côté {n} px"),
            Resize::Fit { width, height } => format!("{width} × {height} px"),
            Resize::Megapixels(m) => format!("{m} Mpx"),
            Resize::Print {
                width_cm,
                height_cm,
                dpi,
            } => {
                let cm = |v: f32| format!("{v}").replace('.', ",");
                format!("{} × {} cm à {dpi} ppp", cm(width_cm), cm(height_cm))
            }
        }
    }
}

/// Le fichier de sortie d'une photo : son dossier, le nom donné par le modèle (ou celui
/// de l'original), l'extension du format.
pub fn output_path(source: &Path, options: &Options, fields: &NameFields) -> PathBuf {
    let dir = match &options.folder {
        Some(d) => d.clone(),
        None => source.parent().unwrap_or(Path::new(".")).join("LÜMA"),
    };
    let name = match &options.name {
        Some(t) if !t.trim().is_empty() => file_name(t, fields),
        _ => fields.name.clone(),
    };
    let name = if name.is_empty() { "photo".to_owned() } else { name };
    dir.join(format!("{name}.{}", options.format.extension()))
}

/// Un fichier de sortie pas encore pris dans cet export : `IMG-2.jpg`, `IMG-3.jpg`… si
/// le modèle donne le même nom à plusieurs photos, ou le nom d'un original réservé par
/// [`reserve_sources`].
pub fn unique(path: PathBuf, taken: &mut std::collections::HashSet<PathBuf>) -> PathBuf {
    let mut out = path.clone();
    let mut n = 2;
    let is_taken = |p: &Path, taken: &std::collections::HashSet<PathBuf>| {
        taken.contains(p) || p.canonicalize().is_ok_and(|c| taken.contains(&c))
    };
    while is_taken(&out, taken) {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let ext = path.extension().unwrap_or_default().to_string_lossy();
        out = path.with_file_name(format!("{stem}-{n}.{ext}"));
        n += 1;
    }
    taken.insert(out.clone());
    out
}

/// Réserve les originaux d'un export (leur chemin absolu) : [`unique`] ne les écrase
/// jamais, même quand le dossier de sortie est le leur.
pub fn reserve_sources<'a>(
    taken: &mut std::collections::HashSet<PathBuf>,
    sources: impl IntoIterator<Item = &'a Path>,
) {
    taken.extend(sources.into_iter().filter_map(|s| s.canonicalize().ok()));
}

/// Ce qu'un modèle de nom peut citer, pour une photo.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NameFields {
    /// Le nom de l'original, sans extension.
    pub name: String,
    /// Année, mois, jour, heure, minute, seconde de la prise de vue (à défaut, la date
    /// du fichier) ; `None` si aucune n'est connue.
    pub date: Option<[u32; 6]>,
    pub make: String,
    pub model: String,
    pub lens: String,
    pub iso: Option<u32>,
    pub rating: u8,
    pub label: String,
    /// Le rang de la photo dans l'export, à partir de 1.
    pub sequence: usize,
}

impl NameFields {
    /// Le nom de l'original et la date du fichier, sans rien lire d'autre.
    pub fn of_file(source: &Path) -> NameFields {
        // Une copie virtuelle : le nom de son original et son numéro (`IMG_0001-2`).
        let copy = luma_project::copies::number(source);
        let source = &luma_project::copies::source(source);
        let date = std::fs::metadata(source)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| civil(d.as_secs()));
        NameFields {
            name: source
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
                + &copy.map(|n| format!("-{n}")).unwrap_or_default(),
            date,
            sequence: 1,
            ..NameFields::default()
        }
    }

    /// Ce que la photothèque sait d'une photo, sans l'ouvrir : la date de prise de vue
    /// (`AAAA-MM-JJTHH:MM:SS`, celle du catalogue), le boîtier, l'objectif, l'ISO, la note
    /// et le libellé.
    #[allow(clippy::too_many_arguments)]
    pub fn of_details(
        source: &Path,
        date: &str,
        make: &str,
        model: &str,
        lens: &str,
        iso: Option<u32>,
        rating: u8,
        label: &str,
    ) -> NameFields {
        let mut f = NameFields::of_file(source);
        if let Some(d) = exif_date(date) {
            f.date = Some(d);
        }
        f.make = make.trim().to_owned();
        f.model = model.trim().to_owned();
        f.lens = lens.trim().to_owned();
        f.iso = iso;
        f.rating = rating;
        f.label = label.to_owned();
        f
    }

    /// Tout ce que l'on sait d'une photo ouverte : son EXIF, sa note et son libellé.
    pub fn of_photo(source: &Path, photo: &Photo, meta: &luma_project::xmp::Meta, sequence: usize) -> NameFields {
        let raw = photo.raw();
        let mut f = NameFields::of_file(source);
        if let Some(d) = raw.exif.date.as_deref().and_then(exif_date) {
            f.date = Some(d);
        }
        f.make = raw.make.trim().to_owned();
        f.model = raw.model.trim().to_owned();
        f.lens = raw.exif.lens.as_deref().unwrap_or("").trim().to_owned();
        f.iso = raw.exif.iso;
        f.rating = meta.rating;
        f.label = meta.label.clone();
        f.sequence = sequence;
        f
    }
}

/// Une date EXIF (`2026:10:26 14:03:00`, ou avec des tirets, comme celles du
/// catalogue) en ses six nombres.
pub fn exif_date(s: &str) -> Option<[u32; 6]> {
    let mut n = [0u32; 6];
    let mut parts = s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty());
    for (i, v) in n.iter_mut().enumerate() {
        match parts.next().and_then(|p| p.parse().ok()) {
            Some(x) => *v = x,
            None if i >= 3 => break,
            None => return None,
        }
    }
    (n[0] > 0 && (1..=12).contains(&n[1]) && (1..=31).contains(&n[2])).then_some(n)
}

/// Des secondes depuis 1970 (UTC) en date du calendrier (algorithme des jours civils).
fn civil(secs: u64) -> [u32; 6] {
    let days = (secs / 86_400) as i64;
    let rem = (secs % 86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as u32;
    [year, month, day, rem / 3600, rem % 3600 / 60, rem % 60]
}

/// Les repères d'un modèle de nom, en français (les noms affichés), avec leurs
/// équivalents anglais et espagnols, et sans accents.
const TOKENS: &[(&str, &[&str])] = &[
    ("nom", &["name", "nombre"]),
    ("date", &["fecha"]),
    ("heure", &["time", "hora"]),
    ("année", &["annee", "year", "año", "ano"]),
    ("mois", &["month", "mes"]),
    ("jour", &["day", "día", "dia"]),
    ("séquence", &["sequence", "secuencia"]),
    ("boîtier", &["boitier", "camera", "cámara", "camara"]),
    ("marque", &["make", "marca"]),
    ("objectif", &["lens", "objetivo"]),
    ("iso", &[]),
    ("note", &["rating", "valoración", "valoracion"]),
    ("libellé", &["libelle", "label", "etiqueta"]),
];

/// Les repères d'un modèle, dans l'ordre : `{nom}`, `{date}`…
pub fn tokens() -> impl Iterator<Item = &'static str> {
    TOKENS.iter().map(|t| t.0)
}

/// Le repère français d'un nom de repère (`séquence` pour `sequence`), et le nombre de
/// chiffres demandé après `:` (`{séquence:3}`).
fn token(raw: &str) -> Option<(&'static str, Option<usize>)> {
    let (name, digits) = match raw.split_once(':') {
        Some((n, d)) => (
            n,
            Some(d.trim().parse::<usize>().ok().filter(|&d| (1..=9).contains(&d))?),
        ),
        None => (raw, None),
    };
    let name = name.trim().to_lowercase();
    TOKENS
        .iter()
        .find(|(fr, others)| *fr == name || others.contains(&name.as_str()))
        .map(|t| (t.0, digits))
}

/// Vérifie un modèle : chaque `{…}` est un repère connu, les accolades sont fermées.
pub fn check_template(template: &str) -> Result<(), String> {
    let mut rest = template;
    loop {
        match (rest.find('{'), rest.find('}')) {
            (None, None) => return Ok(()),
            (_, Some(c)) if rest.find('{').is_none_or(|o| c < o) => {
                return Err(format!("accolade non ouverte dans « {template} »"));
            }
            (Some(o), Some(c)) if !rest[o + 1..c].contains('{') => {
                let inner = &rest[o + 1..c];
                if token(inner).is_none() {
                    let known: Vec<String> = tokens().map(|t| format!("{{{t}}}")).collect();
                    return Err(format!("repère inconnu « {{{inner}}} » ({})", known.join(" ")));
                }
                rest = &rest[c + 1..];
            }
            _ => return Err(format!("accolade non fermée dans « {template} »")),
        }
    }
}

/// Un texte sans les caractères interdits dans un nom de fichier.
fn clean(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Le nom de fichier (sans extension) qu'un modèle donne pour une photo :
/// `{date}_{séquence}` → `2026-10-26_0001`. Un `/` du modèle crée un sous‑dossier
/// (`{année}/{mois}/{nom}`) ; un repère inconnu reste tel quel ; un repère sans valeur
/// (pas d'objectif connu…) disparaît.
pub fn file_name(template: &str, f: &NameFields) -> String {
    let two = |v: u32| format!("{v:02}");
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let inner = &after[..close];
        let value = match token(inner) {
            None => format!("{{{inner}}}"),
            Some((t, digits)) => {
                let d = f.date;
                let v = match t {
                    "nom" => f.name.clone(),
                    "date" => d.map_or_else(String::new, |d| format!("{:04}-{}-{}", d[0], two(d[1]), two(d[2]))),
                    "heure" => d.map_or_else(String::new, |d| format!("{}{}{}", two(d[3]), two(d[4]), two(d[5]))),
                    "année" => d.map_or_else(String::new, |d| format!("{:04}", d[0])),
                    "mois" => d.map_or_else(String::new, |d| two(d[1])),
                    "jour" => d.map_or_else(String::new, |d| two(d[2])),
                    "séquence" => format!("{:0w$}", f.sequence, w = digits.unwrap_or(4)),
                    "boîtier" => camera(&f.make, &f.model),
                    "marque" => f.make.clone(),
                    "objectif" => f.lens.clone(),
                    "iso" => f.iso.map_or_else(String::new, |i| i.to_string()),
                    "note" => f.rating.to_string(),
                    _ => f.label.clone(),
                };
                clean(&v)
            }
        };
        out.push_str(&value);
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    // Les sous‑dossiers : chaque partie nettoyée, ni vide, ni « . », ni « .. ».
    out.split(['/', '\\'])
        .map(clean)
        .filter(|p| !p.is_empty() && p != "." && p != "..")
        .collect::<Vec<_>>()
        .join("/")
}

/// « Sony ILCE-7M3 », sans répéter la marque quand le modèle la contient déjà.
fn camera(make: &str, model: &str) -> String {
    let first = make.split_whitespace().next().unwrap_or("");
    if make.is_empty() || (!first.is_empty() && model.to_lowercase().starts_with(&first.to_lowercase())) {
        model.to_owned()
    } else if model.is_empty() {
        make.to_owned()
    } else {
        format!("{first} {model}")
    }
}

/// Développe et écrit une photo selon les options (le dossier est créé au besoin) ;
/// `meta` (note, libellé, mots‑clés du XMP) va dans les métadonnées de l'export.
pub fn develop(
    photo: &Photo,
    settings: &Settings,
    out: &Path,
    options: &Options,
    meta: &luma_project::xmp::Meta,
) -> Result<(), String> {
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    }
    let size = match options.resize {
        None => Size::Full,
        Some(r) => {
            // La taille demandée est celle de l'image recadrée : son grand côté, estimé
            // sur le recadrage, puis on rend un peu plus grand et on réduit exactement.
            let c = &settings.crop;
            let (fx, fy) = ((c.right - c.left).max(0.05), (c.bottom - c.top).max(0.05));
            let frac = fx.min(fy);
            let (w, h) = photo.full_size();
            let (cw, ch) = ((w as f32 * fx).round() as usize, (h as f32 * fy).round() as usize);
            let (tw, th) = r.fit(cw.max(1), ch.max(1), options.enlarge);
            let pre = (tw.max(th) as f32 / frac).ceil() as usize;
            if pre >= w.max(h) { Size::Full } else { Size::Fit(pre) }
        }
    };
    let rendered = photo
        .render(settings, size, &Cancel::new())
        .map_err(|e| e.to_string())?;
    let mut image = rendered.image;
    if let Some(r) = options.resize {
        let (w, h) = r.fit(image.width, image.height, options.enlarge);
        if w > image.width || h > image.height {
            image = luma_image::enlarge::enlarge(&image, w, h);
        } else if (w, h) != (image.width, image.height) {
            image = image.resize_area(w, h);
        }
    }
    if let Some(sh) = options.sharpen {
        sh.apply(&mut image);
    }
    // Le filigrane après l'accentuation : ses lettres ne sont pas accentuées.
    if let Some(l) = &options.logo {
        l.apply(&mut image)?;
    }
    if let Some(w) = &options.watermark {
        w.apply(&mut image);
    }
    let bits = if options.format == Format::Jpeg {
        8
    } else {
        options.bits
    };
    let raw = photo.raw();
    let mut embedded = Embedded::new(
        &PhotoMetadata {
            make: raw.make.clone(),
            model: raw.model.clone(),
            exif: raw.exif.clone(),
            meta: meta.clone(),
        },
        &options.metadata,
        image.width,
        image.height,
    );
    embedded.dpi = options.resize.and_then(Resize::dpi);
    if options.format == Format::Jpeg {
        embedded.max_bytes = options.max_kb.map(|k| k as usize * 1000);
    }
    let web = options.web && options.format == Format::Jpeg;
    write_file(out, &image, options.space, options.quality, bits, &embedded, web)
}

/// Un préréglage d'export : un nom et des options.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    pub options: Options,
}

/// Les préréglages de base, toujours là.
pub fn builtin() -> Vec<Preset> {
    let preset = |name: &str, options| Preset {
        name: name.into(),
        options,
    };
    vec![
        preset(
            "Web",
            Options {
                quality: 85,
                resize: Some(Resize::LongEdge(2048)),
                web: true,
                ..Options::default()
            },
        ),
        preset(
            "Pleine définition",
            Options {
                quality: 95,
                ..Options::default()
            },
        ),
        preset(
            "Écran P3",
            Options {
                space: OutputSpace::DisplayP3,
                resize: Some(Resize::LongEdge(3840)),
                ..Options::default()
            },
        ),
        preset(
            "Impression",
            Options {
                format: Format::Tiff,
                bits: 16,
                space: OutputSpace::ProPhoto,
                ..Options::default()
            },
        ),
    ]
}

/// Le nom court d'un espace, tel que `OutputSpace::parse` le relit.
pub fn space_id(space: OutputSpace) -> &'static str {
    match space {
        OutputSpace::Srgb => "srgb",
        OutputSpace::DisplayP3 => "p3",
        OutputSpace::Rec2020 => "rec2020",
        OutputSpace::ProPhoto => "prophoto",
    }
}

/// Version du fichier des préréglages.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct StoredPreset {
    name: String,
    format: String,
    quality: u8,
    bits: u8,
    space: String,
    /// Les fichiers d'avant `resize` : le grand côté seul.
    #[serde(default, skip_serializing)]
    long_edge: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resize: Option<String>,
    #[serde(default)]
    folder: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sharpen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    watermark: Option<StoredWatermark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logo: Option<StoredLogo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    metadata: Option<StoredMetadata>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    web: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    enlarge: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    max_kb: Option<u32>,
}

/// Les métadonnées, seulement si elles diffèrent de celles d'un export ordinaire.
#[derive(Serialize, Deserialize)]
struct StoredMetadata {
    #[serde(default = "yes")]
    copy_exif: bool,
    #[serde(default)]
    keep_gps: bool,
    #[serde(default = "yes")]
    keywords: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    copyright: Option<String>,
}

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct StoredLogo {
    path: PathBuf,
    position: String,
    size: f32,
    opacity: f32,
}

#[derive(Serialize, Deserialize)]
struct StoredWatermark {
    text: String,
    position: String,
    size: f32,
    opacity: f32,
    #[serde(default)]
    dark: bool,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    presets: Vec<StoredPreset>,
}

impl StoredPreset {
    fn new(p: &Preset) -> StoredPreset {
        let o = &p.options;
        StoredPreset {
            name: p.name.clone(),
            format: o.format.extension().into(),
            quality: o.quality,
            bits: o.bits,
            space: space_id(o.space).into(),
            long_edge: None,
            resize: o.resize.map(Resize::id),
            folder: o.folder.clone(),
            name_template: o.name.clone(),
            sharpen: o.sharpen.map(OutputSharpen::id),
            watermark: o.watermark.as_ref().map(|w| StoredWatermark {
                text: w.text.clone(),
                position: w.position.id().into(),
                size: w.size,
                opacity: w.opacity,
                dark: w.dark,
            }),
            logo: o.logo.as_ref().map(|l| StoredLogo {
                path: l.path.clone(),
                position: l.position.id().into(),
                size: l.size,
                opacity: l.opacity,
            }),
            metadata: (o.metadata != MetadataOptions::default()).then(|| StoredMetadata {
                copy_exif: o.metadata.copy_exif,
                keep_gps: o.metadata.keep_gps,
                keywords: o.metadata.keywords,
                author: o.metadata.author.clone(),
                copyright: o.metadata.copyright.clone(),
            }),
            web: o.web,
            enlarge: o.enlarge,
            max_kb: o.max_kb,
        }
    }

    /// `None` si une valeur est inconnue : le préréglage est ignoré plutôt que faussé.
    fn preset(self) -> Option<Preset> {
        Some(Preset {
            name: self.name,
            options: Options {
                format: Format::parse(&self.format)?,
                quality: self.quality.clamp(1, 100),
                bits: if self.bits == 16 { 16 } else { 8 },
                space: OutputSpace::parse(&self.space)?,
                resize: match self.resize {
                    Some(r) => Some(Resize::parse(&r)?),
                    None => self.long_edge.filter(|&n| n > 0).map(Resize::LongEdge),
                },
                folder: self.folder,
                name: self.name_template.filter(|t| !t.trim().is_empty()),
                sharpen: match self.sharpen {
                    Some(v) => Some(OutputSharpen::parse(&v)?),
                    None => None,
                },
                watermark: match self.watermark {
                    Some(w) if !w.text.trim().is_empty() => Some(Watermark {
                        position: Position::parse(&w.position)?,
                        size: if w.size.is_finite() {
                            w.size.clamp(0.5, 25.0)
                        } else {
                            3.0
                        },
                        opacity: if w.opacity.is_finite() {
                            w.opacity.clamp(0.0, 1.0)
                        } else {
                            0.7
                        },
                        dark: w.dark,
                        text: w.text,
                    }),
                    _ => None,
                },
                logo: match self.logo {
                    Some(l) => Some(Logo {
                        position: Position::parse(&l.position)?,
                        size: if l.size.is_finite() {
                            l.size.clamp(1.0, 50.0)
                        } else {
                            12.0
                        },
                        opacity: if l.opacity.is_finite() {
                            l.opacity.clamp(0.0, 1.0)
                        } else {
                            0.8
                        },
                        path: l.path,
                    }),
                    None => None,
                },
                metadata: self
                    .metadata
                    .map_or_else(MetadataOptions::default, |m| MetadataOptions {
                        copy_exif: m.copy_exif,
                        keep_gps: m.keep_gps,
                        keywords: m.keywords,
                        author: m.author.filter(|a| !a.trim().is_empty()),
                        copyright: m.copyright.filter(|c| !c.trim().is_empty()),
                    }),
                web: self.web,
                enlarge: self.enlarge,
                max_kb: self.max_kb.filter(|&k| k > 0),
            },
        })
    }
}

/// Le fichier des préréglages d'export, dans le dossier de LÜMA : sur Mac
/// `~/Library/Application Support/LÜMA/Exports.json`, sur iPhone et iPad
/// `Library/Application Support/Exports.json`, sur Windows `%APPDATA%\LÜMA\Exports.json`,
/// ailleurs `~/.local/share/luma/exports.json`.
/// `LUMA_EXPORT_PRESETS` le remplace.
pub fn presets_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LUMA_EXPORT_PRESETS") {
        return Some(PathBuf::from(p));
    }
    if cfg!(windows) {
        return Some(luma_core::windows_dir(false)?.join("Exports.json"));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    Some(if cfg!(target_os = "ios") {
        home.join("Library/Application Support/Exports.json")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support/LÜMA/Exports.json")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map_or_else(|| home.join(".local/share"), PathBuf::from)
            .join("luma/exports.json")
    })
}

/// Les préréglages de l'utilisateur ; ceux de base restent accessibles par leur nom, et
/// un préréglage de l'utilisateur du même nom les remplace.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Presets {
    pub user: Vec<Preset>,
}

impl Presets {
    /// Lit le fichier ; vide s'il n'existe pas, est illisible ou d'une autre version.
    pub fn load(path: &Path) -> Presets {
        let Ok(bytes) = std::fs::read(path) else {
            return Presets::default();
        };
        // Préréglage par préréglage : un seul abîmé (une valeur impossible, retouchée à
        // la main) ne fait pas perdre les autres.
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            // Rien de lisible : le fichier est mis de côté avant que le prochain
            // enregistrement ne le remplace, l'utilisateur peut le reprendre.
            let aside = path.with_extension("json.illisible");
            if !aside.exists() {
                let _ = std::fs::write(&aside, &bytes);
            }
            return Presets::default();
        };
        if value.get("version").and_then(serde_json::Value::as_u64) != Some(u64::from(VERSION)) {
            return Presets::default();
        }
        let user = value
            .get("presets")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| serde_json::from_value::<StoredPreset>(p.clone()).ok())
            .filter_map(StoredPreset::preset)
            .collect();
        Presets { user }
    }

    /// Écrit le fichier (le dossier est créé au besoin), d'un coup : un fichier temporaire
    /// renommé, pour ne jamais laisser un fichier à moitié écrit.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        let stored = Stored {
            version: VERSION,
            presets: self.user.iter().map(StoredPreset::new).collect(),
        };
        let json = serde_json::to_vec_pretty(&stored).map_err(|e| e.to_string())?;
        luma_core::save(path, json).map_err(|e| format!("{} : {e}", path.display()))
    }

    /// Tous les préréglages : ceux de base (sauf remplacés), puis ceux de l'utilisateur.
    pub fn all(&self) -> Vec<Preset> {
        let mut all: Vec<Preset> = builtin()
            .into_iter()
            .filter(|b| !self.user.iter().any(|u| same_name(&u.name, &b.name)))
            .collect();
        all.extend(self.user.iter().cloned());
        all
    }

    /// Le préréglage de ce nom (sans tenir compte de la casse).
    pub fn get(&self, name: &str) -> Option<Preset> {
        self.all().into_iter().find(|p| same_name(&p.name, name))
    }

    /// Ajoute le préréglage ou remplace celui du même nom.
    pub fn set(&mut self, preset: Preset) {
        match self.user.iter_mut().find(|p| same_name(&p.name, &preset.name)) {
            Some(p) => *p = preset,
            None => self.user.push(preset),
        }
    }

    /// Retire le préréglage de l'utilisateur de ce nom ; `false` s'il n'y en a pas.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.user.len();
        self.user.retain(|p| !same_name(&p.name, name));
        self.user.len() != before
    }
}

fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// Une ligne de description : « JPEG 85, sRGB, 2048 px, LÜMA/ », puis le modèle de nom
/// s'il y en a un.
pub fn describe(o: &Options) -> String {
    let format = match o.format {
        Format::Jpeg => match o.max_kb {
            Some(k) => format!("JPEG {}, {k} Ko au plus", o.quality),
            None => format!("JPEG {}", o.quality),
        },
        Format::Png => format!("PNG {} bits", o.bits),
        Format::Tiff => format!("TIFF {} bits", o.bits),
    };
    let space = match o.space {
        OutputSpace::Srgb => "sRGB",
        OutputSpace::DisplayP3 => "Display P3",
        OutputSpace::Rec2020 => "Rec. 2020",
        OutputSpace::ProPhoto => "ProPhoto RGB",
    };
    let size = o
        .resize
        .map_or_else(|| "pleine définition".to_owned(), Resize::describe);
    let folder = o
        .folder
        .as_ref()
        .map_or_else(|| "LÜMA/".to_owned(), |d| d.display().to_string());
    let mut out = format!("{format}, {space}, {size}, {folder}");
    if o.enlarge && o.resize.is_some() {
        out += ", agrandie au besoin";
    }
    if let Some(sh) = o.sharpen {
        out += &format!(", {}", sh.describe());
    }
    if let Some(l) = &o.logo {
        out += &format!(", {}", l.describe());
    }
    if let Some(m) = o.metadata.describe() {
        out += &format!(", {m}");
    }
    if o.web && o.format == Format::Jpeg {
        out += ", allégé pour le web";
    }
    if let Some(w) = &o.watermark {
        out += &format!(", {}", w.describe());
    }
    if let Some(n) = &o.name {
        out += &format!(", {n}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiff_exports_decode_like_before() {
        // Les mêmes pixels que l'ancien encodeur TIFF, en 8 et en 16 bits.
        let img = RgbImage::from_fn(37, 23, |x, y| [x as f32 / 40.0, y as f32 / 25.0, 0.3]);
        let dir = std::env::temp_dir().join(format!("luma-tiff-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for bits in [8, 16] {
            let path = dir.join(format!("x{bits}.tif"));
            write(&path, &img, OutputSpace::ProPhoto, 92, bits).unwrap();
            let mine = image::open(&path).unwrap();
            let (bytes, kind) = if bits == 16 {
                let px = luma_image::encode16(&img, OutputSpace::ProPhoto);
                (
                    px.iter().flat_map(|v| v.to_ne_bytes()).collect::<Vec<u8>>(),
                    ExtendedColorType::Rgb16,
                )
            } else {
                (
                    luma_image::encode8(&img, OutputSpace::ProPhoto),
                    ExtendedColorType::Rgb8,
                )
            };
            let mut old = std::io::Cursor::new(Vec::new());
            image::codecs::tiff::TiffEncoder::new(&mut old)
                .write_image(&bytes, 37, 23, kind)
                .unwrap();
            let before = image::load_from_memory(old.get_ref()).unwrap();
            assert_eq!(mine.color(), before.color());
            assert_eq!(mine.as_bytes(), before.as_bytes(), "{bits} bits");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn output_paths() {
        let mut o = Options::default();
        let src = Path::new("/x/IMG_1.CR3");
        let f = NameFields::of_file(src);
        assert_eq!(output_path(src, &o, &f), PathBuf::from("/x/LÜMA/IMG_1.jpg"));
        // Une copie virtuelle : le nom de son original, suivi de son numéro.
        let copy = luma_project::copies::path(src, 2);
        assert_eq!(
            output_path(&copy, &o, &NameFields::of_file(&copy)),
            PathBuf::from("/x/LÜMA/IMG_1-2.jpg")
        );
        o.format = Format::Tiff;
        o.folder = Some("/sortie".into());
        assert_eq!(output_path(src, &o, &f), PathBuf::from("/sortie/IMG_1.tif"));
        o.name = Some("{année}/{nom}_{séquence:2}".into());
        let f = NameFields {
            date: Some([2026, 10, 26, 14, 3, 0]),
            sequence: 7,
            ..f
        };
        assert_eq!(output_path(src, &o, &f), PathBuf::from("/sortie/2026/IMG_1_07.tif"));
        // Un modèle vide garde le nom de l'original.
        o.name = Some("  ".into());
        assert_eq!(output_path(src, &o, &f), PathBuf::from("/sortie/IMG_1.tif"));
    }

    #[test]
    fn output_sharpening_by_medium_and_strength() {
        let all = [Medium::Screen, Medium::Matte, Medium::Glossy]
            .into_iter()
            .flat_map(|medium| {
                [Strength::Low, Strength::Standard, Strength::High].map(|strength| OutputSharpen { medium, strength })
            });
        for sh in all {
            assert_eq!(OutputSharpen::parse(&sh.id()), Some(sh));
        }
        assert_eq!(
            OutputSharpen::parse("écran"),
            Some(OutputSharpen {
                medium: Medium::Screen,
                strength: Strength::Standard
            })
        );
        assert_eq!(
            OutputSharpen::parse("Brillant:Forte"),
            Some(OutputSharpen {
                medium: Medium::Glossy,
                strength: Strength::High
            })
        );
        assert_eq!(OutputSharpen::parse("toile"), None);
        assert_eq!(OutputSharpen::parse("mat:extrême"), None);
        let d = |m, k, n| OutputSharpen { medium: m, strength: k }.detail(n);
        // Le papier mat demande plus que le brillant, qui demande plus que l'écran.
        let (s, g, m) = (
            d(Medium::Screen, Strength::Standard, 2000),
            d(Medium::Glossy, Strength::Standard, 2000),
            d(Medium::Matte, Strength::Standard, 2000),
        );
        assert!(s.sharpen_radius < g.sharpen_radius && g.sharpen_radius < m.sharpen_radius);
        assert!(s.sharpen_amount < g.sharpen_amount && g.sharpen_amount < m.sharpen_amount);
        assert!(d(Medium::Screen, Strength::Low, 2000).sharpen_amount < s.sharpen_amount);
        assert!(d(Medium::Screen, Strength::High, 2000).sharpen_amount > s.sharpen_amount);
        // Sur papier, le rayon suit la taille de l'image ; pas sur écran.
        assert!(d(Medium::Matte, Strength::Standard, 6000).sharpen_radius > m.sharpen_radius);
        assert_eq!(
            d(Medium::Screen, Strength::Standard, 6000).sharpen_radius,
            s.sharpen_radius
        );
        assert_eq!(
            OutputSharpen {
                medium: Medium::Matte,
                strength: Strength::Low
            }
            .describe(),
            "accentuation papier mat, faible"
        );
    }

    #[test]
    fn output_sharpening_raises_edge_contrast() {
        // Un bord net adouci : l'accentuation creuse le côté sombre et éclaircit l'autre,
        // d'autant plus que l'intensité est forte ; les aplats ne bougent pas.
        let soft = RgbImage::from_fn(64, 16, |x, _| {
            let t = ((x as f32 - 31.5) / 2.0).tanh() * 0.5 + 0.5;
            [0.05 + 0.4 * t; 3]
        });
        let contrast = |img: &RgbImage| img.data[8 * 64 + 34][1] - img.data[8 * 64 + 29][1];
        let mut last = contrast(&soft);
        for strength in [Strength::Low, Strength::Standard, Strength::High] {
            let mut img = soft.clone();
            OutputSharpen {
                medium: Medium::Screen,
                strength,
            }
            .apply(&mut img);
            let c = contrast(&img);
            assert!(c > last, "{strength:?} : {c} ≤ {last}");
            last = c;
            assert!((img.data[8 * 64 + 2][1] - soft.data[8 * 64 + 2][1]).abs() < 1e-4);
        }
    }

    #[test]
    fn output_sharpening_of_tiny_images() {
        // Une image vide ou d'un pixel : l'accentuation la laisse telle quelle, sans
        // paniquer.
        for (w, h) in [(0, 0), (1, 1), (1, 5), (4, 1)] {
            for medium in [Medium::Screen, Medium::Matte, Medium::Glossy] {
                let mut img = RgbImage::from_fn(w, h, |x, _| [0.2 + 0.1 * x as f32; 3]);
                OutputSharpen {
                    medium,
                    strength: Strength::High,
                }
                .apply(&mut img);
                assert_eq!((img.width, img.height, img.data.len()), (w, h, w * h));
                assert!(img.data.iter().flatten().all(|v| v.is_finite()));
            }
        }
    }

    #[test]
    fn resize_modes() {
        let (w, h) = (6000, 4000);
        assert_eq!(Resize::LongEdge(2048).apply(w, h), (2048, 1365));
        assert_eq!(Resize::LongEdge(2048).apply(h, w), (1365, 2048));
        assert_eq!(Resize::ShortEdge(1080).apply(w, h), (1620, 1080));
        assert_eq!(Resize::ShortEdge(1080).apply(h, w), (1080, 1620));
        // Le cadre : le côté le plus contraint décide.
        assert_eq!(
            Resize::Fit {
                width: 1920,
                height: 1080
            }
            .apply(w, h),
            (1620, 1080)
        );
        assert_eq!(
            Resize::Fit {
                width: 1000,
                height: 1000
            }
            .apply(w, h),
            (1000, 667)
        );
        let (mw, mh) = Resize::Megapixels(6.0).apply(w, h);
        assert_eq!((mw, mh), (3000, 2000));
        // Jamais d'agrandissement.
        assert_eq!(Resize::LongEdge(9000).apply(w, h), (w, h));
        assert_eq!(Resize::Megapixels(50.0).apply(w, h), (w, h));
        assert_eq!(
            Resize::Fit {
                width: 8000,
                height: 8000
            }
            .apply(w, h),
            (w, h)
        );
        // Agrandir, sur demande : jusqu'à quatre fois chaque côté et 200 Mpx.
        assert_eq!(Resize::LongEdge(9000).fit(w, h, true), (9000, 6000));
        assert_eq!(Resize::LongEdge(2048).fit(w, h, true), (2048, 1365));
        assert_eq!(Resize::LongEdge(4000).fit(800, 600, true), (3200, 2400));
        // Un tirage de 30 × 40 cm à 300 ppp : 3543 × 4724 pixels, posés dans le sens de
        // la photo (le grand côté sur le grand côté).
        let print = Resize::Print {
            width_cm: 30.0,
            height_cm: 40.0,
            dpi: 300,
        };
        assert_eq!(print.print_pixels(true), (4724, 3543));
        assert_eq!(print.fit(6000, 4000, false), (4724, 3149));
        assert_eq!(print.fit(4000, 6000, false), (3149, 4724));
        assert_eq!(print.dpi(), Some(300));
        assert_eq!(Resize::LongEdge(10).dpi(), None);
        let (bw, bh) = Resize::Megapixels(500.0).fit(w, h, true);
        assert!(bw * bh <= 200_000_000 && bw * bh > 199_000_000, "{bw} × {bh}");
    }

    #[test]
    fn png_prints_carry_their_resolution() {
        // Le CRC des blocs PNG, sur la valeur de référence du format.
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        let dir = std::env::temp_dir().join(format!("luma-png-ppp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let img = RgbImage {
            width: 12,
            height: 8,
            data: (0..96).map(|i| [i as f32 / 96.0, 0.4, 0.2]).collect(),
        };
        let path = dir.join("tirage.png");
        let meta = Embedded {
            dpi: Some(300),
            ..Embedded::default()
        };
        write_file(&path, &img, OutputSpace::Srgb, 90, 8, &meta, false).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        // Juste après l'en‑tête : pHYs, 11811 points par mètre (300 ppp), en mètres.
        assert_eq!(&bytes[37..41], b"pHYs");
        assert_eq!(&bytes[41..45], &11_811u32.to_be_bytes());
        assert_eq!(bytes[49], 1);
        // Le fichier se relit, aux mêmes pixels qu'un PNG sans résolution.
        let plain = dir.join("simple.png");
        write_file(&plain, &img, OutputSpace::Srgb, 90, 8, &Embedded::default(), false).unwrap();
        let (a, b) = (image::open(&path).unwrap(), image::open(&plain).unwrap());
        assert_eq!(a.as_bytes(), b.as_bytes());
        assert!(!std::fs::read(&plain).unwrap().windows(4).any(|w| w == b"pHYs"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Une image détaillée, lourde en JPEG : du bruit sur un dégradé.
    fn busy(w: usize, h: usize) -> RgbImage {
        let mut seed = 12345u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1u32 << 24) as f32
        };
        RgbImage {
            width: w,
            height: h,
            data: (0..w * h)
                .map(|i| {
                    let x = (i % w) as f32 / w as f32;
                    [0.3 * x + 0.5 * noise(), 0.4 + 0.3 * noise(), 0.6 - 0.4 * x * noise()]
                })
                .collect(),
        }
    }

    #[test]
    fn a_weight_limit_keeps_the_best_quality_that_fits() {
        let img = busy(160, 120);
        let rgb = luma_image::encode8(&img, OutputSpace::Srgb);
        let encode = |q: u8| crate::jpeg::encode(&rgb, 160, 120, q, &[], &[]);
        // Un échantillon : la moitié du haut de l'image.
        let sample = |q: u8| crate::jpeg::encode(&rgb[..160 * 64 * 3], 160, 64, q, &[], &[]).map(|j| j.len());
        let full = encode(92).unwrap().len();
        let max = full / 2;
        let (out, q) = under_weight(92, max, encode, sample).unwrap();
        assert!(out.len() <= max, "{} octets pour {max}", out.len());
        // La qualité juste au‑dessus ne tient plus : c'est la meilleure possible.
        assert!(q < 92 && encode(q + 1).unwrap().len() > max, "qualité {q}");
        assert_eq!(out, encode(q).unwrap());
        // Assez léger d'emblée : la qualité demandée, un seul codage.
        assert_eq!(
            under_weight(92, full, encode, sample).unwrap(),
            (encode(92).unwrap(), 92)
        );
        // Impossible à tenir : la qualité 1, le plus léger possible.
        assert_eq!(under_weight(92, 100, encode, sample).unwrap().1, 1);
        // Un échantillon trompeur (tout petit) : le même choix, en plus de codages.
        assert_eq!(under_weight(92, max, encode, |_| Ok(1)).unwrap().1, q);
        assert_eq!(
            under_weight(92, max, encode, |q| Ok(usize::from(100 - q))).unwrap().1,
            q
        );
    }

    /// La dichotomie d'avant, qualité par qualité : la référence du choix.
    fn bisection(quality: u8, max: usize, size: &dyn Fn(u8) -> usize) -> u8 {
        if size(quality) <= max {
            return quality;
        }
        let (mut lo, mut hi) = (1u8, quality);
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if size(mid) <= max {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    #[test]
    fn the_quality_is_found_in_few_encodings_and_is_the_same_as_by_halves() {
        let (w, h) = (128, 512);
        let img = busy(w, h);
        let rgb = luma_image::encode8(&img, OutputSpace::Srgb);
        let sizes: Vec<usize> = (0..=100u8)
            .map(|q| crate::jpeg::encode(&rgb, w, h, q.max(1), &[], &[]).unwrap().len())
            .collect();
        let size = |q: u8| sizes[usize::from(q)];
        let (mut ours, mut halves) = (0, 0);
        for quality in [60u8, 85, 92, 100] {
            for part in [0.25, 0.4, 0.55, 0.7, 0.85, 0.97] {
                let max = (size(quality) as f64 * part) as usize;
                let count = std::cell::Cell::new(0);
                let encode = |q: u8| {
                    count.set(count.get() + 1);
                    crate::jpeg::encode(&rgb, w, h, q, &[], &[])
                };
                let sample = |q: u8| {
                    // Une ligne de blocs sur huit, comme à l'export.
                    let rows: Vec<u8> = (0..h / 8)
                        .flat_map(|y| {
                            let y = (y / 8) * 64 + y % 8;
                            rgb[y * w * 3..(y + 1) * w * 3].iter().copied()
                        })
                        .collect();
                    crate::jpeg::encode(&rows, w, h / 8, q, &[], &[]).map(|j| j.len())
                };
                let (out, q) = under_weight(quality, max, encode, sample).unwrap();
                assert_eq!(q, bisection(quality, max, &size), "qualité {quality}, {max} octets");
                assert_eq!(out.len(), size(q));
                eprintln!(
                    "CAS {quality} {part} q={q} codages={} taille1={} max={max}",
                    count.get(),
                    size(1)
                );
                ours += count.get();
                // La dichotomie : le premier codage, sept au plus ensuite.
                halves += 1 + (quality - 1).ilog2() as usize + 1;
            }
        }
        // Les vrais codages : la moitié de ceux d'une dichotomie, au plus.
        assert!(ours * 2 <= halves, "{ours} codages contre {halves}");
    }

    #[test]
    fn the_weight_limit_counts_the_metadata_and_reaches_the_file() {
        let dir = std::env::temp_dir().join(format!("luma-poids-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let img = busy(200, 150);
        let xmp = format!("<x:xmpmeta>{}</x:xmpmeta>", "mot-clé ".repeat(400));
        let free = dir.join("libre.jpg");
        let heavy = Embedded {
            xmp: Some(xmp),
            ..Embedded::default()
        };
        write_file(&free, &img, OutputSpace::Srgb, 95, 8, &heavy, false).unwrap();
        let size = std::fs::metadata(&free).unwrap().len() as usize;
        let max = size * 2 / 3;
        let limited = Embedded {
            max_bytes: Some(max),
            ..heavy.clone()
        };
        let path = dir.join("limite.jpg");
        write_file(&path, &img, OutputSpace::Srgb, 95, 8, &limited, false).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() <= max, "{} octets pour {max}", bytes.len());
        // Le XMP est toujours là et l'image se relit, à la même taille.
        let needle = "mot-clé ".as_bytes();
        assert!(bytes.windows(needle.len()).any(|w| w == needle));
        let back = image::load_from_memory(&bytes).unwrap();
        assert_eq!((back.width(), back.height()), (200, 150));
        // Une limite déjà tenue : octet pour octet le fichier sans limite.
        let loose = dir.join("large.jpg");
        let roomy = Embedded {
            max_bytes: Some(size),
            ..heavy.clone()
        };
        write_file(&loose, &img, OutputSpace::Srgb, 95, 8, &roomy, false).unwrap();
        assert_eq!(std::fs::read(&loose).unwrap(), std::fs::read(&free).unwrap());
        // Le PNG ne connaît pas de limite : le même fichier qu'avant.
        let (a, b) = (dir.join("a.png"), dir.join("b.png"));
        write_file(&a, &img, OutputSpace::Srgb, 95, 8, &limited, false).unwrap();
        write_file(&b, &img, OutputSpace::Srgb, 95, 8, &heavy, false).unwrap();
        assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
        // Dans la description d'un préréglage.
        let o = Options {
            max_kb: Some(500),
            ..Options::default()
        };
        assert_eq!(describe(&o), "JPEG 92, 500 Ko au plus, sRGB, pleine définition, LÜMA/");
    }

    #[test]
    fn print_sizes_are_parsed_written_and_described() {
        let p = Resize::parse("30x40cm@240").unwrap();
        assert_eq!(
            p,
            Resize::Print {
                width_cm: 30.0,
                height_cm: 40.0,
                dpi: 240
            }
        );
        assert_eq!(Resize::parse(&p.id()), Some(p));
        assert_eq!(p.describe(), "30 × 40 cm à 240 ppp");
        // Virgule décimale, espaces, 300 ppp par défaut.
        let a4 = Resize::parse("21 × 29,7 cm").unwrap();
        assert_eq!(
            a4,
            Resize::Print {
                width_cm: 21.0,
                height_cm: 29.7,
                dpi: 300
            }
        );
        assert_eq!(Resize::parse(&a4.id()), Some(a4));
        for bad in ["0x40cm", "30x40cm@0", "30cm", "30x40cm@abc"] {
            assert_eq!(Resize::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn resize_parsed_and_written() {
        for r in [
            Resize::LongEdge(2048),
            Resize::ShortEdge(1080),
            Resize::Fit {
                width: 1920,
                height: 1080,
            },
            Resize::Megapixels(12.5),
        ] {
            assert_eq!(Resize::parse(&r.id()), Some(r));
        }
        assert_eq!(Resize::parse("2048"), Some(Resize::LongEdge(2048)));
        assert_eq!(Resize::parse("grand:300"), Some(Resize::LongEdge(300)));
        assert_eq!(Resize::parse("petit:300"), Some(Resize::ShortEdge(300)));
        assert_eq!(
            Resize::parse("1920×1080"),
            Some(Resize::Fit {
                width: 1920,
                height: 1080
            })
        );
        assert_eq!(Resize::parse("12 MP"), Some(Resize::Megapixels(12.0)));
        assert_eq!(Resize::parse("2,5mpx"), Some(Resize::Megapixels(2.5)));
        for bad in ["", "0", "abc", "0x100", "long:", "-3mp", "short:0"] {
            assert_eq!(Resize::parse(bad), None, "{bad}");
        }
        assert_eq!(Resize::Megapixels(12.0).describe(), "12 Mpx");
        assert_eq!(Resize::ShortEdge(1080).describe(), "petit côté 1080 px");
    }

    #[test]
    fn old_preset_files_keep_their_long_edge() {
        let dir = std::env::temp_dir().join(format!("luma-exports-old-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Exports.json");
        std::fs::write(
            &path,
            r#"{"version":1,"presets":[{"name":"A","format":"jpg","quality":90,"bits":8,"space":"srgb","long_edge":1600}]}"#,
        )
        .unwrap();
        let p = Presets::load(&path);
        assert_eq!(p.user[0].options.resize, Some(Resize::LongEdge(1600)));
        // Réécrit au nouveau format, relu pareil.
        p.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\"resize\": \"long:1600\"") && !text.contains("long_edge"),
            "{text}"
        );
        assert_eq!(Presets::load(&path), p);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn names_taken_get_a_number() {
        let mut taken = std::collections::HashSet::new();
        let p = PathBuf::from("/s/2026-03-09.jpg");
        assert_eq!(unique(p.clone(), &mut taken), p);
        assert_eq!(unique(p.clone(), &mut taken), PathBuf::from("/s/2026-03-09-2.jpg"));
        assert_eq!(unique(p, &mut taken), PathBuf::from("/s/2026-03-09-3.jpg"));
        // Un original dans le dossier de sortie n'est jamais écrasé.
        let dir = std::env::temp_dir().join(format!("luma-unique-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("IMG_1.jpg");
        std::fs::write(&src, b"original").unwrap();
        let mut taken = std::collections::HashSet::new();
        reserve_sources(&mut taken, [src.as_path()]);
        let o = Options {
            folder: Some(dir.join(".")),
            ..Options::default()
        };
        let out = unique(output_path(&src, &o, &NameFields::of_file(&src)), &mut taken);
        assert_eq!(out.file_name().unwrap(), "IMG_1-2.jpg");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn fields() -> NameFields {
        NameFields {
            name: "DSC_0750".into(),
            date: Some([2026, 3, 9, 8, 5, 1]),
            make: "NIKON CORPORATION".into(),
            model: "NIKON Z 6".into(),
            lens: "NIKKOR Z 24-70mm f/4 S".into(),
            iso: Some(400),
            rating: 4,
            label: "Rouge".into(),
            sequence: 12,
        }
    }

    #[test]
    fn templates_give_file_names() {
        let f = fields();
        let name = |t: &str| file_name(t, &f);
        assert_eq!(name("{date}_{séquence}"), "2026-03-09_0012");
        assert_eq!(name("{nom}"), "DSC_0750");
        assert_eq!(
            name("{année}{mois}{jour}-{heure} {boîtier}"),
            "20260309-080501 NIKON Z 6"
        );
        assert_eq!(
            name("{marque} ISO {iso} {note}★ {libellé}"),
            "NIKON CORPORATION ISO 400 4★ Rouge"
        );
        // Le « / » d'une valeur ne crée pas de dossier ; celui du modèle, si.
        assert_eq!(name("{objectif}"), "NIKKOR Z 24-70mm f-4 S");
        assert_eq!(name("{année}/{mois}/{nom}"), "2026/03/DSC_0750");
        assert_eq!(name("../{nom}"), "DSC_0750");
        // Les repères anglais et espagnols, et sans accents.
        assert_eq!(name("{name}-{sequence:3}-{camera}"), "DSC_0750-012-NIKON Z 6");
        assert_eq!(name("{nombre}-{secuencia:1}-{fecha}"), "DSC_0750-12-2026-03-09");
        assert_eq!(name("{boitier}"), name("{Boîtier}"));
        // Un repère inconnu reste, un repère sans valeur disparaît.
        assert_eq!(name("{inconnu}_{nom}"), "{inconnu}_DSC_0750");
        let bare = NameFields {
            name: "x".into(),
            sequence: 1,
            ..NameFields::default()
        };
        assert_eq!(file_name("{nom}_{date}_{objectif}", &bare), "x__");
        assert_eq!(file_name("a:b*c?", &bare), "a-b-c-");
    }

    #[test]
    fn templates_are_checked() {
        assert!(check_template("{date}_{séquence:3}/{nom}").is_ok());
        assert!(check_template("sans repère").is_ok());
        assert!(check_template("{nom").unwrap_err().contains("non fermée"));
        assert!(check_template("nom}").unwrap_err().contains("non ouverte"));
        let e = check_template("{nom}_{taille}").unwrap_err();
        assert!(e.contains("{taille}") && e.contains("{séquence}"), "{e}");
        assert!(check_template("{séquence:0}").is_err());
        assert!(check_template("{nom:3}").is_ok());
    }

    #[test]
    fn camera_names_and_dates() {
        assert_eq!(camera("SONY", "ILCE-7M3"), "SONY ILCE-7M3");
        assert_eq!(camera("Canon", "Canon EOS R5"), "Canon EOS R5");
        assert_eq!(camera("FUJIFILM", "X-T3"), "FUJIFILM X-T3");
        assert_eq!(camera("", "X"), "X");
        assert_eq!(camera("Apple", ""), "Apple");
        assert_eq!(exif_date("2026:10:26 14:03:00"), Some([2026, 10, 26, 14, 3, 0]));
        assert_eq!(exif_date("2026-10-26"), Some([2026, 10, 26, 0, 0, 0]));
        assert_eq!(exif_date("0000:00:00 00:00:00"), None);
        assert_eq!(exif_date(""), None);
        assert_eq!(civil(0), [1970, 1, 1, 0, 0, 0]);
        assert_eq!(civil(1_772_910_000), [2026, 3, 7, 19, 0, 0]);
    }

    #[test]
    fn spaces_and_formats_round_trip() {
        for s in [
            OutputSpace::Srgb,
            OutputSpace::DisplayP3,
            OutputSpace::Rec2020,
            OutputSpace::ProPhoto,
        ] {
            assert_eq!(OutputSpace::parse(space_id(s)), Some(s));
        }
        for f in [Format::Jpeg, Format::Png, Format::Tiff] {
            assert_eq!(Format::parse(f.extension()), Some(f));
        }
        assert_eq!(Format::parse("JPEG"), Some(Format::Jpeg));
        assert_eq!(Format::parse("gif"), None);
    }

    #[test]
    fn a_damaged_preset_does_not_lose_the_others() {
        let dir = std::env::temp_dir().join(format!("luma-prereglages-abimes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Exports.json");
        // Deux préréglages ; le second, retouché à la main, a une qualité impossible.
        std::fs::write(
            &path,
            r#"{"version":1,"presets":[
                {"name":"Bon","format":"jpg","quality":90,"bits":8,"space":"srgb"},
                {"name":"Abîmé","format":"jpg","quality":300,"bits":8,"space":"srgb"}
            ]}"#,
        )
        .unwrap();
        let p = Presets::load(&path);
        assert_eq!(p.user.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Bon"]);
        // Un fichier coupé net : rien ne se lit, mais il est mis de côté avant d'être
        // réécrit, et l'utilisateur peut le reprendre.
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, &text[..text.len() / 2]).unwrap();
        assert!(Presets::load(&path).user.is_empty());
        let aside = dir.join("Exports.json.illisible");
        assert_eq!(std::fs::read_to_string(&aside).unwrap(), text[..text.len() / 2]);
        // Un fichier absent n'est pas « illisible ».
        let _ = std::fs::remove_file(&aside);
        assert!(Presets::load(&dir.join("absent.json")).user.is_empty());
        assert!(!dir.join("absent.json.illisible").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_export_is_written_in_one_go() {
        let dir = std::env::temp_dir().join(format!("luma-export-d-un-coup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let img = busy(64, 48);
        for ext in ["jpg", "png", "tif"] {
            let path = dir.join(format!("photo.{ext}"));
            write_file(&path, &img, OutputSpace::Srgb, 90, 8, &Embedded::default(), false).unwrap();
            let before = std::fs::read(&path).unwrap();
            // Rien d'autre que le fichier voulu à côté.
            let names: Vec<_> = std::fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .map(|e| e.file_name())
                .collect();
            assert!(
                names.iter().all(|n| !n.to_string_lossy().contains("partiel")),
                "{names:?}"
            );
            // Un nouvel export qui échoue (ici, le temporaire ne peut pas être créé)
            // laisse l'export d'avant entier, et le message nomme le fichier voulu.
            let blocked = dir.join(format!(".photo.{ext}.partiel.{ext}"));
            std::fs::create_dir_all(blocked.join("occupé")).unwrap();
            let e = write_file(
                &path,
                &busy(32, 32),
                OutputSpace::Srgb,
                90,
                8,
                &Embedded::default(),
                false,
            )
            .unwrap_err();
            assert!(e.contains(&path.display().to_string()) && !e.contains("partiel"), "{e}");
            assert_eq!(std::fs::read(&path).unwrap(), before);
            std::fs::remove_dir_all(&blocked).unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_preset_save_keeps_the_file_and_leaves_nothing_behind() {
        let dir = std::env::temp_dir().join(format!("luma-prereglages-temporaire-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Exports.json");
        let mut p = Presets::default();
        p.set(Preset {
            name: "Mine".into(),
            options: Options::default(),
        });
        p.save(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        // Le renommage échoue (un dossier a pris la place du fichier) : rien n'est
        // laissé derrière, et un fichier qui n'a pas été remplacé reste entier.
        let blocked = dir.join("Bloqué.json");
        std::fs::create_dir_all(blocked.join("occupé")).unwrap();
        assert!(p.save(&blocked).is_err());
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert!(
            names.iter().all(|n| !n.to_string_lossy().ends_with(".tmp")),
            "{names:?}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn presets_saved_and_reloaded() {
        let dir = std::env::temp_dir().join(format!("luma-exports-{}", std::process::id()));
        let path = dir.join("sous/Exports.json");
        let mut p = Presets::load(&path);
        assert!(p.user.is_empty());
        assert_eq!(p.all().len(), builtin().len());
        let mine = Preset {
            name: "Instagram".into(),
            options: Options {
                quality: 80,
                resize: Some(Resize::ShortEdge(1080)),
                space: OutputSpace::DisplayP3,
                folder: Some("/tmp/insta".into()),
                name: Some("{date}_{séquence}".into()),
                sharpen: Some(OutputSharpen {
                    medium: Medium::Matte,
                    strength: Strength::High,
                }),
                watermark: Some(Watermark {
                    position: Position::TopLeft,
                    dark: true,
                    ..Watermark::new("© Maxime Lestage")
                }),
                logo: Some(Logo {
                    position: Position::Center,
                    size: 25.0,
                    ..Logo::new("/logos/signature.png")
                }),
                metadata: MetadataOptions {
                    keep_gps: true,
                    author: Some("Maxime Lestage".into()),
                    ..MetadataOptions::default()
                },
                web: true,
                enlarge: true,
                max_kb: Some(500),
                ..Options::default()
            },
        };
        p.set(mine.clone());
        // Un préréglage de l'utilisateur du même nom remplace celui de base.
        let web = Preset {
            name: "web".into(),
            options: Options {
                format: Format::Png,
                bits: 16,
                ..Options::default()
            },
        };
        p.set(web.clone());
        p.save(&path).unwrap();
        let back = Presets::load(&path);
        assert_eq!(back, p);
        assert_eq!(back.get("INSTAGRAM"), Some(mine.clone()));
        assert_eq!(back.get("Web"), Some(web));
        assert_eq!(back.all().len(), builtin().len() + 1);
        assert_eq!(back.get("Impression").unwrap().options.space, OutputSpace::ProPhoto);
        assert!(builtin()[0].options.web, "le préréglage Web allège ses JPEG");
        assert!(describe(&builtin()[0].options).ends_with(", allégé pour le web"));
        // Renommer en gardant le nom remplace, sans doublon.
        let mut again = back.clone();
        again.set(Preset {
            name: " instagram ".into(),
            ..mine
        });
        assert_eq!(again.user.len(), 2);
        assert!(again.remove("Instagram"));
        assert!(!again.remove("Instagram"));
        assert!(again.get("instagram").is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unreadable_or_unknown_values_are_skipped() {
        let dir = std::env::temp_dir().join(format!("luma-exports-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Exports.json");
        std::fs::write(&path, "pas du json").unwrap();
        assert!(Presets::load(&path).user.is_empty());
        std::fs::write(
            &path,
            r#"{"version":1,"presets":[
                {"name":"A","format":"gif","quality":90,"bits":8,"space":"srgb"},
                {"name":"B","format":"png","quality":250,"bits":12,"space":"p3","long_edge":0}
            ]}"#,
        )
        .unwrap();
        let p = Presets::load(&path);
        assert_eq!(p.user.len(), 1);
        let b = &p.user[0].options;
        assert_eq!((b.quality, b.bits, b.resize), (100, 8, None));
        std::fs::write(&path, r#"{"version":99,"presets":[]}"#).unwrap();
        assert!(Presets::load(&path).user.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn descriptions() {
        assert_eq!(
            describe(&builtin()[0].options),
            "JPEG 85, sRGB, 2048 px, LÜMA/, allégé pour le web"
        );
        assert_eq!(
            describe(&builtin()[3].options),
            "TIFF 16 bits, ProPhoto RGB, pleine définition, LÜMA/"
        );
        let big = Options {
            resize: Some(Resize::LongEdge(12000)),
            enlarge: true,
            ..Options::default()
        };
        assert_eq!(describe(&big), "JPEG 92, sRGB, 12000 px, LÜMA/, agrandie au besoin");
    }

    #[test]
    fn jpeg_converted_strip_by_strip_is_byte_identical() {
        let dir = std::env::temp_dir().join(format!("luma-bandes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Des tailles qui tombent juste ou non sur les blocs de 8, des couleurs hors du
        // gamut de sortie.
        for (w, h) in [(48, 32), (37, 23), (8, 9), (1, 1), (130, 7)] {
            let img = RgbImage {
                width: w,
                height: h,
                data: (0..w * h)
                    .map(|i| [(i % w) as f32 / w as f32 * 1.3, 0.3, (i / w) as f32 / h as f32 - 0.1])
                    .collect(),
            };
            for space in [OutputSpace::Srgb, OutputSpace::DisplayP3] {
                let path = dir.join(format!("{w}x{h}.jpg"));
                write_file(&path, &img, space, 90, 8, &Embedded::default(), false).unwrap();
                let profile = icc::profile(space);
                let before = crate::jpeg::encode(&luma_image::encode8(&img, space), w, h, 90, &[], &profile).unwrap();
                assert_eq!(std::fs::read(&path).unwrap(), before, "{w} × {h}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn web_jpeg_is_written_in_4_2_0() {
        let dir = std::env::temp_dir().join(format!("luma-web-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let img = RgbImage {
            width: 48,
            height: 32,
            data: (0..48 * 32)
                .map(|i| [(i % 48) as f32 / 48.0, 0.3, (i / 48) as f32 / 32.0])
                .collect(),
        };
        let sampling = |web: bool| {
            let path = dir.join(format!("{web}.jpg"));
            write_file(&path, &img, OutputSpace::Srgb, 85, 8, &Embedded::default(), web).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            let sof = bytes.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
            // Le facteur d'échantillonnage de la luminance.
            bytes[sof + 11]
        };
        assert_eq!((sampling(false), sampling(true)), (0x11, 0x22));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
