// Diagnostic : répartition de la luminance de scène (IL autour du gris moyen) après l'étape d'entrée.
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let photo = luma_render::Photo::open(&path).unwrap();
    let mut t = luma_core::Timings::default();
    let src = photo.source(luma_render::Size::Fit(700), &mut t);
    let neutral = photo.raw().as_shot_neutral;
    let m = photo.raw().camera_to_working(neutral);
    let gain = photo.raw().baseline_exposure.exp2();
    let mut stops: Vec<f32> = src
        .data
        .iter()
        .map(|&p| (luma_color::luminance(luma_color::apply(&m, p)) * gain).max(1e-6))
        .map(|y| (y / 0.18).log2())
        .collect();
    stops.sort_by(f32::total_cmp);
    for q in [0.01, 0.1, 0.25, 0.5, 0.75, 0.9, 0.99, 0.999] {
        println!(
            "centile {:>5.1} : {:+.2} IL",
            q * 100.0,
            stops[((stops.len() - 1) as f32 * q) as usize]
        );
    }
}
