//! Épreuvage à l'écran : ce que deviendra l'image une fois imprimée avec un profil
//! d'imprimante (ou montrée sur un autre écran).
//!
//! Chaque couleur affichée (sRGB) est envoyée vers l'imprimante avec l'intention de
//! rendu choisie, puis relue depuis l'imprimante en colorimétrique relative : le blanc
//! du papier reste le blanc de l'écran, les couleurs hors de portée de l'encre sont
//! rapprochées comme le fera l'impression. Le tout est calculé une fois, dans une
//! table 3D, puis appliqué à l'aperçu par interpolation.

use crate::profile::{Error, Intent, Profile};
use crate::{D50, SRGB, Vec3, apply, bradford, inverse, mul, rgb_to_xyz, srgb_decode, srgb_encode};

/// Les nœuds de la table sur chaque axe.
const GRID: usize = 33;

#[derive(Clone, Debug)]
pub struct Proof {
    table: Vec<Vec3>,
}

impl Proof {
    pub fn new(profile: &Profile, intent: Intent) -> Result<Self, Error> {
        if !profile.writes() || !profile.reads() {
            return Err(Error("ce profil ne convertit pas dans les deux sens"));
        }
        let to_pcs = mul(&bradford(SRGB.white, D50), &rgb_to_xyz(&SRGB));
        let from_pcs = inverse(&to_pcs).expect("matrice du sRGB");
        let g = (GRID - 1) as f32;
        let mut table = Vec::with_capacity(GRID * GRID * GRID);
        for r in 0..GRID {
            for gr in 0..GRID {
                for b in 0..GRID {
                    let rgb = [r, gr, b].map(|v| srgb_decode(v as f32 / g));
                    let xyz = apply(&to_pcs, rgb);
                    let printed = profile
                        .from_pcs(xyz, intent)
                        .and_then(|device| profile.to_pcs(&device, Intent::Relative))
                        .ok_or(Error("ce profil ne convertit pas dans les deux sens"))?;
                    table.push(apply(&from_pcs, printed).map(|v| srgb_encode(v.clamp(0.0, 1.0))));
                }
            }
        }
        Ok(Proof { table })
    }

    /// Une couleur sRGB (codée, de 0 à 1) telle qu'elle sera imprimée.
    pub fn apply(&self, rgb: Vec3) -> Vec3 {
        let g = (GRID - 1) as f32;
        let mut base = [0usize; 3];
        let mut f = [0.0f32; 3];
        for i in 0..3 {
            let pos = rgb[i].clamp(0.0, 1.0) * g;
            base[i] = (pos as usize).min(GRID - 2);
            f[i] = pos - base[i] as f32;
        }
        self.trilinear(base, f)
    }

    #[inline]
    fn trilinear(&self, [r, g, b]: [usize; 3], [fr, fg, fb]: [f32; 3]) -> Vec3 {
        let at = |i: usize, j: usize, k: usize| self.table[(i * GRID + j) * GRID + k];
        let mut out = [0.0; 3];
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
            for c in 0..3 {
                out[c] += w * v[c];
            }
        }
        out
    }

    /// Applique l'épreuvage à une image RGBA 8 bits en sRGB (l'aperçu), sur tous les
    /// cœurs.
    pub fn apply_rgba(&self, rgba: &mut [u8]) {
        // La place de chaque valeur 8 bits dans la grille, calculée une fois.
        let g = (GRID - 1) as f32;
        let mut cell = [(0usize, 0.0f32); 256];
        for (v, c) in cell.iter_mut().enumerate() {
            let pos = v as f32 / 255.0 * g;
            let i = (pos as usize).min(GRID - 2);
            *c = (i, pos - i as f32);
        }
        let pixels = rgba.len() / 4;
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(16);
        let per = pixels.div_ceil(threads).max(4096) * 4;
        std::thread::scope(|s| {
            for chunk in rgba.chunks_mut(per) {
                s.spawn(|| {
                    for p in chunk.as_chunks_mut::<4>().0 {
                        let [(r, fr), (gr, fg), (b, fb)] = [
                            cell[usize::from(p[0])],
                            cell[usize::from(p[1])],
                            cell[usize::from(p[2])],
                        ];
                        let v = self.trilinear([r, gr, b], [fr, fg, fb]);
                        for c in 0..3 {
                            p[c] = (v[c] * 255.0).round().clamp(0.0, 255.0) as u8;
                        }
                    }
                });
            }
        });
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
    fn an_input_only_profile_cannot_proof() {
        let p = Profile::parse(include_bytes!("../tests/profils/CGATS001Compat-v2-micro.icc")).unwrap();
        assert!(Proof::new(&p, Intent::Perceptual).is_err());
    }
}
