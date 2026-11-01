//! `luma before-after` : l'avant / après de chaque photo d'un dossier, à côté d'elle.

use std::process::Command;

#[test]
fn every_photo_of_a_folder_gets_its_before_and_after() {
    let dir = std::env::temp_dir().join(format!("luma-cli-avant-apres-lot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, v) in [("a.png", 60u8), ("b.png", 180)] {
        image::RgbImage::from_fn(400, 300, |x, y| image::Rgb([v, (x % 255) as u8, (y % 255) as u8]))
            .save(dir.join(name))
            .unwrap();
    }
    let run = || {
        let o = Command::new(env!("CARGO_BIN_EXE_luma"))
            .args(["before-after", dir.to_str().unwrap(), "--size", "300"])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&o.stdout).into_owned();
        assert!(o.status.success(), "{text}{}", String::from_utf8_lossy(&o.stderr));
        text
    };
    let text = run();
    assert!(text.ends_with("2 avant / après écrit(s)\n"), "{text}");
    for name in ["a-avant-apres.jpg", "b-avant-apres.jpg"] {
        let img = image::open(dir.join(name)).unwrap();
        // Deux moitiés de 300 px de large et la bande entre elles.
        assert!(img.width() > 600 && img.width() < 640, "{name} : {}", img.width());
    }
    // Une seconde fois : les avant / après déjà là ne sont pas pris pour des photos, et
    // les nouveaux sont numérotés.
    let text = run();
    assert!(text.ends_with("2 avant / après écrit(s)\n"), "{text}");
    assert!(dir.join("a-avant-apres-2.jpg").exists() && dir.join("b-avant-apres-2.jpg").exists());
    let jpegs = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "jpg"))
        .count();
    assert_eq!(jpegs, 4);
    let _ = std::fs::remove_dir_all(&dir);
}
