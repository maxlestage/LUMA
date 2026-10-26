//! L'aperçu intégré au RAW : présent, dans le bon sens, assez grand, et bien plus rapide
//! que le décodage du capteur.

use std::path::Path;
use std::time::Instant;

use luma_raw::RawPhoto;

#[test]
fn embedded_previews_are_fast_and_upright() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("pas de corpus : test ignoré");
        return;
    };
    let mut seen = 0;
    let (mut fast_total, mut full_total) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().is_none_or(|e| e == "md") {
            continue;
        }
        let t = Instant::now();
        let preview = RawPhoto::embedded_preview(&path, 320);
        let fast = t.elapsed();
        let t = Instant::now();
        let raw = RawPhoto::open(&path).unwrap();
        let full = t.elapsed();
        let Some(p) = preview else {
            eprintln!("{} : pas d'aperçu intégré", path.display());
            continue;
        };
        seen += 1;
        eprintln!(
            "{} : aperçu {}×{} en {:?}, décodage du RAW {:?}",
            path.display(),
            p.width(),
            p.height(),
            fast,
            full
        );
        assert_eq!(p.width().max(p.height()), 320);
        // Dans le même sens que le développement (portrait ou paysage).
        let (w, h) = match raw.orientation {
            luma_raw::Orientation::Rotate90
            | luma_raw::Orientation::Rotate270
            | luma_raw::Orientation::Transpose
            | luma_raw::Orientation::Transverse => (raw.height, raw.width),
            _ => (raw.width, raw.height),
        };
        assert_eq!(p.width() > p.height(), w > h, "{}", path.display());
        fast_total += fast;
        full_total += full;
    }
    eprintln!("{seen} aperçus en {fast_total:?}, décodages en {full_total:?}");
    // Le temps ne compte qu'optimisé : sans optimisation, le décodage JPEG est
    // particulièrement lent, et un RAW non compressé se lit plus vite qu'un aperçu.
    if !cfg!(debug_assertions) {
        assert!(fast_total * 2 < full_total, "aperçus bien plus rapides que le décodage");
    }
}
