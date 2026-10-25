// Diagnostic : balance du boîtier et balance automatique de chaque photo.
fn main() {
    for path in std::env::args().skip(1) {
        let photo = luma_render::Photo::open(&path).unwrap();
        let (s, a) = (photo.as_shot(), photo.auto_white_balance());
        println!(
            "{path:<60} boîtier {:5.0} K {:+4.0}   auto {:5.0} K {:+4.0}",
            s.temperature, s.tint, a.temperature, a.tint
        );
    }
}
