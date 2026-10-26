//! Le doublon de travail : en pleine définition, identique à l'original au 1/255 près,
//! plus rapide à ouvrir, et bien plus léger en mémoire pendant la retouche.

use std::path::{Path, PathBuf};
use std::time::Instant;

use luma_color::OutputSpace;
use luma_core::Cancel;
use luma_project::Settings;
use luma_raw::{Cfa, Exif, Mosaic, Orientation, RawPhoto};
use luma_render::duplicate::{self, Duplicate};
use luma_render::{Photo, Size};

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("luma-dup-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Un RAW Bayer synthétique : dégradés, couleurs et détails fins, tourné à 90°.
fn bayer(w: usize, h: usize) -> RawPhoto {
    let cfa = Cfa::parse("RGGB", 2, 2).unwrap();
    let mut data = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
            let rgb = [
                0.02 + 0.9 * u * u,
                0.05 + 0.6 * (v * 9.0).sin().abs(),
                0.03 + 0.5 * ((x / 7 + y / 5) % 2) as f32 * v,
            ];
            data[y * w + x] = rgb[cfa.color(x, y)];
        }
    }
    RawPhoto {
        make: "Test".into(),
        model: "Bayer".into(),
        width: w,
        height: h,
        mosaic: Mosaic::Bayer(cfa),
        data,
        as_shot_neutral: [0.5, 1.0, 0.7],
        matrices: vec![(6504.0, [[0.74, -0.24, -0.06], [-0.54, 1.32, 0.25], [-0.1, 0.18, 0.66]])],
        orientation: Orientation::Rotate90,
        baseline_exposure: 0.3,
        exif: Exif {
            iso: Some(400),
            lens: Some("50 mm".into()),
            ..Exif::default()
        },
        rendered: false,
    }
}

