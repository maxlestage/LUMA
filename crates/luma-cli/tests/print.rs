//! `luma print` : la mise en page pour l'imprimante, en PDF. Les erreurs se vérifient
//! sans photo ; l'impression d'un vrai RAW n'a lieu qu'avec le corpus
//! (`scripts/fetch-corpus.sh`).

use std::path::{Path, PathBuf};
use std::process::Command;

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

fn corpus(name: &str) -> Option<String> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/raw-corpus")
        .join(name);
    p.is_file().then(|| p.display().to_string())
}

#[test]
fn wrong_options_are_explained() {
    let dir = scratch("print-erreurs");
    let pdf = dir.join("t.pdf").display().to_string();
    for (args, says) in [
        (vec!["print", "a.ARW"], "-o tirage.pdf"),
        (vec!["print", "-o", &pdf], "aucune photo"),
        (vec!["print", "a.ARW", "-o", &pdf, "--grid", "0x3"], "colonnes x lignes"),
        (vec!["print", "a.ARW", "-o", &pdf, "--paper", "géant"], "format inconnu"),
        (vec!["print", "a.ARW", "-o", &pdf, "--margin", "120"], "plus de place"),
        (vec!["print", "a.ARW", "-o", &pdf, "--intent", "vive"], "--intent"),
        (vec!["print", "a.ARW", "-o", &pdf, "--text"], "--text attend un texte"),
    ] {
        let (ok, out) = luma(&args);
        assert!(!ok && out.contains(says), "{args:?} : {out}");
    }
    // Un profil CMJN qui ne sait aller que vers le Lab (pas d'impression possible) :
    // refusé.
    let cmyk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../luma-color/tests/profils/CGATS001Compat-v2-micro.icc");
    let (ok, out) = luma(&["print", "a.ARW", "-o", &pdf, "--profile", &cmyk.display().to_string()]);
    assert!(!ok && out.contains("dans les deux sens"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn contact_sheet_and_print_with_a_profile() {
    let (Some(sony), Some(nikon), Some(fuji)) =
        (corpus("_DSC0009.ARW"), corpus("DSC_0750.NEF"), corpus("AFXT2720.RAF"))
    else {
        eprintln!("corpus absent : essai ignoré");
        return;
    };
    let dir = scratch("print");
    // Trois photos sur une planche de deux par page : deux pages.
    let sheet = dir.join("planche.pdf");
    let (ok, out) = luma(&[
        "print",
        &sony,
        &nikon,
        &fuji,
        "-o",
        &sheet.display().to_string(),
        "--grid",
        "2x1",
        "--captions",
        "--paper",
        "13x18",
        "--dpi",
        "150",
        "--crop-marks",
        "--text",
        "© Maxime Lestage 2026",
    ]);
    assert!(ok, "{out}");
    assert!(out.contains("2 page(s) de 127 × 178 mm, 3 photo(s)"), "{out}");
    let bytes = std::fs::read(&sheet).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(bytes.starts_with(b"%PDF-1.4") && text.contains("/Count 2"));
    assert!(text.contains("/MediaBox [0 0 360.00 504.00]"), "13 × 18 cm en points");
    // Un tirage en paysage, converti vers un profil RVB (ici le Display P3).
    let icc = dir.join("imprimante.icc");
    std::fs::write(&icc, luma_color::icc::profile(luma_color::OutputSpace::DisplayP3)).unwrap();
    let print = dir.join("tirage.pdf");
    let (ok, out) = luma(&[
        "print",
        &sony,
        "-o",
        &print.display().to_string(),
        "--landscape",
        "--fill",
        "--margin",
        "0",
        "--dpi",
        "100",
        "--profile",
        &icc.display().to_string(),
        "--output-sharpen",
        "glossy",
    ]);
    assert!(ok, "{out}");
    assert!(out.contains("1 page(s) de 297 × 210 mm"), "{out}");
    let text = String::from_utf8_lossy(&std::fs::read(&print).unwrap()).into_owned();
    // Remplie sans marge : l'image fait toute la page, 1169 × 827 pixels à 100 ppp.
    assert!(
        text.contains("/Width 1169 /Height 827"),
        "{}",
        &text[..600.min(text.len())]
    );
    // Les pages en JPEG : une image par page, nommée d'après le fichier demandé.
    let jpeg = dir.join("planche.jpg");
    let (ok, out) = luma(&[
        "print",
        &sony,
        &nikon,
        &fuji,
        "-o",
        &jpeg.display().to_string(),
        "--grid",
        "2x1",
        "--dpi",
        "72",
    ]);
    assert!(ok, "{out}");
    assert!(
        dir.join("planche-1.jpg").is_file() && dir.join("planche-2.jpg").is_file(),
        "{out}"
    );
    assert!(!jpeg.exists(), "deux pages : deux fichiers numérotés");
    assert!(out.contains("planche-1.jpg") && out.contains("planche-2.jpg"), "{out}");
    // Vers une imprimante CMJN : un PDF en CMJN.
    let ink = Path::new(env!("CARGO_MANIFEST_DIR")).join("../luma-color/tests/profils/encre-noire.icc");
    let press = dir.join("presse.pdf");
    let (ok, out) = luma(&[
        "print",
        &sony,
        "-o",
        &press.display().to_string(),
        "--dpi",
        "72",
        "--profile",
        &ink.display().to_string(),
    ]);
    assert!(ok, "{out}");
    let text = String::from_utf8_lossy(&std::fs::read(&press).unwrap()).into_owned();
    assert!(
        text.contains("/ColorSpace /DeviceCMYK"),
        "{}",
        &text[..600.min(text.len())]
    );
    let _ = std::fs::remove_dir_all(&dir);
}
