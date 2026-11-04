//! `luma clipping` : la part brûlée et bouchée de chaque photo, la plus écrêtée d'abord ;
//! le drapeau « rejetée » dans le XMP de celles qui passent le seuil.

use std::process::Command;

#[test]
fn the_clipped_photos_come_first_and_are_rejected() {
    let dir = std::env::temp_dir().join(format!("luma-cli-ecretage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // Par nom, la juste d'abord ; l'écrêtée : un tiers blanc, un sixième noir.
    image::RgbImage::from_fn(600, 400, |x, y| {
        let v = 70 + ((x + y) % 90) as u8;
        image::Rgb([v, v + 10, v])
    })
    .save(dir.join("a-juste.png"))
    .unwrap();
    image::RgbImage::from_fn(600, 400, |x, _| match x {
        0..200 => image::Rgb([255, 255, 255]),
        200..300 => image::Rgb([0, 0, 0]),
        _ => {
            let v = 60 + (x % 100) as u8;
            image::Rgb([v, v, v])
        }
    })
    .save(dir.join("b-ecretee.png"))
    .unwrap();
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_luma"))
            .arg("clipping")
            .args(args)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&o.stdout).into_owned();
        assert!(o.status.success(), "{text}{}", String::from_utf8_lossy(&o.stderr));
        text
    };
    let percent = |line: &str, word: &str| -> f32 {
        let at = line.find(word).unwrap();
        line[..at]
            .rsplit("  ")
            .next()
            .unwrap()
            .trim()
            .trim_end_matches(" %")
            .trim()
            .parse()
            .unwrap()
    };
    // Sans option : la mesure seule, l'écrêtée d'abord, aucun XMP écrit.
    let text = run(&[dir.to_str().unwrap()]);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].ends_with("b-ecretee.png"), "{text}");
    assert!(lines[1].ends_with("a-juste.png"), "{text}");
    assert!((percent(lines[0], "% brûlé") - 33.3).abs() < 2.0, "{text}");
    assert!((percent(lines[0], "% bouché") - 16.7).abs() < 2.0, "{text}");
    assert!(
        percent(lines[1], "% brûlé") < 0.1 && percent(lines[1], "% bouché") < 0.1,
        "{text}"
    );
    assert!(luma_library::photo_meta(&dir.join("b-ecretee.png")).flag == luma_project::xmp::Flag::None);
    // --reject 5 : l'écrêtée rejetée, la juste sans drapeau.
    let text = run(&[dir.to_str().unwrap(), "--reject", "5"]);
    assert!(text.contains("b-ecretee.png — rejetée"), "{text}");
    assert!(
        text.ends_with("1 photo(s) rejetée(s) (brûlées ou bouchées sur 5 % au moins)\n"),
        "{text}"
    );
    use luma_project::xmp::Flag;
    assert_eq!(luma_library::photo_meta(&dir.join("b-ecretee.png")).flag, Flag::Reject);
    assert_eq!(luma_library::photo_meta(&dir.join("a-juste.png")).flag, Flag::None);
    // Un seuil impossible est refusé.
    let o = Command::new(env!("CARGO_BIN_EXE_luma"))
        .args(["clipping", dir.to_str().unwrap(), "--reject", "0"])
        .output()
        .unwrap();
    assert!(!o.status.success());
    let _ = std::fs::remove_dir_all(&dir);
}
