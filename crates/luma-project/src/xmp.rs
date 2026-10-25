//! Lecture et écriture des réglages au format XMP de Camera Raw / Lightroom
//! (espace de noms `crs:`).
//!
//! Un photographe qui arrive de Lightroom garde ses retouches : les fichiers `.xmp`
//! posés à côté des RAW (ou exportés depuis son catalogue) sont lus par LÜMA, curseur
//! par curseur. Dans l'autre sens, LÜMA écrit ses réglages dans le même vocabulaire.
//!
//! Les noms des réglages sont ceux du processus 2012 et suivants (`Exposure2012`,
//! `Highlights2012`, `ToneCurvePV2012`…). Les moteurs de rendu étant différents, une
//! même valeur ne donne pas un résultat identique au pixel près, mais elle a le même
//! sens et la même amplitude.

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

const META_KEYS: [&str; 8] = [
    "xmp:Rating",
    "xmp:Label",
    "luma:Pick",
    "luma:Masks",
    "luma:Lut",
    "luma:LutAmount",
    "luma:Film",
    "luma:FilmAmount",
];
const LUMA_NS: &str = "https://luma.photo/ns/1.0/";

/// Le drapeau de la Bibliothèque (P, X, U dans Lightroom).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Flag {
    #[default]
    None,
    Pick,
    Reject,
}

