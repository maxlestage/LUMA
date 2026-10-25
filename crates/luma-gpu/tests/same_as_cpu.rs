//! Le GPU doit donner la même image que le processeur, la référence : au plus deux
//! niveaux d'écart sur 255, et en moyenne bien moins. (Les petits écarts viennent des
//! fonctions mathématiques du GPU, `atan2`, `pow`, `exp`, un peu moins précises.)

use luma_color::OutputSpace;
use luma_core::{Cancel, RgbImage, Timings};
use luma_gpu::Gpu;
use luma_project::{Profile, Settings, Wheel};

/// Une image d'essai : toutes les teintes en largeur, de −4 à +3 IL en hauteur, une
/// bande de gris en bas.
fn test_image(w: usize, h: usize) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| {
        let hue = x as f32 / w as f32 * 6.0;
        let v = 0.18 * ((y as f32 / h as f32) * 7.0 - 4.0).exp2();
        if y > h * 9 / 10 {
            return [v, v, v];
        }
        let f = |k: f32| {
            let t = ((hue + k) % 6.0 - 3.0).abs() - 1.0;
            t.clamp(0.0, 1.0) * 0.85 + 0.15
        };
        [v * f(0.0), v * f(4.0), v * f(2.0)]
    })
}

fn compare(gpu: &Gpu, name: &str, s: &Settings, space: OutputSpace, scale: f32) {
    let base = test_image(480, 320);
    let mut cpu = base.clone();
    luma_image::finish(&mut cpu, scale, s, &Cancel::new(), &mut Timings::default()).unwrap();
    let cpu = luma_image::encode8(&cpu, space);
    assert!(gpu.can_finish(s, base.width, base.height, scale), "{name}");
    let out = gpu.finish(&base, 1, scale, s, space);
    let mut worst = 0u8;
    let mut sum = 0u64;
    for (c, g) in cpu.chunks(3).zip(out.chunks(4)) {
        for k in 0..3 {
            let d = c[k].abs_diff(g[k]);
            worst = worst.max(d);
            sum += u64::from(d);
        }
        assert_eq!(g[3], 255);
    }
    let mean = sum as f64 / cpu.len() as f64;
    eprintln!("{name:<28} écart max {worst}, moyen {mean:.4}");
    assert!(worst <= 2 && mean < 0.05, "{name} : écart max {worst}, moyen {mean}");
}

#[test]
fn every_per_pixel_stage_matches_the_cpu() {
    let Some(gpu) = Gpu::any() else {
        eprintln!("pas de GPU : test ignoré");
        return;
    };
    eprintln!("GPU : {}", gpu.name());
    let mut cases: Vec<(&str, Settings)> = vec![("défaut", Settings::default())];
    let mut s = Settings {
        profile: Profile::Neutral,
        ..Settings::default()
    };
    cases.push(("neutre (couleur sautée)", s.clone()));
    s = Settings {
        contrast: 60.0,
        whites: -30.0,
        blacks: 25.0,
        saturation: 40.0,
        ..Settings::default()
    };
    cases.push(("contraste, saturation", s));
    let mut s = Settings {
        vibrance: 70.0,
        ..Settings::default()
    };
    s.hsl.hue = [20.0, -40.0, 0.0, 60.0, 0.0, -80.0, 30.0, 0.0];
    s.hsl.saturation[5] = -60.0;
    s.hsl.luminance[1] = 40.0;
    cases.push(("vibrance, mélangeur", s));
    let mut s = Settings::default();
    s.color_grading.shadows = Wheel {
        hue: 220.0,
        saturation: 60.0,
        luminance: -20.0,
    };
    s.color_grading.highlights = Wheel {
        hue: 40.0,
        saturation: 40.0,
        luminance: 10.0,
    };
    s.color_grading.global = Wheel {
        hue: 300.0,
        saturation: 10.0,
        luminance: 0.0,
    };
    s.color_grading.balance = 30.0;
    s.color_grading.blending = 70.0;
    cases.push(("étalonnage", s));
    let mut s = Settings {
        profile: Profile::Monochrome,
        ..Settings::default()
    };
    s.tone_curve.highlights = -30.0;
    s.tone_curve.shadows = 20.0;
    s.tone_curve.red = vec![(0.0, 0.0), (128.0, 150.0), (255.0, 255.0)];
    s.hsl.luminance = [-80.0, -40.0, 0.0, 30.0, 20.0, 60.0, 0.0, -10.0];
    cases.push(("monochrome, courbes, mélange", s));
    let mut s = Settings::default();
    s.effects.vignette_amount = -60.0;
    s.effects.vignette_roundness = -50.0;
    s.effects.vignette_highlights = 40.0;
    s.effects.grain_amount = 50.0;
    s.effects.grain_roughness = 70.0;
    cases.push(("vignetage, grain", s));
    let mut s = Settings::default();
    s.effects.vignette_amount = 40.0;
    s.effects.vignette_roundness = 60.0;
    cases.push(("vignetage blanc rond", s));
    for (name, s) in &cases {
        compare(&gpu, name, s, OutputSpace::Srgb, 0.25);
    }
    compare(&gpu, "Display P3", &cases[3].1, OutputSpace::DisplayP3, 0.25);
    compare(&gpu, "ProPhoto", &cases[2].1, OutputSpace::ProPhoto, 0.25);
    // Les 100 films, tels qu'ils sont livrés.
    for p in luma_project::presets::PRESETS {
        compare(
            &gpu,
            p.name,
            &p.applied(&Settings::default(), 1.0),
            OutputSpace::Srgb,
            0.25,
        );
    }
}

#[test]
fn sharpening_at_full_size_stays_on_the_cpu() {
    let Some(gpu) = Gpu::any() else { return };
    let s = Settings::default();
    assert!(gpu.can_finish(&s, 100, 100, 0.3));
    assert!(!gpu.can_finish(&s, 100, 100, 1.0), "accentuation visible à 100 %");
}
