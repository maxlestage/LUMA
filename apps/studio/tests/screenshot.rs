//! Rend toute l'interface, sans écran, avec les vrais RAW du corpus : les captures sont
//! écrites dans `target/studio-*.png` pour être regardées (et publiées par la CI).
//!
//! Le rendu passe par wgpu : Metal sur Mac, lavapipe (Vulkan logiciel) sur Linux.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use luma_studio::Studio;
use luma_studio::app::{Module, View};

/// Une copie du corpus dans un dossier à part : les XMP écrits par l'app ne touchent
/// pas le corpus. `None` si le corpus n'est pas téléchargé. Des liens physiques quand
/// le système le permet (l'app ne modifie jamais un original : rien à copier, 219 Mo
/// de moins par essai), sinon de vraies copies.
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
        let to = dir.join(f.file_name().unwrap());
        if std::fs::hard_link(f, &to).is_err() {
            std::fs::copy(f, &to).unwrap();
        }
    }
    Some(dir.join("_DSC0009.ARW"))
}

struct Session {
    harness: Harness<'static, Option<Studio>>,
    /// La copie du corpus, effacée à la fin de l'essai.
    dir: PathBuf,
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
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
        let dir = raw.parent().unwrap().to_path_buf();
        prepare(&dir);
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
        let mut session = Self { harness, dir };
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

/// Maj+J : forme d'onde, parade RVB, vectorscope, puis retour à l'histogramme ; chaque
/// instrument arrive avec l'aperçu.
#[test]
fn scopes_waveform_parade_vectorscope() {
    use luma_image::scopes::Scope;
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("instruments") else {
        return;
    };
    assert_eq!(s.studio().scope(), (Scope::Histogram, false));
    for (scope, name) in [
        (Scope::Waveform, "forme-d-onde"),
        (Scope::Parade, "parade"),
        (Scope::Vectorscope, "vectorscope"),
    ] {
        s.keys(&[(Modifiers::SHIFT, Key::J)]);
        assert_eq!(s.studio().scope(), (scope, true));
        s.capture(name);
    }
    s.keys(&[(Modifiers::SHIFT, Key::J)]);
    assert_eq!(s.studio().scope(), (Scope::Histogram, false));
}

/// La mesure au point : le pointeur sur la photo donne la couleur de l'aperçu à cet
/// endroit (sans l'écrêtage peint par‑dessus), hors de la photo plus rien.
#[test]
fn point_measurement() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("mesure") else {
        return;
    };
    assert_eq!(s.studio().measured(), None);
    s.keys(&[(NONE, Key::J)]);
    let center = egui::pos2(720.0, 450.0);
    s.harness.hover_at(center);
    s.harness.step();
    s.harness.step();
    let rgb = s.studio().measured().expect("une couleur sous le pointeur");
    s.keys(&[(NONE, Key::J)]);
    s.harness.hover_at(center);
    s.harness.step();
    s.harness.step();
    // Deux rendus du même aperçu : quelques niveaux d'écart au plus (GPU), jamais la
    // couleur de l'indicateur d'écrêtage.
    let again = s.studio().measured().expect("une couleur sous le pointeur");
    assert!(
        (0..3).all(|c| rgb[c].abs_diff(again[c]) <= 4),
        "{rgb:?} {again:?}"
    );
    s.capture("mesure");
    s.harness.hover_at(egui::pos2(2.0, 450.0));
    s.harness.step();
    s.harness.step();
    assert_eq!(s.studio().measured(), None);
}

/// Renommer : la photo ouverte et une autre choisie prennent le nom du modèle, dans
/// l'ordre du film ; leur XMP suit, le film et la photo ouverte aussi.
#[test]
fn batch_rename() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("renommer") else {
        return;
    };
    let open = s.studio().current_path().unwrap().to_path_buf();
    let dir = open.parent().unwrap().to_path_buf();
    let settings = luma_project::Settings {
        exposure: 0.7,
        ..Default::default()
    };
    luma_studio::worker::write_sidecar(&open, &settings, &Default::default()).unwrap();
    let photos = s.studio().photos().to_vec();
    let k = photos.iter().position(|p| *p == open).unwrap();
    let other = if k == 0 { 1 } else { 0 };
    s.studio().selected = [k, other].into_iter().collect();
    let n = s.studio().rename_photos("vacances_{séquence:2}").unwrap();
    assert_eq!(n, 2);
    s.wait();
    let ext = open.extension().unwrap().to_string_lossy().into_owned();
    let new = dir.join(format!(
        "vacances_{:02}.{ext}",
        if k < other { 1 } else { 2 }
    ));
    assert!(new.is_file() && !open.exists());
    assert_eq!(s.studio().current_path(), Some(new.as_path()));
    assert_eq!(s.studio().photos().len(), photos.len());
    assert!(s.studio().photos().contains(&new));
    // Son XMP l'a suivie.
    let got = luma_studio::worker::read_sidecar(&new).unwrap();
    assert_eq!(got.exposure, 0.7);
    // Un modèle qui ne change rien : rien à faire.
    s.studio().selected.clear();
    let again = format!("{}", new.file_stem().unwrap().to_string_lossy());
    assert_eq!(s.studio().rename_photos(&again).unwrap(), 0);
}

