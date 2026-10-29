//! Les bases du décodeur chargées à l'avance : une seule fois, et sans rien changer à ce
//! qu'une photo ouverte ensuite donne.

#[test]
fn warming_up_twice_changes_nothing() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/raw-corpus/DSC_0750.NEF");
    if !path.is_file() {
        eprintln!("corpus absent : essai ignoré");
        return;
    }
    luma_raw::warm_up();
    luma_raw::warm_up();
    let photo = luma_raw::RawPhoto::open(&path).unwrap();
    // L'objectif vient de la base des objectifs : il est bien là.
    assert_eq!(photo.exif.lens.as_deref(), Some("Nikkor Z 24-70mm f/4 S"));
    assert_eq!((photo.width, photo.height), (6048, 4024));
}
