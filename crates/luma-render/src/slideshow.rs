//! Le diaporama exporté : un dossier qui se suffit à lui‑même, `index.html` et les
//! photos en JPEG, à ouvrir dans n'importe quel navigateur (ordinateur, tablette,
//! téléviseur) ou à mettre en ligne tel quel. Rien à installer, rien à télécharger
//! d'ailleurs : la page est écrite par LÜMA, sans bibliothèque.
//!
//! Les photos défilent seules, en fondu, en glissement ou en fondu avec un lent zoom ;
//! une musique, si on en choisit une, joue en boucle. Les flèches (ou un glissement du doigt)
//! passent à la suivante ou à la précédente, la barre d'espace met en pause, F passe en
//! plein écran, M coupe la musique. La page n'affiche aucun texte de commande : seulement le titre au
//! début et, si on le veut, le nom de chaque photo.

use std::path::{Path, PathBuf};

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
    /// Les photos dans un ordre tiré au hasard, chaque fois que la page s'ouvre.
    pub shuffle: bool,
    /// Le passage d'une photo à la suivante.
    pub transition: Transition,
    /// La musique, copiée dans le dossier et jouée en boucle.
    pub music: Option<PathBuf>,
}

/// Le passage d'une photo à la suivante, pendant la durée du fondu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transition {
    /// La photo suivante apparaît sur la précédente.
    #[default]
    Fade,
    /// La photo suivante entre par la droite et pousse la précédente.
    Slide,
    /// Un fondu, et chaque photo s'avance lentement (un zoom de 8 % sur tout son temps,
    /// vers un côté puis vers l'autre).
    Drift,
}

impl Transition {
    pub const ALL: [Transition; 3] = [Transition::Fade, Transition::Slide, Transition::Drift];

    /// Le nom de la ligne de commande.
    pub fn key(self) -> &'static str {
        match self {
            Transition::Fade => "fade",
            Transition::Slide => "slide",
            Transition::Drift => "drift",
        }
    }

    pub fn from_key(key: &str) -> Option<Transition> {
        Transition::ALL.into_iter().find(|t| t.key() == key)
    }
}

/// Les formats de musique que lisent tous les navigateurs (ou presque).
pub const MUSIC_EXTENSIONS: [&str; 6] = ["mp3", "m4a", "aac", "ogg", "wav", "flac"];

/// Le nom de la musique dans le dossier du diaporama : `musique.<extension>`.
pub fn music_file(show: &Slideshow) -> Option<String> {
    let ext = show.music.as_ref()?.extension()?.to_str()?.to_lowercase();
    MUSIC_EXTENSIONS
        .contains(&ext.as_str())
        .then(|| format!("musique.{ext}"))
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
            shuffle: false,
            transition: Transition::Fade,
            music: None,
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
pub(crate) fn escape(s: &str) -> String {
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
pub(crate) fn js(s: &str) -> String {
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
img {{ position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; }}
{style}#nom {{ position: absolute; left: 0; right: 0; bottom: 3vh; text-align: center;
        color: #ddd; font: 2.2vh system-ui, sans-serif; text-shadow: 0 0 6px #000; }}
#titre {{ position: absolute; inset: 0; display: flex; align-items: center; justify-content: center;
          color: #fff; font: 300 6vh system-ui, sans-serif; letter-spacing: .08em;
          transition: opacity 1.5s ease; pointer-events: none; }}
#barre {{ position: absolute; left: 0; bottom: 0; height: 2px; background: #888; width: 0; }}
</style>
</head>
<body>
<img id="a" alt=""><img id="b" alt="">{audio}
<div id="nom"></div><div id="titre">{title}</div><div id="barre"></div>
<script>
const photos = [{list}];
// Dans le désordre, si on l'a demandé : un nouveau tirage à chaque ouverture.
if ({shuffle}) for (let k = photos.length - 1; k > 0; k--) {{
  const j = Math.floor(Math.random() * (k + 1));
  [photos[k], photos[j]] = [photos[j], photos[k]];
}}
const temps = {ms};
let i = -1, haut = document.getElementById("a"), bas = document.getElementById("b");
let pause = false, minuterie = null;
const musique = document.getElementById("musique");
function joue() {{ if (musique && musique.paused && !pause) musique.play().catch(() => {{}}); }}
function montre(k) {{
  if (!photos.length) return;
  i = (k + photos.length) % photos.length;
  [haut, bas] = [bas, haut];
  // La photo qui entre repart de sa place de départ, sans transition.
  haut.style.transition = "none";
  haut.classList.remove("out", "on", "autre");
  void haut.offsetWidth;
  haut.style.transition = "";
  haut.src = photos[i][0];
  if (i % 2) haut.classList.add("autre");
  haut.classList.add("on");
  bas.classList.remove("on");
  bas.classList.add("out");
  document.getElementById("nom").textContent = photos[i][1];
  document.getElementById("barre").style.width = (100 * (i + 1) / photos.length) + "%";
  new Image().src = photos[(i + 1) % photos.length][0];
  relance();
}}
function bascule() {{
  pause = !pause;
  if (musique) {{ if (pause) musique.pause(); else joue(); }}
  relance();
}}
function relance() {{
  clearTimeout(minuterie);
  if (!pause) minuterie = setTimeout(() => montre(i + 1), temps);
}}
// Un navigateur ne lance la musique qu'après un geste : au premier, sinon tout de suite.
for (const geste of ["keydown", "click", "touchend"]) document.addEventListener(geste, joue);
document.addEventListener("keydown", e => {{
  if (e.key === "ArrowRight" || e.key === "PageDown") montre(i + 1);
  else if (e.key === "ArrowLeft" || e.key === "PageUp") montre(i - 1);
  else if (e.key === " ") {{ bascule(); e.preventDefault(); }}
  else if ((e.key === "m" || e.key === "M") && musique) musique.muted = !musique.muted;
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
  if (Math.abs(d) > 40) montre(d < 0 ? i + 1 : i - 1); else bascule();
  depart = null;
}});
document.addEventListener("click", () => montre(i + 1));
setTimeout(() => document.getElementById("titre").style.opacity = 0, {title_ms});
montre(0);
joue();
</script>
</body>
</html>
"#,
        title_ms = if show.title.is_empty() { 0 } else { 2500 },
        shuffle = show.shuffle,
        style = transition_style(show.transition, ms, fade),
        audio = music_file(show)
            .map(|f| format!(
                "\n<audio id=\"musique\" src=\"{}\" loop preload=\"auto\"></audio>",
                escape(&f)
            ))
            .unwrap_or_default(),
    )
}