fn render8(photo: &Photo, size: Size) -> Vec<u8> {
    let s = Settings {
        contrast: 20.0,
        shadows: 30.0,
        ..Settings::default()
    };
    let img = photo.render(&s, size, &Cancel::new()).unwrap().image;
    luma_image::encode8(&img, OutputSpace::Srgb)
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

/// L'original sur le disque (sa taille et sa date signent le doublon) et le dossier
/// des doublons.
fn setup(name: &str) -> (PathBuf, PathBuf) {
    let dir = temp(name);
    let source = dir.join("original.ARW");
    std::fs::write(&source, b"un RAW").unwrap();
    (source, dir.join("Doublons"))
}

#[test]
fn the_duplicate_is_identical_at_full_resolution() {
    let (source, dir) = setup("identique");
    let original = Photo::new(bayer(1200, 800));
    let path = original.write_duplicate(&source, &dir).unwrap();
    let dup = Photo::open_with_duplicates(&source, &dir).unwrap();
    assert!(dup.from_duplicate());
    // Mêmes dimensions (orientées), même balance, mêmes données de prise de vue.
    assert_eq!(dup.full_size(), original.full_size());
    assert_eq!(dup.full_size(), (800, 1200));
    assert_eq!(dup.as_shot(), original.as_shot());
    assert_eq!(dup.raw().exif, original.raw().exif);
    assert_eq!(dup.raw().baseline_exposure, original.raw().baseline_exposure);
    // Pleine définition : identique au 1/255 près, après tout le développement.
    let worst = max_diff(&render8(&original, Size::Full), &render8(&dup, Size::Full));
    assert!(worst <= 1, "écart max {worst}/255");
    // La pyramide : la pleine définition, puis des réductions.
    let sizes = Duplicate::open(&source, &path).unwrap().0.sizes();
    assert_eq!(sizes[0], (1200, 800));
    assert!(sizes.len() >= 2 && sizes[1] == (600, 400), "{sizes:?}");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn the_duplicate_needs_much_less_memory() {
    let (source, dir) = setup("memoire");
    let original = Photo::new(bayer(3000, 2000));
    let _ = render8(&original, Size::Fit(600));
    original.write_duplicate(&source, &dir).unwrap();
    let dup = Photo::open_with_duplicates(&source, &dir).unwrap();
    let _ = render8(&dup, Size::Fit(600));
    let resident = |p: &Photo| p.raw().data.len() * 4 + p.cached_bytes();
    let (a, b) = (resident(&original), resident(&dup));
    eprintln!(
        "mémoire, aperçu 600 px : original {} Mo, doublon {} Mo",
        a >> 20,
        b >> 20
    );
    assert!(b * 4 < a, "original {a} o, doublon {b} o");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

/// Entre 2048 et 4096 px de grand côté, la pyramide garde un niveau à la taille exacte
/// de l'aperçu : l'aperçu ne relit pas la pleine définition.
#[test]
fn the_preview_has_its_own_level() {
    let (source, dir) = setup("apercu");
    let path = Photo::new(bayer(3000, 2000)).write_duplicate(&source, &dir).unwrap();
    let (dup, _) = Duplicate::open(&source, &path).unwrap();
    let sizes = dup.sizes();
    assert_eq!(sizes[1].0.max(sizes[1].1), 2048, "{sizes:?}");
    assert_eq!(dup.level_for(2048), 1);
    assert_eq!(sizes[2], (1024, 683), "{sizes:?}");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn a_changed_original_is_read_again() {
    let (source, dir) = setup("modifie");
    Photo::new(bayer(400, 300)).write_duplicate(&source, &dir).unwrap();
    assert!(Photo::open_with_duplicates(&source, &dir).is_ok_and(|p| p.from_duplicate()));
    // L'original change : le doublon ne correspond plus (et l'original, ici un faux
    // RAW, ne s'ouvre pas).
    std::fs::write(&source, b"un autre RAW, plus long").unwrap();
    assert!(duplicate::path_for(&source, &dir).is_some_and(|p| Duplicate::open(&source, &p).is_none()));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn old_duplicates_leave_when_the_budget_is_exceeded() {
    let dir = temp("menage");
    for (i, len) in [1000usize, 2000, 3000].iter().enumerate() {
        let p = dir.join(format!("{i}.lumadup"));
        std::fs::write(&p, vec![0u8; *len]).unwrap();
        let t = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000 + i as u64);
        std::fs::File::options()
            .append(true)
            .open(&p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }
    duplicate::trim(&dir, 5000);
    assert!(!dir.join("0.lumadup").exists(), "le plus ancien part");
    assert!(dir.join("1.lumadup").exists() && dir.join("2.lumadup").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Sur un vrai RAW (corpus de test) : ouverture plus rapide, mémoire bien moindre,
/// pleine définition identique.
#[test]
fn on_a_real_raw_the_duplicate_opens_faster() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let Some(source) = ["_DSC0009.ARW", "DSC_0750.NEF", "AFXT2720.RAF"]
        .iter()
        .map(|n| root.join(n))
        .find(|p| p.exists())
    else {
        eprintln!("pas de corpus : test ignoré");
        return;
    };
    let dir = temp("vrai");
    let t = Instant::now();
    let original = Photo::open(&source).unwrap();
    let _ = render8(&original, Size::Fit(2048));
    let raw_open = t.elapsed();
    original.write_duplicate(&source, &dir).unwrap();

    let t = Instant::now();
    let dup = Photo::open_with_duplicates(&source, &dir).unwrap();
    let _ = render8(&dup, Size::Fit(2048));
    let dup_open = t.elapsed();
    assert!(dup.from_duplicate());

    let resident = |p: &Photo| p.raw().data.len() * 4 + p.cached_bytes();
    eprintln!(
        "{} : ouverture + aperçu 2048 px : original {:?}, doublon {:?} ; mémoire {} Mo → {} Mo",
        source.display(),
        raw_open,
        dup_open,
        resident(&original) >> 20,
        resident(&dup) >> 20
    );
    assert!(dup_open < raw_open, "le doublon s'ouvre plus vite");
    assert!(resident(&dup) * 2 < resident(&original));
    let worst = max_diff(&render8(&original, Size::Full), &render8(&dup, Size::Full));
    assert!(worst <= 1, "pleine définition : écart max {worst}/255");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_whole_cache_can_be_emptied() {
    let (source, dir) = setup("vider");
    let photo = Photo::new(bayer(600, 400));
    let _ = render8(&photo, Size::Fit(300));
    assert!(photo.cached_bytes() > 0);
    photo.release_caches();
    assert_eq!(photo.cached_bytes(), 0);
    photo.write_duplicate(&source, &dir).unwrap();
    let used = duplicate::usage(&dir);
    assert!(used > 0);
    assert_eq!(duplicate::clear(&dir), used);
    assert_eq!(duplicate::usage(&dir), 0);
    assert!(!Photo::open_with_duplicates(&source, &dir).is_ok_and(|p| p.from_duplicate()));
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}
