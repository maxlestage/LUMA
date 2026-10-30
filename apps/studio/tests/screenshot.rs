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
        // Une copie virtuelle : elle entre dans la photothèque comme dans le dossier.
        std::fs::write(dir.join("_DSC0009.ARW.copie-2.xmp"), "").unwrap();
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
    assert!(
        by_date.iter().any(|n| n == "_DSC0009.ARW.copie-2"),
        "{by_date:?}"
    );
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
fn export_enlarge() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("export-agrandir") else {
        return;
    };
    // En pleine définition, pas de case : rien à agrandir.
    s.keys(&[(Modifiers::COMMAND | Modifiers::SHIFT, Key::E)]);
    assert!(s.harness.query_by_label("Agrandir au besoin").is_none());
    // Un grand côté de 12 000 px : la case apparaît et se coche.
    s.studio().export_options.resize = Some(luma_render::export::Resize::LongEdge(12000));
    s.wait();
    s.harness.get_by_label("Agrandir au besoin").click();
    s.wait();
    assert!(s.studio().export_options.enlarge);
    s.capture("export-agrandir");
}

#[test]
fn export_max_weight() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("export-poids") else {
        return;
    };
    // Sans limite, pas de poids à régler ; la case le fait paraître, à 500 Ko.
    s.keys(&[(Modifiers::COMMAND | Modifiers::SHIFT, Key::E)]);
    assert!(
        s.harness
            .query_by_label("Poids maximal en kilo-octets")
            .is_none()
    );
    s.harness.get_by_label("Au plus").click();
    s.wait();
    assert_eq!(s.studio().export_options.max_kb, Some(500));
    assert!(
        s.harness
            .query_by_label("Poids maximal en kilo-octets")
            .is_some()
    );
    s.capture("export-poids");
    // Décochée : plus de limite.
    s.harness.get_by_label("Au plus").click();
    s.wait();
    assert_eq!(s.studio().export_options.max_kb, None);
}

#[test]
fn export_print_size() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("export-tirage") else {
        return;
    };
    s.keys(&[(Modifiers::COMMAND | Modifiers::SHIFT, Key::E)]);
    // Un tirage de 30 × 40 cm à 300 ppp : ses trois champs.
    s.studio().export_options.resize = Some(luma_render::export::Resize::Print {
        width_cm: 30.0,
        height_cm: 40.0,
        dpi: 300,
    });
    s.wait();
    for label in [
        "Largeur du tirage en centimètres",
        "Hauteur du tirage en centimètres",
        "Points par pouce",
    ] {
        assert!(s.harness.query_by_label(label).is_some(), "{label}");
    }
    s.capture("export-tirage");
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
    // Supprimer : le bouton de l'outil, puis une retouche sans source.
    // Le premier « Supprimer » est l'outil ; l'autre efface la retouche choisie.
    s.harness
        .get_all_by_label("Supprimer")
        .next()
        .unwrap()
        .click();
    s.wait();
    assert_eq!(s.studio().retouch, Some(luma_project::SpotKind::Remove));
    let st = s.studio();
    let gone = luma_studio::retouching::place(
        luma_project::SpotKind::Remove,
        (0.42, 0.62),
        0.02,
        st.scene_map(),
    );
    assert_eq!(gone.source, gone.center);
    st.settings.spots.push(gone);
    st.selected_spot = Some(2);
    s.wait();
    assert!(s.studio().has_image());
    s.capture("retouche-supprimer");
}

#[test]
fn liquify() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("fluidite") else {
        return;
    };
    // Q ouvre la Retouche ; « Pousser » prend la place du correcteur.
    s.keys(&[(NONE, Key::Q)]);
    s.wait();
    s.harness.get_by_label("Pousser").click();
    s.wait();
    assert_eq!(s.studio().liquify, Some(luma_project::WarpKind::Push));
    assert_eq!(s.studio().retouch, None);
    let press = |s: &mut Session, path: &[egui::Pos2]| {
        let (a, b) = (path[0], path[path.len() - 1]);
        let mut events = vec![
            egui::Event::PointerMoved(a),
            egui::Event::PointerButton {
                pos: a,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: NONE,
            },
        ];
        events.extend(path[1..].iter().map(|p| egui::Event::PointerMoved(*p)));
        events.push(egui::Event::PointerButton {
            pos: b,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: NONE,
        });
        for e in events {
            s.harness.input_mut().events.push(e);
            s.harness.step();
        }
        s.wait();
    };
    // Un glissement sur la photo : un coup de pinceau, gardé pas à pas.
    let path: Vec<egui::Pos2> = (0..=8)
        .map(|i| egui::pos2(480.0 + 12.0 * i as f32, 420.0 - 3.0 * i as f32))
        .collect();
    press(&mut s, &path);
    let st = s.studio();
    assert_eq!(st.settings.liquify.len(), 1);
    assert!(
        st.settings.liquify[0].points.len() >= 3,
        "{:?}",
        st.settings.liquify[0]
    );
    // Gonfler : un clic suffit.
    s.harness.get_by_label("Gonfler").click();
    s.wait();
    assert_eq!(s.studio().liquify, Some(luma_project::WarpKind::Bloat));
    press(&mut s, &[egui::pos2(760.0, 360.0)]);
    let st = s.studio();
    assert_eq!(st.settings.liquify.len(), 2);
    assert_eq!(st.settings.liquify[1].kind, luma_project::WarpKind::Bloat);
    assert!(st.has_image());
    s.capture("fluidite");
    // Annulable, puis tout effacé d'un bouton.
    s.keys(&[(Modifiers::COMMAND, Key::Z)]);
    assert_eq!(s.studio().settings.liquify.len(), 1);
    s.harness.get_by_label("Effacer").click();
    s.wait();
    assert!(s.studio().settings.liquify.is_empty());
    // Échap range le pinceau.
    s.keys(&[(NONE, Key::Escape)]);
    assert_eq!(s.studio().liquify, None);
}

#[test]
fn traced_removal() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("supprimer-trace") else {
        return;
    };
    // Q ouvre la Retouche ; le premier « Supprimer » est l'outil.
    s.keys(&[(NONE, Key::Q)]);
    s.wait();
    s.harness
        .get_all_by_label("Supprimer")
        .next()
        .unwrap()
        .click();
    s.wait();
    assert_eq!(s.studio().retouch, Some(luma_project::SpotKind::Remove));
    // Un trait sur la photo, d'un geste.
    let path: Vec<egui::Pos2> = (0..=12)
        .map(|i| egui::pos2(380.0 + 30.0 * i as f32, 250.0 + 12.0 * i as f32))
        .collect();
    let (a, b) = (path[0], path[path.len() - 1]);
    let mut events = vec![
        egui::Event::PointerMoved(a),
        egui::Event::PointerButton {
            pos: a,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: NONE,
        },
    ];
    events.extend(path[1..].iter().map(|p| egui::Event::PointerMoved(*p)));
    events.push(egui::Event::PointerButton {
        pos: b,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: NONE,
    });
    for e in events {
        s.harness.input_mut().events.push(e);
        s.harness.step();
    }
    s.wait();
    let st = s.studio();
    assert_eq!(st.settings.spots.len(), 1);
    let spot = &st.settings.spots[0];
    assert_eq!(spot.kind, luma_project::SpotKind::Remove);
    assert!(spot.path.len() >= 3, "{:?}", spot.path);
    assert!(st.has_image());
    s.capture("supprimer-trace");
    // Annulable d'un coup.
    s.keys(&[(Modifiers::COMMAND, Key::Z)]);
    assert!(s.studio().settings.spots.is_empty());
}

#[test]
fn traced_heal() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("correcteur-trace") else {
        return;
    };
    // Q : le correcteur ; un trait sur la photo, d'un geste.
    s.keys(&[(NONE, Key::Q)]);
    s.wait();
    assert_eq!(s.studio().retouch, Some(luma_project::SpotKind::Heal));
    let path: Vec<egui::Pos2> = (0..=10)
        .map(|i| egui::pos2(420.0 + 25.0 * i as f32, 560.0 - 6.0 * i as f32))
        .collect();
    let (a, b) = (path[0], path[path.len() - 1]);
    let mut events = vec![
        egui::Event::PointerMoved(a),
        egui::Event::PointerButton {
            pos: a,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: NONE,
        },
    ];
    events.extend(path[1..].iter().map(|p| egui::Event::PointerMoved(*p)));
    events.push(egui::Event::PointerButton {
        pos: b,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: NONE,
    });
    for e in events {
        s.harness.input_mut().events.push(e);
        s.harness.step();
    }
    s.wait();
    let st = s.studio();
    assert_eq!(st.settings.spots.len(), 1);
    let spot = &st.settings.spots[0];
    assert_eq!(spot.kind, luma_project::SpotKind::Heal);
    assert!(spot.path.len() >= 3, "{:?}", spot.path);
    assert!(spot.source != spot.center, "une source cherchée au départ");
    assert!(st.has_image());
    s.capture("correcteur-trace");
}

