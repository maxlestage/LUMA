//! Les fusions sur des scènes dont on connaît la vérité : une vraie photo du corpus
//! découpée en vues qui se recouvrent (panorama), ou dupliquée avec du bruit et de
//! petits bougés (empilement), et un ciel étoilé synthétique qui tourne.

use std::path::Path;
use std::sync::Arc;

use luma_core::RgbImage;
use luma_merge::geometry::Homography;
use luma_merge::pano::panorama_images;
use luma_merge::stack::stack_images;
use luma_merge::{Projection, StackMode};
use luma_render::{Photo, Size};

/// La scène : la photo Sony du corpus, ou à défaut une texture synthétique.
fn scene(long_side: usize) -> RgbImage {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus/_DSC0009.ARW");
    match Photo::open(&path) {
        Ok(p) => (*p.source(Size::Fit(long_side), &mut Default::default())).clone(),
        Err(_) => RgbImage::from_fn(long_side, long_side * 2 / 3, |x, y| {
            let (fx, fy) = (x as f32 * 0.05, y as f32 * 0.07);
            let v = 0.3 + 0.2 * (fx.sin() * fy.cos()) + 0.1 * ((fx * 3.1).sin() * (fy * 2.3).sin());
            [v, v * 0.9, v * 0.7]
        }),
    }
}

