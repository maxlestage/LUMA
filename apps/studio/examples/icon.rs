//! `cargo run -p luma-studio --example icon -- icon.png [mac|mac-petit|ios] [taille]` :
//! l'icône de l'app, le logo de LÜMA (voir `luma-brand`). `mac` : le carré arrondi de
//! macOS avec « LÜMA » ; `mac-petit` : sans le nom, pour 16 et 32 px ; `ios` : tout le
//! carré (iPhone et iPad l'arrondissent) ; `transparent` et `transparent-seul` : le
//! logo sans fond, avec ou sans le nom (pour les documents, la presse) ; `windows` :
//! un fichier `.ico` de 16 à 256 px (l'anneau seul jusqu'à 32 px), pour l'exécutable,
//! les raccourcis et l'installateur de Windows.

/// Un `.ico` : chaque taille y est un PNG (lu par Windows depuis Vista).
fn windows_icon(out: &str) {
    let sizes = [16u32, 24, 32, 48, 64, 128, 256];
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&n| {
            let rgba = luma_brand::logo(n as usize, luma_brand::Backdrop::MacIcon, n > 32);
            let mut png = Vec::new();
            image::RgbaImage::from_raw(n, n, rgba)
                .expect("taille")
                .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                .expect("PNG");
            png
        })
        .collect();
    let mut ico = vec![0, 0, 1, 0];
    ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len();
    for (&n, png) in sizes.iter().zip(&images) {
        // 256 px s'écrit 0 ; pas de palette, un plan, 32 bits par pixel.
        let side = if n >= 256 { 0 } else { n as u8 };
        ico.extend_from_slice(&[side, side, 0, 0, 1, 0, 32, 0]);
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for png in &images {
        ico.extend_from_slice(png);
    }
    std::fs::write(out, ico).expect("écriture de l'icône");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "icon.png".into());
    let kind = args.next().unwrap_or_else(|| "mac".into());
    if kind == "windows" {
        return windows_icon(&out);
    }
    let size: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(1024);
    let (backdrop, wordmark) = match kind.as_str() {
        "mac-petit" => (luma_brand::Backdrop::MacIcon, false),
        "ios" => (luma_brand::Backdrop::Full, true),
        "transparent" => (luma_brand::Backdrop::None, true),
        "transparent-seul" => (luma_brand::Backdrop::None, false),
        _ => (luma_brand::Backdrop::MacIcon, true),
    };
    let rgba = luma_brand::logo(size, backdrop, wordmark);
    image::RgbaImage::from_raw(size as u32, size as u32, rgba)
        .expect("taille")
        .save(&out)
        .expect("écriture de l'icône");
}
