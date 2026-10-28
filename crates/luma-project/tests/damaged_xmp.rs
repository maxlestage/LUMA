//! Des XMP abîmés (coupés par une écriture interrompue, des caractères brouillés,
//! modifiés à la main ou par un autre logiciel) : à la lecture, des réglages ou une
//! erreur, jamais une panique.

use luma_project::{Dab, Mask, Range, Settings, Shape, Spot, SpotKind, Warp, WarpKind, xmp};

fn everything() -> Settings {
    let mut s = Settings {
        exposure: 0.7,
        contrast: 20.0,
        clarity: 15.0,
        ..Settings::default()
    };
    let mut brush = Mask::brush();
    if let Shape::Brush { dabs } = &mut brush.shape {
        dabs.push(Dab {
            x: 0.4,
            y: 0.5,
            radius: 0.1,
            feather: 50.0,
            flow: 80.0,
            erase: false,
        });
    }
    let mut ranged = Mask::radial();
    ranged.range = Some(Range::luminance());
    s.masks = vec![Mask::linear(), ranged, brush, Mask::sky()];
    let mut traced = Spot::new(SpotKind::Heal, (0.3, 0.3), 0.05);
    traced.path = vec![(0.3, 0.3), (0.6, 0.4)];
    s.spots = vec![Spot::new(SpotKind::Remove, (0.5, 0.5), 0.1), traced];
    let mut warp = Warp::new(WarpKind::Push, (0.5, 0.5), 0.2, 50.0);
    warp.points.push((0.6, 0.6));
    s.liquify = vec![warp];
    s.tone_curve.point = vec![(0.0, 0.0), (128.0, 150.0), (255.0, 255.0)];
    s.hsl.saturation[5] = -30.0;
    s
}

#[test]
fn damaged_xmp_files_never_panic() {
    let text = xmp::write(&everything());
    assert_eq!(xmp::read(&text).unwrap().exposure, 0.7);
    let bytes = text.as_bytes();
    let mut tried = 0;
    let mut panics = Vec::new();
    let mut check = |what: String, candidate: String| {
        tried += 1;
        let read = std::panic::catch_unwind(|| {
            // Lus, les réglages sont dans leurs plages : les y ramener ne change rien, et
            // une valeur NaN (jamais égale à elle‑même) se verrait aussi.
            if let Ok(s) = xmp::read(&candidate) {
                assert!(s.clone().clamped() == s, "réglage hors plage ou NaN : {s:?}");
            }
            let _ = xmp::read_meta(&candidate);
            let _ = xmp::read_preset(&candidate);
        });
        if read.is_err() {
            panics.push(what);
        }
    };
    // Coupé à chaque endroit (tous les 7 octets).
    for cut in (0..bytes.len()).step_by(7) {
        check(
            format!("coupé à {cut}"),
            String::from_utf8_lossy(&bytes[..cut]).into_owned(),
        );
    }
    // Un caractère sur 37 remplacé, à partir de chaque décalage ; puis des chiffres
    // changés en signes et en lettres.
    for start in 0..37 {
        let scrambled: String = text
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i % 37 == start {
                    ['"', '<', '>', '-', '9', 'e', '='][i % 7]
                } else {
                    c
                }
            })
            .collect();
        check(format!("brouillé depuis {start}"), scrambled);
    }
    for (from, to) in [("0.", "-1e38"), ("0.", "nan"), ("1", "99999999999"), ("\"", "")] {
        check(format!("{from} → {to}"), text.replace(from, to));
    }
    assert!(panics.is_empty(), "{} paniques sur {tried} : {panics:#?}", panics.len());
}
