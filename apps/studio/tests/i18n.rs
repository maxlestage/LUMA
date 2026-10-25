//! Les langues de l'app : le catalogue (`apps/studio/i18n.txt`) est bien formé, chaque
//! texte de l'interface y a sa traduction anglaise et espagnole, les trous `{nom}` sont
//! gardés, et toute l'interface se dessine en anglais et en espagnol.
//!
//! Un binaire de test à part : la langue y change, sans toucher aux captures (en
//! français) de `screenshot.rs`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use luma_i18n::{Catalog, Lang, literals};
use luma_studio::Studio;
use luma_studio::app::{Module, Tool};
use luma_studio::i18n;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{} : {e}", path.display()))
}

/// Le source sans ses tests.
fn without_tests(src: &str) -> &str {
    src.split("#[cfg(test)]").next().unwrap_or(src)
}

/// Les `label: "…"` d'un source (les curseurs, traduits à l'affichage).
fn labels(src: &str) -> Vec<String> {
    src.split("label: \"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_owned)
        .collect()
}

/// Tous les textes de l'app à traduire : les `tr("…")` et `trf("…")` du code, les
/// libellés des curseurs, et les textes venus des autres crates (familles et
/// descriptions des films, modes de fusion, formules, messages de licence…).
fn texts() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut add = |from: &str, list: Vec<String>| {
        out.extend(list.into_iter().map(|t| (from.to_owned(), t)));
    };
    let dir = root().join("apps/studio/src");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .collect();
    files.sort();
    assert!(
        files.len() >= 10,
        "les sources de l'app : {}",
        dir.display()
    );
    for f in &files {
        let src = read(f);
        let src = without_tests(&src);
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        add(&name, literals(src, "tr"));
        add(&name, literals(src, "trf"));
        add(&name, literals(src, "spec"));
        add(&name, literals(src, "field"));
        add(&name, labels(src));
    }

    use luma_project::presets::{Group, PRESETS};
    add(
        "luma-project",
        Group::ALL.iter().map(|g| g.name().to_owned()).collect(),
    );
    add(
        "luma-project",
        PRESETS.iter().map(|p| p.description.to_owned()).collect(),
    );
    // Les noms des looks (pas ceux des pellicules, des noms propres) : « Traitement
    // croisé », « Heure dorée »…
    add(
        "luma-project",
        PRESETS
            .iter()
            .filter(|p| !p.description.starts_with("Inspiré") || p.name == "Infrarouge")
            .map(|p| p.name.to_owned())
            .collect(),
    );
    add(
        "luma-project",
        [
            luma_project::Mask::linear(),
            luma_project::Mask::radial(),
            luma_project::Mask::brush(),
            luma_project::Mask::full(),
        ]
        .into_iter()
        .map(|m| m.name)
        .collect(),
    );
    let presets_src = read(&root().join("crates/luma-project/src/presets.rs"));
    add(
        "luma-project",
        literals(without_tests(&presets_src), "panels.push"),
    );

    use luma_merge::{Projection, StackMode};
    add(
        "luma-merge",
        [
            StackMode::Mean,
            StackMode::Median,
            StackMode::Maximum,
            StackMode::Stars,
        ]
        .map(|m| m.name().to_owned())
        .to_vec(),
    );
    add(
        "luma-merge",
        [
            Projection::Perspective,
            Projection::Cylindrical,
            Projection::Spherical,
        ]
        .map(|p| p.name().to_owned())
        .to_vec(),
    );
    for f in ["stack.rs", "pano.rs"] {
        let src = read(&root().join("crates/luma-merge/src").join(f));
        let src = without_tests(&src);
        add("luma-merge", literals(src, "progress"));
        add("luma-merge", literals(src, "step"));
    }

    use luma_licence::Plan;
    for p in Plan::ALL {
        add(
            "luma-licence",
            [p.name(), p.price(), p.period(), p.note()]
                .map(str::to_owned)
                .to_vec(),
        );
    }
    let licence_src = read(&root().join("crates/luma-licence/src/lib.rs"));
    let licence_src = without_tests(&licence_src);
    for f in ["Invalid", "Offline", "ok_or"] {
        add("luma-licence", literals(licence_src, f));
    }
    // Les refus de `verify` écrits en `map_err(|_| "…")`.
    add(
        "luma-licence",
        [
            "clé publique invalide",
            "clé non reconnue (signature fausse)",
        ]
        .map(str::to_owned)
        .to_vec(),
    );
    out.retain(|(_, t)| t.chars().any(char::is_alphabetic));
    out
}

