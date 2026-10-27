//! Le diaporama exporté : un dossier qui se suffit à lui‑même, `index.html` et les
//! photos en JPEG, à ouvrir dans n'importe quel navigateur (ordinateur, tablette,
//! téléviseur) ou à mettre en ligne tel quel. Rien à installer, rien à télécharger
//! d'ailleurs : la page est écrite par LÜMA, sans bibliothèque.
//!
//! Les photos défilent seules, en fondu ; les flèches (ou un glissement du doigt)
//! passent à la suivante ou à la précédente, la barre d'espace met en pause, F passe en
//! plein écran. La page n'affiche aucun texte de commande : seulement le titre au
//! début et, si on le veut, le nom de chaque photo.

use std::path::Path;

/// Les réglages du diaporama.
#[derive(Clone, Debug, PartialEq)]
pub struct Slideshow {
    pub title: String,
    /// Le temps de chaque photo, en secondes.
    pub seconds: f32,
    /// La durée du fondu, en secondes.
    pub fade: f32,
    /// Le grand côté des photos, en pixels.
    pub size: usize,
    pub quality: u8,
    /// Le nom de chaque photo sous elle.
    pub captions: bool,
}

impl Default for Slideshow {
    fn default() -> Self {
        Slideshow {
            title: String::new(),
            seconds: 5.0,
            fade: 1.0,
            size: 2048,
            quality: 85,
            captions: false,
        }
    }
}

/// Une photo du diaporama : son fichier (dans `photos/`) et son nom.
#[derive(Clone, Debug, PartialEq)]
pub struct Slide {
    pub file: String,
    pub caption: String,
}

/// Le texte sûr dans du HTML.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Le texte sûr dans une chaîne JavaScript entre guillemets, dans un `<script>` : ni
/// guillemet, ni barre oblique inverse, ni `</script>` ne peuvent en sortir.
fn js(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' | '\\' | '\'' | '<' | '>' | '&' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// La page du diaporama.
pub fn html(show: &Slideshow, slides: &[Slide]) -> String {
    let list: String = slides
        .iter()
        .map(|s| {
            let caption = if show.captions { js(&s.caption) } else { String::new() };
            format!("[\"photos/{}\",\"{caption}\"]", js(&s.file))
        })
        .collect::<Vec<_>>()
        .join(",");
    let title = escape(&show.title);
    let (ms, fade) = (
        (show.seconds.max(0.5) * 1000.0).round() as u32,
        (show.fade.clamp(0.0, 5.0) * 1000.0).round() as u32,
    );
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="LÜMA">
<title>{title}</title>
<style>
html, body {{ margin: 0; height: 100%; background: #000; overflow: hidden; }}
img {{ position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain;
       opacity: 0; transition: opacity {fade}ms ease; }}
img.on {{ opacity: 1; }}
#nom {{ position: absolute; left: 0; right: 0; bottom: 3vh; text-align: center;
        color: #ddd; font: 2.2vh system-ui, sans-serif; text-shadow: 0 0 6px #000; }}
#titre {{ position: absolute; inset: 0; display: flex; align-items: center; justify-content: center;
          color: #fff; font: 300 6vh system-ui, sans-serif; letter-spacing: .08em;
          transition: opacity 1.5s ease; pointer-events: none; }}
#barre {{ position: absolute; left: 0; bottom: 0; height: 2px; background: #888; width: 0; }}
</style>
</head>
<body>
<img id="a" alt=""><img id="b" alt="">
<div id="nom"></div><div id="titre">{title}</div><div id="barre"></div>
<script>
const photos = [{list}];
const temps = {ms};
let i = -1, haut = document.getElementById("a"), bas = document.getElementById("b");
let pause = false, minuterie = null;
function montre(k) {{
  if (!photos.length) return;
  i = (k + photos.length) % photos.length;
  [haut, bas] = [bas, haut];
  haut.src = photos[i][0];
  haut.classList.add("on");
  bas.classList.remove("on");
  document.getElementById("nom").textContent = photos[i][1];
  document.getElementById("barre").style.width = (100 * (i + 1) / photos.length) + "%";
  new Image().src = photos[(i + 1) % photos.length][0];
  relance();
}}
function relance() {{
  clearTimeout(minuterie);
  if (!pause) minuterie = setTimeout(() => montre(i + 1), temps);
}}
document.addEventListener("keydown", e => {{
  if (e.key === "ArrowRight" || e.key === "PageDown") montre(i + 1);
  else if (e.key === "ArrowLeft" || e.key === "PageUp") montre(i - 1);
  else if (e.key === " ") {{ pause = !pause; relance(); e.preventDefault(); }}
  else if (e.key === "f" || e.key === "F") {{
    if (document.fullscreenElement) document.exitFullscreen();
    else document.documentElement.requestFullscreen();
  }}
}});
let depart = null;
document.addEventListener("touchstart", e => depart = e.touches[0].clientX);
document.addEventListener("touchend", e => {{
  if (depart === null) return;
  const d = e.changedTouches[0].clientX - depart;
  if (Math.abs(d) > 40) montre(d < 0 ? i + 1 : i - 1); else {{ pause = !pause; relance(); }}
  depart = null;
}});
document.addEventListener("click", () => montre(i + 1));
setTimeout(() => document.getElementById("titre").style.opacity = 0, {title_ms});
montre(0);
</script>
</body>
</html>
"#,
        title_ms = if show.title.is_empty() { 0 } else { 2500 },
    )
}

