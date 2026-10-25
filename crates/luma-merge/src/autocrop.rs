//! Le rognage automatique : le plus grand rectangle entièrement couvert par les photos
//! fusionnées (les bords d'un panorama ou d'un empilement aligné sont irréguliers).
//!
//! Il n'est jamais imposé : la fusion est enregistrée entière, et ce rectangle devient
//! un recadrage ordinaire dans ses réglages, qu'on peut modifier ou retirer.

use luma_project::Crop;

/// Le plus grand rectangle de `true` dans une grille `w × h` (méthode des histogrammes,
/// en temps linéaire). Rend (gauche, haut, droite, bas) en cellules, bornes exclues.
pub fn largest_rectangle(mask: &[bool], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let mut heights = vec![0usize; w];
    let mut best = (0usize, None);
    for y in 0..h {
        for x in 0..w {
            heights[x] = if mask[y * w + x] { heights[x] + 1 } else { 0 };
        }
        // Pile des colonnes à hauteur croissante.
        let mut stack: Vec<usize> = Vec::new();
        for x in 0..=w {
            let cur = if x < w { heights[x] } else { 0 };
            while let Some(&top) = stack.last() {
                if heights[top] <= cur {
                    break;
                }
                stack.pop();
                let height = heights[top];
                let left = stack.last().map_or(0, |&l| l + 1);
                let area = height * (x - left);
                if area > best.0 {
                    best = (area, Some((left, y + 1 - height, x, y + 1)));
                }
            }
            stack.push(x);
        }
    }
    best.1
}

/// Le recadrage qui garde le plus grand rectangle entièrement couvert, `None` si tout
/// est déjà couvert. `covered[i]` : le pixel `i` (image `w × h`) a des données.
pub fn auto_crop(covered: &[bool], w: usize, h: usize) -> Option<Crop> {
    if covered.iter().all(|c| *c) {
        return None;
    }
    // Sur une grille réduite (au plus 600 cellules de large) : une cellule ne compte
    // que si tous ses pixels sont couverts.
    let step = w.max(h).div_ceil(600).max(1);
    let (gw, gh) = (w.div_ceil(step), h.div_ceil(step));
    let mut grid = vec![true; gw * gh];
    for y in 0..h {
        for x in 0..w {
            if !covered[y * w + x] {
                grid[(y / step) * gw + x / step] = false;
            }
        }
    }
    let (l, t, r, b) = largest_rectangle(&grid, gw, gh)?;
    let fx = |c: usize| ((c * step).min(w)) as f32 / w as f32;
    let fy = |c: usize| ((c * step).min(h)) as f32 / h as f32;
    Some(Crop {
        left: fx(l),
        top: fy(t),
        right: fx(r),
        bottom: fy(b),
        angle: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_largest_rectangle() {
        // Un losange dans 9 × 7 : le plus grand rectangle est au centre.
        let (w, h) = (9usize, 7usize);
        let mask: Vec<bool> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as i32 - 4, (i / w) as i32 - 3);
                x.abs() + y.abs() <= 4
            })
            .collect();
        let (l, t, r, b) = largest_rectangle(&mask, w, h).unwrap();
        assert_eq!((r - l) * (b - t), 25, "{:?}", (l, t, r, b));
        for y in t..b {
            for x in l..r {
                assert!(mask[y * w + x]);
            }
        }
    }

    #[test]
    fn full_coverage_needs_no_crop_and_borders_are_cut() {
        assert!(auto_crop(&[true; 12], 4, 3).is_none());
        let (w, h) = (100, 50);
        let covered: Vec<bool> = (0..w * h).map(|i| (i % w) >= 10 && (i / w) < 45).collect();
        let c = auto_crop(&covered, w, h).unwrap();
        assert!((c.left - 0.1).abs() < 1e-6 && c.right == 1.0 && c.top == 0.0 && (c.bottom - 0.9).abs() < 1e-6);
    }
}
