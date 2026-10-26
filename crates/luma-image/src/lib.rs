//! Le moteur de développement de LÜMA : d'une image caméra linéaire à une image prête
//! pour l'écran, en rejouant les réglages (jamais appliqués aux originaux).
//!
//! Ordre des étapes :
//!
//! 1. **entrée** : matrice caméra → espace de travail (balance des blancs comprise),
//!    étalonnage de l'appareil, exposition, hautes lumières saturées neutralisées ;
//! 2. **bruit** : réduction du bruit de luminance et de couleur ;
//! 3. **tonalité locale** : Correction du voile, Hautes lumières, Ombres, Clarté,
//!    Texture ; puis les retouches locales du panneau Masquage ;
//! 4. **rendu** : courbe filmique (Contraste, Blancs, Noirs) qui préserve les teintes
//!    et blanchit doucement les hautes lumières, puis courbe des tonalités ;
//! 5. **couleur** : Vibrance, Saturation, Mélangeur, Étalonnage des couleurs ;
//! 6. **accentuation** ;
//! 7. **effets** : vignetage, grain.
//!
//! Tout est calculé en `f32`, en parallèle (rayon). Le résultat est en Rec. 2020
//! linéaire ; [`encode8`] et [`encode16`] le convertissent vers l'espace de sortie.

pub mod color;
pub mod cube;
pub mod curve;
pub mod detail;
pub mod effects;
pub mod geometry;
pub mod lens;
pub mod local;
pub mod tone;

use luma_color::{self as lc, EncodeLut, Mat3, OutputSpace, luminance};
use luma_core::{Cancel, Result, RgbImage, Timings};
use luma_project::Settings;
use rayon::prelude::*;
use std::sync::Arc;

use crate::curve::{Filmic, Lut};

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Ce que le développement doit savoir de l'image d'entrée.
#[derive(Clone, Copy, Debug)]
pub struct Input {
    /// Caméra → Rec. 2020 linéaire, pour la balance des blancs choisie.
    pub camera_to_working: Mat3,
    /// Exposition prévue par le fichier (DNG), ajoutée au curseur.
    pub exposure_bias: f32,
    /// Pixels rendus par pixel de l'image pleine définition (1 pour un export).
    pub scale: f32,
    /// Une image déjà développée (JPEG…) : ses couleurs vives proches du blanc sont de
    /// vraies couleurs, et non un canal saturé au capteur.
    pub rendered: bool,
}

/// Développe `cam` (RGB caméra linéaire, déjà orientée et recadrée).
pub fn develop(cam: &RgbImage, input: &Input, s: &Settings, cancel: &Cancel, t: &mut Timings) -> Result<RgbImage> {
    let base = develop_base(cam, input, s, cancel, t)?;
    finish_layers(
        &base,
        input.scale,
        s,
        &mut |look, t| develop_base(cam, input, look, cancel, t).map(Arc::new),
        cancel,
        t,
    )
}

/// Les étapes de voisinage du début (entrée, bruit, tonalité locale) : leur résultat
/// ne dépend que des réglages gardés par [`base_key`], et peut être gardé en cache
/// pendant qu'on règle le contraste, les couleurs ou les effets.
pub fn develop_base(cam: &RgbImage, input: &Input, s: &Settings, cancel: &Cancel, t: &mut Timings) -> Result<RgbImage> {
    // L'étape d'entrée lit la source (en cache, partagée) et écrit l'image de travail.
    let mut img = t.time("entrée", || input_stage(cam, input, s));
    cancel.check()?;
    t.time("défrangeage", || lens::defringe(&mut img, &s.lens, input.scale));
    t.time("bruit", || detail::noise_reduction(&mut img, &s.detail, input.scale));
    cancel.check()?;
    t.time("tonalité locale", || {
        tone::dehaze(&mut img, s.dehaze);
        tone::local_tone(&mut img, s);
    });
    cancel.check()?;
    if !s.masks.is_empty() {
        t.time("masques", || local::apply(&mut img, &s.masks));
        cancel.check()?;
    }
    Ok(img)
}