/// Ce que la Bibliothèque sait d'une photo : note de 0 à 5 étoiles, drapeau, libellé
/// de couleur. La note et le libellé sont ceux de Lightroom et Bridge (`xmp:Rating`,
/// `xmp:Label`) ; un rejet s'écrit `xmp:Rating="-1"` comme dans Bridge, et le drapeau
/// « retenue », que Lightroom garde dans son catalogue, dans l'espace `luma:`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meta {
    pub rating: u8,
    pub flag: Flag,
    pub label: String,
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
    }
}

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
                if let Some(name) = tag.strip_prefix("crs:") {
                    if !empty {
                        open = Some(name.to_owned());
                    }
                } else if tag == "rdf:li" && open.is_some() && !empty {
                    in_li = true;
                }
            }
            Event::Text(t) => {
                let Some(name) = &open else { continue };
                let text = t.xml_content(XmlVersion::Implicit1_0);
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                if in_li {
                    values.lists.entry(name.clone()).or_default().push(text.to_owned());
                } else {
                    values.scalars.insert(name.clone(), text.to_owned());
                }
            }
            Event::End(ref e) => {
                let qname = e.name();
                let tag: &str = qname.as_ref();
                if tag == "rdf:li" {
                    in_li = false;
                } else if tag.starts_with("crs:") {
                    open = None;
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
pub fn read(text: &str) -> luma_core::Result<Settings> {
    let values = parse_values(text)?;
    if values.scalars.is_empty()
        && values.lists.is_empty()
        && !values.meta.contains_key("luma:Masks")
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
    // Les profils d'Adobe ont un équivalent dans LÜMA ; les profils « Camera … » des
    // fabricants restent sur le profil Couleur.
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
        // Le mélange noir et blanc de Lightroom (crs:GrayMixer…) : en noir et blanc, c'est
        // la luminance du mélangeur.
        for (i, name) in XMP_COLORS.iter().enumerate() {
            if let Some(v) = values.scalars.get(&format!("GrayMixer{name}")).and_then(|v| number(v)) {
                s.hsl.luminance[i] = v;
            }
        }
    }
    s.lens.remove_ca = values.scalars.get("AutoLateralCA").is_some_and(|v| v.trim() == "1");
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
    // Les masques de LÜMA (ceux de Lightroom, dans crs:MaskGroupBasedCorrections, ne
    // sont pas encore lus).
    if let Some(json) = values.meta.get("luma:Masks")
        && let Ok(masks) = serde_json::from_str(json)
    {
        s.masks = masks;
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
    Ok(s.clamped())
}

/// Les réglages qu'un préréglage n'emporte pas : ceux de la photo (balance des blancs,
/// exposition, recadrage, objectif, perspective, détail, calibration).
fn photo_only(name: &str) -> bool {
    const KEYS: [&str; 12] = [
        "WhiteBalance",
        "Temperature",
        "Tint",
        "Exposure2012",
        "HasCrop",
        "AutoLateralCA",
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

/// Un préréglage au format de Lightroom Classic et Camera Raw (un `.xmp` qu'on importe
/// dans le panneau Préréglages) : le look de `s` seulement, sous `name`, rangé dans le
/// groupe `group`.
pub fn write_preset(name: &str, group: &str, s: &Settings) -> String {
    let full = write(s);
    let mut out = String::new();
    // Un identifiant stable, tiré du nom (Lightroom en veut un par préréglage).
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
        if t.starts_with("xmp:Rating") || t.starts_with("luma:Masks") || t.starts_with("luma:Film") {
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

/// Un préréglage `.xmp` (de LÜMA, de Lightroom ou de Camera Raw) : son nom (ou `None`)
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

/// Un fichier XMP complet (sidecar), lisible par Lightroom et Camera Raw.
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

    if s.profile == Profile::Monochrome {
        for (i, name) in XMP_COLORS.iter().enumerate() {
            attrs.push((format!("GrayMixer{name}"), fmt_value("GrayMixer", s.hsl.luminance[i])));
        }
    }
    let mut out = String::from(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"LÜMA\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  <rdf:Description rdf:about=\"\"\n",
    );
    out.push_str(&format!("    xmlns:crs=\"{CRS}\"\n"));
    out.push_str("    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n");
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
    if !s.masks.is_empty() {
        let json = serde_json::to_string(&s.masks).unwrap_or_default();
        out.push_str(&format!("    luma:Masks=\"{}\"\n", escape(&json)));
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
    out.push_str("  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n");
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
    use super::*;

    /// Un extrait de sidecar tel que l'écrit Lightroom Classic.
    const LIGHTROOM: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core 7.0-c000">
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
    fn reads_a_lightroom_sidecar() {
        let s = read(LIGHTROOM).unwrap();
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
        let mut s = read(LIGHTROOM).unwrap();
        s.hsl.hue[7] = 12.0;
        s.tone_curve.red = vec![(0.0, 0.0), (64.0, 70.0), (255.0, 255.0)];
        let text = write(&s);
        assert!(text.contains("crs:Exposure2012=\"+0.65\""));
        assert!(text.contains("crs:HueAdjustmentMagenta=\"+12\""));
        assert_eq!(read(&text).unwrap(), s);
    }

    #[test]
    fn masks_round_trip() {
        let mut s = read(LIGHTROOM).unwrap();
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
        s.masks = vec![sky, brush];
        s.lens.remove_ca = true;
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
        assert!(text.contains("luma:Masks="));
        assert!(text.contains("luma:LutAmount=\"70\""));
        assert_eq!(read(&text).unwrap(), s);
    }

    #[test]
    fn presets_are_written_like_lightroom_and_read_back() {
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
    fn black_and_white_mix_round_trips_as_lightroom_gray_mixer() {
        let mut s = Settings {
            profile: Profile::Monochrome,
            ..Settings::default()
        };
        s.hsl.luminance[0] = 40.0;
        s.hsl.luminance[5] = -60.0;
        let text = write(&s);
        assert!(text.contains("crs:GrayMixerRed=\"+40\""), "{text}");
        assert!(text.contains("crs:GrayMixerBlue=\"-60\""));
        let lr = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/" crs:ConvertToGrayscale="True" crs:GrayMixerRed="-30" crs:GrayMixerGreen="+25"/></rdf:RDF></x:xmpmeta>"#;
        let back = read(lr).unwrap();
        assert_eq!(back.profile, Profile::Monochrome);
        assert_eq!((back.hsl.luminance[0], back.hsl.luminance[3]), (-30.0, 25.0));
    }

    #[test]
    fn rating_flag_and_label() {
        let s = read(LIGHTROOM).unwrap();
        assert_eq!(read_meta(LIGHTROOM), Meta::default());
        for meta in [
            Meta {
                rating: 4,
                flag: Flag::Pick,
                label: "Rouge".into(),
            },
            Meta {
                rating: 0,
                flag: Flag::Reject,
                label: String::new(),
            },
            Meta {
                rating: 5,
                flag: Flag::None,
                label: String::new(),
            },
        ] {
            let text = write_with_meta(&s, &meta);
            assert_eq!(read_meta(&text), meta);
            assert_eq!(read(&text).unwrap(), s);
        }
        let bridge = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmp:Rating="3" xmp:Label="Vert"/></rdf:RDF></x:xmpmeta>"#;
        assert_eq!(
            read_meta(bridge),
            Meta {
                rating: 3,
                flag: Flag::None,
                label: "Vert".into()
            }
        );
    }

    #[test]
    fn rejects_documents_without_settings() {
        assert!(read("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>").is_err());
        assert!(read("<pas du xml").is_err());
        assert_eq!(color_index("Aqua"), Some(4));
    }
}
