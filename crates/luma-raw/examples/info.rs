//! Le temps de lecture de l'en‑tête de chaque photo (`luma_raw::info`), comme le fait la
//! photothèque quand elle ajoute un dossier, et ce qui est lu sur le disque.
//!
//! ```sh
//! cargo run --release -p luma-raw --example info -- tests/raw-corpus/*
//! ```
//!
//! Une seule lecture par photo : vider le cache du système avant (sous Linux,
//! `echo 3 > /proc/sys/vm/drop_caches`) mesure une lecture à froid, comme celle d'un
//! disque qu'on vient de brancher.

use std::time::Instant;

/// Les octets lus sur le disque par ce processus (Linux), pour savoir ce qu'une lecture
/// coûte vraiment ; 0 ailleurs.
fn disk_read() -> u64 {
    std::fs::read_to_string("/proc/self/io")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("read_bytes: "))
                .and_then(|v| v.trim().parse().ok())
        })
        .unwrap_or(0)
}

fn main() {
    let (mut total, mut bytes) = (0.0, 0);
    for path in std::env::args().skip(1) {
        let before = disk_read();
        let t = Instant::now();
        let found = luma_raw::info(&path);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let read = disk_read() - before;
        total += ms;
        bytes += read;
        let camera = found
            .map(|i| format!("{} {}", i.make, i.model))
            .unwrap_or_else(|| "—".into());
        println!("{ms:7.2} ms  {:6.1} Mo lus  {camera}  {path}", read as f64 / 1e6);
    }
    println!("{total:7.2} ms en tout, {:.1} Mo lus sur le disque", bytes as f64 / 1e6);
}
