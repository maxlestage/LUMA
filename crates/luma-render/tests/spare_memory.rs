//! La mémoire des bases chassées du cache, reprise par l'étape d'entrée suivante : des
//! rendus à la suite (chaque Exposition une nouvelle base, les plus anciennes chassées)
//! donnent exactement les pixels de rendus faits chacun sur une photo neuve.

use luma_core::Cancel;
use luma_render::{Photo, Size};

#[test]
fn renders_in_a_row_give_the_same_bits_as_fresh_ones() {
    let dir = std::env::temp_dir().join(format!("luma-memoire-reprise-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("scene.png");
    image::RgbImage::from_fn(1200, 800, |x, y| {
        let v = (((x / 3) ^ (y / 5)) % 7) as u8 * 30 + ((x * y) % 17) as u8;
        image::Rgb([v, v / 2 + 60, 255 - v])
    })
    .save(&src)
    .unwrap();
    let photo = Photo::open(&src).unwrap();
    let base = photo.default_settings();
    let size = Size::Fit(900);
    let bits = |img: &luma_core::RgbImage| img.data.iter().flat_map(|p| p.map(f32::to_bits)).collect::<Vec<_>>();
    // Neuf expositions : au‑delà des bases gardées, les plus anciennes sont chassées et
    // leur mémoire reprise.
    for i in 0..9 {
        let mut s = base.clone();
        s.exposure = -1.0 + 0.25 * i as f32;
        s.detail.color_nr = 25.0 + i as f32;
        let in_a_row = photo.render(&s, size, &Cancel::new()).unwrap();
        let fresh = Photo::open(&src).unwrap().render(&s, size, &Cancel::new()).unwrap();
        assert_eq!(bits(&in_a_row.image), bits(&fresh.image), "rendu {i}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
