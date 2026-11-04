//! `luma sharpness` : la part au point de chaque photo, la plus nette d'abord ; les
//! drapeaux « retenue » et « rejetée » dans les XMP.

use std::process::Command;

#[test]
fn the_sharpest_is_picked_and_the_blurred_rejected() {
    let dir = std::env::temp_dir().join(format!("luma-cli-nettete-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sharp = image::RgbImage::from_fn(1200, 800, |x, y| {
        let v = (((x / 3) ^ (y / 5)) % 7) as u8 * 30 + ((x * y) % 17) as u8;
        image::Rgb([v, v / 2 + 60, 255 - v])
    });
    // Par nom, la floue d'abord ; une copie un peu adoucie, ni la plus nette ni floue.
    image::imageops::blur(&sharp, 4.0)
        .save(dir.join("a-floue.png"))
        .unwrap();
    image::imageops::blur(&sharp, 0.6)
        .save(dir.join("b-douce.png"))
        .unwrap();
    sharp.save(dir.join("c-nette.png")).unwrap();
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_luma"))
            .arg("sharpness")
            .args(args)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&o.stdout).into_owned();
        assert!(o.status.success(), "{text}{}", String::from_utf8_lossy(&o.stderr));
        text
    };
    // Sans option : la mesure seule, aucun XMP écrit.
    let text = run(&[dir.to_str().unwrap()]);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].ends_with("c-nette.png"), "{text}");
    assert!(lines[1].ends_with("b-douce.png"), "{text}");
    assert!(lines[2].ends_with("a-floue.png — floue"), "{text}");
    assert!(!dir.join("c-nette.png.xmp").exists() && !dir.join("c-nette.xmp").exists());
    let text = run(&[dir.to_str().unwrap(), "--pick", "--reject"]);
    assert!(text.contains("c-nette.png — retenue"), "{text}");
    assert!(text.contains("a-floue.png — rejetée"), "{text}");
    assert!(
        text.ends_with("1 photo(s) rejetée(s) (moins du tiers de la plus nette)\n"),
        "{text}"
    );
    let flag = |name: &str| {
        let p = dir.join(name);
        luma_library::photo_meta(&p).flag
    };
    use luma_project::xmp::Flag;
    assert_eq!(flag("c-nette.png"), Flag::Pick);
    assert_eq!(flag("b-douce.png"), Flag::None);
    assert_eq!(flag("a-floue.png"), Flag::Reject);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `--bursts` : rafale par rafale (deux rafales à dix minutes d'écart, d'après la date
/// des fichiers), la plus nette de chacune retenue, la floue de chacune rejetée ; la
/// photo seule mesurée, sans drapeau.
#[test]
fn burst_by_burst() {
    let dir = std::env::temp_dir().join(format!("luma-cli-rafales-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let scene = |k: u32| {
        image::RgbImage::from_fn(1200, 800, move |x, y| {
            let v = (((x / 3) ^ (y / (5 + k))) % 7) as u8 * 30 + ((x * y) % 17) as u8;
            image::Rgb([v, v / 2 + 60, 255 - v])
        })
    };
    image::imageops::blur(&scene(0), 4.0).save(dir.join("a-1.png")).unwrap();
    scene(0).save(dir.join("a-2.png")).unwrap();
    scene(1).save(dir.join("b-1.png")).unwrap();
    image::imageops::blur(&scene(1), 4.0).save(dir.join("b-2.png")).unwrap();
    image::imageops::blur(&scene(2), 4.0)
        .save(dir.join("seule.png"))
        .unwrap();
    let start = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    for (name, minutes) in [
        ("a-1.png", 0),
        ("a-2.png", 0),
        ("b-1.png", 10),
        ("b-2.png", 10),
        ("seule.png", 20),
    ] {
        let file = std::fs::File::options().write(true).open(dir.join(name)).unwrap();
        file.set_modified(start + std::time::Duration::from_secs(minutes * 60))
            .unwrap();
    }
    let o = Command::new(env!("CARGO_BIN_EXE_luma"))
        .args(["sharpness", dir.to_str().unwrap(), "--bursts", "--pick", "--reject"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(o.status.success(), "{text}{}", String::from_utf8_lossy(&o.stderr));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "Rafale 1 (2 photos)", "{text}");
    assert!(lines[1].ends_with("a-2.png — retenue"), "{text}");
    assert!(lines[2].ends_with("a-1.png — rejetée"), "{text}");
    assert_eq!(lines[3], "Rafale 2 (2 photos)", "{text}");
    assert!(lines[4].ends_with("b-1.png — retenue"), "{text}");
    assert!(lines[5].ends_with("b-2.png — rejetée"), "{text}");
    assert_eq!(lines[6], "Hors rafale", "{text}");
    assert!(lines[7].ends_with("seule.png"), "{text}");
    assert!(
        text.contains("2 photo(s) retenue(s) (la plus nette de chaque rafale)"),
        "{text}"
    );
    assert!(
        text.ends_with("2 photo(s) rejetée(s) (moins du tiers de la plus nette)\n"),
        "{text}"
    );
    use luma_project::xmp::Flag;
    let flag = |name: &str| luma_library::photo_meta(&dir.join(name)).flag;
    assert_eq!(
        ["a-1.png", "a-2.png", "b-1.png", "b-2.png", "seule.png"].map(flag),
        [Flag::Reject, Flag::Pick, Flag::Pick, Flag::Reject, Flag::None]
    );
    // --one : une seule par rafale, même la photo nette mais moins nette que la
    // meilleure rejetée ; la photo hors rafale reste sans drapeau.
    let c3 = dir.join("c-3.png");
    image::imageops::blur(&scene(3), 0.6).save(&c3).unwrap();
    scene(3).save(dir.join("c-4.png")).unwrap();
    for name in ["c-3.png", "c-4.png"] {
        let file = std::fs::File::options().write(true).open(dir.join(name)).unwrap();
        file.set_modified(start + std::time::Duration::from_secs(40 * 60))
            .unwrap();
    }
    let o = Command::new(env!("CARGO_BIN_EXE_luma"))
        .args(["sharpness", dir.to_str().unwrap(), "--bursts", "--one"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(o.status.success(), "{text}{}", String::from_utf8_lossy(&o.stderr));
    assert!(text.contains("c-4.png — retenue"), "{text}");
    assert!(text.contains("c-3.png — rejetée"), "{text}");
    assert!(
        text.ends_with("3 photo(s) rejetée(s) (le reste de chaque rafale)\n"),
        "{text}"
    );
    assert_eq!(
        ["c-3.png", "c-4.png", "seule.png"].map(flag),
        [Flag::Reject, Flag::Pick, Flag::None]
    );
    // Sans --bursts, refusé.
    let o = Command::new(env!("CARGO_BIN_EXE_luma"))
        .args(["sharpness", dir.to_str().unwrap(), "--one"])
        .output()
        .unwrap();
    assert!(!o.status.success());
    let _ = std::fs::remove_dir_all(&dir);
}
