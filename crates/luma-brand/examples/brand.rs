//! `cargo run -p luma-brand --example brand -- <dossier>` : écrit le logo et le fond en
//! PNG et en SVG, pour les regarder (et pour le site).

fn save(dir: &std::path::Path, name: &str, w: usize, h: usize, rgba: Vec<u8>) {
    image::RgbaImage::from_raw(w as u32, h as u32, rgba)
        .expect("taille")
        .save(dir.join(name))
        .expect("écriture");
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir).expect("dossier");
    use luma_brand::Backdrop;
    save(
        &dir,
        "logo-mac.png",
        1024,
        1024,
        luma_brand::logo(1024, Backdrop::MacIcon, true),
    );
    save(
        &dir,
        "logo-plein.png",
        1024,
        1024,
        luma_brand::logo(1024, Backdrop::Full, true),
    );
    save(
        &dir,
        "anneau.png",
        256,
        256,
        luma_brand::logo(256, Backdrop::None, false),
    );
    save(&dir, "fond.png", 1440, 900, luma_brand::backdrop(1440, 900));
    // Les fichiers du site (`… --example brand -- site/public/assets site`).
    if std::env::args().nth(2).as_deref() == Some("site") {
        // Les SVG portent le rendu exact du logo (une image PNG intégrée) : le même
        // anneau que dans l'app, sans les marches d'un dégradé en arcs.
        std::fs::write(
            dir.join("favicon.svg"),
            svg_png(&luma_brand::logo(96, Backdrop::MacIcon, false), 96, false),
        )
        .expect("svg");
        std::fs::write(
            dir.join("marque.svg"),
            svg_png(&luma_brand::logo(72, Backdrop::None, false), 72, true),
        )
        .expect("svg");
        save(
            &dir,
            "apple-touch-icon.png",
            180,
            180,
            luma_brand::logo(180, Backdrop::Full, false),
        );
        save(
            &dir,
            "icon-192.png",
            192,
            192,
            luma_brand::logo(192, Backdrop::Full, true),
        );
        save(
            &dir,
            "icon-512.png",
            512,
            512,
            luma_brand::logo(512, Backdrop::Full, true),
        );
        save(&dir, "icon-maskable-512.png", 512, 512, maskable());
        save(&dir, "og-image.png", 1200, 630, og_image());
        for f in ["logo-mac.png", "logo-plein.png", "anneau.png", "fond.png"] {
            let _ = std::fs::remove_file(dir.join(f));
        }
    }
}

/// L'icône « masquable » : le logo dans la zone sûre centrale (80 %).
fn maskable() -> Vec<u8> {
    let n = 512;
    let inner = 410;
    let logo = luma_brand::logo(inner, luma_brand::Backdrop::Full, true);
    // Fond noir plein, puis le logo au centre.
    let mut out = [1u8, 1, 2, 255].repeat(n * n);
    let o = (n - inner) / 2;
    for y in 0..inner {
        for x in 0..inner {
            let (s, d) = ((y * inner + x) * 4, ((y + o) * n + x + o) * 4);
            out[d..d + 4].copy_from_slice(&logo[s..s + 4]);
        }
    }
    out
}

/// L'image des partages (1200 × 630) : le fond de LÜMA et le logo au centre.
fn og_image() -> Vec<u8> {
    let (w, h) = (1200, 630);
    let mut out = luma_brand::backdrop(w, h);
    let side = 520;
    let logo = luma_brand::logo(side, luma_brand::Backdrop::None, true);
    let (ox, oy) = ((w - side) / 2, (h - side) / 2);
    for y in 0..side {
        for x in 0..side {
            let s = (y * side + x) * 4;
            let d = ((y + oy) * w + x + ox) * 4;
            let a = f32::from(logo[s + 3]) / 255.0;
            for k in 0..3 {
                out[d + k] = (f32::from(out[d + k]) * (1.0 - a) + f32::from(logo[s + k]) * a).round() as u8;
            }
        }
    }
    out
}

/// Un SVG qui porte une image PNG (`rgba`, `n` × `n`), en base 64.
fn svg_png(rgba: &[u8], n: usize, hidden: bool) -> String {
    let mut png = Vec::new();
    image::RgbaImage::from_raw(n as u32, n as u32, rgba.to_vec())
        .expect("taille")
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("PNG");
    let aria = if hidden { " aria-hidden=\"true\"" } else { "" };
    format!(
        "<svg{aria} xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {n} {n}\"><image width=\"{n}\" height=\"{n}\" href=\"data:image/png;base64,{}\"/></svg>",
        base64(&png)
    )
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let v = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= c.len() {
                out.push(T[(v >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}
