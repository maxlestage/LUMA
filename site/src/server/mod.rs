//! Le serveur : une `paquets::App` qui rend la page d'accueil, la photo du pipeline en
//! SVG, le plan du site et l'app installable (PWA). Les fichiers statiques viennent de
//! `public/` et le bundle du navigateur de `pkg/` (trouvés par Paquets dans le dossier
//! courant).

pub mod docs;
pub mod export;
pub mod pages;
pub mod photo;
pub mod stripe;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use paquets::prelude::*;

use crate::i18n::{self, Lang, t};

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
    /// Le bundle WebAssembly (`pkg/`), dont les adresses ne portent pas de version : son
    /// empreinte entre dans le nom du cache du service worker.
    pub pkg: String,
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
        // Paquets sert `pkg/` depuis le dossier courant.
        let pkg: Vec<u8> = ["pkg/luma_site.js", "pkg/luma_site_bg.wasm"]
            .iter()
            .flat_map(|f| std::fs::read(f).unwrap_or_default())
            .collect();
        Site {
            site_url: site_url
                .map(|u| u.trim().trim_end_matches('/').to_owned())
                .filter(|u| !u.is_empty()),
            versions: Versions {
                css: version(&read("site.css")),
                scenes: version(&scenes),
                pkg: version(&pkg),
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

/// Le manifeste de la PWA dans `lang` : nom, icônes, couleurs, raccourcis et captures
/// (pour la fenêtre d'installation). L'app installée s'ouvre dans sa langue.
fn manifest(lang: Lang) -> String {
    let json = |s: &str| s.replace('"', "'");
    let icon = |src: &str, size: &str, kind: &str, purpose: &str| {
        format!(r#"{{"src":"{src}","sizes":"{size}","type":"{kind}","purpose":"{purpose}"}}"#)
    };
    let shortcut = |name: &str, short: &str, url: &str, about: &str| {
        format!(
            r#"{{"name":"{}","short_name":"{}","url":"{url}","description":"{}","icons":[{{"src":"/assets/icon-192.png","sizes":"192x192","type":"image/png"}}]}}"#,
            json(name),
            json(short),
            json(about),
        )
    };
    let home = i18n::path(lang, "/");
    format!(
        r##"{{"id":"{home}","name":"{name}","short_name":"LÜMA","description":"{desc}","lang":"{code}","dir":"ltr","start_url":"{home}?source=pwa","scope":"/","display":"standalone","display_override":["window-controls-overlay","standalone"],"orientation":"any","theme_color":"#050507","background_color":"#050507","categories":["photo","productivity","graphics"],"icons":[{icons}],"shortcuts":[{shortcuts}],"screenshots":[{shots}]}}"##,
        name = json(t(lang, "LÜMA — l'éditeur photo")),
        desc = json(t(lang, crate::content::DESCRIPTION)),
        code = lang.code(),
        icons = [
            icon("/assets/icon-192.png", "192x192", "image/png", "any"),
            icon("/assets/icon-512.png", "512x512", "image/png", "any"),
            icon("/assets/icon-maskable-512.png", "512x512", "image/png", "maskable"),
            icon("/assets/favicon.svg", "any", "image/svg+xml", "any"),
        ]
        .join(","),
        shortcuts = pages::downloads_open()
            .then(|| {
                shortcut(
                    t(lang, "Télécharger pour Mac"),
                    t(lang, "Télécharger"),
                    &format!("{home}#tarifs"),
                    t(lang, "LÜMA pour Mac, 14 jours d'essai"),
                )
            })
            .into_iter()
            .chain([
            shortcut(
                t(lang, "Tarifs"),
                t(lang, "Tarifs"),
                &format!("{home}#tarifs"),
                t(lang, "LÜMA Premium, au mois ou à l'an"),
            ),
            shortcut(
                t(lang, "Le labo"),
                t(lang, "Labo"),
                &format!("{home}#labo"),
                t(lang, "Le moteur en direct, dans le navigateur"),
            ),
        ])
        .collect::<Vec<_>>()
        .join(","),
        shots = [
            format!(r#"{{"src":"/assets/capture-large.jpg","sizes":"1440x900","type":"image/jpeg","form_factor":"wide","label":"{}"}}"#, json(t(lang, "LÜMA : l'éditeur photo qui développe la lumière"))),
            format!(r#"{{"src":"/assets/capture-mobile.jpg","sizes":"786x1704","type":"image/jpeg","form_factor":"narrow","label":"{}"}}"#, json(t(lang, "LÜMA sur iPhone"))),
        ]
        .join(","),
    )
}

/// Le service worker : les fichiers du site mis de côté pour l'usage hors ligne ; un
/// nouveau nom de cache à chaque version des styles, des scripts ou du WebAssembly.
fn service_worker(site: &Site) -> String {
    let mut precache = vec![
        "/".to_owned(),
        format!("/site.css?v={}", site.versions.css),
        format!("/js/scenes.js?v={}", site.versions.scenes),
        // Les modules chargés par scenes.js portent la même version.
        format!("/js/hero.js?v={}", site.versions.scenes),
        format!("/js/devices.js?v={}", site.versions.scenes),
        format!("/js/screen.js?v={}", site.versions.scenes),
    ];
    precache.extend(
        [
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
    // L'accueil et les pages légales dans chaque langue, la documentation en français.
    for lang in Lang::ALL {
        if lang != Lang::Fr {
            precache.push(i18n::path(lang, "/"));
        }
        precache.extend(
            crate::content::LEGAL
                .iter()
                .map(|l| i18n::path(lang, l.path)),
        );
    }
    precache.extend(docs::DOCS.iter().map(|d| d.path.to_owned()));
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
            &format!(
                "{}-{}-{}",
                site.versions.css, site.versions.scenes, site.versions.pkg
            ),
        )
        .replace("__PRECACHE__", &list)
}

/// Décode `%XX` (formulaire).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Les pages publiques du site, sans préfixe de langue : l'accueil, les pages légales et
/// la documentation (pour le plan du site).
pub fn public_pages() -> Vec<&'static str> {
    std::iter::once("/")
        .chain(crate::content::LEGAL.iter().map(|l| l.path))
        .chain(docs::DOCS.iter().map(|d| d.path))
        .collect()
}

/// Le plan du site : chaque page dans chaque langue, avec ses traductions.
fn sitemap(origin: &str) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">",
    );
    for page in public_pages() {
        let alternates: String = Lang::ALL
            .iter()
            .map(|l| (l.code(), i18n::path(*l, page)))
            .chain([("x-default", page.to_owned())])
            .map(|(code, p)| {
                format!(r#"<xhtml:link rel="alternate" hreflang="{code}" href="{origin}{p}"/>"#)
            })
            .collect();
        for lang in Lang::ALL {
            xml.push_str(&format!(
                "<url><loc>{origin}{}</loc>{alternates}</url>",
                i18n::path(lang, page)
            ));
        }
    }
    xml.push_str("</urlset>\n");
    xml
}

/// La page de la clé de licence (`/licence/`, `/en/licence/`…), dans `lang`.
fn licence_page(site: &Site, req: &Request, lang: Lang) -> Response {
    let session = req
        .query("session_id")
        .map(|s| s.into_owned())
        .unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let result = match (stripe::Stripe::from_env(), stripe::signing_key()) {
        (Some(api), Some(seed)) => stripe::key_for_session(&api, &seed, &session, now),
        _ => Err("le paiement en ligne n'est pas encore ouvert".into()),
    };
    // Le sélecteur de langue garde la session : la clé s'affiche dans l'autre langue.
    let here = if !session.is_empty()
        && session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        format!("/licence/?session_id={session}")
    } else {
        "/licence/".to_owned()
    };
    pages::licence(
        site,
        &site.origin(req),
        pages::Here::new(lang, &here),
        result,
        stripe::billing_portal(),
    )
    .into_response()
    .header("cache-control", "no-store")
    .header("vary", "accept-language")
}

/// L'application, prête à être lancée ou testée. Chaque page existe en français (aux
/// adresses d'origine), en anglais (`/en/…`) et en espagnol (`/es/…`).
pub fn app(site: Arc<Site>) -> App {
    let mut app = App::new();
    for lang in Lang::ALL {
        let home = site.clone();
        app.get(&i18n::path(lang, "/"), move |req: Request| {
            let site = home.clone();
            async move {
                pages::home(&site, &site.origin(&req), lang)
                    .into_response()
                    .header("cache-control", "no-cache")
            }
        });
        // La PWA : un manifeste par langue, pour que l'app installée s'ouvre dans la
        // sienne.
        app.get(
            &i18n::path(lang, "/manifest.webmanifest"),
            move |_req: Request| async move {
                Response::new()
                    .header("content-type", "application/manifest+json; charset=utf-8")
                    .header("cache-control", "no-cache")
                    .send(manifest(lang))
            },
        );
        // La documentation (docs/*.md du dépôt, qui peut rester privé).
        for doc in &docs::DOCS {
            let site = site.clone();
            app.get(&i18n::path(lang, doc.path), move |req: Request| {
                let site = site.clone();
                async move {
                    pages::doc(&site, &site.origin(&req), lang, doc)
                        .into_response()
                        .header("cache-control", "no-cache")
                }
            });
        }
        // Les pages légales (le routeur sert la même page avec ou sans la barre finale).
        for page in &crate::content::LEGAL {
            let site = site.clone();
            app.get(&i18n::path(lang, page.path), move |req: Request| {
                let site = site.clone();
                async move {
                    pages::legal(&site, &site.origin(&req), lang, page)
                        .into_response()
                        .header("cache-control", "no-cache")
                }
            });
        }
        // LÜMA Premium : la page de la clé. Stripe renvoie toujours vers `/licence/`,
        // sans langue : elle est alors choisie d'après `Accept-Language`, sans
        // redirection (la session reste dans l'adresse).
        let premium = site.clone();
        app.get(&i18n::path(lang, "/licence/"), move |req: Request| {
            let site = premium.clone();
            async move {
                let lang = match lang {
                    Lang::Fr => i18n::negotiate(req.header("accept-language")),
                    other => other,
                };
                licence_page(&site, &req, lang)
            }
        });
    }
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
                .send(sitemap(&site.origin(&req)))
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
    // « Gérer mon abonnement » : le portail client Stripe (une adresse pour toutes les
    // langues).
    let portal = site.clone();
    app.get("/abonnement/", move |req: Request| {
        let site = portal.clone();
        async move {
            match stripe::billing_portal() {
                Some(url) => Response::new()
                    .status(302)
                    .header("location", url)
                    .header("cache-control", "no-store")
                    .send(""),
                None => pages::licence(
                    &site,
                    &site.origin(&req),
                    pages::Here::new(i18n::negotiate(req.header("accept-language")), "/licence/"),
                    Err("le portail client n'est pas encore ouvert".into()),
                    None,
                )
                .into_response()
                .header("vary", "accept-language"),
            }
        }
    });
    // La clé fraîche de l'app (sans langue : l'app lit le JSON).
    app.post("/api/licence", |req: Request| async move {
        let body = req.text().await.unwrap_or_default();
        let key = body
            .split('&')
            .find_map(|kv| kv.strip_prefix("key="))
            .map(|v| v.replace('+', " "))
            .map(|v| percent_decode(&v))
            .unwrap_or_default();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let (status, json) = match (stripe::Stripe::from_env(), stripe::signing_key()) {
            (Some(api), Some(seed)) => match stripe::refresh(&api, &seed, &key, now) {
                Ok(k) => (200, serde_json::json!({ "key": k }).to_string()),
                Err(e) => (402, serde_json::json!({ "error": e }).to_string()),
            },
            _ => (
                503,
                r#"{"error":"serveur de licences non configuré"}"#.to_owned(),
            ),
        };
        Response::new()
            .status(status)
            .header("content-type", "application/json; charset=utf-8")
            .header("cache-control", "no-store")
            .send(json)
    });
    // Introuvable : dans la langue de l'adresse demandée.
    let missing = site.clone();
    app.fallback(move |req: Request| {
        let site = missing.clone();
        async move {
            let (lang, _) = i18n::split(req.path());
            pages::not_found(&site, &site.origin(&req), lang)
        }
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
        assert!(
            html.contains("<title>LÜMA — L&#x27;éditeur photo qui développe la lumière</title>")
        );
        assert!(html.contains(r#"data-island="lab""#));
        // Les sections pour les photographes : étapes, films, vie privée, questions.
        for id in ["etapes", "films", "vie-privee", "questions"] {
            assert!(html.contains(&format!(r#"id="{id}""#)), "section {id}");
        }
        // Rien sur le code : ni Rust, ni crates, ni cargo.
        for code in ["Rust", "crate", "cargo", "wgpu", "WebAssembly"] {
            assert!(!html.contains(code), "le code sur l'accueil : {code}");
        }
        assert!(html.contains("/js/scenes.js?v="));
        assert!(html.contains(r#"id="tarifs""#));
        // Les prix : le montant (qui monte à l'écran) puis le symbole.
        assert!(
            html.contains(r#"data-sep=",">239,88</i> €"#)
                && html.contains(r#"data-sep=",">19,99</i> €"#)
        );
        // Le téléchargement est fermé tant que LUMA_DOWNLOADS ne vaut pas 1.
        assert!(!html.contains("/download/LUMA-macos.zip") && !html.contains("Télécharger"));
        assert!(!html.contains("/download/LUMA-linux"));
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
            3,
            "le nom et deux raccourcis (sans Télécharger, fermé)"
        );
        assert_eq!(m.matches(r#""form_factor""#).count(), 2, "deux captures");
        let sw = client.get("/sw.js").await.text();
        assert!(sw.contains("const PRECACHE = [\"/\"") && sw.contains("/site.css?v="));
        assert!(!sw.contains("__"), "gabarit rempli");
        let home = client.get("/").await.text();
        assert!(home.contains(r#"rel="manifest""#) && home.contains("serviceWorker"));
        assert!(
            !home.contains("github.com/maxlestage/LUMA"),
            "le dépôt est privé"
        );
        assert!(home.contains("footer-cols") && home.contains("Mentions légales"));
        assert!(
            home.contains("Maxime Nathan Lestage")
                && home.contains("footer-wordmark")
                && home.contains("github.com/maxlestage/Paquets")
        );
        for path in [
            "/mentions-legales/",
            "/confidentialite/",
            "/conditions/",
            "/docs/boitiers/",
            "/docs/feuille-de-route/",
            "/nouveautes/",
        ] {
            let res = client.get(path).await;
            assert_eq!(res.status().as_u16(), 200, "{path}");
            assert!(res.text().contains(r#"class="legal"#), "{path}");
            assert!(
                !res.text().contains("github.com/maxlestage/LUMA"),
                "{path} : dépôt privé"
            );
        }
    }

    #[tokio::test]
    async fn without_stripe_the_licence_pages_say_so() {
        let client = client().await;
        let res = client.get("/licence/?session_id=cs_test_1").await;
        assert_eq!(res.status().as_u16(), 200);
        assert!(res.text().contains("pas encore ouvert"));
        let res = client.post("/api/licence").body("key=LUMA1.abc").await;
        assert_eq!(res.status().as_u16(), 503);
        assert!(res.text().contains("error"));
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

    /// Chaque page publique, sans préfixe de langue, plus la page de la clé.
    fn every_page() -> Vec<&'static str> {
        let mut pages = public_pages();
        pages.push("/licence/");
        pages
    }

    #[tokio::test]
    async fn every_page_exists_in_every_language() {
        let client = client().await;
        for page in every_page() {
            for lang in Lang::ALL {
                let path = i18n::path(lang, page);
                let res = client.get(&path).await;
                assert_eq!(res.status().as_u16(), 200, "{path}");
                let html = res.text();
                assert!(
                    html.contains(&format!(r#"<html lang="{}">"#, lang.code())),
                    "{path} : <html lang>"
                );
                for l in Lang::ALL {
                    let alt = format!(
                        r#"<link rel="alternate" hreflang="{}" href="http://localhost{}"/>"#,
                        l.code(),
                        i18n::path(l, page)
                    );
                    assert!(html.contains(&alt), "{path} : {alt}");
                    // Le sélecteur de langue, dans la barre et le pied de page.
                    let switch =
                        format!(r#"href="{}" hreflang="{}""#, i18n::path(l, page), l.code());
                    assert!(html.matches(&switch).count() >= 2, "{path} : {switch}");
                }
                assert!(html.contains(&format!(
                    r#"<link rel="alternate" hreflang="x-default" href="http://localhost{page}"/>"#
                )));
                if page != "/licence/" {
                    assert!(
                        html.contains(&format!(
                            r#"<link rel="canonical" href="http://localhost{path}"/>"#
                        )),
                        "{path} : canonique"
                    );
                }
                assert!(html.contains(&format!(
                    r#"<meta property="og:locale" content="{}"/>"#,
                    i18n::locale(lang)
                )));
                assert!(html.contains(&format!(
                    r#"rel="manifest" href="{}""#,
                    i18n::path(lang, "/manifest.webmanifest")
                )));
                assert!(
                    !html.contains("github.com/maxlestage/LUMA"),
                    "{path} : dépôt privé"
                );
            }
        }
    }

    #[tokio::test]
    async fn the_english_and_spanish_home_pages_are_translated() {
        let client = client().await;
        let en = client.get("/en/").await.text();
        assert!(en.contains("<title>LÜMA — The photo editor that develops light</title>"));
        assert!(
            en.contains(r#"€<i data-count="19.99""#)
                && en.contains(">239.88</i>")
                && en.contains("per month")
        );
        assert!(en.contains(r#"data-island="lab" data-props="en""#));
        assert!(
            en.contains(r#"href="/en/#tarifs""#) && en.contains(r#"href="/en/docs/fonctions/""#)
        );
        assert!(en.contains(r#"href="/en/mentions-legales/""#));
        // Des textes français qui ne doivent pas rester sur la page anglaise.
        for french in [
            "Télécharger",
            "Feuille de route",
            "Tarifs",
            "La lumière",
            "Mentions légales",
            "Confidentialité",
            "Tous droits réservés",
            "Une belle photo",
            "Exposition",
            "Hautes lumières",
            "Bientôt",
            "par mois",
            "19,99 €",
            "Questions fréquentes",
            "Vos photos",
            "Pellicules couleur",
            "Aucun cookie",
        ] {
            assert!(!en.contains(french), "texte français sur /en/ : {french}");
        }
        let es = client.get("/es/").await.text();
        assert!(es.contains("<title>LÜMA — El editor de fotos que revela la luz</title>"));
        assert!(es.contains(">19,99</i> €") && es.contains("al mes"));
        assert!(es.contains(r#"data-props="es""#));
        for french in [
            "Télécharger",
            "Feuille de route",
            "Tarifs",
            "Mentions légales",
            "par mois",
        ] {
            assert!(!es.contains(french), "texte français sur /es/ : {french}");
        }
        // Le français reste aux adresses d'origine.
        let fr = client.get("/").await.text();
        assert!(fr.contains(r#"data-props="fr""#) && fr.contains(r#"href="/#tarifs""#));
    }

    #[tokio::test]
    async fn translated_legal_pages_say_the_french_version_prevails() {
        let client = client().await;
        let en = client.get("/en/conditions/").await.text();
        assert!(en.contains("Subscription terms") && en.contains("French version prevails"));
        assert!(en.contains("L221‑28") && en.contains("French Consumer Code"));
        assert!(en.contains(r#"href="/conditions/" hreflang="fr""#));
        let fr = client.get("/conditions/").await.text();
        assert!(!fr.contains("fait foi"));
        let es = client.get("/es/docs/boitiers/").await.text();
        assert!(es.contains("Cámaras compatibles") && es.contains("<th>Marca</th>"));
    }

    #[tokio::test]
    async fn the_licence_page_speaks_the_visitors_language() {
        let client = client().await;
        let page = |accept: Option<&'static str>| {
            let client = &client;
            async move {
                let mut req = client.get("/licence/?session_id=cs_test_1");
                if let Some(a) = accept {
                    req = req.header("accept-language", a);
                }
                let res = req.await;
                assert_eq!(res.status().as_u16(), 200);
                res.text()
            }
        };
        let fr = page(None).await;
        assert!(fr.contains(r#"<html lang="fr">"#) && fr.contains("pas encore ouvert"));
        let es = page(Some("es-ES,es;q=0.9,en;q=0.8")).await;
        assert!(es.contains(r#"<html lang="es">"#) && es.contains("aún no está disponible"));
        let en = page(Some("de-DE,de;q=0.9")).await;
        assert!(en.contains(r#"<html lang="en">"#) && en.contains("online payment isn"));
        assert!(page(Some("fr-CA")).await.contains(r#"<html lang="fr">"#));
        // La session reste dans les liens vers les autres langues.
        assert!(en.contains(r#"href="/es/licence/?session_id=cs_test_1""#));
        // Les adresses préfixées ne négocient pas.
        let res = client
            .get("/es/licence/")
            .header("accept-language", "en")
            .await;
        assert!(res.text().contains(r#"<html lang="es">"#));
    }

    #[tokio::test]
    async fn the_sitemap_and_the_pwa_know_every_language() {
        let client = client().await;
        let map = client.get("/sitemap.xml").await.text();
        assert!(map.contains("<loc>http://localhost/en/conditions/</loc>"));
        assert!(map.contains("<loc>http://localhost/es/docs/architecture/</loc>"));
        assert!(map.contains(r#"hreflang="x-default" href="http://localhost/nouveautes/""#));
        assert_eq!(
            map.matches("<url>").count(),
            public_pages().len() * Lang::ALL.len()
        );
        let en = client.get("/en/manifest.webmanifest").await.text();
        assert!(en.contains(r#""lang":"en""#) && en.contains(r#""start_url":"/en/?source=pwa""#));
        assert!(
            en.contains("Pricing") && !en.contains("Download for Mac") && !en.contains("Tarifs")
        );
        let sw = client.get("/sw.js").await.text();
        assert!(sw.contains("\"/en/\"") && sw.contains("\"/es/conditions/\""));
    }

    #[tokio::test]
    async fn unknown_pages_are_404_in_their_language() {
        let client = client().await;
        let res = client.get("/en/nowhere").await;
        assert_eq!(res.status().as_u16(), 404);
        let html = res.text();
        assert!(html.contains(r#"<html lang="en">"#) && html.contains("Out of "));
        let res = client.get("/es/ninguna-parte/").await;
        assert_eq!(res.status().as_u16(), 404);
        assert!(res.text().contains("Fuera de "));
    }

    #[test]
    fn versions_change_with_the_content() {
        assert_eq!(version(b"a").len(), 10);
        assert_ne!(version(b"a"), version(b"b"));
    }
}