/// Les réglages dont dépend [`develop_base`] ; les autres sont remis à leur défaut.
/// Deux réglages de même clé donnent la même base.
pub fn base_key(s: &Settings) -> Settings {
    let d = Settings::default();
    let mut detail = d.detail.clone();
    detail.luminance_nr = s.detail.luminance_nr;
    detail.luminance_nr_detail = s.detail.luminance_nr_detail;
    detail.luminance_nr_contrast = s.detail.luminance_nr_contrast;
    detail.color_nr = s.detail.color_nr;
    detail.color_nr_detail = s.detail.color_nr_detail;
    detail.color_nr_smoothness = s.detail.color_nr_smoothness;
    Settings {
        white_balance: s.white_balance,
        exposure: s.exposure,
        calibration: s.calibration.clone(),
        detail,
        dehaze: s.dehaze,
        highlights: s.highlights,
        shadows: s.shadows,
        clarity: s.clarity,
        texture: s.texture,
        crop: s.crop,
        lens: s.lens.clone(),
        transform: s.transform.clone(),
        // Les films des masques n'agissent qu'à la fin : ils ne changent pas la base.
        masks: s
            .masks
            .iter()
            .map(|m| luma_project::Mask {
                look: None,
                ..m.clone()
            })
            .filter(|m| m.is_active())
            .collect(),
        ..d
    }
}

/// Les étapes par pixel de la fin (rendu, couleur, effets) et l'accentuation. Les
/// étapes par pixel ont un double sur le GPU (`luma-gpu`).
pub fn finish(img: &mut RgbImage, scale: f32, s: &Settings, cancel: &Cancel, t: &mut Timings) -> Result<()> {
    t.time("rendu", || render_stage(img, s));
    cancel.check()?;
    t.time("couleur", || {
        if let Some(stage) = color::ColorStage::new(s) {
            img.data.par_iter_mut().for_each(|p| *p = stage.apply(*p));
        }
    });
    cancel.check()?;
    t.time("film mesuré", || cube::apply(img, s));
    t.time("accentuation", || detail::sharpen(img, &s.detail, scale));
    t.time("effets", || effects::apply(img, &s.effects, scale));
    Ok(())
}

/// Les films empilés par les masques : pour chacun, le masque, les réglages complets
/// avec ce film, et l'intensité du film (0 à 1).
pub fn look_layers(s: &Settings) -> Vec<(&luma_project::Mask, Settings, f32)> {
    s.masks
        .iter()
        .filter_map(|m| {
            let (name, k) = m.active_look()?;
            // Un film intégré, ou un film mesuré (LUT chargée).
            let look = match luma_project::presets::find(name) {
                Some(preset) => preset.applied(s, 1.0),
                None => {
                    cube::get(name)?;
                    luma_project::presets::with_lut(s, name, 1.0)
                }
            };
            Some((m, look, k))
        })
        .collect()
}

/// Le film d'un masque a‑t‑il besoin de sa propre base ? Oui s'il change un réglage
/// des étapes de voisinage (hautes lumières, ombres, clarté, texture, correction du
/// voile) : il est alors complet dans sa zone, comme posé sur toute la photo.
pub fn needs_own_base(s: &Settings, look: &Settings) -> bool {
    base_key(look) != base_key(s)
}