#[test]
fn point_curve() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("courbe-points") else {
        return;
    };
    // La courbe de luminosité, amenée à l'écran ; un clic au milieu de la diagonale y
    // pose un point, puis ce point tiré vers le haut : des tons moyens éclaircis.
    s.harness.get_by_label("Courbe à points").scroll_to_me();
    s.wait();
    s.harness.get_by_label("Courbe à points").click();
    s.wait();
    assert_eq!(s.studio().settings.tone_curve.point.len(), 3);
    let r = s.harness.get_by_label("Courbe à points").rect();
    let (a, b) = (r.center(), r.center() - egui::vec2(0.0, 0.2 * r.height()));
    let mut events = vec![
        egui::Event::PointerMoved(a),
        egui::Event::PointerButton {
            pos: a,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: NONE,
        },
    ];
    events.extend((1..=8).map(|i| egui::Event::PointerMoved(a + (b - a) * i as f32 / 8.0)));
    events.push(egui::Event::PointerButton {
        pos: b,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: NONE,
    });
    for e in events {
        s.harness.input_mut().events.push(e);
        s.harness.step();
    }
    s.wait();
    let point = s.studio().settings.tone_curve.point.clone();
    assert_eq!(point.len(), 3, "{point:?}");
    assert!(
        (point[1].0 - 127.5).abs() < 6.0 && point[1].1 > 165.0,
        "{point:?}"
    );
    assert!(s.studio().has_image());
    s.capture("courbe-points");
    // Annulable geste par geste : le point remis en place, puis retiré.
    s.keys(&[(Modifiers::COMMAND, Key::Z)]);
    assert_eq!(s.studio().settings.tone_curve.point.len(), 3);
    s.keys(&[(Modifiers::COMMAND, Key::Z)]);
    assert_eq!(s.studio().settings.tone_curve.point.len(), 2);
    // L'onglet Rouge : un clic sur la courbe y ajoute un point.
    s.harness.get_all_by_label("Rouge").next().unwrap().click();
    s.wait();
    s.harness.get_by_label("Courbe à points").click();
    s.wait();
    assert_eq!(s.studio().settings.tone_curve.red.len(), 3);
    assert_eq!(s.studio().settings.tone_curve.point.len(), 2);
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
fn wavelet_noise_reduction() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("ondelettes") else {
        return;
    };
    let st = s.studio();
    st.tool = luma_studio::app::Tool::Detail;
    st.settings.detail.luminance_nr = 60.0;
    s.wait();
    assert!(!s.studio().settings.detail.luminance_nr_wavelets);
    // La case du panneau Détail passe la luminance aux ondelettes, et la décoche revient.
    s.harness.get_by_label("Ondelettes").click();
    s.wait();
    assert!(s.studio().settings.detail.luminance_nr_wavelets);
    assert!(s.studio().has_image());
    s.capture("ondelettes");
    s.harness.get_by_label("Ondelettes").click();
    s.wait();
    assert!(!s.studio().settings.detail.luminance_nr_wavelets);
}

#[test]
fn highlight_reconstruction() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("hautes-lumieres") else {
        return;
    };
    s.studio().tool = luma_studio::app::Tool::Light;
    s.wait();
    assert!(!s.studio().settings.highlight_reconstruction);
    s.harness
        .get_by_label("Reconstruire les hautes lumières")
        .click();
    s.wait();
    assert!(s.studio().settings.highlight_reconstruction);
    assert!(s.studio().has_image());
    s.capture("hautes-lumieres");
    s.keys(&[(Modifiers::COMMAND, Key::Z)]);
    assert!(!s.studio().settings.highlight_reconstruction, "annulée");
}

#[test]
fn deconvolution_sharpening() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("deconvolution") else {
        return;
    };
    s.studio().tool = luma_studio::app::Tool::Detail;
    s.wait();
    assert!(!s.studio().settings.detail.sharpen_deconvolution);
    // La case de l'Accentuation : cochée, puis décochée par Annuler.
    s.harness.get_by_label("Déconvolution").click();
    s.wait();
    assert!(s.studio().settings.detail.sharpen_deconvolution);
    assert!(s.studio().has_image());
    s.capture("deconvolution");
    s.keys(&[(Modifiers::COMMAND, Key::Z)]);
    assert!(!s.studio().settings.detail.sharpen_deconvolution, "annulée");
}

#[test]
fn profile_from_a_chart() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("profil-charte") else {
        return;
    };
    s.studio().tool = luma_studio::app::Tool::Color;
    s.wait();
    // La photo n'est pas une charte : rien n'est changé, et la barre d'état le dit.
    assert!(s.studio().profile_from_chart().is_none());
    assert!(s.studio().settings.channel_mixer.is_identity());
    s.wait();
    s.capture("profil-charte");
}

#[test]
fn colors_matched_to_another_photo() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("accord") else {
        return;
    };
    s.studio().tool = luma_studio::app::Tool::Color;
    s.wait();
    // Accordée sur elle-même : la correction est presque neutre.
    let me = s.studio().current_path().unwrap().to_path_buf();
    let label = s.studio().match_colors_to(&me).expect("même scène");
    assert!(label.starts_with("Couleurs accordées"), "{label}");
    let m = s.studio().settings.channel_mixer;
    for (r, row) in [m.red, m.green, m.blue].iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            assert!((v - if r == c { 100.0 } else { 0.0 }).abs() < 3.0, "{m:?}");
        }
    }
    // Sur une autre scène : refusé, le mélangeur ne bouge pas.
    s.studio().settings.channel_mixer = luma_project::ChannelMixer::default();
    let other = s
        .studio()
        .photos()
        .iter()
        .find(|p| **p != me)
        .cloned()
        .expect("une autre photo");
    assert!(s.studio().match_colors_to(&other).is_none());
    assert!(s.studio().settings.channel_mixer.is_identity());
    s.wait();
    s.capture("accord");
}

#[test]
fn guided_upright() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("redressement-guide") else {
        return;
    };
    s.studio().tool = luma_studio::app::Tool::Geometry;
    s.wait();
    s.harness.get_by_label("Redressement guidé").click();
    s.wait();
    assert!(s.studio().guiding);
    // Une arête presque verticale, tracée en glissant sur la photo (penchée de 3 %).
    let drag = |s: &mut Session, a: egui::Pos2, b: egui::Pos2| {
        let events = [
            egui::Event::PointerMoved(a),
            egui::Event::PointerButton {
                pos: a,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: NONE,
            },
            egui::Event::PointerMoved(a.lerp(b, 0.5)),
            egui::Event::PointerMoved(b),
            egui::Event::PointerButton {
                pos: b,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: NONE,
            },
        ];
        for e in events {
            s.harness.input_mut().events.push(e);
            s.harness.step();
        }
        s.wait();
    };
    drag(&mut s, egui::pos2(500.0, 220.0), egui::pos2(512.0, 620.0));
    assert_eq!(s.studio().guides.len(), 1);
    assert!(s.studio().guides[0].vertical);
    let r = s.studio().settings.transform.rotate;
    assert!(r.abs() > 0.5, "rotation {r}");
    assert_eq!(
        s.studio().settings.transform.vertical,
        0.0,
        "une ligne : la rotation seule"
    );
    // Une deuxième verticale, qui converge vers le haut : la Verticale s'en mêle.
    drag(&mut s, egui::pos2(850.0, 620.0), egui::pos2(830.0, 220.0));
    assert_eq!(s.studio().guides.len(), 2);
    assert!(s.studio().settings.transform.vertical != 0.0);
    s.capture("redressement-guide");
    // Terminé : les lignes s'effacent, la Transformation reste.
    s.harness.get_by_label("Terminer le tracé").click();
    s.wait();
    assert!(!s.studio().guiding && s.studio().guides.is_empty());
    assert!(s.studio().settings.transform.vertical != 0.0);
    s.capture("redressement-guide-fini");
}

#[test]
fn empty_edges_are_cropped() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("bords-vides") else {
        return;
    };
    let st = s.studio();
    st.tool = luma_studio::app::Tool::Geometry;
    st.settings.transform.vertical = -40.0;
    st.settings.transform.rotate = 3.0;
    s.wait();
    s.harness.get_by_label("Rogner les bords vides").click();
    s.wait();
    let c = s.studio().settings.crop;
    assert!(c.right - c.left < 0.95 && c.right - c.left > 0.4, "{c:?}");
    // Centré, aux proportions de la photo.
    assert!((c.left + c.right - 1.0).abs() < 1e-4 && (c.top + c.bottom - 1.0).abs() < 1e-4);
    assert!(((c.right - c.left) - (c.bottom - c.top)).abs() < 1e-4);
    // Annulable comme tout réglage.
    s.keys(&[(Modifiers::COMMAND, Key::Z)]);
    assert!(s.studio().settings.crop.is_identity());
    s.keys(&[(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)]);
    assert!(!s.studio().settings.crop.is_identity());
    s.capture("bords-vides");
}

