// `cargo run --release -p luma-gpu --example bench -- photo.ARW` : le temps d'un aperçu
// 2048 px quand on règle une couleur (la base est en cache), sur le processeur et sur
// le GPU.
use std::time::Instant;

use luma_color::OutputSpace;
use luma_core::{Cancel, Timings};
use luma_project::Settings;
use luma_render::{Photo, Size};

fn main() {
    let path = std::env::args().nth(1).expect("un RAW");
    let photo = Photo::open(&path).unwrap();
    let gpu = luma_gpu::Gpu::any();
    let mut s = Settings {
        highlights: -60.0,
        shadows: 40.0,
        clarity: 25.0,
        ..Default::default()
    };
    s.color_grading.shadows.saturation = 20.0;
    s.effects.vignette_amount = -20.0;
    let base = photo
        .base(&s, Size::Fit(2048), &Cancel::new(), &mut Timings::default())
        .unwrap();
    println!("{path} — base {} × {}", base.image.width, base.image.height);
    let runs = 20;
    let mut cpu = Vec::new();
    let mut on_gpu = Vec::new();
    for i in 0..runs {
        s.vibrance = i as f32 * 4.0;
        s.contrast = i as f32 * 2.0;
        let t = Instant::now();
        let mut img = (*base.image).clone();
        luma_image::finish(&mut img, base.scale, &s, &Cancel::new(), &mut Timings::default()).unwrap();
        let _ = luma_image::encode8(&img, OutputSpace::Srgb);
        cpu.push(t.elapsed().as_secs_f64() * 1000.0);
        if let Some(g) = &gpu {
            let t = Instant::now();
            let _ = g.finish(&base.image, base.id, base.scale, &s, OutputSpace::Srgb);
            on_gpu.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    let p50 = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    println!("processeur  P50 {:6.1} ms (finition + encodage)", p50(&mut cpu));
    match &gpu {
        Some(g) if !on_gpu.is_empty() => println!("GPU         P50 {:6.1} ms ({})", p50(&mut on_gpu), g.name()),
        _ => println!("GPU         absent"),
    }
}
