//! L'en‑tête lu par `luma_raw::info` (le début du fichier seulement) dit exactement ce
//! que dit le décodage complet, sur chaque RAW du corpus (`scripts/fetch-corpus.sh`).

use std::path::Path;

#[test]
fn the_head_of_the_file_says_what_the_whole_file_says() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    let mut checked = 0;
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().is_some_and(|x| x == "md") {
            continue;
        }
        let Ok(full) = luma_raw::RawPhoto::open(&path) else {
            continue;
        };
        let info = luma_raw::info(&path).unwrap_or_else(|| panic!("{} : pas d'en‑tête", path.display()));
        assert_eq!(
            (info.make.as_str(), info.model.as_str()),
            (full.make.as_str(), full.model.as_str()),
            "{}",
            path.display()
        );
        assert_eq!(info.exif, full.exif, "{}", path.display());
        assert!(info.exif.date.is_some(), "{} : date de prise de vue", path.display());
        checked += 1;
    }
    println!("{checked} RAW vérifiés");
}

/// Le Sony A7 III du corpus range ses corrections d'objectif (FE 16-35 mm GM) dans son
/// ARW : elles sont lues et appliquées.
#[test]
fn the_sony_lens_corrections_are_read_from_the_arw() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus/_DSC0009.ARW");
    let Ok(photo) = luma_raw::RawPhoto::open(&path) else {
        return;
    };
    let Some((ops, _)) = &photo.opcodes else {
        panic!("aucune correction lue")
    };
    let [
        luma_raw::opcodes::Opcode::Spline {
            knots,
            distortion,
            vignetting,
            zoom,
        },
    ] = ops.as_slice()
    else {
        panic!("{ops:?}")
    };
    assert_eq!(knots.len(), 16);
    // Aucun effet au centre ; au coin, l'objectif perd de la lumière et pousse les points
    // un peu vers l'extérieur.
    assert!((vignetting[0] - 1.0).abs() < 1e-3, "{vignetting:?}");
    assert!(vignetting[15] < 0.9 && vignetting[15] > 0.6, "{vignetting:?}");
    assert!(distortion[1][15] > 1.0 && distortion[1][15] < 1.05, "{distortion:?}");
    assert!(*zoom >= 1.0 && *zoom < 1.05, "{zoom}");
    // Le rouge et le bleu se décalent un peu par rapport au vert (aberration latérale).
    assert_ne!(distortion[0], distortion[1]);
    // Les coins de l'image corrigée sont plus clairs qu'avant la correction.
    let corrected = photo.binned();
    let mut plain = photo.clone();
    plain.opcodes = None;
    let raw = plain.binned();
    let corner = |img: &luma_core::RgbImage| {
        let n = 20;
        let mut sum = 0.0;
        for y in 0..n {
            for x in 0..n {
                sum += img.data[y * img.width + x][1];
            }
        }
        sum / (n * n) as f32
    };
    assert!(
        corner(&corrected) > corner(&raw) * 1.1,
        "{} {}",
        corner(&corrected),
        corner(&raw)
    );
}
