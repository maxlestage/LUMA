//! `luma catalog --duplicates` : les photos en double du catalogue, sans rien effacer.

use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

fn luma(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_luma")).args(args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

#[test]
fn duplicates_are_listed_and_kept() {
    let dir = std::env::temp_dir().join(format!("luma-cli-doublons-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["carte", "copie"] {
        std::fs::create_dir_all(dir.join("photos").join(sub)).unwrap();
    }
    let png = |rel: &str, w: u32| {
        let p = dir.join("photos").join(rel);
        image::RgbImage::from_pixel(w, 8, image::Rgb([90, 120, 200]))
            .save(&p)
            .unwrap();
        // Même date de fichier : sans EXIF, c'est elle qui date la photo.
        std::fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(1_780_000_000))
            .unwrap();
        p
    };
    let a = png("carte/IMG_0001.png", 16);
    let b = png("copie/IMG_0001.png", 16);
    let other = png("carte/IMG_0002.png", 32);
    let cat = dir.join("cat.lumacat");
    let (ok, text) = luma(&[
        "catalog",
        dir.join("photos").to_str().unwrap(),
        "--catalog",
        cat.to_str().unwrap(),
        "--duplicates",
    ]);
    assert!(ok, "{text}");
    assert!(text.contains("1 photos en double"), "{text}");
    assert!(
        text.contains("carte/IMG_0001.png") && text.contains("copie/IMG_0001.png"),
        "{text}"
    );
    assert!(!text.contains("IMG_0002"), "{text}");
    // Rien n'est effacé.
    assert!(a.is_file() && b.is_file() && other.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `luma catalog` : un mot‑clé renommé puis retiré dans les XMP, une collection
/// renommée, une photo retirée d'une collection.
#[test]
fn keywords_and_collections_are_edited() {
    let dir = std::env::temp_dir().join(format!("luma-cli-mots-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("photos")).unwrap();
    let photo = dir.join("photos/plage.png");
    image::RgbImage::from_pixel(8, 8, image::Rgb([90, 120, 200]))
        .save(&photo)
        .unwrap();
    let xmp = luma_project::xmp::sidecar_path(&photo);
    let meta = luma_project::xmp::Meta {
        rating: 4,
        keywords: vec!["mer".into(), "Plage".into()],
        ..luma_project::xmp::Meta::default()
    };
    luma_project::xmp::save(&xmp, &luma_project::xmp::write_meta(&meta)).unwrap();
    let cat = dir.join("cat.lumacat");
    let (c, p, folder) = (
        cat.to_str().unwrap(),
        photo.to_str().unwrap(),
        dir.join("photos").to_str().unwrap().to_owned(),
    );
    let (ok, text) = luma(&[
        "catalog",
        &folder,
        "--catalog",
        c,
        "--add",
        "Tirages",
        p,
        "--rename-keyword",
        "MER",
        "Océan",
    ]);
    assert!(ok, "{text}");
    assert!(text.contains("« MER » devient « Océan » sur 1 photos"), "{text}");
    let read = || luma_project::xmp::read_meta(&std::fs::read_to_string(&xmp).unwrap());
    assert_eq!(
        (read().rating, read().keywords),
        (4, vec!["Océan".to_owned(), "Plage".to_owned()])
    );
    let (ok, text) = luma(&[
        "catalog",
        "--catalog",
        c,
        "--delete-keyword",
        "plage",
        "--rename-collection",
        "Tirages",
        "Albums",
        "--collections",
    ]);
    assert!(ok, "{text}");
    assert_eq!(read().keywords, ["Océan"]);
    assert!(text.contains("collection « Tirages » renommée « Albums »"), "{text}");
    let (ok, text) = luma(&[
        "catalog",
        "--catalog",
        c,
        "--remove-from",
        "Albums",
        p,
        "--collection",
        "Albums",
    ]);
    assert!(ok, "{text}");
    assert!(text.contains("1 photo retirée de « Albums »"), "{text}");
    let (ok, text) = luma(&["catalog", "--catalog", c, "--rename-collection", "Absente", "X"]);
    assert!(!ok && text.contains("pas de collection « Absente »"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}
