//! Le catalogue sur les vrais RAW du corpus (`scripts/fetch-corpus.sh`) : date de prise
//! de vue, boîtier, objectif et ISO lus dans les en‑têtes, sans décoder les pixels.

use std::path::Path;
use std::time::Instant;

use luma_library::Catalog;

#[test]
fn the_corpus_is_indexed_from_headers() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus");
    let Ok(dir) = dir.canonicalize() else { return };
    if !dir.join("_DSC0009.ARW").exists() {
        return;
    }
    let mut cat = Catalog::default();
    let t = Instant::now();
    let u = cat.add_folder(&dir, false);
    let first = t.elapsed();
    assert!(u.added >= 5, "{u:?}");

    let sony = cat.get(&dir.join("_DSC0009.ARW")).unwrap();
    assert_eq!(sony.date, "2018-03-13T16:38:13");
    assert_eq!(sony.camera(), "Sony ILCE-7M3");
    assert_eq!(sony.iso, Some(400));
    assert!(sony.lens.contains("16-35"), "{}", sony.lens);
    let fuji = cat.get(&dir.join("AFXT2720.RAF")).unwrap();
    assert_eq!(fuji.camera(), "Fujifilm X-T3");

    // Triées par date de prise de vue, et non par nom.
    let dates: Vec<&str> = cat.by_date().iter().map(|e| e.date.as_str()).collect();
    assert!(dates.windows(2).all(|w| w[0] <= w[1]), "{dates:?}");

    // La seconde fois, rien n'est relu.
    let t = Instant::now();
    assert_eq!(cat.add_folder(&dir, false).unchanged, u.added);
    let again = t.elapsed();
    println!("catalogue du corpus : {first:?} la première fois, {again:?} ensuite");
    assert!(again < first);
}
