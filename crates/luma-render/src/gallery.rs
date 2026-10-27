//! La galerie web : un dossier statique, à mettre en ligne tel quel (n'importe quel
//! hébergement de fichiers suffit) ou à ouvrir dans un navigateur. `index.html` montre
//! la grille des vignettes ; un clic ouvre la photo en grand, avec les flèches, un
//! glissement du doigt et Échap. Toutes les images sont des JPEG allégés pour le web
//! ([`crate::jpeg::encode_web`]), les vignettes ne se chargent qu'en arrivant à l'écran.
//! La page est écrite par LÜMA, sans bibliothèque, et n'affiche aucun texte de
//! commande : seulement le titre, la description et, si on le veut, les légendes.

use std::path::Path;

use luma_color::OutputSpace;
use luma_core::RgbImage;

use crate::slideshow::{escape, js};

/// Les réglages de la galerie.
#[derive(Clone, Debug, PartialEq)]
pub struct Gallery {
    pub title: String,
    pub description: String,
    /// Le grand côté des photos, et celui des vignettes, en pixels.
    pub size: usize,
    pub thumb: usize,
    pub quality: u8,
    /// La légende de chaque photo (son titre IPTC, sinon son nom).
    pub captions: bool,
}

impl Default for Gallery {
    fn default() -> Self {
        Gallery {
            title: String::new(),
            description: String::new(),
            size: 2048,
            thumb: 480,
            quality: 85,
            captions: true,
        }
    }
}

/// Une photo de la galerie, prête à écrire : la grande image, la vignette (JPEG) et
/// leurs tailles, la légende.
#[derive(Clone, Debug)]
pub struct Entry {
    pub photo: Vec<u8>,
    pub size: (usize, usize),
    pub thumb: Vec<u8>,
    pub thumb_size: (usize, usize),
    pub caption: String,
}

/// Réduit une image (linéaire) à ce grand côté, sans jamais l'agrandir.
fn fit(image: &RgbImage, long: usize) -> RgbImage {
    let edge = image.width.max(image.height);
    if edge <= long {
        return image.clone();
    }
    let k = long as f64 / edge as f64;
    let w = ((image.width as f64 * k).round() as usize).max(1);
    let h = ((image.height as f64 * k).round() as usize).max(1);
    image.resize_area(w, h)
}

/// La grande image et la vignette d'une photo développée.
pub fn entry(image: &RgbImage, g: &Gallery, caption: String) -> Result<Entry, String> {
    let encode = |img: &RgbImage, quality: u8| -> Result<(Vec<u8>, (usize, usize)), String> {
        let rgb = luma_image::encode8(img, OutputSpace::Srgb);
        Ok((
            crate::jpeg::encode_web(&rgb, img.width, img.height, quality, &[], &[])?,
            (img.width, img.height),
        ))
    };
    let big = fit(image, g.size);
    let (photo, size) = encode(&big, g.quality)?;
    let (thumb, thumb_size) = encode(&fit(&big, g.thumb), g.quality.min(80))?;
    Ok(Entry {
        photo,
        size,
        thumb,
        thumb_size,
        caption,
    })
}

