//! Tests colorimétriques : la fidélité des couleurs, mesurée en ΔE 2000 (CIE Lab D65),
//! sur les 24 plages d'une charte ColorChecker.
//!
//! Le RAW d'essai est fabriqué à partir des couleurs vraies de la charte, vues par un
//! vrai capteur (la matrice D65 du Sony A7 III, celle de la base du décodeur) :
//!
//! 1. la couleur d'entrée (matrice caméra, balance des blancs, adaptation chromatique)
//!    doit rendre la charte presque exactement (ΔE 2000 moyen < 1, max < 2), en lumière
//!    du jour comme sous une lampe à 3200 K (interpolation des matrices du capteur) ;
//! 2. le développement complet (profil Neutre) change la luminosité (courbe filmique)
//!    mais doit garder les teintes : écart de teinte moyen < 3°, max < 8°.

use luma_color::{self as lc, Mat3, Vec3};
use luma_core::Cancel;
use luma_project::{Profile, Settings};
use luma_raw::{Exif, Mosaic, Orientation, RawPhoto};
use luma_render::chart::{CHART, delta_e2000, lab, xyz_of_srgb};
use luma_render::{Photo, Size};

/// La matrice XYZ → caméra du Sony ILCE-7M3 sous l'illuminant A, tungstène (base de
/// rawler).
const SONY_A7M3_A: Mat3 = [
    [0.8664, -0.4056, 0.0403],
    [-0.4565, 1.2378, 0.2453],
    [-0.0301, 0.0889, 0.7157],
];

/// La matrice XYZ → caméra du Sony ILCE-7M3 sous D65 (base de rawler).
const SONY_A7M3_D65: Mat3 = [
    [0.7374, -0.2389, -0.0551],
    [-0.5435, 1.3162, 0.2519],
    [-0.1006, 0.1795, 0.6552],
];

/// Côté d'une plage, en pixels : assez grand pour que les étapes de voisinage (bruit,
/// accentuation, tonalité locale) ne mélangent pas les plages en leur centre.
const PATCH: usize = 96;

/// Un « RAW » de la charte : 6 × 4 plages de PATCH × PATCH pixels, vues par le capteur (déjà
/// dématriçées).
fn chart_raw(exposure: f32) -> RawPhoto {
    let to_cam = SONY_A7M3_D65;
    let white = lc::apply(&to_cam, lc::xy_to_xyz(lc::D65));
    let neutral = [white[0] / white[1], 1.0, white[2] / white[1]];
    let mut data = Vec::new();
    for y in 0..4 * PATCH {
        for x in 0..6 * PATCH {
            let cam = lc::apply(&to_cam, xyz_of_srgb(CHART[y / PATCH * 6 + x / PATCH]));
            data.extend(cam.map(|v| v * exposure / white[1]));
        }
    }
    RawPhoto {
        make: "Sony".into(),
        model: "ILCE-7M3".into(),
        width: 6 * PATCH,
        height: 4 * PATCH,
        mosaic: Mosaic::Linear,
        data,
        as_shot_neutral: neutral,
        matrices: vec![(6504.0, to_cam)],
        orientation: Orientation::Normal,
        baseline_exposure: 0.0,
        exif: Exif::default(),
    }
}

#[test]
fn the_camera_color_reproduces_the_chart() {
    let raw = chart_raw(1.0);
    let m = raw.camera_to_working(raw.as_shot_neutral);
    let to_xyz = lc::rgb_to_xyz(&lc::WORKING);
    let mut sum = 0.0;
    let mut worst = 0.0f32;
    for (k, p) in CHART.iter().enumerate() {
        let o = ((k / 6 * PATCH + PATCH / 2) * 6 * PATCH + k % 6 * PATCH + PATCH / 2) * 3;
        let cam = [raw.data[o], raw.data[o + 1], raw.data[o + 2]];
        let got = lab(lc::apply(&to_xyz, lc::apply(&m, cam)));
        let de = delta_e2000(lab(xyz_of_srgb(*p)), got);
        sum += de;
        worst = worst.max(de);
    }
    let mean = sum / 24.0;
    eprintln!("couleur d'entrée : ΔE 2000 moyen {mean:.3}, max {worst:.3}");
    assert!(mean < 1.0 && worst < 2.0, "ΔE 2000 moyen {mean}, max {worst}");
}