/// Les instantanés : les réglages d'un moment, gardés dans le XMP, retrouvés d'un clic
/// (une étape de l'historique), puis retirés.
#[test]
fn snapshots() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("instantanes") else {
        return;
    };
    let path = s.studio().current_path().unwrap().to_path_buf();
    s.studio().settings.exposure = 0.6;
    s.studio().take_snapshot(Some("Lumineuse"));
    s.studio().settings.exposure = -1.0;
    s.studio().take_snapshot(None);
    let names: Vec<String> = s.studio().snapshots().into_iter().map(|n| n.name).collect();
    assert_eq!(names, ["Lumineuse", "Instantané 2"]);
    s.studio().apply_snapshot(0);
    assert_eq!(s.studio().settings.exposure, 0.6);
    s.wait();
    s.studio().flush_xmp();
    let meta = luma_studio::worker::read_meta(&path);
    assert_eq!(meta.snapshots.len(), 2);
    assert_eq!(meta.snapshots[1].settings().unwrap().exposure, -1.0);
    s.studio().delete_snapshot(0);
    s.studio().flush_xmp();
    assert_eq!(luma_studio::worker::read_meta(&path).snapshots.len(), 1);
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
fn culling_full_screen_from_the_keyboard() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("tri") else {
        return;
    };
    // La première photo du film, en plein écran (F).
    s.keys(&[(NONE, Key::G)]);
    s.studio().set_current(0);
    s.wait();
    s.keys(&[(NONE, Key::F)]);
    assert!(s.studio().is_culling());
    let photos = s.studio().photos().to_vec();
    assert!(photos.len() >= 3);
    s.capture("tri");
    // Une note, puis la suivante d'elle‑même ; un libellé rouge (6), puis la suivante.
    s.keys(&[(NONE, Key::Num3)]);
    assert_eq!(s.studio().current_path(), Some(photos[1].as_path()));
    s.keys(&[(NONE, Key::Num6)]);
    assert_eq!(s.studio().current_path(), Some(photos[2].as_path()));
    // Sans avance automatique (A), la photo reste ; la même touche retire le libellé.
    s.keys(&[(NONE, Key::A), (NONE, Key::Num9), (NONE, Key::P)]);
    assert_eq!(s.studio().current_path(), Some(photos[2].as_path()));
    assert_eq!(s.studio().current_meta().label, "Blue");
    s.keys(&[(NONE, Key::Num9)]);
    assert_eq!(s.studio().current_meta().label, "");
    // Échap sort du tri ; tout est dans les XMP, aux noms du format.
    s.keys(&[(NONE, Key::Escape)]);
    assert!(!s.studio().is_culling());
    s.studio().flush_xmp();
    let read = |p: &std::path::Path| luma_studio::worker::read_meta(p);
    assert_eq!(read(&photos[0]).rating, 3);
    assert_eq!(read(&photos[1]).label, "Red");
    assert_eq!(read(&photos[2]).flag, luma_project::xmp::Flag::Pick);
}

#[test]
fn compare_two_to_four_photos() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("comparaison") else {
        return;
    };
    // Sans choix : la photo ouverte et la suivante (ou la précédente, en fin de film).
    s.keys(&[(NONE, Key::C)]);
    assert_eq!(s.studio().compared().len(), 2);
    s.keys(&[(NONE, Key::Escape)]);
    assert!(s.studio().compared().is_empty());

    // Trois photos choisies : trois cases, rendues avec leurs réglages.
    let photos = s.studio().photos().to_vec();
    s.studio().selected = [0, 1, 2].into_iter().collect();
    s.studio().start_compare();
    assert_eq!(s.studio().compared(), photos[..3].to_vec());
    let start = Instant::now();
    while !s.studio().compare_ready() {
        s.harness.step();
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "la comparaison n'arrive pas"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    s.harness.step();
    s.capture("comparaison");

    // Zoom et déplacement communs, jamais hors de la photo ; la loupe (L).
    s.studio().compare_zoom(2.0, [0.1, 0.1]);
    let (zoom, center, _) = s.studio().compare_view_state().unwrap();
    assert_eq!(zoom, 2.0);
    assert!(
        center.iter().all(|c| (0.25..=0.75).contains(c)),
        "{center:?}"
    );
    s.studio().compare_pan([-1.0, 0.0]);
    let (_, center, _) = s.studio().compare_view_state().unwrap();
    assert_eq!(center[0], 0.75, "bord droit de la photo");
    s.studio().compare_zoom(0.1, [0.5, 0.5]);
    assert_eq!(
        s.studio().compare_view_state().unwrap().0,
        1.0,
        "jamais plus petit que la photo entière"
    );
    s.keys(&[(NONE, Key::L)]);
    assert!(s.studio().compare_view_state().unwrap().2);
    s.keys(&[(NONE, Key::C)]);
    assert!(s.studio().compared().is_empty(), "C referme");
}

