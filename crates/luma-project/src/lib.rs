//! Les réglages d'une photo, et leur enregistrement en JSON et en XMP.
//!
//! Une retouche n'est jamais appliquée aux pixels d'origine : c'est une liste de
//! paramètres, rejouée à chaque rendu. Ces paramètres sont rangés par panneau
//! (Base, Courbe des tonalités, Mélangeur de couleurs…), chacun avec sa plage et sa
//! valeur par défaut pour un RAW ; ils s'enregistrent et se relisent en XMP (voir
//! [`xmp`]).

pub mod copies;
pub mod defaults;
pub mod liquify;
pub mod masks;
pub mod paste;
pub mod presets;
pub mod retouch;
pub mod xmp;
pub mod xmp_masks;

pub use liquify::{Warp, WarpKind};
pub use masks::{Combine, Dab, Local, Look, Mask, Part, Range, Shape};
pub use retouch::{Spot, SpotKind};

use serde::{Deserialize, Serialize};

/// La balance des blancs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum WhiteBalance {
    /// Celle enregistrée par le boîtier (« Telle quelle »).
    #[default]
    AsShot,
    /// Température en kelvins (2000 à 50 000) et teinte (−150 à +150).
    Custom { temperature: f32, tint: f32 },
}

/// Le profil de rendu (navigateur de profils) : le point de départ de la
/// retouche, avant tout curseur.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    /// Contraste et saturation modérés, adaptés à la plupart des photos (« Couleur »).
    #[default]
    Color,
    /// Rendu doux et peu saturé, pour une retouche poussée (« Neutre », « Standard »).
    Neutral,
    /// Contraste et saturation marqués (« Vif »).
    Vivid,
    /// Noir et blanc.
    Monochrome,
    /// Le rendu du fichier lui‑même, pour une image déjà développée (JPEG, PNG, TIFF…) :
    /// sans courbe filmique ni accentuation, l'image reste telle quelle tant qu'aucun
    /// réglage ne change (le profil « incorporé »).
    Original,
}

impl Profile {
    /// (multiplicateur de contraste, multiplicateur de chroma)
    pub fn look(self) -> (f32, f32) {
        match self {
            Profile::Color => (1.08, 1.15),
            Profile::Neutral => (0.94, 1.0),
            Profile::Vivid => (1.16, 1.35),
            Profile::Monochrome => (1.08, 0.0),
            Profile::Original => (1.0, 1.0),
        }
    }
}

/// Panneau Courbe des tonalités.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToneCurve {
    /// Courbe paramétrique : −100 à +100 par région.
    pub highlights: f32,
    pub lights: f32,
    pub darks: f32,
    pub shadows: f32,
    /// Séparations des régions, 0 à 100 (25, 50, 75 par défaut).
    pub shadow_split: f32,
    pub midtone_split: f32,
    pub highlight_split: f32,
    /// Courbes à points (0 à 255), sur la luminosité puis par couche.
    pub point: Vec<(f32, f32)>,
    pub red: Vec<(f32, f32)>,
    pub green: Vec<(f32, f32)>,
    pub blue: Vec<(f32, f32)>,
}

impl Default for ToneCurve {
    fn default() -> Self {
        let linear = vec![(0.0, 0.0), (255.0, 255.0)];
        Self {
            highlights: 0.0,
            lights: 0.0,
            darks: 0.0,
            shadows: 0.0,
            shadow_split: 25.0,
            midtone_split: 50.0,
            highlight_split: 75.0,
            point: linear.clone(),
            red: linear.clone(),
            green: linear.clone(),
            blue: linear,
        }
    }
}

/// Les huit couleurs du panneau Mélangeur de couleurs (TSL).
pub const HSL_COLORS: [&str; 8] = ["red", "orange", "yellow", "green", "aqua", "blue", "purple", "magenta"];

/// Panneau Mélangeur de couleurs : teinte, saturation et luminance de huit couleurs,
/// −100 à +100.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hsl {
    pub hue: [f32; 8],
    pub saturation: [f32; 8],
    pub luminance: [f32; 8],
}

/// Une roue de l'étalonnage des couleurs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Wheel {
    /// 0 à 360.
    pub hue: f32,
    /// 0 à 100.
    pub saturation: f32,
    /// −100 à +100.
    pub luminance: f32,
}

