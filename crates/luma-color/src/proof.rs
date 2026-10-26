//! Épreuvage à l'écran : ce que deviendra l'image une fois imprimée avec un profil
//! d'imprimante (ou montrée sur un autre écran), et l'alerte de gamut.
//!
//! Chaque couleur affichée (sRGB) est envoyée vers l'imprimante avec l'intention de
//! rendu choisie, puis relue depuis l'imprimante en colorimétrique relative : le blanc
//! du papier reste le blanc de l'écran, les couleurs hors de portée de l'encre sont
//! rapprochées comme le fera l'impression. Le tout est calculé une fois, dans une
//! table 3D, puis appliqué à l'aperçu par interpolation.
//!
//! L'alerte de gamut : une couleur que l'imprimante ne sait pas rendre ne revient pas
//! d'un aller et retour en colorimétrique relative ; au‑delà d'un écart de
//! [`GAMUT_DELTA_E`], elle est signalée.

use crate::profile::{Error, Intent, Profile, xyz_to_lab};
use crate::{D50, SRGB, Vec3, apply, bradford, inverse, mul, rgb_to_xyz, srgb_decode, srgb_encode};

/// Les nœuds de la table sur chaque axe.
const GRID: usize = 33;

/// L'écart (ΔE 1976) au‑delà duquel une couleur est hors de portée de l'imprimante :
/// au‑dessus des erreurs d'interpolation des tables d'un profil, bien en dessous de ce
/// que perd une couleur vraiment hors gamut.
pub const GAMUT_DELTA_E: f32 = 3.0;

/// La couleur de l'alerte de gamut sur l'aperçu : un bleu franc, rare dans les photos.
pub const ALERT: [u8; 3] = [40, 120, 255];

const NOT_BOTH_WAYS: Error = Error("ce profil ne convertit pas dans les deux sens");

#[derive(Clone, Debug)]
pub struct Proof {
    /// Pour chaque nœud : la couleur imprimée (sRGB codé) et 1 si la couleur d'origine
    /// est hors gamut, 0 sinon.
    table: Vec<[f32; 4]>,
}

impl Proof {
    pub fn new(profile: &Profile, intent: Intent) -> Result<Self, Error> {
        if !profile.writes() || !profile.reads() {
            return Err(NOT_BOTH_WAYS);
        }
        let to_pcs = mul(&bradford(SRGB.white, D50), &rgb_to_xyz(&SRGB));
        let from_pcs = inverse(&to_pcs).expect("matrice du sRGB");
        let round_trip = |xyz: Vec3, intent: Intent| {
            profile
                .from_pcs(xyz, intent)
                .and_then(|device| profile.to_pcs(&device, Intent::Relative))
                .ok_or(NOT_BOTH_WAYS)
        };
        let g = (GRID - 1) as f32;
        let mut table = Vec::with_capacity(GRID * GRID * GRID);
        for r in 0..GRID {
            for gr in 0..GRID {
                for b in 0..GRID {
                    let rgb = [r, gr, b].map(|v| srgb_decode(v as f32 / g));
                    let xyz = apply(&to_pcs, rgb);
                    let printed = round_trip(xyz, intent)?;
                    let exact = if intent == Intent::Relative {
                        printed
                    } else {
                        round_trip(xyz, Intent::Relative)?
                    };
                    let (a, e) = (xyz_to_lab(xyz), xyz_to_lab(exact));
                    let delta = ((a[0] - e[0]).powi(2) + (a[1] - e[1]).powi(2) + (a[2] - e[2]).powi(2)).sqrt();
                    let [pr, pg, pb] = apply(&from_pcs, printed).map(|v| srgb_encode(v.clamp(0.0, 1.0)));
                    table.push([pr, pg, pb, if delta > GAMUT_DELTA_E { 1.0 } else { 0.0 }]);
                }
            }
        }
        Ok(Proof { table })
    }

    fn cell(rgb: Vec3) -> ([usize; 3], [f32; 3]) {
        let g = (GRID - 1) as f32;
        let mut base = [0usize; 3];
        let mut f = [0.0f32; 3];
        for i in 0..3 {
            let pos = rgb[i].clamp(0.0, 1.0) * g;
            base[i] = (pos as usize).min(GRID - 2);
            f[i] = pos - base[i] as f32;
        }
        (base, f)
    }

    /// Une couleur sRGB (codée, de 0 à 1) telle qu'elle sera imprimée.
    pub fn apply(&self, rgb: Vec3) -> Vec3 {
        let (base, f) = Self::cell(rgb);
        let v = self.trilinear(base, f);
        [v[0], v[1], v[2]]
    }

