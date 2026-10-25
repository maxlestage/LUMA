// Diagnostic : l'aberration chromatique latérale estimée sur chaque photo.
fn main() {
    for path in std::env::args().skip(1) {
        let photo = luma_render::Photo::open(&path).unwrap();
        let t = std::time::Instant::now();
        let (r, b) = photo.chromatic_aberration(&mut luma_core::Timings::default());
        println!(
            "{path:<60} rouge {:+.5}  bleu {:+.5}  ({:.0} ms)",
            r,
            b,
            t.elapsed().as_secs_f64() * 1000.0
        );
    }
}
