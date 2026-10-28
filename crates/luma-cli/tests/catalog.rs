//! `luma catalog --duplicates` : les photos en double du catalogue, sans rien effacer.

use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

fn luma(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_luma")).args(args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

#[test]
fn duplicates_are_listed_and_kept() {
    let dir = std::env::temp_dir().join(format!("luma-cli-doublons-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["carte", "copie"] {
        std::fs::create_dir_all(dir.join("photos").join(sub)).unwrap();
    }
    let png = |rel: &str, w: u32| {
        let p = dir.join("photos").join(rel);
        image::RgbImage::from_pixel(w, 8, image::Rgb([90, 120, 200]))
            .save(&p)
            .unwrap();
        // Même date de fichier : sans EXIF, c'est elle qui date la photo.
        std::fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(1_780_000_000))
            .unwrap();
        p
    };
    let a = png("carte/IMG_0001.png", 16);
    let b = png("copie/IMG_0001.png", 16);
    let other = png("carte/IMG_0002.png", 32);
    let cat = dir.join("cat.lumacat");
    let (ok, text) = luma(&[
        "catalog",
        dir.join("photos").to_str().unwrap(),
        "--catalog",
        cat.to_str().unwrap(),
        "--duplicates",
    ]);
    assert!(ok, "{text}");
    assert!(text.contains("1 photos en double"), "{text}");
    assert!(
        text.contains("carte/IMG_0001.png") && text.contains("copie/IMG_0001.png"),
        "{text}"
    );
    assert!(!text.contains("IMG_0002"), "{text}");
    // Rien n'est effacé.
    assert!(a.is_file() && b.is_file() && other.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}