    /// La couleur sRGB (codée) est‑elle hors de portée de l'imprimante ?
    pub fn out_of_gamut(&self, rgb: Vec3) -> bool {
        let (base, f) = Self::cell(rgb);
        self.trilinear(base, f)[3] > 0.5
    }

    #[inline]
    fn trilinear(&self, [r, g, b]: [usize; 3], [fr, fg, fb]: [f32; 3]) -> [f32; 4] {
        let at = |i: usize, j: usize, k: usize| self.table[(i * GRID + j) * GRID + k];
        let mut out = [0.0; 4];
        for (corner, w) in [
            ((0, 0, 0), (1.0 - fr) * (1.0 - fg) * (1.0 - fb)),
            ((1, 0, 0), fr * (1.0 - fg) * (1.0 - fb)),
            ((0, 1, 0), (1.0 - fr) * fg * (1.0 - fb)),
            ((1, 1, 0), fr * fg * (1.0 - fb)),
            ((0, 0, 1), (1.0 - fr) * (1.0 - fg) * fb),
            ((1, 0, 1), fr * (1.0 - fg) * fb),
            ((0, 1, 1), (1.0 - fr) * fg * fb),
            ((1, 1, 1), fr * fg * fb),
        ] {
            let v = at(r + corner.0, g + corner.1, b + corner.2);
            for c in 0..4 {
                out[c] += w * v[c];
            }
        }
        out
    }

    /// La place de chaque valeur 8 bits dans la grille.
    fn cells() -> [(usize, f32); 256] {
        let g = (GRID - 1) as f32;
        let mut cell = [(0usize, 0.0f32); 256];
        for (v, c) in cell.iter_mut().enumerate() {
            let pos = v as f32 / 255.0 * g;
            let i = (pos as usize).min(GRID - 2);
            *c = (i, pos - i as f32);
        }
        cell
    }

    /// Les pixels d'une image RGBA (ou RGB…, `step` octets par pixel), par tranches, sur
    /// tous les cœurs.
    fn each_pixel<T: Send>(data: &mut [T], step: usize, f: impl Fn(&mut [T]) + Sync) {
        let pixels = data.len() / step;
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(16);
        let per = pixels.div_ceil(threads).max(4096) * step;
        std::thread::scope(|s| {
            for chunk in data.chunks_mut(per) {
                s.spawn(|| {
                    for p in chunk.chunks_exact_mut(step) {
                        f(p);
                    }
                });
            }
        });
    }

    /// Applique l'épreuvage à une image RGBA 8 bits en sRGB (l'aperçu).
    pub fn apply_rgba(&self, rgba: &mut [u8]) {
        let cell = Self::cells();
        Self::each_pixel(rgba, 4, |p| {
            let [(r, fr), (g, fg), (b, fb)] = [0, 1, 2].map(|c| cell[usize::from(p[c])]);
            let v = self.trilinear([r, g, b], [fr, fg, fb]);
            for c in 0..3 {
                p[c] = (v[c] * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        });
    }

    /// Les pixels d'une image RGBA 8 bits en sRGB que l'imprimante ne saura pas rendre.
    pub fn gamut_mask(&self, rgba: &[u8]) -> Vec<bool> {
        let cell = Self::cells();
        let mut mask: Vec<(bool, [u8; 3])> = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| (false, [p[0], p[1], p[2]]))
            .collect();
        Self::each_pixel(&mut mask, 1, |m| {
            let [(r, fr), (g, fg), (b, fb)] = [0, 1, 2].map(|c| cell[usize::from(m[0].1[c])]);
            m[0].0 = self.trilinear([r, g, b], [fr, fg, fb])[3] > 0.5;
        });
        mask.into_iter().map(|(m, _)| m).collect()
    }
}

/// Peint en [`ALERT`] les pixels hors gamut d'une image RGBA.
pub fn paint_alert(rgba: &mut [u8], mask: &[bool]) {
    for (p, out) in rgba.as_chunks_mut::<4>().0.iter_mut().zip(mask) {
        if *out {
            p[..3].copy_from_slice(&ALERT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OutputSpace, icc};

    #[test]
    fn proofing_to_srgb_changes_nothing() {
        let p = Profile::parse(&icc::profile(OutputSpace::Srgb)).unwrap();
        for intent in [Intent::Perceptual, Intent::Relative] {
            let proof = Proof::new(&p, intent).unwrap();
            for c in [
                [0.2, 0.5, 0.8],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 1.0],
                [0.53, 0.51, 0.49],
            ] {
                let v = proof.apply(c);
                for i in 0..3 {
                    assert!((v[i] - c[i]).abs() < 3e-3, "{c:?} → {v:?}");
                }
            }
            let mut rgba = vec![10, 128, 250, 255, 255, 0, 0, 7];
            proof.apply_rgba(&mut rgba);
            assert!(
                rgba.iter()
                    .zip([10, 128, 250, 255, 255, 0, 0, 7])
                    .all(|(a, b)| a.abs_diff(b) <= 1),
                "{rgba:?}"
            );
        }
    }

    #[test]
    fn a_wider_screen_changes_nothing() {
        // Tout le sRGB tient dans le Display P3 : rien ne bouge.
        let p = Profile::parse(&icc::profile(OutputSpace::DisplayP3)).unwrap();
        let proof = Proof::new(&p, Intent::Relative).unwrap();
        for c in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.1, 0.3, 0.9]] {
            let v = proof.apply(c);
            for i in 0..3 {
                assert!((v[i] - c[i]).abs() < 4e-3, "{c:?} → {v:?}");
            }
        }
    }