/// Les trous `{nom}` d'un texte.
fn holes(text: &str) -> Vec<&str> {
    let mut v: Vec<&str> = text
        .split('{')
        .skip(1)
        .filter_map(|r| r.split_once('}').map(|x| x.0))
        .filter(|h| !h.is_empty() && h.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn the_catalog_is_well_formed() {
    let problems = Catalog::check(i18n::SOURCE);
    assert!(problems.is_empty(), "{problems:#?}");
}

#[test]
fn every_text_of_the_interface_is_translated() {
    let catalog = i18n::catalog();
    let mut missing: Vec<String> = texts()
        .into_iter()
        .filter(|(_, t)| !catalog.has(t))
        .map(|(from, t)| format!("{from} : {t:?}"))
        .collect();
    missing.sort();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "{} textes sans traduction dans apps/studio/i18n.txt :\n{}",
        missing.len(),
        missing.join("\n")
    );
}

#[test]
fn the_holes_are_kept() {
    let catalog = i18n::catalog();
    let mut wrong = Vec::new();
    for fr in catalog.keys() {
        for lang in [Lang::En, Lang::Es] {
            let t = catalog.get(lang, fr);
            if holes(fr) != holes(t) {
                wrong.push(format!("{fr:?} → {t:?}"));
            }
            if t.trim().is_empty() {
                wrong.push(format!("{fr:?} : vide en {}", lang.code()));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Attend que `cond` soit vraie (le rendu en arrière‑plan), au plus trois minutes.
fn wait(harness: &mut Harness<'static, Option<Studio>>, cond: impl Fn(&Studio) -> bool) {
    let start = Instant::now();
    loop {
        harness.step();
        if harness.state().as_ref().is_some_and(&cond) {
            for _ in 0..3 {
                harness.step();
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

/// L'interface dessinée dans chaque langue (sans panique), avec chaque outil ; les
/// captures sont écrites dans `target/studio-<langue>-*.png`.
#[test]
fn the_interface_draws_in_every_language() {
    let tmp = std::env::temp_dir().join("luma-studio-langues");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    // L'écran Premium d'un essai fini : un dossier de licence à part.
    let expired = tmp.join("licence-finie");
    std::fs::create_dir_all(&expired).unwrap();
    std::fs::write(
        expired.join("licence.json"),
        luma_licence::State::new(luma_studio::licence::now() - 20 * 86_400).to_json(),
    )
    .unwrap();
    // Une photo du corpus, si elle est là.
    let corpus = root().join("tests/raw-corpus/_DSC0009.ARW");
    let raw = corpus.exists().then(|| {
        let copy = tmp.join("_DSC0009.ARW");
        std::fs::copy(&corpus, &copy).unwrap();
        copy
    });
    let out = |name: &str| root().join(format!("target/studio-{name}.png"));

    for lang in [Lang::En, Lang::Es] {
        i18n::set_lang(lang);
        assert_eq!(i18n::lang(), lang);
        assert_ne!(i18n::tr("Exporter"), "Exporter");
        let code = lang.code();

        // L'écran Premium.
        let dir = expired.clone();
        let mut h = Harness::builder()
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
            h.step();
        }
        h.render()
            .expect("rendu de l'interface")
            .save(out(&format!("{code}-premium")))
            .unwrap();
        drop(h);

        // Une photo, et chaque outil.
        let Some(raw) = raw.clone() else {
            eprintln!("corpus absent (scripts/fetch-corpus.sh) : outils non dessinés");
            continue;
        };
        let licence = tmp.join(format!("licence-{code}"));
        let mut h = Harness::builder()
            .with_size([1440.0, 900.0])
            .wgpu()
            .build_ui_state(
                move |ui, studio: &mut Option<Studio>| {
                    studio
                        .get_or_insert_with(|| {
                            let mut s = Studio::new(ui.ctx());
                            s.use_licence_dir(licence.clone());
                            s.open_path(&raw);
                            s
                        })
                        .ui(ui);
                },
                None,
            );
        wait(&mut h, Studio::is_idle);
        for tool in Tool::ALL {
            let s = h.state_mut().as_mut().unwrap();
            s.tool = tool;
            s.module = if tool == Tool::Library {
                Module::Library
            } else {
                Module::Develop
            };
            if tool == Tool::Masks {
                s.settings.masks.push(luma_project::Mask::radial());
                s.selected_mask = Some(0);
            }
            if tool == Tool::Merge {
                s.selected.insert(0);
            }
            for _ in 0..3 {
                h.step();
            }
            if matches!(tool, Tool::Light | Tool::Films | Tool::Masks) {
                wait(&mut h, Studio::is_idle);
                h.render()
                    .expect("rendu de l'interface")
                    .save(out(&format!(
                        "{code}-{}",
                        format!("{tool:?}").to_lowercase()
                    )))
                    .unwrap();
            }
        }
    }
    i18n::set_lang(Lang::Fr);
}
