//! La case « Corrections du boîtier » : décochée, le rendu revient à la photo telle que
//! l'objectif l'a donnée ; sans corrections dans le fichier, elle ne change rien.

use std::path::Path;

use luma_core::Cancel;
use luma_project::Settings;
use luma_render::{Photo, Size};

fn corpus(name: &str) -> Option<Photo> {
    Photo::open(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/raw-corpus")
            .join(name),
    )
    .ok()
}

/// La luminance moyenne d'un coin (le dixième de l'image en haut à gauche).
fn corner(photo: &Photo, camera: bool) -> f32 {
    let mut s = Settings::default();
    s.lens.camera = camera;
    let img = photo.render(&s, Size::Fit(600), &Cancel::new()).unwrap().image;
    let (w, h) = (img.width / 10, img.height / 10);
    let mut sum = 0.0;
    for y in 0..h {
        for x in 0..w {
            sum += img.data[y * img.width + x][1];
        }
    }
    sum / (w * h) as f32
}

#[test]
fn the_camera_corrections_can_be_turned_off() {
    let Some(sony) = corpus("_DSC0009.ARW") else { return };
    assert!(sony.has_camera_corrections());
    let (on, off) = (corner(&sony, true), corner(&sony, false));
    assert!(
        on > off * 1.05,
        "le vignetage corrigé éclaircit le coin : {on} contre {off}"
    );
    // Une photo sans corrections dans son fichier : la case ne change rien.
    let Some(nikon) = corpus("DSC_0750.NEF") else { return };
    assert!(!nikon.has_camera_corrections());
    assert_eq!(corner(&nikon, true), corner(&nikon, false));
}

#[test]
fn a_duplicate_carrying_the_corrections_asks_for_the_original() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus/_DSC0009.ARW");
    if !src.exists() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("luma-lens-dup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let photo = Photo::open(&src).unwrap();
    photo.write_duplicate(&src, &dir).unwrap();
    let reopened = Photo::open_with_duplicates(&src, &dir).unwrap();
    assert!(reopened.from_duplicate());
    assert!(reopened.has_camera_corrections());
    assert!(reopened.needs_original(false), "le doublon porte déjà les corrections");
    assert!(!reopened.needs_original(true));
    assert!(!photo.needs_original(false), "depuis le RAW, rien à relire");
    let _ = std::fs::remove_dir_all(&dir);
}