#[test]
fn bursts_are_stacked() {
    let _gpu = one_at_a_time();
    // Trois prises du même boîtier dans la même seconde : le RAW Sony et deux copies.
    let Some(mut s) = Session::open_prepared("piles", [1440.0, 900.0], 1.0, |dir| {
        for n in ["_DSC0010.ARW", "_DSC0011.ARW"] {
            std::fs::copy(dir.join("_DSC0009.ARW"), dir.join(n)).unwrap();
        }
    }) else {
        return;
    };
    s.keys(&[(NONE, Key::G)]);
    let all = s.studio().photos().len();
    let stacks = s.studio().stacks().to_vec();
    assert_eq!(stacks.len(), 1, "{stacks:?}");
    assert_eq!(stacks[0].kind, luma_library::stacks::Kind::Burst);
    assert_eq!(stacks[0].photos.len(), 3);
    // Repliée : la première seule dans la grille.
    assert_eq!(s.studio().visible().len(), all - 2);
    s.capture("piles");
    // S déplie la pile de la photo ouverte, puis la replie.
    let cover = stacks[0].photos[0].clone();
    let i = s
        .studio()
        .photos()
        .iter()
        .position(|p| *p == cover)
        .unwrap();
    s.studio().set_current(i);
    s.wait();
    s.keys(&[(NONE, Key::S)]);
    assert_eq!(s.studio().visible().len(), all);
    s.keys(&[(NONE, Key::S)]);
    assert_eq!(s.studio().visible().len(), all - 2);
    // Sans empilement, tout se voit.
    s.studio().set_stacking(false);
    assert_eq!(s.studio().visible().len(), all);
}

#[test]
fn defaults_per_camera_and_import_film() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("defauts") else {
        return;
    };
    let dir = s
        .studio()
        .current_path()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    s.studio().use_defaults_file(dir.join("defauts.json"));
    // Le Sony du corpus (ISO 400) : +0,7 IL deviennent ses réglages à partir d'ISO 400.
    let sony = dir.join("_DSC0009.ARW");
    assert_eq!(s.studio().current_path(), Some(sony.as_path()));
    s.studio().settings.exposure = 0.7;
    s.studio().settings.crop.left = 0.25;
    s.studio().set_camera_defaults(true);
    let saved = luma_project::defaults::Defaults::load(&dir.join("defauts.json"));
    assert_eq!(saved.rules.len(), 1);
    assert_eq!(saved.rules[0].from_iso, Some(400));
    // Une copie du même Sony, sans XMP, s'ouvre avec ces réglages (pas le recadrage) ;
    // un autre boîtier garde ceux de LÜMA.
    std::fs::copy(&sony, dir.join("autre-sony.ARW")).unwrap();
    s.studio().open_path(&dir.join("autre-sony.ARW"));
    s.wait();
    assert_eq!(s.studio().settings.exposure, 0.7);
    assert_eq!(s.studio().settings.crop.left, 0.0);
    s.studio().open_path(&dir.join("DSC_0750.NEF"));
    s.wait();
    assert_eq!(s.studio().settings.exposure, 0.0);
    s.studio().open_path(&sony);
    s.wait();
    s.studio().clear_camera_defaults();
    assert!(
        luma_project::defaults::Defaults::load(&dir.join("defauts.json"))
            .rules
            .is_empty()
    );

    // Le film d'import : donné aux photos qui arrivent dans le dossier surveillé.
    s.studio()
        .set_import_preset(Some(luma_project::presets::PRESETS[0].name.to_owned()));
    s.studio().watch_folder(&dir);
    s.wait();
    let new = dir.join("arrivee.ARW");
    std::fs::copy(&sony, &new).unwrap();
    s.studio().poll_watch();
    s.studio().poll_watch();
    let got = luma_studio::worker::read_sidecar(&new).expect("un XMP à l'arrivée");
    assert_ne!(
        got,
        luma_project::Settings::default(),
        "le film est appliqué"
    );
    s.studio().stop_watching();
}

#[test]
fn iptc_information_and_templates() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("informations") else {
        return;
    };
    let dir = s
        .studio()
        .current_path()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    s.studio().use_templates_file(dir.join("modeles.json"));
    s.keys(&[(NONE, Key::G)]);
    let open = s.studio().current_path().unwrap().to_path_buf();
    // Les informations de la photo ouverte, et un mot‑clé hiérarchique.
    s.studio().set_info("title", "Le parc en hiver");
    s.studio().set_info("creator", "Maxime Lestage");
    s.studio().set_info("rights", "© 2026 Maxime Lestage");
    s.studio().set_info("city", "Strasbourg");
    s.studio().set_info("country", "France");
    assert_eq!(s.studio().add_keywords("Lieux > France > Strasbourg"), 1);
    s.studio().flush_xmp();
    let m = luma_studio::worker::read_meta(&open);
    assert_eq!(m.title, "Le parc en hiver");
    assert_eq!(
        (m.location.city.as_str(), m.location.country.as_str()),
        ("Strasbourg", "France")
    );
    assert_eq!(m.keywords, ["Lieux|France|Strasbourg"]);
    let text = std::fs::read_to_string(luma_project::xmp::sidecar_path(&open)).unwrap();
    assert!(
        text.contains("<rdf:li>Strasbourg</rdf:li>"),
        "chaque niveau dans dc:subject"
    );

    // Un modèle (auteur, droits, lieu, mots‑clés), enregistré puis donné à deux autres
    // photos choisies ; leur titre ne change pas.
    s.studio().save_template("Strasbourg");
    assert_eq!(s.studio().templates().len(), 1);
    assert_eq!(
        luma_project::xmp::load_templates(&dir.join("modeles.json")).len(),
        1
    );
    let photos = s.studio().photos().to_vec();
    let others: Vec<usize> = (0..photos.len())
        .filter(|&i| photos[i] != open)
        .take(2)
        .collect();
    s.studio().selected = others.iter().copied().collect();
    s.studio().set_info("title", "Sans titre");
    s.studio().apply_template("Strasbourg");
    s.studio().flush_xmp();
    for &i in &others {
        let m = luma_studio::worker::read_meta(&photos[i]);
        assert_eq!(m.creator, "Maxime Lestage", "{}", photos[i].display());
        assert_eq!(m.location.city, "Strasbourg");
        assert_eq!(m.title, "Sans titre");
        assert_eq!(m.keywords, ["Lieux|France|Strasbourg"]);
    }
    s.wait();
    s.capture("informations");
}

