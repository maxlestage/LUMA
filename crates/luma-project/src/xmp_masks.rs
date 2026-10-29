//! Les masques écrits dans un XMP par un autre logiciel de développement, en
//! `crs:MaskGroupBasedCorrections` (et les listes plus anciennes
//! `crs:GradientBasedCorrections`, `crs:CircularGradientBasedCorrections`) : chaque
//! correction, ses réglages locaux et ses formes.
//!
//! Lus : les dégradés linéaires (`Mask/Gradient`) et radiaux (`Mask/CircularGradient`),
//! l'intensité, l'inversion, et les réglages que LÜMA a aussi (exposition, contraste,
//! hautes lumières, ombres, blancs, noirs, température, teinte, saturation, clarté,
//! texture, netteté, bruit, voile, moiré, frange). Les autres formes (pinceau, sujet,
//! ciel, plages), faites de pixels ou de calculs propres à ce logiciel, sont laissées :
//! une correction sans forme lue n'est pas reprise.
//!
//! Les coordonnées du XMP se rapportent à l'image entière (0 à 1) ; celles de LÜMA, à
//! l'image recadrée : elles sont ramenées dans le cadre (sans sa rotation).

use quick_xml::XmlVersion;
use quick_xml::events::Event;

use crate::masks::{Local, Mask, Shape};
use crate::{Crop, Part};

/// Les listes de corrections lues.
const LISTS: [&str; 3] = [
    "crs:MaskGroupBasedCorrections",
    "crs:GradientBasedCorrections",
    "crs:CircularGradientBasedCorrections",
];

/// Une correction en cours de lecture : ses attributs, ses formes.
#[derive(Default)]
struct Correction {
    values: Vec<(String, String)>,
    shapes: Vec<(Shape, bool)>,
    /// La liste d'où elle vient (les anciennes inversent le radial autrement).
    legacy: bool,
}

impl Correction {
    fn get(&self, name: &str) -> Option<f32> {
        self.values
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.trim().trim_start_matches('+').parse().ok())
    }
}

