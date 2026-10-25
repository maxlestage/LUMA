//! Le serveur : une `paquets::App` qui rend la page d'accueil, la photo du pipeline en
//! SVG, le plan du site et l'app installable (PWA). Les fichiers statiques viennent de
//! `public/` et le bundle du navigateur de `pkg/` (trouvés par Paquets dans le dossier
//! courant).

pub mod export;
pub mod pages;
pub mod photo;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use paquets::prelude::*;

static BASE: OnceLock<String> = OnceLock::new();

/// Le chemin sous lequel le site est publié (`BASE_PATH`, ex. `/LUMA` sur GitHub Pages),
/// sans `/` final. À appeler une fois, avant le premier rendu.
pub fn set_base(base: &str) {
    let _ = BASE.set(base.trim().trim_end_matches('/').to_owned());
}

pub fn base() -> &'static str {
    BASE.get().map_or("", String::as_str)
}

/// Un lien interne, derrière le chemin de base.
pub fn link(path: &str) -> String {
    format!("{}{path}", base())
}

/// Empreintes des fichiers statiques, pour `?v=` dans leurs liens : un navigateur
/// recharge le style ou les scènes dès qu'ils changent.
pub struct Versions {
    pub css: String,
    pub scenes: String,
}

pub struct Site {
    /// L'adresse publique, chemin de base compris (`SITE_URL`) ; `None` : déduite de
    /// chaque requête.
    pub site_url: Option<String>,
    pub versions: Versions,
}

/// Le dossier `site/` : `SITE_DIR`, le dossier courant s'il contient `public/`, sinon le
/// dossier du crate.
pub fn site_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("SITE_DIR") {
        return PathBuf::from(dir);
    }
    if Path::new("public").is_dir() {
        return PathBuf::from(".");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FNV-1a, suffisant pour distinguer deux versions d'un fichier.
pub fn version(bytes: &[u8]) -> String {
    let h = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{h:016x}")[..10].to_owned()
}

impl Site {
    pub fn load(site_url: Option<String>) -> Site {
        let dir = site_dir().join("public");
        let read = |file: &str| std::fs::read(dir.join(file)).unwrap_or_default();
        let scenes: Vec<u8> = [
            "js/scenes.js",
            "js/hero.js",
            "js/devices.js",
            "js/screen.js",
        ]
        .iter()
        .flat_map(|f| read(f))
        .collect();
        Site {
            site_url: site_url
                .map(|u| u.trim().trim_end_matches('/').to_owned())
                .filter(|u| !u.is_empty()),
            versions: Versions {
                css: version(&read("site.css")),
                scenes: version(&scenes),
            },
        }
    }

    /// L'adresse publique du site, avec le chemin de base.
    pub fn origin(&self, req: &Request) -> String {
        if let Some(url) = &self.site_url {
            return url.clone();
        }
        let header = |name: &str| {
            req.header(name)
                .map(|v| v.split(',').next().unwrap_or("").trim().to_owned())
        };
        let host = header("x-forwarded-host")
            .or_else(|| header("host"))
            .unwrap_or_default();
        // Un en-tête Host inattendu ne doit pas finir dans les balises de la page.
        if host.is_empty()
            || !host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':'))
        {
            return format!("http://localhost{}", base());
        }
        let proto = if header("x-forwarded-proto").as_deref() == Some("https") {
            "https"
        } else {
            "http"
        };
        format!("{proto}://{host}{}", base())
    }
}

fn svg(body: String) -> Response {
    Response::new()
        .header("content-type", "image/svg+xml")
        .header("cache-control", "public, max-age=86400")
        .send(body)
}