#[test]
fn sync_and_auto_balance_a_selection() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("synchroniser") else {
        return;
    };
    let photos = s.studio().photos().to_vec();
    let open = s.studio().current_path().unwrap().to_path_buf();
    let k = photos.iter().position(|p| *p == open).unwrap();
    // La photo ouverte : +1 IL et un recadrage ; deux autres choisies avec elle.
    s.studio().settings.exposure = 1.0;
    s.studio().settings.crop.left = 0.2;
    s.studio().selected = [k, 0, 1].into_iter().collect();
    s.keys(&[(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)]);
    let groups = luma_project::paste::default_groups();
    assert_eq!(s.studio().sync_selected(&groups), 2);
    for p in &photos[..2] {
        let got = luma_studio::worker::read_sidecar(p).unwrap();
        assert_eq!(got.exposure, 1.0, "{}", p.display());
        assert_eq!(got.crop.left, 0.0, "le recadrage reste à chaque photo");
    }

    // Chacune sa balance automatique : des scènes différentes, des balances différentes.
    s.studio().selected = [0, 1].into_iter().collect();
    s.studio().auto_balance_selected();
    let start = Instant::now();
    while !s.studio().auto_balance_done() {
        s.harness.step();
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "la balance n'arrive pas"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let wb = |p: &std::path::Path| luma_studio::worker::read_sidecar(p).unwrap().white_balance;
    let (a, b) = (wb(&photos[0]), wb(&photos[1]));
    assert!(
        matches!(a, luma_project::WhiteBalance::Custom { .. }),
        "{a:?}"
    );
    assert!(
        matches!(b, luma_project::WhiteBalance::Custom { .. }),
        "{b:?}"
    );
    assert_ne!(a, b);
    // La synchronisation précédente est restée.
    assert_eq!(
        luma_studio::worker::read_sidecar(&photos[0])
            .unwrap()
            .exposure,
        1.0
    );
}