#[test]
fn sky_mask() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("ciel") else {
        return;
    };
    // Le paysage de l'OM System : un ciel bleu à nuages au‑dessus d'un arbre et d'une
    // vallée.
    let orf = s.dir.join("OM_System_OM-5_2_regular.ORF");
    s.studio().open_path(&orf);
    s.wait();
    s.studio().tool = luma_studio::app::Tool::Masks;
    s.wait();
    s.harness.get_by_label("Sélectionner le ciel").click();
    s.wait();
    let st = s.studio();
    assert_eq!(st.settings.masks.len(), 1);
    assert_eq!(st.settings.masks[0].shape, luma_project::Shape::Sky);
    // Le masque choisi : sa zone se lit sur la photo, la copie réduite arrive.
    st.selected_mask = Some(0);
    s.wait();
    let st = s.studio();
    // La zone, lue sur la copie réduite de la photo (le masque n'agit pas encore) : le
    // haut est du ciel, le bas non.
    let scene = st.scene_map().expect("copie réduite").clone();
    let zone = luma_image::local::weights_on(&st.settings.masks[0], &scene);
    let (w, h) = (zone.width, zone.height);
    let mean = |y0: usize, y1: usize| {
        let rows = &zone.data[y0 * w..y1 * w];
        rows.iter().sum::<f32>() / rows.len() as f32
    };
    let (top, bottom) = (mean(h / 10, h / 4), mean(h * 3 / 4, h));
    assert!(top > 0.5 && bottom < 0.05, "haut {top}, bas {bottom}");
    // Un ciel plus sombre, et sa zone affichée (O).
    st.settings.masks[0].local.exposure = -1.0;
    st.selected_mask = Some(0);
    s.wait();
    s.keys(&[(NONE, Key::O)]);
    s.capture("masque-ciel");
}

#[test]
fn mask_texture_and_dehaze() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("masque-texture-voile") else {
        return;
    };
    // Maj+M : un dégradé radial.
    s.keys(&[(Modifiers::SHIFT, Key::M)]);
    s.wait();
    // Les curseurs du masque, à côté de la Clarté.
    assert!(s.harness.query_all_by_label("Texture").next().is_some());
    assert!(
        s.harness
            .query_all_by_label("Correction du voile")
            .next()
            .is_some()
    );
    assert!(s.harness.query_all_by_label("Netteté").next().is_some());
    assert!(
        s.harness
            .query_all_by_label("Réduction du bruit")
            .next()
            .is_some()
    );
    let st = s.studio();
    assert_eq!(st.settings.masks.len(), 1);
    st.settings.masks[0].local.sharpness = 40.0;
    st.settings.masks[0].local.noise = 50.0;
    st.settings.masks[0].local.texture = -60.0;
    st.settings.masks[0].local.dehaze = 50.0;
    s.wait();
    assert!(s.studio().has_image());
    s.capture("masque-texture-voile");
}

#[test]
fn auto_tone() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("tons-auto") else {
        return;
    };
    // La veste du RW2, nettement sous‑exposée.
    let rw2 = s.dir.join("P1000951.RW2");
    s.studio().open_path(&rw2);
    s.wait();
    s.studio().tool = luma_studio::app::Tool::Light;
    s.wait();
    s.harness.get_by_label("Tons auto").click();
    s.wait();
    let st = s.studio();
    assert!(
        st.settings.exposure > 0.3,
        "exposition {}",
        st.settings.exposure
    );
    assert!(st.has_image());
    s.capture("tons-auto");
    // Annulable d'un coup.
    s.keys(&[(Modifiers::COMMAND, Key::Z)]);
    assert_eq!(s.studio().settings.exposure, 0.0);
}