/// Une vue de la scène : pour chaque pixel de la vue, `map` donne sa position dans la
/// scène.
fn view(scene: &RgbImage, w: usize, h: usize, map: &Homography, noise: f32, seed: u64) -> RgbImage {
    RgbImage::from_fn(w, h, |x, y| {
        let (u, v) = map.apply((x as f64, y as f64)).unwrap();
        let mut p = luma_merge::frames::sample(scene, u, v).unwrap_or([0.0; 3]);
        if noise > 0.0 {
            let mut k = (x as u64 * 7919 + y as u64 * 104_729 + seed * 1_299_709).wrapping_mul(0x9E37_79B9_7F4A_7C15);
            for c in &mut p {
                k ^= k >> 29;
                k = k.wrapping_mul(0xBF58_476D_1CE4_E5B9);
                *c = (*c + ((k >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * noise).max(0.0);
            }
        }
        p
    })
}

fn rms(a: &RgbImage, b: &RgbImage, margin: usize) -> f32 {
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for y in margin..a.height - margin {
        for x in margin..a.width - margin {
            let (p, q) = (a.data[y * a.width + x], b.data[y * b.width + x]);
            for c in 0..3 {
                sum += f64::from((p[c] - q[c]).powi(2));
                n += 1;
            }
        }
    }
    (sum / n as f64).sqrt() as f32
}

#[test]
fn mean_stack_aligns_and_lowers_the_noise() {
    let scene = scene(900);
    let (w, h) = (scene.width - 60, scene.height - 60);
    let shifts = [
        (30.0, 30.0, 0.0),
        (27.5, 33.2, 0.004),
        (33.1, 28.4, -0.003),
        (31.0, 26.0, 0.002),
        (28.2, 31.7, -0.001),
    ];
    let frames: Vec<Arc<RgbImage>> = shifts
        .iter()
        .enumerate()
        .map(|(i, &(dx, dy, a))| {
            Arc::new(view(
                &scene,
                w,
                h,
                &Homography::similarity(1.0, a, dx, dy),
                0.08,
                i as u64,
            ))
        })
        .collect();
    let truth = view(
        &scene,
        w,
        h,
        &Homography::similarity(1.0, shifts[2].2, shifts[2].0, shifts[2].1),
        0.0,
        0,
    );
    let merged = stack_images(frames.clone(), StackMode::Mean, true).unwrap();
    assert_eq!(merged.used, 5);
    let single = rms(&frames[2], &truth, 20);
    let stacked = rms(&merged.image, &truth, 20);
    eprintln!("bruit : une pose {single:.4}, empilement {stacked:.4}");
    assert!(stacked < single * 0.6, "{stacked} / {single}");
    // Les bords non couverts par toutes les poses sont rognés.
    let c = merged.crop.expect("rognage proposé");
    assert!(c.right < 1.0 && c.bottom <= 1.0 && c.right - c.left > 0.9, "{c:?}");
}

#[test]
fn median_removes_what_moves() {
    let scene = scene(600);
    let (w, h) = (scene.width, scene.height);
    let mut frames: Vec<Arc<RgbImage>> = (0..5)
        .map(|i| Arc::new(view(&scene, w, h, &Homography::IDENTITY, 0.02, i)))
        .collect();
    // Un « passant » blanc dans une seule pose.
    let mut intruder = (*frames[1]).clone();
    for y in 100..160 {
        for x in 200..240 {
            intruder.data[y * w + x] = [1.0; 3];
        }
    }
    frames[1] = Arc::new(intruder);
    let merged = stack_images(frames, StackMode::Median, false).unwrap();
    let p = merged.image.data[130 * w + 220];
    let truth = scene.data[130 * w + 220];
    assert!(
        (p[1] - truth[1]).abs() < 0.05,
        "le passant a disparu : {p:?} / {truth:?}"
    );
}

#[test]
fn star_stack_follows_the_rotating_sky_and_rejects_a_satellite() {
    let (w, h) = (700, 460);
    let mut stars = Vec::new();
    let mut k = 12345u64;
    for _ in 0..180 {
        let mut r = || {
            k ^= k << 13;
            k ^= k >> 7;
            k ^= k << 17;
            (k % 100_000) as f64 / 100_000.0
        };
        stars.push((
            r() * w as f64 * 1.4 - 0.2 * w as f64,
            r() * h as f64 * 1.4 - 0.2 * h as f64,
            0.05 + 0.5 * r() as f32,
        ));
    }
    let sky = |frame: usize| -> RgbImage {
        // Le ciel tourne d'environ 0,15° par pose autour d'un point hors champ.
        let m = Homography::translation(-1500.0, 2200.0)
            .then(&Homography::similarity(1.0, 0.0026 * frame as f64, 0.0, 0.0))
            .then(&Homography::translation(1500.0, -2200.0));
        RgbImage::from_fn(w, h, |x, y| {
            let mut v = 0.02f32 + ((x * 31 + y * 17 + frame * 7) % 13) as f32 * 0.0006;
            for &(sx, sy, f) in &stars {
                let (u, vv) = m.apply((sx, sy)).unwrap();
                let d2 = (x as f64 - u).powi(2) + (y as f64 - vv).powi(2);
                if d2 < 25.0 {
                    v += f * (-d2 / 2.9) as f32 * 0.0 + f * (-d2 / 2.9).exp() as f32;
                }
            }
            [v, v, v * 1.1]
        })
    };
    let mut frames: Vec<Arc<RgbImage>> = (0..7).map(|i| Arc::new(sky(i))).collect();
    // Un satellite traverse la pose 2.
    let mut sat = (*frames[2]).clone();
    for x in 50..650 {
        let y = 300 - x / 4;
        sat.data[y * w + x] = [0.9; 3];
    }
    frames[2] = Arc::new(sat);
    let merged = stack_images(frames.clone(), StackMode::Stars, true).unwrap();
    assert_eq!(merged.used, 7, "toutes les poses alignées");
    // La traînée du satellite est rejetée.
    let p = merged.image.data[(300 - 300 / 4) * w + 300];
    assert!(p[0] < 0.3, "satellite rejeté : {p:?}");
    // Les étoiles restent fines : le maximum local reste élevé (alignement juste).
    let reference = &frames[3];
    let peak = |img: &RgbImage| img.data.iter().map(|p| p[1]).fold(0.0f32, f32::max);
    assert!(
        peak(&merged.image) > 0.7 * peak(reference),
        "{} / {}",
        peak(&merged.image),
        peak(reference)
    );
}

#[test]
fn panorama_from_three_rotated_views() {
    let scene = scene(1500);
    let (sw, sh) = (scene.width as f64, scene.height as f64);
    // Trois vues de 40 % de la largeur, qui se recouvrent d'un tiers, avec un léger
    // basculement de perspective (la caméra tourne).
    let (w, h) = ((sw * 0.42) as usize, (sh * 0.8) as usize);
    let views: Vec<Arc<RgbImage>> = [0.0, 0.29, 0.58]
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let tilt = (i as f64 - 1.0) * 2e-5;
            let m = Homography([[1.0, 0.0, start * sw], [0.0, 1.0, sh * 0.1], [tilt, 0.0, 1.0]]);
            Arc::new(view(&scene, w, h, &m, 0.0, 0))
        })
        .collect();
    for projection in [Projection::Perspective, Projection::Cylindrical] {
        let merged = panorama_images(views.clone(), projection).unwrap();
        let (pw, ph) = (merged.image.width, merged.image.height);
        eprintln!("{projection:?} : {pw} × {ph}, rognage {:?}", merged.crop);
        assert!(pw as f64 > 1.6 * w as f64 && (pw as f64) < 1.15 * sw, "largeur {pw}");
        assert!((ph as f64) < 1.3 * h as f64);
        let covered = merged.covered.iter().filter(|c| **c).count() as f64 / merged.covered.len() as f64;
        assert!(covered > 0.85, "couverture {covered}");
    }
}

