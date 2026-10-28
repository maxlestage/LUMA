//! Hautes lumières reconstruites : un canal saturé au capteur retrouve une valeur
//! plausible à partir des canaux qui ne le sont pas.
//!
//! Là où un seul canal sature (le rouge d'un coucher de soleil, le vert d'un feuillage
//! au soleil), les deux autres disent encore combien il y a de lumière ; il manque la
//! couleur. Elle est prise aux pixels non saturés du voisinage : leur chromaticité
//! (chaque canal sur la somme des trois), moyennée à trois portées de plus en plus
//! larges, sur une copie réduite. Chaque pixel touché garde ses canaux sûrs et prend,
//! pour les autres, la valeur qui lui donne cette couleur (jamais moins que ce que le
//! capteur a mesuré). Un pixel où tout sature reste blanc : rien ne dit sa couleur.

use luma_core::{Plane, RgbImage};
use rayon::prelude::*;

/// Le début et la fin du fondu vers la valeur reconstruite (1 = saturation du capteur).
const FROM: f32 = 0.94;
const TO: f32 = 0.995;
/// Un canal reconstruit ne dépasse pas quatre fois la saturation (deux diaphragmes).
const CEILING: f32 = 4.0;

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `cam` (RGB caméra, 0 à 1) aux canaux saturés reconstruits ; `None` s'il n'y a rien
/// à reconstruire (aucun pixel près de la saturation).
pub fn reconstruct(cam: &RgbImage) -> Option<RgbImage> {
    let touched = |p: &[f32; 3]| p[0].max(p[1]).max(p[2]) >= FROM;
    if cam.width < 2 || cam.height < 2 || !cam.data.par_iter().any(touched) {
        return None;
    }
    let chroma = neighbour_chroma(cam);
    let mut out = cam.clone();
    let (w, h) = (cam.width, cam.height);
    let columns = chroma[0].upsample_columns(w);
    out.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        if !row.iter().any(touched) {
            return;
        }
        let mut up: [Vec<f32>; 3] = std::array::from_fn(|_| vec![0.0; w]);
        for (c, plane) in chroma.iter().enumerate() {
            plane.upsample_row(y, h, &columns, &mut up[c]);
        }
        for (x, p) in row.iter_mut().enumerate() {
            if !touched(p) {
                continue;
            }
            let k = [up[0][x], up[1][x], up[2][x]];
            if k.iter().any(|v| !v.is_finite() || *v <= 1e-4) {
                continue;
            }
            // L'intensité : celle qui explique le mieux les canaux mesurés. Un canal saturé
            // n'en donne qu'une borne basse, si bien que le plus grand rapport est celui
            // d'un canal sûr.
            let scale = (0..3).map(|c| p[c] / k[c]).fold(0.0, f32::max);
            // Tous les canaux saturés : plus aucun canal sûr, le pixel reste blanc.
            let sure = 1.0 - p.map(|v| smoothstep(FROM, TO, v)).into_iter().fold(1.0, f32::min);
            for c in 0..3 {
                let t = smoothstep(FROM, TO, p[c]) * sure;
                if t > 0.0 {
                    // `min` puis `max` : un canal déjà au-dessus du plafond (RAW flottant,
                    // HDR) reste tel quel, là où `clamp` paniquerait.
                    let rebuilt = (scale * k[c]).min(CEILING).max(p[c]);
                    p[c] += (rebuilt - p[c]) * t;
                }
            }
        }
    });
    Some(out)
}

