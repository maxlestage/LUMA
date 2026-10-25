//! `cargo run -p luma-studio --example icon -- icon.png [mac|mac-petit|ios] [taille]` :
//! l'icône de l'app, le logo de LÜMA (voir `luma-brand`). `mac` : le carré arrondi de
//! macOS avec « LÜMA » ; `mac-petit` : sans le nom, pour 16 et 32 px ; `ios` : tout le
//! carré (iPhone et iPad l'arrondissent).

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "icon.png".into());
    let kind = args.next().unwrap_or_else(|| "mac".into());
    let size: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(1024);
    let (backdrop, wordmark) = match kind.as_str() {
        "mac-petit" => (luma_brand::Backdrop::MacIcon, false),
        "ios" => (luma_brand::Backdrop::Full, true),
        _ => (luma_brand::Backdrop::MacIcon, true),
    };
    let rgba = luma_brand::logo(size, backdrop, wordmark);
    image::RgbaImage::from_raw(size as u32, size as u32, rgba)
        .expect("taille")
        .save(&out)
        .expect("écriture de l'icône");
}