/// Panneau Étalonnage des couleurs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorGrading {
    pub shadows: Wheel,
    pub midtones: Wheel,
    pub highlights: Wheel,
    pub global: Wheel,
    /// 0 à 100 (50 par défaut).
    pub blending: f32,
    /// −100 à +100.
    pub balance: f32,
}

impl Default for ColorGrading {
    fn default() -> Self {
        Self {
            shadows: Wheel::default(),
            midtones: Wheel::default(),
            highlights: Wheel::default(),
            global: Wheel::default(),
            blending: 50.0,
            balance: 0.0,
        }
    }
}

/// Panneau Détail.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Detail {
    /// Accentuation : gain 0 à 150, rayon 0,5 à 3, détail 0 à 100, masquage 0 à 100.
    pub sharpen_amount: f32,
    pub sharpen_radius: f32,
    pub sharpen_detail: f32,
    pub sharpen_masking: f32,
    /// Accentuation par déconvolution (Richardson‑Lucy) plutôt que par masque flou ;
    /// faux par défaut, ce qui garde le rendu des photos déjà réglées.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub sharpen_deconvolution: bool,
    /// Réduction du bruit de luminance : 0 à 100, détail, contraste.
    pub luminance_nr: f32,
    pub luminance_nr_detail: f32,
    pub luminance_nr_contrast: f32,
    /// Luminance par ondelettes (plus fin, environ deux fois plus lent) plutôt que par le filtre
    /// guidé ; faux par défaut, ce qui garde le rendu des photos déjà réglées.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub luminance_nr_wavelets: bool,
    /// Réduction du bruit de couleur : 0 à 100, détail, lissage.
    pub color_nr: f32,
    pub color_nr_detail: f32,
    pub color_nr_smoothness: f32,
}

impl Default for Detail {
    fn default() -> Self {
        // Les valeurs par défaut pour un RAW.
        Self {
            sharpen_amount: 40.0,
            sharpen_radius: 1.0,
            sharpen_detail: 25.0,
            sharpen_masking: 0.0,
            sharpen_deconvolution: false,
            luminance_nr: 0.0,
            luminance_nr_detail: 50.0,
            luminance_nr_contrast: 0.0,
            luminance_nr_wavelets: false,
            color_nr: 25.0,
            color_nr_detail: 50.0,
            color_nr_smoothness: 50.0,
        }
    }
}

/// Panneau Effets : vignetage après recadrage et grain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Effects {
    /// −100 à +100.
    pub vignette_amount: f32,
    /// 0 à 100 (50).
    pub vignette_midpoint: f32,
    /// −100 à +100.
    pub vignette_roundness: f32,
    /// 0 à 100 (50).
    pub vignette_feather: f32,
    /// 0 à 100 : protège les hautes lumières d'un vignetage sombre.
    pub vignette_highlights: f32,
    /// 0 à 100.
    pub grain_amount: f32,
    /// 0 à 100 (25).
    pub grain_size: f32,
    /// 0 à 100 (50).
    pub grain_roughness: f32,
}

impl Default for Effects {
    fn default() -> Self {
        Self {
            vignette_amount: 0.0,
            vignette_midpoint: 50.0,
            vignette_roundness: 0.0,
            vignette_feather: 50.0,
            vignette_highlights: 0.0,
            grain_amount: 0.0,
            grain_size: 25.0,
            grain_roughness: 50.0,
        }
    }
}

/// Panneau Étalonnage de l'appareil photo : −100 à +100.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Calibration {
    pub shadows_tint: f32,
    pub red_hue: f32,
    pub red_saturation: f32,
    pub green_hue: f32,
    pub green_saturation: f32,
    pub blue_hue: f32,
    pub blue_saturation: f32,
}

/// Le mélangeur de couches : chaque couche de sortie (rouge, vert, bleu) faite d'un
/// mélange des trois couches de la photo, en pour cent (−200 à +200). Par défaut, chaque
/// couche est elle‑même à 100 %.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelMixer {
    pub red: [f32; 3],
    pub green: [f32; 3],
    pub blue: [f32; 3],
}

impl Default for ChannelMixer {
    fn default() -> Self {
        Self {
            red: [100.0, 0.0, 0.0],
            green: [0.0, 100.0, 0.0],
            blue: [0.0, 0.0, 100.0],
        }
    }
}