#[test]
fn mask_duplicate() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("masque-duplique") else {
        return;
    };
    // Maj+M : un dégradé radial, assombri.
    s.keys(&[(Modifiers::SHIFT, Key::M)]);
    s.wait();
    s.studio().settings.masks[0].local.exposure = -1.0;
    s.wait();
    s.harness.get_by_label("Dupliquer").click();
    s.wait();
    let st = s.studio();
    assert_eq!(st.settings.masks.len(), 2);
    assert_eq!(st.settings.masks[1], st.settings.masks[0]);
    assert_eq!(st.selected_mask, Some(1));
    // La copie inversée : tout le reste, réglages à zéro.
    s.harness.get_by_label("Dupliquer inversé").click();
    s.wait();
    let st = s.studio();
    assert_eq!(st.settings.masks.len(), 3);
    let (a, b) = (&st.settings.masks[1], &st.settings.masks[2]);
    assert_eq!(a.shape, b.shape);
    assert!(b.invert != a.invert && b.local.is_neutral());
    s.capture("masque-duplique");
    // Annulable.
    s.keys(&[(Modifiers::COMMAND, Key::Z)]);
    assert_eq!(s.studio().settings.masks.len(), 2);
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
        // Le lien du dessous ouvre l'app Fichiers, sans demander Photos.
        let (photos, files) = (s.studio().import_asks, s.studio().files_asks);
        let link = s
            .harness
            .get_by_label("Depuis Fichiers (carte mémoire, iCloud Drive)")
            .rect()
            .center();
        s.harness.event(egui::Event::PointerMoved(link));
        for pressed in [true, false] {
            s.harness.event(egui::Event::PointerButton {
                pos: link,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
        }
        s.harness.event(egui::Event::PointerGone);
        for _ in 0..3 {
            s.harness.step();
        }
        assert_eq!(s.studio().files_asks, files + 1, "{name} : Fichiers");
        assert_eq!(s.studio().import_asks, photos, "{name} : pas Photos");
        // Sur iPhone, le menu « ••• » le propose aussi.
        if name.starts_with("iphone") {
            s.harness.get_by_label("Plus").click();
            for _ in 0..5 {
                s.harness.step();
            }
            s.harness.get_by_label("Importer depuis Fichiers").click();
            for _ in 0..3 {
                s.harness.step();
            }
            assert_eq!(s.studio().files_asks, files + 2, "menu « ••• »");
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
fn hdr_merge_is_offered() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("hdr") else {
        return;
    };
    let st = s.studio();
    st.tool = luma_studio::app::Tool::Merge;
    st.selected = [0, 1, 2].into_iter().collect();
    s.wait();
    // Le choix HDR de la fusion, avec son explication.
    s.harness.get_by_label("HDR").click();
    s.wait();
    assert_eq!(s.studio().stack_mode, luma_merge::StackMode::Hdr);
    s.harness
        .get_by_label("Bracketing : ombres et ciel détaillés, sans fantômes");
    s.capture("fusion-hdr");
    // Et l'empilement de mise au point.
    s.harness.get_by_label("Mise au point").click();
    s.wait();
    assert_eq!(s.studio().stack_mode, luma_merge::StackMode::Focus);
    s.capture("fusion-mise-au-point");
    // Le panorama : ses bords peuvent être déformés au lieu d'être rognés.
    s.studio().merge_stack = false;
    s.wait();
    assert!(!s.studio().pano_warp);
    s.harness.get_by_label("Déformer les bords").click();
    s.wait();
    assert!(s.studio().pano_warp);
    s.capture("fusion-panorama");
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
    // Un essai commencé il y a 40 jours (il dure un mois).
    let started = luma_studio::licence::now() - 40 * 86_400;
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

    // La photo affichée n'a pas de position : le viseur et le bouton la placent au
    // centre de la carte, dans son XMP et dans la photothèque.
    let current = s.studio().current_path().unwrap().to_path_buf();
    if s.studio()
        .catalog()
        .get(&current)
        .is_some_and(|e| e.gps.is_none())
    {
        let name = current.file_name().unwrap().to_string_lossy().into_owned();
        s.harness
            .get_by_label(&format!("Placer « {name} » ici"))
            .click();
        s.harness.step();
        s.harness.step();
        let gps = s.studio().catalog().get(&current).unwrap().gps.unwrap();
        let expected = luma_library::map::unproject(view.center);
        assert!((gps[0] - expected[0]).abs() < 1e-6 && (gps[1] - expected[1]).abs() < 1e-6);
        let side = std::fs::read_to_string(luma_project::xmp::sidecar_path(&current)).unwrap();
        assert!(
            luma_project::xmp::read_meta(&side).position.is_some(),
            "{side}"
        );
        assert!(
            s.harness
                .query_by_label(&format!("Placer « {name} » ici"))
                .is_none(),
            "placée : plus de bouton"
        );
    }
    // Deux photos choisies, sans position : placées d'un coup.
    let free: Vec<usize> = (3..photos.len())
        .filter(|&i| {
            s.studio()
                .catalog()
                .get(&photos[i])
                .is_some_and(|e| e.gps.is_none())
        })
        .take(2)
        .collect();
    if free.len() == 2 {
        s.studio().selected = free.iter().copied().collect();
        s.harness.step();
        s.harness.get_by_label("Placer les 2 photos ici").click();
        s.harness.step();
        s.harness.step();
        for &i in &free {
            assert!(
                s.studio().catalog().get(&photos[i]).unwrap().gps.is_some(),
                "{}",
                photos[i].display()
            );
        }
        assert!(
            s.harness
                .query_by_label("Placer les 2 photos ici")
                .is_none()
        );
        s.studio().selected.clear();
    }
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
    // Une trace GPX : la photo de Rome, sa position retirée, prise à l'heure de Paris
    // (+02:00), est placée sur la trace, l'écart de l'horloge deviné (la trace ne dure
    // que deux minutes : un autre écart ne la placerait pas).
    {
        let i = 1;
        let mut e = s.studio().catalog().get(&photos[i]).unwrap().clone();
        e.gps = None;
        s.studio().catalog_mut().insert(e);
        let date = s.studio().catalog().get(&photos[i]).unwrap().date.clone();
        let t = luma_library::geotag::parse_time(&date).unwrap() - 7200;
        // Un instant en texte, heure universelle (les jours civils depuis 1970).
        let iso = |t: i64| {
            let (d, r) = (t.div_euclid(86_400), t.rem_euclid(86_400));
            let z = d + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z - era * 146_097;
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let day = doy - (153 * mp + 2) / 5 + 1;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let year = yoe + era * 400 + i64::from(month <= 2);
            format!(
                "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
                r / 3600,
                r % 3600 / 60,
                r % 60
            )
        };
        let gpx = s.dir.join("trace.gpx");
        std::fs::write(
            &gpx,
            format!(
                "<gpx><trk><trkseg><trkpt lat=\"45.76\" lon=\"4.83\"><time>{}</time></trkpt><trkpt lat=\"45.78\" lon=\"4.85\"><time>{}</time></trkpt></trkseg></trk></gpx>",
                iso(t - 60),
                iso(t + 60)
            ),
        )
        .unwrap();
        // Comme une trace choisie dans l'app Fichiers (iPhone, iPad) : un autre fichier
        // avec elle ne place rien, la trace place la photo.
        let other = s.dir.join("notes.txt");
        std::fs::write(&other, "pas une trace").unwrap();
        assert_eq!(s.studio().take_picked_documents(vec![other]), 0);
        assert!(s.studio().take_picked_documents(vec![gpx.clone()]) >= 1);
        let gps = s.studio().catalog().get(&photos[i]).unwrap().gps.unwrap();
        assert!(
            (gps[0] - 45.77).abs() < 1e-3 && (gps[1] - 4.84).abs() < 1e-3,
            "au milieu de la trace : {gps:?}"
        );
        let side = std::fs::read_to_string(luma_project::xmp::sidecar_path(&photos[i])).unwrap();
        assert!(luma_project::xmp::read_meta(&side).position.is_some());
    }
}

#[test]
fn print_layout_and_pdf() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("imprimer") else {
        return;
    };
    let dir = s.dir.clone();
    // Trois photos choisies, sur une planche contact de deux par page, au profil d'un
    // écran plus terne que le sRGB (une « imprimante »).
    s.studio().selected = [0, 1, 2].into_iter().collect();
    let icc = dir.join("Terne.icc");
    std::fs::write(&icc, dull_profile()).unwrap();
    s.studio().load_proof_profile(&icc);
    s.wait();
    s.harness
        .get_all_by_label("Imprimer…")
        .next()
        .unwrap()
        .click();
    for _ in 0..3 {
        s.harness.step();
    }
    s.harness.get_by_label("Planche contact").click();
    for _ in 0..3 {
        s.harness.step();
    }
    let l = &s.studio().print_layout;
    assert_eq!((l.columns, l.rows, l.captions), (4, 5, true));
    s.capture("imprimer");
    {
        let l = &mut s.studio().print_layout;
        (l.columns, l.rows, l.dpi) = (2, 1, 100);
        l.paper = luma_render::print::paper("13x18").unwrap();
    }
    let pdf = dir.join("Tirage.pdf");
    s.studio().print_pdf(pdf.clone());
    s.wait();
    let bytes = std::fs::read(&pdf).expect("le PDF est écrit");
    let text = String::from_utf8_lossy(&bytes);
    assert!(bytes.starts_with(b"%PDF-1.4"));
    assert!(text.contains("/Count 2"), "trois photos, deux par page");
    assert!(text.contains("/MediaBox [0 0 360.00 504.00]"));
    // 13 × 18 cm (127 × 177,8 mm) à 100 ppp : 500 × 700 pixels par page.
    assert!(text.contains("/Width 500 /Height 700"));
    // Le PDF fait, « Ouvrir » le propose à l'app du système ; ici, sans elle
    // (`xdg-open` absent du conteneur d'essai), un message le dit.
    assert_eq!(s.studio().last_output.as_deref(), Some(pdf.as_path()));
    let name = pdf.file_name().unwrap().to_string_lossy().into_owned();
    for _ in 0..3 {
        s.harness.step();
    }
    assert!(
        s.harness
            .query_by_label(&format!("Ouvrir « {name} »"))
            .is_some(),
        "le bouton Ouvrir"
    );
    if std::process::Command::new("xdg-open")
        .arg("--version")
        .output()
        .is_err()
    {
        assert!(!s.studio().open_last_output());
    }
    // L'envoi direct à une imprimante du système : proposé pour le PDF ; ici, sans
    // service d'impression, l'envoi échoue et le dit.
    if luma_studio::printers::AVAILABLE {
        s.studio()
            .use_printers(vec!["Bureau".into(), "Salon_Photo".into()]);
        for _ in 0..3 {
            s.harness.step();
        }
        assert!(s.harness.query_by_label("Envoyer à l'imprimante").is_some());
        if std::process::Command::new("lp").arg("-h").output().is_err() {
            assert!(!s.studio().send_to_printer());
            assert!(s.studio().status().starts_with("Impression impossible"));
        }
        s.studio().use_printers(Vec::new());
        for _ in 0..3 {
            s.harness.step();
        }
        assert!(s.harness.query_by_label("Envoyer à l'imprimante").is_none());
        assert!(
            s.harness
                .query_by_label("Aucune imprimante trouvée : ouvrez le PDF pour l'imprimer.")
                .is_some()
        );
    }
    // Les mêmes pages en JPEG : une image par page, la première à ouvrir.
    let jpeg = dir.join("Planche.jpg");
    s.studio().print_pdf(jpeg.clone());
    s.wait();
    let first = dir.join("Planche-1.jpg");
    assert!(first.is_file() && dir.join("Planche-2.jpg").is_file());
    assert_eq!(s.studio().last_output.as_deref(), Some(first.as_path()));
    // Des pages en JPEG : pas d'envoi direct (le PDF seul).
    for _ in 0..3 {
        s.harness.step();
    }
    assert!(s.harness.query_by_label("Envoyer à l'imprimante").is_none());
}

#[test]
fn slideshow_and_photo_book() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("livre") else {
        return;
    };
    let dir = s.dir.clone();
    let all = s.studio().visible().len();
    s.keys(&[(NONE, Key::G)]);
    s.harness
        .get_all_by_label("Livre, diaporama, galerie…")
        .next()
        .unwrap()
        .click();
    for _ in 0..3 {
        s.harness.step();
    }
    s.capture("livre-diaporama");

    // Le diaporama dans l'app : il avance seul, l'espace le met en pause, Échap l'arrête.
    s.studio().show.seconds = 0.5;
    s.studio().show.fade = 0.4;
    s.studio().show.transition = luma_render::slideshow::Transition::Slide;
    s.harness.get_by_label("Lancer le diaporama").click();
    s.harness.step();
    assert!(s.studio().is_playing());
    let first = s.studio().current_path().unwrap().to_path_buf();
    let start = Instant::now();
    while s.studio().current_path() == Some(first.as_path()) {
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "le diaporama n'avance pas"
        );
        s.harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    // La photo d'avant reste à l'écran le temps du glissement, puis s'en va.
    assert!(
        s.studio().in_transition(),
        "la photo d'avant part en glissant"
    );
    s.wait();
    let start = Instant::now();
    while s.studio().in_transition() {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "la transition ne finit pas"
        );
        s.harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    s.capture("diaporama");
    s.keys(&[(NONE, Key::Space)]);
    assert!(!s.studio().is_playing(), "en pause");
    s.keys(&[(NONE, Key::Escape)]);
    assert!(!s.studio().is_playing());

    // Au hasard : chaque photo une fois avant de revenir à une déjà vue.
    s.studio().show.shuffle = true;
    s.studio().show.fade = 0.0;
    s.studio().show.seconds = 0.5;
    s.studio().start_slideshow();
    s.harness.step();
    let mut seen = vec![s.studio().current_path().unwrap().to_path_buf()];
    let start = Instant::now();
    while seen.len() < all {
        assert!(
            start.elapsed() < Duration::from_secs(90),
            "le diaporama n'avance pas : {seen:?}"
        );
        s.harness.step();
        std::thread::sleep(Duration::from_millis(20));
        let now = s.studio().current_path().unwrap().to_path_buf();
        if now != *seen.last().unwrap() {
            assert!(
                !seen.contains(&now),
                "{now:?} revient avant les autres : {seen:?}"
            );
            seen.push(now);
        }
    }
    s.keys(&[(NONE, Key::Escape)]);
    s.studio().show.shuffle = false;

    // Les pages du livre : chacune en petit ; deux photos échangées, le livre suit.
    s.studio().book_pages_open = true;
    for _ in 0..3 {
        s.harness.step();
    }
    s.capture("pages-du-livre");
    assert!(s.harness.query_by_label("Couverture").is_some());
    let before = s.studio().book_photos();
    s.studio().swap_book_photos(0, 2);
    let after = s.studio().book_photos();
    assert_eq!(after.len(), before.len());
    assert_eq!((&after[0], &after[2]), (&before[2], &before[0]));
    assert_eq!(after[1], before[1]);
    // Quatre photos sur la première page après la couverture, puis la disposition
    // automatique.
    s.harness.get_all_by_label("4").next().unwrap().click();
    s.harness.step();
    s.harness.step();
    assert_eq!(s.studio().book.pages, [4]);
    s.capture("pages-du-livre-quatre");
    s.harness.get_by_label("Disposition automatique").click();
    s.harness.step();
    assert!(s.studio().book.pages.is_empty());
    // Un vrai glisser‑déposer : la dernière photo lâchée sur le fond de la première page
    // après la couverture (sa marge) en devient la première photo.
    for _ in 0..2 {
        s.harness.step();
    }
    let photos = s.studio().book_photos();
    let name = |p: &std::path::Path| p.file_name().unwrap().to_string_lossy().into_owned();
    let (last, target) = (name(photos.last().unwrap()), name(&photos[1]));
    // Les photos du livre se nomment « fichier · page ».
    let find = |s: &Session, label: &str| {
        s.harness
            .get_all_by_label_contains(&format!("{label} · "))
            .next()
            .unwrap()
            .rect()
    };
    let from = find(&s, &last).center();
    // Dans la marge de la page, hors de la zone de la photo (élargie de quelques points
    // pour le toucher).
    let margin = find(&s, &target).min - egui::vec2(7.0, 7.0);
    s.harness.hover_at(from);
    s.harness.step();
    s.harness.drag_at(from);
    s.harness.step();
    for k in 1..=6 {
        s.harness
            .hover_at(from + (margin - from) * (k as f32 / 6.0));
        s.harness.step();
    }
    s.harness.drop_at(margin);
    s.harness.step();
    s.harness.step();
    let moved = s.studio().book_photos();
    assert_eq!(moved[1], *photos.last().unwrap(), "{moved:?}");
    assert_eq!(moved[2], photos[1], "{photos:?} → {moved:?}");
    assert_eq!(moved.len(), photos.len());
    s.studio().book_pages_open = false;
    // Le livre photo de tout le film, et le diaporama exporté.
    s.studio().book.dpi = 50;
    s.studio().book.title = "Corpus".into();
    let pdf = dir.join("Livre.pdf");
    s.studio().make_book(pdf.clone());
    s.wait();
    let text =
        String::from_utf8_lossy(&std::fs::read(&pdf).expect("le livre est écrit")).into_owned();
    assert!(text.contains("/MediaBox [0 0 566.93 566.93]"));
    s.studio().show.size = 320;
    let show = dir.join("Diaporama");
    s.studio().export_slideshow(show.clone());
    s.wait();
    let page = std::fs::read_to_string(show.join("index.html")).expect("le diaporama est écrit");
    assert_eq!(page.matches("\"photos/").count(), all);
    assert!(show.join(format!("photos/{all:03}.jpg")).is_file());
    // La galerie web : la page, les photos et les vignettes.
    s.studio().gallery.size = 400;
    s.studio().gallery.title = "Corpus".into();
    let site = dir.join("Galerie");
    s.studio().export_gallery(site.clone());
    s.wait();
    let page = std::fs::read_to_string(site.join("index.html")).expect("la galerie est écrite");
    assert!(page.contains("<h1>Corpus</h1>"));
    assert_eq!(page.matches("loading=\"lazy\"").count(), all);
    assert!(site.join(format!("vignettes/{all:03}.jpg")).is_file());
}

