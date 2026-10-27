//! `luma colorchecker` : la mesure d'une charte et le profil du boîtier, sur une charte
//! de synthèse écrite en DNG par un boîtier imaginaire dont les couleurs dérivent.

use std::path::PathBuf;
use std::process::Command;

use luma_color as lc;
use luma_raw::{Exif, Mosaic, Orientation, RawPhoto};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luma-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// La moyenne affichée sur la ligne qui commence par `prefix`.
fn mean_after(text: &str, prefix: &str) -> f32 {
    let line = text
        .lines()
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("{text}"));
    let rest = &line[line.find("moyenne ").unwrap() + 8..];
    rest[..rest.find(',').unwrap()].parse().unwrap()
}

#[test]
fn a_chart_gives_a_camera_profile() {
    let dir = scratch("colorchecker");
    // Le boîtier : sa matrice dit « sRGB linéaire », mais ses couleurs dérivent (lignes
    // de somme 1 : le gris reste gris).
    let to_srgb = lc::inverse(&lc::rgb_to_xyz(&lc::SRGB)).unwrap();
    let template = RawPhoto {
        make: "LÜMA".into(),
        model: "Essai".into(),
        width: 0,
        height: 0,
        mosaic: Mosaic::Linear,
        data: Vec::new(),
        as_shot_neutral: [1.0, 1.0, 1.0],
        matrices: vec![(6504.0, to_srgb)],
        orientation: Orientation::Normal,
        baseline_exposure: 0.0,
        exif: Exif::default(),
        rendered: false,
        opcodes: None,
    };
    let drift = [[0.85, 0.2, -0.05], [0.08, 0.82, 0.1], [-0.04, 0.16, 0.88]];
    let (pw, ph) = (60, 60);
    let rgb: Vec<[f32; 3]> = (0..pw * 6 * ph * 4)
        .map(|i| {
            let (x, y) = (i % (pw * 6), i / (pw * 6));
            let p = luma_render::chart::CHART[(y / ph) * 6 + x / pw];
            let lin = p.map(|v| lc::srgb_decode(f32::from(v) / 255.0) * 0.8);
            lc::apply(&drift, lin)
        })
        .collect();
    let raw = dir.join("charte.dng");
    luma_raw::dng::write_linear_dng(&raw, pw * 6, ph * 4, &rgb, &template, 0.0, "LÜMA essai").unwrap();
    let profile = dir.join("profil.xmp");
    let out = Command::new(env!("CARGO_BIN_EXE_luma"))
        .args(["colorchecker", raw.to_str().unwrap(), "-o", profile.to_str().unwrap()])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{text}");
    let (before, after) = (mean_after(&text, "moyenne "), mean_after(&text, "avec le profil"));
    eprintln!("{text}");
    assert!(after < before / 3.0, "{before} → {after}");
    // Le profil : un préréglage du groupe « Profils de boîtier », avec son mélangeur.
    let xmp = std::fs::read_to_string(&profile).unwrap();
    assert!(
        xmp.contains("luma:ChannelMixer=") && xmp.contains("Profil LÜMA Essai"),
        "{xmp}"
    );
    let s = luma_project::xmp::read(&xmp).unwrap();
    assert!(!s.channel_mixer.is_identity());
    // Posé sur la charte, le profil la rend juste.
    let out = Command::new(env!("CARGO_BIN_EXE_luma"))
        .args([
            "colorchecker",
            raw.to_str().unwrap(),
            "--xmp",
            profile.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(mean_after(&text, "moyenne ") < 0.2, "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_second_camera_is_matched_to_the_first() {
    // La référence : le RAW Sony du corpus ; l'autre boîtier : la même scène, recadrée,
    // aux couleurs qui dérivent, écrite en DNG (sa matrice dit « espace de travail »).
    let arw = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus/_DSC0009.ARW");
    if !arw.is_file() {
        eprintln!("corpus absent (scripts/fetch-corpus.sh) : essai ignoré");
        return;
    }
    let dir = scratch("match-colors");
    let photo = luma_render::Photo::open(&arw).unwrap();
    let mut t = luma_core::Timings::default();
    let base = photo
        .base(
            &luma_project::Settings::default(),
            luma_render::Size::Fit(1600),
            &luma_core::Cancel::new(),
            &mut t,
        )
        .unwrap();
    let img = &base.image;
    let drift = [[0.9, 0.14, -0.04], [0.05, 0.86, 0.09], [-0.02, 0.12, 0.9]];
    let (x0, y0) = (img.width / 20, img.height / 20);
    let (w, h) = (img.width - 2 * x0, img.height - 2 * y0);
    let rgb: Vec<[f32; 3]> = (0..w * h)
        .map(|i| {
            let p = img.data[(i / w + y0) * img.width + i % w + x0];
            lc::apply(&drift, p).map(|v| (v * 0.8).clamp(0.0, 1.0))
        })
        .collect();
    let to_working = lc::inverse(&lc::rgb_to_xyz(&lc::WORKING)).unwrap();
    let template = RawPhoto {
        make: "LÜMA".into(),
        model: "Second".into(),
        width: 0,
        height: 0,
        mosaic: Mosaic::Linear,
        data: Vec::new(),
        as_shot_neutral: [1.0, 1.0, 1.0],
        matrices: vec![(6504.0, to_working)],
        orientation: Orientation::Normal,
        baseline_exposure: 0.0,
        exif: Exif::default(),
        rendered: false,
        opcodes: None,
    };
    let other = dir.join("second.dng");
    luma_raw::dng::write_linear_dng(&other, w, h, &rgb, &template, 0.0, "LÜMA essai").unwrap();
    let preset = dir.join("accord.xmp");
    let out = Command::new(env!("CARGO_BIN_EXE_luma"))
        .args([
            "match-colors",
            arw.to_str().unwrap(),
            other.to_str().unwrap(),
            "-o",
            preset.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{text}");
    eprintln!("{text}");
    let line = text.lines().find(|l| l.contains("écart de couleur")).unwrap();
    let numbers: Vec<f32> = line
        .split("écart de couleur ")
        .nth(1)
        .unwrap()
        .split(" (")
        .next()
        .unwrap()
        .split(" → ")
        .map(|v| v.parse().unwrap())
        .collect();
    assert!(numbers[1] < numbers[0] * 0.5, "{line}");
    let xmp = std::fs::read_to_string(&preset).unwrap();
    assert!(
        xmp.contains("luma:ChannelMixer=") && xmp.contains("Accord LÜMA Second sur Sony"),
        "{xmp}"
    );
    // Une seule photo : l'erreur le dit.
    let out = Command::new(env!("CARGO_BIN_EXE_luma"))
        .args(["match-colors", arw.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("deux photos"));
    let _ = std::fs::remove_dir_all(&dir);
}
