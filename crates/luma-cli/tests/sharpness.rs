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
