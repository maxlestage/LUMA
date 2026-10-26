// `cargo run --release -p luma-gpu --example measure -- photo.ARW …` : les chiffres de
// la section Performance du site, mesurés sur de vrais RAW, comme l'app les vit :
//
// - vignette : un RAW jamais ouvert jusqu'à sa vignette 320 px instantanée (l'aperçu
//   intégré, décodé à taille réduite), puis jusqu'à son vrai rendu (la bibliothèque) ;
// - ouverture : un RAW jusqu'à son premier aperçu 2048 px (première fois), puis depuis
//   son doublon de travail (les fois suivantes) ;
// - aperçu après réglage : un curseur bouge, finition sur le GPU ;
// - mémoire : ce que la photo retouchée occupe (pixels gardés et caches), depuis le RAW
//   puis depuis le doublon.
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

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Ce que la photo occupe : les pixels gardés (le RAW, s'il est là) et ses caches.
fn memory_mb(p: &Photo) -> f64 {
    (p.raw().data.len() * 4 + p.cached_bytes()) as f64 / 1024.0 / 1024.0
}

/// Le premier aperçu 2048 px, comme à l'ouverture dans l'app.
fn first_preview(photo: &Photo, gpu: &luma_gpu::Gpu) {
    let s = photo.default_settings();
    let base = photo
        .base(&s, Size::Fit(2048), &Cancel::new(), &mut Timings::default())
        .unwrap();
    let _ = gpu.finish(&base.image, base.id, base.scale, &s, OutputSpace::DisplayP3);
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    assert!(!paths.is_empty(), "des RAW");
    let gpu = luma_gpu::Gpu::any().expect("un GPU");
    println!("GPU : {}", gpu.name());
    let dir = std::env::temp_dir().join(format!("luma-measure-{}", std::process::id()));
    let mut rows: Vec<[f64; 7]> = Vec::new();
    for path in &paths {
        let t = Instant::now();
        let quick = luma_raw::RawPhoto::embedded_preview(std::path::Path::new(path), 320);
        let thumbnail = ms(t);
        let t = Instant::now();
        let photo = Photo::open(path).unwrap();
        let s = photo.default_settings();
        let _ = photo.render(&s, Size::Fit(320), &Cancel::new()).unwrap();
        let rendered = ms(t);
        // Sans aperçu intégré, la vignette est le vrai rendu.
        let thumbnail = if quick.is_some() { thumbnail } else { rendered };
        drop(photo);

        let t = Instant::now();
        let photo = Photo::open(path).unwrap();
        first_preview(&photo, &gpu);
        let open_raw = ms(t);
        let mem_raw = memory_mb(&photo);
        photo.write_duplicate(std::path::Path::new(path), &dir).unwrap();
        drop(photo);

        let t = Instant::now();
        let photo = Photo::open_with_duplicates(path, &dir).unwrap();
        assert!(photo.from_duplicate());
        first_preview(&photo, &gpu);
        let open_dup = ms(t);

        let mut s = photo.default_settings();
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
        let adjust = median(runs);
        let mem_dup = memory_mb(&photo);
        // À 3072 px (écran Retina, iPad Pro), l'accentuation s'applique : le GPU la fait
        // maintenant ; avant, toute la finition repassait au processeur.
        let big = photo
            .base(&s, Size::Fit(3072), &Cancel::new(), &mut Timings::default())
            .unwrap();
        let (mut on_gpu, mut on_cpu) = (Vec::new(), Vec::new());
        for i in 0..10 {
            s.contrast = i as f32 * 2.0;
            let t = Instant::now();
            let _ = gpu.finish(&big.image, big.id, big.scale, &s, OutputSpace::DisplayP3);
            on_gpu.push(ms(t));
            let t = Instant::now();
            let mut img = (*big.image).clone();
            luma_image::finish(&mut img, big.scale, &s, &Cancel::new(), &mut Timings::default()).unwrap();
            let _ = luma_image::encode8(&img, OutputSpace::DisplayP3);
            on_cpu.push(ms(t));
        }
        let (retina, retina_cpu) = (median(on_gpu), median(on_cpu));
        println!(
            "{path} : réglage à 3072 px avec accentuation (×{:.2}) : {retina:.1} ms sur le GPU, {retina_cpu:.0} ms sur le processeur",
            big.scale
        );
        println!(
            "{path} : vignette {thumbnail:.0} ms (rendu {rendered:.0} ms) · ouverture {open_raw:.0} ms (RAW), {open_dup:.0} ms (doublon) · \
             réglage P50 {adjust:.1} ms · mémoire {mem_raw:.0} Mo (RAW), {mem_dup:.0} Mo (doublon)"
        );
        rows.push([thumbnail, open_raw, open_dup, adjust, mem_raw, mem_dup, rendered]);
    }
    let _ = std::fs::remove_dir_all(&dir);
    let m: [f64; 7] = std::array::from_fn(|k| median(rows.iter().map(|r| r[k]).collect()));
    println!(
        "médianes : vignette {:.0} ms (rendu {:.0} ms) · ouverture {:.0} ms (RAW), {:.0} ms (doublon) · réglage {:.1} ms · mémoire {:.0} Mo (RAW), {:.0} Mo (doublon)",
        m[0], m[6], m[1], m[2], m[3], m[4], m[5]
    );
    println!(
        "site: vignette={:.0} vignette_rendu={:.0} ouverture_raw={:.0} ouverture={:.0} reglage={:.1} memoire_raw={:.0} memoire={:.0}",
        m[0], m[6], m[1], m[2], m[3], m[4], m[5]
    );
}
