//! Mesure des briques de base sur un plan de 2048 × 1365.
use luma_core::{Plane, RgbImage};
use rayon::prelude::*;
use std::time::Instant;

fn time<T>(name: &str, f: impl Fn() -> T) {
    let _ = f();
    let t = Instant::now();
    for _ in 0..5 {
        std::hint::black_box(f());
    }
    println!("{name:<28} {:>7.2} ms", t.elapsed().as_secs_f64() * 1000.0 / 5.0);
}

fn main() {
    let (w, h) = (2048, 1365);
    let p = Plane {
        width: w,
        height: h,
        data: (0..w * h).map(|i| ((i * 7919) % 1000) as f32 / 1000.0).collect(),
    };
    let img = RgbImage::from_fn(w, h, |x, y| [x as f32 / w as f32, y as f32 / h as f32, 0.3]);
    time("map sqrt (plan)", || p.map(|v| v.sqrt()));
    time("blur 1", || p.blur(1.0));
    time("blur 3", || p.blur(3.0));
    time("blur 20", || p.blur(20.0));
    time("guided 0.5", || p.guided(0.5, 0.01));
    time("guided 40", || p.guided(40.0, 0.01));
    time("rgb sqrt collect", || {
        img.data.par_iter().map(|q| q.map(|v| v.sqrt())).collect::<Vec<_>>()
    });
    time("rgb clone", || img.clone());
    let ok = luma_color::OkLab::for_working();
    time("oklab aller-retour", || {
        img.data
            .par_iter()
            .map(|&q| ok.to_rgb(ok.from_rgb(q)))
            .collect::<Vec<_>>()
    });
}
