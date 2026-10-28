//! Lecture et écriture des réglages au format XMP (ISO 16684), espace de noms `crs:`.
//!
//! Les retouches sont enregistrées dans un fichier `.xmp` posé à côté du RAW, ou dans
//! un préréglage : LÜMA les relit curseur par curseur, et les écrit dans le même
//! vocabulaire.
//!
//! Les noms des réglages sont les étiquettes du format (`Exposure2012`,
//! `Highlights2012`, `ToneCurvePV2012`…) ; chacune porte une valeur dans la plage et le
//! sens du curseur correspondant de LÜMA.

use std::collections::HashMap;

use quick_xml::XmlVersion;
use quick_xml::events::Event;

use crate::{HSL_COLORS, Profile, Settings, WhiteBalance};

const CRS: &str = "http://ns.adobe.com/camera-raw-settings/1.0/";

/// Les noms XMP des huit couleurs du mélangeur.
const XMP_COLORS: [&str; 8] = ["Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple", "Magenta"];

/// Les valeurs `crs:` d'un document : simples ou listes (courbes).
#[derive(Debug, Default)]
struct Values {
    scalars: HashMap<String, String>,
    lists: HashMap<String, Vec<String>>,
    /// Hors réglages : `xmp:Rating`, `xmp:Label`, `luma:Pick`.
    meta: HashMap<String, String>,
}

const META_KEYS: [&str; 17] = [
    "luma:Snapshots",
    "exif:GPSLatitude",
    "exif:GPSLongitude",
    "xmp:Rating",
    "xmp:Label",
    "luma:Pick",
    "luma:Masks",
    "luma:Spots",
    "luma:Liquify",
    "luma:ChannelMixer",
    "luma:Lut",
    "luma:LutAmount",
    "luma:Film",
    "luma:FilmAmount",
    "luma:NoiseWavelets",
    "luma:Deconvolution",
    "luma:HighlightReconstruction",
];
const LUMA_NS: &str = "https://luma.photo/ns/1.0/";

/// Le drapeau de la Bibliothèque (touches P, X, U).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Flag {
    #[default]
    None,
    Pick,
    Reject,
}

/// Ce que la Bibliothèque sait d'une photo : note de 0 à 5 étoiles, drapeau, libellé
/// de couleur, mots‑clés. La note et le libellé sont les propriétés de base du XMP (`xmp:Rating`,
/// `xmp:Label`) ; un rejet s'écrit `xmp:Rating="-1"`, la valeur que le format lui
/// réserve, et le drapeau « retenue », qui n'a pas de propriété de base, dans l'espace
/// `luma:`. Les mots‑clés sont la liste `dc:subject`, que lisent tous les logiciels.
///
/// Les informations de la photo sont celles de l'IPTC : titre (`dc:title`), légende
/// (`dc:description`), auteur (`dc:creator`), droits (`dc:rights`), et le lieu de la
/// prise de vue dans la structure `Iptc4xmpExt:LocationCreated`. Un mot‑clé
/// hiérarchique s'écrit « Lieux|France|Paris » : rangé tel quel dans l'espace
/// `luma:`, et chacun de ses niveaux dans `dc:subject`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meta {
    pub rating: u8,
    pub flag: Flag,
    pub label: String,
    pub keywords: Vec<String>,
    pub title: String,
    pub caption: String,
    pub creator: String,
    pub rights: String,
    pub location: Location,
    /// La position de la prise de vue, si elle a été placée après coup (une trace
    /// GPX, la carte) : `exif:GPSLatitude` et `exif:GPSLongitude`, comme l'écrit tout
    /// logiciel. Elle passe devant celle de l'EXIF du fichier.
    pub position: Option<Position>,
    /// Les instantanés : des états des réglages, nommés, qu'on retrouve d'un clic.
    pub snapshots: Vec<Snapshot>,
}

/// Une position, en degrés (nord et est positifs).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    pub latitude: f64,
    pub longitude: f64,
}

// Jamais de NaN : une position vient d'une coordonnée lue ou calculée.
impl Eq for Position {}

impl Position {
    /// Une coordonnée XMP : degrés, minutes décimales et hémisphère (`48,51.504N`).
    fn write(v: f64, positive: char, negative: char) -> String {
        let a = v.abs();
        let deg = a.floor();
        format!(
            "{},{:.6}{}",
            deg as u32,
            (a - deg) * 60.0,
            if v < 0.0 { negative } else { positive }
        )
    }

    /// Lit une coordonnée XMP : `48,51.504N`, `48,51,30.2N` ou des degrés décimaux.
    fn read(text: &str) -> Option<f64> {
        let t = text.trim();
        let (body, sign) = match t.chars().last()? {
            'N' | 'E' | 'n' | 'e' => (&t[..t.len() - 1], 1.0),
            'S' | 'W' | 's' | 'w' => (&t[..t.len() - 1], -1.0),
            _ => (t, 1.0),
        };
        let parts: Vec<f64> = body
            .split(',')
            .map(|p| p.trim().parse::<f64>())
            .collect::<std::result::Result<_, _>>()
            .ok()?;
        let v = match parts.as_slice() {
            [d] => *d,
            [d, m] => d + m / 60.0,
            [d, m, s] => d + m / 60.0 + s / 3600.0,
            _ => return None,
        };
        v.is_finite().then_some(v * sign)
    }
}

/// Un instantané : un nom et les réglages d'un moment (en JSON, le format des
/// réglages de LÜMA), rangés dans le XMP (`luma:Snapshots`).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub name: String,
    pub settings: String,
}

impl Snapshot {
    pub fn new(name: &str, settings: &Settings) -> Snapshot {
        Snapshot {
            name: name.trim().to_owned(),
            settings: serde_json::to_string(settings).unwrap_or_default(),
        }
    }

    /// Les réglages de l'instantané (ceux qui manquent prennent leur valeur par défaut).
    pub fn settings(&self) -> Option<Settings> {
        serde_json::from_str(&self.settings).ok()
    }
}

/// Le lieu de la prise de vue.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Location {
    /// Le lieu dans la ville (un quartier, un monument).
    pub sublocation: String,
    pub city: String,
    /// La région, la province, l'État.
    pub state: String,
    pub country: String,
}

impl Location {
    pub fn is_empty(&self) -> bool {
        self.sublocation.is_empty() && self.city.is_empty() && self.state.is_empty() && self.country.is_empty()
    }
}

/// Le séparateur des niveaux d'un mot‑clé hiérarchique.
pub const LEVEL: char = '|';

/// Les mots de `dc:subject` : chaque niveau de chaque mot‑clé, sans doublon, dans
/// l'ordre.
pub fn flat_keywords(keywords: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for k in keywords {
        for w in k.split(LEVEL).map(str::trim).filter(|w| !w.is_empty()) {
            if !out.iter().any(|o| o == w) {
                out.push(w.to_owned());
            }
        }
    }
    out
}

/// Un modèle de métadonnées : l'auteur, les droits, le lieu et des mots‑clés, à donner
/// d'un coup à des photos (ce qui est vide dans le modèle ne change rien).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Template {
    pub name: String,
    pub creator: String,
    pub rights: String,
    pub location: Location,
    pub keywords: Vec<String>,
}

impl Template {
    /// Le modèle des informations d'une photo.
    pub fn of(name: &str, m: &Meta) -> Template {
        Template {
            name: name.to_owned(),
            creator: m.creator.clone(),
            rights: m.rights.clone(),
            location: m.location.clone(),
            keywords: m.keywords.clone(),
        }
    }

    pub fn apply(&self, m: &mut Meta) {
        let set = |field: &mut String, v: &str| {
            if !v.trim().is_empty() {
                *field = v.trim().to_owned();
            }
        };
        set(&mut m.creator, &self.creator);
        set(&mut m.rights, &self.rights);
        set(&mut m.location.sublocation, &self.location.sublocation);
        set(&mut m.location.city, &self.location.city);
        set(&mut m.location.state, &self.location.state);
        set(&mut m.location.country, &self.location.country);
        for k in &self.keywords {
            if !m.keywords.iter().any(|w| w == k) {
                m.keywords.push(k.clone());
            }
        }
    }
}