/// [`finish`], puis les films des masques, fondus dans l'ordre des masques selon leur
/// zone et l'intensité du film. Chaque film part de la même base, sauf s'il touche aux
/// étapes de voisinage ([`needs_own_base`]) : `base_for` donne alors la base de ses
/// réglages (développée, ou prise dans un cache).
pub fn finish_layers(
    base: &RgbImage,
    scale: f32,
    s: &Settings,
    base_for: &mut dyn FnMut(&Settings, &mut Timings) -> Result<Arc<RgbImage>>,
    cancel: &Cancel,
    t: &mut Timings,
) -> Result<RgbImage> {
    let mut out = base.clone();
    finish(&mut out, scale, s, cancel, t)?;
    for (mask, look, k) in look_layers(s) {
        let mut alt = if needs_own_base(s, &look) {
            let own = base_for(&look, t)?;
            // Même recadrage, même taille : sinon (ne devrait pas arriver), la base commune.
            if (own.width, own.height) == (base.width, base.height) {
                Arc::unwrap_or_clone(own)
            } else {
                base.clone()
            }
        } else {
            base.clone()
        };
        cancel.check()?;
        finish(&mut alt, scale, &look, cancel, t)?;
        let weights = t.time("films des masques", || local::weights(mask, out.width, out.height));
        out.data
            .par_iter_mut()
            .zip(alt.data.par_iter().zip(weights.data.par_iter()))
            .for_each(|(o, (a, &w))| {
                let w = w * k;
                if w > 0.0 {
                    *o = [
                        o[0] + (a[0] - o[0]) * w,
                        o[1] + (a[1] - o[1]) * w,
                        o[2] + (a[2] - o[2]) * w,
                    ];
                }
            });
    }
    Ok(out)
}

/// Les tables de l'étape de rendu, pour le GPU : la courbe filmique (8193 valeurs,
/// indexées par la racine quatrième de x / 4) et, si elles ne sont pas neutres, les
/// trois courbes des tonalités (4097 valeurs chacune, en sRGB encodé).
pub fn render_tables(s: &Settings) -> (Vec<f32>, Option<[Vec<f32>; 3]>) {
    let filmic = tone_table(s);
    (
        filmic.values().to_vec(),
        tone_luts(s).map(|l| l.map(|lut| lut.values().to_vec())),
    )
}

fn input_stage(cam: &RgbImage, input: &Input, s: &Settings) -> RgbImage {
    let gain = (s.exposure + input.exposure_bias).exp2();
    let m = lc::mul(&color::calibration_matrix(&s.calibration), &input.camera_to_working).map(|r| r.map(|v| v * gain));
    let tint = s.calibration.shadows_tint / 100.0;
    let mut img = RgbImage {
        width: cam.width,
        height: cam.height,
        data: vec![[0.0; 3]; cam.data.len()],
    };
    img.data.par_iter_mut().zip(&cam.data).for_each(|(p, src)| {
        let src = *src;
        let raw_max = src[0].max(src[1]).max(src[2]);
        let mut c = lc::apply(&m, src);
        // Un canal saturé au capteur donne une fausse teinte (le rose des ciels brûlés) :
        // on fond ces pixels vers un neutre de même luminance.
        let clip = if input.rendered {
            0.0
        } else {
            smoothstep(0.88, 1.0, raw_max)
        };
        if clip > 0.0 {
            let y = luminance(c).max(0.0);
            c = [
                c[0] + (y - c[0]) * clip,
                c[1] + (y - c[1]) * clip,
                c[2] + (y - c[2]) * clip,
            ];
        }
        if tint != 0.0 {
            let w = 1.0 - smoothstep(0.0, 0.2, luminance(c));
            c[1] *= 1.0 - tint * 0.25 * w;
        }
        *p = c;
    });
    img
}

/// Les courbes du panneau Courbe des tonalités, une par couche (`None` si neutres).
fn tone_luts(s: &Settings) -> Option<[Lut; 3]> {
    let c = &s.tone_curve;
    let master = curve::parametric(c).then(&curve::spline(&c.point));
    let luts = [&c.red, &c.green, &c.blue].map(|ch| master.then(&curve::spline(ch)));
    (!luts.iter().all(Lut::is_identity)).then_some(luts)
}

/// La courbe de rendu : filmique pour un RAW, celle de l'image elle‑même (profil
/// Original) pour une image déjà développée.
pub fn tone_table(s: &Settings) -> curve::FilmicTable {
    if s.profile == luma_project::Profile::Original {
        curve::display_table(s.contrast, s.whites, s.blacks)
    } else {
        Filmic::with_look(s.contrast, s.whites, s.blacks, s.profile.look().0).table()
    }
}

