// Diagnostic : le temps de chaque étape d'un aperçu 2048 px, réglages lourds.
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let photo = luma_render::Photo::open(&path).unwrap();
    let mut s = luma_project::Settings {
        highlights: -60.0,
        shadows: 40.0,
        clarity: 25.0,
        texture: 15.0,
        dehaze: 10.0,
        vibrance: 20.0,
        ..Default::default()
    };
    s.hsl.saturation[5] = -20.0;
    s.color_grading.shadows.saturation = 20.0;
    s.detail.luminance_nr = 20.0;
    s.effects.vignette_amount = -20.0;
    let cancel = luma_core::Cancel::new();
    let mut total = std::collections::BTreeMap::<&str, f64>::new();
    let runs = 8;
    for i in 0..runs + 2 {
        s.exposure = i as f32 * 0.05;
        let r = photo.render(&s, luma_render::Size::Fit(2048), &cancel).unwrap();
        if i >= 2 {
            for (k, d) in &r.timings.steps {
                *total.entry(k).or_default() += d.as_secs_f64() * 1000.0 / runs as f64;
            }
        }
    }
    for (k, v) in total {
        println!("{k:<20} {v:6.1} ms");
    }
}