#[test]
fn import_from_a_memory_card() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("importer") else {
        return;
    };
    let dir = s.dir.clone();
    // Une « carte » : deux photos du corpus dans DCIM.
    let card = dir.join("CARTE/DCIM/100MSDCF");
    std::fs::create_dir_all(&card).unwrap();
    for name in ["_DSC0009.ARW", "DSC_0750.NEF"] {
        std::fs::hard_link(dir.join(name), card.join(name))
            .or_else(|_| std::fs::copy(dir.join(name), card.join(name)).map(|_| ()))
            .unwrap();
    }
    let (lib, second) = (dir.join("Photos"), dir.join("Second"));
    s.studio().set_import_source(dir.join("CARTE"));
    s.studio().import.dest = Some(lib.clone());
    s.studio().import.backup = Some(second.clone());
    s.studio().import.name = "essai_{séquence:2}".into();
    s.studio().wait_import_days();
    s.studio().open_import();
    for _ in 0..3 {
        s.harness.step();
    }
    s.capture("importer");
    // Deux jours sur la carte : en décocher un, puis le reprendre.
    let days = s.studio().import.days.clone();
    if days.len() == 2 {
        let label = format!("{} ({})", days[1].0, days[1].1);
        s.harness.get_by_label(&label).click();
        s.harness.step();
        s.harness.step();
        assert!(s.harness.query_by_label("Importer 1 photos").is_some());
        assert_eq!(s.studio().import.chosen_days(), [days[0].0.clone()]);
        s.harness.get_by_label(&label).click();
        s.harness.step();
        s.harness.step();
        assert!(
            s.studio().import.chosen_days().is_empty(),
            "tous : rien à filtrer"
        );
    }
    // Photo par photo : les aperçus de la carte, et une photo laissée dessus.
    s.harness.get_by_label("Choisir photo par photo").click();
    s.harness.step();
    let ctx = s.harness.ctx.clone();
    s.studio().wait_import_previews(&ctx);
    assert_eq!(
        s.studio().import_previews(),
        2,
        "les aperçus intégrés aux RAW"
    );
    let left = s
        .studio()
        .import
        .shots
        .iter()
        .position(|p| p.main().ends_with("DSC_0750.NEF"))
        .unwrap();
    s.studio().import.shots[left].on = false;
    s.harness.step();
    s.harness.step();
    s.capture("importer-photo-par-photo");
    // L'éjection, proposée pour une carte (un dossier DCIM) : ici un simple dossier,
    // que le système refuse d'éjecter ; l'import le dit sans échouer.
    s.harness
        .get_by_label("Éjecter la carte après l'import")
        .click();
    s.harness.step();
    assert!(s.studio().import.eject);
    s.harness.get_by_label("Importer 1 photos").click();
    s.harness.step();
    s.wait();
    // Rangées par date, renommées, sur les deux disques ; puis ouvertes.
    let imported = luma_library::import::photos(&lib);
    assert_eq!(imported.len(), 1, "{imported:?}");
    assert!(
        imported[0].extension().is_some_and(|x| x == "ARW"),
        "le NEF reste sur la carte"
    );
    assert!(imported.iter().all(|p| {
        p.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("essai_")
    }));
    assert_eq!(luma_library::import::photos(&second).len(), 1);
    assert!(
        card.join("_DSC0009.ARW").is_file(),
        "la carte reste intacte"
    );
    assert!(
        s.studio()
            .import
            .report
            .contains("la carte n'a pas pu être éjectée"),
        "{}",
        s.studio().import.report
    );
    let shown = s.studio().photos().to_vec();
    assert!(
        !shown.is_empty() && shown.iter().all(|p| p.starts_with(&lib)),
        "{shown:?}"
    );
}

