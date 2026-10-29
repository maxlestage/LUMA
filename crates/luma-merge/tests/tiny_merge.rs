//! Des vues minuscules ou de tailles différentes : une fusion rend une image ou une
//! erreur, jamais une panique.

use std::sync::Arc;

use luma_core::RgbImage;
use luma_merge::pano::panorama_images;
use luma_merge::stack::stack_images;
use luma_merge::{Projection, StackMode};

fn flat(w: usize, h: usize, v: f32) -> Arc<RgbImage> {
    Arc::new(RgbImage::from_fn(w, h, |x, y| {
        [v, v * 0.5 + (x % 2) as f32 * 0.1, (y % 3) as f32 * 0.2]
    }))
}

#[test]
fn tiny_or_mismatched_frames_never_panic() {
    let sizes = [(1, 1), (2, 1), (1, 5), (3, 3), (7, 2)];
    let modes = [
        StackMode::Mean,
        StackMode::Median,
        StackMode::Maximum,
        StackMode::Stars,
        StackMode::Hdr,
        StackMode::Focus,
    ];
    let mut panics = Vec::new();
    for &(w, h) in &sizes {
        for (w2, h2) in [(w, h), (w + 1, h + 2)] {
            let frames = || vec![flat(w, h, 0.2), flat(w2, h2, 0.6), flat(w, h, 0.9)];
            for mode in modes {
                for align in [false, true] {
                    let f = frames();
                    if std::panic::catch_unwind(move || stack_images(f, mode, align).map(|_| ())).is_err() {
                        panics.push(format!("{mode:?} {w}×{h} {w2}×{h2} alignées {align}"));
                    }
                }
            }
            for p in [Projection::Perspective, Projection::Cylindrical, Projection::Spherical] {
                let f = frames();
                if std::panic::catch_unwind(move || panorama_images(f, p).map(|_| ())).is_err() {
                    panics.push(format!("panorama {p:?} {w}×{h} {w2}×{h2}"));
                }
            }
        }
    }
    assert!(panics.is_empty(), "{} paniques : {panics:#?}", panics.len());
}

/// Une image vide (0 × 0, un fichier abîmé) ou d'un pixel : la mesure d'une pellicule
/// rend une erreur, l'étirement d'un panorama rend l'image telle quelle.
#[test]
fn empty_images_never_panic_in_film_or_warp() {
    use luma_merge::film::{Pair, measure};
    let img = |w: usize, h: usize| RgbImage::from_fn(w, h, |x, y| [x as f32 * 0.1, y as f32 * 0.1, 0.5]);
    let mut panics = Vec::new();
    for (w, h) in [(0, 0), (1, 1), (2, 1), (1, 7)] {
        for aligned in [false, true] {
            for (w2, h2) in [(w, h), (w + 3, h + 1), (0, 0)] {
                let p = vec![Pair {
                    digital: img(w, h),
                    film: img(w2, h2),
                    aligned,
                }];
                if std::panic::catch_unwind(move || measure(&p, 17, "essai").map(|_| ())).is_err() {
                    panics.push(format!("pellicule {w}×{h} sur {w2}×{h2}, alignées {aligned}"));
                }
            }
        }
        let i = img(w, h);
        for covered in [
            vec![true; w * h],
            (0..w * h).map(|k| k % 2 == 0).collect(),
            vec![true; w * h + 1],
        ] {
            let n = covered.len();
            match std::panic::catch_unwind(|| luma_merge::warp::boundary_warp(&i, &covered)) {
                Ok((out, _)) => assert_eq!((out.width, out.height), (w, h)),
                Err(_) => panics.push(format!("étirement {w}×{h}, couverture de {n}")),
            }
        }
    }
    assert!(panics.is_empty(), "{} paniques : {panics:#?}", panics.len());
}
