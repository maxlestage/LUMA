//! Rend toute l'interface, sans écran, avec une vraie photo du corpus : la capture est
//! écrite dans `target/studio-*.png` pour être regardée (et publiée par la CI).
//!
//! Le rendu passe par wgpu : Metal sur Mac, lavapipe (Vulkan logiciel) sur Linux.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use luma_studio::Studio;

fn corpus() -> Option<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let file = luma_studio::app::scan(&dir)
        .into_iter()
        .find(|p| p.extension().is_some_and(|e| e == "ARW"))?;
    Some(file)
}

fn capture(name: &str, setup: impl FnOnce(&mut Studio), keys: &[(egui::Modifiers, egui::Key)]) {
    let Some(raw) = corpus() else {
        eprintln!("corpus absent (scripts/fetch-corpus.sh) : capture ignorée");
        return;
    };
    // Un dossier à part : le XMP écrit par l'app ne touche pas le corpus.
    let dir = std::env::temp_dir().join(format!("luma-studio-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let copy = dir.join(raw.file_name().unwrap());
    if !copy.exists() {
        std::fs::copy(&raw, &copy).unwrap();
    }
    let _ = std::fs::remove_file(dir.join("_DSC0009.xmp"));

    let mut harness = Harness::builder()
        .with_size([1440.0, 900.0])
        .wgpu()
        .build_ui_state(
            move |ui, studio: &mut Option<Studio>| {
                let s = studio.get_or_insert_with(|| {
                    let mut s = Studio::new(ui.ctx());
                    s.open_path(&copy);
                    s
                });
                s.ui(ui);
            },
            None,
        );
    let start = Instant::now();
    let wait = |harness: &mut Harness<Option<Studio>>| {
        loop {
            harness.step();
            if harness.state().as_ref().is_some_and(Studio::is_idle) {
                // Les vignettes du film arrivent juste après.
                for _ in 0..10 {
                    std::thread::sleep(Duration::from_millis(50));
                    harness.step();
                }
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(120),
                "le rendu n'arrive pas"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait(&mut harness);
    if let Some(s) = harness.state_mut() {
        setup(s);
    }
    wait(&mut harness);
    for (m, k) in keys {
        harness.key_press_modifiers(*m, *k);
        harness.step();
        wait(&mut harness);
    }
    let image = harness.render().expect("rendu de l'interface");
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/studio-{name}.png"));
    image.save(&out).unwrap();
    eprintln!("capture : {}", out.display());
}

#[test]
fn develop_module() {
    capture(
        "developpement",
        |s| {
            s.settings.exposure = 0.3;
            s.settings.shadows = 35.0;
            s.settings.vibrance = 20.0;
        },
        &[],
    );
}

#[test]
fn before_after_and_clipping() {
    capture(
        "avant",
        |_| {},
        &[
            (egui::Modifiers::NONE, egui::Key::Backslash),
            (egui::Modifiers::NONE, egui::Key::J),
        ],
    );
}