#[test]
fn custom_shortcuts_and_hidden_panels() {
    use luma_studio::keys::Combo;
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("raccourcis") else {
        return;
    };
    let file = s.dir.join("Raccourcis.txt");
    s.studio().use_keys_file(file.clone());
    // B pour la Bibliothèque : G ne fait plus rien, B y va.
    assert_eq!(s.studio().set_shortcut("library", Combo::new(Key::B)), None);
    s.keys(&[(NONE, Key::G)]);
    assert_eq!(s.studio().module, Module::Develop);
    s.keys(&[(NONE, Key::B)]);
    assert_eq!(s.studio().module, Module::Library);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "library = B\n");
    // La fenêtre attend une touche : Maj+F pour le tri en plein écran, que F perd.
    s.studio().listen_shortcut("culling");
    s.keys(&[(Modifiers::SHIFT, Key::F)]);
    assert_eq!(
        s.studio().keymap.get("culling"),
        [Combo {
            shift: true,
            ..Combo::new(Key::F)
        }]
    );
    s.keys(&[(NONE, Key::F)]);
    assert!(!s.studio().is_culling(), "F seul ne fait plus rien");
    // Tab masque les panneaux, et les remontre.
    s.keys(&[(NONE, Key::Tab)]);
    assert!(s.studio().hide_panels);
    s.capture("panneaux-masques");
    s.keys(&[(NONE, Key::Tab)]);
    assert!(!s.studio().hide_panels);
    s.studio().open_shortcuts();
    for _ in 0..3 {
        s.harness.step();
    }
    s.harness.get_by_label("Raccourcis clavier");
    s.capture("raccourcis");
}

#[test]
fn workspaces_switch_tool_and_panels() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("espaces") else {
        return;
    };
    let file = s.dir.join("Espaces de travail.txt");
    s.studio().use_workspaces_file(file.clone());
    // Cmd+1 : le tri (la Bibliothèque) ; Cmd+4 : la présentation, panneaux masqués.
    s.keys(&[(Modifiers::COMMAND, Key::Num1)]);
    assert_eq!(s.studio().module, Module::Library);
    s.keys(&[(Modifiers::COMMAND, Key::Num4)]);
    assert_eq!(s.studio().tool, luma_studio::app::Tool::Light);
    assert!(s.studio().hide_panels);
    // Un espace à soi : la couleur, panneaux montrés, en cinquième place.
    s.keys(&[(Modifiers::COMMAND, Key::Num3)]);
    assert_eq!(s.studio().tool, luma_studio::app::Tool::Masks);
    assert!(!s.studio().hide_panels, "la retouche montre les panneaux");
    s.harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "Couleur")
        .click();
    s.wait();
    s.studio().save_workspace("Couleur seule");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "Couleur seule = Color, panneaux\n"
    );
    s.keys(&[
        (Modifiers::COMMAND, Key::Num1),
        (Modifiers::COMMAND, Key::Num5),
    ]);
    assert_eq!(s.studio().tool, luma_studio::app::Tool::Color);
    s.studio().remove_workspace("Couleur seule");
    assert!(s.studio().workspaces.is_empty());
}