/// La page de la galerie.
pub fn html(g: &Gallery, entries: &[Entry]) -> String {
    let title = escape(&g.title);
    let description = escape(&g.description);
    let mut grid = String::new();
    let mut list = Vec::with_capacity(entries.len());
    for (k, e) in entries.iter().enumerate() {
        let n = crate::slideshow::file_name(k);
        let caption = if g.captions { e.caption.as_str() } else { "" };
        grid.push_str(&format!(
            "<a href=\"photos/{n}\" data-i=\"{k}\"><img src=\"vignettes/{n}\" width=\"{}\" height=\"{}\" loading=\"lazy\" alt=\"{}\"></a>\n",
            e.thumb_size.0,
            e.thumb_size.1,
            escape(caption)
        ));
        list.push(format!("[\"photos/{n}\",\"{}\"]", js(caption)));
    }
    let list = list.join(",");
    let header = if g.title.is_empty() && g.description.is_empty() {
        String::new()
    } else {
        format!("<header><h1>{title}</h1><p>{description}</p></header>\n")
    };
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="LÜMA">
<title>{title}</title>
<style>
:root {{ color-scheme: dark; }}
body {{ margin: 0; background: #111; color: #ddd; font: 16px system-ui, sans-serif; }}
header {{ padding: 6vh 5vw 3vh; }}
h1 {{ margin: 0; font-weight: 300; font-size: 2.4rem; letter-spacing: .04em; color: #fff; }}
header p {{ margin: .6rem 0 0; color: #999; max-width: 50rem; }}
main {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 6px; padding: 0 5vw 6vh; }}
main a {{ display: block; aspect-ratio: 3 / 2; overflow: hidden; background: #1c1c1c; }}
main img {{ width: 100%; height: 100%; object-fit: cover; transition: transform .4s ease; }}
main a:hover img {{ transform: scale(1.03); }}
#vue {{ position: fixed; inset: 0; background: rgba(0, 0, 0, .96); display: none; }}
#vue.on {{ display: block; }}
#vue img {{ position: absolute; inset: 4vh 4vw 8vh; width: 92vw; height: 88vh; object-fit: contain; }}
#nom {{ position: absolute; left: 0; right: 0; bottom: 2.5vh; text-align: center; color: #bbb; }}
</style>
</head>
<body>
{header}<main>
{grid}</main>
<div id="vue"><img alt=""><div id="nom"></div></div>
<script>
const photos = [{list}];
const vue = document.getElementById("vue"), grande = vue.querySelector("img"), nom = document.getElementById("nom");
let i = -1;
function montre(k) {{
  i = (k + photos.length) % photos.length;
  grande.src = photos[i][0];
  nom.textContent = photos[i][1];
  vue.classList.add("on");
  new Image().src = photos[(i + 1) % photos.length][0];
}}
function ferme() {{ vue.classList.remove("on"); i = -1; }}
document.querySelectorAll("main a").forEach(a => a.addEventListener("click", e => {{
  e.preventDefault();
  montre(+a.dataset.i);
}}));
vue.addEventListener("click", e => {{ if (e.target !== grande) ferme(); else montre(i + 1); }});
document.addEventListener("keydown", e => {{
  if (i < 0) return;
  if (e.key === "Escape") ferme();
  else if (e.key === "ArrowRight") montre(i + 1);
  else if (e.key === "ArrowLeft") montre(i - 1);
}});
let depart = null;
vue.addEventListener("touchstart", e => depart = e.touches[0].clientX);
vue.addEventListener("touchend", e => {{
  if (depart === null) return;
  const d = e.changedTouches[0].clientX - depart;
  if (Math.abs(d) > 40) montre(d < 0 ? i + 1 : i - 1);
  depart = null;
}});
</script>
</body>
</html>
"#
    )
}

/// Écrit la galerie dans `dir` : `index.html`, `photos/` et `vignettes/`.
pub fn write(dir: &Path, g: &Gallery, entries: &[Entry]) -> Result<(), String> {
    for sub in ["photos", "vignettes"] {
        let d = dir.join(sub);
        std::fs::create_dir_all(&d).map_err(|e| format!("{} : {e}", d.display()))?;
    }
    for (k, e) in entries.iter().enumerate() {
        let n = crate::slideshow::file_name(k);
        std::fs::write(dir.join("photos").join(&n), &e.photo).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("vignettes").join(&n), &e.thumb).map_err(|e| e.to_string())?;
    }
    std::fs::write(dir.join("index.html"), html(g, entries)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: usize, h: usize) -> RgbImage {
        RgbImage {
            width: w,
            height: h,
            data: (0..w * h)
                .map(|i| [(i % w) as f32 / w as f32, 0.4, (i / w) as f32 / h as f32])
                .collect(),
        }
    }

    #[test]
    fn photos_and_thumbnails_at_their_size() {
        let g = Gallery {
            size: 300,
            thumb: 120,
            ..Gallery::default()
        };
        let e = entry(&image(600, 400), &g, "IMG_0001".into()).unwrap();
        assert_eq!((e.size, e.thumb_size), ((300, 200), (120, 80)));
        let back = image::load_from_memory(&e.thumb).unwrap();
        assert_eq!((back.width(), back.height()), (120, 80));
        // Jamais agrandie.
        let small = entry(&image(90, 60), &g, String::new()).unwrap();
        assert_eq!((small.size, small.thumb_size), ((90, 60), (90, 60)));
    }

    #[test]
    fn the_page_and_the_folder() {
        let g = Gallery {
            title: "Islande <été>".into(),
            description: "Dix jours au nord".into(),
            size: 200,
            thumb: 100,
            ..Gallery::default()
        };
        let entries: Vec<Entry> = ["l'aube", "Höfn"]
            .iter()
            .map(|c| entry(&image(240, 160), &g, (*c).into()).unwrap())
            .collect();
        let page = html(&g, &entries);
        assert!(page.contains("<h1>Islande &lt;été&gt;</h1>"));
        assert!(page.contains("vignettes/002.jpg\" width=\"100\" height=\"67\" loading=\"lazy\" alt=\"Höfn\""));
        assert!(page.contains(r#"["photos/001.jpg","l\u0027aube"]"#));
        assert_eq!(page.matches("</script>").count(), 1);
        // Sans légendes : ni dans la grille, ni dans la vue.
        let quiet = html(
            &Gallery {
                captions: false,
                ..g.clone()
            },
            &entries,
        );
        assert!(!quiet.contains("Höfn"));

        let dir = std::env::temp_dir().join(format!("luma-galerie-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        write(&dir, &g, &entries).unwrap();
        for f in [
            "index.html",
            "photos/001.jpg",
            "photos/002.jpg",
            "vignettes/001.jpg",
            "vignettes/002.jpg",
        ] {
            assert!(dir.join(f).is_file(), "{f}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
