//! `cargo run --release -p luma-raw --example inspect -- fichiers…` : ce que LÜMA lit
//! dans chaque RAW, et le temps de chaque étape.
use std::time::Instant;

fn main() {
    for path in std::env::args().skip(1) {
        let t = Instant::now();
        let photo = match luma_raw::RawPhoto::open(&path) {
            Ok(p) => p,
            Err(e) => {
                println!("{path}: {e}");
                continue;
            }
        };
        let open = t.elapsed();
        let t = Instant::now();
        let small = photo.binned();
        let binned = t.elapsed();
        let t = Instant::now();
        let full = photo.demosaic();
        let demosaic = t.elapsed();
        let wb = photo.as_shot();
        println!(
            "{} {} {}×{} {:?} | {:.0} K {:+.0} | base {:+.2} IL | ISO {:?} | ouvrir {:.0} ms, réduit {}×{} {:.0} ms, dématriçage {:.0} ms ({}×{})",
            photo.make,
            photo.model,
            photo.width,
            photo.height,
            match photo.mosaic {
                luma_raw::Mosaic::Bayer(_) => "Bayer",
                luma_raw::Mosaic::XTrans(_) => "X-Trans",
                luma_raw::Mosaic::Linear => "linéaire",
            },
            wb.temperature,
            wb.tint,
            photo.baseline_exposure,
            photo.exif.iso,
            open.as_secs_f64() * 1e3,
            small.width,
            small.height,
            binned.as_secs_f64() * 1e3,
            demosaic.as_secs_f64() * 1e3,
            full.width,
            full.height
        );
    }
}