#[test]
fn duplicates_and_batch_ratings() {
    let _gpu = one_at_a_time();
    // Une copie du RAW Sony sous un autre nom : la même image.
    let Some(mut s) = Session::open_prepared("doublons", [1440.0, 900.0], 1.0, |dir| {
        std::fs::copy(dir.join("_DSC0009.ARW"), dir.join("copie.ARW")).unwrap();
    }) else {
        return;
    };
    s.keys(&[(NONE, Key::G)]);
    let dir = s
        .studio()
        .current_path()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    assert_eq!(s.studio().show_duplicates(), 1);
    let shown: Vec<_> = s
        .studio()
        .visible()
        .into_iter()
        .map(|i| s.studio().photos()[i].clone())
        .collect();
    assert_eq!(shown, [dir.join("_DSC0009.ARW"), dir.join("copie.ARW")]);
    s.wait();
    s.capture("doublons");
    s.studio().hide_duplicates();

    // Trois photos choisies : la note et le drapeau vont aux trois.
    let photos = s.studio().photos().to_vec();
    s.studio().selected = [0, 1, 2].into_iter().collect();
    s.keys(&[(NONE, Key::Num4), (NONE, Key::P)]);
    s.studio().flush_xmp();
    for p in &photos[..3] {
        let m = luma_studio::worker::read_meta(p);
        assert_eq!(
            (m.rating, m.flag),
            (4, luma_project::xmp::Flag::Pick),
            "{}",
            p.display()
        );
    }
    // Une seule photo choisie : seule la photo ouverte change.
    s.studio().selected = [0].into_iter().collect();
    let open = s.studio().current_path().unwrap().to_path_buf();
    s.keys(&[(NONE, Key::Num1)]);
    s.studio().flush_xmp();
    assert_eq!(luma_studio::worker::read_meta(&open).rating, 1);
    if open != photos[0] {
        assert_eq!(luma_studio::worker::read_meta(&photos[0]).rating, 4);
    }
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

/// La couleur moyenne du centre de la photo.
fn mean_rgb(image: &image::RgbaImage) -> [f64; 3] {
    let crop = image::imageops::crop_imm(image, 300, 200, 600, 450).to_image();
    let mut sum = [0.0; 3];
    for p in crop.pixels() {
        for c in 0..3 {
            sum[c] += f64::from(p[c]);
        }
    }
    sum.map(|v| v / f64::from(600 * 450))
}

/// La part du centre de la photo peinte de la couleur de l'alerte de gamut.
fn alert_share(image: &image::RgbaImage) -> f64 {
    let crop = image::imageops::crop_imm(image, 300, 200, 600, 450).to_image();
    let alert = luma_color::proof::ALERT;
    let n = crop.pixels().filter(|p| p.0[..3] == alert).count();
    n as f64 / f64::from(600 * 450)
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
    // Un papier crème : le blanc du support, en XYZ.
    let wtpt = tag(&bytes, b"wtpt");
    for (i, v) in [0.88f32, 0.90, 0.66].into_iter().enumerate() {
        bytes[wtpt + 4 * i..wtpt + 4 * i + 4]
            .copy_from_slice(&((v * 65536.0).round() as i32).to_be_bytes());
    }
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
    // Le papier crème du profil : la photo s'assombrit et se réchauffe.
    let plain = mean_rgb(&s.harness.render().unwrap());
    s.studio().set_proof_media(true, false);
    s.wait();
    s.capture("epreuvage-papier");
    let paper = mean_rgb(&s.harness.render().unwrap());
    assert!(
        paper[2] < plain[2] - 5.0 && paper[0] - paper[2] > plain[0] - plain[2] + 3.0,
        "{plain:?} → {paper:?}"
    );
    s.studio().set_proof_media(false, false);
    // Coupé, la photo revient telle quelle.
    s.studio().set_proofing(false, false);
    s.wait();
    let back = saturation(&s.harness.render().unwrap());
    assert!(
        (back - before).abs() < before * 0.02,
        "{before:.2} → {back:.2}"
    );
    // L'alerte de gamut : les couleurs trop vives pour le profil étroit passent au bleu
    // de l'alerte, sans simulation.
    s.studio().set_gamut_alert(true);
    assert!(s.studio().gamut_alert() && !s.studio().proofing_active());
    s.wait();
    s.capture("alerte-gamut");
    let alerted = alert_share(&s.harness.render().unwrap());
    assert!(alerted > 0.05, "{alerted:.3} de la photo en alerte");
    s.studio().set_gamut_alert(false);
    s.wait();
    assert!(alert_share(&s.harness.render().unwrap()) < 0.001);
    // Un fichier qui n'est pas un profil est refusé sans rien casser.
    let junk = dir.join("faux.icc");
    std::fs::write(&junk, b"pas un profil").unwrap();
    s.studio().load_proof_profile(&junk);
    assert!(!s.studio().proofing_active());
}

#[test]
fn library_watched_folder() {
    let _gpu = one_at_a_time();
    // Deux photos attendent sur la « carte », hors du dossier surveillé.
    let Some(mut s) = Session::open_prepared("surveille", [1440.0, 900.0], 1.0, |dir| {
        std::fs::create_dir_all(dir.join("carte")).unwrap();
        for f in ["DSC_0750.NEF", "P1000951.RW2"] {
            let _ = std::fs::rename(dir.join(f), dir.join("carte").join(f));
        }
    }) else {
        return;
    };
    let dir = s
        .studio()
        .current_path()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    if !dir.join("carte/DSC_0750.NEF").exists() || !dir.join("carte/P1000951.RW2").exists() {
        return;
    }
    s.studio().tool = luma_studio::app::Tool::Library;
    s.studio().watch_folder(&dir);
    s.wait();
    let before = s.studio().photos().len();
    assert_eq!(s.studio().watching().map(|(_, n)| n), Some(0));

    // Une photo arrive : au relevé suivant (complète), elle entre dans le film et dans
    // la photothèque, et s'ouvre.
    let nef = dir.join("DSC_0750.NEF");
    std::fs::copy(dir.join("carte/DSC_0750.NEF"), &nef).unwrap();
    s.studio().poll_watch();
    assert_eq!(
        s.studio().photos().len(),
        before,
        "vue une fois, pas encore donnée"
    );
    s.studio().poll_watch();
    assert!(s.studio().photos().contains(&nef));
    assert_eq!(s.studio().current_path(), Some(nef.as_path()));
    s.wait();
    assert!(
        s.studio().catalog().get(&nef).is_some(),
        "dans la photothèque"
    );
    assert_eq!(s.studio().watching().map(|(_, n)| n), Some(1));
    s.capture("dossier-surveille");

    // Sans « Ouvrir chaque nouvelle photo », elle s'ajoute au film, la photo ouverte
    // reste.
    s.studio().set_watch_follow(false);
    let rw2 = dir.join("P1000951.RW2");
    std::fs::copy(dir.join("carte/P1000951.RW2"), &rw2).unwrap();
    s.studio().poll_watch();
    s.studio().poll_watch();
    assert!(s.studio().photos().contains(&rw2));
    assert_eq!(s.studio().current_path(), Some(nef.as_path()));
    assert_eq!(s.studio().photos().len(), before + 2);
    s.studio().stop_watching();
    assert!(s.studio().watching().is_none());
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
fn retouch_spots() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("retouche") else {
        return;
    };
    // Q : le correcteur ; la copie réduite de la photo arrive pour chercher les sources.
    s.keys(&[(NONE, Key::Q)]);
    s.wait();
    let st = s.studio();
    assert_eq!(st.retouch, Some(luma_project::SpotKind::Heal));
    let heal = luma_studio::retouching::place(
        luma_project::SpotKind::Heal,
        (0.3, 0.6),
        0.03,
        st.scene_map(),
    );
    assert!(
        st.scene_map().is_some(),
        "la copie réduite pour les sources"
    );
    assert!(heal.source != heal.center);
    // Les yeux rouges n'ont pas de source à chercher.
    let eye = luma_studio::retouching::place(
        luma_project::SpotKind::RedEye,
        (0.4, 0.4),
        0.01,
        st.scene_map(),
    );
    assert_eq!(eye.source, eye.center);
    st.settings.spots = vec![
        heal,
        luma_project::Spot {
            source: (0.75, 0.3),
            ..luma_project::Spot::new(luma_project::SpotKind::Clone, (0.55, 0.3), 0.05)
        },
    ];
    st.selected_spot = Some(1);
    s.wait();
    s.capture("retouche");
    assert_eq!(s.studio().settings.spots.len(), 2);
}

#[test]
fn sensor_dust() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("poussieres") else {
        return;
    };
    // Le fond gris et flou du RAF : ses taches de capteur.
    let raf = s.dir.join("AFXT2720.RAF");
    if !raf.exists() {
        return;
    }
    s.studio().open_path(&raf);
    s.wait();
    s.keys(&[(NONE, Key::Q)]);
    let st = s.studio();
    st.show_dust = true;
    s.wait();
    s.capture("poussieres-vue");
    // Retirées d'un coup : des correcteurs posés, puis rien de plus au second essai.
    s.studio().find_dust();
    s.wait();
    s.wait();
    let found = s.studio().settings.spots.len();
    assert!(found > 0 && found <= 60, "{found}");
    assert!(
        s.studio()
            .settings
            .spots
            .iter()
            .all(|p| p.kind == luma_project::SpotKind::Heal)
    );
    s.studio().find_dust();
    s.wait();
    s.wait();
    assert_eq!(s.studio().settings.spots.len(), found, "déjà retirées");
    s.studio().show_dust = false;
    s.wait();
    s.capture("poussieres");
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
    // La même zone en blanc sur noir, puis en bleu à 35 %.
    s.studio().mask_style = luma_studio::masking::Overlay {
        color: luma_studio::masking::COLORS.len() - 1,
        opacity: 100.0,
    };
    s.wait();
    s.capture("masques-noir-blanc");
    s.studio().mask_style = luma_studio::masking::Overlay {
        color: 2,
        opacity: 35.0,
    };
    s.wait();
    s.capture("masques-bleu");
    let st = s.studio();
    assert_eq!(st.settings.masks[0].parts.len(), 1);
    assert_eq!(st.selected_part, Some((0, 0)));
}

