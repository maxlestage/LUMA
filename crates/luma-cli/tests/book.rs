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
        (
            vec!["book", "a.ARW", "-o", "l.pdf", "--pages", "1,3"],
            "--pages : « 3 »",
        ),
        (
            vec!["book", "a.ARW", "-o", "l.pdf", "--layout", "5"],
            "--layout : « 5 »",
        ),
        (vec!["slideshow", "a.ARW"], "-o diaporama"),
        (vec!["slideshow", "-o", "d"], "aucune photo"),
        (vec!["slideshow", "a.ARW", "-o", "d", "--seconds", "0"], "--seconds"),
        (vec!["gallery", "a.ARW"], "-o galerie"),
        (vec!["gallery", "-o", "g"], "aucune photo"),
        (vec!["gallery", "a.ARW", "-o", "g", "--thumb", "5"], "--thumb"),
    ] {
        let (ok, out) = luma(&args);
        assert!(!ok && out.contains(says), "{args:?} : {out}");
    }
}

/// Les premières pages choisies une à une : deux photos, puis une par page.
#[test]
fn a_book_with_pages_chosen_one_by_one() {
    let Some(photos) = corpus(&["_DSC0009.ARW", "DSC_0750.NEF", "AFXT2720.RAF", "P1000951.RW2"]) else {
        eprintln!("corpus absent : essai ignoré");
        return;
    };
    let dir = scratch("livre-pages");
    let pdf = dir.join("livre.pdf").display().to_string();
    let mut args = vec!["book", "-o", &pdf, "--dpi", "72", "--layout", "2", "--pages", "1"];
    args.extend(photos.iter().map(String::as_str));
    let (ok, out) = luma(&args);
    assert!(ok, "{out}");
    let text = String::from_utf8_lossy(&std::fs::read(&pdf).unwrap()).into_owned();
    let pages: usize = text
        .split("/Count ")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    // La couverture, une page de deux photos, une page d'une.
    assert_eq!(pages, 3, "{pages}");
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
        "--opening",
        "Pour Lou,\\nqui était là.",
        "--closing",
        "Islande, été 2026",
        "--pages",
        "1",
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
    // Une photo par page : la couverture, trois pages, l'ouverture et la dernière page.
    assert_eq!(pages, 6, "{pages}");

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

    // La galerie : la grille, les grandes photos et les vignettes, allégées pour le web.
    let site = dir.join("galerie");
    let mut args = vec![
        "gallery",
        "-o",
        site.to_str().unwrap(),
        "--size",
        "800",
        "--title",
        "Essais",
    ];
    args.extend(photos.iter().map(String::as_str));
    let (ok, out) = luma(&args);
    assert!(ok, "{out}");
    let page = std::fs::read_to_string(site.join("index.html")).unwrap();
    assert!(page.contains("<h1>Essais</h1>") && page.contains("alt=\"_DSC0009.ARW\""));
    for k in 1..=4 {
        for sub in ["photos", "vignettes"] {
            let jpeg = std::fs::read(site.join(format!("{sub}/{k:03}.jpg"))).unwrap();
            let sof = jpeg.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
            assert_eq!(jpeg[sof + 11], 0x22, "allégé pour le web : 4:2:0");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