/// Les modèles de métadonnées d'un fichier (vide s'il n'existe pas ou ne se lit pas).
pub fn load_templates(path: &std::path::Path) -> Vec<Template> {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Écrit les modèles de métadonnées (à côté puis renommé).
pub fn save_templates(path: &std::path::Path, templates: &[Template]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(templates).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(&tmp, path)
}

/// Les listes d'informations de l'IPTC (en `rdf:Alt` ou `rdf:Seq`), et les mots‑clés
/// hiérarchiques.
const TEXT_LISTS: [&str; 5] = ["dc:title", "dc:description", "dc:creator", "dc:rights", HIERARCHY];
const HIERARCHY: &str = "luma:HierarchicalSubject";
/// Les champs du lieu, dans la structure `Iptc4xmpExt:LocationCreated`.
const PLACE: [&str; 4] = [
    "Iptc4xmpExt:Sublocation",
    "Iptc4xmpExt:City",
    "Iptc4xmpExt:ProvinceState",
    "Iptc4xmpExt:CountryName",
];
const IPTC_EXT: &str = "http://iptc.org/std/Iptc4xmpExt/2008-02-29/";

/// Les cinq libellés de couleur, tels qu'ils s'écrivent dans `xmp:Label` : les noms
/// anglais, que lisent les autres logiciels photo.
pub const LABELS: [&str; 5] = ["Red", "Yellow", "Green", "Blue", "Purple"];

/// Le libellé de couleur (0 à 4, dans l'ordre de [`LABELS`]) que nomme `name`, en
/// anglais, en français ou en espagnol, sans tenir compte de la casse ni des accents.
pub fn label_color(name: &str) -> Option<usize> {
    let folded: String = name
        .trim()
        .chars()
        .map(|c| match c {
            'ú' | 'Ú' => 'u',
            c => c.to_ascii_lowercase(),
        })
        .collect();
    Some(match folded.as_str() {
        "red" | "rouge" | "rojo" => 0,
        "yellow" | "jaune" | "amarillo" => 1,
        "green" | "vert" | "verde" => 2,
        "blue" | "bleu" | "azul" => 3,
        "purple" | "violet" | "morado" | "purpura" => 4,
        _ => return None,
    })
}

/// Le fichier XMP d'une photo : `IMG_0001.CR3.xmp` s'il existe, sinon `IMG_0001.xmp` ;
/// celui d'une copie virtuelle, `IMG_0001.CR3.copie-2.xmp`.
pub fn sidecar_path(photo: &std::path::Path) -> std::path::PathBuf {
    if crate::copies::number(photo).is_some() {
        return crate::copies::sidecar(photo);
    }
    let long = std::path::PathBuf::from(format!("{}.xmp", photo.display()));
    if long.is_file() {
        long
    } else {
        photo.with_extension("xmp")
    }
}

/// La note, le drapeau et le libellé d'un document XMP (vides s'il n'y en a pas).
pub fn read_meta(text: &str) -> Meta {
    let Ok(values) = parse_values(text) else {
        return Meta::default();
    };
    let rating: i32 = values
        .meta
        .get("xmp:Rating")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    let pick = values
        .meta
        .get("luma:Pick")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"));
    Meta {
        rating: rating.clamp(0, 5) as u8,
        flag: if rating < 0 {
            Flag::Reject
        } else if pick {
            Flag::Pick
        } else {
            Flag::None
        },
        label: values.meta.get("xmp:Label").cloned().unwrap_or_default(),
        keywords: {
            // Les mots‑clés hiérarchiques, puis les mots de `dc:subject` qui n'en sont
            // pas un niveau (écrits par un autre logiciel).
            let mut k = values.lists.get(HIERARCHY).cloned().unwrap_or_default();
            let levels = flat_keywords(&k);
            for w in values.lists.get(SUBJECT).into_iter().flatten() {
                if !levels.contains(w) && !k.contains(w) {
                    k.push(w.clone());
                }
            }
            k
        },
        title: first(&values, "dc:title"),
        caption: first(&values, "dc:description"),
        creator: values.lists.get("dc:creator").map(|l| l.join(", ")).unwrap_or_default(),
        rights: first(&values, "dc:rights"),
        location: Location {
            sublocation: place(&values, 0),
            city: place(&values, 1),
            state: place(&values, 2),
            country: place(&values, 3),
        },
        position: match (
            values.meta.get("exif:GPSLatitude").and_then(|v| Position::read(v)),
            values.meta.get("exif:GPSLongitude").and_then(|v| Position::read(v)),
        ) {
            (Some(latitude), Some(longitude)) if latitude.abs() <= 90.0 && longitude.abs() <= 180.0 => {
                Some(Position { latitude, longitude })
            }
            _ => None,
        },
        snapshots: values
            .meta
            .get("luma:Snapshots")
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default(),
    }
}

/// La première valeur d'une liste (celle de la langue par défaut d'un `rdf:Alt`).
fn first(values: &Values, tag: &str) -> String {
    values
        .lists
        .get(tag)
        .and_then(|l| l.first())
        .cloned()
        .unwrap_or_default()
}

fn place(values: &Values, i: usize) -> String {
    values.scalars.get(PLACE[i]).cloned().unwrap_or_default()
}

/// La liste des mots‑clés, rangée avec les listes `crs:`.
const SUBJECT: &str = "dc:subject";

fn parse_values(text: &str) -> luma_core::Result<Values> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut values = Values::default();
    // L'élément `crs:…` ouvert (valeur en texte ou liste `rdf:li`).
    let mut open: Option<String> = None;
    let mut in_li = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| luma_core::Error::Format(e.to_string()))?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(event, Event::Empty(_));
                for attr in e.attributes().flatten() {
                    let key: &str = attr.key.as_ref();
                    if let Some(name) = key.strip_prefix("crs:") {
                        let value = attr
                            .normalized_value(XmlVersion::Implicit1_0)
                            .map_err(|e| luma_core::Error::Format(e.to_string()))?;
                        values.scalars.insert(name.to_owned(), value.into_owned());
                    } else if META_KEYS.contains(&key) {
                        let value = attr
                            .normalized_value(XmlVersion::Implicit1_0)
                            .map_err(|e| luma_core::Error::Format(e.to_string()))?;
                        values.meta.insert(key.to_owned(), value.into_owned());
                    }
                }
                let qname = e.name();
                let tag: &str = qname.as_ref();
                let listed = TEXT_LISTS.iter().chain(&PLACE).find(|t| **t == tag).copied();
                if let Some(name) = tag
                    .strip_prefix("crs:")
                    .or((tag == SUBJECT).then_some(SUBJECT))
                    .or(listed)
                {
                    if !empty {
                        open = Some(name.to_owned());
                    }
                } else if tag == "rdf:li"
                    && !empty
                    && let Some(name) = &open
                {
                    in_li = true;
                    values.lists.entry(name.clone()).or_default().push(String::new());
                }
            }
            // Un texte arrive en morceaux, coupé à chaque entité (`&amp;`, `&#233;`…).
            Event::Text(_) | Event::GeneralRef(_) => {
                let Some(name) = &open else { continue };
                let piece = match &event {
                    Event::Text(t) => t.xml_content(XmlVersion::Implicit1_0).into_owned(),
                    Event::GeneralRef(r) => match r.resolve_char_ref() {
                        Ok(Some(c)) => c.to_string(),
                        _ => {
                            let entity = r.xml_content(XmlVersion::Implicit1_0);
                            quick_xml::escape::resolve_xml_entity(&entity)
                                .unwrap_or_default()
                                .to_owned()
                        }
                    },
                    _ => unreachable!(),
                };
                if in_li {
                    if let Some(item) = values.lists.get_mut(name).and_then(|l| l.last_mut()) {
                        item.push_str(&piece);
                    }
                } else if !piece.trim().is_empty() || values.scalars.contains_key(name) {
                    values.scalars.entry(name.clone()).or_default().push_str(&piece);
                }
            }
            Event::End(ref e) => {
                let qname = e.name();
                let tag: &str = qname.as_ref();
                if tag == "rdf:li" {
                    in_li = false;
                    if let Some(item) = open
                        .as_ref()
                        .and_then(|n| values.lists.get_mut(n))
                        .and_then(|l| l.last_mut())
                    {
                        *item = item.trim().to_owned();
                    }
                } else if (tag.starts_with("crs:")
                    || tag == SUBJECT
                    || TEXT_LISTS.contains(&tag)
                    || PLACE.contains(&tag))
                    && let Some(v) = open.take().and_then(|n| values.scalars.get_mut(&n))
                {
                    *v = v.trim().to_owned();
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(values)
}

fn number(s: &str) -> Option<f32> {
    s.trim().trim_start_matches('+').parse().ok()
}

fn curve(items: &[String]) -> Option<Vec<(f32, f32)>> {
    let points: Option<Vec<(f32, f32)>> = items
        .iter()
        .map(|item| {
            let (x, y) = item.split_once(',')?;
            Some((number(x)?, number(y)?))
        })
        .collect();
    points.filter(|p| p.len() >= 2)
}

/// Les correspondances entre les réglages de LÜMA et les noms `crs:`.
fn fields(s: &mut Settings) -> Vec<(&'static str, &mut f32)> {
    let mut v: Vec<(&'static str, &mut f32)> = vec![
        ("Exposure2012", &mut s.exposure),
        ("LensManualDistortionAmount", &mut s.lens.distortion),
        ("VignetteAmount", &mut s.lens.vignetting),
        ("VignetteMidpoint", &mut s.lens.vignetting_midpoint),
        ("DefringePurpleAmount", &mut s.lens.defringe_purple),
        ("DefringeGreenAmount", &mut s.lens.defringe_green),
        ("PerspectiveVertical", &mut s.transform.vertical),
        ("PerspectiveHorizontal", &mut s.transform.horizontal),
        ("PerspectiveRotate", &mut s.transform.rotate),
        ("PerspectiveAspect", &mut s.transform.aspect),
        ("PerspectiveScale", &mut s.transform.scale),
        ("Contrast2012", &mut s.contrast),
        ("Highlights2012", &mut s.highlights),
        ("Shadows2012", &mut s.shadows),
        ("Whites2012", &mut s.whites),
        ("Blacks2012", &mut s.blacks),
        ("Texture", &mut s.texture),
        ("Clarity2012", &mut s.clarity),
        ("Dehaze", &mut s.dehaze),
        ("Vibrance", &mut s.vibrance),
        ("Saturation", &mut s.saturation),
        ("ParametricHighlights", &mut s.tone_curve.highlights),
        ("ParametricLights", &mut s.tone_curve.lights),
        ("ParametricDarks", &mut s.tone_curve.darks),
        ("ParametricShadows", &mut s.tone_curve.shadows),
        ("ParametricShadowSplit", &mut s.tone_curve.shadow_split),
        ("ParametricMidtoneSplit", &mut s.tone_curve.midtone_split),
        ("ParametricHighlightSplit", &mut s.tone_curve.highlight_split),
        ("SplitToningShadowHue", &mut s.color_grading.shadows.hue),
        ("SplitToningShadowSaturation", &mut s.color_grading.shadows.saturation),
        ("ColorGradeShadowLum", &mut s.color_grading.shadows.luminance),
        ("ColorGradeMidtoneHue", &mut s.color_grading.midtones.hue),
        ("ColorGradeMidtoneSat", &mut s.color_grading.midtones.saturation),
        ("ColorGradeMidtoneLum", &mut s.color_grading.midtones.luminance),
        ("SplitToningHighlightHue", &mut s.color_grading.highlights.hue),
        (
            "SplitToningHighlightSaturation",
            &mut s.color_grading.highlights.saturation,
        ),
        ("ColorGradeHighlightLum", &mut s.color_grading.highlights.luminance),
        ("ColorGradeGlobalHue", &mut s.color_grading.global.hue),
        ("ColorGradeGlobalSat", &mut s.color_grading.global.saturation),
        ("ColorGradeGlobalLum", &mut s.color_grading.global.luminance),
        ("ColorGradeBlending", &mut s.color_grading.blending),
        ("SplitToningBalance", &mut s.color_grading.balance),
        ("Sharpness", &mut s.detail.sharpen_amount),
        ("SharpenRadius", &mut s.detail.sharpen_radius),
        ("SharpenDetail", &mut s.detail.sharpen_detail),
        ("SharpenEdgeMasking", &mut s.detail.sharpen_masking),
        ("LuminanceSmoothing", &mut s.detail.luminance_nr),
        ("LuminanceNoiseReductionDetail", &mut s.detail.luminance_nr_detail),
        ("LuminanceNoiseReductionContrast", &mut s.detail.luminance_nr_contrast),
        ("ColorNoiseReduction", &mut s.detail.color_nr),
        ("ColorNoiseReductionDetail", &mut s.detail.color_nr_detail),
        ("ColorNoiseReductionSmoothness", &mut s.detail.color_nr_smoothness),
        ("PostCropVignetteAmount", &mut s.effects.vignette_amount),
        ("PostCropVignetteMidpoint", &mut s.effects.vignette_midpoint),
        ("PostCropVignetteRoundness", &mut s.effects.vignette_roundness),
        ("PostCropVignetteFeather", &mut s.effects.vignette_feather),
        ("PostCropVignetteHighlightContrast", &mut s.effects.vignette_highlights),
        ("GrainAmount", &mut s.effects.grain_amount),
        ("GrainSize", &mut s.effects.grain_size),
        ("GrainFrequency", &mut s.effects.grain_roughness),
        ("ShadowTint", &mut s.calibration.shadows_tint),
        ("RedHue", &mut s.calibration.red_hue),
        ("RedSaturation", &mut s.calibration.red_saturation),
        ("GreenHue", &mut s.calibration.green_hue),
        ("GreenSaturation", &mut s.calibration.green_saturation),
        ("BlueHue", &mut s.calibration.blue_hue),
        ("BlueSaturation", &mut s.calibration.blue_saturation),
        ("CropLeft", &mut s.crop.left),
        ("CropTop", &mut s.crop.top),
        ("CropRight", &mut s.crop.right),
        ("CropBottom", &mut s.crop.bottom),
        ("CropAngle", &mut s.crop.angle),
    ];
    let hsl = &mut s.hsl;
    for (i, ((hue, sat), lum)) in hsl
        .hue
        .iter_mut()
        .zip(hsl.saturation.iter_mut())
        .zip(hsl.luminance.iter_mut())
        .enumerate()
    {
        let name = XMP_COLORS[i];
        // Noms construits une fois pour toutes (chaînes statiques).
        v.push((leak(format!("HueAdjustment{name}")), hue));
        v.push((leak(format!("SaturationAdjustment{name}")), sat));
        v.push((leak(format!("LuminanceAdjustment{name}")), lum));
    }
    v
}

/// Les 24 noms du mélangeur, alloués une seule fois pour toute la vie du programme.
fn leak(name: String) -> &'static str {
    use std::sync::{Mutex, OnceLock};
    static NAMES: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut names = NAMES.get_or_init(Default::default).lock().expect("noms XMP");
    if let Some(n) = names.get(&name) {
        return n;
    }
    let s: &'static str = Box::leak(name.clone().into_boxed_str());
    names.insert(name, s);
    s
}