/// Sous une lampe de studio à 3200 K (entre les deux étalonnages du capteur, A et D65) :
/// l'app doit retrouver la température depuis le neutre, interpoler les deux matrices,
/// puis adapter les couleurs au blanc D65. Le « vrai » capteur est simulé par
/// l'interpolation exacte en 1/T ; l'écart mesure tout le reste.
#[test]
fn under_studio_tungsten_light_the_chart_comes_back_true() {
    let lamp = lc::temp_tint_to_xy(lc::TempTint {
        temperature: 3200.0,
        tint: 0.0,
    });
    let scene = lc::bradford(lc::D65, lamp);
    let g = ((1.0 / 3200.0 - 1.0 / 6504.0) / (1.0 / 2856.0 - 1.0 / 6504.0)) as f32;
    let to_cam: Mat3 =
        std::array::from_fn(|i| std::array::from_fn(|j| g * SONY_A7M3_A[i][j] + (1.0 - g) * SONY_A7M3_D65[i][j]));
    let white = lc::apply(&to_cam, lc::xy_to_xyz(lamp));
    let neutral = [white[0] / white[1], 1.0, white[2] / white[1]];
    let mut raw = chart_raw(1.0);
    raw.matrices = vec![(2856.0, SONY_A7M3_A), (6504.0, SONY_A7M3_D65)];
    raw.as_shot_neutral = neutral;
    let m = raw.camera_to_working(neutral);
    let to_xyz = lc::rgb_to_xyz(&lc::WORKING);
    let mut sum = 0.0;
    let mut worst = 0.0f32;
    for p in CHART {
        let cam = lc::apply(&to_cam, lc::apply(&scene, xyz_of_srgb(p))).map(|v| v / white[1]);
        let got = lab(lc::apply(&to_xyz, lc::apply(&m, cam)));
        let de = delta_e2000(lab(xyz_of_srgb(p)), got);
        sum += de;
        worst = worst.max(de);
    }
    let mean = sum / 24.0;
    eprintln!("3200 K : ΔE 2000 moyen {mean:.3}, max {worst:.3}");
    assert!(mean < 1.5 && worst < 3.0, "ΔE 2000 moyen {mean}, max {worst}");
}

#[test]
fn the_full_development_keeps_the_hues() {
    let photo = Photo::new(chart_raw(1.0));
    let s = Settings {
        profile: Profile::Neutral,
        ..Settings::default()
    };
    let img = photo.render(&s, Size::Full, &Cancel::new()).unwrap().image;
    let to_xyz = lc::rgb_to_xyz(&lc::WORKING);
    let mut errors = Vec::new();
    for (k, p) in CHART.iter().enumerate().take(18) {
        // Le centre de la plage (loin des bords, où l'accentuation agit).
        let px = img.data[(k / 6 * PATCH + PATCH / 2) * img.width + k % 6 * PATCH + PATCH / 2];
        let got = lab(lc::apply(&to_xyz, px));
        let want = lab(xyz_of_srgb(*p));
        let h = |l: Vec3| l[2].atan2(l[1]).to_degrees();
        let mut d = (h(got) - h(want)).abs();
        if d > 180.0 {
            d = 360.0 - d;
        }
        errors.push(d);
    }
    let mean = errors.iter().sum::<f32>() / errors.len() as f32;
    let worst = errors.iter().copied().fold(0.0, f32::max);
    eprintln!("développement complet : écart de teinte moyen {mean:.2}°, max {worst:.2}°");
    assert!(
        mean < 3.0 && worst < 8.0,
        "teinte : moyen {mean}°, max {worst}° ({errors:?})"
    );
}

/// L'outil de mesure (`luma colorchecker`) retrouve une charte parfaite, quelle que soit
/// l'exposition de la photo.
#[test]
fn the_chart_measurement_finds_a_perfect_chart_perfect() {
    let photo = Photo::new(chart_raw(0.35));
    let mut t = luma_core::Timings::default();
    let base = photo
        .base(&Settings::default(), Size::Full, &Cancel::new(), &mut t)
        .unwrap();
    let de = luma_render::chart::measure_chart(&base.image);
    let worst = de.iter().copied().fold(0.0, f32::max);
    assert!(worst < 0.5, "{de:?}");
}
