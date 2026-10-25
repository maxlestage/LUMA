// `cargo run --release -p luma-gpu --example measure -- photo.ARW …` : les chiffres de
// la section Performance du site, mesurés sur de vrais RAW, comme l'app les vit :
//
// - ouverture : décodage du RAW jusqu'au premier aperçu 2048 px affiché (cache froid) ;
// - aperçu après réglage : un curseur bouge, la base est en cache, finition sur le GPU ;
// - vignette : un RAW jusqu'à sa vignette 320 px (la bibliothèque) ;
// - mémoire : la mémoire du processus, photo ouverte et aperçu affiché.
//
// Une ligne par photo, puis les médianes ; la dernière ligne (`site:`) se recopie dans
// `site/src/content.rs` (PERF).
use std::time::Instant;

use luma_color::OutputSpace;
use luma_core::{Cancel, Timings};
use luma_render::{Photo, Size};

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// La mémoire résidente du processus, en Mo.
fn rss_mb() -> f64 {
    let pid = std::process::id().to_string();
    std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
        .map_or(f64::NAN, |kb| kb / 1024.0)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    assert!(!paths.is_empty(), "des RAW");
    let gpu = luma_gpu::Gpu::any().expect("un GPU");
    println!("GPU : {}", gpu.name());
    let (mut open, mut adjust, mut thumb, mut memory) = (vec![], vec![], vec![], vec![]);
    for path in &paths {
        // Vignette : un RAW jusqu'à 320 px, comme la bibliothèque.
        let t = Instant::now();
        let photo = Photo::open(path).unwrap();
        let s = photo.default_settings();
        let _ = photo.render(&s, Size::Fit(320), &Cancel::new()).unwrap();
        let thumbnail = ms(t);
        drop(photo);

        // Ouverture : décodage, base 2048 px, finition GPU, encodage pour l'écran.
        let t = Instant::now();
        let photo = Photo::open(path).unwrap();
        let mut s = photo.default_settings();
        let base = photo
            .base(&s, Size::Fit(2048), &Cancel::new(), &mut Timings::default())
            .unwrap();
        let _ = gpu.finish(&base.image, base.id, base.scale, &s, OutputSpace::DisplayP3);
        let opened = ms(t);

        // Un curseur qui bouge : contraste, vibrance (base en cache).
        let mut runs = Vec::new();
        for i in 0..30 {
            s.contrast = i as f32 * 2.0;
            s.vibrance = i as f32 * 3.0;
            let t = Instant::now();
            let base = photo
                .base(&s, Size::Fit(2048), &Cancel::new(), &mut Timings::default())
                .unwrap();
            let _ = gpu.finish(&base.image, base.id, base.scale, &s, OutputSpace::DisplayP3);
            runs.push(ms(t));
        }
        let adjusted = median(runs);
        let mem = rss_mb();
        println!(
            "{path} : ouverture {opened:.0} ms · réglage P50 {adjusted:.1} ms · vignette {thumbnail:.0} ms · mémoire {mem:.0} Mo"
        );
        open.push(opened);
        adjust.push(adjusted);
        thumb.push(thumbnail);
        memory.push(mem);
        drop(photo);
    }
    let (o, a, t, m) = (median(open), median(adjust), median(thumb), median(memory));
    println!("médianes : ouverture {o:.0} ms · réglage {a:.1} ms · vignette {t:.0} ms · mémoire {m:.0} Mo");
    println!("site: ouverture={o:.0} reglage={a:.1} vignette={t:.0} memoire={m:.0}");
}
