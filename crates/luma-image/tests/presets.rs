//! Les 275 films vérifiés par la machine, pas à l'œil : chacun est rendu sur une mire
//! (les 24 couleurs d'une charte, une rampe de gris, des tons chair et des couleurs
//! saturées), puis mesuré en OKLab (ΔE ×100, perceptuel).
//!
//! - chaque film se distingue nettement du rendu neutre ;
//! - aucun film n'est le quasi‑double d'un autre ;
//! - aucun n'écrase l'image (ni presque tout noir, ni presque tout blanc) ;
//! - les noir et blanc donnent des gris neutres ;
//!
//! La planche‑contact de la mire (le rendu neutre puis les 275 films, dans l'ordre, dix
//! par ligne) est écrite dans `target/planche-films.png` ; la CI la publie.

use luma_color::{self as lc, OkLab};
use luma_core::{Cancel, RgbImage, Timings};
use luma_project::presets::{Group, PRESETS};
use luma_project::{Profile, Settings};

/// La charte : 24 couleurs (sRGB 8 bits, valeurs de la ColorChecker classique), une rampe
/// de gris de −6 à +3 IL, et des couleurs pures.
fn chart() -> RgbImage {
    const PATCHES: [[u8; 3]; 24] = [
        [115, 82, 68],
        [194, 150, 130],
        [98, 122, 157],
        [87, 108, 67],
        [133, 128, 177],
        [103, 189, 170],
        [214, 126, 44],
        [80, 91, 166],
        [193, 90, 99],
        [94, 60, 108],
        [157, 188, 64],
        [224, 163, 46],
        [56, 61, 150],
        [70, 148, 73],
        [175, 54, 60],
        [231, 199, 31],
        [187, 86, 149],
        [8, 133, 161],
        [243, 243, 242],
        [200, 200, 200],
        [160, 160, 160],
        [122, 122, 121],
        [85, 85, 85],
        [52, 52, 52],
    ];
    let to_working = lc::rgb_to_rgb(&lc::SRGB, &lc::WORKING);
    let (w, h) = (240, 160);
    RgbImage::from_fn(w, h, |x, y| {
        if y < 120 {
            let (cx, cy) = (x / 40, y / 30);
            let p = PATCHES[cy * 6 + cx];
            return lc::apply(&to_working, p.map(|v| lc::srgb_decode(f32::from(v) / 255.0)));
        }
        if y < 140 {
            let v = 0.18 * ((x as f32 / w as f32) * 9.0 - 6.0).exp2();
            return [v, v, v];
        }
        let hue = x as f32 / w as f32 * 6.0;
        let f = |k: f32| (((hue + k) % 6.0 - 3.0).abs() - 1.0).clamp(0.0, 1.0) * 0.5 + 0.02;
        lc::apply(&to_working, [f(0.0), f(4.0), f(2.0)])
    })
}

fn render(s: &Settings) -> RgbImage {
    let mut img = chart();
    luma_image::finish(&mut img, 1.0, s, &Cancel::new(), &mut Timings::default()).unwrap();
    img
}

fn oklab(img: &RgbImage) -> Vec<[f32; 3]> {
    let ok = OkLab::for_working();
    img.data
        .iter()
        .map(|p| ok.from_rgb(p.map(|v| v.clamp(0.0, 1.0))))
        .collect()
}

/// La planche‑contact : dix rendus par ligne.
fn contact_sheet(images: &[RgbImage]) {
    let (w, h) = (images[0].width, images[0].height);
    let cols = 10;
    let rows = images.len().div_ceil(cols);
    let mut sheet = image::RgbImage::new((w * cols) as u32, (h * rows) as u32);
    for (k, img) in images.iter().enumerate() {
        let px = luma_image::encode8(img, luma_color::OutputSpace::Srgb);
        let (ox, oy) = ((k % cols) * w, (k / cols) * h);
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 3;
                sheet.put_pixel(
                    (ox + x) as u32,
                    (oy + y) as u32,
                    image::Rgb([px[i], px[i + 1], px[i + 2]]),
                );
            }
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/planche-films.png");
    // Une planche qui ne s'écrit pas (dossier en lecture seule) ne fait pas échouer le test.
    let _ = sheet.save(path);
}

