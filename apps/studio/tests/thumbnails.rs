//! Les miniatures montrent les retouches : une photo retouchée n'apparaît jamais sans
//! elles (pas d'aperçu intégré au RAW avant son rendu), et une nouvelle retouche
//! refait sa miniature.

use std::path::Path;
use std::time::{Duration, Instant};

use luma_studio::worker::{Event, Worker};

/// La part de couleur d'une miniature (écart moyen entre les canaux, 0 à 255).
fn colourfulness(rgba: &[u8]) -> f32 {
    let px: Vec<&[u8]> = rgba.chunks(4).collect();
    let sum: u32 = px
        .iter()
        .map(|p| {
            let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
            ((r - g).abs() + (g - b).abs() + (b - r).abs()) as u32
        })
        .sum();
    sum as f32 / px.len() as f32 / 3.0
}

/// Les miniatures reçues jusqu'à `n` (ou 30 s).
fn thumbnails(worker: &Worker, n: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let start = Instant::now();
    while out.len() < n && start.elapsed() < Duration::from_secs(30) {
        for e in worker.poll() {
            if let Event::Thumbnail { rgba, .. } = e {
                out.push(colourfulness(&rgba));
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    out
}

#[test]
fn thumbnails_show_the_edits() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus/_DSC0009.ARW");
    if !corpus.exists() {
        eprintln!("pas de corpus : test ignoré");
        return;
    }
    let dir = std::env::temp_dir().join(format!("luma-miniatures-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let raw = dir.join("photo.ARW");
    std::fs::copy(&corpus, &raw).unwrap();

    // Retouchée en noir et blanc : la seule miniature est grise, sans passage en
    // couleur par l'aperçu intégré.
    let bw = luma_project::presets::find("Tri-X 400")
        .unwrap()
        .applied(&luma_project::Settings::default(), 1.0);
    std::fs::write(raw.with_extension("xmp"), luma_project::xmp::write(&bw)).unwrap();
    let worker = Worker::new(|| {});
    worker.thumbnail(raw.clone());
    let got = thumbnails(&worker, 1);
    std::thread::sleep(Duration::from_millis(300));
    let extra: Vec<Event> = worker
        .poll()
        .into_iter()
        .filter(|e| matches!(e, Event::Thumbnail { .. }))
        .collect();
    assert_eq!(got.len() + extra.len(), 1, "une seule miniature : {got:?}");
    assert!(got[0] < 2.0, "noir et blanc : {}", got[0]);

    // La retouche retirée : la miniature refaite est en couleur.
    std::fs::remove_file(raw.with_extension("xmp")).unwrap();
    worker.thumbnail(raw.clone());
    let got = thumbnails(&worker, 2);
    assert!(got.last().is_some_and(|&c| c > 5.0), "couleur : {got:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