impl ChannelMixer {
    pub fn is_identity(&self) -> bool {
        *self == ChannelMixer::default()
    }

    /// La matrice du mélange (lignes : rouge, vert, bleu de sortie), en fractions.
    pub fn matrix(&self) -> [[f32; 3]; 3] {
        [self.red, self.green, self.blue].map(|row| row.map(|v| v / 100.0))
    }
}

/// Panneau Corrections de l'objectif (réglages manuels).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lens {
    /// Les corrections enregistrées par le boîtier (ou écrites dans le DNG) : distorsion,
    /// aberration chromatique et vignetage, appliquées automatiquement. Les curseurs
    /// ci‑dessous s'y ajoutent, ou les remplacent quand elles sont coupées.
    pub camera: bool,
    /// Supprimer l'aberration chromatique (latérale, estimée sur la photo).
    pub remove_ca: bool,
    /// Distorsion : −100 à +100 (positif corrige le barillet).
    pub distortion: f32,
    /// Vignetage de l'objectif : −100 à +100 (positif éclaircit les coins), milieu 0 à 100.
    pub vignetting: f32,
    pub vignetting_midpoint: f32,
    /// Défrangeage : violet et vert, de 0 à 20.
    pub defringe_purple: f32,
    pub defringe_green: f32,
}

impl Default for Lens {
    fn default() -> Self {
        Self {
            camera: true,
            remove_ca: false,
            distortion: 0.0,
            vignetting: 0.0,
            vignetting_midpoint: 50.0,
            defringe_purple: 0.0,
            defringe_green: 0.0,
        }
    }
}

/// Panneau Transformation : perspective et échelle (curseurs manuels).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform {
    /// −100 à +100 : redresse les verticales (positif : le haut s'élargit).
    pub vertical: f32,
    /// −100 à +100 : redresse les horizontales.
    pub horizontal: f32,
    /// −10 à +10 degrés.
    pub rotate: f32,
    /// −100 à +100 : étire en largeur (positif) ou en hauteur.
    pub aspect: f32,
    /// 50 à 150 %.
    pub scale: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            vertical: 0.0,
            horizontal: 0.0,
            rotate: 0.0,
            aspect: 0.0,
            scale: 100.0,
        }
    }
}

impl Lens {
    pub fn is_identity(&self) -> bool {
        *self == Lens::default()
    }
}

impl Transform {
    pub fn is_identity(&self) -> bool {
        *self == Transform::default()
    }
}

/// Recadrage (bords en fraction de l'image orientée, 0 à 1) et redressement (degrés).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Crop {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub angle: f32,
}

impl Default for Crop {
    fn default() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            right: 1.0,
            bottom: 1.0,
            angle: 0.0,
        }
    }
}

impl Crop {
    pub fn is_identity(&self) -> bool {
        *self == Crop::default()
    }
}

/// Tous les réglages d'une photo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub profile: Profile,
    pub white_balance: WhiteBalance,
    /// −5 à +5 IL.
    pub exposure: f32,
    /// −100 à +100.
    pub contrast: f32,
    pub highlights: f32,
    /// Les canaux saturés au capteur reconstruits à partir des autres ; faux par défaut,
    /// ce qui garde le rendu des photos déjà réglées.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub highlight_reconstruction: bool,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub texture: f32,
    pub clarity: f32,
    pub dehaze: f32,
    pub vibrance: f32,
    pub saturation: f32,
    pub tone_curve: ToneCurve,
    pub hsl: Hsl,
    pub color_grading: ColorGrading,
    pub detail: Detail,
    pub effects: Effects,
    pub calibration: Calibration,
    /// Mélangeur de couches.
    #[serde(skip_serializing_if = "ChannelMixer::is_identity")]
    pub channel_mixer: ChannelMixer,
    pub crop: Crop,
    pub lens: Lens,
    pub transform: Transform,
    /// Panneau Masquage : les retouches locales, dans l'ordre.
    pub masks: Vec<Mask>,
    /// Tampon et correcteur : les retouches, dans l'ordre (chacune voit les précédentes).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub spots: Vec<Spot>,
    /// Fluidité : les coups de pinceau, dans l'ordre, avant les retouches.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub liquify: Vec<Warp>,
    /// Un film mesuré sur une vraie pellicule ou une LUT 3D importée (`.cube`) : son nom
    /// et son intensité (0 à 100).
    pub lut: Option<Look>,
    /// Le film (préréglage) appliqué, pour le savoir d'un coup d'œil : son nom et son
    /// intensité (0 à 100). Il ne change pas le rendu.
    pub film: Option<Look>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profile: Profile::Color,
            white_balance: WhiteBalance::AsShot,
            exposure: 0.0,
            contrast: 0.0,
            highlights: 0.0,
            highlight_reconstruction: false,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            texture: 0.0,
            clarity: 0.0,
            dehaze: 0.0,
            vibrance: 0.0,
            saturation: 0.0,
            tone_curve: ToneCurve::default(),
            hsl: Hsl::default(),
            color_grading: ColorGrading::default(),
            detail: Detail::default(),
            effects: Effects::default(),
            calibration: Calibration::default(),
            channel_mixer: ChannelMixer::default(),
            crop: Crop::default(),
            lens: Lens::default(),
            transform: Transform::default(),
            masks: Vec::new(),
            spots: Vec::new(),
            liquify: Vec::new(),
            lut: None,
            film: None,
        }
    }
}