fn delta(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(p, q)| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt())
        .sum::<f32>()
        / a.len() as f32
        * 100.0
}

#[test]
fn every_film_is_distinct_sane_and_true_to_its_family() {
    let mut images = vec![render(&Settings::default())];
    images.extend(PRESETS.iter().map(|p| render(&p.applied(&Settings::default(), 1.0))));
    contact_sheet(&images);
    let neutral = oklab(&images[0]);
    let renders: Vec<Vec<[f32; 3]>> = images[1..].iter().map(oklab).collect();
    let mut failures = Vec::new();
    for (p, r) in PRESETS.iter().zip(&renders) {
        let d = delta(r, &neutral);
        if d < 2.0 {
            failures.push(format!("{} : trop proche du rendu neutre (ΔE {d:.2})", p.name));
        }
        let mean_l = r.iter().map(|q| q[0]).sum::<f32>() / r.len() as f32;
        if !(0.3..=0.85).contains(&mean_l) {
            failures.push(format!("{} : luminosité moyenne {mean_l:.2}", p.name));
        }
        let crushed = r.iter().filter(|q| q[0] < 0.03 || q[0] > 0.995).count() as f32 / r.len() as f32;
        if crushed > 0.12 {
            failures.push(format!(
                "{} : {:.0} % de pixels bouchés ou brûlés",
                p.name,
                crushed * 100.0
            ));
        }
        let applied = p.applied(&Settings::default(), 1.0);
        let max_chroma = r
            .iter()
            .map(|q| (q[1] * q[1] + q[2] * q[2]).sqrt())
            .fold(0.0f32, f32::max);
        if applied.profile == Profile::Monochrome && max_chroma > 0.004 {
            failures.push(format!("{} : noir et blanc mais chroma {max_chroma:.4}", p.name));
        }
        if p.group == Group::BlackAndWhiteFilm && applied.profile != Profile::Monochrome {
            failures.push(format!("{} : famille noir et blanc sans profil monochrome", p.name));
        }
    }
    for i in 0..PRESETS.len() {
        for j in i + 1..PRESETS.len() {
            let d = delta(&renders[i], &renders[j]);
            if d < 1.0 {
                failures.push(format!(
                    "{} et {} : presque identiques (ΔE {d:.2})",
                    PRESETS[i].name, PRESETS[j].name
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} problème(s) :\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// L'orthochromatique ne voit pas le rouge (il sort presque noir) ; le Retro 80S, sensible
/// au rouge lointain, assombrit fortement le bleu.
#[test]
fn spectral_black_and_white_films_behave_like_their_emulsions() {
    let lightness = |name: &str, srgb: [f32; 3]| {
        let p = PRESETS.iter().find(|p| p.name == name).unwrap();
        let to_working = lc::rgb_to_rgb(&lc::SRGB, &lc::WORKING);
        let c = lc::apply(&to_working, srgb.map(lc::srgb_decode));
        let mut img = RgbImage::from_fn(4, 4, |_, _| c);
        luma_image::finish(
            &mut img,
            1.0,
            &p.applied(&Settings::default(), 1.0),
            &Cancel::new(),
            &mut Timings::default(),
        )
        .unwrap();
        OkLab::for_working().from_rgb(img.data[5])[0]
    };
    let (red, blue) = ([0.75, 0.12, 0.12], [0.15, 0.25, 0.75]);
    let ortho = (lightness("Ortho Plus", red), lightness("Ortho Plus", blue));
    assert!(
        ortho.0 < ortho.1 - 0.2,
        "Ortho : rouge {:.2}, bleu {:.2}",
        ortho.0,
        ortho.1
    );
    let retro = (lightness("Retro 80S", red), lightness("Retro 80S", blue));
    assert!(
        retro.1 < retro.0 - 0.2,
        "Retro 80S : rouge {:.2}, bleu {:.2}",
        retro.0,
        retro.1
    );
}
