//! L'écriture d'un DNG linéaire : le résultat d'une fusion (empilement, panorama) reste
//! un vrai fichier RAW, qui se développe comme les originaux (balance des blancs, matrices de couleur et exposition de base conservées).

use std::io::BufWriter;
use std::path::Path;

use rawler::dng::writer::DngWriter;
use rawler::dng::{DNG_VERSION_V1_4, DngCompression};
use rawler::formats::tiff::value::{Rational, SRational};
use rawler::imgop::xyz::Illuminant;
use rawler::tags::{DngTag, ExifTag, TiffCommonTag};

use crate::{Error, RawPhoto, Result};

fn illuminant(temperature: f32) -> Illuminant {
    match temperature {
        t if t < 3500.0 => Illuminant::A,
        t if t < 5250.0 => Illuminant::D50,
        t if t < 6000.0 => Illuminant::D55,
        t if t < 7000.0 => Illuminant::D65,
        _ => Illuminant::D75,
    }
}

fn srational(v: f32) -> SRational {
    SRational::new((f64::from(v) * 1_000_000.0).round() as i32, 1_000_000)
}

/// Écrit `rgb` (couleurs caméra linéaires, 1 = blanc du capteur, `width × height`
/// pixels) dans un DNG, avec la couleur et la balance de `template` (la photo de
/// référence de la fusion).
pub fn write_linear_dng(
    path: &Path,
    width: usize,
    height: usize,
    rgb: &[[f32; 3]],
    template: &RawPhoto,
    software: &str,
) -> Result<()> {
    if rgb.len() != width * height || width == 0 {
        return Err(Error::Decode("image vide ou de mauvaise taille".into()));
    }
    let err = |e: rawler::formats::tiff::TiffError| Error::Decode(format!("écriture du DNG : {e}"));
    let data: Vec<u16> = rgb
        .iter()
        .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 65535.0).round() as u16))
        .collect();
    let file = std::fs::File::create(path).map_err(|e| Error::Decode(format!("{} : {e}", path.display())))?;
    let mut dng = DngWriter::new(BufWriter::new(file), DNG_VERSION_V1_4).map_err(err)?;
    let mut raw = dng.subframe_on_root(0);
    raw.rgb_image_u16(&data, width, height, DngCompression::Lossless, 1)
        .map_err(err)?;
    raw.finalize().map_err(err)?;

    let matrices = &template.matrices;
    let chosen: Vec<&(f32, crate::Mat3)> = match matrices.len() {
        0 => Vec::new(),
        1 => vec![&matrices[0]],
        n => vec![&matrices[0], &matrices[n - 1]],
    };
    for (slot, (t, m)) in chosen.into_iter().enumerate() {
        let values: Vec<SRational> = m.iter().flatten().map(|v| srational(*v)).collect();
        dng.color_matrix(slot + 1, illuminant(*t), values);
    }
    let neutral: Vec<Rational> = template
        .as_shot_neutral
        .iter()
        .map(|v| Rational {
            n: (f64::from(*v) * 1_000_000.0).round().max(1.0) as u32,
            d: 1_000_000,
        })
        .collect();
    dng.as_shot_neutral(neutral);
    let root = dng.root_ifd_mut();
    root.add_tag(TiffCommonTag::Make, template.make.as_str());
    root.add_tag(TiffCommonTag::Model, template.model.as_str());
    root.add_tag(
        DngTag::UniqueCameraModel,
        format!("{} {}", template.make, template.model).as_str(),
    );
    root.add_tag(DngTag::BaselineExposure, srational(template.baseline_exposure));
    root.add_tag(ExifTag::Orientation, 1u16);
    root.add_tag(TiffCommonTag::Software, software);
    dng.close().map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Exif, Mosaic, Orientation};

    #[test]
    fn a_written_dng_reads_back_identically() {
        let m = [[0.7, 0.1, -0.05], [-0.2, 1.1, 0.1], [0.05, -0.1, 0.9]];
        let template = RawPhoto {
            make: "LÜMA".into(),
            model: "Fusion".into(),
            width: 0,
            height: 0,
            mosaic: Mosaic::Linear,
            data: Vec::new(),
            as_shot_neutral: [0.5, 1.0, 0.7],
            matrices: vec![(2856.0, m), (6504.0, m)],
            orientation: Orientation::Normal,
            baseline_exposure: 0.5,
            exif: Exif::default(),
            rendered: false,
            opcodes: None,
        };
        let (w, h) = (40, 30);
        let rgb: Vec<[f32; 3]> = (0..w * h)
            .map(|i| [(i % w) as f32 / w as f32, (i / w) as f32 / h as f32, 0.25])
            .collect();
        let dir = std::env::temp_dir().join("luma-dng-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fusion.dng");
        write_linear_dng(&path, w, h, &rgb, &template, "LÜMA test").unwrap();
        let back = RawPhoto::open(&path).unwrap();
        assert_eq!((back.width, back.height), (w, h));
        assert!(matches!(back.mosaic, Mosaic::Linear));
        for (a, b) in back.data.chunks(3).zip(&rgb) {
            for c in 0..3 {
                assert!((a[c] - b[c]).abs() < 2e-4, "{a:?} / {b:?}");
            }
        }
        for c in 0..3 {
            assert!((back.as_shot_neutral[c] - template.as_shot_neutral[c]).abs() < 1e-4);
        }
        assert!((back.baseline_exposure - 0.5).abs() < 1e-4);
        assert_eq!(back.matrices.len(), 2);
        assert!((back.matrices[0].1[1][1] - 1.1).abs() < 1e-4);
    }
}