fn clamp(v: &mut f32, lo: f32, hi: f32) {
    *v = if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        0.0f32.clamp(lo, hi)
    };
}

impl Settings {
    /// Les réglages de départ d'une image déjà développée (JPEG, PNG, TIFF…) : le
    /// profil Original, ni accentuation ni réduction du bruit de couleur (l'usage
    /// pour un JPEG) : l'image s'ouvre telle qu'elle est.
    pub fn for_rendered() -> Self {
        let mut s = Settings {
            profile: Profile::Original,
            ..Settings::default()
        };
        s.detail.sharpen_amount = 0.0;
        s.detail.color_nr = 0.0;
        s
    }

    /// Ramène chaque réglage dans sa plage (fichiers abîmés, valeurs venues d'ailleurs).
    pub fn clamped(mut self) -> Self {
        let s = &mut self;
        if let WhiteBalance::Custom { temperature, tint } = &mut s.white_balance {
            clamp(temperature, 2000.0, 50000.0);
            clamp(tint, -150.0, 150.0);
        }
        clamp(&mut s.exposure, -5.0, 5.0);
        clamp(&mut s.lens.distortion, -100.0, 100.0);
        clamp(&mut s.lens.vignetting, -100.0, 100.0);
        clamp(&mut s.lens.vignetting_midpoint, 0.0, 100.0);
        clamp(&mut s.lens.defringe_purple, 0.0, 20.0);
        clamp(&mut s.lens.defringe_green, 0.0, 20.0);
        clamp(&mut s.transform.vertical, -100.0, 100.0);
        clamp(&mut s.transform.horizontal, -100.0, 100.0);
        clamp(&mut s.transform.rotate, -10.0, 10.0);
        clamp(&mut s.transform.aspect, -100.0, 100.0);
        clamp(&mut s.transform.scale, 50.0, 150.0);
        for spot in &mut s.spots {
            spot.clamp();
        }
        for warp in &mut s.liquify {
            warp.clamp();
        }
        s.liquify.retain(|w| !w.points.is_empty());
        let mixer = &mut s.channel_mixer;
        for row in [&mut mixer.red, &mut mixer.green, &mut mixer.blue] {
            for v in row.iter_mut() {
                clamp(v, -200.0, 200.0);
            }
        }
        for m in &mut s.masks {
            m.local.clamp();
            if let Some(r) = &mut m.range {
                r.clamp();
            }
            clamp(&mut m.amount, 0.0, 100.0);
            if let Some(l) = &mut m.look {
                clamp(&mut l.amount, 0.0, 100.0);
            }
        }
        for l in [&mut s.lut, &mut s.film].into_iter().flatten() {
            clamp(&mut l.amount, 0.0, 100.0);
        }
        for v in [
            &mut s.contrast,
            &mut s.highlights,
            &mut s.shadows,
            &mut s.whites,
            &mut s.blacks,
            &mut s.texture,
            &mut s.clarity,
            &mut s.dehaze,
            &mut s.vibrance,
            &mut s.saturation,
        ] {
            clamp(v, -100.0, 100.0);
        }
        let c = &mut s.tone_curve;
        for v in [&mut c.highlights, &mut c.lights, &mut c.darks, &mut c.shadows] {
            clamp(v, -100.0, 100.0);
        }
        clamp(&mut c.shadow_split, 10.0, 70.0);
        clamp(&mut c.midtone_split, 20.0, 80.0);
        clamp(&mut c.highlight_split, 30.0, 90.0);
        for curve in [&mut c.point, &mut c.red, &mut c.green, &mut c.blue] {
            for p in curve.iter_mut() {
                clamp(&mut p.0, 0.0, 255.0);
                clamp(&mut p.1, 0.0, 255.0);
            }
            curve.sort_by(|a, b| a.0.total_cmp(&b.0));
            curve.dedup_by(|a, b| a.0 == b.0);
            if curve.len() < 2 {
                *curve = vec![(0.0, 0.0), (255.0, 255.0)];
            }
        }
        for band in [&mut s.hsl.hue, &mut s.hsl.saturation, &mut s.hsl.luminance] {
            band.iter_mut().for_each(|v| clamp(v, -100.0, 100.0));
        }
        let g = &mut s.color_grading;
        for w in [&mut g.shadows, &mut g.midtones, &mut g.highlights, &mut g.global] {
            w.hue = w.hue.rem_euclid(360.0);
            clamp(&mut w.saturation, 0.0, 100.0);
            clamp(&mut w.luminance, -100.0, 100.0);
        }
        clamp(&mut g.blending, 0.0, 100.0);
        clamp(&mut g.balance, -100.0, 100.0);
        let d = &mut s.detail;
        clamp(&mut d.sharpen_amount, 0.0, 150.0);
        clamp(&mut d.sharpen_radius, 0.5, 3.0);
        for v in [
            &mut d.sharpen_detail,
            &mut d.sharpen_masking,
            &mut d.luminance_nr,
            &mut d.luminance_nr_detail,
            &mut d.luminance_nr_contrast,
            &mut d.color_nr,
            &mut d.color_nr_detail,
            &mut d.color_nr_smoothness,
        ] {
            clamp(v, 0.0, 100.0);
        }
        let e = &mut s.effects;
        clamp(&mut e.vignette_amount, -100.0, 100.0);
        clamp(&mut e.vignette_roundness, -100.0, 100.0);
        for v in [
            &mut e.vignette_midpoint,
            &mut e.vignette_feather,
            &mut e.vignette_highlights,
            &mut e.grain_amount,
            &mut e.grain_size,
            &mut e.grain_roughness,
        ] {
            clamp(v, 0.0, 100.0);
        }
        let k = &mut s.calibration;
        for v in [
            &mut k.shadows_tint,
            &mut k.red_hue,
            &mut k.red_saturation,
            &mut k.green_hue,
            &mut k.green_saturation,
            &mut k.blue_hue,
            &mut k.blue_saturation,
        ] {
            clamp(v, -100.0, 100.0);
        }
        let r = &mut s.crop;
        clamp(&mut r.left, 0.0, 1.0);
        clamp(&mut r.top, 0.0, 1.0);
        clamp(&mut r.right, 0.0, 1.0);
        clamp(&mut r.bottom, 0.0, 1.0);
        if r.right - r.left < 0.01 || r.bottom - r.top < 0.01 {
            *r = Crop {
                angle: r.angle,
                ..Crop::default()
            };
        }
        clamp(&mut r.angle, -45.0, 45.0);
        self
    }

