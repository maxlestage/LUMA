//! `luma video` : un film sur chaque image d'une vidéo AVI en Motion JPEG.

use std::path::PathBuf;
use std::process::Command;

use luma_render::video::{self, Clip};

fn luma(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_luma")).args(args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luma-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Trois images : un dégradé coloré qui glisse.
fn sample() -> Vec<u8> {
    let (w, h) = (64, 40);
    let frames = (0..3)
        .map(|k| {
            let rgb: Vec<u8> = (0..w * h)
                .flat_map(|i| {
                    let (x, y) = (i % w, i / w);
                    [((x + k * 8) * 255 / (w + 16)) as u8, (y * 255 / h) as u8, 120]
                })
                .collect();
            luma_render::jpeg::encode(&rgb, w, h, 92, &[], &[]).unwrap()
        })
        .collect();
    video::write(&Clip::from_frames(w, h, 30_000, 1001, frames)).unwrap()
}

#[test]
fn a_film_on_every_frame() {
    let dir = scratch("video");
    let (input, plain, film) = (dir.join("clip.avi"), dir.join("neutre.avi"), dir.join("film.avi"));
    std::fs::write(&input, sample()).unwrap();
    let (ok, text) = luma(&["video", input.to_str().unwrap(), "-o", plain.to_str().unwrap()]);
    assert!(ok, "{text}");
    assert!(text.contains("3 images (64 × 40, 29.97 images/s"), "{text}");
    let (ok, text) = luma(&[
        "video",
        input.to_str().unwrap(),
        "-o",
        film.to_str().unwrap(),
        "--film",
        "Tri-X 400",
    ]);
    assert!(ok, "{text}");
    let (a, b) = (
        video::read(&std::fs::read(&plain).unwrap()).unwrap(),
        video::read(&std::fs::read(&film).unwrap()).unwrap(),
    );
    assert_eq!(
        (b.frame_count(), b.width, b.height, b.rate, b.scale),
        (3, 64, 40, 30_000, 1001)
    );
    // Sans réglage, l'image reste proche de l'original ; en noir et blanc, plus de couleur.
    let chroma = |c: &video::Chunk| {
        let img = image::load_from_memory(&c.data).unwrap().to_rgb8();
        img.pixels()
            .map(|p| f64::from(p[0].max(p[1]).max(p[2]) - p[0].min(p[1]).min(p[2])))
            .sum::<f64>()
            / f64::from(img.width() * img.height())
    };
    for (p, f) in a.frames().zip(b.frames()) {
        assert!(chroma(p) > 40.0, "{}", chroma(p));
        assert!(chroma(f) < 3.0, "{}", chroma(f));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn wrong_options_are_explained() {
    let dir = scratch("video-erreurs");
    let input = dir.join("clip.avi");
    std::fs::write(&input, sample()).unwrap();
    let i = input.to_str().unwrap();
    let o = dir.join("o.avi");
    let o = o.to_str().unwrap();
    for (args, says) in [
        (vec!["video", i], "-o sortie.avi"),
        (vec!["video", "-o", o], "il faut une vidéo"),
        (vec!["video", i, "-o", i], "un autre fichier"),
        (
            vec!["video", i, "-o", o, "--film", "Pellicule imaginaire"],
            "film inconnu",
        ),
        (vec!["video", "Cargo.toml", "-o", o], "pas une vidéo AVI"),
    ] {
        let (ok, text) = luma(&args);
        assert!(!ok && text.contains(says), "{args:?} : {text}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