/// L'écran Premium sur iPhone, pendant l'essai : les formules l'une sous l'autre, et
/// un toucher sur le voile (autour de la carte) le ferme au lieu de passer devant elle.
#[test]
fn premium_screen_on_iphone() {
    let _gpu = one_at_a_time();
    let dir = std::env::temp_dir().join("luma-premium-iphone");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut harness = Harness::builder()
        .with_size([440.0, 956.0])
        .with_pixels_per_point(2.0)
        .wgpu()
        .build_ui_state(
            move |ui, studio: &mut Option<Studio>| {
                studio
                    .get_or_insert_with(|| {
                        let mut s = Studio::new(ui.ctx());
                        s.use_licence_dir(dir.clone());
                        s.premium_open = true;
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
    image
        .save(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/studio-premium-iphone.png"))
        .unwrap();
    // « Fermer » répond (la carte est devant le voile).
    harness.get_by_label("Fermer").click();
    harness.step();
    harness.step();
    assert!(!harness.state().as_ref().unwrap().premium_open);
    // Rouverte, un toucher sur le voile la ferme aussi.
    harness.state_mut().as_mut().unwrap().premium_open = true;
    harness.step();
    harness.drag_at(egui::pos2(6.0, 950.0));
    harness.step();
    harness.drop_at(egui::pos2(6.0, 950.0));
    harness.step();
    harness.step();
    assert!(!harness.state().as_ref().unwrap().premium_open);
}

#[test]
fn an_idle_screen_does_not_redraw() {
    let _gpu = one_at_a_time();
    let dir = std::env::temp_dir().join(format!("luma-repos-{}", std::process::id()));
    let tmp = dir.clone();
    let mut harness = Harness::builder()
        .with_size([440.0, 956.0])
        .with_pixels_per_point(3.0)
        .build_ui_state(
            move |ui, studio: &mut Option<Studio>| {
                studio
                    .get_or_insert_with(|| {
                        let mut s = Studio::new(ui.ctx());
                        s.use_licence_dir(dir.join("licence"));
                        s.use_catalog(dir.join("cat.lumacat"));
                        s
                    })
                    .ui(ui);
            },
            None,
        );
    // Le temps que la licence soit vérifiée et le reste prêt.
    let start = Instant::now();
    loop {
        harness.step();
        let busy = harness
            .state()
            .as_ref()
            .is_some_and(|s| s.licence.busy() || s.is_indexing());
        if !busy || start.elapsed() > Duration::from_secs(30) {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    for _ in 0..5 {
        harness.step();
    }
    // Rien ne bouge : l'écran ne demande pas d'image de plus (la batterie).
    let repaint = harness.ctx.has_requested_repaint();
    let causes = harness.ctx.repaint_causes();
    drop(harness);
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(!repaint, "redessin demandé : {causes:?}");
}

/// La fenêtre d'entretien : un dossier déplacé hors de LÜMA s'y montre manquant, est
/// retrouvé (ses voisins aussi), puis une sauvegarde d'avant, restaurée après
/// confirmation, remet le catalogue tel qu'il était.
#[test]
fn library_upkeep_window() {
    let _gpu = one_at_a_time();
    let dir = std::env::temp_dir().join(format!("luma-entretien-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let old = dir.join("Voyage");
    for f in ["jour 1/a.png", "jour 2/b.png", "jour 2/c.png"] {
        let p = old.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"\x89PNG\r\n\x1a\n").unwrap();
    }
    let tmp = dir.clone();
    let mut harness = Harness::builder()
        .with_size([1280.0, 800.0])
        .build_ui_state(
            move |ui, studio: &mut Option<Studio>| {
                studio
                    .get_or_insert_with(|| {
                        let mut s = Studio::new(ui.ctx());
                        s.use_licence_dir(tmp.join("licence"));
                        s.use_catalog(tmp.join("cat.lumacat"));
                        s
                    })
                    .ui(ui);
            },
            None,
        );
    harness.step();
    let s = harness.state_mut().as_mut().unwrap();
    s.catalog_mut().add_folder(&old, true);
    s.backup_catalog();
    let new = dir.join("Archives/Voyage");
    std::fs::create_dir_all(new.parent().unwrap()).unwrap();
    std::fs::rename(&old, &new).unwrap();
    s.open_upkeep();
    assert_eq!(
        s.upkeep_missing().unwrap(),
        [(old.join("jour 1"), 1), (old.join("jour 2"), 2)]
    );
    harness.run_steps(3);
    harness.get_by_label("Entretien de la photothèque");
    harness.get_by_label("jour 2 (2)");
    harness.get_all_by_label("Retirer").next().unwrap().click();
    harness.run_steps(2);
    // « Retirer » : le premier dossier, « jour 1 », sort de la photothèque.
    let s = harness.state_mut().as_mut().unwrap();
    assert_eq!(s.upkeep_missing().unwrap(), [(old.join("jour 2"), 2)]);
    assert_eq!(s.catalog().len(), 2);
    assert_eq!(
        s.relocate_folder(&old.join("jour 2"), &new.join("jour 2")),
        2
    );
    assert!(s.upkeep_missing().unwrap().is_empty());
    harness.run_steps(2);
    harness.get_by_label("Toutes les photos sont là.");

    // La sauvegarde d'avant : demandée, confirmée, elle revient (les trois photos, à
    // leur ancienne place).
    harness.get_by_label("Restaurer…").click();
    harness.run_steps(2);
    harness.get_by_label("Confirmer").click();
    harness.run_steps(2);
    let s = harness.state_mut().as_mut().unwrap();
    assert_eq!(s.catalog().len(), 3);
    assert_eq!(s.upkeep_missing().unwrap().len(), 2);
    drop(harness);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Les photos par date, dans le panneau Photothèque : chaque année avec son nombre ;
/// le triangle déplie ses mois ; une année ou un mois montre ses photos.
#[test]
fn library_by_year_and_month() {
    let _gpu = one_at_a_time();
    let dir = std::env::temp_dir().join(format!("luma-dates-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let catalog = dir.join("cat.lumacat");
    let mut cat = luma_library::Catalog::default();
    for (name, date) in [
        ("a", "2024-06-15T10:00:00"),
        ("b", "2024-06-16T09:00:00"),
        ("c", "2024-01-02T08:00:00"),
        ("d", "2026-10-29T20:00:00"),
    ] {
        let path = dir.join(format!("{name}.png"));
        std::fs::write(&path, b"\x89PNG\r\n\x1a\n").unwrap();
        cat.insert(luma_library::Entry {
            path,
            date: date.into(),
            ..luma_library::Entry::default()
        });
    }
    // Une photo déjà faite (notée, avec mots‑clés et lieu), sans date : ni dans les
    // dates, ni dans « À faire ».
    let done = dir.join("e.png");
    std::fs::write(&done, b"\x89PNG\r\n\x1a\n").unwrap();
    cat.insert(luma_library::Entry {
        path: done,
        rating: 3,
        keywords: vec!["mer".into(), "coucher de soleil".into()],
        gps: Some([48.85, 2.35]),
        make: "SONY".into(),
        model: "ILCE-7M3".into(),
        lens: "FE 24-70mm F2.8 GM".into(),
        label: "Rouge".into(),
        ..luma_library::Entry::default()
    });
    cat.save(&catalog).unwrap();
    let tmp = dir.clone();
    let mut harness = Harness::builder()
        .with_size([1280.0, 1400.0])
        .build_ui_state(
            move |ui, studio: &mut Option<Studio>| {
                studio
                    .get_or_insert_with(|| {
                        let mut s = Studio::new(ui.ctx());
                        s.use_licence_dir(tmp.join("licence"));
                        s.use_catalog(tmp.join("cat.lumacat"));
                        s.tool = luma_studio::app::Tool::Library;
                        s
                    })
                    .ui(ui);
            },
            None,
        );
    harness.run_steps(3);
    harness.get_by_label("Dates");
    harness.get_by_label("2026 (1)");
    // « À faire » : les quatre photos sans note, sans mot‑clé, sans lieu.
    harness.get_by_label("À faire");
    harness.get_by_label("Pas encore notées (4)");
    harness.get_by_label("Sans mot-clé (4)");
    harness.get_by_label("Sans lieu (4)").click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().as_ref().unwrap().view,
        View::Search("sans:lieu".into())
    );
    // Les mots‑clés, repliés ; dépliés, un mot de plusieurs mots montre sa photo.
    assert!(harness.query_by_label("mer (1)").is_none());
    harness.get_by_label("Mots-clés (2)").click();
    harness.run_steps(2);
    harness.get_by_label("mer (1)");
    harness.get_by_label("coucher de soleil (1)").click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().as_ref().unwrap().view,
        View::Search("mot:\"coucher de soleil\"".into())
    );
    // Les boîtiers et les objectifs, de même : le nom entier, espaces compris.
    harness.get_by_label("Boîtiers (1)").click();
    harness.run_steps(2);
    harness.get_by_label("SONY ILCE-7M3 (1)").click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().as_ref().unwrap().view,
        View::Search("boîtier:\"SONY ILCE-7M3\"".into())
    );
    harness.get_by_label("Objectifs (1)").click();
    harness.run_steps(2);
    harness.get_by_label("FE 24-70mm F2.8 GM (1)").click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().as_ref().unwrap().view,
        View::Search("objectif:\"FE 24-70mm F2.8 GM\"".into())
    );
    // Les notes (seule la note 3 est donnée) et les libellés.
    harness.get_by_label("Notes").click();
    harness.run_steps(2);
    harness.get_by_label("3 étoiles (1)").click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().as_ref().unwrap().view,
        View::Search("note:=3".into())
    );
    harness.get_by_label("Libellés (1)").click();
    harness.run_steps(2);
    harness.get_by_label("Rouge (1)").click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().as_ref().unwrap().view,
        View::Search("libellé:Rouge".into())
    );
    harness.get_by_label("Sans lieu (4)").click();
    harness.run_steps(2);
    // La recherche devient une collection dynamique, nommée, puis supprimée par son
    // menu (clic droit) ; ses photos restent.
    harness
        .get_by_label("Enregistrer comme collection…")
        .click();
    harness.run_steps(3);
    harness
        .get_all_by_role(egui::accesskit::Role::TextInput)
        .find(|n| n.is_focused())
        .expect("le nom, prêt à écrire")
        .type_text("À indexer");
    harness.run_steps(2);
    harness
        .get_all_by_label("Enregistrer")
        .next()
        .unwrap()
        .click();
    harness.run_steps(3);
    assert_eq!(
        harness.state().as_ref().unwrap().view,
        View::Collection("À indexer".into())
    );
    // Renommée : affichée, elle le reste sous son nouveau nom ; puis supprimée par son
    // menu (clic droit) ; ses photos restent.
    assert!(
        harness
            .state_mut()
            .as_mut()
            .unwrap()
            .rename_collection("À indexer", "À ranger")
    );
    harness.run_steps(2);
    assert_eq!(
        harness.state().as_ref().unwrap().view,
        View::Collection("À ranger".into())
    );
    assert!(harness.query_by_label("À indexer (4)").is_none());
    harness.get_by_label("À ranger (4)").click_secondary();
    harness.run_steps(3);
    harness.get_by_label("Renommer");
    harness.get_by_label("Supprimer la collection").click();
    harness.run_steps(3);
    assert!(harness.query_by_label("À ranger (4)").is_none());
    assert_eq!(harness.state().as_ref().unwrap().view, View::All);
    assert_eq!(harness.state().as_ref().unwrap().catalog().len(), 5);
    assert!(
        harness.query_by_label("juin (2)").is_none(),
        "les mois repliés"
    );
    harness.get_by_label("2024 (3)").click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().as_ref().unwrap().view,
        View::Search("date:2024".into())
    );
    // Le triangle de 2024 (la seconde année de la liste) déplie ses mois.
    harness
        .get_all_by_label("Les mois de cette année")
        .nth(1)
        .unwrap()
        .click();
    harness.run_steps(2);
    harness.get_by_label("janvier (1)");
    harness.get_by_label("juin (2)").click();
    harness.run_steps(2);
    assert_eq!(
        harness.state().as_ref().unwrap().view,
        View::Search("date:2024-06".into())
    );
    // La liste des photos montrées, en tableau : toutes les photos, une ligne chacune.
    harness.state_mut().as_mut().unwrap().show_view(View::All);
    harness.run_steps(2);
    let list = dir.join("liste.csv");
    assert_eq!(harness.state_mut().as_mut().unwrap().export_list(&list), 5);
    let csv = std::fs::read_to_string(&list).unwrap();
    assert!(csv.starts_with("Fichier,Date,Boîtier,Objectif"), "{csv}");
    assert_eq!(csv.lines().count(), 6);
    assert!(csv.contains("SONY ILCE-7M3,FE 24-70mm F2.8 GM"), "{csv}");
    // Le dossier retiré de la photothèque par son menu : plus aucune photo suivie, les
    // fichiers restent.
    // Au sommet de l'arbre des dossiers, tout son chemin.
    let folder = format!("{} (5)", dir.display());
    harness.get_by_label(&folder).click_secondary();
    harness.run_steps(3);
    harness.get_by_label("Retirer de la photothèque").click();
    harness.run_steps(3);
    assert_eq!(harness.state().as_ref().unwrap().catalog().len(), 0);
    assert!(harness.query_by_label(&folder).is_none());
    assert!(dir.join("a.png").is_file());
    drop(harness);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Le menu « Collection » des photos : la photo ouverte rangée dans une collection
/// manuelle nouvelle (pas deux fois), montrée, puis retirée par le menu.
#[test]
fn photos_in_a_manual_collection() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("collection-manuelle") else {
        return;
    };
    // Le dossier entre dans la photothèque en arrière‑plan.
    let start = Instant::now();
    while s.studio().is_indexing() && start.elapsed() < Duration::from_secs(60) {
        s.harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    s.studio().tool = luma_studio::app::Tool::Library;
    s.harness.run_steps(3);
    assert!(s.harness.query_all_by_label("Collection").next().is_some());
    assert_eq!(s.studio().add_to_collection("Choix"), 1);
    assert_eq!(s.studio().add_to_collection("Choix"), 0, "pas deux fois");
    assert_eq!(
        s.studio()
            .catalog()
            .collection_photos("Choix")
            .unwrap()
            .len(),
        1
    );
    s.studio().show_view(View::Collection("Choix".into()));
    s.wait();
    s.harness
        .get_all_by_label("Collection")
        .next()
        .unwrap()
        .click();
    s.harness.run_steps(3);
    s.harness.get_by_label("Retirer de « Choix »").click();
    s.harness.run_steps(3);
    assert!(
        s.studio()
            .catalog()
            .collection_photos("Choix")
            .unwrap()
            .is_empty()
    );
    assert!(s.studio().status().contains("retirées de « Choix »"));
}

/// Un mot‑clé renommé partout, puis supprimé partout : les XMP des photos et les listes
/// de la photothèque suivent ; le reste des métadonnées (la note) ne bouge pas.
#[test]
fn a_keyword_renamed_and_deleted_everywhere() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open("mot-cle-partout") else {
        return;
    };
    let start = Instant::now();
    while s.studio().is_indexing() && start.elapsed() < Duration::from_secs(60) {
        s.harness.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    let path = s.studio().current_path().unwrap().to_path_buf();
    s.keys(&[(NONE, Key::Num3)]);
    assert_eq!(s.studio().add_keywords("mer, Été"), 1);
    s.studio().flush_xmp();
    s.studio().catalog_mut().refresh_meta(&path);
    assert_eq!(s.studio().rename_keyword_everywhere("MER", "Océan"), 1);
    let meta = luma_studio::worker::read_meta(&path);
    assert_eq!(meta.keywords, ["Océan", "Été"]);
    assert_eq!(meta.rating, 3, "la note reste");
    assert!(
        s.studio()
            .catalog()
            .keywords()
            .iter()
            .any(|(k, n)| k == "Océan" && *n == 1)
    );
    // Renommé en un mot déjà là : pas deux fois.
    assert_eq!(s.studio().rename_keyword_everywhere("Océan", "été"), 1);
    assert_eq!(luma_studio::worker::read_meta(&path).keywords, ["été"]);
    assert_eq!(s.studio().delete_keyword_everywhere("Ete"), 1);
    assert!(luma_studio::worker::read_meta(&path).keywords.is_empty());
    assert!(s.studio().catalog().keywords().is_empty());
    assert!(s.studio().status().contains("retiré de 1 photos"));
}

/// L'arbre des dossiers : le sommet, déplié, montre ses sous‑dossiers avec leurs
/// nombres ; le triangle les replie et les déplie ; un clic ouvre un sous‑dossier.
#[test]
fn library_folders_as_a_tree() {
    let _gpu = one_at_a_time();
    let dir = std::env::temp_dir().join(format!("luma-arbre-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut cat = luma_library::Catalog::default();
    for (sub, name) in [("Mer", "a.png"), ("Mer", "b.png"), ("Montagne", "c.png")] {
        let path = dir.join("Photos").join(sub).join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        image::RgbImage::from_pixel(8, 8, image::Rgb([90, 120, 200]))
            .save(&path)
            .unwrap();
        cat.insert(luma_library::Entry {
            path,
            date: "2026-06-01T10:00:00".into(),
            ..luma_library::Entry::default()
        });
    }
    cat.save(&dir.join("cat.lumacat")).unwrap();
    let tmp = dir.clone();
    let mut harness = Harness::builder()
        .with_size([1280.0, 1400.0])
        .build_ui_state(
            move |ui, studio: &mut Option<Studio>| {
                studio
                    .get_or_insert_with(|| {
                        let mut s = Studio::new(ui.ctx());
                        s.use_licence_dir(tmp.join("licence"));
                        s.use_catalog(tmp.join("cat.lumacat"));
                        s.tool = luma_studio::app::Tool::Library;
                        s
                    })
                    .ui(ui);
            },
            None,
        );
    harness.run_steps(3);
    harness.get_by_label(&format!("{} (3)", dir.join("Photos").display()));
    harness.get_by_label("Mer (2)");
    harness.get_by_label("Montagne (1)");
    harness.get_by_label("Les sous-dossiers").click();
    harness.run_steps(2);
    assert!(harness.query_by_label("Mer (2)").is_none(), "replié");
    harness.get_by_label("Les sous-dossiers").click();
    harness.run_steps(2);
    harness.get_by_label("Montagne (1)").click();
    harness.run_steps(3);
    let s = harness.state().as_ref().unwrap();
    assert_eq!(s.view, View::Folder);
    assert!(
        s.current_path()
            .is_some_and(|p| p.ends_with("Montagne/c.png"))
    );
    // « Toutes ses photos » : le dossier et ses sous‑dossiers d'un coup.
    let top = dir.join("Photos");
    harness
        .get_by_label(&format!("{} (3)", top.display()))
        .click_secondary();
    harness.run_steps(3);
    harness.get_by_label("Toutes ses photos").click();
    harness.run_steps(3);
    let s = harness.state().as_ref().unwrap();
    assert_eq!(
        s.view,
        View::Search(luma_library::query::filter(
            "dossier",
            &top.display().to_string()
        ))
    );
    assert_eq!(s.photos().len(), 3);
    drop(harness);
    let _ = std::fs::remove_dir_all(&dir);
}

/// « Tout remettre à zéro » : la photo ouverte et une autre choisie perdent leurs
/// réglages (la note reste, et Annuler rend ceux de l'ouverte), un raccourci changé
/// revient, la langue choisie est oubliée, les panneaux masqués reviennent.
#[test]
fn everything_is_reset_after_a_confirmation() {
    let _gpu = one_at_a_time();
    let Some(mut s) = Session::open_sized("tout-a-zero", [1440.0, 1000.0], 1.0) else {
        return;
    };
    let photos = s.studio().photos().to_vec();
    let open = s.studio().current_path().unwrap().to_path_buf();
    let k = photos.iter().position(|p| *p == open).unwrap();
    let other = (k + 1) % photos.len();
    let edited = luma_project::Settings {
        exposure: 1.5,
        ..luma_project::Settings::default()
    };
    let meta = luma_project::xmp::Meta {
        rating: 3,
        ..luma_project::xmp::Meta::default()
    };
    luma_studio::worker::write_sidecar(&photos[other], &edited, &meta).unwrap();
    // +0,8 IL gardé dans l'historique (copié puis collé).
    s.studio().settings.exposure = 0.8;
    let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
    s.keys(&[(cmd_shift, Key::C), (cmd_shift, Key::V)]);
    s.studio().selected = [k, other].into_iter().collect();
    let keys = s.dir.join("Raccourcis.txt");
    s.studio().use_keys_file(keys.clone());
    s.studio()
        .set_shortcut("library", luma_studio::keys::Combo::new(Key::B));
    let lang = s.dir.join("langue.txt");
    std::fs::write(&lang, "es").unwrap();
    s.studio().use_language_file(lang.clone());
    s.studio().hide_panels = true;

    // La fenêtre dit combien de photos ; Annuler ne touche à rien.
    s.studio().reset_all_open = true;
    s.wait();
    s.harness.get_by_label_contains("de 2 photo(s)");
    // (le dernier « Annuler » : celui de la fenêtre, pas celui de l'historique)
    s.harness
        .get_all_by_label("Annuler")
        .last()
        .unwrap()
        .click();
    s.wait();
    assert!(!s.studio().reset_all_open);
    assert_eq!(s.studio().settings.exposure, 0.8);

    s.studio().reset_all_open = true;
    s.wait();
    s.capture("tout-a-zero");
    s.harness
        .get_by_role_and_label(egui::accesskit::Role::Button, "Tout remettre à zéro")
        .click();
    s.wait();
    let now = luma_studio::i18n::lang();
    // La langue d'avant pour les essais suivants.
    luma_studio::i18n::set_lang(luma_studio::i18n::Lang::Fr);
    assert!(!s.studio().reset_all_open);
    assert_eq!(s.studio().settings.exposure, 0.0);
    assert!(luma_studio::worker::read_sidecar(&photos[other]).is_none());
    assert_eq!(luma_studio::worker::read_meta(&photos[other]).rating, 3);
    assert!(!lang.exists(), "la langue choisie est oubliée");
    assert_eq!(now, luma_studio::i18n::system());
    assert_eq!(
        std::fs::read_to_string(&keys).unwrap_or_default().trim(),
        ""
    );
    assert!(!s.studio().hide_panels);
    // Annuler rend les réglages de la photo ouverte.
    s.keys(&[(Modifiers::COMMAND, Key::Z)]);
    assert_eq!(s.studio().settings.exposure, 0.8);
}