/// Les réglages d'un document XMP. Ce qui manque garde sa valeur par défaut.
/// Les éléments d'une liste JSON (masques, retouches, coups de fluidité), un à un : un
/// élément illisible (une forme d'une version plus récente, une valeur abîmée) est
/// sauté, les autres sont gardés — sinon le prochain enregistrement les perdrait tous.
/// `None` si ce n'est pas une liste.
fn each<T: serde::de::DeserializeOwned>(json: &str) -> Option<Vec<T>> {
    let items: Vec<serde_json::Value> = serde_json::from_str(json).ok()?;
    Some(
        items
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect(),
    )
}

pub fn read(text: &str) -> luma_core::Result<Settings> {
    let values = parse_values(text)?;
    // Les mots‑clés et les informations de la photo seuls ne sont pas des réglages.
    if values.scalars.keys().all(|k| PLACE.contains(&k.as_str()))
        && values
            .lists
            .keys()
            .all(|k| k == SUBJECT || TEXT_LISTS.contains(&k.as_str()))
        && !values.meta.contains_key("luma:Masks")
        && !values.meta.contains_key("luma:Spots")
        && !values.meta.contains_key("luma:Liquify")
        && !values.meta.contains_key("luma:ChannelMixer")
        && !values.meta.contains_key("luma:Lut")
        && !values.meta.contains_key("luma:Film")
    {
        return Err(luma_core::Error::Format("aucun réglage crs: dans ce XMP".into()));
    }
    let mut s = Settings::default();
    for (name, slot) in fields(&mut s) {
        if let Some(v) = values.scalars.get(name).and_then(|v| number(v)) {
            *slot = v;
        }
    }
    // Sans recadrage déclaré, les bords lus ne s'appliquent pas.
    if values
        .scalars
        .get("HasCrop")
        .is_some_and(|v| !v.eq_ignore_ascii_case("true"))
    {
        s.crop = crate::Crop::default();
    }
    // Les noms de profils du format XMP ont un équivalent dans LÜMA ; les profils
    // « Camera … » des fabricants restent sur le profil Couleur.
    if let Some(p) = values.scalars.get("CameraProfile").or(values.scalars.get("Look")) {
        let p = p.to_ascii_lowercase();
        if p.contains("neutral") || p.contains("standard") {
            s.profile = Profile::Neutral;
        } else if p.contains("vivid") {
            s.profile = Profile::Vivid;
        } else if p.contains("monochrome") {
            s.profile = Profile::Monochrome;
        } else if p.contains("embedded") {
            s.profile = Profile::Original;
        }
    }
    if values
        .scalars
        .get("ConvertToGrayscale")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    {
        s.profile = Profile::Monochrome;
        // Le mélange noir et blanc (crs:GrayMixer…) : en noir et blanc, c'est
        // la luminance du mélangeur.
        for (i, name) in XMP_COLORS.iter().enumerate() {
            if let Some(v) = values.scalars.get(&format!("GrayMixer{name}")).and_then(|v| number(v)) {
                s.hsl.luminance[i] = v;
            }
        }
    }
    s.lens.remove_ca = values.scalars.get("AutoLateralCA").is_some_and(|v| v.trim() == "1");
    // Les corrections de l'objectif enregistrées : actives sauf si le XMP les coupe.
    s.lens.camera = values.scalars.get("LensProfileEnable").is_none_or(|v| v.trim() != "0");
    let wb = values
        .scalars
        .get("WhiteBalance")
        .map(String::as_str)
        .unwrap_or("As Shot");
    let temp = values.scalars.get("Temperature").and_then(|v| number(v));
    let tint = values.scalars.get("Tint").and_then(|v| number(v));
    if !wb.eq_ignore_ascii_case("As Shot")
        && let Some(temperature) = temp
    {
        s.white_balance = WhiteBalance::Custom {
            temperature,
            tint: tint.unwrap_or(0.0),
        };
    }
    let c = &mut s.tone_curve;
    for (name, slot) in [
        ("ToneCurvePV2012", &mut c.point),
        ("ToneCurvePV2012Red", &mut c.red),
        ("ToneCurvePV2012Green", &mut c.green),
        ("ToneCurvePV2012Blue", &mut c.blue),
    ] {
        if let Some(points) = values.lists.get(name).and_then(|l| curve(l)) {
            *slot = points;
        }
    }
    // Les masques de LÜMA (ceux rangés dans crs:MaskGroupBasedCorrections ne
    // sont pas encore lus).
    if let Some(masks) = values.meta.get("luma:Masks").and_then(|j| each(j)) {
        s.masks = masks;
    }
    // Les retouches (tampon et correcteur).
    if let Some(spots) = values.meta.get("luma:Spots").and_then(|j| each(j)) {
        s.spots = spots;
    }
    // La fluidité : les coups de pinceau.
    if let Some(warps) = values.meta.get("luma:Liquify").and_then(|j| each(j)) {
        s.liquify = warps;
    }
    // Le mélangeur de couches : trois lignes de trois pour cent.
    if let Some(json) = values.meta.get("luma:ChannelMixer")
        && let Ok(rows) = serde_json::from_str::<[[f32; 3]; 3]>(json)
    {
        s.channel_mixer = crate::ChannelMixer {
            red: rows[0],
            green: rows[1],
            blue: rows[2],
        };
    }
    // Le film mesuré (LUT 3D), et le nom du film appliqué : nom et intensité.
    let look = |name: &str, amount: &str| {
        values.meta.get(name).filter(|n| !n.is_empty()).map(|n| crate::Look {
            name: n.clone(),
            amount: values.meta.get(amount).and_then(|v| number(v)).unwrap_or(100.0),
        })
    };
    s.lut = look("luma:Lut", "luma:LutAmount");
    s.film = look("luma:Film", "luma:FilmAmount");
    // La réduction du bruit de luminance par ondelettes.
    s.detail.luminance_nr_wavelets = values.meta.get("luma:NoiseWavelets").is_some_and(|v| v == "True");
    // L'accentuation par déconvolution.
    s.detail.sharpen_deconvolution = values.meta.get("luma:Deconvolution").is_some_and(|v| v == "True");
    // Les hautes lumières reconstruites.
    s.highlight_reconstruction = values
        .meta
        .get("luma:HighlightReconstruction")
        .is_some_and(|v| v == "True");
    Ok(s.clamped())
}

