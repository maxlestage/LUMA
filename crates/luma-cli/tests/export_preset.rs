//! `luma export-preset` et `luma develop --preset` : les préréglages d'export en ligne de
//! commande, dans un fichier à part (`LUMA_EXPORT_PRESETS`). Le développement d'un vrai
//! RAW n'a lieu qu'avec le corpus (`scripts/fetch-corpus.sh`).

use std::path::{Path, PathBuf};
use std::process::Command;

fn luma(file: &Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_luma"))
        .args(args)
        .env("LUMA_EXPORT_PRESETS", file)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("luma-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn presets_listed_created_and_deleted() {
    let dir = scratch("presets");
    let file = dir.join("Exports.json");
    let (ok, list) = luma(&file, &["export-preset"]);
    assert!(ok, "{list}");
    assert!(
        list.contains("Web") && list.contains("JPEG 85, sRGB, 2048 px, LÜMA/"),
        "{list}"
    );
    assert!(
        list.contains("Impression") && list.contains("TIFF 16 bits, ProPhoto RGB"),
        "{list}"
    );

    let (ok, out) = luma(
        &file,
        &[
            "export-preset",
            "Galerie",
            "--format",
            "png",
            "--bits",
            "16",
            "--space",
            "p3",
            "--size",
            "1600",
        ],
    );
    assert!(ok, "{out}");
    let (_, list) = luma(&file, &["export-preset"]);
    assert!(
        list.contains("Galerie") && list.contains("PNG 16 bits, Display P3, 1600 px"),
        "{list}"
    );

    // Modifier garde le reste.
    let (ok, out) = luma(&file, &["export-preset", "galerie", "--size", "0"]);
    assert!(ok, "{out}");
    let (_, list) = luma(&file, &["export-preset"]);
    assert!(list.contains("PNG 16 bits, Display P3, pleine définition"), "{list}");

    // Les tailles : petit côté, cadre, mégapixels ; une taille inconnue est refusée.
    for (size, shown) in [
        ("short:1080", "petit côté 1080 px"),
        ("1920x1080", "1920 × 1080 px"),
        ("12mp", "12 Mpx"),
    ] {
        let (ok, out) = luma(&file, &["export-preset", "Galerie", "--size", size]);
        assert!(ok && out.contains(shown), "{out}");
    }
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--size", "grand"]);
    assert!(!ok && out.contains("taille inconnue"), "{out}");
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--size", "0"]);
    assert!(ok && out.contains("pleine définition"), "{out}");

    // Le modèle de nom, vérifié.
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--name", "{date}_{séquence:3}"]);
    assert!(ok, "{out}");
    let (_, list) = luma(&file, &["export-preset"]);
    assert!(list.contains("pleine définition, LÜMA/, {date}_{séquence:3}"), "{list}");
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--name", "{taille}"]);
    assert!(!ok && out.contains("repère inconnu"), "{out}");

    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--quality", "abc"]);
    assert!(!ok && out.contains("--quality"), "{out}");
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--delete"]);
    assert!(ok, "{out}");
    let (ok, out) = luma(&file, &["export-preset", "Web", "--delete"]);
    assert!(!ok, "{out}");
    let (_, list) = luma(&file, &["export-preset"]);
    assert!(!list.contains("Galerie"), "{list}");
    let (ok, out) = luma(&file, &["develop", "x.ARW", "--preset", "Inconnu"]);
    assert!(!ok, "{out}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn develop_with_a_preset() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let Some(raw) = std::fs::read_dir(corpus)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e != "md"))
        .min()
    else {
        eprintln!("corpus vide : lancer scripts/fetch-corpus.sh");
        return;
    };
    let dir = scratch("develop");
    let file = dir.join("Exports.json");
    let folder = dir.join("sortie");
    let folder_arg = folder.to_str().unwrap();
    let (ok, out) = luma(
        &file,
        &["export-preset", "Petit", "--size", "300", "--folder", folder_arg],
    );
    assert!(ok, "{out}");
    let (ok, out) = luma(
        &file,
        &["develop", raw.to_str().unwrap(), "--no-sidecar", "--preset", "petit"],
    );
    assert!(ok, "{out}");
    let name = raw.with_extension("jpg");
    let img = image_size(&folder.join(name.file_name().unwrap()));
    assert_eq!(img.0.max(img.1), 300, "{out}");

    // Un modèle de nom : l'année de prise de vue en sous‑dossier, le boîtier, le rang.
    let (ok, out) = luma(
        &file,
        &[
            "develop",
            raw.to_str().unwrap(),
            "--no-sidecar",
            "--preset",
            "petit",
            "--name",
            "{année}/{boîtier}_{séquence}",
        ],
    );
    assert!(ok, "{out}");
    let years: Vec<PathBuf> = std::fs::read_dir(&folder)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    assert_eq!(years.len(), 1, "{out}");
    let year = years[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(year.len() == 4 && year.parse::<u32>().is_ok_and(|y| y > 1990), "{year}");
    let files: Vec<String> = std::fs::read_dir(&years[0])
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(files.len() == 1 && files[0].ends_with("_0001.jpg"), "{files:?}");

    // Petit côté, puis un cadre, puis des mégapixels : la taille exacte du JPEG.
    for (resize, check) in [
        (
            "short:200",
            Box::new(|w: usize, h: usize| w.min(h) == 200) as Box<dyn Fn(usize, usize) -> bool>,
        ),
        (
            "400x100",
            Box::new(|w: usize, h: usize| w <= 400 && h <= 100 && (w == 400 || h == 100)),
        ),
        (
            "0.1mp",
            Box::new(|w: usize, h: usize| (w * h).abs_diff(100_000) < 1_500),
        ),
    ] {
        let out_file = dir.join("taille.jpg");
        let (ok, out) = luma(
            &file,
            &[
                "develop",
                raw.to_str().unwrap(),
                "--no-sidecar",
                "--resize",
                resize,
                "-o",
                out_file.to_str().unwrap(),
            ],
        );
        assert!(ok, "{out}");
        let (w, h) = image_size(&out_file);
        assert!(check(w, h), "{resize} : {w} × {h}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// La taille d'un JPEG, lue dans son marqueur SOF.
fn image_size(path: &Path) -> (usize, usize) {
    let b = std::fs::read(path).unwrap();
    let mut i = 2;
    while i + 9 < b.len() {
        let marker = b[i + 1];
        let len = usize::from(b[i + 2]) << 8 | usize::from(b[i + 3]);
        if (0xC0..=0xC2).contains(&marker) {
            let h = usize::from(b[i + 5]) << 8 | usize::from(b[i + 6]);
            let w = usize::from(b[i + 7]) << 8 | usize::from(b[i + 8]);
            return (w, h);
        }
        i += 2 + len;
    }
    panic!("pas de SOF dans {}", path.display());
}
