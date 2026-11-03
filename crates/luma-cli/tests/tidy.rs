//! `luma tidy` : les photos rejetées rangées avec leur XMP dans « Rejetées », rien
//! d'effacé ; `--dry-run` les nomme seulement.

use std::process::Command;

fn luma(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_luma")).args(args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

#[test]
fn rejects_are_tidied_away() {
    let dir = std::env::temp_dir().join(format!("luma-cli-ranger-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["garde.png", "rejet.png"] {
        image::RgbImage::from_pixel(32, 24, image::Rgb([90, 90, 90]))
            .save(dir.join(name))
            .unwrap();
    }
    // Le drapeau « rejetée » posé comme le fait luma sharpness --reject.
    let rejected = dir.join("rejet.png");
    let meta = luma_project::xmp::Meta {
        flag: luma_project::xmp::Flag::Reject,
        ..Default::default()
    };
    let xmp = luma_project::xmp::sidecar_path(&rejected);
    luma_project::xmp::save(
        &xmp,
        &luma_project::xmp::write_with_meta(&luma_project::Settings::default(), &meta),
    )
    .unwrap();
    let d = dir.to_str().unwrap();
    let (ok, out) = luma(&["tidy", d, "--dry-run"]);
    assert!(
        ok && out.contains("rejet.png : rejetée") && out.contains("1 photo(s) rejetée(s) à ranger"),
        "{out}"
    );
    assert!(rejected.is_file());
    let (ok, out) = luma(&["tidy", d]);
    assert!(
        ok && out.contains("1 photo(s) rejetée(s) rangée(s) dans « Rejetées », 0 laissée(s)"),
        "{out}"
    );
    let aside = dir.join("Rejetées");
    assert!(aside.join("rejet.png").is_file() && !rejected.exists());
    assert!(aside.join(xmp.file_name().unwrap()).is_file() && !xmp.exists());
    assert!(dir.join("garde.png").is_file());
    // Un nom de dossier qui serait un chemin est refusé.
    let (ok, out) = luma(&["tidy", d, "--folder", "../ailleurs"]);
    assert!(!ok && out.contains("est un nom, pas un chemin"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
