// Planche‑contact des préréglages : `cargo run -p luma-render --example presets -- photo.RAF planche.png`.
use luma_color::OutputSpace;
use luma_core::Cancel;
use luma_project::Settings;
use luma_project::presets::PRESETS;
use luma_render::{Photo, Size};

fn main() {
    let mut args = std::env::args().skip(1);
    let (path, out) = (
        args.next().expect("un RAW"),
        args.next().unwrap_or("planche.png".into()),
    );
    let photo = Photo::open(&path).unwrap();
    let base = Settings::default();
    let mut tiles = vec![("Sans préréglage".to_string(), base.clone())];
    tiles.extend(PRESETS.iter().map(|p| (p.name.to_string(), p.applied(&base, 1.0))));
    let images: Vec<_> = tiles
        .iter()
        .map(|(_, s)| {
            let r = photo.render(s, Size::Fit(420), &Cancel::new()).unwrap().image;
            (r.width, r.height, luma_image::encode8(&r, OutputSpace::Srgb))
        })
        .collect();
    let (tw, th) = (images[0].0, images[0].1);
    let cols = 5;
    let rows = images.len().div_ceil(cols);
    let mut sheet = image::RgbImage::new((tw * cols) as u32, (th * rows) as u32);
    for (k, (w, h, px)) in images.iter().enumerate() {
        let (ox, oy) = ((k % cols) * tw, (k / cols) * th);
        for y in 0..*h.min(&th) {
            for x in 0..*w.min(&tw) {
                let i = (y * w + x) * 3;
                sheet.put_pixel(
                    (ox + x) as u32,
                    (oy + y) as u32,
                    image::Rgb([px[i], px[i + 1], px[i + 2]]),
                );
            }
        }
    }
    sheet.save(&out).unwrap();
    for (k, (name, _)) in tiles.iter().enumerate() {
        println!("{:>2} ({}, {}) {name}", k + 1, k / cols + 1, k % cols + 1);
    }
}
