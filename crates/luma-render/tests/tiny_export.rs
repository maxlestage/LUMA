//! L'export des plus petites images (un pixel, une ligne) avec toutes ses options :
//! agrandissement, accentuation de sortie, filigrane, poids maximal, quatre formats.

use luma_render::Photo;
use luma_render::export::{self, Format, Medium, Options, OutputSharpen, Resize, Strength, Watermark};

#[test]
fn the_tiniest_photos_export_with_every_option() {
    let dir = std::env::temp_dir().join(format!("luma-minuscule-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (w, h) in [(1u32, 1u32), (1, 5), (5, 1), (2, 3)] {
        let src = dir.join(format!("source-{w}x{h}.png"));
        image::RgbImage::from_fn(w, h, |x, y| image::Rgb([40 + 50 * x as u8, 90, 30 + 40 * y as u8]))
            .save(&src)
            .unwrap();
        let photo = Photo::open(&src).unwrap();
        let settings = photo.default_settings();
        for format in [Format::Jpeg, Format::Png, Format::Tiff, Format::WebP] {
            for resize in [None, Some(Resize::LongEdge(9)), Some(Resize::Megapixels(0.0001))] {
                let options = Options {
                    format,
                    resize,
                    enlarge: true,
                    sharpen: Some(OutputSharpen {
                        medium: Medium::Glossy,
                        strength: Strength::High,
                    }),
                    watermark: Some(Watermark::new("© Maxime Lestage")),
                    max_kb: Some(1),
                    ..Options::default()
                };
                let out = dir.join(format!("sortie-{w}x{h}.{}", format.extension()));
                export::develop(&photo, &settings, &out, &options, &Default::default())
                    .unwrap_or_else(|e| panic!("{w} × {h} {format:?} {resize:?} : {e}"));
                let back = image::open(&out).unwrap_or_else(|e| panic!("{w} × {h} {format:?} : {e}"));
                assert!(back.width() >= 1 && back.height() >= 1);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
