//! Des fichiers abîmés (copie interrompue, carte défaillante) : à l'ouverture puis au
//! rendu, une image ou une erreur, jamais une panique — sinon l'app attendrait la photo
//! sans fin.

use std::path::Path;

use luma_core::Cancel;
use luma_project::Settings;
use luma_render::{Photo, Size};

#[test]
fn truncated_or_damaged_files_never_panic() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let tmp = std::env::temp_dir().join(format!("luma-abimes-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut panics = Vec::new();
    let mut tried = 0;
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if ext.eq_ignore_ascii_case("md") {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let n = bytes.len();
        // Coupé tôt, au milieu, presque à la fin ; des octets brouillés ; des zéros
        // après l'en‑tête ; vide.
        let scrambled: Vec<u8> = bytes
            .iter()
            .enumerate()
            .map(|(i, b)| if i > 512 && i % 997 == 0 { b ^ 0x5A } else { *b })
            .collect();
        let zeroed: Vec<u8> = bytes.iter().enumerate().map(|(i, b)| if i < 4096 { *b } else { 0 }).collect();
        let variants: [(&str, &[u8]); 6] = [
            ("coupé à 10 %", &bytes[..n / 10]),
            ("coupé à 50 %", &bytes[..n / 2]),
            ("coupé à 90 %", &bytes[..n * 9 / 10]),
            ("brouillé", &scrambled),
            ("mis à zéro", &zeroed),
            ("vide", &[]),
        ];
        for (what, data) in variants {
            let file = tmp.join(format!("abime.{ext}"));
            std::fs::write(&file, data).unwrap();
            tried += 1;
            let opened = std::panic::catch_unwind(|| {
                if let Ok(photo) = Photo::open(&file) {
                    let _ = photo.render(&Settings::default(), Size::Fit(256), &Cancel::new());
                }
            });
            if opened.is_err() {
                panics.push(format!("{} {what}", path.display()));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(panics.is_empty(), "{} paniques sur {tried} : {panics:#?}", panics.len());
}
