//! Les lettres que la police de LÜMA n'a pas (japonais, chinois, coréen, arabe,
//! hébreu, symboles…) dans un nom de fichier, un mot‑clé ou une légende : elles sont
//! cherchées dans les polices du système, à la demande, au lieu de s'afficher en
//! petites boîtes vides. Rien n'est chargé tant qu'aucune lettre ne manque ; une police
//! trouvée sert ensuite à toutes les lettres qu'elle a.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use egui::{FallbackRequest, FontData, FontInsert, FontPriority, InsertFontFamily};
use skrifa::MetadataProvider;
use skrifa::raw::FileRef;

/// Les dossiers de polices du système, dans l'ordre où les chercher.
pub fn system_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs = Vec::new();
    if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        dirs.extend(
            [
                "/System/Library/Fonts",
                "/Library/Fonts",
                "/System/Library/Fonts/Supplemental",
            ]
            .map(PathBuf::from),
        );
        dirs.extend(home.map(|h| h.join("Library/Fonts")));
    } else if cfg!(windows) {
        let windir =
            std::env::var_os("WINDIR").map_or_else(|| PathBuf::from("C:\\Windows"), PathBuf::from);
        dirs.push(windir.join("Fonts"));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Microsoft\\Windows\\Fonts"));
        }
    } else {
        dirs.extend(["/usr/share/fonts", "/usr/local/share/fonts"].map(PathBuf::from));
        if let Some(h) = home {
            dirs.push(h.join(".local/share/fonts"));
            dirs.push(h.join(".fonts"));
        }
    }
    dirs
}

/// Les fichiers de polices sous `dirs` (trois niveaux de dossiers au plus), les plus
/// complètes d'abord : celles connues pour couvrir beaucoup d'écritures, puis les
/// autres par nom. Les polices d'emoji en couleur sont laissées : leurs dessins ne
/// sont pas des contours, la lettre resterait vide.
pub fn font_files(dirs: &[PathBuf]) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for e in read.flatten() {
            let p = e.path();
            if p.is_dir() {
                if depth > 0 {
                    walk(&p, depth - 1, out);
                }
            } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                matches!(
                    x.to_ascii_lowercase().as_str(),
                    "ttf" | "otf" | "ttc" | "otc"
                )
            }) {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    for d in dirs {
        walk(d, 3, &mut files);
    }
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    };
    files.retain(|p| !name(p).contains("emoji"));
    const WIDE: [&str; 17] = [
        "pingfang",
        "hiragino",
        "ヒラギノ角ゴシック",
        "stheiti",
        "applesdgothic",
        "arial unicode",
        "notosanscjk",
        "noto sans cjk",
        "msyh",
        "yugoth",
        "malgun",
        "notosans-",
        "dejavusans",
        "segoeui",
        "apple symbols",
        "geezapro",
        "arial hebrew",
    ];
    let rank = |p: &Path| {
        let n = name(p);
        WIDE.iter()
            .position(|w| n.contains(w))
            .unwrap_or(WIDE.len())
    };
    files.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.cmp(b)));
    files.dedup();
    files
}

/// Le visage de `data` (un fichier, ou une collection) qui a `c`, s'il y en a un.
pub fn face_with(data: &[u8], c: char) -> Option<u32> {
    let has = |f: &skrifa::FontRef| f.charmap().map(c).is_some();
    match FileRef::new(data).ok()? {
        FileRef::Font(f) => has(&f).then_some(0),
        FileRef::Collection(col) => (0..col.len()).find(|&i| col.get(i).is_ok_and(|f| has(&f))),
    }
}

/// Le fournisseur de polices de secours d'egui : la première police du système (dans
/// l'ordre de [`font_files`]) qui a la lettre. Les fichiers se lisent un à un, à la
/// première lettre qui manque ; la liste n'est faite qu'une fois.
pub struct SystemFallback {
    dirs: Vec<PathBuf>,
    files: Mutex<Option<Vec<PathBuf>>>,
}