/// Le nom du fichier de la photo de rang `k` (à partir de 0) : 001.jpg, 002.jpg…
pub fn file_name(k: usize) -> String {
    format!("{:03}.jpg", k + 1)
}

/// Écrit le diaporama dans `dir` : `index.html` et `photos/` (des JPEG déjà encodés,
/// avec leur nom).
pub fn write(dir: &Path, show: &Slideshow, photos: &[(Vec<u8>, String)]) -> Result<(), String> {
    let folder = dir.join("photos");
    std::fs::create_dir_all(&folder).map_err(|e| format!("{} : {e}", folder.display()))?;
    let mut slides = Vec::with_capacity(photos.len());
    for (k, (jpeg, caption)) in photos.iter().enumerate() {
        let file = file_name(k);
        std::fs::write(folder.join(&file), jpeg).map_err(|e| e.to_string())?;
        slides.push(Slide {
            file,
            caption: caption.clone(),
        });
    }
    std::fs::write(dir.join("index.html"), html(show, &slides)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_lists_the_photos_safely() {
        let show = Slideshow {
            title: "Voyage <en> \"Islande\"".into(),
            seconds: 3.5,
            captions: true,
            ..Slideshow::default()
        };
        let slides = [
            Slide {
                file: file_name(0),
                caption: "IMG_0001.ARW".into(),
            },
            Slide {
                file: file_name(1),
                caption: "l'aube</script>".into(),
            },
        ];
        let page = html(&show, &slides);
        assert!(page.contains("[\"photos/001.jpg\",\"IMG_0001.ARW\"]"));
        assert!(page.contains("photos/002.jpg"));
        assert!(page.contains("const temps = 3500;"));
        assert!(page.contains("<title>Voyage &lt;en&gt; &quot;Islande&quot;</title>"));
        // Un nom ne peut ni fermer le script ni sortir de sa chaîne.
        assert_eq!(page.matches("</script>").count(), 1);
        assert!(page.contains(r"l\u0027aube\u003c/script\u003e"));
        // Sans les noms : aucun nom dans la page.
        let quiet = html(&Slideshow::default(), &slides);
        assert!(!quiet.contains("IMG_0001") && quiet.contains("\"photos/001.jpg\",\"\""));
    }

    #[test]
    fn written_folder() {
        let dir = std::env::temp_dir().join(format!("luma-diaporama-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let jpeg = crate::jpeg::encode(&[128; 16 * 16 * 3], 16, 16, 85, &[], &[]).unwrap();
        write(
            &dir,
            &Slideshow::default(),
            &[(jpeg.clone(), "a".into()), (jpeg, "b".into())],
        )
        .unwrap();
        assert!(dir.join("photos/001.jpg").is_file() && dir.join("photos/002.jpg").is_file());
        let page = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(page.starts_with("<!doctype html>") && page.contains("photos/002.jpg"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
