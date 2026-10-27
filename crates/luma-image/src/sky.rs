//! Le ciel d'une photo, sans IA : ce qui est clair, peu texturé, d'une couleur proche de
//! celle du haut de l'image (ou clair et sans couleur, comme un nuage), et relié à son
//! bord supérieur. Un haut d'image qui n'est ni nettement plus clair que le bas, ni bleu
//! ou neutre, n'est pas un ciel (un mur, un intérieur). Le masque grossier est
//! ensuite affiné par un filtre guidé sur la clarté de la photo : il suit les toits, les
//! arbres et les branches fines.
//!
//! Tout se calcule sur une copie de 256 px de grand côté, puis s'agrandit : la zone est
//! la même à toutes les tailles de rendu.

use luma_color::{OkLab, luminance};
use luma_core::{Plane, RgbImage};

/// Le grand côté de la copie d'analyse.
const ANALYSIS: usize = 256;

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// La zone du ciel de `img` (lumière linéaire de travail), à la taille `w × h`.
pub fn sky(img: &RgbImage, w: usize, h: usize) -> Plane {
    // Toujours la même résolution d'analyse : la texture se mesure à la même échelle,
    // et la zone est la même quelle que soit la taille du rendu.
    let (aw, ah) = RgbImage::fit(img.width, img.height, ANALYSIS);
    let small = if (aw, ah) == (img.width, img.height) {
        img.clone()
    } else {
        img.resize_area(aw, ah)
    };
    let (sw, sh) = (small.width, small.height);
    if sw < 8 || sh < 8 {
        return Plane::new(w, h);
    }
    let ok = OkLab::for_working();
    let lab: Vec<[f32; 3]> = small.data.iter().map(|p| ok.from_rgb(p.map(|v| v.max(0.0)))).collect();
    let lightness = Plane {
        width: sw,
        height: sh,
        data: lab.iter().map(|l| l[0]).collect(),
    };
    // La texture : l'écart de la clarté à sa moyenne locale.
    let mean = lightness.blur(1.0);
    let texture = lightness.zip(&mean, |l, m| (l - m).abs()).blur(1.0);
    // La couleur du ciel : la médiane des pixels clairs et lisses du dixième supérieur.
    let top: Vec<usize> = (0..sw * (sh / 10).max(1))
        .filter(|&i| texture.data[i] < 0.03 && lab[i][0] > 0.4)
        .collect();
    if top.len() < sw / 4 {
        return Plane::new(w, h);
    }
    let median = |c: usize| {
        let mut v: Vec<f32> = top.iter().map(|&i| lab[i][c]).collect();
        let mid = v.len() / 2;
        *v.select_nth_unstable_by(mid, f32::total_cmp).1
    };
    let reference = [median(0), median(1), median(2)];
    // Un ciel est bien plus clair que le bas de l'image, et bleu ou neutre : un mur beige
    // ou un intérieur clair n'en est pas un.
    let mut bottom: Vec<f32> = lab[sw * sh / 2..].iter().map(|l| l[0]).collect();
    let mid = bottom.len() / 2;
    let ground = *bottom.select_nth_unstable_by(mid, f32::total_cmp).1;
    let chroma = (reference[1].powi(2) + reference[2].powi(2)).sqrt();
    let warm = chroma > 0.025 && reference[2] > 0.0;
    if reference[0] - ground < 0.15 || warm {
        return Plane::new(w, h);
    }
    // La ressemblance : couleur (OKLab) proche de celle du ciel, ou plus claire encore
    // (le soleil, les nuages blancs), peu de texture.
    let likeness: Vec<f32> = (0..sw * sh)
        .map(|i| {
            let [l, a, b] = lab[i];
            let dl = (reference[0] - l).max(0.0) * 0.6;
            let dc = ((a - reference[1]).powi(2) + (b - reference[2]).powi(2)).sqrt();
            let colour = 1.0 - smoothstep(0.05, 0.14, (dl * dl + dc * dc).sqrt());
            // Les nuages : presque aussi clairs que le ciel, sans couleur.
            let c = (a * a + b * b).sqrt();
            let cloud = smoothstep(reference[0] - 0.4, reference[0] - 0.18, l) * (1.0 - smoothstep(0.03, 0.07, c));
            let colour = colour.max(cloud);
            let smooth = 1.0 - smoothstep(0.02, 0.07, texture.data[i]);
            let bright = smoothstep(0.25, 0.45, l);
            colour * smooth * bright
        })
        .collect();
    // Relié au haut : on part du bord supérieur et on avance de proche en proche sur
    // ce qui ressemble au ciel.
    let mut reached = vec![false; sw * sh];
    let mut stack: Vec<usize> = (0..sw).filter(|&x| likeness[x] > 0.5).collect();
    for &i in &stack {
        reached[i] = true;
    }
    while let Some(i) = stack.pop() {
        let (x, y) = (i % sw, i / sw);
        let mut visit = |j: usize| {
            if !reached[j] && likeness[j] > 0.5 {
                reached[j] = true;
                stack.push(j);
            }
        };
        if x > 0 {
            visit(i - 1);
        }
        if x + 1 < sw {
            visit(i + 1);
        }
        if y > 0 {
            visit(i - sw);
        }
        if y + 1 < sh {
            visit(i + sw);
        }
    }
    // Le masque grossier, puis affiné sur la clarté de la photo (le filtre guidé garde
    // les bords nets et laisse passer le ciel entre les branches).
    let coarse = Plane {
        width: sw,
        height: sh,
        data: (0..sw * sh)
            .map(|i| if reached[i] { likeness[i].max(0.5) } else { 0.0 })
            .collect(),
    };
    let guide = Plane {
        width: sw,
        height: sh,
        data: small.data.iter().map(|p| luminance(*p).max(0.0).sqrt()).collect(),
    };
    let diag = ((sw * sw + sh * sh) as f32).sqrt();
    let refined = coarse.guided_by(&guide, (diag * 0.006).max(1.0), 1e-4);
    let refined = refined.map(|v| smoothstep(0.2, 0.8, v));
    if (sw, sh) == (w, h) {
        refined
    } else {
        refined.upsample(w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un paysage : un ciel en dégradé, un sol texturé, un tronc qui monte dans le ciel,
    /// et un lac bleu en bas (de la couleur du ciel, mais pas relié au haut).
    fn landscape() -> RgbImage {
        RgbImage::from_fn(400, 300, |x, y| {
            let n = (((x * 7919 + y * 104_729) % 97) as f32 / 97.0 - 0.5) * 0.08;
            let trunk = (190..205).contains(&x) && y > 60;
            if trunk {
                return [0.04 + n * 0.1, 0.03, 0.02];
            }
            if y < 150 {
                let t = y as f32 / 150.0;
                [0.25 + 0.15 * t, 0.4 + 0.15 * t, 0.75]
            } else if (60..160).contains(&x) && (220..260).contains(&y) {
                [0.3, 0.45, 0.75]
            } else {
                [0.12 + n, 0.16 + n, 0.06 + n * 0.5]
            }
        })
    }

    #[test]
    fn the_sky_is_found_and_the_lake_is_not() {
        let img = landscape();
        let m = sky(&img, 400, 300);
        let at = |x: usize, y: usize| m.data[y * 400 + x];
        assert!(
            at(100, 40) > 0.9 && at(330, 120) > 0.9,
            "ciel {} {}",
            at(100, 40),
            at(330, 120)
        );
        assert!(
            at(100, 220) < 0.1 && at(300, 280) < 0.1,
            "sol {} {}",
            at(100, 220),
            at(300, 280)
        );
        assert!(at(110, 240) < 0.1, "le lac n'est pas le ciel : {}", at(110, 240));
        assert!(at(197, 120) < 0.3, "le tronc dans le ciel : {}", at(197, 120));
        // Une photo sans ciel (le haut texturé et sombre) : rien.
        let forest = RgbImage::from_fn(200, 150, |x, y| {
            let n = (((x * 31 + y * 17) % 13) as f32 / 13.0) * 0.2;
            [0.05 + n, 0.12 + n, 0.04]
        });
        assert!(sky(&forest, 200, 150).data.iter().all(|v| *v < 0.1));
    }
}
