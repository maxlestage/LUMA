//! Les images déjà développées (JPEG, PNG…) : ouvertes sans réglage, elles ressortent
//! telles quelles ; les réglages agissent ensuite comme sur un RAW.

use luma_core::Cancel;
use luma_render::{Photo, Size};

/// Un dégradé de gris et des couleurs vives (dont du rouge, du jaune et du cyan purs,
/// tout près du blanc).
fn test_png(path: &std::path::Path) -> image::RgbImage {
    let img = image::RgbImage::from_fn(64, 48, |x, y| {
        let v = (x * 4) as u8;
        match y / 8 {
            0 => image::Rgb([v, v, v]),
            1 => image::Rgb([255, v, 0]),
            2 => image::Rgb([v, 255, 255]),
            3 => image::Rgb([255, 255, v]),
            4 => image::Rgb([(x * 3) as u8, 90, 200 - x as u8 * 2]),
            _ => image::Rgb([200, 150 - x as u8, 120]),
        }
    });
    img.save(path).unwrap();
    img
}

fn render8(photo: &Photo, s: &luma_project::Settings) -> Vec<u8> {
    let img = photo.render(s, Size::Full, &Cancel::new()).unwrap().image;
    luma_image::encode8(&img, luma_color::OutputSpace::Srgb)
}

#[test]
fn a_png_opened_without_adjustments_comes_back_unchanged() {
    let path = std::env::temp_dir().join(format!("luma-rendered-{}.png", std::process::id()));
    let original = test_png(&path);
    let photo = Photo::open(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(photo.raw().rendered);
    let s = photo.default_settings();
    assert_eq!(s.profile, luma_project::Profile::Original);
    let out = render8(&photo, &s);
    let worst = out
        .iter()
        .zip(original.as_raw())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    assert!(worst <= 1, "écart max {worst}");

    // L'exposition éclaircit, le contraste écarte les tons du gris moyen.
    let brighter = render8(
        &photo,
        &luma_project::Settings {
            exposure: 1.0,
            ..s.clone()
        },
    );
    assert!(
        brighter[3 * 16] > out[3 * 16] + 20,
        "{} → {}",
        out[3 * 16],
        brighter[3 * 16]
    );
    let contrast = render8(
        &photo,
        &luma_project::Settings {
            contrast: 60.0,
            ..s.clone()
        },
    );
    assert!(contrast[3 * 4] < out[3 * 4], "les ombres descendent");
    assert!(contrast[3 * 60] > out[3 * 60], "les hautes lumières montent");
}

/// En pleine définition, la base n'est gardée nulle part : le rendu se fait dans son
/// image même. Les mêmes pixels qu'un rendu de même taille dont la base est gardée (et
/// rendu dans une copie), réglages lourds et accentuation compris.
#[test]
fn rendering_in_the_base_itself_gives_the_same_pixels() {
    let dir = std::env::temp_dir().join(format!("luma-rendu-sur-place-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("essai.png");
    test_png(&path);
    let photo = Photo::open(&path).unwrap();
    let mut s = photo.default_settings();
    s.exposure = 0.4;
    s.contrast = 30.0;
    s.vibrance = 20.0;
    s.detail.sharpen_amount = 60.0;
    s.effects.vignette_amount = -25.0;
    let full = photo.render(&s, Size::Full, &Cancel::new()).unwrap().image;
    // Un aperçu plus grand que l'image : la même taille, sa base gardée en cache.
    let kept = photo.render(&s, Size::Fit(4096), &Cancel::new()).unwrap().image;
    assert_eq!((full.width, full.height), (kept.width, kept.height));
    assert!(full.data == kept.data);
    let _ = std::fs::remove_dir_all(&dir);
}