/// La part de désaturation des hautes lumières (comme un film) : aucune pour le profil
/// Original, qui garde les couleurs de l'image.
pub fn highlight_rolloff(s: &Settings) -> f32 {
    if s.profile == luma_project::Profile::Original {
        0.0
    } else {
        0.85
    }
}

fn render_stage(img: &mut RgbImage, s: &Settings) {
    let filmic = tone_table(s);
    let rolloff = highlight_rolloff(s);
    let luts = tone_luts(s);
    let encode = EncodeLut::new(lc::srgb_encode);
    let decode = Lut::from_fn(lc::srgb_decode);
    img.data.par_iter_mut().for_each(|p| {
        // Norme entre la luminance et le maximum : les teintes restent, les couleurs
        // vives ne s'écrasent pas.
        let mx = p[0].max(p[1]).max(p[2]);
        let n = (0.5 * mx + 0.5 * luminance(*p)).max(1e-7);
        let f = filmic.get(n);
        let r = f / n;
        let mut c = [p[0] * r, p[1] * r, p[2] * r];
        // Vers le blanc : les hautes lumières se désaturent comme sur un film.
        let t = smoothstep(0.72, 1.0, f) * rolloff;
        c = [c[0] + (f - c[0]) * t, c[1] + (f - c[1]) * t, c[2] + (f - c[2]) * t];
        // Aucun canal au‑dessus de 1 : on rapproche du gris plutôt que d'écrêter.
        let over = c[0].max(c[1]).max(c[2]);
        if over > 1.0 && over > f {
            let k = (1.0 - f) / (over - f);
            c = [f + (c[0] - f) * k, f + (c[1] - f) * k, f + (c[2] - f) * k];
        }
        if let Some(luts) = &luts {
            for (v, lut) in c.iter_mut().zip(luts) {
                if *v >= 0.0 {
                    *v = decode.get(lut.get(encode.get(*v)));
                }
            }
        }
        *p = c;
    });
}

/// Un pixel converti vers l'espace de sortie, linéaire, dans le gamut (0 à 1).
#[inline]
pub(crate) fn to_output(m: &Mat3, p: [f32; 3]) -> [f32; 3] {
    let y = luminance(p).clamp(0.0, 1.0);
    let mut c = lc::apply(m, p);
    // Hors gamut : on rapproche du gris de même luminance jusqu'à y rentrer, au lieu
    // d'écrêter chaque canal (qui tordrait la teinte).
    let lo = c[0].min(c[1]).min(c[2]);
    if lo < 0.0 && y > lo {
        let k = y / (y - lo);
        c = c.map(|v| y + (v - y) * k);
    }
    let hi = c[0].max(c[1]).max(c[2]);
    if hi > 1.0 && hi > y {
        let k = (1.0 - y) / (hi - y);
        c = c.map(|v| y + (v - y) * k);
    }
    c.map(|v| v.clamp(0.0, 1.0))
}

/// L'image encodée sur 8 bits (RGB entrelacé), prête pour un JPEG ou l'écran.
pub fn encode8(img: &RgbImage, space: OutputSpace) -> Vec<u8> {
    let lut = EncodeLut::new(|v| space.encode(v));
    let m = lc::rgb_to_rgb(&lc::WORKING, &space.primaries());
    let mut out = vec![0u8; img.data.len() * 3];
    out.par_chunks_mut(3 * img.width.max(1))
        .zip(img.data.par_chunks(img.width.max(1)))
        .for_each(|(dst, src)| {
            for (d, &p) in dst.as_chunks_mut::<3>().0.iter_mut().zip(src) {
                let c = to_output(&m, p);
                for i in 0..3 {
                    d[i] = (lut.get(c[i]) * 255.0 + 0.5) as u8;
                }
            }
        });
    out
}

