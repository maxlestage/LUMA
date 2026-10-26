//! Le catalogue à grande échelle : 100 000 photos synthétiques (dossiers par jour,
//! quatre boîtiers, mots‑clés), enregistrées puis relues, et une recherche.
//!
//! ```sh
//! cargo run --release -p luma-library --example catalog [-- nombre]
//! ```

use std::time::Instant;

use luma_library::{Catalog, Entry, Query};

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(100_000);
    let cameras = [
        ("Sony", "ILCE-7M3"),
        ("Canon", "EOS R5"),
        ("Nikon", "Z 6"),
        ("Fujifilm", "X-T3"),
    ];
    let lenses = [
        "FE 24-70mm F2.8 GM",
        "RF 50mm F1.2 L USM",
        "Nikkor Z 24-70mm f/4 S",
        "XF35mmF2 R WR",
    ];
    let keywords = ["Mer", "Été", "Montagne", "Portrait", "Mariage", "Ville", "Nuit"];
    let mut cat = Catalog::default();
    for i in 0..n {
        // Un générateur pseudo‑aléatoire simple et reproductible.
        let r = (i as u64)
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407)
            >> 33;
        let (make, model) = cameras[(r % 4) as usize];
        let (year, month, day) = (2015 + i % 10, 1 + i / 300 % 12, 1 + i / 30 % 28);
        cat.insert(Entry {
            path: format!("/Users/photographe/Photos/{year}/{year}-{month:02}-{day:02}/IMG_{i:06}.ARW").into(),
            size: 25_000_000 + r % 10_000_000,
            modified: 1_600_000_000 + i as u64,
            date: format!("{year}-{month:02}-{day:02}T{:02}:{:02}:00", i % 24, i % 60),
            make: make.into(),
            model: model.into(),
            lens: lenses[(r % 4) as usize].into(),
            iso: Some([100, 200, 400, 800, 1600, 3200][(r % 6) as usize]),
            aperture: Some(2.8),
            exposure_time: Some(1.0 / 250.0),
            focal_length: Some([24.0, 35.0, 50.0, 70.0][(r / 7 % 4) as usize]),
            rating: (r % 6) as u8,
            flag: [0, 0, 1, -1][(r / 3 % 4) as usize],
            keywords: vec![
                keywords[(r % 7) as usize].into(),
                keywords[(r / 11 % 7) as usize].into(),
            ],
            ..Entry::default()
        });
    }
    let file = std::env::temp_dir().join("luma-bench-catalogue");
    let t = Instant::now();
    cat.save(&file).unwrap();
    let saved = t.elapsed();
    let size = std::fs::metadata(&file).unwrap().len();
    let mut loads = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        let back = Catalog::load(&file);
        loads.push(t.elapsed());
        assert_eq!(back.len(), n);
    }
    loads.sort();
    let q = Query::parse("mer note:4").unwrap();
    let t = Instant::now();
    let found = cat.search(&q).len();
    println!(
        "{n} photos : index de {:.1} Mo, enregistré en {:.0} ms, relu en {:.0} ms (médiane de 5), recherche en {:.1} ms ({found} trouvées)",
        size as f64 / 1e6,
        saved.as_secs_f64() * 1e3,
        loads[2].as_secs_f64() * 1e3,
        t.elapsed().as_secs_f64() * 1e3,
    );
    let _ = std::fs::remove_file(&file);
}