/// La chromaticité des pixels sûrs du voisinage, sur une copie d'au plus 512 px de
/// grand côté : trois plans, chacun un canal sur la somme. Là où les pixels sûrs
/// manquent à la portée courte (le cœur d'une grande zone saturée), la portée suivante
/// prend le relais.
fn neighbour_chroma(cam: &RgbImage) -> [Plane; 3] {
    let factor = (cam.width.max(cam.height) / 512).max(1);
    let small = cam.resize_area(cam.width.div_ceil(factor), cam.height.div_ceil(factor));
    let (w, h) = (small.width, small.height);
    let diag = ((w * w + h * h) as f32).sqrt();
    // Poids d'un pixel sûr : aucun canal près de la saturation, assez de lumière pour
    // que sa couleur ne soit pas du bruit.
    let weight = Plane::from_rgb(&small, |p| {
        let hi = p[0].max(p[1]).max(p[2]);
        (1.0 - smoothstep(FROM - 0.1, FROM, hi)) * smoothstep(0.02, 0.08, hi)
    });
    let part =
        |c: usize| Plane::from_rgb(&small, move |p| p[c] / (p[0] + p[1] + p[2]).max(1e-6)).zip(&weight, |k, w| k * w);
    let parts = [part(0), part(1), part(2)];
    let mut sum = [Plane::new(w, h), Plane::new(w, h), Plane::new(w, h)];
    let mut found = Plane::new(w, h);
    for reach in [0.01, 0.04, 0.16] {
        let sigma = (reach * diag).max(1.0);
        let wb = weight.blur(sigma);
        let kb: [Plane; 3] = std::array::from_fn(|c| parts[c].blur(sigma));
        for i in 0..w * h {
            // Ce que cette portée ajoute : ce qui manque encore de poids.
            let need = (1.0 - found.data[i]).max(0.0);
            if need <= 0.0 || wb.data[i] <= 1e-6 {
                continue;
            }
            let take = need * smoothstep(0.0, 0.05, wb.data[i]);
            for c in 0..3 {
                sum[c].data[i] += take * kb[c].data[i] / wb.data[i];
            }
            found.data[i] += take;
        }
    }
    // Ni sûr ni voisin : un gris, qui laisse le pixel tel quel (blanc).
    for i in 0..w * h {
        let rest = (1.0 - found.data[i]).max(0.0);
        for s in &mut sum {
            s.data[i] += rest / 3.0;
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_above_the_ceiling_do_not_panic() {
        // Un RAW flottant ou HDR : un canal déjà au-dessus du plafond.
        let cam = RgbImage::from_fn(16, 16, |x, _| if x < 8 { [5.0, 0.3, 0.2] } else { [0.5, 0.4, 0.3] });
        if let Some(out) = reconstruct(&cam) {
            assert!(out.data.iter().flatten().all(|v| v.is_finite()));
        }
    }

    /// Un ciel orangé, de teinte changeante, dont le rouge dépasse la saturation au centre : la vérité, et ce
    /// que le capteur en garde (canaux bornés à 1).
    fn sunset() -> (RgbImage, RgbImage) {
        let truth = RgbImage::from_fn(200, 120, |x, y| {
            let d = ((x as f32 - 100.0).powi(2) + (y as f32 - 60.0).powi(2)).sqrt();
            let light = 1.6 * (-d / 70.0).exp() + 0.1;
            // Une teinte qui change d'un bord à l'autre : du rouge vers l'orangé.
            let g = 0.4 + 0.25 * x as f32 / 200.0;
            [light, light * g, light * 0.25]
        });
        let mut clipped = truth.clone();
        clipped
            .data
            .iter_mut()
            .for_each(|p| p.iter_mut().for_each(|v| *v = v.min(1.0)));
        (truth, clipped)
    }

    fn hue_error(truth: &RgbImage, img: &RgbImage) -> f32 {
        // L'écart moyen de chromaticité (rouge sur vert) là où la vérité sature.
        let mut sum = 0.0;
        let mut n = 0.0;
        for (t, p) in truth.data.iter().zip(&img.data) {
            if t[0] > 1.0 {
                sum += (p[0] / p[1] - t[0] / t[1]).abs();
                n += 1.0;
            }
        }
        sum / n
    }

    #[test]
    fn a_clipped_red_gets_its_color_back() {
        let (truth, clipped) = sunset();
        let rebuilt = reconstruct(&clipped).expect("des pixels saturés");
        let (before, after) = (hue_error(&truth, &clipped), hue_error(&truth, &rebuilt));
        eprintln!("écart de teinte : {before:.3} → {after:.3}");
        assert!(after < before * 0.25, "{before} → {after}");
        // Et l'intensité du rouge revient, au‑delà de la saturation.
        let (mut lost, mut err, mut n) = (0.0, 0.0, 0.0);
        for ((t, c), r) in truth.data.iter().zip(&clipped.data).zip(&rebuilt.data) {
            if t[0] > 1.0 {
                lost += (t[0] - c[0]) / t[0];
                err += (t[0] - r[0]).abs() / t[0];
                n += 1.0;
            }
        }
        eprintln!("rouge perdu : {:.1} % → {:.1} %", lost / n * 100.0, err / n * 100.0);
        assert!(err < lost * 0.2);
        // Le vert et le bleu, sûrs, ne bougent pas ; le rouge ne descend jamais.
        for (c, r) in clipped.data.iter().zip(&rebuilt.data) {
            assert!(r[0] >= c[0] && (r[1] - c[1]).abs() < 1e-6 && (r[2] - c[2]).abs() < 1e-6);
        }
    }

    #[test]
    fn nothing_to_do_without_clipping_and_white_stays_white() {
        let dim = RgbImage::from_fn(50, 40, |x, _| [0.3 + x as f32 * 0.005, 0.4, 0.2]);
        assert!(reconstruct(&dim).is_none());
        // Tout saturé : aucune couleur à inventer.
        let white = RgbImage::from_fn(50, 40, |_, _| [1.0; 3]);
        let out = reconstruct(&white).unwrap();
        assert!(out.data.iter().all(|p| p.iter().all(|v| (v - 1.0).abs() < 1e-3)));
        // Un cœur tout blanc au milieu d'un ciel orangé reste blanc lui aussi.
        let sky = RgbImage::from_fn(80, 60, |x, y| {
            let d = ((x as f32 - 40.0).powi(2) + (y as f32 - 30.0).powi(2)).sqrt();
            if d < 10.0 { [1.0; 3] } else { [0.6, 0.35, 0.15] }
        });
        let out = reconstruct(&sky).unwrap();
        let core = out.data[30 * 80 + 40];
        assert!(core.iter().all(|v| (v - 1.0).abs() < 1e-3), "{core:?}");
    }
}
