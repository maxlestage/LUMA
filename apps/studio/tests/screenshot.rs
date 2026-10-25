//! Rend toute l'interface, sans écran, avec les vrais RAW du corpus : les captures sont
//! écrites dans `target/studio-*.png` pour être regardées (et publiées par la CI).
//!
//! Le rendu passe par wgpu : Metal sur Mac, lavapipe (Vulkan logiciel) sur Linux.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use luma_studio::Studio;
use luma_studio::app::Module;

/// Une copie du corpus dans un dossier à part : les XMP écrits par l'app ne touchent
/// pas le corpus. `None` si le corpus n'est pas téléchargé.
fn corpus_copy(name: &str) -> Option<PathBuf> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let files = luma_studio::app::scan(&src);
    files
        .iter()
        .find(|p| p.extension().is_some_and(|e| e == "ARW"))?;
    let dir = std::env::temp_dir().join(format!("luma-studio-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for f in &files {
        std::fs::copy(f, dir.join(f.file_name().unwrap())).unwrap();
    }
    Some(dir.join("_DSC0009.ARW"))
}

struct Session {
    harness: Harness<'static, Option<Studio>>,
}

impl Session {
    fn open(name: &str) -> Option<Self> {
        Self::open_sized(name, [1440.0, 900.0], 1.0)
    }

    fn open_sized(name: &str, size: [f32; 2], scale: f32) -> Option<Self> {
        Self::open_prepared(name, size, scale, |_| {})
    }

    /// Comme `open_sized`, après avoir préparé la copie du corpus (des XMP, par exemple).
    fn open_prepared(
        name: &str,
        size: [f32; 2],
        scale: f32,
        prepare: impl FnOnce(&Path),
    ) -> Option<Self> {
        let Some(raw) = corpus_copy(name) else {
            eprintln!("corpus absent (scripts/fetch-corpus.sh) : capture ignorée");
            return None;
        };
        prepare(raw.parent().unwrap());
        let harness = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(scale)
            .wgpu()
            .build_ui_state(
                move |ui, studio: &mut Option<Studio>| {
                    let s = studio.get_or_insert_with(|| {
                        let mut s = Studio::new(ui.ctx());
                        s.open_path(&raw);
                        s
                    });
                    s.ui(ui);
                },
                None,
            );
        let mut session = Self { harness };
        session.wait();
        Some(session)
    }

