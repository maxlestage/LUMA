//! `luma develop --before-after` : l'avant / après côte à côte, en une image.

use std::path::Path;
use std::process::Command;

#[test]
fn before_and_after_are_written_side_by_side() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    if !corpus.join("_DSC0009.ARW").exists() {
        eprintln!("corpus absent (scripts/fetch-corpus.sh) : essai ignoré");
        return;
    }
    let dir = std::env::temp_dir().join(format!("luma-cli-avant-apres-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let raw = dir.join("photo.ARW");
    std::fs::copy(corpus.join("_DSC0009.ARW"), &raw).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_luma"))
        .args([
            "develop",
            raw.to_str().unwrap(),
            "--before-after",
            "--size",
            "800",
            "--set",
            "exposure=1",
            "--no-sidecar",
        ])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{text}");
    let out = dir.join("photo-avant-apres.jpg");
    let img = image::open(&out).unwrap();
    // Deux photos de 800 px de large (paysage) et la bande entre elles.
    assert!(
        img.width() > 1600 && img.width() < 1700,
        "{} × {}",
        img.width(),
        img.height()
    );
    assert!(img.height() <= 800);
    let _ = std::fs::remove_dir_all(&dir);
}
