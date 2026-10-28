//! `luma geotag` : des photos placées d'après une trace GPX, la position dans le XMP.

use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

fn luma(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_luma")).args(args).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    (out.status.success(), text)
}

const TRACE: &str = r#"<gpx version="1.1" xmlns="http://www.topografix.com/GPX/1/1"><trk><trkseg>
<trkpt lat="48.8606" lon="2.3376"><time>2026-06-01T08:10:00Z</time></trkpt>
<trkpt lat="48.8530" lon="2.3499"><time>2026-06-01T08:30:00Z</time></trkpt>
</trkseg></trk></gpx>"#;

#[test]
fn photos_follow_the_track() {
    let dir = std::env::temp_dir().join(format!("luma-cli-geotag-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("photos")).unwrap();
    let gpx = dir.join("trace.gpx");
    std::fs::write(&gpx, TRACE).unwrap();
    // Sans EXIF, la date de la photo est celle de son fichier (ici, en heure universelle).
    let photo = |name: &str, unix: u64| {
        let p = dir.join("photos").join(name);
        image::RgbImage::from_pixel(8, 8, image::Rgb([90, 120, 200]))
            .save(&p)
            .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(unix))
            .unwrap();
        p
    };
    let t0 = 1_780_301_400; // 2026-06-01T08:10:00Z
    let inside = photo("dedans.png", t0 + 600);
    let outside = photo("dehors.png", t0 + 86_400);
    let (dir_s, gpx_s) = (dir.join("photos").display().to_string(), gpx.display().to_string());

    // Un essai : rien d'écrit.
    let (ok, text) = luma(&["geotag", &dir_s, "--gpx", &gpx_s, "--dry-run"]);
    assert!(ok, "{text}");
    assert!(text.contains("1 photos placées (essai"), "{text}");
    assert!(!luma_project::xmp::sidecar_path(&inside).exists());

    let (ok, text) = luma(&["geotag", &dir_s, "--gpx", &gpx_s]);
    assert!(ok, "{text}");
    assert!(
        text.contains("48.856800, 2.343750"),
        "au milieu des deux points : {text}"
    );
    assert!(text.contains("dehors.png : hors de la trace"), "{text}");
    // La photothèque lit la position du XMP.
    let gps = luma_library::entry_of(&inside).gps.unwrap();
    assert!(
        (gps[0] - 48.8568).abs() < 1e-6 && (gps[1] - 2.34375).abs() < 1e-6,
        "{gps:?}"
    );
    assert!(luma_library::entry_of(&outside).gps.is_none());
    // Une seconde fois : déjà placée, gardée.
    let (_, text) = luma(&["geotag", &dir_s, "--gpx", &gpx_s]);
    assert!(text.contains("0 photos placées, 1 déjà placées"), "{text}");
    // Lue comme l'heure d'un boîtier réglé sur Paris (+02:00), la photo de 08:20 a été
    // prise à 06:20 en heure universelle : avant la trace.
    let (_, text) = luma(&[
        "geotag",
        inside.to_str().unwrap(),
        "--gpx",
        &gpx_s,
        "--offset",
        "+02:00",
        "--replace",
    ]);
    assert!(text.contains("hors de la trace"), "{text}");

    // L'écart deviné : la photo de 08:20, lue comme l'heure de Paris, n'est pas dans la
    // trace ; à l'heure universelle, si.
    let (ok, text) = luma(&[
        "geotag",
        inside.to_str().unwrap(),
        "--gpx",
        &gpx_s,
        "--offset",
        "auto",
        "--replace",
    ]);
    assert!(
        ok && text.contains("horloge du boîtier : UTC+00:00 (devinée)"),
        "{text}"
    );
    assert!(text.contains("1 photos placées"), "{text}");

    for (args, says) in [
        (vec!["geotag", &dir_s], "--gpx"),
        (vec!["geotag", "--gpx", &gpx_s], "aucune photo"),
        (
            vec!["geotag", &dir_s, "--gpx", &gpx_s, "--offset", "midi"],
            "n'est pas un écart",
        ),
    ] {
        let (ok, text) = luma(&args);
        assert!(!ok && text.contains(says), "{args:?} : {text}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