/// Les masques d'un XMP, dans le cadre `crop` de LÜMA ; vide s'il n'y en a pas.
pub fn read(text: &str, crop: &Crop) -> Vec<Mask> {
    let mut reader = quick_xml::Reader::from_str(text);
    let mut list: Option<(String, usize)> = None;
    let mut depth = 0usize;
    let mut current: Option<Correction> = None;
    let mut out = Vec::new();
    // Un XMP mal formé s'arrête là : les masques lus jusque‑là restent.
    while let Ok(event) = reader.read_event() {
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(event, Event::Empty(_));
                let qname = e.name();
                let tag: &str = qname.as_ref();
                if list.is_none() && LISTS.contains(&tag) && !empty {
                    list = Some((tag.to_owned(), depth));
                }
                if let Some((name, _)) = &list {
                    let attrs: Vec<(String, String)> = e
                        .attributes()
                        .flatten()
                        .filter_map(|a| {
                            let key: &str = a.key.as_ref();
                            let key = key.strip_prefix("crs:")?.to_owned();
                            let value = a.normalized_value(XmlVersion::Implicit1_0).ok()?.into_owned();
                            Some((key, value))
                        })
                        .collect();
                    let what = attrs.iter().find(|(k, _)| k == "What").map(|(_, v)| v.as_str());
                    match what {
                        Some("Correction") => {
                            if let Some(c) = current.take() {
                                out.extend(mask_of(&c, crop));
                            }
                            current = Some(Correction {
                                values: attrs,
                                shapes: Vec::new(),
                                legacy: name != LISTS[0],
                            });
                        }
                        Some(kind) if kind.starts_with("Mask/") => {
                            if let Some(c) = &mut current
                                && let Some(shape) = shape_of(kind, &attrs, c.legacy, crop)
                            {
                                c.shapes.push(shape);
                            }
                        }
                        _ => {}
                    }
                }
                if !empty {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                if list.as_ref().is_some_and(|(_, d)| *d == depth) {
                    list = None;
                    if let Some(c) = current.take() {
                        out.extend(mask_of(&c, crop));
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    out
}

/// Un nombre d'une forme.
fn number(attrs: &[(String, String)], name: &str) -> Option<f32> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .and_then(|(_, v)| v.trim().trim_start_matches('+').parse().ok())
}

fn yes(attrs: &[(String, String)], name: &str) -> bool {
    attrs
        .iter()
        .any(|(k, v)| k == name && v.trim().eq_ignore_ascii_case("true"))
}

/// Un point de l'image entière, dans le cadre de LÜMA.
fn in_crop(crop: &Crop, (x, y): (f32, f32)) -> (f32, f32) {
    let w = (crop.right - crop.left).max(1e-6);
    let h = (crop.bottom - crop.top).max(1e-6);
    ((x - crop.left) / w, (y - crop.top) / h)
}

/// Une forme lue, et si elle est inversée.
fn shape_of(kind: &str, a: &[(String, String)], legacy: bool, crop: &Crop) -> Option<(Shape, bool)> {
    if a.iter()
        .any(|(k, v)| k == "MaskActive" && v.trim().eq_ignore_ascii_case("false"))
    {
        return None;
    }
    let inverted = yes(a, "MaskInverted");
    match kind {
        "Mask/Gradient" => {
            // Plein effet au point « Full », plus rien au point « Zero ».
            let full = (number(a, "FullX")?, number(a, "FullY")?);
            let zero = (number(a, "ZeroX")?, number(a, "ZeroY")?);
            let shape = Shape::Linear {
                start: in_crop(crop, full),
                end: in_crop(crop, zero),
            };
            Some((shape, inverted))
        }
        "Mask/CircularGradient" => {
            let (top, left) = (number(a, "Top")?, number(a, "Left")?);
            let (bottom, right) = (number(a, "Bottom")?, number(a, "Right")?);
            let (w, h) = ((crop.right - crop.left).max(1e-6), (crop.bottom - crop.top).max(1e-6));
            let shape = Shape::Radial {
                center: in_crop(crop, ((left + right) / 2.0, (top + bottom) / 2.0)),
                radius: (((right - left) / 2.0).abs() / w, ((bottom - top) / 2.0).abs() / h),
                angle: number(a, "Angle").unwrap_or(0.0),
                feather: number(a, "Feather").unwrap_or(50.0).clamp(0.0, 100.0),
            };
            // Les anciens dégradés radiaux agissent hors de l'ellipse, sauf « Flipped ».
            let outside = legacy && !yes(a, "Flipped");
            Some((shape, inverted != outside))
        }
        _ => None,
    }
}

/// Le masque d'une correction : sa première forme, les autres ajoutées ; ses réglages.
fn mask_of(c: &Correction, _crop: &Crop) -> Option<Mask> {
    if c.values
        .iter()
        .any(|(k, v)| k == "CorrectionActive" && v.trim().eq_ignore_ascii_case("false"))
    {
        return None;
    }
    let ((shape, invert), rest) = c.shapes.split_first()?;
    // Les réglages du XMP vont de −1 à 1 (l'exposition de −4 à 4 IL, en quarts) ; ceux
    // de LÜMA de −100 à 100, l'exposition en IL.
    let k = |name: &str| c.get(name).unwrap_or(0.0) * 100.0;
    let local = Local {
        exposure: c.get("LocalExposure2012").unwrap_or(0.0) * 4.0,
        contrast: k("LocalContrast2012"),
        highlights: k("LocalHighlights2012"),
        shadows: k("LocalShadows2012"),
        whites: k("LocalWhites2012"),
        blacks: k("LocalBlacks2012"),
        temperature: k("LocalTemperature"),
        tint: k("LocalTint"),
        saturation: k("LocalSaturation"),
        clarity: k("LocalClarity2012"),
        texture: k("LocalTexture"),
        sharpness: k("LocalSharpness"),
        noise: k("LocalLuminanceNoise").max(0.0),
        dehaze: k("LocalDehaze"),
        moire: k("LocalMoire").max(0.0),
        defringe: k("LocalDefringe").max(0.0),
    };
    let name = c
        .values
        .iter()
        .find(|(k, _)| k == "CorrectionName")
        .map(|(_, v)| v.clone())
        .filter(|n| !n.trim().is_empty());
    let mut mask = match shape {
        Shape::Linear { .. } => Mask::linear(),
        _ => Mask::radial(),
    };
    mask.shape = shape.clone();
    mask.invert = *invert;
    mask.amount = (c.get("CorrectionAmount").unwrap_or(1.0) * 100.0).clamp(0.0, 100.0);
    mask.local = local;
    if let Some(n) = name {
        mask.name = n;
    }
    mask.parts = rest
        .iter()
        .map(|(shape, invert)| Part {
            combine: crate::masks::Combine::Add,
            shape: shape.clone(),
            invert: *invert,
        })
        .collect();
    Some(mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XMP: &str = r#"<x:xmpmeta xmlns:x="x">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:crs="c"
   crs:Exposure2012="+0.30">
   <crs:MaskGroupBasedCorrections>
    <rdf:Seq>
     <rdf:li>
      <rdf:Description crs:What="Correction" crs:CorrectionAmount="0.8"
       crs:CorrectionActive="true" crs:CorrectionName="Ciel"
       crs:LocalExposure2012="-0.25" crs:LocalHighlights2012="-0.4"
       crs:LocalSaturation="0.2" crs:LocalClarity2012="0.1">
       <crs:CorrectionMasks>
        <rdf:Seq>
         <rdf:li>
          <rdf:Description crs:What="Mask/Gradient" crs:MaskActive="true"
           crs:MaskInverted="false" crs:ZeroX="0.5" crs:ZeroY="0.6"
           crs:FullX="0.5" crs:FullY="0.1"/>
         </rdf:li>
        </rdf:Seq>
       </crs:CorrectionMasks>
      </rdf:Description>
     </rdf:li>
     <rdf:li>
      <rdf:Description crs:What="Correction" crs:CorrectionAmount="1"
       crs:LocalExposure2012="0.125" crs:LocalShadows2012="0.3">
       <crs:CorrectionMasks>
        <rdf:Seq>
         <rdf:li>
          <rdf:Description crs:What="Mask/CircularGradient" crs:MaskInverted="true"
           crs:Top="0.2" crs:Left="0.3" crs:Bottom="0.8" crs:Right="0.7"
           crs:Angle="15" crs:Feather="70"/>
         </rdf:li>
         <rdf:li>
          <rdf:Description crs:What="Mask/Paint" crs:MaskValue="1"/>
         </rdf:li>
        </rdf:Seq>
       </crs:CorrectionMasks>
      </rdf:Description>
     </rdf:li>
     <rdf:li>
      <rdf:Description crs:What="Correction" crs:LocalExposure2012="0.5">
       <crs:CorrectionMasks>
        <rdf:Seq>
         <rdf:li>
          <rdf:Description crs:What="Mask/Image" crs:MaskSubType="1"/>
         </rdf:li>
        </rdf:Seq>
       </crs:CorrectionMasks>
      </rdf:Description>
     </rdf:li>
    </rdf:Seq>
   </crs:MaskGroupBasedCorrections>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5
    }

    #[test]
    fn gradients_and_their_settings_are_read() {
        let masks = read(XMP, &Crop::default());
        // Le masque de sujet (des pixels) n'est pas repris.
        assert_eq!(masks.len(), 2);
        let sky = &masks[0];
        assert_eq!(sky.name, "Ciel");
        assert!((sky.amount - 80.0).abs() < 1e-4);
        assert!((sky.local.exposure + 1.0).abs() < 1e-5, "−0,25 : −1 IL");
        assert!((sky.local.highlights + 40.0).abs() < 1e-4);
        assert!((sky.local.saturation - 20.0).abs() < 1e-4);
        assert!((sky.local.clarity - 10.0).abs() < 1e-4);
        let Shape::Linear { start, end } = sky.shape else {
            panic!("{:?}", sky.shape)
        };
        // Plein effet en haut (Full), plus rien vers le milieu (Zero).
        assert!(close(start, (0.5, 0.1)) && close(end, (0.5, 0.6)));
        let radial = &masks[1];
        assert!(radial.invert);
        assert!((radial.local.exposure - 0.5).abs() < 1e-5);
        assert!((radial.local.shadows - 30.0).abs() < 1e-4);
        let Shape::Radial {
            center,
            radius,
            angle,
            feather,
        } = radial.shape
        else {
            panic!("{:?}", radial.shape)
        };
        assert!(close(center, (0.5, 0.5)) && close(radius, (0.2, 0.3)));
        assert!((angle - 15.0).abs() < 1e-5 && (feather - 70.0).abs() < 1e-5);
        // Le pinceau qui l'accompagne n'est pas lu : le radial seul.
        assert!(radial.parts.is_empty());
    }

    /// Par la lecture du XMP : les réglages et les masques ; écrits par LÜMA, relus
    /// tels quels (en `luma:Masks`).
    #[test]
    fn a_whole_xmp_brings_its_masks() {
        let s = crate::xmp::read(XMP).unwrap();
        assert!((s.exposure - 0.3).abs() < 1e-5);
        assert_eq!(s.masks.len(), 2);
        let back = crate::xmp::read(&crate::xmp::write(&s)).unwrap();
        assert_eq!(back.masks, s.masks);
    }

    #[test]
    fn shapes_are_brought_into_the_crop() {
        let crop = Crop {
            left: 0.2,
            top: 0.1,
            right: 0.7,
            bottom: 0.6,
            angle: 0.0,
        };
        let masks = read(XMP, &crop);
        let Shape::Linear { start, .. } = masks[0].shape else {
            panic!()
        };
        // (0,5 ; 0,1) de l'image entière : (0,6 ; 0) du cadre.
        assert!(close(start, (0.6, 0.0)), "{start:?}");
        let Shape::Radial { center, radius, .. } = masks[1].shape else {
            panic!()
        };
        assert!(
            close(center, (0.6, 0.8)) && close(radius, (0.4, 0.6)),
            "{center:?} {radius:?}"
        );
    }

    #[test]
    fn older_radial_gradients_act_outside_unless_flipped() {
        let old = |flipped: &str| {
            format!(
                r#"<rdf:Description xmlns:crs="c" xmlns:rdf="r">
                 <crs:CircularGradientBasedCorrections><rdf:Seq><rdf:li>
                  <rdf:Description crs:What="Correction" crs:LocalExposure2012="0.25">
                   <crs:CorrectionMasks><rdf:Seq><rdf:li>
                    <rdf:Description crs:What="Mask/CircularGradient" crs:Top="0.2"
                     crs:Left="0.2" crs:Bottom="0.8" crs:Right="0.8" crs:Flipped="{flipped}"/>
                   </rdf:li></rdf:Seq></crs:CorrectionMasks>
                  </rdf:Description>
                 </rdf:li></rdf:Seq></crs:CircularGradientBasedCorrections>
                </rdf:Description>"#
            )
        };
        assert!(read(&old("false"), &Crop::default())[0].invert);
        assert!(!read(&old("true"), &Crop::default())[0].invert);
        // Rien à lire : aucun masque.
        assert!(read("<x:xmpmeta/>", &Crop::default()).is_empty());
    }
}