#[test]
fn channel_mixer() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("melangeur") else {
        return;
    };
    let st = s.studio();
    st.tool = luma_studio::app::Tool::Color;
    s.wait();
    let before = s.studio().histogram();
    // La couche rouge à moitié : l'image vire au cyan, le vert bouge peu.
    let st = s.studio();
    st.settings.channel_mixer.red = [50.0, 0.0, 0.0];
    s.wait();
    let after = s.studio().histogram();
    let mean = |h: &[u32; 256]| {
        let n: u64 = h.iter().map(|&c| c as u64).sum();
        h.iter()
            .enumerate()
            .map(|(v, &c)| v as u64 * c as u64)
            .sum::<u64>() as f32
            / n as f32
    };
    let (r0, g0) = (mean(&before[0]), mean(&before[1]));
    let (r1, g1) = (mean(&after[0]), mean(&after[1]));
    assert!(r1 < r0 - 15.0, "rouge {r0} → {r1}");
    assert!(r0 - r1 > 2.0 * (g0 - g1).abs(), "vert {g0} → {g1}");
    s.capture("melangeur");
}

#[test]
fn crop_guides() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("guides") else {
        return;
    };
    s.keys(&[(NONE, Key::R)]);
    assert_eq!(s.studio().crop_guide, luma_studio::crop::Guide::Thirds);
    // O : le guide suivant, jusqu'à la spirale ; Maj+O la tourne.
    for _ in 0..4 {
        s.keys(&[(NONE, Key::O)]);
    }
    assert_eq!(s.studio().crop_guide, luma_studio::crop::Guide::Spiral);
    s.keys(&[(Modifiers::SHIFT, Key::O)]);
    assert_eq!(s.studio().crop_turn, 1);
    s.wait();
    s.capture("recadrage-spirale");
    // Hors du recadrage, O montre toujours la zone d'un masque.
    s.keys(&[(NONE, Key::Escape)]);
    assert_eq!(s.studio().crop_guide, luma_studio::crop::Guide::Spiral);
}

