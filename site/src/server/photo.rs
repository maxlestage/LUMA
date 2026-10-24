//! `/photo.svg` et `/photo-mono.svg` : un paysage au coucher du soleil généré par le
//! serveur (ciel, soleil, trois lignes de crêtes, lac et reflets), utilisé par les
//! couches du pipeline. Aucun fichier image à stocker, rendu net à toute taille.

use std::fmt::Write;

const W: f64 = 640.0;
const H: f64 = 480.0;

/// Ligne de crête : somme de sinusoïdes, stable pour une graine donnée (la même que
/// celle de `public/js/screen.js`, pour que la photo soit la même partout).
fn ridge(x: f64, seed: f64, amp: f64, freq: f64) -> f64 {
    let (mut y, mut a, mut f) = (0.0, amp, freq);
    for o in 0..5 {
        let o = f64::from(o);
        y += (x * f + seed * (o + 1.0) * 1.7).sin() * a
            + (x * f * 2.1 + seed * 3.1).sin() * a * 0.35;
        a *= 0.5;
        f *= 2.05;
    }
    y
}

/// Générateur de Park et Miller, comme `rand()` de `screen.js`.
struct Rand(u64);

impl Rand {
    fn next(&mut self) -> f64 {
        self.0 = self.0 * 16807 % 2_147_483_647;
        (self.0 as f64 - 1.0) / 2_147_483_646.0
    }
}

fn polygon(out: &mut String, fill: &str, points: impl Iterator<Item = (f64, f64)>) {
    out.push_str("<polygon fill=\"");
    out.push_str(fill);
    out.push_str("\" points=\"");
    for (x, y) in points {
        let _ = write!(out, "{x:.1},{y:.1} ");
    }
    out.push_str("\"/>");
}

/// Le paysage en SVG ; `mono` le rend en noir et blanc (l'étape « décodage »).
pub fn svg(mono: bool) -> String {
    let seed = 3.0;
    let horizon = H * 0.62;
    let (sx, sy) = (W * 0.62, horizon - H * 0.07);
    let mut s = String::with_capacity(16_000);
    let _ = write!(
        s,
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" preserveAspectRatio="xMidYMid slice"><defs>
<linearGradient id="sky" x1="0" y1="0" x2="0" y2="{horizon}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="hsl(235 45% 14%)"/><stop offset=".45" stop-color="hsl(280 40% 32%)"/><stop offset=".78" stop-color="hsl(18 85% 58%)"/><stop offset="1" stop-color="hsl(38 100% 72%)"/></linearGradient>
<radialGradient id="glow" cx="{sx}" cy="{sy}" r="{r}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="hsl(45 100% 90%)" stop-opacity=".95"/><stop offset=".08" stop-color="hsl(40 100% 70%)" stop-opacity=".6"/><stop offset="1" stop-color="hsl(20 100% 50%)" stop-opacity="0"/></radialGradient>
<linearGradient id="lake" x1="0" y1="{horizon}" x2="0" y2="{H}" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="hsl(30 80% 55%)"/><stop offset=".25" stop-color="hsl(290 30% 24%)"/><stop offset="1" stop-color="hsl(235 40% 8%)"/></linearGradient>
<filter id="mono"><feColorMatrix type="saturate" values="0"/></filter>
</defs><g{filter}><rect width="{W}" height="{H}" fill="url(#sky)"/><rect width="{W}" height="{H}" fill="url(#glow)"/><circle cx="{sx}" cy="{sy}" r="{sun:.1}" fill="hsl(50 100% 94%)"/>"##,
        r = W * 0.45,
        sun = W * 0.028,
        filter = if mono { r#" filter="url(#mono)""# } else { "" },
    );
    let layers = [
        (0.5, 0.07, "hsl(290 30% 34%)", 4.0),
        (0.56, 0.06, "hsl(270 32% 22%)", 6.0),
        (0.61, 0.035, "hsl(250 30% 12%)", 9.0),
    ];
    for (i, (base, amp, col, f)) in layers.into_iter().enumerate() {
        let crest = (0..=W as usize / 4).map(|px| {
            let x = px as f64 * 4.0;
            (x, H * (base - ridge(x / W, seed + i as f64, amp, f).abs()))
        });
        polygon(
            &mut s,
            col,
            [(0.0, horizon + 2.0)]
                .into_iter()
                .chain(crest)
                .chain([(W, horizon + 2.0)]),
        );
    }
    let _ = write!(
        s,
        r#"<rect y="{horizon}" width="{W}" height="{}" fill="url(#lake)"/>"#,
        H - horizon
    );
    let mut rand = Rand(977 * 3);
    for _ in 0..70 {
        let t = rand.next().powf(1.6);
        let ly = horizon + t * H * 0.38;
        let depth = (ly - horizon) / H;
        let lw = W * (0.03 + rand.next() * 0.18) * (1.0 - depth);
        let x = sx - lw / 2.0 + (rand.next() - 0.5) * W * 0.1;
        let _ = write!(
            s,
            r#"<rect x="{x:.1}" y="{ly:.1}" width="{lw:.1}" height="2" fill="hsl(45 100% 80%)" fill-opacity="{:.2}"/>"#,
            0.5 * (1.0 - t)
        );
    }
    let shore = (0..=W as usize / 4).map(|px| {
        let x = px as f64 * 4.0;
        (x, H * (0.93 - ridge(x / W, seed + 9.0, 0.05, 3.0).abs()))
    });
    polygon(
        &mut s,
        "hsl(250 30% 5%)",
        [(0.0, H)].into_iter().chain(shore).chain([(W, H)]),
    );
    s.push_str("</g></svg>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_a_landscape() {
        let color = svg(false);
        assert!(color.starts_with("<svg"));
        assert!(color.ends_with("</svg>"));
        assert_eq!(color.matches("<polygon").count(), 4);
        assert!(!color.contains("NaN"));
        assert!(svg(true).contains(r#"filter="url(#mono)""#));
    }

    #[test]
    fn the_same_seed_gives_the_same_photo() {
        assert_eq!(svg(false), svg(false));
    }
}
