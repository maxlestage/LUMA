//! `luma lens` : la distorsion et le vignetage d'après le JPEG du boîtier.

use std::path::Path;
use std::process::Command;

#[test]
fn the_lens_is_measured_and_written() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    if !corpus.join("DSC_0750.NEF").exists() {
        eprintln!("corpus absent (scripts/fetch-corpus.sh) : essai ignoré");
        return;
    }
    let dir = std::env::temp_dir().join(format!("luma-cli-objectif-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Le NEF (aucune correction que LÜMA lise) et l'ARW (qui porte les siennes).
    for name in ["DSC_0750.NEF", "_DSC0009.ARW"] {
        std::fs::copy(corpus.join(name), dir.join(name)).unwrap();
    }
    let luma = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_luma"))
            .arg("lens")
            .args(args)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
        (o.status.success(), text)
    };
    let (nef, arw) = (dir.join("DSC_0750.NEF"), dir.join("_DSC0009.ARW"));
    // Mesurer seulement : rien d'écrit.
    let (ok, text) = luma(&[dir.to_str().unwrap()]);
    assert!(ok, "{text}");
    assert!(text.contains("DSC_0750.NEF : distorsion -3"), "{text}");
    assert!(
        text.contains("_DSC0009.ARW : corrections du boîtier déjà dans le fichier"),
        "{text}"
    );
    assert!(text.contains("1 photo(s) mesurée(s), 1 laissée(s) de côté"), "{text}");
    assert!(!luma_project::xmp::sidecar_path(&nef).exists());
    // Avec --write : le XMP du NEF (sa note gardée), pas celui de l'ARW.
    luma_project::xmp::save(
        &luma_project::xmp::sidecar_path(&nef),
        &luma_project::xmp::write_meta(&luma_project::xmp::Meta {
            rating: 4,
            ..Default::default()
        }),
    )
    .unwrap();
    let (ok, text) = luma(&["--write", nef.to_str().unwrap(), arw.to_str().unwrap()]);
    assert!(ok, "{text}");
    assert!(text.contains("1 photo(s) corrigée(s), 1 laissée(s) de côté"), "{text}");
    let s = luma_library::photo_settings(&nef).expect("les réglages écrits");
    assert!(s.lens.distortion < -15.0 && s.lens.vignetting > 15.0, "{:?}", s.lens);
    assert!(!s.crop.is_identity(), "les bords vides retirés");
    assert_eq!(luma_library::photo_meta(&nef).rating, 4);
    assert!(!luma_project::xmp::sidecar_path(&arw).exists());
    // Sans photo, ou une option inconnue : refusé.
    assert!(!luma(&[]).0);
    assert!(!luma(&["--rien"]).0);
    let _ = std::fs::remove_dir_all(&dir);
}
