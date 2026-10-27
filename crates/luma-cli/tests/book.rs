//! `luma book` et `luma slideshow` : le livre photo en PDF et le diaporama autonome.
//! Les erreurs se vérifient sans photo ; les vrais RAW n'entrent qu'avec le corpus
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

fn corpus(names: &[&str]) -> Option<Vec<String>> {
    names
        .iter()
        .map(|n| {
            let p = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/raw-corpus")
                .join(n);
            p.is_file().then(|| p.display().to_string())
        })
        .collect()
}

#[test]
fn wrong_options_are_explained() {
    for (args, says) in [
        (vec!["book", "a.ARW"], "-o livre.pdf"),
        (vec!["book", "-o", "l.pdf"], "aucune photo"),
        (
            vec!["book", "a.ARW", "-o", "l.pdf", "--format", "géant"],
            "format inconnu",
        ),
        (vec!["book", "a.ARW", "-o", "l.pdf", "--margin", "99"], "plus de place"),
        (vec!["slideshow", "a.ARW"], "-o diaporama"),
        (vec!["slideshow", "-o", "d"], "aucune photo"),
        (vec!["slideshow", "a.ARW", "-o", "d", "--seconds", "0"], "--seconds"),
    ] {
        let (ok, out) = luma(&args);
        assert!(!ok && out.contains(says), "{args:?} : {out}");
    }
}

#[test]
fn a_book_and_a_slideshow() {
    let Some(photos) = corpus(&["_DSC0009.ARW", "DSC_0750.NEF", "AFXT2720.RAF", "P1000951.RW2"]) else {
        eprintln!("corpus absent : essai ignoré");
        return;
    };
    let dir = scratch("livre");
    let pdf = dir.join("livre.pdf").display().to_string();
    let mut args = vec![
        "book",
        "-o",
        &pdf,
        "--title",
        "Essais",
        "--subtitle",
        "Corpus",
        "--dpi",
        "72",
    ];
    args.extend(photos.iter().map(String::as_str));
    let (ok, out) = luma(&args);
    assert!(ok, "{out}");
    assert!(out.contains("de 200 × 200 mm, 4 photo(s)"), "{out}");
    let text = String::from_utf8_lossy(&std::fs::read(&pdf).unwrap()).into_owned();
    assert!(text.contains("/MediaBox [0 0 566.93 566.93]"), "20 cm en points");
    // La couverture, puis au moins une page de plus.
    let pages: usize = text
        .split("/Count ")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!((2..=4).contains(&pages), "{pages}");

    let show = dir.join("diaporama");
    let mut args = vec![
        "slideshow",
        "-o",
        show.to_str().unwrap(),
        "--size",
        "640",
        "--captions",
        "--title",
        "Essais",
    ];
    args.extend(photos.iter().map(String::as_str));
    let (ok, out) = luma(&args);
    assert!(ok, "{out}");
    let page = std::fs::read_to_string(show.join("index.html")).unwrap();
    assert!(page.contains("_DSC0009.ARW") && page.contains("<title>Essais</title>"));
    for k in 1..=4 {
        let jpeg = std::fs::read(show.join(format!("photos/{k:03}.jpg"))).unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8]));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
