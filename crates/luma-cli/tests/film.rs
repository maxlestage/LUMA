//! `luma film-lut --camera` : le rendu du boîtier mesuré sur le JPEG rangé dans ses RAW.

use std::path::Path;
use std::process::Command;

#[test]
fn the_camera_look_is_measured_into_a_lut() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let raw = corpus.join("_DSC0009.ARW");
    if !raw.exists() {
        eprintln!("corpus absent (scripts/fetch-corpus.sh) : essai ignoré");
        return;
    }
    let dir = std::env::temp_dir().join(format!("luma-cli-boitier-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("Sony.cube");
    let luma = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_luma"))
            .arg("film-lut")
            .args(args)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
        (o.status.success(), text)
    };
    let (ok, text) = luma(&["--camera", raw.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert!(ok, "{text}");
    assert!(text.contains("numérique → boîtier"), "{text}");
    let cube = luma_image::cube::Cube::parse(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(cube.size, 33);
    // Sans RAW, ou une image sans JPEG du boîtier : refusé, rien d'écrit.
    assert!(!luma(&["--camera"]).0);
    let png = dir.join("a.png");
    image::RgbImage::from_pixel(32, 24, image::Rgb([90, 120, 150]))
        .save(&png)
        .unwrap();
    let (ok, text) = luma(&["--camera", png.to_str().unwrap()]);
    assert!(!ok && text.contains("pas de JPEG du boîtier"), "{text}");
    assert!(!png.with_extension("cube").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