    /// Attend que la photo soit rendue et les vignettes arrivées.
    fn wait(&mut self) {
        let start = Instant::now();
        loop {
            self.harness.step();
            if self.harness.state().as_ref().is_some_and(Studio::is_idle) {
                for _ in 0..3 {
                    self.harness.step();
                }
                return;
            }
            assert!(
                start.elapsed() < Duration::from_secs(180),
                "le rendu n'arrive pas"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn studio(&mut self) -> &mut Studio {
        self.harness.state_mut().as_mut().unwrap()
    }

    fn keys(&mut self, keys: &[(Modifiers, Key)]) {
        for (m, k) in keys {
            self.harness.key_press_modifiers(*m, *k);
            self.harness.step();
            self.wait();
        }
    }

    fn capture(&mut self, name: &str) {
        let image = self.harness.render().expect("rendu de l'interface");
        let out =
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/studio-{name}.png"));
        image.save(&out).unwrap();
        eprintln!("capture : {}", out.display());
    }
}

const NONE: Modifiers = Modifiers::NONE;

#[test]
fn develop_module() {
    let Some(mut s) = Session::open("developpement") else {
        return;
    };
    let st = s.studio();
    st.settings.exposure = 0.3;
    st.settings.shadows = 35.0;
    st.settings.vibrance = 20.0;
    s.wait();
    s.capture("developpement");
}

#[test]
fn before_after_and_clipping() {
    let Some(mut s) = Session::open("avant") else {
        return;
    };
    s.keys(&[(NONE, Key::Backslash), (NONE, Key::J)]);
    s.capture("avant");
}

#[test]
fn library_ratings_and_flags() {
    let Some(mut s) = Session::open("bibliotheque") else {
        return;
    };
    // Note 4 et drapeau « retenue » sur _DSC0009 (la dernière par ordre de nom), note 2 et rejet sur la précédente.
    s.keys(&[
        (NONE, Key::G),
        (NONE, Key::Num4),
        (NONE, Key::P),
        (NONE, Key::ArrowLeft),
    ]);
    s.keys(&[(NONE, Key::Num2), (NONE, Key::X)]);
    assert_eq!(s.studio().module, Module::Library);
    s.capture("bibliotheque");
    // Les notes sont dans les XMP, lisibles par Lightroom.
    s.studio().flush_xmp();
    let dir = std::env::temp_dir().join("luma-studio-bibliotheque");
    let rejected = luma_studio::worker::read_meta(s.studio().current_path().unwrap());
    assert_eq!(rejected.flag, luma_project::xmp::Flag::Reject);
    let first = luma_studio::app::scan(&dir)
        .into_iter()
        .find(|p| p.ends_with("_DSC0009.ARW"))
        .unwrap();
    let meta = luma_studio::worker::read_meta(&first);
    assert_eq!((meta.rating, meta.flag), (4, luma_project::xmp::Flag::Pick));
}

#[test]
fn crop_tool_and_export_window() {
    let Some(mut s) = Session::open("recadrage") else {
        return;
    };
    s.studio().settings.crop = luma_project::Crop {
        left: 0.1,
        top: 0.15,
        right: 0.8,
        bottom: 0.9,
        angle: 2.0,
    };
    s.keys(&[(NONE, Key::R)]);
    s.capture("recadrage");
    s.keys(&[
        (NONE, Key::Enter),
        (Modifiers::COMMAND | Modifiers::SHIFT, Key::E),
    ]);
    s.capture("export");
}

#[test]
fn masks_linear_and_radial() {
    let Some(mut s) = Session::open("masques") else {
        return;
    };
    // Un ciel assombri et refroidi (dégradé linéaire), puis un radial éclairci.
    s.keys(&[(NONE, Key::M)]);
    let st = s.studio();
    st.settings.masks[0].local.exposure = -1.2;
    st.settings.masks[0].local.temperature = -30.0;
    s.wait();
    s.keys(&[(Modifiers::SHIFT, Key::M)]);
    let st = s.studio();
    st.settings.masks[1].local.exposure = 0.8;
    st.settings.masks[1].local.clarity = 30.0;
    s.wait();
    s.capture("masques");
    s.keys(&[(NONE, Key::O)]);
    s.capture("masques-zone");
    assert_eq!(s.studio().settings.masks.len(), 2);
    assert_eq!(s.studio().selected_mask, Some(1));
}

#[test]
fn iphone_layout() {
    // Un iPhone en portrait (points), écran Retina ×3.
    let Some(mut s) = Session::open_sized("iphone", [393.0, 852.0], 3.0) else {
        return;
    };
    s.capture("iphone");
}

#[test]
fn films_and_merge_tools() {
    // Une pellicule « mesurée » : ombres froides, hautes lumières chaudes, courbe en S.
    let mut cube = luma_image::cube::Cube::identity(17);
    for p in &mut cube.data {
        let s = |v: f32| v * v * (3.0 - 2.0 * v) * 0.7 + v * 0.3;
        let l = (p[0] + p[1] + p[2]) / 3.0;
        *p = [
            s(p[0]) + 0.05 * l,
            s(p[1]) + 0.02,
            s(p[2]) + 0.06 * (1.0 - l) - 0.04 * l,
        ]
        .map(|v| v.clamp(0.0, 1.0));
    }
    luma_image::cube::register("Ma Portra 400 (scan)", cube);
    let Some(mut s) = Session::open("films") else {
        return;
    };
    s.studio().tool = luma_studio::app::Tool::Films;
    // Les aperçus des préréglages arrivent un à un.
    for _ in 0..200 {
        s.harness.step();
        std::thread::sleep(Duration::from_millis(30));
    }
    s.capture("films");
    let st = s.studio();
    st.tool = luma_studio::app::Tool::Merge;
    st.selected = [0, 1, 2].into_iter().collect();
    s.wait();
    s.capture("fusion");
}

#[test]
fn films_stacked_with_masks() {
    let Some(mut s) = Session::open("films-empiles") else {
        return;
    };
    let st = s.studio();
    // Un film sur toute l'image à 60 %, un autre dans un cercle au centre.
    let mut full = luma_project::Mask::full();
    full.look = Some(luma_project::Look {
        name: "Kodak 2383".into(),
        amount: 60.0,
    });
    let mut radial = luma_project::Mask::radial();
    radial.look = Some(luma_project::Look {
        name: "Tri-X 400".into(),
        amount: 100.0,
    });
    st.settings.masks = vec![full, radial];
    st.tool = luma_studio::app::Tool::Masks;
    st.selected_mask = Some(1);
    s.wait();
    s.capture("films-empiles");
}

/// Les photos retouchées se voient : un signe et le nom du film sur les vignettes, et
/// sous la photo affichée, son film et ses retouches.
#[test]
fn edited_photos_show_their_film() {
    let Some(mut s) = Session::open_prepared("modifiees", [1440.0, 900.0], 1.0, |dir| {
        let mut portra = luma_project::presets::find("Portra 400")
            .unwrap()
            .applied(&luma_project::Settings::default(), 0.8);
        portra.exposure = 0.3;
        std::fs::write(dir.join("_DSC0009.xmp"), luma_project::xmp::write(&portra)).unwrap();
        let tri_x = luma_project::presets::find("Tri-X 400")
            .unwrap()
            .applied(&luma_project::Settings::default(), 1.0);
        for f in luma_studio::app::scan(dir).iter().skip(1).take(1) {
            std::fs::write(f.with_extension("xmp"), luma_project::xmp::write(&tri_x)).unwrap();
        }
    }) else {
        return;
    };
    assert_eq!(
        s.studio().settings.film.as_ref().map(|f| f.name.as_str()),
        Some("Portra 400")
    );
    s.capture("modifiees");
}

/// L'accueil, sans photo : le logo sur le fond de LÜMA.
#[test]
fn welcome_screen() {
    let mut harness = Harness::builder()
        .with_size([1440.0, 900.0])
        .wgpu()
        .build_ui_state(
            |ui, studio: &mut Option<Studio>| {
                studio.get_or_insert_with(|| Studio::new(ui.ctx())).ui(ui);
            },
            None,
        );
    for _ in 0..5 {
        harness.step();
    }
    let image = harness.render().expect("rendu de l'interface");
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/studio-accueil.png");
    image.save(&out).unwrap();
}
