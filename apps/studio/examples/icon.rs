//! `cargo run -p luma-studio --example icon -- icon.png` : dessine l'icône de l'app
//! (1024 × 1024) : un carré arrondi sombre, et un anneau de lumière dont la teinte fait
//! le tour du spectre, comme les roues de l'étalonnage des couleurs.

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "icon.png".into());
    let n = 1024u32;
    let img = image::RgbaImage::from_fn(n, n, |x, y| {
        // Suréchantillonnage 4×4 pour des bords nets.
        let mut acc = [0.0f32; 4];
        for sy in 0..4 {
            for sx in 0..4 {
                let px = (x as f32 + (sx as f32 + 0.5) / 4.0) / n as f32 - 0.5;
                let py = (y as f32 + (sy as f32 + 0.5) / 4.0) / n as f32 - 0.5;
                let c = sample(px, py);
                for k in 0..4 {
                    acc[k] += c[k] / 16.0;
                }
            }
        }
        let a = acc[3];
        let un = |v: f32| {
            if a > 0.0 {
                (v / a).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };
        image::Rgba([
            (un(acc[0]) * 255.0) as u8,
            (un(acc[1]) * 255.0) as u8,
            (un(acc[2]) * 255.0) as u8,
            (a * 255.0) as u8,
        ])
    });
    img.save(&out).expect("écriture de l'icône");
}

/// Couleur prémultipliée au point (x, y) ∈ [-0,5 ; 0,5]².
fn sample(x: f32, y: f32) -> [f32; 4] {
    // Le carré arrondi de macOS : 824 px sur 1024, rayon ≈ 22 %.
    let half = 0.4023;
    let r = 0.18;
    let qx = (x.abs() - (half - r)).max(0.0);
    let qy = (y.abs() - (half - r)).max(0.0);
    if (qx * qx + qy * qy).sqrt() > r {
        return [0.0; 4];
    }
    // Fond : gris très sombre, légèrement plus clair en haut.
    let g = 0.10 - y * 0.06;
    let mut c = [g, g, g * 1.04];
    let d = (x * x + y * y).sqrt();
    // Anneau aux couleurs du spectre.
    let ring = 0.24;
    let width = 0.055;
    let t = ((d - ring).abs() / width).min(1.0);
    let ring_a = (1.0 - t * t).max(0.0);
    if ring_a > 0.0 {
        let hue = (y.atan2(x) / std::f32::consts::TAU + 1.0).fract() * 6.0;
        let rgb = hsv(hue, 0.65, 1.0);
        for k in 0..3 {
            c[k] = c[k] * (1.0 - ring_a) + rgb[k] * ring_a;
        }
    }
    // Au centre : une lueur blanche, la lumière (« lūmen »).
    let glow = (-(d / 0.11).powi(2)).exp();
    for v in &mut c {
        *v += glow * 0.95;
    }
    [c[0].min(1.0), c[1].min(1.0), c[2].min(1.0), 1.0]
}

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let i = h.floor() as i32 % 6;
    let f = h - h.floor();
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}
