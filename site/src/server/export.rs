//! `luma-site export <dossier>` : le site en fichiers statiques, pour GitHub Pages ou
//! n'importe quel hébergeur statique. Chaque fichier vient du serveur lui-même, par le
//! client de test en mémoire de Paquets (`app.test()`) : les deux sont identiques.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{Site, app, base, set_base, site_dir};

/// Écrit le site dans `out`, liens derrière `base_path` (`""` ou `/LUMA`), adresses
/// absolues sur `site_url`. Renvoie le nombre de fichiers écrits.
pub async fn export(out: &Path, base_path: &str, site_url: Option<String>) -> io::Result<usize> {
    set_base(base_path);
    let base = base();
    let out = std::path::absolute(out)?;
    // Paquets trouve `public/` et `pkg/` (le bundle) dans le dossier courant.
    let dir = std::path::absolute(site_dir())?;
    std::env::set_current_dir(&dir)?;
    if !dir.join("pkg").is_dir() {
        eprintln!(
            "attention : site/pkg manque, les animations et le labo ne marcheront pas (voir README.md)"
        );
    }
    let site_url = site_url
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| format!("https://maxlestage.github.io{base}"));
    let mut app = app(Arc::new(Site::load(Some(site_url))));
    // Un site statique n'a pas de serveur à protéger, et l'export demande tout d'un coup.
    app.security(None);
    let client = app.test().await;

    let mut written = 0;
    let mut write = |path: &str, bytes: &[u8]| -> io::Result<()> {
        let file = out.join(path.trim_start_matches('/'));
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, bytes)?;
        written += 1;
        Ok(())
    };
    let get = |path: &'static str, expected: u16| {
        let client = &client;
        async move {
            let res = client.get(path).await;
            if res.status().as_u16() != expected {
                return Err(io::Error::other(format!(
                    "GET {path} : {} au lieu de {expected}",
                    res.status()
                )));
            }
            Ok(res.bytes().to_vec())
        }
    };

    let text = |bytes: Vec<u8>| String::from_utf8(bytes).map_err(io::Error::other);
    write(
        "index.html",
        with_base(&text(get("/", 200).await?)?, base).as_bytes(),
    )?;
    write(
        "404.html",
        with_base(&text(get("/404/", 404).await?)?, base).as_bytes(),
    )?;
    // Les pages légales : un dossier chacune (`/conditions/` → conditions/index.html).
    for page in &crate::content::LEGAL {
        let html = with_base(&text(get(page.path, 200).await?)?, base);
        write(&format!("{}index.html", page.path), html.as_bytes())?;
    }
    for file in [
        "/photo.svg",
        "/photo-mono.svg",
        "/sitemap.xml",
        "/robots.txt",
    ] {
        write(file, &get(file, 200).await?)?;
    }
    // La PWA : le manifeste et le service worker contiennent des adresses absolues.
    for file in ["/manifest.webmanifest", "/sw.js"] {
        let body = text(get(file, 200).await?)?;
        write(file, with_base_everywhere(&body, base).as_bytes())?;
    }
    for folder in ["public", "pkg"] {
        let root = dir.join(folder);
        let prefix = if folder == "pkg" { "pkg/" } else { "" };
        for file in files(&root) {
            let relative = file.strip_prefix(&root).map_err(io::Error::other)?;
            let url = format!("{prefix}{}", relative.to_string_lossy().replace('\\', "/"));
            if url.ends_with(".gitignore") || url.ends_with("package.json") {
                continue;
            }
            write(&url, &std::fs::read(&file)?)?;
        }
    }
    // Les fichiers sont servis tels quels : pas de Jekyll sur GitHub Pages.
    write(".nojekyll", b"")?;
    Ok(written)
}

/// Les adresses que Paquets écrit lui-même (le chargeur des îlots, la PWA) ne
/// connaissent pas le chemin de base : elles le reçoivent ici. Les liens du site l'ont
/// déjà.
pub fn with_base(html: &str, base: &str) -> String {
    if base.is_empty() {
        return html.to_owned();
    }
    html.replace("\"/pkg/", &format!("\"{base}/pkg/"))
        .replace(
            "\"/manifest.webmanifest\"",
            &format!("\"{base}/manifest.webmanifest\""),
        )
        .replace("\"/sw.js\"", &format!("\"{base}/sw.js\""))
}

/// Chaque adresse absolue (`"/…"`) d'un fichier JSON ou JavaScript, derrière `base`.
pub fn with_base_everywhere(text: &str, base: &str) -> String {
    if base.is_empty() {
        return text.to_owned();
    }
    text.replace("\"/", &format!("\"{base}/"))
}

/// Chaque fichier sous `dir`, récursivement.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files(&path));
        } else {
            found.push(path);
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_the_islands_loader() {
        let html = r#"<link rel="modulepreload" href="/pkg/a.js"/><script type="module">import init from "/pkg/a.js";init({module_or_path:"/pkg/a_bg.wasm"});</script>"#;
        assert_eq!(with_base(html, "/LUMA").matches("\"/LUMA/pkg/").count(), 3);
        assert_eq!(with_base(html, ""), html);
    }
}