/// L'image encodée sur 16 bits (TIFF, PNG).
pub fn encode16(img: &RgbImage, space: OutputSpace) -> Vec<u16> {
    let m = lc::rgb_to_rgb(&lc::WORKING, &space.primaries());
    let mut out = vec![0u16; img.data.len() * 3];
    out.par_chunks_mut(3 * img.width.max(1))
        .zip(img.data.par_chunks(img.width.max(1)))
        .for_each(|(dst, src)| {
            for (d, &p) in dst.as_chunks_mut::<3>().0.iter_mut().zip(src) {
                let c = to_output(&m, p);
                for i in 0..3 {
                    d[i] = (space.encode(c[i]) * 65535.0 + 0.5) as u16;
                }
            }
        });
    out
}

/// L'histogramme RGB (256 classes) de l'image encodée, celui qu'affiche le panneau.
pub fn histogram(encoded: &[u8]) -> [[u32; 256]; 3] {
    let mut h = [[0u32; 256]; 3];
    for px in encoded.as_chunks::<3>().0 {
        for c in 0..3 {
            h[c][px[c] as usize] += 1;
        }
    }
    h
}

#[cfg(test)]
mod tests {

    #[test]
    fn mask_films_stack_where_their_masks_are() {
        use luma_project::{Look, Mask, Shape};
        let cam = RgbImage::from_fn(40, 30, |x, y| [0.02 + x as f32 * 0.012, 0.12 - y as f32 * 0.003, 0.06]);
        let mut t = Timings::default();
        let cancel = Cancel::new();
        let with_film = |name: &str, mut mask: Mask| {
            let mut s = Settings::default();
            mask.look = Some(Look {
                name: name.into(),
                amount: 100.0,
            });
            s.masks.push(mask);
            s
        };

        // Un film sur toute l'image à 100 % = ce film appliqué globalement, y compris ses
        // hautes lumières, ses ombres et sa clarté (Tri-X : clarté ; Haute clé : ombres).
        for name in ["Tri-X 400", "Haute clé", "Portra 400"] {
            let film = luma_project::presets::find(name).unwrap();
            let s = with_film(name, Mask::full());
            let layered = develop(&cam, &gray_input(), &s, &cancel, &mut t).unwrap();
            let global = film.applied(&Settings::default(), 1.0);
            let direct = develop(&cam, &gray_input(), &global, &cancel, &mut t).unwrap();
            for (a, b) in layered.data.iter().zip(&direct.data) {
                for c in 0..3 {
                    assert!((a[c] - b[c]).abs() < 1e-5, "{name}");
                }
            }
            assert!(needs_own_base(&s, &global), "{name}");
        }
        // Les ombres de Haute clé agissent bien dans la zone : le film rendu depuis la
        // base commune (sans ses ombres) est plus sombre.
        let s = with_film("Haute clé", Mask::full());
        let base = develop_base(&cam, &gray_input(), &s, &cancel, &mut t).unwrap();
        let mut shared = base.clone();
        let global = luma_project::presets::find("Haute clé")
            .unwrap()
            .applied(&Settings::default(), 1.0);
        finish(&mut shared, 1.0, &global, &cancel, &mut t).unwrap();
        let layered = develop(&cam, &gray_input(), &s, &cancel, &mut t).unwrap();
        let lum = |img: &RgbImage| img.data.iter().map(|p| luminance(*p)).sum::<f32>();
        assert!(lum(&layered) > lum(&shared) * 1.02);

        // Dans un cercle seulement : le centre est en noir et blanc, le coin en couleur.
        let mut radial = Mask::radial();
        radial.shape = Shape::Radial {
            center: (0.5, 0.5),
            radius: (0.2, 0.2),
            angle: 0.0,
            feather: 0.0,
        };
        let s = with_film("Tri-X 400", radial);
        let out = develop(&cam, &gray_input(), &s, &cancel, &mut t).unwrap();
        let center = out.data[15 * 40 + 20];
        let corner = out.data[0];
        assert!(
            (center[0] - center[1]).abs() < 1e-3 && (center[1] - center[2]).abs() < 1e-3,
            "{center:?}"
        );
        assert!((corner[1] - corner[0]).abs() > 0.01, "{corner:?}");
        // La base de la photo ne dépend pas des films des masques.
        assert_eq!(base_key(&s), base_key(&Settings::default()));
    }

