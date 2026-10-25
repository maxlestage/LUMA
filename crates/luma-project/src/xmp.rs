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

const META_KEYS: [&str; 3] = ["xmp:Rating", "xmp:Label", "luma:Pick"];
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
    if values.scalars.is_empty() && values.lists.is_empty() {
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
        }
    }
    if values
        .scalars
        .get("ConvertToGrayscale")
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
    {
        s.profile = Profile::Monochrome;
    }
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
    Ok(s.clamped())
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