#[test]
fn auto_straighten() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("redresser") else {
        return;
    };
    let (lake, bowl) = (
        s.dir.join("IMG_1361.DNG"),
        s.dir.join("Canon_EOS_R5_CRAW_ISO_100_nocrop_dual.CR3"),
    );
    if !lake.exists() || !bowl.exists() {
        return;
    }
    // Le lac : son horizon penche un peu ; Maj+R le redresse.
    s.studio().open_path(&lake);
    s.wait();
    s.keys(&[(Modifiers::SHIFT, Key::R)]);
    s.wait();
    let a = s.studio().settings.crop.angle;
    assert!((a + 2.3).abs() < 0.3, "lac : {a}");
    s.capture("redresser");
    // La corbeille sur la table, vue de près : des lignes de fuite, pas d'horizon.
    s.studio().open_path(&bowl);
    s.wait();
    s.keys(&[(Modifiers::SHIFT, Key::R)]);
    s.wait();
    assert_eq!(s.studio().settings.crop.angle, 0.0);
    // Le lac n'a pas de verticales : Maj+V n'y change rien.
    s.studio().open_path(&lake);
    s.wait();
    s.keys(&[(Modifiers::SHIFT, Key::V)]);
    s.wait();
    assert_eq!(s.studio().settings.transform.vertical, 0.0);
    // L'étagère du NEF, vue un peu d'en haut : ses verticales redeviennent parallèles.
    let shelf = s.dir.join("DSC_0750.NEF");
    if shelf.exists() {
        s.studio().open_path(&shelf);
        s.wait();
        s.keys(&[(Modifiers::SHIFT, Key::V)]);
        s.wait();
        let v = s.studio().settings.transform.vertical;
        assert!((v - 17.0).abs() <= 3.0, "étagère : {v}");
        s.capture("verticales");
    }
}

#[test]
fn iphone_layout() {
    let _gpu = one_at_a_time();
    // Un iPhone en portrait (points), écran Retina ×3.
    let Some(mut s) = Session::open_sized("iphone", [393.0, 852.0], 3.0) else {
        return;
    };
    s.capture("iphone");
    // Le menu « ••• » de l'iPhone : les commandes d'édition, la langue, le cache.
    s.harness.get_by_label("Plus").click();
    for _ in 0..5 {
        s.harness.step();
    }
    s.harness.get_by_label("Réinitialiser la photo");
    s.capture("iphone-menu");
    // Annuler (le menu et la flèche font la même chose) : la photo reste ouverte.
    s.harness
        .get_all_by_label("Annuler")
        .next()
        .unwrap()
        .click();
    s.wait();
    assert!(s.studio().current_path().is_some());
    // La barre d'outils nomme chaque outil : Couleur ouvre son panneau.
    s.harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "Couleur")
        .click();
    s.wait();
    assert_eq!(s.studio().tool, luma_studio::app::Tool::Color);
    s.capture("iphone-couleur");
    // La flèche annule une vraie retouche (un masque ajouté).
    s.keys(&[(NONE, Key::M)]);
    s.wait();
    assert_eq!(s.studio().settings.masks.len(), 1);
    s.harness.get_by_label("Annuler").click();
    s.wait();
    assert!(s.studio().settings.masks.is_empty(), "annulé");
    // Le bouton de l'avant dit ce qu'il montre.
    assert!(s.harness.query_by_label("Après").is_none());
    s.harness.get_by_label("Avant").click();
    s.wait();
    assert!(s.studio().before);
}

/// Sur iPhone, sans photo : la barre du haut n'a que le menu « ••• » (la langue), rien
/// à exporter ni à annuler.
#[test]
fn iphone_welcome_is_bare() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open_sized("iphone-accueil", [393.0, 852.0], 3.0) else {
        return;
    };
    let empty = s.dir.join("vide");
    std::fs::create_dir_all(&empty).unwrap();
    s.studio().open_path(&empty);
    for _ in 0..10 {
        s.harness.step();
    }
    assert!(s.studio().current_path().is_none());
    assert!(
        !s.studio().has_image(),
        "plus d'image de la photo précédente"
    );
    s.harness.get_by_label("Plus");
    for label in ["Exporter", "Annuler", "Avant", "Après"] {
        assert!(s.harness.query_by_label(label).is_none(), "{label}");
    }
    s.capture("iphone-accueil");
}

/// Sur iPhone et iPad, sans photo : un vrai toucher du doigt sur « Importer des photos »
/// (appui, relâché et doigt levé, tous dans la même image de l'interface comme sur un
/// appareil) demande l'import. Sur l'écran d'un iPhone Pro Max et sur un iPad.
#[test]
fn a_tap_on_import_photos_imports() {
    let _gpu = one_at_a_time();
    for (name, size) in [
        ("iphone-importer", [440.0, 956.0]),
        ("ipad-importer", [1032.0, 1376.0]),
    ] {
        let Some(mut s) = Session::open_sized(name, size, 3.0) else {
            return;
        };
        let empty = s.dir.join("vide");
        std::fs::create_dir_all(&empty).unwrap();
        s.studio().photos_button = true;
        s.studio().open_path(&empty);
        for _ in 0..10 {
            s.harness.step();
        }
        let button = s.harness.get_by_label("Importer des photos").rect();
        let at = button.center();
        for one_frame in [true, false] {
            let asked = s.studio().import_asks;
            s.harness.event(egui::Event::PointerMoved(at));
            s.harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            });
            if !one_frame {
                s.harness.step();
            }
            s.harness.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            });
            s.harness.event(egui::Event::PointerGone);
            for _ in 0..3 {
                s.harness.step();
            }
            assert_eq!(
                s.studio().import_asks,
                asked + 1,
                "{name} : toucher en {at:?} ({one_frame})"
            );
        }
        s.capture(name);
    }
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

