//! Rend toute l'interface, sans écran, avec les vrais RAW du corpus : les captures sont
//! écrites dans `target/studio-*.png` pour être regardées (et publiées par la CI).
//!
//! Le rendu passe par wgpu : Metal sur Mac, lavapipe (Vulkan logiciel) sur Linux.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use luma_studio::Studio;
use luma_studio::app::{Module, View};

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
        // Un essai tout neuf : l'écran Premium ne cache pas les captures.
        let licence_dir = raw.parent().unwrap().join("licence");
        // La photothèque de l'essai, dans sa copie du corpus.
        let catalog = raw.parent().unwrap().join("catalogue.lumacat");
        let harness = Harness::builder()
            .with_size(size)
            .with_pixels_per_point(scale)
            .wgpu()
            .build_ui_state(
                move |ui, studio: &mut Option<Studio>| {
                    let s = studio.get_or_insert_with(|| {
                        let mut s = Studio::new(ui.ctx());
                        s.use_licence_dir(licence_dir.clone());
                        s.use_catalog(catalog.clone());
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

/// Une capture à la fois : plusieurs interfaces rendues en même temps sur le GPU
/// logiciel de Linux (lavapipe) le font parfois planter.
fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static GPU: std::sync::Mutex<()> = std::sync::Mutex::new(());
    GPU.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn develop_module() {
    let _gpu = one_at_a_time();
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
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("avant") else {
        return;
    };
    s.keys(&[(NONE, Key::Backslash), (NONE, Key::J)]);
    s.capture("avant");
}

#[test]
fn library_ratings_and_flags() {
    let _gpu = one_at_a_time();
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
    // Les notes sont enregistrées dans les XMP.
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
fn library_keywords() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("mots-cles") else {
        return;
    };
    s.keys(&[(NONE, Key::G)]);
    let dir = std::env::temp_dir().join("luma-studio-mots-cles");
    let files = luma_studio::app::scan(&dir);
    // Deux photos choisies, autres que celle qui est affichée : leurs XMP sont écrits
    // tout de suite, sans réglages (elles gardent leur rendu par défaut).
    let current = s.studio().current_path().unwrap().to_path_buf();
    let others: Vec<usize> = (0..files.len())
        .filter(|i| files[*i] != current)
        .take(2)
        .collect();
    s.studio().selected = others.iter().copied().collect();
    assert_eq!(s.studio().add_keywords("Mer, Été ; mer"), 2);
    for i in &others {
        let meta = luma_studio::worker::read_meta(&files[*i]);
        assert_eq!(meta.keywords, ["Mer", "Été"], "sans doublon");
        assert!(
            luma_studio::worker::read_sidecar(&files[*i]).is_none(),
            "aucun réglage écrit"
        );
    }
    // Puis la photo affichée seule.
    s.studio().selected.clear();
    assert_eq!(s.studio().add_keywords("Portrait"), 1);
    s.studio().flush_xmp();
    assert_eq!(
        luma_studio::worker::read_meta(&current).keywords,
        ["Portrait"]
    );
    assert_eq!(
        s.studio().folder_keywords(),
        [
            ("Été".to_owned(), 2),
            ("Mer".to_owned(), 2),
            ("Portrait".to_owned(), 1)
        ]
    );
    // Le filtre par mot‑clé, majuscules et accents mis à part.
    s.studio().keyword_filter = Some("ete".into());
    assert_eq!(s.studio().visible(), others);
    s.studio().keyword_filter = Some("Été".into());
    s.studio().selected = others.iter().copied().collect();
    assert_eq!(
        s.studio().target_keywords(),
        ["Mer", "Été"],
        "ceux de la sélection"
    );
    s.wait();
    s.capture("bibliotheque-mots-cles");
    s.studio().remove_keyword("MER");
    assert_eq!(
        luma_studio::worker::read_meta(&files[others[0]]).keywords,
        ["Été"]
    );
}

#[test]
fn library_catalog_views() {
    let _gpu = one_at_a_time();
    // Une collection dynamique, déjà dans le catalogue de l'essai.
    let prepare = |dir: &Path| {
        let mut cat = luma_library::Catalog::default();
        cat.set_collection(luma_library::Collection::smart("Bas ISO", "iso:..160").unwrap())
            .unwrap();
        cat.save(&dir.join("catalogue.lumacat")).unwrap();
    };
    let Some(mut s) = Session::open_prepared("phototheque", [1440.0, 900.0], 1.0, prepare) else {
        return;
    };
    s.keys(&[(NONE, Key::G)]);
    // Le dossier ouvert est entré dans la photothèque, en arrière‑plan : toutes ses
    // photos (le corpus peut grandir).
    let dir = std::env::temp_dir().join("luma-studio-phototheque");
    let count = luma_studio::app::scan(&dir).len();
    assert!(count >= 5);
    assert_eq!(s.studio().catalog().len(), count);
    assert_eq!(
        luma_library::Catalog::load(&dir.join("catalogue.lumacat")).len(),
        count,
        "enregistrée"
    );
    let file = |p: &Path| p.file_name().unwrap().to_string_lossy().into_owned();
    let names = |s: &mut Session| -> Vec<String> {
        let st = s.studio();
        (0..count)
            .filter_map(|i| st.photo_at(i))
            .map(file)
            .collect()
    };
    // Toutes les photos, par date de prise de vue (et non par nom de fichier).
    s.studio().show_view(View::All);
    s.wait();
    let by_date: Vec<String> = s
        .studio()
        .catalog()
        .by_date()
        .iter()
        .map(|e| file(&e.path))
        .collect();
    assert_eq!(names(&mut s), by_date);
    let mut by_name = by_date.clone();
    by_name.sort();
    assert_ne!(by_date, by_name, "le corpus n'est pas rangé par nom");
    // Une recherche, puis la collection dynamique.
    s.studio().show_view(View::Search("boîtier:nikon".into()));
    s.wait();
    assert_eq!(names(&mut s), ["DSC_0750.NEF"]);
    s.studio().show_view(View::Search("couleur:rouge".into()));
    assert_eq!(
        s.studio().view,
        View::Search("boîtier:nikon".into()),
        "une erreur ne change rien"
    );
    s.studio().show_view(View::Collection("Bas ISO".into()));
    s.wait();
    let low = s
        .studio()
        .catalog()
        .collection_photos("Bas ISO")
        .unwrap()
        .len();
    assert!(low >= 4, "les ISO 32 à 160 du corpus");
    assert_eq!(names(&mut s).len(), low, "{:?}", names(&mut s));
    s.capture("bibliotheque-phototheque");
}

#[test]
fn library_virtual_copies() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("copies") else {
        return;
    };
    s.keys(&[(NONE, Key::G), (NONE, Key::Num2)]);
    let original = s.studio().current_path().unwrap().to_path_buf();
    // Une copie : juste après son original, ouverte, avec son propre XMP.
    let copy = s.studio().create_copy().unwrap();
    s.wait();
    assert_eq!(luma_project::copies::source(&copy), original);
    assert_eq!(s.studio().current_path(), Some(copy.as_path()));
    assert!(luma_project::copies::sidecar(&copy).is_file());
    let dir = original.parent().unwrap().to_path_buf();
    let listed = luma_studio::app::scan(&dir);
    let at = listed.iter().position(|p| *p == original).unwrap();
    assert_eq!(
        listed[at + 1],
        copy,
        "relue dans le dossier, après son original"
    );
    // Sa note est la sienne ; l'original garde la sienne.
    s.keys(&[(NONE, Key::Num5)]);
    s.studio().flush_xmp();
    assert_eq!(luma_studio::worker::read_meta(&copy).rating, 5);
    assert_eq!(luma_studio::worker::read_meta(&original).rating, 2);
    s.capture("bibliotheque-copie");
    // Supprimée : son XMP disparaît, l'original reste et s'affiche.
    assert!(s.studio().delete_copy());
    s.wait();
    assert!(!luma_project::copies::sidecar(&copy).exists());
    assert!(original.is_file());
    assert_eq!(s.studio().current_path(), Some(original.as_path()));
    assert!(
        !s.studio().delete_copy(),
        "une photo ordinaire n'est jamais supprimée"
    );
}

#[test]
fn camera_lens_corrections_can_be_turned_off() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("corrections") else {
        return;
    };
    // Le Sony du corpus porte ses corrections : la case est dans l'outil Géométrie.
    s.studio().tool = luma_studio::app::Tool::Geometry;
    assert!(s.studio().settings.lens.camera, "cochée par défaut");
    s.wait();
    s.capture("geometrie-corrections");
    s.studio().settings.lens.camera = false;
    s.wait();
    s.studio().flush_xmp();
    s.studio().settings.lens.camera = false;
    // Le réglage est enregistré dans le XMP (coupé), et relu.
    let path = s.studio().current_path().unwrap().to_path_buf();
    luma_studio::worker::write_sidecar(
        &path,
        &s.studio().settings,
        &luma_studio::worker::read_meta(&path),
    )
    .unwrap();
    assert!(
        !luma_studio::worker::read_sidecar(&path)
            .unwrap()
            .lens
            .camera
    );
}

/// La saturation moyenne du centre de la photo, dans une capture en 1440 × 900.
fn saturation(image: &image::RgbaImage) -> f64 {
    let (x0, y0, w, h) = (300, 200, 600, 450);
    let sum: u64 = image::imageops::crop_imm(image, x0, y0, w, h)
        .to_image()
        .pixels()
        .map(|p| {
            let (r, g, b) = (i32::from(p[0]), i32::from(p[1]), i32::from(p[2]));
            u64::from((r.max(g).max(b) - r.min(g).min(b)).unsigned_abs())
        })
        .sum();
    sum as f64 / f64::from(w * h)
}

/// Le profil sRGB aux primaires rapprochées du blanc : une « imprimante » plus terne.
fn dull_profile() -> Vec<u8> {
    let mut bytes = luma_color::icc::profile(luma_color::OutputSpace::Srgb);
    let tag = |bytes: &[u8], sig: &[u8; 4]| {
        let at = bytes.windows(4).position(|w| w == sig).unwrap();
        u32::from_be_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize + 8
    };
    let read = |bytes: &[u8], at: usize| {
        i32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as f32 / 65536.0
    };
    let cols: Vec<usize> = [b"rXYZ", b"gXYZ", b"bXYZ"]
        .iter()
        .map(|s| tag(&bytes, s))
        .collect();
    let white: Vec<f32> = (0..3)
        .map(|i| cols.iter().map(|&c| read(&bytes, c + 4 * i)).sum())
        .collect();
    for &c in &cols {
        for i in 0..3 {
            let v = 0.15 * read(&bytes, c + 4 * i) + 0.85 * white[i] / 3.0;
            bytes[c + 4 * i..c + 4 * i + 4]
                .copy_from_slice(&((v * 65536.0).round() as i32).to_be_bytes());
        }
    }
    bytes
}

#[test]
fn soft_proofing_shows_the_print() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("epreuvage") else {
        return;
    };
    s.studio().tool = luma_studio::app::Tool::Light;
    s.wait();
    let before = saturation(&s.harness.render().unwrap());
    let dir = s
        .studio()
        .current_path()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let icc = dir.join("Terne.icc");
    std::fs::write(&icc, dull_profile()).unwrap();
    s.studio().load_proof_profile(&icc);
    assert!(s.studio().proofing_active());
    s.wait();
    s.capture("epreuvage");
    let proofed = saturation(&s.harness.render().unwrap());
    assert!(
        proofed < before * 0.9,
        "l'épreuvage ternit la photo : {before:.2} → {proofed:.2}"
    );
    // Coupé, la photo revient telle quelle.
    s.studio().set_proofing(false, false);
    s.wait();
    let back = saturation(&s.harness.render().unwrap());
    assert!(
        (back - before).abs() < before * 0.02,
        "{before:.2} → {back:.2}"
    );
    // Un fichier qui n'est pas un profil est refusé sans rien casser.
    let junk = dir.join("faux.icc");
    std::fs::write(&junk, b"pas un profil").unwrap();
    s.studio().load_proof_profile(&junk);
    assert!(!s.studio().proofing_active());
}

#[test]
fn crop_tool_and_export_window() {
    let _gpu = one_at_a_time();
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
    let _gpu = one_at_a_time();
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
fn masks_combined() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("masques-combines") else {
        return;
    };
    // Un radial éclairci dont on retire un autre radial : un anneau, vu en rouge.
    s.keys(&[(Modifiers::SHIFT, Key::M)]);
    let st = s.studio();
    st.settings.masks[0].local.exposure = 1.0;
    st.settings.masks[0].parts.push(luma_project::Part {
        combine: luma_project::Combine::Subtract,
        shape: luma_project::Shape::Radial {
            center: (0.5, 0.5),
            radius: (0.12, 0.12),
            angle: 0.0,
            feather: 30.0,
        },
        invert: false,
    });
    st.selected_part = Some((0, 0));
    s.wait();
    s.keys(&[(NONE, Key::O)]);
    s.capture("masques-combines");
    let st = s.studio();
    assert_eq!(st.settings.masks[0].parts.len(), 1);
    assert_eq!(st.selected_part, Some((0, 0)));
}

#[test]
fn iphone_layout() {
    let _gpu = one_at_a_time();
    // Un iPhone en portrait (points), écran Retina ×3.
    let Some(mut s) = Session::open_sized("iphone", [393.0, 852.0], 3.0) else {
        return;
    };
    s.capture("iphone");
}

#[test]
fn films_and_merge_tools() {
    let _gpu = one_at_a_time();
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
    let _gpu = one_at_a_time();
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
    let _gpu = one_at_a_time();
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
    let _gpu = one_at_a_time();
    let mut harness = Harness::builder()
        .with_size([1440.0, 900.0])
        .wgpu()
        .build_ui_state(
            |ui, studio: &mut Option<Studio>| {
                studio
                    .get_or_insert_with(|| {
                        let mut s = Studio::new(ui.ctx());
                        s.use_licence_dir(std::env::temp_dir().join("luma-accueil-licence"));
                        s
                    })
                    .ui(ui);
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

/// L'écran Premium quand l'essai est fini : les deux formules et la clé.
#[test]
fn premium_screen_after_the_trial() {
    let _gpu = one_at_a_time();
    let dir = std::env::temp_dir().join("luma-premium-licence");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Un essai commencé il y a 20 jours.
    let started = luma_studio::licence::now() - 20 * 86_400;
    std::fs::write(
        dir.join("licence.json"),
        luma_licence::State::new(started).to_json(),
    )
    .unwrap();
    let mut harness = Harness::builder()
        .with_size([1440.0, 900.0])
        .wgpu()
        .build_ui_state(
            move |ui, studio: &mut Option<Studio>| {
                studio
                    .get_or_insert_with(|| {
                        let mut s = Studio::new(ui.ctx());
                        s.use_licence_dir(dir.clone());
                        s
                    })
                    .ui(ui);
            },
            None,
        );
    for _ in 0..5 {
        harness.step();
    }
    assert!(
        !harness
            .state()
            .as_ref()
            .unwrap()
            .licence
            .access()
            .is_allowed()
    );
    let image = harness.render().expect("rendu de l'interface");
    image
        .save(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/studio-premium.png"))
        .unwrap();
}

/// Le ciblage direct : T, puis un glissé vers le haut sur la photo règle la teinte des
/// couleurs sous le doigt dans le mélangeur, et rien d'autre.
#[test]
fn targeted_adjustment_moves_the_color_under_the_pointer() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("ciblage") else {
        return;
    };
    s.keys(&[(NONE, Key::T)]);
    // Sur le vert du pin, un glissé de 80 points vers le haut.
    let at = egui::pos2(720.0, 300.0);
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    s.harness.event(egui::Event::PointerMoved(at));
    s.harness.step();
    s.harness.event(press(true));
    s.harness.step();
    for k in 1..=8 {
        s.harness.event(egui::Event::PointerMoved(
            at - egui::vec2(0.0, 10.0 * k as f32),
        ));
        s.harness.step();
    }
    s.harness.event(egui::Event::PointerButton {
        pos: at - egui::vec2(0.0, 80.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    s.harness.step();
    s.wait();
    let st = s.studio();
    let moved: Vec<f32> = st
        .settings
        .hsl
        .hue
        .iter()
        .copied()
        .filter(|v| *v != 0.0)
        .collect();
    assert!(
        !moved.is_empty() && moved.len() <= 2 && moved.iter().all(|v| *v > 0.0),
        "teintes : {:?}",
        st.settings.hsl.hue
    );
    assert!((moved.iter().sum::<f32>() - 40.0).abs() < 1.0, "{moved:?}");
    assert!(st.settings.hsl.saturation.iter().all(|v| *v == 0.0));
    s.capture("ciblage");
}