    /// Le profil sRGB aux primaires rapprochées du blanc : un écran plus terne.
    fn dull_screen() -> Profile {
        let mut bytes = icc::profile(OutputSpace::Srgb);
        let tag = |bytes: &[u8], sig: &[u8; 4]| {
            let at = bytes.windows(4).position(|w| w == sig).unwrap();
            u32::from_be_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize + 8
        };
        let read = |bytes: &[u8], at: usize| i32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as f32 / 65536.0;
        let cols: Vec<usize> = [b"rXYZ", b"gXYZ", b"bXYZ"].iter().map(|s| tag(&bytes, s)).collect();
        let white: Vec<f32> = (0..3)
            .map(|i| cols.iter().map(|&c| read(&bytes, c + 4 * i)).sum())
            .collect();
        for &c in &cols {
            for i in 0..3 {
                let v = 0.6 * read(&bytes, c + 4 * i) + 0.4 * white[i] / 3.0;
                bytes[c + 4 * i..c + 4 * i + 4].copy_from_slice(&((v * 65536.0).round() as i32).to_be_bytes());
            }
        }
        Profile::parse(&bytes).unwrap()
    }

    #[test]
    fn a_duller_screen_dulls_strong_colours_only() {
        let proof = Proof::new(&dull_screen(), Intent::Relative).unwrap();
        let red = proof.apply([1.0, 0.0, 0.0]);
        assert!(red[1] > 0.15 && red[2] > 0.15, "le rouge pur ne tient plus : {red:?}");
        for grey in [[0.5, 0.5, 0.5], [0.9, 0.9, 0.9], [0.1, 0.1, 0.1]] {
            let v = proof.apply(grey);
            for c in 0..3 {
                assert!((v[c] - grey[c]).abs() < 4e-3, "{grey:?} → {v:?}");
            }
        }
        // Une couleur douce, à la portée de l'écran terne, ne bouge pas.
        let soft = [0.55, 0.5, 0.45];
        let v = proof.apply(soft);
        for c in 0..3 {
            assert!((v[c] - soft[c]).abs() < 5e-3, "{soft:?} → {v:?}");
        }
    }

    #[test]
    fn gamut_alert_flags_what_the_print_cannot_hold() {
        let srgb = Proof::new(
            &Profile::parse(&icc::profile(OutputSpace::Srgb)).unwrap(),
            Intent::Perceptual,
        )
        .unwrap();
        let dull = Proof::new(&dull_screen(), Intent::Perceptual).unwrap();
        for c in [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.5, 0.5, 0.5],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
        ] {
            assert!(!srgb.out_of_gamut(c), "tout le sRGB tient dans le sRGB : {c:?}");
        }
        for c in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.9, 0.2, 0.6]] {
            assert!(dull.out_of_gamut(c), "{c:?}");
        }
        for c in [[0.5, 0.5, 0.5], [0.55, 0.5, 0.45], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0]] {
            assert!(!dull.out_of_gamut(c), "{c:?}");
        }
        // Sur l'aperçu : le rouge pur passe à la couleur de l'alerte, le reste ne bouge pas.
        let mut rgba = vec![255, 0, 0, 255, 128, 128, 128, 255, 140, 128, 115, 255];
        let mask = dull.gamut_mask(&rgba);
        assert_eq!(mask, [true, false, false]);
        paint_alert(&mut rgba, &mask);
        assert_eq!(&rgba[..3], &ALERT);
        assert_eq!(&rgba[4..], &[128, 128, 128, 255, 140, 128, 115, 255]);
    }

    #[test]
    fn an_input_only_profile_cannot_proof() {
        let p = Profile::parse(include_bytes!("../tests/profils/CGATS001Compat-v2-micro.icc")).unwrap();
        assert!(Proof::new(&p, Intent::Perceptual).is_err());
    }
}
