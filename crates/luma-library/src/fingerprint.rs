//! L'empreinte d'une image, pour trouver les doublons : la même photo copiée, exportée
//! à une autre taille ou recompressée garde la même empreinte, à quelques bits près.
//!
//! L'image est réduite à 9 × 8 cases de luminance (moyennes) ; chaque bit dit si une
//! case est plus claire que sa voisine de gauche (« dHash ») : 64 bits qui ne
//! dépendent ni de la taille, ni de la compression, ni d'une exposition un peu
//! différente.

use std::path::PathBuf;

/// L'écart (en bits) en deçà duquel deux empreintes sont celles de la même image.
pub const SAME: u32 = 5;

/// L'empreinte d'une image RGBA 8 bits (`width` × `height`), `None` si elle est trop
/// petite pour en avoir une.
pub fn fingerprint(rgba: &[u8], width: usize, height: usize) -> Option<u64> {
    if width < 9 || height < 8 || rgba.len() < width * height * 4 {
        return None;
    }
    let mut cells = [[0.0f32; 9]; 8];
    for (cy, row) in cells.iter_mut().enumerate() {
        let (y0, y1) = (cy * height / 8, (cy + 1) * height / 8);
        for (cx, cell) in row.iter_mut().enumerate() {
            let (x0, x1) = (cx * width / 9, (cx + 1) * width / 9);
            let mut sum = 0u64;
            for y in y0..y1 {
                for p in rgba[(y * width + x0) * 4..(y * width + x1) * 4].as_chunks::<4>().0 {
                    // Luminance approchée (poids entiers du Rec. 709).
                    sum += u64::from(p[0]) * 54 + u64::from(p[1]) * 183 + u64::from(p[2]) * 19;
                }
            }
            *cell = sum as f32 / ((y1 - y0) * (x1 - x0)).max(1) as f32;
        }
    }
    let mut bits = 0u64;
    for (cy, row) in cells.iter().enumerate() {
        for cx in 0..8 {
            if row[cx + 1] > row[cx] {
                bits |= 1 << (cy * 8 + cx);
            }
        }
    }
    Some(bits)
}

/// L'écart entre deux empreintes, en bits.
pub fn distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Les groupes de doublons (deux photos au moins), dans l'ordre de leur première photo ;
/// une photo proche d'une autre du groupe y entre.
pub fn duplicates(prints: &[(PathBuf, u64)]) -> Vec<Vec<PathBuf>> {
    let n = prints.len();
    // Union des photos proches deux à deux.
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for i in 0..n {
        for j in i + 1..n {
            if distance(prints[i].1, prints[j].1) <= SAME {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                if a != b {
                    parent[b.max(a)] = a.min(b);
                }
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<PathBuf>> = Default::default();
    for (i, (path, _)) in prints.iter().enumerate() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(path.clone());
    }
    groups.into_values().filter(|g| g.len() >= 2).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une « photo » : des formes douces, à la taille voulue.
    fn picture(w: usize, h: usize, seed: f32, gain: f32) -> Vec<u8> {
        let mut out = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
                let base = (u * 6.0 + seed).sin() * (v * 4.0 - seed).cos() * 0.5 + 0.5;
                let px = |k: f32| ((base * k * gain).clamp(0.0, 1.0) * 255.0) as u8;
                out.extend([px(1.0), px(0.8), px(0.6), 255]);
            }
        }
        out
    }

    #[test]
    fn the_same_picture_at_any_size() {
        let big = fingerprint(&picture(600, 400, 1.0, 1.0), 600, 400).unwrap();
        let small = fingerprint(&picture(150, 100, 1.0, 1.0), 150, 100).unwrap();
        let brighter = fingerprint(&picture(300, 200, 1.0, 1.1), 300, 200).unwrap();
        let other = fingerprint(&picture(600, 400, 2.5, 1.0), 600, 400).unwrap();
        assert!(distance(big, small) <= SAME, "{}", distance(big, small));
        assert!(distance(big, brighter) <= SAME, "{}", distance(big, brighter));
        assert!(distance(big, other) > 3 * SAME, "{}", distance(big, other));
        assert!(fingerprint(&[0; 32], 4, 2).is_none());
    }

    #[test]
    fn groups_of_duplicates() {
        let p = |name: &str, print: u64| (PathBuf::from(name), print);
        let groups = duplicates(&[
            p("a.ARW", 0b1111_0000),
            p("b.jpg", 0b1111_0001),
            p("c.ARW", u64::MAX),
            p("d.tif", 0b0111_0000),
            p("e.ARW", u64::MAX ^ 1),
        ]);
        assert_eq!(
            groups,
            vec![
                vec![PathBuf::from("a.ARW"), PathBuf::from("b.jpg"), PathBuf::from("d.tif")],
                vec![PathBuf::from("c.ARW"), PathBuf::from("e.ARW")],
            ]
        );
    }
}