#[test]
fn raw_files_merge_into_a_dng_that_develops() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus/_DSC0009.ARW");
    if !src.exists() {
        return;
    }
    let dir = std::env::temp_dir().join("luma-merge-dng");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let paths: Vec<_> = (0..3)
        .map(|i| {
            let p = dir.join(format!("pose{i}.ARW"));
            std::fs::copy(&src, &p).unwrap();
            p
        })
        .collect();
    let frames = luma_merge::RawFiles {
        paths: paths.clone(),
        size: Size::Fit(1200),
    };
    let merged = luma_merge::stack(&frames, StackMode::Mean, true, &|_, _| {}, &luma_core::Cancel::new()).unwrap();
    assert_eq!(merged.used, 3);
    let template = luma_raw::RawPhoto::open(&paths[1]).unwrap();
    let out = luma_merge::output_path(&paths[0], "Empilement");
    merged.save(&out, &template, true).unwrap();
    assert!(out.ends_with("pose0-Empilement.dng"));
    assert!(out.with_extension("xmp").exists());
    // Le DNG se rouvre et se développe comme l'original, au même rendu.
    let dng = Photo::open(&out).unwrap();
    let original = Photo::open(&paths[1]).unwrap();
    let s = luma_project::Settings::default();
    let a = dng.render(&s, Size::Fit(300), &luma_core::Cancel::new()).unwrap().image;
    let b = original
        .render(&s, Size::Fit(300), &luma_core::Cancel::new())
        .unwrap()
        .image;
    assert_eq!((a.width, a.height), (b.width, b.height));
    let diff = rms(&a, &b, 4);
    eprintln!("écart DNG / original : {diff:.4}");
    assert!(diff < 0.02, "{diff}");
}

/// L'écart moyen (en IL) à la vérité `truth`, sur les pixels dont la vérité (somme des
/// canaux) est entre `lo` et `hi` ; `img` est multipliée par `k`.
fn log_rms(img: &RgbImage, k: f32, truth: &RgbImage, lo: f32, hi: f32, margin: usize) -> f32 {
    let l = |p: [f32; 3]| p[0] + 2.0 * p[1] + p[2];
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for y in margin..img.height - margin {
        for x in margin..img.width - margin {
            let i = y * img.width + x;
            let t = l(truth.data[i]);
            if (lo..hi).contains(&t) {
                sum += f64::from(((l(img.data[i]) * k).max(1e-4) / t).log2().powi(2));
                n += 1;
            }
        }
    }
    assert!(n > 500, "trop peu de pixels entre {lo} et {hi} : {n}");
    (sum / n as f64).sqrt() as f32
}

