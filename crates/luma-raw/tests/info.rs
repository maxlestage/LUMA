//! L'en‑tête lu par `luma_raw::info` (le début du fichier seulement) dit exactement ce
//! que dit le décodage complet, sur chaque RAW du corpus (`scripts/fetch-corpus.sh`).

use std::path::Path;

#[test]
fn the_head_of_the_file_says_what_the_whole_file_says() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    let mut checked = 0;
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().is_some_and(|x| x == "md") {
            continue;
        }
        let Ok(full) = luma_raw::RawPhoto::open(&path) else {
            continue;
        };
        let info = luma_raw::info(&path).unwrap_or_else(|| panic!("{} : pas d'en‑tête", path.display()));
        assert_eq!(
            (info.make.as_str(), info.model.as_str()),
            (full.make.as_str(), full.model.as_str()),
            "{}",
            path.display()
        );
        assert_eq!(info.exif, full.exif, "{}", path.display());
        assert!(info.exif.date.is_some(), "{} : date de prise de vue", path.display());
        checked += 1;
    }
    println!("{checked} RAW vérifiés");
}
