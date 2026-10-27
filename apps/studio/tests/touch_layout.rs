//! La mise en page au doigt : iPhone en portrait et en paysage, iPad. Captures dans
//! `target/studio-tactile-*.png`.

use std::path::Path;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use luma_studio::Studio;
use luma_studio::app::{Module, Tool};

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

/// Un doigt qui touche, glisse et se lève, comme egui-winit le traduit sur iPhone : un
/// événement tactile et le pointeur qui le suit.
fn finger(harness: &mut Harness<'static, Option<Studio>>, from: egui::Pos2, to: egui::Pos2) {
    let m = egui::Modifiers::default();
    let touch = |phase, pos| egui::Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(1),
        phase,
        pos,
        force: None,
    };
    let input = harness.input_mut();
    input.events.push(touch(egui::TouchPhase::Start, from));
    input.events.push(egui::Event::PointerMoved(from));
    input.events.push(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: m,
    });
    harness.step();
    for k in 1..=12 {
        let p = from + (to - from) * (k as f32 / 12.0);
        let input = harness.input_mut();
        input.events.push(touch(egui::TouchPhase::Move, p));
        input.events.push(egui::Event::PointerMoved(p));
        harness.step();
    }
    let input = harness.input_mut();
    input.events.push(touch(egui::TouchPhase::End, to));
    input.events.push(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: m,
    });
    input.events.push(egui::Event::PointerGone);
    for _ in 0..30 {
        harness.step();
    }
}

/// Au doigt (l'interface de l'iPhone) : les boutons de la barre du haut y tiennent en
/// entier, et glisser le doigt sur le panneau le fait défiler jusqu'à son dernier
/// réglage.
#[test]
fn finger_scrolls_the_panel() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus/_DSC0009.ARW");
    if !src.exists() {
        eprintln!("corpus absent (scripts/fetch-corpus.sh) : essai ignoré");
        return;
    }
    luma_studio::force_touch(true);
    let dir = std::env::temp_dir().join("luma-tactile-doigt");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let raw = dir.join("_DSC0009.ARW");
    std::fs::copy(&src, &raw).unwrap();
    let (licence, catalog) = (dir.join("licence"), dir.join("catalogue.lumacat"));
    // L'iPhone 17 Pro Max en portrait, hors encoche et barre d'accueil.
    let mut harness = Harness::builder()
        .with_size([440.0, 860.0])
        .with_pixels_per_point(2.0)
        .wgpu()
        .build_ui_state(
            move |ui, studio: &mut Option<Studio>| {
                let s = studio.get_or_insert_with(|| {
                    let mut s = Studio::new(ui.ctx());
                    s.use_licence_dir(licence.clone());
                    s.use_catalog(catalog.clone());
                    s.open_path(&raw);
                    s
                });
                s.module = Module::Develop;
                s.tool = Tool::Light;
                s.ui(ui);
            },
            None,
        );
    let start = Instant::now();
    while !harness.state().as_ref().is_some_and(Studio::is_idle) {
        harness.step();
        assert!(
            start.elapsed() < Duration::from_secs(180),
            "le rendu n'arrive pas"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    harness.step();
    // La barre du haut : le bouton « ••• » tient dans son îlot.
    let more = harness.get_by_label("Plus").rect();
    assert!(more.height() >= 40.0, "{more:?}");
    let base = harness.get_by_label("Base").rect().min.y;
    for _ in 0..12 {
        finger(
            &mut harness,
            egui::pos2(60.0, 800.0),
            egui::pos2(60.0, 560.0),
        );
    }
    let after = harness.get_by_label("Base").rect().min.y;
    assert!(
        after < base - 200.0,
        "le panneau n'a pas défilé : {base} → {after}"
    );
    // Le bas du panneau est atteint : « Instantanés », sous l'historique, est à l'écran.
    let last = harness.get_by_label("Instantanés").rect();
    assert!(last.max.y <= 860.0, "{last:?}");
    harness
        .render()
        .unwrap()
        .save(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/studio-tactile-doigt.png"))
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
