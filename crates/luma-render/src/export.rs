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

use crate::{Photo, Size};

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
    /// Le grand côté de l'image recadrée, en pixels ; `None` : pleine définition.
    pub long_edge: Option<usize>,
    /// `None` : un dossier `LÜMA/` à côté de chaque RAW.
    pub folder: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: Format::Jpeg,
            quality: 92,
            bits: 8,
            space: OutputSpace::Srgb,
            long_edge: None,
            folder: None,
        }
    }
}

/// Le fichier de sortie d'un RAW.
pub fn output_path(source: &Path, options: &Options) -> PathBuf {
    let dir = match &options.folder {
        Some(d) => d.clone(),
        None => source.parent().unwrap_or(Path::new(".")).join("LÜMA"),
    };
    dir.join(
        source
            .with_extension(options.format.extension())
            .file_name()
            .unwrap_or_default(),
    )
}

/// Développe et écrit une photo selon les options (le dossier est créé au besoin).
pub fn develop(photo: &Photo, settings: &Settings, out: &Path, options: &Options) -> Result<(), String> {
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    }
    let size = match options.long_edge {
        None => Size::Full,
        Some(n) => {
            // La taille demandée est celle de l'image recadrée : on rend un peu plus
            // grand, puis on réduit exactement.
            let c = &settings.crop;
            let frac = (c.right - c.left).min(c.bottom - c.top).max(0.05);
            let (w, h) = photo.full_size();
            let pre = (n as f32 / frac).ceil() as usize;
            if pre >= w.max(h) { Size::Full } else { Size::Fit(pre) }
        }
    };
    let rendered = photo
        .render(settings, size, &Cancel::new())
        .map_err(|e| e.to_string())?;
    let mut image = rendered.image;
    if let Some(n) = options.long_edge {
        let (w, h) = RgbImage::fit(image.width, image.height, n);
        if (w, h) != (image.width, image.height) {
            image = image.resize_area(w, h);
        }
    }
    let bits = if options.format == Format::Jpeg {
        8
    } else {
        options.bits
    };
    write(out, &image, options.space, options.quality, bits)
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
                long_edge: Some(2048),
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
                long_edge: Some(3840),
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
    #[serde(default)]
    long_edge: Option<usize>,
    #[serde(default)]
    folder: Option<PathBuf>,
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
            long_edge: o.long_edge,
            folder: o.folder.clone(),
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
                long_edge: self.long_edge.filter(|&n| n > 0),
                folder: self.folder,
            },
        })
    }
}

/// Le fichier des préréglages d'export, dans le dossier de LÜMA : sur Mac
/// `~/Library/Application Support/LÜMA/Exports.json`, sur iPhone et iPad
/// `Library/Application Support/Exports.json`, ailleurs `~/.local/share/luma/exports.json`.
/// `LUMA_EXPORT_PRESETS` le remplace.
pub fn presets_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LUMA_EXPORT_PRESETS") {
        return Some(PathBuf::from(p));
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

/// Une ligne de description : « JPEG 85, sRGB, 2048 px, LÜMA/ ».
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
        .long_edge
        .map_or_else(|| "pleine définition".to_owned(), |n| format!("{n} px"));
    let folder = o
        .folder
        .as_ref()
        .map_or_else(|| "LÜMA/".to_owned(), |d| d.display().to_string());
    format!("{format}, {space}, {size}, {folder}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_paths() {
        let mut o = Options::default();
        assert_eq!(
            output_path(Path::new("/x/IMG_1.CR3"), &o),
            PathBuf::from("/x/LÜMA/IMG_1.jpg")
        );
        o.format = Format::Tiff;
        o.folder = Some("/sortie".into());
        assert_eq!(
            output_path(Path::new("/x/IMG_1.CR3"), &o),
            PathBuf::from("/sortie/IMG_1.tif")
        );
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
                long_edge: Some(1080),
                space: OutputSpace::DisplayP3,
                folder: Some("/tmp/insta".into()),
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
        assert_eq!((b.quality, b.bits, b.long_edge), (100, 8, None));
        std::fs::write(&path, r#"{"version":99,"presets":[]}"#).unwrap();
        assert!(Presets::load(&path).user.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn descriptions() {
        assert_eq!(describe(&builtin()[0].options), "JPEG 85, sRGB, 2048 px, LÜMA/");
        assert_eq!(
            describe(&builtin()[3].options),
            "TIFF 16 bits, ProPhoto RGB, pleine définition, LÜMA/"
        );
    }
}