/// Le manifeste de la PWA : nom, icônes, couleurs, raccourcis et captures (pour la
/// fenêtre d'installation).
fn manifest() -> String {
    let icon = |src: &str, size: &str, kind: &str, purpose: &str| {
        format!(r#"{{"src":"{src}","sizes":"{size}","type":"{kind}","purpose":"{purpose}"}}"#)
    };
    let shortcut = |name: &str, short: &str, url: &str, about: &str| {
        format!(
            r#"{{"name":"{name}","short_name":"{short}","url":"{url}","description":"{about}","icons":[{{"src":"/assets/icon-192.png","sizes":"192x192","type":"image/png"}}]}}"#
        )
    };
    format!(
        r##"{{"id":"/","name":"LÜMA — l'éditeur photo","short_name":"LÜMA","description":"{desc}","lang":"fr","dir":"ltr","start_url":"/?source=pwa","scope":"/","display":"standalone","display_override":["window-controls-overlay","standalone"],"orientation":"any","theme_color":"#050507","background_color":"#050507","categories":["photo","productivity","graphics"],"icons":[{icons}],"shortcuts":[{shortcuts}],"screenshots":[{shots}]}}"##,
        desc = crate::content::DESCRIPTION.replace('"', "'"),
        icons = [
            icon("/assets/icon-192.png", "192x192", "image/png", "any"),
            icon("/assets/icon-512.png", "512x512", "image/png", "any"),
            icon("/assets/icon-maskable-512.png", "512x512", "image/png", "maskable"),
            icon("/assets/favicon.svg", "any", "image/svg+xml", "any"),
        ]
        .join(","),
        shortcuts = [
            shortcut("Télécharger pour Mac", "Télécharger", "/#tarifs", "LÜMA pour Mac, 14 jours d'essai"),
            shortcut("Tarifs", "Tarifs", "/#tarifs", "LÜMA Premium, au mois ou à l'an"),
            shortcut("Le labo", "Labo", "/#labo", "Le moteur en direct, dans le navigateur"),
        ]
        .join(","),
        shots = [
            r#"{"src":"/assets/capture-large.jpg","sizes":"1440x900","type":"image/jpeg","form_factor":"wide","label":"LÜMA : l'éditeur photo qui développe la lumière"}"#,
            r#"{"src":"/assets/capture-mobile.jpg","sizes":"786x1704","type":"image/jpeg","form_factor":"narrow","label":"LÜMA sur iPhone"}"#,
        ]
        .join(","),
    )
}

/// Le service worker : les fichiers du site mis de côté pour l'usage hors ligne ; un
/// nouveau nom de cache à chaque version des styles ou des scripts.
fn service_worker(site: &Site) -> String {
    let mut precache = vec![
        "/".to_owned(),
        format!("/site.css?v={}", site.versions.css),
        format!("/js/scenes.js?v={}", site.versions.scenes),
    ];
    precache.extend(
        [
            "/js/hero.js",
            "/js/devices.js",
            "/js/screen.js",
            "/vendor/three.module.min.js",
            "/vendor/RoomEnvironment.js",
            "/vendor/RoundedBoxGeometry.js",
            "/pkg/luma_site.js",
            "/pkg/luma_site_bg.wasm",
            "/photo.svg",
            "/photo-mono.svg",
            "/assets/favicon.svg",
            "/assets/marque.svg",
            "/assets/icon-192.png",
            "/assets/icon-512.png",
            "/assets/apple-touch-icon.png",
        ]
        .map(String::from),
    );
    precache.extend(crate::content::LEGAL.iter().map(|l| l.path.to_owned()));
    // Seuls les fichiers présents : `public/…`, le bundle `pkg/…` (absent en
    // développement), les pages et les SVG servis par le serveur.
    precache.retain(|u| {
        let file = u.split('?').next().unwrap_or(u);
        let rel = file.trim_start_matches('/');
        file.ends_with('/')
            || matches!(file, "/photo.svg" | "/photo-mono.svg")
            || Path::new("public").join(rel).is_file()
            || (rel.starts_with("pkg/") && Path::new(rel).is_file())
    });
    let list = format!(
        "[{}]",
        precache
            .iter()
            .map(|u| format!("\"{u}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    include_str!("sw.js")
        .replace(
            "__VERSION__",
            &format!("{}-{}", site.versions.css, site.versions.scenes),
        )
        .replace("__PRECACHE__", &list)
}

/// L'application, prête à être lancée ou testée.
pub fn app(site: Arc<Site>) -> App {
    let mut app = App::new();
    let home = site.clone();
    app.get("/", move |req: Request| {
        let site = home.clone();
        async move {
            pages::home(&site, &site.origin(&req))
                .into_response()
                .header("cache-control", "no-cache")
        }
    });
    app.get("/photo.svg", |_req: Request| async {
        svg(photo::svg(false))
    });
    app.get("/photo-mono.svg", |_req: Request| async {
        svg(photo::svg(true))
    });
    let map = site.clone();
    app.get("/sitemap.xml", move |req: Request| {
        let site = map.clone();
        async move {
            Response::new()
                .header("content-type", "application/xml; charset=utf-8")
                .send(format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\"><url><loc>{0}/</loc></url>{1}</urlset>\n",
                    site.origin(&req),
                    crate::content::LEGAL
                        .iter()
                        .map(|l| format!("<url><loc>{}{}</loc></url>", site.origin(&req), l.path))
                        .collect::<String>()
                ))
        }
    });
    let robots = site.clone();
    app.get("/robots.txt", move |req: Request| {
        let site = robots.clone();
        async move {
            Response::new()
                .header("content-type", "text/plain; charset=utf-8")
                .send(format!(
                    "User-agent: *\nAllow: /\n\nSitemap: {}/sitemap.xml\n",
                    site.origin(&req)
                ))
        }
    });
    // La PWA : installable sur l'écran d'accueil de l'iPhone, de l'iPad et du Mac, et
    // lisible hors ligne.
    app.get("/manifest.webmanifest", |_req: Request| async {
        Response::new()
            .header("content-type", "application/manifest+json; charset=utf-8")
            .header("cache-control", "no-cache")
            .send(manifest())
    });
    let worker = site.clone();
    app.get("/sw.js", move |_req: Request| {
        let site = worker.clone();
        async move {
            Response::new()
                .header("content-type", "text/javascript; charset=utf-8")
                .header("cache-control", "no-cache")
                .send(service_worker(&site))
        }
    });
    // Les pages légales.
    for page in &crate::content::LEGAL {
        // Le routeur sert la même page avec ou sans la barre finale.
        {
            let site = site.clone();
            app.get(page.path, move |req: Request| {
                let site = site.clone();
                async move {
                    pages::legal(&site, &site.origin(&req), page)
                        .into_response()
                        .header("cache-control", "no-cache")
                }
            });
        }
    }
    let missing = site.clone();
    app.fallback(move |req: Request| {
        let site = missing.clone();
        async move { pages::not_found(&site, &site.origin(&req)) }
    });
    app
}

/// `cargo run` : le serveur, sur `PORT` (Heroku) ou 3000.
pub fn run() -> io::Result<()> {
    set_base(&std::env::var("BASE_PATH").unwrap_or_default());
    // Paquets sert `public/` et `pkg/` depuis le dossier courant.
    let dir = site_dir();
    if dir != Path::new(".") {
        std::env::set_current_dir(&dir)?;
    }
    let site = Arc::new(Site::load(std::env::var("SITE_URL").ok()));
    app(site).run()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn client() -> paquets::test::TestClient {
        let mut app = app(Arc::new(Site::load(None)));
        app.security(None);
        app.test().await
    }

    #[tokio::test]
    async fn the_home_page_is_rendered_by_the_server() {
        let client = client().await;
        let res = client.get("/").await;
        assert_eq!(res.status().as_u16(), 200);
        let html = res.text();
        assert!(html.contains(r#"<html lang="fr">"#));
        assert!(html.contains("<title>LÜMA — La lumière, développée en Rust</title>"));
        assert!(html.contains(r#"data-island="lab""#));
        assert!(html.contains(r#"data-island="crates""#));
        assert!(
            html.contains("Faisabilité"),
            "la feuille de route est dans le HTML"
        );
        assert!(html.contains("cargo deny --target aarch64-apple-ios check"));
        assert!(html.contains("/js/scenes.js?v="));
        assert!(html.contains(r#"id="tarifs""#));
        assert!(html.contains("79,99 €") && html.contains("9,99 €"));
        assert!(html.contains("/download/LUMA-macos.zip"));
        assert!(
            html.contains(r#"<span class="brand-mark"><svg"#),
            "le logo de la barre"
        );
    }

    #[tokio::test]
    async fn the_site_is_an_installable_pwa_with_its_legal_pages() {
        let client = client().await;
        let res = client.get("/manifest.webmanifest").await;
        assert_eq!(res.status().as_u16(), 200);
        let m = res.text();
        assert!(m.starts_with('{') && m.ends_with('}'));
        assert!(m.contains(r#""display":"standalone""#));
        assert_eq!(
            m.matches(r#""short_name""#).count(),
            4,
            "le nom et trois raccourcis"
        );
        assert_eq!(m.matches(r#""form_factor""#).count(), 2, "deux captures");
        let sw = client.get("/sw.js").await.text();
        assert!(sw.contains("const PRECACHE = [\"/\"") && sw.contains("/site.css?v="));
        assert!(!sw.contains("__"), "gabarit rempli");
        let home = client.get("/").await.text();
        assert!(home.contains(r#"rel="manifest""#) && home.contains("serviceWorker"));
        assert!(home.contains("footer-cols") && home.contains("Mentions légales"));
        for path in ["/mentions-legales/", "/confidentialite/", "/conditions/"] {
            let res = client.get(path).await;
            assert_eq!(res.status().as_u16(), 200, "{path}");
            assert!(res.text().contains(r#"class="legal""#), "{path}");
        }
    }

    #[tokio::test]
    async fn the_photo_is_an_svg() {
        let client = client().await;
        let res = client.get("/photo.svg").await;
        assert_eq!(res.status().as_u16(), 200);
        assert!(res.text().starts_with("<svg"));
    }

    #[tokio::test]
    async fn unknown_pages_are_404() {
        let client = client().await;
        let res = client.get("/nulle-part").await;
        assert_eq!(res.status().as_u16(), 404);
        assert!(res.text().contains("Hors "));
    }

    #[test]
    fn versions_change_with_the_content() {
        assert_eq!(version(b"a").len(), 10);
        assert_ne!(version(b"a"), version(b"b"));
    }
}
