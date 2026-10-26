//! Le copier‑coller sélectif : les réglages d'une photo collés sur une autre, groupe par
//! groupe (la balance des blancs sans le recadrage, la couleur sans les masques…).

use crate::Settings;

/// Un groupe de réglages qu'on peut coller seul.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    WhiteBalance,
    Exposure,
    Tone,
    Presence,
    Curve,
    Mixer,
    Grading,
    Detail,
    Effects,
    Calibration,
    Look,
    Crop,
    Geometry,
    Masks,
}

/// Tous les groupes, dans l'ordre des panneaux, avec leur nom (clé de traduction).
pub const GROUPS: [(Group, &str); 14] = [
    (Group::WhiteBalance, "Balance des blancs"),
    (Group::Exposure, "Exposition"),
    (Group::Tone, "Contraste, hautes lumières, ombres, blancs, noirs"),
    (Group::Presence, "Texture, clarté, voile, vibrance, saturation"),
    (Group::Curve, "Courbe des tonalités"),
    (Group::Mixer, "Mélangeur de couleurs"),
    (Group::Grading, "Étalonnage des couleurs"),
    (Group::Detail, "Détail (accentuation, bruit)"),
    (Group::Effects, "Effets (vignetage, grain)"),
    (Group::Calibration, "Étalonnage de l'appareil photo"),
    (Group::Look, "Profil et film"),
    (Group::Crop, "Recadrage"),
    (Group::Geometry, "Objectif et transformation"),
    (Group::Masks, "Masques"),
];

/// Les groupes cochés par défaut : tout sauf ce qui dépend du cadrage de chaque photo
/// (recadrage, masques), qu'on ne veut presque jamais recopier.
pub fn default_groups() -> Vec<Group> {
    GROUPS
        .iter()
        .map(|g| g.0)
        .filter(|g| !matches!(g, Group::Crop | Group::Masks))
        .collect()
}

/// `target` avec les groupes `groups` pris dans `source` ; le reste ne change pas.
pub fn paste(target: &Settings, source: &Settings, groups: &[Group]) -> Settings {
    let mut s = target.clone();
    for g in groups {
        match g {
            Group::WhiteBalance => s.white_balance = source.white_balance,
            Group::Exposure => s.exposure = source.exposure,
            Group::Tone => {
                s.contrast = source.contrast;
                s.highlights = source.highlights;
                s.shadows = source.shadows;
                s.whites = source.whites;
                s.blacks = source.blacks;
            }
            Group::Presence => {
                s.texture = source.texture;
                s.clarity = source.clarity;
                s.dehaze = source.dehaze;
                s.vibrance = source.vibrance;
                s.saturation = source.saturation;
            }
            Group::Curve => s.tone_curve = source.tone_curve.clone(),
            Group::Mixer => s.hsl = source.hsl.clone(),
            Group::Grading => s.color_grading = source.color_grading.clone(),
            Group::Detail => s.detail = source.detail.clone(),
            Group::Effects => s.effects = source.effects.clone(),
            Group::Calibration => s.calibration = source.calibration.clone(),
            Group::Look => {
                s.profile = source.profile;
                s.lut = source.lut.clone();
                s.film = source.film.clone();
            }
            Group::Crop => s.crop = source.crop,
            Group::Geometry => {
                s.lens = source.lens.clone();
                s.transform = source.transform.clone();
            }
            Group::Masks => s.masks = source.masks.clone(),
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Crop, WhiteBalance};

    fn edited() -> Settings {
        let mut s = Settings {
            white_balance: WhiteBalance::Custom {
                temperature: 4200.0,
                tint: 12.0,
            },
            exposure: 0.7,
            contrast: 25.0,
            shadows: 40.0,
            clarity: 15.0,
            vibrance: 20.0,
            crop: Crop {
                left: 0.1,
                ..Crop::default()
            },
            ..Settings::default()
        };
        s.hsl.saturation[5] = -30.0;
        s.detail.sharpen_amount = 80.0;
        s.effects.grain_amount = 30.0;
        s.masks.push(crate::Mask::full());
        s
    }

    #[test]
    fn everything_pasted_is_the_source() {
        let all: Vec<Group> = GROUPS.iter().map(|g| g.0).collect();
        assert_eq!(paste(&Settings::default(), &edited(), &all), edited());
    }

    #[test]
    fn only_the_chosen_groups_are_pasted() {
        let target = Settings {
            exposure: -0.5,
            contrast: -10.0,
            ..Settings::default()
        };
        let out = paste(&target, &edited(), &[Group::WhiteBalance, Group::Mixer]);
        assert_eq!(out.white_balance, edited().white_balance);
        assert_eq!(out.hsl, edited().hsl);
        // Le reste est celui de la photo cible.
        assert_eq!((out.exposure, out.contrast), (-0.5, -10.0));
        assert_eq!(out.crop, Crop::default());
        assert!(out.masks.is_empty());
        assert_eq!(out.detail, target.detail);
    }

    #[test]
    fn by_default_the_framing_stays_with_each_photo() {
        let out = paste(&Settings::default(), &edited(), &default_groups());
        assert_eq!(out.exposure, 0.7);
        assert_eq!(out.detail.sharpen_amount, 80.0);
        assert_eq!(out.crop, Crop::default(), "le recadrage reste celui de la photo");
        assert!(out.masks.is_empty(), "les masques restent ceux de la photo");
    }
}
