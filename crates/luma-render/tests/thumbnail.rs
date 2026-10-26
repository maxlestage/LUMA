//! Les vignettes enregistrées : sur de vrais RAW, relire une vignette est bien plus
//! rapide que la redévelopper, et l'image est exactement la même.

use std::path::Path;
use std::time::{Duration, Instant};

use luma_color::OutputSpace;
use luma_core::Cancel;
use luma_render::{Photo, Size, thumbnail};

#[test]
fn a_saved_thumbnail_comes_back_at_once() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let Ok(entries) = std::fs::read_dir(&root) else {
        eprintln!("pas de corpus : test ignoré");
        return;
    };
    let dir = std::env::temp_dir().join(format!("luma-vignettes-{}", std::process::id()));
    let (mut rendered, mut loaded) = (Duration::ZERO, Duration::ZERO);
    let mut n = 0;
    for source in entries.flatten().map(|e| e.path()) {
        if source.extension().is_none_or(|e| e == "md") {
            continue;
        }
        let t = Instant::now();
        let photo = Photo::open(&source).unwrap();
        let r = photo
            .render(&photo.default_settings(), Size::Fit(320), &Cancel::new())
            .unwrap();
        let rgb = luma_image::encode8(&r.image, OutputSpace::Srgb);
        rendered += t.elapsed();
        let file = thumbnail::path_for(&source, None, 320, &dir).unwrap();
        thumbnail::store(&file, &rgb, r.image.width as u32, r.image.height as u32).unwrap();

        let t = Instant::now();
        let back = thumbnail::load(&file).unwrap();
        loaded += t.elapsed();
        assert_eq!(back.dimensions(), (r.image.width as u32, r.image.height as u32));
        assert_eq!(back.as_raw(), &rgb, "{} : même image", source.display());
        n += 1;
    }
    eprintln!("{n} vignettes : rendues en {rendered:?}, relues en {loaded:?}");
    let _ = std::fs::remove_dir_all(&dir);
    if n > 0 && !cfg!(debug_assertions) {
        assert!(loaded * 10 < rendered, "relire est bien plus rapide que redévelopper");
    }
}
