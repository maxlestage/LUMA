//! `luma import` : une carte mémoire (un faux dossier DCIM) importée dans une
//! photothèque et sur un second disque, puis importée de nouveau sans rien copier.

use std::path::PathBuf;
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

/// Tous les fichiers d'un dossier, sous‑dossiers compris, par nom.
fn files(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else {
                out.push(e.file_name().to_string_lossy().into_owned());
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_card_to_the_library_and_a_second_disk() {
    let root = scratch("import");
    let card = root.join("CARTE/DCIM/100MSDCF");
    std::fs::create_dir_all(&card).unwrap();
    for (name, bytes) in [
        ("DSC0001.ARW", "un"),
        ("DSC0001.JPG", "un, jpeg"),
        ("DSC0002.ARW", "deux"),
    ] {
        std::fs::write(card.join(name), bytes).unwrap();
    }
    let (lib, second, cat) = (root.join("Photos"), root.join("Second"), root.join("cat.lumacat"));
    let (lib_s, second_s, cat_s, card_s) = (
        lib.display().to_string(),
        second.display().to_string(),
        cat.display().to_string(),
        root.join("CARTE").display().to_string(),
    );
    let base = [
        "import",
        card_s.as_str(),
        "--to",
        &lib_s,
        "--backup",
        &second_s,
        "--catalog",
        &cat_s,
    ];
    let with = |extra: &[&'static str]| -> Vec<String> { base.iter().chain(extra).map(|s| s.to_string()).collect() };

    // À blanc : rien n'est copié.
    let args = with(&["--name", "voyage_{séquence:3}", "--dry-run"]);
    let (ok, out) = luma(&args.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(ok && out.contains("3 photo(s) à importer"), "{out}");
    assert!(!lib.exists());
    // Les jours de la carte (sans EXIF : celui des fichiers), et un autre jour : rien.
    assert!(out.contains("jours : ") && out.contains(" (3)"), "{out}");
    let args = with(&["--dry-run", "--days", "2000-01-01"]);
    let (ok, out) = luma(&args.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(ok && out.contains("0 photo(s) à importer"), "{out}");
    let args = with(&["--dry-run", "--days", "hier"]);
    let (ok, out) = luma(&args.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(!ok && out.contains("n'est pas un jour"), "{out}");

    let args = with(&["--name", "voyage_{séquence:3}"]);
    let (ok, out) = luma(&args.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(ok && out.contains("3 photo(s) importée(s) et vérifiée(s)"), "{out}");
    let names = ["voyage_001.ARW", "voyage_001.JPG", "voyage_002.ARW"];
    assert_eq!(files(&lib), names, "le RAW et son JPEG gardent un même nom");
    assert_eq!(files(&second), names);
    assert_eq!(
        files(&root.join("CARTE")),
        ["DSC0001.ARW", "DSC0001.JPG", "DSC0002.ARW"],
        "la carte reste intacte"
    );
    assert!(cat.is_file(), "la photothèque connaît les photos");

    // Une seconde fois : déjà importées.
    let (ok, out) = luma(&args.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(
        ok && out.contains("0 photo(s) à importer") && out.contains("3 déjà importée(s)"),
        "{out}"
    );

    let (ok, out) = luma(&["import", "--to", &lib_s, "/nulle/part"]);
    assert!(!ok && out.contains("aucune photo"), "{out}");
    let (ok, out) = luma(&["import", &card_s]);
    assert!(!ok && out.contains("--to"), "{out}");
    let _ = std::fs::remove_dir_all(&root);
}
