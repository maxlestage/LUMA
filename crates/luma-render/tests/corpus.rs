//! Développe chaque RAW du corpus (`tests/raw-corpus`, rempli par
//! `scripts/fetch-corpus.sh`) : décodage, couleur, rendu, sans valeur aberrante. Sans
//! corpus, le test ne fait rien.

use std::path::{Path, PathBuf};

use luma_color::OutputSpace;
use luma_core::Cancel;
use luma_project::Settings;
use luma_render::{Photo, Size};

fn corpus() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e != "md"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

#[test]
fn every_raw_of_the_corpus_renders() {
    let files = corpus();
    if files.is_empty() {
        eprintln!("corpus vide : lancer scripts/fetch-corpus.sh");
        return;
    }
    let mut heavy = Settings::default();
    for set in [
        "highlights=-70",
        "shadows=60",
        "clarity=25",
        "texture=15",
        "dehaze=10",
        "vibrance=25",
        "vignette=-20",
    ] {
        heavy.set(set).unwrap();
    }
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let photo = Photo::open(&path).unwrap_or_else(|e| panic!("{name} : {e}"));
        let wb = photo.as_shot();
        assert!((2000.0..12000.0).contains(&wb.temperature), "{name} : balance {wb:?}");
        for settings in [Settings::default(), heavy.clone()] {
            let r = photo.render(&settings, Size::Fit(512), &Cancel::new()).unwrap();
            assert!(r.image.width.max(r.image.height) == 512, "{name}");
            assert!(
                r.image.data.iter().flatten().all(|v| v.is_finite()),
                "{name} : valeur non finie"
            );
            let px = luma_image::encode8(&r.image, OutputSpace::DisplayP3);
            let mean = px.iter().map(|&v| f64::from(v)).sum::<f64>() / px.len() as f64;
            assert!((12.0..220.0).contains(&mean), "{name} : luminosité moyenne {mean:.0}");
        }
        eprintln!("{name} : {:.0} K, rendu correct", wb.temperature);
    }
}
