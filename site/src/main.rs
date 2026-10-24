//! `cargo run` : le site sur `http://localhost:3000` (`PORT` sur Heroku). À lancer depuis
//! `site/`, après avoir construit le bundle du navigateur (voir `README.md`).
//!
//! `cargo run -- export dist` : le site entier en fichiers statiques dans `dist/`.
//! `BASE_PATH` (ex. `/LUMA`) préfixe chaque lien, `SITE_URL` donne les adresses absolues.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("export") => {
            let dir = args.next().unwrap_or_else(|| "dist".into());
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            let files = runtime.block_on(luma_site::server::export::export(
                std::path::Path::new(&dir),
                &std::env::var("BASE_PATH").unwrap_or_default(),
                std::env::var("SITE_URL").ok(),
            ))?;
            println!("✨ {files} fichiers écrits dans {dir}");
            Ok(())
        }
        Some(other) => {
            eprintln!("commande inconnue {other:?} : lancer sans argument, ou `export <dossier>`");
            std::process::exit(2);
        }
        None => luma_site::server::run(),
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
