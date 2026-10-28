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
pub fn write_file(
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
    // L'image en octets, sauf pour le JPEG ordinaire, converti bande par bande.
    let (bytes, kind) = if sixteen {
        (luma_image::encode16_bytes(img, space), ExtendedColorType::Rgb16)
    } else if jpeg && !web {
        (Vec::new(), ExtendedColorType::Rgb8)
    } else {
        (luma_image::encode8(img, space), ExtendedColorType::Rgb8)
    };
    let create = || std::fs::File::create(path).map_err(|e| format!("{} : {e}", path.display()));
    match ext.as_str() {
        "jpg" | "jpeg" => {
            // Sur tous les cœurs, octet pour octet le JPEG d'avant ; en mémoire d'abord :
            // le paquet XMP s'insère après les premiers segments.
            let mut out = if web {
                crate::jpeg::encode_web(&bytes, img.width, img.height, quality, &meta.exif, &profile)?
            } else {
                let enc = luma_image::Encoder8::new(space);
                let w = img.width;
                let lines = |y: usize, x0: usize, dst: &mut [u8]| {
                    let start = y * w + x0;
                    enc.pixels(&img.data[start..start + dst.len() / 3], dst);
                };
                crate::jpeg::encode_with(&lines, img.width, img.height, quality, &meta.exif, &profile)?
            };
            if let Some(x) = &meta.xmp {
                insert_jpeg_xmp(&mut out, x);
            }
            std::fs::write(path, out).map_err(|e| format!("{} : {e}", path.display()))
        }
        "png" => {
            let mut enc = image::codecs::png::PngEncoder::new(std::io::BufWriter::new(create()?));
            let _ = enc.set_icc_profile(profile);
            if !meta.exif.is_empty() {
                let _ = enc.set_exif_metadata(meta.exif.clone());
            }
            enc.write_image(&bytes, w, h, kind).map_err(err)
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

    /// Lit une taille : `2048` ou `long:2048` (grand côté), `short:1080` (petit côté),
    /// `1920x1080` (cadre), `12mp` (mégapixels). `None` si la valeur est vide ou nulle.
    pub fn parse(s: &str) -> Option<Resize> {
        let s = s.trim().to_lowercase();
        let num = |v: &str| v.trim().parse::<usize>().ok().filter(|&n| n > 0);
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
        }
    }

    /// En clair : « 2048 px », « petit côté 1080 px », « 1920 × 1080 px », « 12 Mpx ».
    pub fn describe(self) -> String {
        match self {
            Resize::LongEdge(n) => format!("{n} px"),
            Resize::ShortEdge(n) => format!("petit côté {n} px"),
            Resize::Fit { width, height } => format!("{width} × {height} px"),
            Resize::Megapixels(m) => format!("{m} Mpx"),
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
/// le modèle donne le même nom à plusieurs photos.
pub fn unique(path: PathBuf, taken: &mut std::collections::HashSet<PathBuf>) -> PathBuf {
    let mut out = path.clone();
    let mut n = 2;
    while taken.contains(&out) {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let ext = path.extension().unwrap_or_default().to_string_lossy();
        out = path.with_file_name(format!("{stem}-{n}.{ext}"));
        n += 1;
    }
    taken.insert(out.clone());
    out
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
    let embedded = Embedded::new(
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
        let stored: Option<Stored> = std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let user = stored
            .filter(|s| s.version == VERSION)
            .map(|s| s.presets.into_iter().filter_map(StoredPreset::preset).collect())
            .unwrap_or_default();
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
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| format!("{} : {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{} : {e}", path.display()))
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
        Format::Jpeg => format!("JPEG {}", o.quality),
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
        let (bw, bh) = Resize::Megapixels(500.0).fit(w, h, true);
        assert!(bw * bh <= 200_000_000 && bw * bh > 199_000_000, "{bw} × {bh}");
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