#[test]
fn hdr_merges_brackets_without_ghosts() {
    // Une scène de grande dynamique : des ombres à 1 % de la saturation jusqu'à quatre
    // fois la saturation (ce que la pose la plus sombre enregistre encore).
    let base = scene(900);
    let radiance = RgbImage {
        width: base.width,
        height: base.height,
        data: base.data.iter().map(|p| p.map(|v| v * v * 4.0)).collect(),
    };
    let (w, h) = (base.width - 60, base.height - 60);
    // +2, −2 et 0 IL, en désordre, chacune un peu décalée (à main levée).
    let poses = [(4.0f32, 27.0, 31.0), (0.25, 30.0, 30.0), (1.0, 32.0, 28.0)];
    let clip = |img: RgbImage| RgbImage {
        width: img.width,
        height: img.height,
        data: img.data.iter().map(|p| p.map(|v| v.min(1.0))).collect(),
    };
    let mut frames: Vec<Arc<RgbImage>> = poses
        .iter()
        .enumerate()
        .map(|(i, &(e, dx, dy))| {
            let exposed = RgbImage {
                width: radiance.width,
                height: radiance.height,
                data: radiance.data.iter().map(|p| p.map(|v| v * e)).collect(),
            };
            Arc::new(clip(view(
                &exposed,
                w,
                h,
                &Homography::translation(dx, dy),
                0.02,
                i as u64,
            )))
        })
        .collect();
    // Un passant clair dans la pose la plus exposée seulement.
    let mut ghost = (*frames[0]).clone();
    for y in 200..260 {
        for x in 300..340 {
            ghost.data[y * w + x] = [0.9; 3];
        }
    }
    frames[0] = Arc::new(ghost);
    let truth = view(&radiance, w, h, &Homography::translation(32.0, 28.0), 0.0, 0);
    let merged = stack_images(frames.clone(), StackMode::Hdr, true).unwrap();
    assert_eq!(merged.used, 3);
    assert!(merged.exposure > 0.5, "ramenée sous le blanc : {} IL", merged.exposure);
    let k = merged.exposure.exp2();
    // Les lumières que la référence (0 IL) sature, et les ombres qu'elle noie dans le
    // bruit mais que la pose à +2 IL voit encore.
    let (lights, shadows) = ((4.4, 16.0), (0.02, 0.08));
    let single = (
        log_rms(&frames[2], 1.0, &truth, lights.0, lights.1, 40),
        log_rms(&frames[2], 1.0, &truth, shadows.0, shadows.1, 40),
    );
    let fused = (
        log_rms(&merged.image, k, &truth, lights.0, lights.1, 40),
        log_rms(&merged.image, k, &truth, shadows.0, shadows.1, 40),
    );
    eprintln!(
        "écart à la scène (IL) : lumières {:.3} → {:.3}, ombres {:.3} → {:.3}",
        single.0, fused.0, single.1, fused.1
    );
    assert!(fused.0 < single.0 * 0.3, "lumières {fused:?} / {single:?}");
    assert!(fused.1 < single.1 * 0.6, "ombres {fused:?} / {single:?}");
    // La référence est l'exposition médiane ; le passant n'y est pas, il ne reste pas.
    let at = |img: &RgbImage, k: f32| {
        let p = img.data[230 * w + 320];
        (p[0] + 2.0 * p[1] + p[2]) * k
    };
    let (got, want) = (at(&merged.image, k), at(&truth, 1.0));
    assert!((got / want).log2().abs() < 0.3, "fantôme : {got} au lieu de {want}");
}