/// Les réglages qu'un préréglage n'emporte pas : ceux de la photo (balance des blancs,
/// exposition, recadrage, objectif, perspective, détail, calibration).
fn photo_only(name: &str) -> bool {
    const KEYS: [&str; 13] = [
        "WhiteBalance",
        "Temperature",
        "Tint",
        "Exposure2012",
        "HasCrop",
        "AutoLateralCA",
        "LensProfileEnable",
        "LensManualDistortionAmount",
        "VignetteAmount",
        "VignetteMidpoint",
        "ShadowTint",
        "Sharpness",
        "ColorNoiseReduction",
    ];
    const PREFIXES: [&str; 12] = [
        "Crop",
        "Perspective",
        "Defringe",
        "Sharpen",
        "LuminanceSmoothing",
        "LuminanceNoiseReduction",
        "ColorNoiseReduction",
        "RedHue",
        "RedSaturation",
        "GreenHue",
        "GreenSaturation",
        "Blue",
    ];
    KEYS.contains(&name) || PREFIXES.iter().any(|p| name.starts_with(p))
}

/// Un préréglage XMP (un `.xmp` qu'on importe
/// dans le panneau Préréglages) : le look de `s` seulement, sous `name`, rangé dans le
/// groupe `group`.
pub fn write_preset(name: &str, group: &str, s: &Settings) -> String {
    let full = write(s);
    let mut out = String::new();
    // Un identifiant stable, tiré du nom (le format en veut un par préréglage).
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in format!("{group}/{name}").bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    let uuid = format!("{h:016X}{:016X}", h.rotate_left(29) ^ 0x5bd1_e995_7a3c_c2b1);
    for line in full.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("crs:")
            && let Some((key, _)) = rest.split_once('=')
            && photo_only(key)
        {
            continue;
        }
        if t.starts_with("xmp:Rating")
            || t.starts_with("luma:Masks")
            || t.starts_with("luma:Spots")
            || t.starts_with("luma:Liquify")
            || t.starts_with("luma:Film")
            || t.starts_with("luma:NoiseWavelets")
            || t.starts_with("luma:Deconvolution")
        {
            continue;
        }
        if t.starts_with("crs:ProcessVersion") {
            out.push_str("    crs:PresetType=\"Normal\"\n    crs:Cluster=\"\"\n");
            out.push_str(&format!("    crs:UUID=\"{uuid}\"\n"));
            out.push_str("    crs:SupportsAmount2=\"True\"\n    crs:SupportsAmount=\"True\"\n");
            out.push_str("    crs:SupportsColor=\"True\"\n    crs:SupportsMonochrome=\"True\"\n");
            out.push_str("    crs:SupportsHighDynamicRange=\"True\"\n    crs:SupportsNormalDynamicRange=\"True\"\n");
            out.push_str("    crs:SupportsSceneReferred=\"True\"\n    crs:SupportsOutputReferred=\"True\"\n");
            out.push_str("    crs:RequiresRGBTables=\"False\"\n    crs:CameraModelRestriction=\"\"\n");
            out.push_str("    crs:Copyright=\"\"\n    crs:ContactInfo=\"\"\n    crs:Version=\"15.0\"\n");
        }
        if t == "</rdf:Description>" {
            for (tag, value) in [("Name", name), ("ShortName", name), ("Group", group)] {
                out.push_str(&format!(
                    "   <crs:{tag}>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n    </rdf:Alt>\n   </crs:{tag}>\n",
                    escape(value)
                ));
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Un préréglage `.xmp` : son nom (ou `None`)
/// et ses réglages.
pub fn read_preset(text: &str) -> luma_core::Result<(Option<String>, Settings)> {
    let settings = read(text)?;
    // Le nom est dans <crs:Name><rdf:Alt><rdf:li>…</rdf:li>.
    let name = text.find("<crs:Name>").and_then(|i| {
        let rest = &text[i..];
        let start = rest.find("<rdf:li")?;
        let open = rest[start..].find('>')? + start + 1;
        let end = rest[open..].find("</rdf:li>")? + open;
        let raw = rest[open..end].trim();
        (!raw.is_empty()).then(|| {
            raw.replace("&quot;", "\"")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&")
        })
    });
    Ok((name, settings))
}

fn fmt_value(name: &str, v: f32) -> String {
    match name {
        "Exposure2012" => format!("{v:+.2}"),
        "SharpenRadius" => format!("{v:+.1}"),
        n if n.starts_with("Crop") => format!("{v:.6}"),
        _ if v == v.round() => {
            let signed = !(name.ends_with("Hue") && !name.contains("Adjustment")) && !name.contains("Split") && v > 0.0;
            if signed { format!("+{v:.0}") } else { format!("{v:.0}") }
        }
        _ => format!("{v:.2}"),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Un fichier XMP complet (sidecar), posé à côté du RAW.
pub fn write(settings: &Settings) -> String {
    write_with_meta(settings, &Meta::default())
}

/// Comme [`write`], avec la note, le drapeau et le libellé de la Bibliothèque.
pub fn write_with_meta(settings: &Settings, meta: &Meta) -> String {
    let mut s = settings.clone().clamped();
    let mut attrs = vec![
        ("ProcessVersion".to_owned(), "11.0".to_owned()),
        ("HasSettings".to_owned(), "True".to_owned()),
        (
            "CameraProfile".to_owned(),
            match s.profile {
                Profile::Color => "Adobe Color",
                Profile::Neutral => "Adobe Neutral",
                Profile::Vivid => "Adobe Vivid",
                Profile::Monochrome => "Adobe Monochrome",
                Profile::Original => "Embedded",
            }
            .to_owned(),
        ),
        (
            "ConvertToGrayscale".to_owned(),
            if s.profile == Profile::Monochrome {
                "True"
            } else {
                "False"
            }
            .to_owned(),
        ),
    ];
    match s.white_balance {
        WhiteBalance::AsShot => attrs.push(("WhiteBalance".into(), "As Shot".into())),
        WhiteBalance::Custom { temperature, tint } => {
            attrs.push(("WhiteBalance".into(), "Custom".into()));
            attrs.push(("Temperature".into(), format!("{temperature:.0}")));
            attrs.push(("Tint".into(), format!("{tint:+.0}")));
        }
    }
    let has_crop = !s.crop.is_identity();
    for (name, v) in fields(&mut s) {
        if name.starts_with("Crop") && !has_crop {
            continue;
        }
        attrs.push((name.to_owned(), fmt_value(name, *v)));
    }
    attrs.push(("HasCrop".into(), if has_crop { "True".into() } else { "False".into() }));
    attrs.push((
        "AutoLateralCA".into(),
        if s.lens.remove_ca { "1".into() } else { "0".into() },
    ));
    attrs.push((
        "LensProfileEnable".into(),
        if s.lens.camera { "1".into() } else { "0".into() },
    ));

    if s.profile == Profile::Monochrome {
        for (i, name) in XMP_COLORS.iter().enumerate() {
            attrs.push((format!("GrayMixer{name}"), fmt_value("GrayMixer", s.hsl.luminance[i])));
        }
    }
    let mut out = open_document(meta, true);
    if !s.masks.is_empty() {
        let json = serde_json::to_string(&s.masks).unwrap_or_default();
        out.push_str(&format!("    luma:Masks=\"{}\"\n", escape(&json)));
    }
    if !s.channel_mixer.is_identity() {
        let m = &s.channel_mixer;
        let json = serde_json::to_string(&[m.red, m.green, m.blue]).unwrap_or_default();
        out.push_str(&format!("    luma:ChannelMixer=\"{}\"\n", escape(&json)));
    }
    if !s.spots.is_empty() {
        let json = serde_json::to_string(&s.spots).unwrap_or_default();
        out.push_str(&format!("    luma:Spots=\"{}\"\n", escape(&json)));
    }
    if !s.liquify.is_empty() {
        let json = serde_json::to_string(&s.liquify).unwrap_or_default();
        out.push_str(&format!("    luma:Liquify=\"{}\"\n", escape(&json)));
    }
    if s.detail.luminance_nr_wavelets {
        out.push_str("    luma:NoiseWavelets=\"True\"\n");
    }
    if s.detail.sharpen_deconvolution {
        out.push_str("    luma:Deconvolution=\"True\"\n");
    }
    if s.highlight_reconstruction {
        out.push_str("    luma:HighlightReconstruction=\"True\"\n");
    }
    for (key, look) in [("Lut", &s.lut), ("Film", &s.film)] {
        if let Some(l) = look {
            out.push_str(&format!("    luma:{key}=\"{}\"\n", escape(&l.name)));
            out.push_str(&format!("    luma:{key}Amount=\"{}\"\n", l.amount));
        }
    }
    for (name, value) in &attrs {
        out.push_str(&format!("    crs:{name}=\"{}\"\n", escape(value)));
    }
    out.push_str("   >\n");
    let c = &s.tone_curve;
    for (name, points) in [
        ("ToneCurvePV2012", &c.point),
        ("ToneCurvePV2012Red", &c.red),
        ("ToneCurvePV2012Green", &c.green),
        ("ToneCurvePV2012Blue", &c.blue),
    ] {
        out.push_str(&format!("   <crs:{name}>\n    <rdf:Seq>\n"));
        for (x, y) in points {
            out.push_str(&format!("     <rdf:li>{x:.0}, {y:.0}</rdf:li>\n"));
        }
        out.push_str(&format!("    </rdf:Seq>\n   </crs:{name}>\n"));
    }
    close_document(&mut out, meta);
    out
}

/// Un XMP qui ne porte que la note, le drapeau, le libellé et les mots‑clés : celui
/// d'une photo jamais développée, dont on change les métadonnées depuis la
/// Bibliothèque. Sans réglages, la photo garde son rendu par défaut.
pub fn write_meta(meta: &Meta) -> String {
    let mut out = open_document(meta, false);
    out.push_str("   >\n");
    close_document(&mut out, meta);
    out
}

/// Le début d'un document : espaces de noms et métadonnées en attributs.
fn open_document(meta: &Meta, settings: bool) -> String {
    let mut out = String::from(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"LÜMA\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  <rdf:Description rdf:about=\"\"\n",
    );
    if settings {
        out.push_str(&format!("    xmlns:crs=\"{CRS}\"\n"));
    }
    out.push_str("    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n");
    let texts =
        !meta.title.is_empty() || !meta.caption.is_empty() || !meta.creator.is_empty() || !meta.rights.is_empty();
    if !meta.keywords.is_empty() || texts {
        out.push_str("    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n");
    }
    if !meta.location.is_empty() {
        out.push_str(&format!("    xmlns:Iptc4xmpExt=\"{IPTC_EXT}\"\n"));
    }
    if let Some(p) = meta.position {
        out.push_str("    xmlns:exif=\"http://ns.adobe.com/exif/1.0/\"\n");
        out.push_str(&format!(
            "    exif:GPSLatitude=\"{}\"\n    exif:GPSLongitude=\"{}\"\n",
            Position::write(p.latitude, 'N', 'S'),
            Position::write(p.longitude, 'E', 'W')
        ));
    }
    out.push_str(&format!("    xmlns:luma=\"{LUMA_NS}\"\n"));
    let rating = if meta.flag == Flag::Reject {
        -1
    } else {
        i32::from(meta.rating.min(5))
    };
    out.push_str(&format!("    xmp:Rating=\"{rating}\"\n"));
    if !meta.label.is_empty() {
        out.push_str(&format!("    xmp:Label=\"{}\"\n", escape(&meta.label)));
    }
    if meta.flag == Flag::Pick {
        out.push_str("    luma:Pick=\"True\"\n");
    }
    if !meta.snapshots.is_empty() {
        let json = serde_json::to_string(&meta.snapshots).unwrap_or_default();
        out.push_str(&format!("    luma:Snapshots=\"{}\"\n", escape(&json)));
    }
    out
}

/// La fin d'un document : la liste des mots‑clés, puis les balises fermantes.
fn close_document(out: &mut String, meta: &Meta) {
    out.push_str(&meta_elements(meta));
    out.push_str("  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n");
}

/// Les éléments des informations d'une photo (titre, légende, auteur, droits,
/// mots‑clés, lieu), pour un XMP ; les espaces `dc:`, `luma:` et `Iptc4xmpExt:` sont
/// déclarés par celui qui l'écrit.
pub fn meta_elements(meta: &Meta) -> String {
    let mut out = String::new();
    let alt = |out: &mut String, tag: &str, text: &str| {
        if !text.trim().is_empty() {
            out.push_str(&format!(
                "   <{tag}>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n    </rdf:Alt>\n   </{tag}>\n",
                escape(text.trim())
            ));
        }
    };
    alt(&mut out, "dc:title", &meta.title);
    alt(&mut out, "dc:description", &meta.caption);
    if !meta.creator.trim().is_empty() {
        out.push_str("   <dc:creator>\n    <rdf:Seq>\n");
        for c in meta.creator.split(',').map(str::trim).filter(|c| !c.is_empty()) {
            out.push_str(&format!("     <rdf:li>{}</rdf:li>\n", escape(c)));
        }
        out.push_str("    </rdf:Seq>\n   </dc:creator>\n");
    }
    alt(&mut out, "dc:rights", &meta.rights);
    let bag = |out: &mut String, tag: &str, items: &[String]| {
        if !items.is_empty() {
            out.push_str(&format!("   <{tag}>\n    <rdf:Bag>\n"));
            for k in items {
                out.push_str(&format!("     <rdf:li>{}</rdf:li>\n", escape(k)));
            }
            out.push_str(&format!("    </rdf:Bag>\n   </{tag}>\n"));
        }
    };
    bag(&mut out, SUBJECT, &flat_keywords(&meta.keywords));
    let paths: Vec<String> = meta.keywords.iter().filter(|k| k.contains(LEVEL)).cloned().collect();
    bag(&mut out, HIERARCHY, &paths);
    let l = &meta.location;
    if !l.is_empty() {
        out.push_str("   <Iptc4xmpExt:LocationCreated>\n    <rdf:Bag>\n     <rdf:li rdf:parseType=\"Resource\">\n");
        for (tag, v) in PLACE.iter().zip([&l.sublocation, &l.city, &l.state, &l.country]) {
            if !v.trim().is_empty() {
                out.push_str(&format!("      <{tag}>{}</{tag}>\n", escape(v.trim())));
            }
        }
        out.push_str("     </rdf:li>\n    </rdf:Bag>\n   </Iptc4xmpExt:LocationCreated>\n");
    }
    out
}

/// L'index d'une couleur du mélangeur à partir de son nom XMP.
pub fn color_index(name: &str) -> Option<usize> {
    XMP_COLORS
        .iter()
        .position(|c| c.eq_ignore_ascii_case(name))
        .or_else(|| HSL_COLORS.iter().position(|c| *c == name))
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_position_is_written_and_read_back() {
        let meta = Meta {
            rating: 3,
            position: Some(Position {
                latitude: 48.858_37,
                longitude: -2.294_48,
            }),
            ..Meta::default()
        };
        for text in [write_meta(&meta), write_with_meta(&Settings::default(), &meta)] {
            assert!(text.contains("exif:GPSLatitude=\"48,51.502200N\""), "{text}");
            assert!(text.contains("exif:GPSLongitude=\"2,17.668800W\""), "{text}");
            let back = read_meta(&text);
            let p = back.position.unwrap();
            assert!((p.latitude - 48.858_37).abs() < 1e-8 && (p.longitude + 2.294_48).abs() < 1e-8);
            assert_eq!(back.rating, 3);
        }
        assert!(!write_meta(&Meta::default()).contains("exif:"));
        // Les autres écritures : secondes, degrés décimaux ; une coordonnée fausse est ignorée.
        assert_eq!(Position::read("48,51,30.6N"), Some(48.0 + 51.0 / 60.0 + 30.6 / 3600.0));
        assert_eq!(Position::read("-33.5"), Some(-33.5));
        assert_eq!(Position::read("151,12.5E"), Some(151.0 + 12.5 / 60.0));
        assert_eq!(Position::read("nord"), None);
        let odd = write_meta(&meta).replace("48,51.502200N", "148,0N");
        assert_eq!(read_meta(&odd).position, None);
    }
    /// Les instantanés font l'aller‑retour par le XMP, avec ou sans réglages, masques et
    /// guillemets compris ; le reste des métadonnées n'en souffre pas.
    #[test]
    fn snapshots_round_trip() {
        let mut a = Settings {
            exposure: 0.8,
            ..Settings::default()
        };
        a.masks.push(crate::Mask::full());
        let b = Settings {
            contrast: -20.0,
            ..Settings::default()
        };
        let meta = Meta {
            rating: 3,
            title: "Le \"phare\" & la mer".into(),
            snapshots: vec![Snapshot::new("Avant le ciel", &a), Snapshot::new("N&B <doux>", &b)],
            ..Meta::default()
        };
        for text in [write_with_meta(&b, &meta), write_meta(&meta)] {
            let back = read_meta(&text);
            assert_eq!(back, meta);
            assert_eq!(back.snapshots[0].settings().unwrap(), a);
            assert_eq!(back.snapshots[1].settings().unwrap().contrast, -20.0);
        }
        // Sans instantané, rien d'écrit.
        assert!(!write_meta(&Meta::default()).contains("Snapshots"));
        assert!(
            Snapshot {
                name: "x".into(),
                settings: "pas du json".into()
            }
            .settings()
            .is_none()
        );
    }

    use super::*;

    #[test]
    fn one_unreadable_mask_spot_or_stroke_keeps_the_others() {
        // Un masque d'une forme inconnue (écrite par une version plus récente), une
        // retouche et un coup de fluidité aux valeurs impossibles : seuls eux sont
        // sautés.
        let s = Settings {
            masks: vec![crate::Mask::linear(), crate::Mask::radial()],
            spots: vec![
                crate::Spot::new(crate::SpotKind::Heal, (0.3, 0.4), 0.02),
                crate::Spot::new(crate::SpotKind::Remove, (0.6, 0.6), 0.05),
            ],
            liquify: vec![
                crate::Warp::new(crate::WarpKind::Bloat, (0.5, 0.5), 0.1, 40.0),
                crate::Warp::new(crate::WarpKind::Pucker, (0.2, 0.2), 0.1, 40.0),
            ],
            ..Settings::default()
        };
        let q = "&quot;";
        let text = write(&s)
            .replacen(
                &format!("{q}kind{q}:{q}linear{q}"),
                &format!("{q}kind{q}:{q}spirale{q}"),
                1,
            )
            .replacen(
                &format!("{q}kind{q}:{q}remove{q}"),
                &format!("{q}kind{q}:{q}gomme{q}"),
                1,
            )
            .replacen(
                &format!("{q}kind{q}:{q}pucker{q}"),
                &format!("{q}kind{q}:{q}tourbillon{q}"),
                1,
            );
        let back = read(&text).unwrap();
        assert_eq!(back.masks.len(), 1);
        assert!(matches!(back.masks[0].shape, crate::Shape::Radial { .. }));
        assert_eq!(back.spots.len(), 1);
        assert_eq!(back.spots[0].kind, crate::SpotKind::Heal);
        assert_eq!(back.liquify.len(), 1);
        assert_eq!(back.liquify[0].kind, crate::WarpKind::Bloat);
    }

    /// Un extrait de sidecar XMP, tel qu'on en trouve à côté des RAW.
    const SIDECAR: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core 7.0-c000">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/"
   crs:Version="15.0"
   crs:ProcessVersion="11.0"
   crs:WhiteBalance="Custom"
   crs:Temperature="5350"
   crs:Tint="+12"
   crs:Exposure2012="+0.65"
   crs:Contrast2012="+18"
   crs:Highlights2012="-72"
   crs:Shadows2012="+45"
   crs:Whites2012="+10"
   crs:Blacks2012="-14"
   crs:Texture="+8"
   crs:Clarity2012="+15"
   crs:Dehaze="+5"
   crs:Vibrance="+20"
   crs:Saturation="-5"
   crs:HueAdjustmentOrange="-6"
   crs:SaturationAdjustmentBlue="-25"
   crs:LuminanceAdjustmentAqua="+30"
   crs:SplitToningShadowHue="220"
   crs:SplitToningShadowSaturation="12"
   crs:ColorGradeMidtoneHue="35"
   crs:ColorGradeBlending="60"
   crs:Sharpness="55"
   crs:PostCropVignetteAmount="-18"
   crs:HasCrop="True"
   crs:CropLeft="0.05"
   crs:CropTop="0.1"
   crs:CropRight="0.95"
   crs:CropBottom="0.9"
   crs:CropAngle="1.5"
   crs:CameraProfile="Adobe Vivid">
   <crs:ToneCurvePV2012>
    <rdf:Seq>
     <rdf:li>0, 12</rdf:li>
     <rdf:li>128, 136</rdf:li>
     <rdf:li>255, 245</rdf:li>
    </rdf:Seq>
   </crs:ToneCurvePV2012>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

    #[test]
    fn reads_a_sidecar() {
        let s = read(SIDECAR).unwrap();
        assert_eq!(
            s.white_balance,
            WhiteBalance::Custom {
                temperature: 5350.0,
                tint: 12.0
            }
        );
        assert_eq!(
            (s.exposure, s.contrast, s.highlights, s.shadows),
            (0.65, 18.0, -72.0, 45.0)
        );
        assert_eq!(
            (s.whites, s.blacks, s.texture, s.clarity, s.dehaze),
            (10.0, -14.0, 8.0, 15.0, 5.0)
        );
        assert_eq!((s.vibrance, s.saturation), (20.0, -5.0));
        assert_eq!(s.hsl.hue[1], -6.0);
        assert_eq!(s.hsl.saturation[5], -25.0);
        assert_eq!(s.hsl.luminance[4], 30.0);
        assert_eq!(s.color_grading.shadows.hue, 220.0);
        assert_eq!(s.color_grading.midtones.hue, 35.0);
        assert_eq!(s.color_grading.blending, 60.0);
        assert_eq!(s.detail.sharpen_amount, 55.0);
        assert_eq!(s.effects.vignette_amount, -18.0);
        assert_eq!(
            (s.crop.left, s.crop.top, s.crop.right, s.crop.bottom, s.crop.angle),
            (0.05, 0.1, 0.95, 0.9, 1.5)
        );
        assert_eq!(s.tone_curve.point, vec![(0.0, 12.0), (128.0, 136.0), (255.0, 245.0)]);
        assert_eq!(s.profile, Profile::Vivid);
    }

    #[test]
    fn as_shot_keeps_the_camera_balance() {
        let s = read(r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/" crs:WhiteBalance="As Shot" crs:Exposure2012="-1.00"/></rdf:RDF></x:xmpmeta>"#).unwrap();
        assert_eq!(s.white_balance, WhiteBalance::AsShot);
        assert_eq!(s.exposure, -1.0);
    }

    #[test]
    fn written_settings_read_back_identically() {
        let mut s = read(SIDECAR).unwrap();
        s.hsl.hue[7] = 12.0;
        s.tone_curve.red = vec![(0.0, 0.0), (64.0, 70.0), (255.0, 255.0)];
        let text = write(&s);
        assert!(text.contains("crs:Exposure2012=\"+0.65\""));
        assert!(text.contains("crs:HueAdjustmentMagenta=\"+12\""));
        assert_eq!(read(&text).unwrap(), s);
    }

    #[test]
    fn masks_round_trip() {
        let mut s = read(SIDECAR).unwrap();
        let mut sky = crate::Mask::linear();
        sky.local.exposure = -0.8;
        sky.local.temperature = -20.0;
        let mut brush = crate::Mask::brush();
        brush.local.clarity = 40.0;
        if let crate::Shape::Brush { dabs } = &mut brush.shape {
            dabs.push(crate::Dab {
                x: 0.3,
                y: 0.4,
                radius: 0.05,
                feather: 50.0,
                flow: 100.0,
                erase: false,
            });
        }
        // Un masque radial dont on retire un pinceau inversé.
        let mut combined = crate::Mask::radial();
        combined.parts.push(crate::Part {
            combine: crate::Combine::Subtract,
            shape: brush.shape.clone(),
            invert: true,
        });
        s.masks = vec![sky, brush, combined];
        s.liquify = vec![crate::Warp {
            points: vec![(0.3, 0.3), (0.32, 0.31)],
            ..crate::Warp::new(crate::WarpKind::Push, (0.3, 0.3), 0.06, 40.0)
        }];
        s.spots = vec![
            crate::Spot::new(crate::SpotKind::Heal, (0.4, 0.3), 0.02),
            crate::Spot {
                opacity: 70.0,
                ..crate::Spot::new(crate::SpotKind::Clone, (0.95, 0.8), 0.03)
            },
            crate::Spot::new(crate::SpotKind::RedEye, (0.5, 0.45), 0.01),
            crate::Spot::new(crate::SpotKind::Remove, (0.2, 0.7), 0.03),
        ];
        s.lens.remove_ca = true;
        s.lens.camera = false;
        s.lens.distortion = 12.0;
        s.transform.vertical = -25.0;
        s.transform.scale = 95.0;
        s.lut = Some(crate::Look {
            name: "Ma Portra 400 \"scan\"".into(),
            amount: 70.0,
        });
        s.film = Some(crate::Look {
            name: "Portra 400".into(),
            amount: 80.0,
        });
        let text = write(&s);
        assert!(text.contains("crs:LensProfileEnable=\"0\""), "{text}");
        assert!(text.contains("luma:Masks="));
        assert!(text.contains("luma:Spots="));
        assert!(text.contains("luma:Liquify="));
        assert!(text.contains("luma:LutAmount=\"70\""));
        assert_eq!(read(&text).unwrap(), s);
        // Sans l'étiquette, les corrections du boîtier restent actives.
        let without = text.replace("crs:LensProfileEnable=\"0\"", "");
        assert!(read(&without).unwrap().lens.camera);
    }

    #[test]
    fn highlight_reconstruction_round_trip() {
        let s = Settings {
            highlight_reconstruction: true,
            ..Settings::default()
        };
        let text = write(&s);
        assert!(text.contains("luma:HighlightReconstruction=\"True\""), "{text}");
        assert!(read(&text).unwrap().highlight_reconstruction);
        // Un choix de rendu : il va dans les préréglages.
        assert!(write_preset("Ciels", "LÜMA", &s).contains("HighlightReconstruction"));
        assert!(!write(&Settings::default()).contains("HighlightReconstruction"));
    }

    #[test]
    fn deconvolution_round_trip() {
        let mut s = Settings::default();
        s.detail.sharpen_deconvolution = true;
        let text = write(&s);
        assert!(text.contains("luma:Deconvolution=\"True\""), "{text}");
        assert!(read(&text).unwrap().detail.sharpen_deconvolution);
        // Propre à la photo, comme le reste du Détail.
        assert!(!write_preset("Netteté", "LÜMA", &s).contains("Deconvolution"));
        assert!(!write(&Settings::default()).contains("Deconvolution"));
    }

    #[test]
    fn wavelet_noise_reduction_round_trip() {
        let mut s = Settings::default();
        s.detail.luminance_nr = 40.0;
        s.detail.luminance_nr_wavelets = true;
        let text = write(&s);
        assert!(text.contains("luma:NoiseWavelets=\"True\""), "{text}");
        assert!(read(&text).unwrap().detail.luminance_nr_wavelets);
        // Le détail reste à la photo : un préréglage ne l'emporte pas.
        assert!(!write_preset("Bruit", "LÜMA", &s).contains("NoiseWavelets"));
        // Par défaut, rien d'écrit.
        assert!(!write(&Settings::default()).contains("NoiseWavelets"));
    }

    #[test]
    fn channel_mixer_round_trip() {
        let mut s = Settings::default();
        s.channel_mixer.red = [80.0, 30.0, -10.0];
        s.channel_mixer.blue = [0.0, 20.0, 80.0];
        let text = write(&s);
        assert!(text.contains("luma:ChannelMixer="), "{text}");
        assert_eq!(read(&text).unwrap().channel_mixer, s.channel_mixer);
        // Le mélangeur va dans les préréglages : c'est un rendu, pas une retouche de photo.
        let preset = write_preset("Mélange", "LÜMA", &s);
        assert!(preset.contains("luma:ChannelMixer="));
        // Neutre : rien d'écrit.
        assert!(!write(&Settings::default()).contains("ChannelMixer"));
        // Hors des bornes : ramené.
        s.channel_mixer.green = [500.0, f32::NAN, -900.0];
        assert_eq!(s.clamped().channel_mixer.green, [200.0, 0.0, -200.0]);
    }

    #[test]
    fn spots_stay_with_their_photo() {
        // Les retouches sont propres à une photo : un préréglage ne les emporte pas, et
        // un film appliqué les garde.
        let s = Settings {
            exposure: 0.5,
            spots: vec![crate::Spot::new(crate::SpotKind::Heal, (0.5, 0.5), 0.02)],
            ..Settings::default()
        };
        assert!(!write_preset("Moi", "LÜMA", &s).contains("luma:Spots"));
        let pushed = Settings {
            liquify: vec![crate::Warp::new(crate::WarpKind::Pucker, (0.5, 0.5), 0.05, 60.0)],
            ..Settings::default()
        };
        assert!(!write_preset("Moi", "LÜMA", &pushed).contains("luma:Liquify"));
        // La fluidité seule suffit, elle aussi, à faire un XMP de réglages.
        assert_eq!(read(&write(&pushed)).unwrap().liquify, pushed.liquify);
        let film = crate::presets::find("Portra 400").unwrap().applied(&s, 0.6);
        assert_eq!(film.spots, s.spots);
        // Des retouches seules suffisent à faire un XMP de réglages.
        let only = Settings {
            spots: s.spots.clone(),
            ..Settings::default()
        };
        assert_eq!(read(&write(&only)).unwrap().spots, s.spots);
    }

    #[test]
    fn masks_without_parts_still_read() {
        // Un masque enregistré avant les combinaisons n'a pas de parties.
        let old: crate::Mask = serde_json::from_str(r#"{"name":"A","shape":{"kind":"full"}}"#).unwrap();
        assert!(old.parts.is_empty());
        assert!(!serde_json::to_string(&old).unwrap().contains("parts"));
    }

    #[test]
    fn presets_are_written_as_xmp_and_read_back() {
        let mut s = crate::presets::find("Kodachrome 64")
            .unwrap()
            .applied(&Settings::default(), 1.0);
        s.exposure = 1.5;
        s.white_balance = WhiteBalance::Custom {
            temperature: 3200.0,
            tint: 5.0,
        };
        s.crop.left = 0.2;
        let text = write_preset("Kodachrome 64 & moi", "LÜMA — Pellicules couleur", &s);
        for needed in [
            "crs:PresetType=\"Normal\"",
            "crs:UUID=",
            "<crs:Group>",
            "crs:Contrast2012=",
        ] {
            assert!(text.contains(needed), "{needed}");
        }
        for photo in ["Exposure2012", "Temperature", "CropLeft", "luma:Film"] {
            assert!(!text.contains(photo), "{photo}");
        }
        let (name, back) = read_preset(&text).unwrap();
        assert_eq!(name.as_deref(), Some("Kodachrome 64 & moi"));
        assert_eq!(back.contrast, s.contrast);
        assert_eq!(back.hsl, s.hsl);
        assert_eq!(back.tone_curve, s.tone_curve);
        assert_eq!(back.exposure, 0.0, "l'exposition reste celle de la photo");
    }

    #[test]
    fn black_and_white_mix_round_trips_as_gray_mixer() {
        let mut s = Settings {
            profile: Profile::Monochrome,
            ..Settings::default()
        };
        s.hsl.luminance[0] = 40.0;
        s.hsl.luminance[5] = -60.0;
        let text = write(&s);
        assert!(text.contains("crs:GrayMixerRed=\"+40\""), "{text}");
        assert!(text.contains("crs:GrayMixerBlue=\"-60\""));
        let external = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/" crs:ConvertToGrayscale="True" crs:GrayMixerRed="-30" crs:GrayMixerGreen="+25"/></rdf:RDF></x:xmpmeta>"#;
        let back = read(external).unwrap();
        assert_eq!(back.profile, Profile::Monochrome);
        assert_eq!((back.hsl.luminance[0], back.hsl.luminance[3]), (-30.0, 25.0));
    }

    #[test]
    fn iptc_information_round_trip() {
        let meta = Meta {
            rating: 3,
            keywords: vec!["Lieux|France|Paris".into(), "Nuit".into(), "Lieux|France".into()],
            title: "La Seine & ses ponts".into(),
            caption: "Le pont des Arts au crépuscule.\nPose de 4 s.".into(),
            creator: "Maxime Lestage, Assistante".into(),
            rights: "© 2026 Maxime Lestage".into(),
            location: Location {
                sublocation: "Pont des Arts".into(),
                city: "Paris".into(),
                state: "Île-de-France".into(),
                country: "France".into(),
            },
            ..Meta::default()
        };
        let text = write_meta(&meta);
        // Chaque niveau dans `dc:subject`, que lisent les autres logiciels.
        for w in ["Lieux", "France", "Paris", "Nuit"] {
            assert!(text.contains(&format!("<rdf:li>{w}</rdf:li>")), "{w}");
        }
        let back = read_meta(&text);
        assert_eq!(back.title, meta.title);
        assert_eq!(back.caption, meta.caption);
        assert_eq!(back.creator, "Maxime Lestage, Assistante");
        assert_eq!(back.rights, meta.rights);
        assert_eq!(back.location, meta.location);
        assert_eq!(back.keywords, ["Lieux|France|Paris", "Lieux|France", "Nuit"]);
        // Un XMP d'informations seules n'a pas de réglages.
        assert!(read(&text).is_err());
    }

    #[test]
    fn iptc_written_elsewhere() {
        // Le lieu dans sa structure, des mots‑clés à plat, le titre dans une autre langue.
        let other = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:Iptc4xmpExt="http://iptc.org/std/Iptc4xmpExt/2008-02-29/"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Marée</rdf:li><rdf:li xml:lang="en">Tide</rdf:li></rdf:Alt></dc:title><dc:subject><rdf:Bag><rdf:li>mer</rdf:li></rdf:Bag></dc:subject><Iptc4xmpExt:LocationCreated><rdf:Bag><rdf:li rdf:parseType="Resource"><Iptc4xmpExt:City>Saint-Malo</Iptc4xmpExt:City><Iptc4xmpExt:CountryName>France</Iptc4xmpExt:CountryName></rdf:li></rdf:Bag></Iptc4xmpExt:LocationCreated></rdf:Description></rdf:RDF></x:xmpmeta>"#;
        let m = read_meta(other);
        assert_eq!(m.title, "Marée");
        assert_eq!(m.keywords, ["mer"]);
        assert_eq!(
            (m.location.city.as_str(), m.location.country.as_str()),
            ("Saint-Malo", "France")
        );
    }

    #[test]
    fn metadata_templates() {
        let t = Template {
            name: "Studio".into(),
            creator: "Maxime Lestage".into(),
            rights: "Tous droits réservés".into(),
            keywords: vec!["Studio".into()],
            ..Template::default()
        };
        let mut m = Meta {
            title: "Portrait".into(),
            keywords: vec!["Studio".into(), "Lumière".into()],
            location: Location {
                city: "Lyon".into(),
                ..Location::default()
            },
            ..Meta::default()
        };
        t.apply(&mut m);
        assert_eq!(
            (m.creator.as_str(), m.rights.as_str()),
            ("Maxime Lestage", "Tous droits réservés")
        );
        assert_eq!(m.title, "Portrait", "le modèle ne touche pas au titre");
        assert_eq!(m.location.city, "Lyon", "un champ vide du modèle ne change rien");
        assert_eq!(m.keywords, ["Studio", "Lumière"], "pas de mot‑clé en double");
        assert_eq!(Template::of("Copie", &m).creator, "Maxime Lestage");
    }

    #[test]
    fn colour_labels_in_three_languages() {
        assert_eq!(label_color("Red"), Some(0));
        assert_eq!(label_color("rouge"), Some(0));
        assert_eq!(label_color(" Púrpura "), Some(4));
        assert_eq!(label_color("VERT"), Some(2));
        assert_eq!(label_color("Bleu"), label_color("azul"));
        assert_eq!(label_color("À revoir"), None);
        assert_eq!(LABELS.map(label_color), [0, 1, 2, 3, 4].map(Some));
    }

    #[test]
    fn rating_flag_and_label() {
        let s = read(SIDECAR).unwrap();
        assert_eq!(read_meta(SIDECAR), Meta::default());
        for meta in [
            Meta {
                rating: 4,
                flag: Flag::Pick,
                label: "Rouge".into(),
                keywords: vec!["Montagne".into(), "Été & neige <2026>".into()],
                ..Meta::default()
            },
            Meta {
                rating: 0,
                flag: Flag::Reject,
                label: String::new(),
                keywords: Vec::new(),
                ..Meta::default()
            },
            Meta {
                rating: 5,
                flag: Flag::None,
                label: String::new(),
                keywords: Vec::new(),
                ..Meta::default()
            },
        ] {
            let text = write_with_meta(&s, &meta);
            assert_eq!(read_meta(&text), meta);
            assert_eq!(read(&text).unwrap(), s);
        }
        let basic = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="3" xmp:Label="Vert"/></rdf:RDF></x:xmpmeta>"#;
        assert_eq!(
            read_meta(basic),
            Meta {
                rating: 3,
                flag: Flag::None,
                label: "Vert".into(),
                keywords: Vec::new(),
                ..Meta::default()
            }
        );
        // Les mots‑clés écrits par un autre logiciel : la liste `dc:subject`.
        let tagged = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:subject><rdf:Bag><rdf:li>Portrait</rdf:li><rdf:li>Studio</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF></x:xmpmeta>"#;
        assert_eq!(read_meta(tagged).keywords, ["Portrait", "Studio"]);
        assert!(read(tagged).is_err(), "des mots‑clés seuls ne sont pas des réglages");
        // Sans réglages : les métadonnées seules, et la photo garde son rendu par défaut.
        let only = Meta {
            rating: 2,
            flag: Flag::Pick,
            label: String::new(),
            keywords: vec!["Mer".into()],
            ..Meta::default()
        };
        let text = write_meta(&only);
        assert_eq!(read_meta(&text), only);
        assert!(read(&text).is_err(), "aucun réglage : {text}");
    }

    #[test]
    fn rejects_documents_without_settings() {
        assert!(read("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>").is_err());
        assert!(read("<pas du xml").is_err());
        assert_eq!(color_index("Aqua"), Some(4));
    }
}