    use super::*;

    fn gray_input() -> Input {
        Input {
            camera_to_working: lc::IDENTITY,
            exposure_bias: 0.0,
            scale: 1.0,
            rendered: false,
        }
    }

    fn develop_flat(v: f32, s: &Settings) -> [f32; 3] {
        let img = RgbImage::from_fn(32, 32, |_, _| [v, v, v]);
        let mut t = Timings::default();
        let out = develop(&img, &gray_input(), s, &Cancel::new(), &mut t).unwrap();
        out.pixel(16, 16)
    }

    #[test]
    fn mid_gray_renders_as_mid_gray_and_stays_neutral() {
        let p = develop_flat(0.18, &Settings::default());
        assert!(p.iter().all(|v| (v - Filmic::MID_OUT).abs() < 0.01), "{p:?}");
        let srgb = encode8(&RgbImage::from_fn(1, 1, |_, _| p), OutputSpace::Srgb);
        assert!((118..=124).contains(&srgb[0]), "{srgb:?}");
    }

    #[test]
    fn exposure_is_in_stops() {
        let base = develop_flat(0.09, &Settings::default());
        let plus = develop_flat(
            0.09,
            &Settings {
                exposure: 1.0,
                ..Settings::default()
            },
        );
        let ref18 = develop_flat(0.18, &Settings::default());
        assert!((plus[1] - ref18[1]).abs() < 1e-4, "+1 IL double la lumière");
        assert!(plus[1] > base[1]);
    }

    #[test]
    fn saturated_sensor_highlights_turn_white_not_pink() {
        let img = RgbImage::from_fn(8, 8, |_, _| [1.0, 1.0, 0.7]);
        let input = Input {
            camera_to_working: lc::diag([2.0, 1.0, 1.5]),
            exposure_bias: 0.0,
            scale: 1.0,
            rendered: false,
        };
        let out = develop(
            &img,
            &input,
            &Settings::default(),
            &Cancel::new(),
            &mut Timings::default(),
        )
        .unwrap();
        let p = out.pixel(4, 4);
        assert!((p[0] - p[1]).abs() < 0.02 && (p[1] - p[2]).abs() < 0.02, "{p:?}");
    }

    #[test]
    fn cancelled_renders_stop() {
        let cancel = Cancel::new();
        cancel.cancel();
        let img = RgbImage::new(4, 4);
        assert!(
            develop(
                &img,
                &gray_input(),
                &Settings::default(),
                &cancel,
                &mut Timings::default()
            )
            .is_err()
        );
    }

    #[test]
    fn output_stays_in_gamut_and_keeps_white() {
        let img = RgbImage::from_fn(2, 1, |x, _| if x == 0 { [1.0, 1.0, 1.0] } else { [0.0, 0.9, 0.0] });
        for space in [OutputSpace::Srgb, OutputSpace::DisplayP3] {
            let px = encode8(&img, space);
            assert_eq!(&px[0..3], &[255, 255, 255]);
            assert!(px[4] > 200 && px[3] < 60, "{space:?} {px:?}");
        }
        assert_eq!(encode16(&img, OutputSpace::Srgb)[0], 65535);
    }

    #[test]
    fn tone_curve_point_curve_applies() {
        let mut s = Settings::default();
        s.tone_curve.point = vec![(0.0, 0.0), (128.0, 180.0), (255.0, 255.0)];
        let lifted = develop_flat(0.18, &s);
        assert!(lifted[0] > Filmic::MID_OUT * 1.4, "{lifted:?}");
    }
}
