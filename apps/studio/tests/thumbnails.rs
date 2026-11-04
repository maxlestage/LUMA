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
    let start = Instant::now();
    let got = thumbnails(&worker, 1);
    eprintln!("première vignette après {:?} : {got:?}", start.elapsed());
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

/// À l'ouverture, un coup d'œil (l'aperçu intégré au RAW) arrive avant la fin du
/// décodage ; pour une photo retouchée, c'est sa vignette (avec ses retouches) ou rien.
#[test]
fn a_glimpse_comes_before_the_raw_is_decoded() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let dir = std::env::temp_dir().join(format!("luma-coup-d-oeil-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let worker = Worker::new(|| {});
    for name in [
        "_DSC0009.ARW",
        "Canon_EOS_R5_CRAW_ISO_100_nocrop_dual.CR3",
        "DSC_0750.NEF",
    ] {
        let src = corpus.join(name);
        if !src.exists() {
            continue;
        }
        let raw = dir.join(name);
        std::fs::copy(&src, &raw).unwrap();
        let start = Instant::now();
        worker.open(raw.clone());
        let (mut glimpse, mut loaded) = (None, None);
        while loaded.is_none() && start.elapsed() < Duration::from_secs(60) {
            for e in worker.poll() {
                match e {
                    Event::Glimpse { size, .. } if glimpse.is_none() => {
                        assert!(
                            (1000..=2048).contains(&size[0].max(size[1])),
                            "{name} : {size:?}"
                        );
                        glimpse = Some(start.elapsed());
                    }
                    Event::Loaded { .. } => loaded = Some(start.elapsed()),
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let (g, l) = (glimpse.expect("coup d'œil"), loaded.expect("photo ouverte"));
        eprintln!("{name} : coup d'œil en {g:?}, photo ouverte en {l:?}");
        assert!(g < l, "{name} : le coup d'œil arrive avant la photo");

        // Retouchée, sans vignette enregistrée : pas de coup d'œil non retouché.
        let bw = luma_project::presets::find("Tri-X 400")
            .unwrap()
            .applied(&luma_project::Settings::default(), 1.0);
        let copy = dir.join(format!("retouchée-{name}"));
        std::fs::copy(&src, &copy).unwrap();
        std::fs::write(copy.with_extension("xmp"), luma_project::xmp::write(&bw)).unwrap();
        worker.open(copy);
        let start = Instant::now();
        let mut done = false;
        while !done && start.elapsed() < Duration::from_secs(60) {
            for e in worker.poll() {
                match e {
                    Event::Glimpse { rgba, .. } => {
                        assert!(
                            colourfulness(&rgba) < 2.0,
                            "{name} : coup d'œil retouché seulement"
                        )
                    }
                    Event::Loaded { .. } => done = true,
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Un fichier illisible ne bloque pas les vignettes : la photo demandée après lui a la
/// sienne.
#[test]
fn a_broken_file_does_not_stop_the_thumbnails() {
    let dir = std::env::temp_dir().join(format!("luma-vignette-cassee-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let broken = dir.join("cassée.ARW");
    std::fs::write(&broken, b"II*\0pas vraiment un RAW").unwrap();
    let good = dir.join("bonne.png");
    image::RgbImage::from_fn(64, 48, |x, _| image::Rgb([x as u8 * 4, 90, 200]))
        .save(&good)
        .unwrap();
    let worker = Worker::new(|| {});
    worker.thumbnail(broken);
    worker.thumbnail(good.clone());
    let start = Instant::now();
    let mut got = None;
    while got.is_none() && start.elapsed() < Duration::from_secs(30) {
        for e in worker.poll() {
            if let Event::Thumbnail { path, .. } = e
                && path == good
            {
                got = Some(path);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(got.is_some(), "la vignette de la bonne photo");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Les vignettes déjà enregistrées, demandées d'un coup (un dossier qui s'ouvre), sont
/// relues sur tous les cœurs : chacune arrive une fois, avec les octets de son rendu.
#[test]
fn saved_thumbnails_asked_at_once_arrive_each_once() {
    let dir = std::env::temp_dir().join(format!("luma-vignettes-relues-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let photos: Vec<std::path::PathBuf> = (0..24u32)
        .map(|k| {
            let p = dir.join(format!("p{k:02}.png"));
            image::RgbImage::from_fn(300, 200, |x, y| {
                let v = ((x * (k + 3) + y * 7) % 200) as u8;
                image::Rgb([v, v / 2 + 40, 255 - v])
            })
            .save(&p)
            .unwrap();
            p
        })
        .collect();
    let collect = |worker: &Worker| {
        let mut got: Vec<(std::path::PathBuf, Vec<u8>)> = Vec::new();
        let start = Instant::now();
        while got.len() < photos.len() && start.elapsed() < Duration::from_secs(60) {
            for e in worker.poll() {
                if let Event::Thumbnail { path, rgba, .. } = e {
                    got.push((path, rgba));
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // Encore un instant : rien d'autre n'arrive (pas de seconde vignette).
        std::thread::sleep(Duration::from_millis(300));
        for e in worker.poll() {
            if let Event::Thumbnail { path, rgba, .. } = e {
                got.push((path, rgba));
            }
        }
        got
    };
    // Rendues et enregistrées une première fois.
    let first = Worker::new(|| {});
    for p in &photos {
        first.thumbnail(p.clone());
    }
    let rendered = collect(&first);
    assert_eq!(rendered.len(), photos.len());
    // Puis relues, toutes demandées d'un coup.
    let again = Worker::new(|| {});
    for p in &photos {
        again.thumbnail(p.clone());
    }
    let reread = collect(&again);
    assert_eq!(reread.len(), photos.len(), "une vignette chacune");
    for p in &photos {
        let a = rendered.iter().find(|(q, _)| q == p).map(|r| &r.1);
        let b = reread.iter().find(|(q, _)| q == p).map(|r| &r.1);
        assert!(a.is_some() && a == b, "{}", p.display());
    }
    let _ = std::fs::remove_dir_all(&dir);
}
