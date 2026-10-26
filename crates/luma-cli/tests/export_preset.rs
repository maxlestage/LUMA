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

    // L'accentuation de sortie.
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--output-sharpen", "matte:high"]);
    assert!(ok && out.contains("accentuation papier mat, forte"), "{out}");
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--output-sharpen", "toile"]);
    assert!(!ok && out.contains("accentuation inconnue"), "{out}");
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--output-sharpen", "none"]);
    assert!(ok && !out.contains("accentuation"), "{out}");

    // Le filigrane : créé, réglé, puis retiré.
    let (ok, out) = luma(
        &file,
        &[
            "export-preset",
            "Galerie",
            "--watermark",
            "© Moi",
            "--watermark-position",
            "haut-gauche",
            "--watermark-size",
            "5",
        ],
    );
    assert!(
        ok && out.contains("filigrane « © Moi », en haut à gauche, 5 %, 70 %"),
        "{out}"
    );
    let (ok, out) = luma(
        &file,
        &[
            "export-preset",
            "Galerie",
            "--watermark-opacity",
            "40",
            "--watermark-dark",
        ],
    );
    assert!(ok && out.contains("en haut à gauche, 5 %, 40 %, noir"), "{out}");
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--watermark-position", "dessus"]);
    assert!(!ok && out.contains("emplacement inconnu"), "{out}");
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--watermark-size", "90"]);
    assert!(!ok && out.contains("0.5 à 25"), "{out}");
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--no-watermark"]);
    assert!(ok && !out.contains("filigrane"), "{out}");

    // Le logo : un fichier qui existe, gardé avec son chemin complet.
    let logo = dir.join("logo.png");
    let mut png = image::RgbaImage::new(40, 20);
    for (x, _, p) in png.enumerate_pixels_mut() {
        *p = image::Rgba(if x < 20 { [255, 255, 255, 255] } else { [0, 0, 0, 0] });
    }
    png.save(&logo).unwrap();
    let (ok, out) = luma(
        &file,
        &[
            "export-preset",
            "Galerie",
            "--logo",
            logo.to_str().unwrap(),
            "--logo-position",
            "top-right",
            "--logo-opacity",
            "50",
        ],
    );
    assert!(
        ok && out.contains("logo logo.png, en haut à droite, 12 %, 50 %"),
        "{out}"
    );
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--logo", "absent.png"]);
    assert!(!ok && out.contains("pas de fichier"), "{out}");
    let (ok, out) = luma(&file, &["export-preset", "Galerie", "--no-logo"]);
    assert!(ok && !out.contains("logo"), "{out}");

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

    // L'accentuation de sortie change l'image, pas sa taille.
    let plain = dir.join("sans.png");
    let sharp = dir.join("avec.png");
    for (out_file, extra) in [(&plain, None), (&sharp, Some("screen:high"))] {
        let mut args = vec![
            "develop",
            raw.to_str().unwrap(),
            "--no-sidecar",
            "--resize",
            "300",
            "-o",
            out_file.to_str().unwrap(),
        ];
        if let Some(e) = extra {
            args.extend(["--output-sharpen", e]);
        }
        let (ok, out) = luma(&file, &args);
        assert!(ok, "{out}");
    }
    let (a, b) = (std::fs::read(&plain).unwrap(), std::fs::read(&sharp).unwrap());
    assert!(a != b && a.len().abs_diff(b.len()) < a.len());

    // Le filigrane marque le coin demandé, et lui seul.
    let marked = dir.join("filigrane.png");
    let (ok, out) = luma(
        &file,
        &[
            "develop",
            raw.to_str().unwrap(),
            "--no-sidecar",
            "--resize",
            "300",
            "--watermark",
            "LÜMA",
            "--watermark-size",
            "10",
            "-o",
            marked.to_str().unwrap(),
        ],
    );
    assert!(ok, "{out}");
    let (a, b) = (png_rgb(&plain), png_rgb(&marked));
    assert_eq!((a.0, a.1), (b.0, b.1));
    let (w, h) = (a.0, a.1);
    let changed = |x0: usize, y0: usize| {
        (y0..y0 + h / 2).any(|y| (x0..x0 + w / 2).any(|x| a.2[(y * w + x) * 3..][..3] != b.2[(y * w + x) * 3..][..3]))
    };
    assert!(changed(w / 2, h / 2), "coin en bas à droite");
    assert!(!changed(0, 0) && !changed(w / 2, 0) && !changed(0, h / 2));

    // Le logo marque le coin en haut à gauche, et lui seul.
    let logo = dir.join("logo.png");
    image::RgbaImage::from_pixel(40, 20, image::Rgba([255, 0, 0, 255]))
        .save(&logo)
        .unwrap();
    let with_logo = dir.join("logo-pose.png");
    let (ok, out) = luma(
        &file,
        &[
            "develop",
            raw.to_str().unwrap(),
            "--no-sidecar",
            "--resize",
            "300",
            "--logo",
            logo.to_str().unwrap(),
            "--logo-position",
            "top-left",
            "-o",
            with_logo.to_str().unwrap(),
        ],
    );
    assert!(ok, "{out}");
    let c = png_rgb(&with_logo);
    let changed_logo = |x0: usize, y0: usize| {
        (y0..y0 + h / 2).any(|y| (x0..x0 + w / 2).any(|x| a.2[(y * w + x) * 3..][..3] != c.2[(y * w + x) * 3..][..3]))
    };
    assert!(changed_logo(0, 0));
    assert!(!changed_logo(w / 2, 0) && !changed_logo(0, h / 2) && !changed_logo(w / 2, h / 2));

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

/// Les pixels d'un PNG 8 bits RGB : largeur, hauteur, octets.
fn png_rgb(path: &Path) -> (usize, usize, Vec<u8>) {
    let img = image::open(path).unwrap().to_rgb8();
    (img.width() as usize, img.height() as usize, img.into_raw())
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