impl SystemFallback {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        SystemFallback {
            dirs,
            files: Mutex::new(None),
        }
    }

    /// La police (fichier, visage) qui a `c`.
    pub fn find(&self, c: char) -> Option<(PathBuf, Arc<Vec<u8>>, u32)> {
        let mut files = self.files.lock().ok()?;
        let files = files.get_or_insert_with(|| font_files(&self.dirs));
        // Au plus 32 fichiers lus par lettre : un dossier de polices immense ne fige pas
        // l'affichage (les plus complètes sont en tête).
        files.iter().take(32).find_map(|p| {
            // Une police de plus de 64 Mo n'est pas une police d'interface.
            if std::fs::metadata(p).is_ok_and(|m| m.len() > 64 << 20) {
                return None;
            }
            let data = Arc::new(std::fs::read(p).ok()?);
            let face = face_with(&data, c)?;
            Some((p.clone(), data, face))
        })
    }
}

impl egui::FontProvider for SystemFallback {
    fn font_for(&self, request: &FallbackRequest<'_>) -> Option<FontInsert> {
        let c = request.cluster.chars().next()?;
        let (path, data, face) = self.find(c)?;
        Some(FontInsert {
            name: format!("{}#{face}", path.display()),
            data: FontData::from_blob(data, face),
            families: vec![InsertFontFamily {
                family: request.family.clone(),
                priority: FontPriority::Lowest,
            }],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une police trouvée par sa lettre, dans un dossier et ses sous‑dossiers ; une
    /// lettre qu'aucune n'a ne trouve rien ; les polices d'emoji sont laissées.
    #[test]
    fn a_missing_letter_is_found_in_a_system_font() {
        let dir = std::env::temp_dir().join(format!("luma-polices-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sous/dossier")).unwrap();
        let defs = egui::FontDefinitions::default();
        let bytes = |name: &str| (*defs.font_data[name].font).as_ref().to_vec();
        std::fs::write(
            dir.join("sous/dossier/Ubuntu-Light.ttf"),
            bytes("Ubuntu-Light"),
        )
        .unwrap();
        std::fs::write(dir.join("Couleur-Emoji.ttf"), bytes("Ubuntu-Light")).unwrap();
        std::fs::write(dir.join("pas-une-police.ttf"), b"rien").unwrap();
        std::fs::write(dir.join("notes.txt"), b"rien").unwrap();

        let files = font_files(std::slice::from_ref(&dir));
        assert_eq!(files.len(), 2, "{files:?}");
        let fallback = SystemFallback::new(vec![dir.clone()]);
        let (path, data, face) = fallback.find('é').expect("é");
        assert!(path.ends_with("Ubuntu-Light.ttf") && face == 0);
        assert_eq!(face_with(&data, 'A'), Some(0));
        assert!(fallback.find('日').is_none());
        assert!(face_with(b"rien", 'A').is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// egui lui‑même demande la lettre manquante au fournisseur : sans lui, l'essai
    /// (réglé pour paniquer sur une lettre absente) panique ; avec lui, la lettre vient
    /// de la police du « système ».
    #[test]
    fn egui_asks_for_the_missing_letter() {
        let defs = egui::FontDefinitions::default();
        let hack = (*defs.font_data["Hack"].font).as_ref().to_vec();
        let ubuntu = (*defs.font_data["Ubuntu-Light"].font).as_ref().to_vec();
        // Une lettre que la police monospace a et que la police de l'app n'a pas.
        let c = ('\u{2500}'..='\u{25FF}')
            .find(|&c| face_with(&hack, c).is_some() && face_with(&ubuntu, c).is_none())
            .expect("une lettre de la police monospace seule");
        let dir = std::env::temp_dir().join(format!("luma-polices-egui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Hack.ttf"), &hack).unwrap();

        let draw = |provider: bool| {
            let ctx = egui::Context::default();
            let mut only = egui::FontDefinitions::default();
            only.font_data.retain(|name, _| name == "Ubuntu-Light");
            for list in only.families.values_mut() {
                list.retain(|name| name == "Ubuntu-Light");
            }
            ctx.set_fonts(only);
            ctx.set_missing_glyph_policy(egui::MissingGlyphPolicy::Panic);
            if provider {
                ctx.add_font_provider(Arc::new(SystemFallback::new(vec![dir.clone()])));
            }
            let text = format!("Photo {c}");
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                    ui.label(&text);
                });
                // Les textures du dessin ne vont nulle part : les laisser tomber sans les
                // vider est une erreur pour egui (en mode vérifié).
                out.textures_delta.clear();
            }))
            .is_ok()
        };
        assert!(!draw(false), "sans le fournisseur, la lettre manque");
        assert!(draw(true), "avec lui, elle est trouvée");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