/// Les règles de la transition : `on` est la photo affichée, `out` celle qui part,
/// `autre` une photo sur deux (le zoom lent change de côté).
fn transition_style(transition: Transition, ms: u32, fade: u32) -> String {
    match transition {
        Transition::Fade => {
            format!("img {{ opacity: 0; transition: opacity {fade}ms ease; }}\nimg.on {{ opacity: 1; }}\n")
        }
        Transition::Slide => format!(
            "img {{ transform: translateX(100%); transition: transform {fade}ms ease-in-out; }}\n\
             img.on {{ transform: none; }}\nimg.out {{ transform: translateX(-100%); }}\n"
        ),
        Transition::Drift => format!(
            "img {{ opacity: 0; transition: opacity {fade}ms ease; transform-origin: 30% 40%; }}\n\
             img.autre {{ transform-origin: 70% 60%; }}\nimg.on {{ opacity: 1; }}\n\
             img.on, img.out {{ animation: avance {total}ms linear forwards; }}\n\
             @keyframes avance {{ from {{ transform: scale(1); }} to {{ transform: scale(1.08); }} }}\n",
            total = ms + 2 * fade
        ),
    }
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
    if let (Some(from), Some(file)) = (&show.music, music_file(show)) {
        let to = dir.join(file);
        // La musique déjà dans le dossier (un diaporama refait) : la copier sur elle‑même
        // la viderait.
        let same = matches!((from.canonicalize(), to.canonicalize()), (Ok(a), Ok(b)) if a == b);
        if !same {
            std::fs::copy(from, &to).map_err(|e| format!("{} : {e}", from.display()))?;
        }
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

    #[test]
    fn transitions_and_music() {
        let slides = [Slide {
            file: file_name(0),
            caption: String::new(),
        }];
        let page = |t: Transition, music: Option<&str>| {
            html(
                &Slideshow {
                    transition: t,
                    seconds: 4.0,
                    fade: 1.0,
                    music: music.map(PathBuf::from),
                    ..Slideshow::default()
                },
                &slides,
            )
        };
        let fade = page(Transition::Fade, None);
        assert!(fade.contains("transition: opacity 1000ms") && !fade.contains("translateX"));
        // Sans musique : ni balise, ni fichier.
        assert!(!fade.contains("<audio"));
        let slide = page(Transition::Slide, None);
        assert!(slide.contains("img.out { transform: translateX(-100%); }"));
        // Le zoom lent dure toute la photo, fondus compris.
        let drift = page(Transition::Drift, None);
        assert!(drift.contains("animation: avance 6000ms") && drift.contains("scale(1.08)"));
        let music = page(Transition::Fade, Some("/musiques/Été.MP3"));
        assert!(music.contains("<audio id=\"musique\" src=\"musique.mp3\" loop"));
        // Un fichier qui n'est pas une musique est ignoré.
        assert!(!page(Transition::Fade, Some("/a/notes.txt")).contains("<audio"));
        for t in Transition::ALL {
            assert_eq!(Transition::from_key(t.key()), Some(t));
        }
        // L'ordre au hasard : tiré dans la page, seulement si on le demande.
        assert!(page(Transition::Fade, None).contains("if (false) for (let k"));
        let shuffled = html(
            &Slideshow {
                shuffle: true,
                ..Slideshow::default()
            },
            &slides,
        );
        assert!(shuffled.contains("if (true) for (let k"));
    }

    #[test]
    fn the_music_is_copied_into_the_folder() {
        let dir = std::env::temp_dir().join(format!("luma-diaporama-musique-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let song = dir.join("chanson.ogg");
        std::fs::write(&song, b"OggS musique").unwrap();
        let jpeg = crate::jpeg::encode(&[128; 16 * 16 * 3], 16, 16, 85, &[], &[]).unwrap();
        let out = dir.join("diaporama");
        let show = Slideshow {
            music: Some(song),
            ..Slideshow::default()
        };
        write(&out, &show, &[(jpeg, "a".into())]).unwrap();
        assert_eq!(std::fs::read(out.join("musique.ogg")).unwrap(), b"OggS musique");
        let page = std::fs::read_to_string(out.join("index.html")).unwrap();
        assert!(page.contains("src=\"musique.ogg\""));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