    /// Les réglages en JSON (le format natif de LÜMA, lisible et versionnable).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("réglages sérialisables")
    }

    pub fn from_json(text: &str) -> luma_core::Result<Settings> {
        serde_json::from_str::<Settings>(text)
            .map(Settings::clamped)
            .map_err(|e| luma_core::Error::Format(e.to_string()))
    }

    /// Applique `nom=valeur` (ex. `exposure=+0.5`, `hsl.saturation.blue=-30`,
    /// `temperature=5200`). Utilisé par la ligne de commande et les tests.
    pub fn set(&mut self, assignment: &str) -> Result<(), String> {
        let (key, value) = assignment
            .split_once('=')
            .ok_or_else(|| format!("« {assignment} » : attendu nom=valeur"))?;
        let key = key.trim().to_ascii_lowercase();
        if key == "profile" {
            self.profile = match value.trim().to_ascii_lowercase().as_str() {
                "color" | "couleur" => Profile::Color,
                "neutral" | "neutre" => Profile::Neutral,
                "vivid" | "vif" => Profile::Vivid,
                "monochrome" | "bw" | "nb" => Profile::Monochrome,
                "original" | "embedded" => Profile::Original,
                other => {
                    return Err(format!(
                        "profil inconnu « {other} » (color, neutral, vivid, monochrome, original)"
                    ));
                }
            };
            return Ok(());
        }
        // Les interrupteurs : oui ou non.
        let switch = match key.as_str() {
            "highlights.reconstruction" => Some(&mut self.highlight_reconstruction),
            "sharpen.deconvolution" => Some(&mut self.detail.sharpen_deconvolution),
            "noise.wavelets" => Some(&mut self.detail.luminance_nr_wavelets),
            _ => None,
        };
        if let Some(slot) = switch {
            *slot = match value.trim().to_ascii_lowercase().as_str() {
                "on" | "oui" | "yes" | "true" | "1" => true,
                "off" | "non" | "no" | "false" | "0" => false,
                other => return Err(format!("« {other} » : attendu on ou off")),
            };
            return Ok(());
        }
        let v: f32 = value
            .trim()
            .trim_start_matches('+')
            .parse()
            .map_err(|_| format!("« {value} » n'est pas un nombre"))?;
        let band = |name: &str| HSL_COLORS.iter().position(|c| *c == name);
        let slot: &mut f32 = match key.as_str() {
            "temperature" | "tint" => {
                let current = match self.white_balance {
                    WhiteBalance::Custom { temperature, tint } => (temperature, tint),
                    WhiteBalance::AsShot => (5500.0, 0.0),
                };
                let (temperature, tint) = if key == "temperature" {
                    (v, current.1)
                } else {
                    (current.0, v)
                };
                self.white_balance = WhiteBalance::Custom { temperature, tint };
                return Ok(());
            }
            "exposure" => &mut self.exposure,
            "contrast" => &mut self.contrast,
            "highlights" => &mut self.highlights,
            "shadows" => &mut self.shadows,
            "whites" => &mut self.whites,
            "blacks" => &mut self.blacks,
            "texture" => &mut self.texture,
            "clarity" => &mut self.clarity,
            "dehaze" => &mut self.dehaze,
            "vibrance" => &mut self.vibrance,
            "saturation" => &mut self.saturation,
            "curve.highlights" => &mut self.tone_curve.highlights,
            "curve.lights" => &mut self.tone_curve.lights,
            "curve.darks" => &mut self.tone_curve.darks,
            "curve.shadows" => &mut self.tone_curve.shadows,
            "sharpen" | "sharpen.amount" => &mut self.detail.sharpen_amount,
            "sharpen.radius" => &mut self.detail.sharpen_radius,
            "sharpen.detail" => &mut self.detail.sharpen_detail,
            "sharpen.masking" => &mut self.detail.sharpen_masking,
            "noise.luminance" => &mut self.detail.luminance_nr,
            "noise.color" => &mut self.detail.color_nr,
            "vignette" | "vignette.amount" => &mut self.effects.vignette_amount,
            "vignette.midpoint" => &mut self.effects.vignette_midpoint,
            "vignette.feather" => &mut self.effects.vignette_feather,
            "vignette.roundness" => &mut self.effects.vignette_roundness,
            "grain" | "grain.amount" => &mut self.effects.grain_amount,
            "grain.size" => &mut self.effects.grain_size,
            "grain.roughness" => &mut self.effects.grain_roughness,
            "grading.blending" => &mut self.color_grading.blending,
            "grading.balance" => &mut self.color_grading.balance,
            "crop.angle" => &mut self.crop.angle,
            "lens.ca" => {
                self.lens.remove_ca = v != 0.0;
                return Ok(());
            }
            "lens.distortion" => &mut self.lens.distortion,
            "lens.vignetting" => &mut self.lens.vignetting,
            "lens.midpoint" => &mut self.lens.vignetting_midpoint,
            "defringe.purple" => &mut self.lens.defringe_purple,
            "defringe.green" => &mut self.lens.defringe_green,
            "transform.vertical" => &mut self.transform.vertical,
            "transform.horizontal" => &mut self.transform.horizontal,
            "transform.rotate" => &mut self.transform.rotate,
            "transform.aspect" => &mut self.transform.aspect,
            "transform.scale" => &mut self.transform.scale,
            other => {
                let parts: Vec<&str> = other.split('.').collect();
                match parts.as_slice() {
                    ["hsl", kind, color] => {
                        let i = band(color).ok_or_else(|| format!("couleur inconnue « {color} »"))?;
                        match *kind {
                            "hue" => &mut self.hsl.hue[i],
                            "saturation" => &mut self.hsl.saturation[i],
                            "luminance" => &mut self.hsl.luminance[i],
                            _ => return Err(format!("réglage inconnu « {other} »")),
                        }
                    }
                    ["grading", wheel, field] => {
                        let w = match *wheel {
                            "shadows" => &mut self.color_grading.shadows,
                            "midtones" => &mut self.color_grading.midtones,
                            "highlights" => &mut self.color_grading.highlights,
                            "global" => &mut self.color_grading.global,
                            _ => return Err(format!("roue inconnue « {wheel} »")),
                        };
                        match *field {
                            "hue" => &mut w.hue,
                            "saturation" => &mut w.saturation,
                            "luminance" => &mut w.luminance,
                            _ => return Err(format!("réglage inconnu « {other} »")),
                        }
                    }
                    _ => return Err(format!("réglage inconnu « {other} »")),
                }
            }
        };
        *slot = v;
        *self = std::mem::take(self).clamped();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_usual_ones_for_a_raw() {
        let s = Settings::default();
        assert_eq!(s.detail.sharpen_amount, 40.0);
        assert_eq!(s.detail.sharpen_radius, 1.0);
        assert_eq!(s.detail.color_nr, 25.0);
        assert_eq!(s.tone_curve.midtone_split, 50.0);
        assert_eq!(s.white_balance, WhiteBalance::AsShot);
    }

    #[test]
    fn values_are_clamped_to_their_ranges() {
        let mut s = Settings {
            exposure: 9.0,
            contrast: -300.0,
            ..Settings::default()
        };
        s.hsl.hue[2] = f32::NAN;
        let s = s.clamped();
        assert_eq!((s.exposure, s.contrast, s.hsl.hue[2]), (5.0, -100.0, 0.0));
    }

    #[test]
    fn assignments() {
        let mut s = Settings::default();
        s.set("exposure=+0.7").unwrap();
        s.set("hsl.saturation.blue=-30").unwrap();
        s.set("grading.shadows.hue=220").unwrap();
        s.set("temperature=5200").unwrap();
        s.set("profile=vivid").unwrap();
        assert_eq!(s.profile, Profile::Vivid);
        assert!(s.set("profile=sepia").is_err());
        s.set("sharpen.deconvolution=on").unwrap();
        s.set("noise.wavelets=oui").unwrap();
        assert!(s.detail.sharpen_deconvolution && s.detail.luminance_nr_wavelets);
        s.set("noise.wavelets=off").unwrap();
        assert!(!s.detail.luminance_nr_wavelets);
        assert!(s.set("sharpen.deconvolution=peut-être").is_err());
        s.set("highlights.reconstruction=on").unwrap();
        assert!(s.highlight_reconstruction);
        assert_eq!(s.exposure, 0.7);
        assert_eq!(s.hsl.saturation[5], -30.0);
        assert_eq!(s.color_grading.shadows.hue, 220.0);
        assert_eq!(
            s.white_balance,
            WhiteBalance::Custom {
                temperature: 5200.0,
                tint: 0.0
            }
        );
        assert!(s.set("nimporte=1").is_err());
        assert!(s.set("exposure=beaucoup").is_err());
    }

    #[test]
    fn json_round_trips() {
        let mut s = Settings::default();
        s.set("clarity=25").unwrap();
        s.tone_curve.point = vec![(0.0, 10.0), (128.0, 140.0), (255.0, 250.0)];
        let back = Settings::from_json(&s.to_json()).unwrap();
        assert_eq!(back, s);
    }
}