/// L'accueil sur iPhone : rien qui ne serve sans photo dans la barre du haut, et l'écran
/// immobile ne se redessine pas (la batterie).
#[test]
fn iphone_welcome_is_simple_and_quiet() {
    let _gpu = one_at_a_time();
    let mut harness = Harness::builder()
        .with_size([393.0, 852.0])
        .with_pixels_per_point(3.0)
        .wgpu()
        .build_ui_state(
            |ui, studio: &mut Option<Studio>| {
                studio
                    .get_or_insert_with(|| {
                        let mut s = Studio::new(ui.ctx());
                        s.use_licence_dir(std::env::temp_dir().join("luma-accueil-iphone-licence"));
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
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/studio-accueil-iphone.png");
    image.save(&out).unwrap();
    for useless in ["Après", "Avant"] {
        assert!(
            harness.query_by_label(useless).is_none(),
            "« {useless} » sans photo"
        );
    }
    assert!(
        !harness.ctx.has_requested_repaint(),
        "l'accueil immobile se redessine"
    );
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

/// Le copier‑coller sélectif : les réglages d'une photo copiés, la photo suivante
/// ouverte, « Coller… » : par défaut, tout sauf le recadrage et les masques.
#[test]
fn selective_paste_keeps_the_framing_of_each_photo() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("coller") else {
        return;
    };
    let first = s.studio().current_path().unwrap().to_path_buf();
    {
        let st = s.studio();
        st.settings.exposure = 0.8;
        st.settings.vibrance = 25.0;
        st.settings.crop = luma_project::Crop {
            left: 0.2,
            ..luma_project::Crop::default()
        };
    }
    s.keys(&[
        (Modifiers::COMMAND | Modifiers::SHIFT, Key::C),
        (NONE, Key::ArrowLeft),
    ]);
    s.wait();
    assert_ne!(s.studio().current_path().unwrap(), first, "une autre photo");
    s.keys(&[(
        Modifiers::COMMAND | Modifiers::SHIFT | Modifiers::ALT,
        Key::V,
    )]);
    s.capture("coller");
    s.harness.get_by_label("Coller (12)").click();
    s.harness.step();
    s.wait();
    let st = s.studio();
    assert_eq!((st.settings.exposure, st.settings.vibrance), (0.8, 25.0));
    assert_eq!(
        st.settings.crop,
        luma_project::Crop::default(),
        "le recadrage reste le sien"
    );
}

#[test]
fn map_of_geotagged_photos() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("carte") else {
        return;
    };
    let tiles = s.dir.join("tuiles");
    s.studio().use_tiles_dir(tiles.clone());
    s.studio().set_map_online(false);
    // Trois photos placées : deux à Paris, une à Rome.
    let photos = s.studio().photos().to_vec();
    assert!(photos.len() >= 4);
    for (p, gps) in photos
        .iter()
        .zip([[48.8584, 2.2945], [41.8902, 12.4922], [48.8606, 2.3376]])
    {
        let mut e = s.studio().catalog().get(p).unwrap().clone();
        e.gps = Some(gps);
        s.studio().catalog_mut().insert(e);
    }
    s.keys(&[(NONE, Key::G)]);
    s.harness.get_by_label("Carte").click();
    s.harness.step();
    s.wait();
    let Some(Some(view)) = s.studio().map_view() else {
        panic!("la carte est montrée et cadrée");
    };
    // Les photos du corpus qui ont déjà une position (lue dans leur EXIF).
    let already = photos[3..]
        .iter()
        .filter(|p| s.studio().catalog().get(p).is_some_and(|e| e.gps.is_some()))
        .count();
    let groups = s.studio().map_groups().to_vec();
    assert_eq!(
        groups.iter().filter(|g| g.1 == 2).count(),
        1,
        "Paris : {groups:?}"
    );
    assert_eq!(groups.iter().map(|g| g.1).sum::<usize>(), 3 + already);
    assert_eq!(
        s.studio().map_tiles(),
        0,
        "aucune tuile gardée, pas de réseau"
    );

    // Un fond de carte déjà gardé : des tuiles unies (la mer), lues sans réseau.
    let sea = image::RgbaImage::from_pixel(256, 256, image::Rgba([170, 211, 223, 255]));
    for (tile, _, _) in view.tiles([2400.0, 1600.0]) {
        let file = tiles.join(tile.file());
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        sea.save_with_format(&file, image::ImageFormat::Png)
            .unwrap();
    }
    s.studio().set_map_online(false);
    s.harness.step();
    s.wait();
    assert!(s.studio().map_tiles() >= 4, "{}", s.studio().map_tiles());
    s.capture("carte");

    // Toucher le groupe de Paris : ses deux photos dans la grille.
    let paris = groups.iter().find(|g| g.1 == 2).unwrap().0;
    s.harness.event(egui::Event::PointerMoved(paris));
    for pressed in [true, false] {
        s.harness.event(egui::Event::PointerButton {
            pos: paris,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        s.harness.step();
    }
    s.wait();
    assert!(s.studio().map_view().is_none(), "retour à la grille");
    let shown: Vec<PathBuf> = s
        .studio()
        .visible()
        .iter()
        .map(|&i| photos[i].clone())
        .collect();
    assert_eq!(shown, [photos[0].clone(), photos[2].clone()]);
    s.harness.event(egui::Event::PointerGone);
    s.harness
        .get_all_by_label("Tout afficher")
        .next()
        .unwrap()
        .click();
    for _ in 0..5 {
        s.harness.step();
    }
    s.wait();
    assert_eq!(s.studio().visible().len(), photos.len());
}
