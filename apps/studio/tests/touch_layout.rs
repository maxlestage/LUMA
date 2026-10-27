//! La mise en page au doigt : iPhone en portrait et en paysage, iPad. Captures dans
//! `target/studio-tactile-*.png`.

use std::path::Path;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use luma_studio::Studio;

fn capture(name: &str, size: [f32; 2]) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus/_DSC0009.ARW");
    if !src.exists() {
        eprintln!("corpus absent (scripts/fetch-corpus.sh) : capture ignorée");
        return;
    }
    let dir = std::env::temp_dir().join(format!("luma-tactile-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let raw = dir.join("_DSC0009.ARW");
    std::fs::copy(&src, &raw).unwrap();
    let licence = dir.join("licence");
    let catalog = dir.join("catalogue.lumacat");
    let mut harness = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(2.0)
        .wgpu()
        .build_ui_state(
            move |ui, studio: &mut Option<Studio>| {
                let s = studio.get_or_insert_with(|| {
                    let mut s = Studio::new(ui.ctx());
                    s.touch_layout = true;
                    s.use_licence_dir(licence.clone());
                    s.use_catalog(catalog.clone());
                    s.open_path(&raw);
                    s
                });
                s.ui(ui);
            },
            None,
        );
    let start = Instant::now();
    loop {
        harness.step();
        if harness.state().as_ref().is_some_and(Studio::is_idle) {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(180),
            "le rendu n'arrive pas"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    for _ in 0..3 {
        harness.step();
    }
    let image = harness.render().expect("rendu de l'interface");
    image
        .save(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../target/studio-tactile-{name}.png")),
        )
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn touch_layouts() {
    // iPhone 17 Pro Max en paysage (hors encoche), puis iPad en paysage.
    capture("iphone-paysage", [832.0, 420.0]);
    capture("ipad-paysage", [1194.0, 834.0]);
}
