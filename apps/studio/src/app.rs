//! LÜMA Studio : l'état de l'application et son interface.
//!
//! L'interface ne ressemble à aucune autre : la photo occupe tout l'écran, et les
//! outils flottent dessus dans des îlots de verre fumé. À gauche, le rail des outils
//! (Bibliothèque, Lumière, Couleur, Détail, Masques, Films, Géométrie, Fusion) ; à
//! droite, l'inspecteur de l'outil choisi, et lui seul ; en bas, la table lumineuse du
//! dossier. L'anneau spectral de LÜMA marque ce qui est actif.
//!
//! Les gestes et les réglages sont pensés pour le travail photo : curseurs fins,
//! historique, XMP enregistré à côté du RAW, raccourcis (`\`
//! avant/après, ⌘Z, ⇧⌘C et ⇧⌘V, ⇧⌘E exporter, ← →, J écrêtage, Z zoom 1:1, G, D, M,
//! Maj+M, K, Q, R, W, O).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{
    Color32, ColorImage, Key, Rect, RichText, TextureHandle, TextureOptions, Ui, pos2, vec2,
};
use luma_color::OutputSpace;
use luma_image::scopes::Scope;
use luma_library::query::fold;
use luma_project::presets::{self, PRESETS};
use luma_project::xmp::{Flag, Meta};
use luma_project::{Crop, Mask, Profile, Range, Settings, Shape, WhiteBalance};
use luma_render::{Photo, Size};

use crate::crop;
use crate::curving;
use crate::i18n::{tr, trf};
use crate::icons::{self, Icon};
use crate::liquifying;
use crate::masking;
use crate::retouching;
use crate::widgets::{self, Change, SliderSpec, Track, format_value, theme};
use crate::worker::{
    Event, ExportJob, ExportOptions, Format, MergeJob, MergeKind, Worker, read_meta, read_sidecar,
    write_meta, write_sidecar,
};

pub use luma_raw::{RAW_EXTENSIONS, is_photo, is_raw};

/// Les extensions de la fenêtre d'ouverture : les RAW et les images de ce système.
pub fn photo_extensions() -> Vec<String> {
    let raster = luma_raw::raster::EXTENSIONS
        .iter()
        .chain(luma_raw::raster::SYSTEM_EXTENSIONS.iter())
        .filter(|e| luma_raw::raster::supported(e));
    RAW_EXTENSIONS
        .iter()
        .chain(raster)
        .flat_map(|e| [e.to_string(), e.to_uppercase()])
        .collect()
}

/// Les photos d'un dossier, triées par nom.
pub fn scan(dir: &Path) -> Vec<PathBuf> {
    let paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    let mut files: Vec<PathBuf> = paths
        .iter()
        .filter(|p| p.is_file() && is_photo(p))
        .cloned()
        .collect();
    // Les copies virtuelles, d'après leur XMP (`IMG_0001.ARW.copie-2.xmp`), quand leur
    // original est là.
    let copies: Vec<PathBuf> = paths
        .iter()
        .filter_map(|p| p.to_str()?.strip_suffix(".xmp").map(PathBuf::from))
        .filter(|c| luma_project::copies::number(c).is_some())
        .filter(|c| files.contains(&luma_project::copies::source(c)))
        .collect();
    files.extend(copies);
    sort_photos(&mut files);
    files
}

/// Chaque photo, puis ses copies dans l'ordre de leur numéro.
fn sort_photos(files: &mut [PathBuf]) {
    files.sort_by_key(|p| {
        (
            luma_project::copies::source(p),
            luma_project::copies::number(p),
        )
    });
}

/// Un réglage du panneau : son curseur et l'endroit où il vit dans les réglages.
struct Field {
    spec: SliderSpec,
    get: fn(&mut Settings) -> &mut f32,
}

const fn field(
    label: &'static str,
    min: f32,
    max: f32,
    default: f32,
    decimals: usize,
    get: fn(&mut Settings) -> &mut f32,
) -> Field {
    Field {
        spec: SliderSpec {
            label,
            min,
            max,
            default,
            decimals,
            track: Track::Plain,
            log: false,
        },
        get,
    }
}

const TONE: [Field; 6] = [
    field("Exposition", -5.0, 5.0, 0.0, 2, |s| &mut s.exposure),
    field("Contraste", -100.0, 100.0, 0.0, 0, |s| &mut s.contrast),
    field("Hautes lumières", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.highlights
    }),
    field("Ombres", -100.0, 100.0, 0.0, 0, |s| &mut s.shadows),
    field("Blancs", -100.0, 100.0, 0.0, 0, |s| &mut s.whites),
    field("Noirs", -100.0, 100.0, 0.0, 0, |s| &mut s.blacks),
];
const PRESENCE: [Field; 5] = [
    field("Texture", -100.0, 100.0, 0.0, 0, |s| &mut s.texture),
    field("Clarté", -100.0, 100.0, 0.0, 0, |s| &mut s.clarity),
    field("Correction du voile", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.dehaze
    }),
    field("Vibrance", -100.0, 100.0, 0.0, 0, |s| &mut s.vibrance),
    field("Saturation", -100.0, 100.0, 0.0, 0, |s| &mut s.saturation),
];
const CURVE: [Field; 7] = [
    field("Hautes lumières", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.tone_curve.highlights
    }),
    field("Tons clairs", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.tone_curve.lights
    }),
    field("Tons foncés", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.tone_curve.darks
    }),
    field("Ombres", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.tone_curve.shadows
    }),
    field("Séparation ombres", 10.0, 70.0, 25.0, 0, |s| {
        &mut s.tone_curve.shadow_split
    }),
    field("Séparation tons moyens", 20.0, 80.0, 50.0, 0, |s| {
        &mut s.tone_curve.midtone_split
    }),
    field("Séparation hautes lum.", 30.0, 90.0, 75.0, 0, |s| {
        &mut s.tone_curve.highlight_split
    }),
];
const SHARPEN: [Field; 4] = [
    field("Gain", 0.0, 150.0, 40.0, 0, |s| {
        &mut s.detail.sharpen_amount
    }),
    field("Rayon", 0.5, 3.0, 1.0, 1, |s| &mut s.detail.sharpen_radius),
    field("Détail", 0.0, 100.0, 25.0, 0, |s| {
        &mut s.detail.sharpen_detail
    }),
    field("Masquage", 0.0, 100.0, 0.0, 0, |s| {
        &mut s.detail.sharpen_masking
    }),
];
const NOISE: [Field; 6] = [
    field("Luminance", 0.0, 100.0, 0.0, 0, |s| {
        &mut s.detail.luminance_nr
    }),
    field("Détail lum.", 0.0, 100.0, 50.0, 0, |s| {
        &mut s.detail.luminance_nr_detail
    }),
    field("Contraste lum.", 0.0, 100.0, 0.0, 0, |s| {
        &mut s.detail.luminance_nr_contrast
    }),
    field("Couleur", 0.0, 100.0, 25.0, 0, |s| &mut s.detail.color_nr),
    field("Détail coul.", 0.0, 100.0, 50.0, 0, |s| {
        &mut s.detail.color_nr_detail
    }),
    field("Lissage", 0.0, 100.0, 50.0, 0, |s| {
        &mut s.detail.color_nr_smoothness
    }),
];
const VIGNETTE: [Field; 5] = [
    field("Gain", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.effects.vignette_amount
    }),
    field("Milieu", 0.0, 100.0, 50.0, 0, |s| {
        &mut s.effects.vignette_midpoint
    }),
    field("Arrondi", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.effects.vignette_roundness
    }),
    field("Contour progressif", 0.0, 100.0, 50.0, 0, |s| {
        &mut s.effects.vignette_feather
    }),
    field("Hautes lumières", 0.0, 100.0, 0.0, 0, |s| {
        &mut s.effects.vignette_highlights
    }),
];
const GRAIN: [Field; 3] = [
    field("Gain", 0.0, 100.0, 0.0, 0, |s| &mut s.effects.grain_amount),
    field("Taille", 0.0, 100.0, 25.0, 0, |s| &mut s.effects.grain_size),
    field("Rugosité", 0.0, 100.0, 50.0, 0, |s| {
        &mut s.effects.grain_roughness
    }),
];
const CALIBRATION: [Field; 7] = [
    field("Teinte des ombres", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.calibration.shadows_tint
    }),
    field("Rouge, teinte", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.calibration.red_hue
    }),
    field("Rouge, saturation", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.calibration.red_saturation
    }),
    field("Vert, teinte", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.calibration.green_hue
    }),
    field("Vert, saturation", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.calibration.green_saturation
    }),
    field("Bleu, teinte", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.calibration.blue_hue
    }),
    field("Bleu, saturation", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.calibration.blue_saturation
    }),
];

const LENS: [Field; 5] = [
    field("Distorsion", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.lens.distortion
    }),
    field("Vignetage", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.lens.vignetting
    }),
    field("Milieu", 0.0, 100.0, 50.0, 0, |s| {
        &mut s.lens.vignetting_midpoint
    }),
    field("Défrangeage violet", 0.0, 20.0, 0.0, 0, |s| {
        &mut s.lens.defringe_purple
    }),
    field("Défrangeage vert", 0.0, 20.0, 0.0, 0, |s| {
        &mut s.lens.defringe_green
    }),
];
const TRANSFORM: [Field; 5] = [
    field("Verticale", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.transform.vertical
    }),
    field("Horizontale", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.transform.horizontal
    }),
    field("Rotation", -10.0, 10.0, 0.0, 1, |s| &mut s.transform.rotate),
    field("Proportions", -100.0, 100.0, 0.0, 0, |s| {
        &mut s.transform.aspect
    }),
    field("Échelle", 50.0, 150.0, 100.0, 0, |s| {
        &mut s.transform.scale
    }),
];

/// Les préréglages de balance des blancs (température, teinte).
const WB_PRESETS: [(&str, f32, f32); 6] = [
    ("Lumière du jour", 5500.0, 10.0),
    ("Nuageux", 6500.0, 10.0),
    ("Ombre", 7500.0, 10.0),
    ("Tungstène", 2850.0, 0.0),
    ("Fluorescent", 3800.0, 21.0),
    ("Flash", 5500.0, 0.0),
];

const HSL_NAMES: [&str; 8] = [
    "Rouge",
    "Orange",
    "Jaune",
    "Vert",
    "Bleu-vert",
    "Bleu",
    "Violet",
    "Magenta",
];
const HSL_HUES: [f32; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0];

/// Les libellés de couleur, dans l'ordre de `xmp::LABELS` (touches 6 à 9 pour les
/// quatre premiers).
const LABEL_NAMES: [&str; 5] = ["Rouge", "Jaune", "Vert", "Bleu", "Violet"];
const LABEL_COLORS: [Color32; 5] = [
    Color32::from_rgb(0xe0, 0x50, 0x50),
    Color32::from_rgb(0xe8, 0xc4, 0x40),
    Color32::from_rgb(0x52, 0xb0, 0x62),
    Color32::from_rgb(0x4a, 0x80, 0xe0),
    Color32::from_rgb(0x9a, 0x62, 0xd0),
];

/// La couleur du libellé d'une photo, s'il en est un de couleur.
fn label_color32(label: &str) -> Option<Color32> {
    luma_project::xmp::label_color(label).map(|i| LABEL_COLORS[i])
}

/// Les modules de la barre du haut (touches G et D).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Module {
    Library,
    Develop,
}

/// Les outils du rail.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Library,
    Light,
    Color,
    Detail,
    Masks,
    Films,
    Geometry,
    Merge,
}

impl Tool {
    pub const ALL: [Tool; 8] = [
        Tool::Library,
        Tool::Light,
        Tool::Color,
        Tool::Detail,
        Tool::Masks,
        Tool::Films,
        Tool::Geometry,
        Tool::Merge,
    ];

    /// Le nom sous l'icône, dans la barre de l'iPhone : un mot.
    pub fn short_name(self) -> &'static str {
        match self {
            Tool::Library => tr("Photos"),
            Tool::Geometry => tr("Cadrage"),
            t => t.name(),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Tool::Library => tr("Bibliothèque"),
            Tool::Light => tr("Lumière"),
            Tool::Color => tr("Couleur"),
            Tool::Detail => tr("Détail"),
            Tool::Masks => tr("Masques"),
            Tool::Films => tr("Films"),
            Tool::Geometry => tr("Géométrie"),
            Tool::Merge => tr("Fusion"),
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Tool::Library => tr("Trier, noter, choisir"),
            Tool::Light => tr("Profil, balance, tonalité, courbe"),
            Tool::Color => tr("Mélangeur, étalonnage, calibration"),
            Tool::Detail => tr("Netteté, bruit, vignetage, grain"),
            Tool::Masks => tr("Retouches locales"),
            Tool::Films => tr("Pellicules et looks"),
            Tool::Geometry => tr("Recadrage, objectif, perspective"),
            Tool::Merge => tr("Empilements et panoramas"),
        }
    }

    fn icon(self) -> Icon {
        match self {
            Tool::Library => Icon::Library,
            Tool::Light => Icon::Light,
            Tool::Color => Icon::Color,
            Tool::Detail => Icon::Detail,
            Tool::Masks => Icon::Masks,
            Tool::Films => Icon::Films,
            Tool::Geometry => Icon::Geometry,
            Tool::Merge => Icon::Merge,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Tool::Library => "G",
            Tool::Light => "D",
            Tool::Masks => tr("M, Maj+M, K, Q"),
            Tool::Geometry => "R",
            _ => "",
        }
    }
}

/// Ce que montrent la grille et le film : un dossier, ou une vue de la photothèque.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum View {
    #[default]
    Folder,
    /// Toutes les photos du catalogue, par date de prise de vue.
    All,
    Collection(String),
    /// Une recherche, telle qu'elle a été tapée.
    Search(String),
}

/// Le filtre de la Bibliothèque, appliqué aussi au film et à ← →.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FlagFilter {
    #[default]
    All,
    Picks,
    NotRejected,
}

/// L'outil Recadrage ouvert : le cadre d'avant (pour Échap) et les proportions.
struct CropTool {
    saved: Crop,
    aspect: usize,
}

/// Ce qui a été demandé au rendu, pour ne pas redemander la même chose.
#[derive(PartialEq)]
struct Request {
    path: PathBuf,
    settings: Settings,
    size: Size,
    clipping: bool,
    /// La version de la table d'épreuvage, la simulation et l'alerte de gamut.
    proof: Option<(u64, bool, bool)>,
    scene: bool,
    dust_view: bool,
    find_dust: bool,
    scope: Scope,
}

/// La comparaison de 2 à 4 photos : leurs rendus, et la vue commune (zoom et centre
/// partagés par toutes, loupe).
struct Compare {
    paths: Vec<PathBuf>,
    textures: Vec<Option<TextureHandle>>,
    tag: u64,
    /// 1 : la photo entière ; au‑delà, agrandie.
    zoom: f32,
    /// Le point de la photo au centre de chaque case (0 à 1 sur chaque axe).
    center: egui::Vec2,
    loupe: bool,
}

/// Une sortie en cours de fabrication, en arrière‑plan : un PDF d'impression, un livre
/// ou un diaporama.
struct Printing {
    done: std::sync::mpsc::Receiver<Result<(PathBuf, usize), String>>,
    placed: Arc<std::sync::atomic::AtomicUsize>,
    total: usize,
    output: Output,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    Print,
    Book,
    Slideshow,
    Gallery,
    Import,
}

/// L'import depuis une carte : d'où, vers où, comment.
pub struct Import {
    /// La carte (ou le dossier) à importer.
    pub source: Option<PathBuf>,
    /// Le dossier de la photothèque.
    pub dest: Option<PathBuf>,
    /// Les sous‑dossiers d'après la date (voir `luma_library::import::folder`).
    pub folders: String,
    /// Le modèle des nouveaux noms ; vide : les noms d'origine.
    pub name: String,
    pub backup: Option<PathBuf>,
    /// Les photos trouvées sur la source (comptées à son choix).
    found: usize,
    /// Les jours de prise de vue de la source, leur nombre de photos, et s'ils sont
    /// choisis ; lus en arrière‑plan (`days_job`).
    pub days: Vec<(String, usize, bool)>,
    days_job: Option<std::sync::mpsc::Receiver<SourceContents>>,
    /// Les prises de la source, pour choisir photo par photo.
    pub shots: Vec<ImportShot>,
    /// Le choix photo par photo est ouvert.
    pub pick: bool,
    /// Éjecter la carte une fois l'import fini et vérifié.
    pub eject: bool,
    /// Ce qu'a donné l'éjection de la carte, à la fin de l'import.
    ejected: Arc<std::sync::Mutex<Option<Result<(), String>>>>,
    /// Le compte rendu du dernier import (photos, dossier, éjection).
    pub report: String,
    /// Les cartes montées, cherchées en arrière‑plan toutes les deux secondes tant que
    /// la fenêtre est ouverte (sous Windows, un lecteur réseau absent peut faire
    /// attendre plusieurs secondes).
    cards: Vec<PathBuf>,
    cards_at: Option<std::time::Instant>,
    cards_job: Option<std::sync::mpsc::Receiver<Vec<PathBuf>>>,
    /// Les aperçus des prises, lus en arrière‑plan à l'ouverture du choix.
    previews: HashMap<PathBuf, TextureHandle>,
    previews_job: Option<std::sync::mpsc::Receiver<Preview>>,
}

/// L'aperçu d'une prise de la source : son fichier, sa taille et ses pixels RGB.
type Preview = (PathBuf, [usize; 2], Vec<u8>);

/// Ce qu'on lit des photos d'une source : ses jours de prise de vue et ses prises.
type SourceContents = (Vec<(String, usize)>, Vec<ImportShot>);

/// Une prise de la source : ses fichiers (le RAW et son JPEG), son jour, et si elle est
/// à importer.
#[derive(Clone, Debug)]
pub struct ImportShot {
    pub paths: Vec<PathBuf>,
    pub day: String,
    pub on: bool,
}

impl ImportShot {
    /// Le fichier qui la montre : le RAW s'il y en a un (son aperçu est intégré).
    pub fn main(&self) -> &Path {
        self.paths
            .iter()
            .find(|p| is_raw(p))
            .unwrap_or(&self.paths[0])
    }
}

/// Les prises des photos, dans l'ordre de prise de vue : les fichiers d'un même dossier
/// et d'un même nom sans extension ensemble.
fn shots_of(entries: &[luma_library::Entry]) -> Vec<ImportShot> {
    let mut sorted: Vec<&luma_library::Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| (&a.date, &a.path).cmp(&(&b.date, &b.path)));
    let mut index: HashMap<_, usize> = HashMap::new();
    let mut shots: Vec<ImportShot> = Vec::new();
    for e in sorted {
        let key = luma_library::import::shot(&e.path);
        match index.get(&key) {
            Some(&k) => shots[k].paths.push(e.path.clone()),
            None => {
                index.insert(key, shots.len());
                shots.push(ImportShot {
                    paths: vec![e.path.clone()],
                    day: e.date.get(..10).unwrap_or(&e.date).to_owned(),
                    on: true,
                });
            }
        }
    }
    shots
}

impl Import {
    /// Les jours à importer : vide si tous le sont.
    pub fn chosen_days(&self) -> Vec<String> {
        if self.days.iter().all(|d| d.2) {
            return Vec::new();
        }
        self.days
            .iter()
            .filter(|d| d.2)
            .map(|d| d.0.clone())
            .collect()
    }

    /// Le jour est‑il choisi ?
    fn day_on(&self, day: &str) -> bool {
        self.days.iter().all(|d| d.0 != day || d.2)
    }

    /// Les photos laissées sur la carte : les prises décochées.
    pub fn skipped(&self) -> Vec<PathBuf> {
        self.shots
            .iter()
            .filter(|s| !s.on)
            .flat_map(|s| s.paths.iter().cloned())
            .collect()
    }

    /// Le nombre de photos à importer : celles des jours choisis, moins celles laissées
    /// sur la carte.
    pub fn chosen(&self) -> usize {
        if self.days.is_empty() {
            self.found
        } else if self.shots.is_empty() {
            self.days.iter().filter(|d| d.2).map(|d| d.1).sum()
        } else {
            self.shots
                .iter()
                .filter(|s| s.on && self.day_on(&s.day))
                .map(|s| s.paths.len())
                .sum()
        }
    }

    /// Les jours de la source, quand ils sont lus.
    fn receive_days(&mut self) {
        if let Some(rx) = &self.days_job
            && let Ok((days, shots)) = rx.try_recv()
        {
            self.days = days.into_iter().map(|(d, n)| (d, n, true)).collect();
            self.shots = shots;
            self.days_job = None;
        }
    }
}

impl Default for Import {
    fn default() -> Self {
        // Le dossier Images de l'utilisateur, s'il existe.
        let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
        let dest = home
            .map(|h| PathBuf::from(h).join("Pictures"))
            .filter(|d| d.is_dir() && cfg!(any(target_os = "macos", windows)));
        Import {
            source: None,
            dest,
            folders: "{année}/{année}-{mois}-{jour}".into(),
            name: String::new(),
            backup: None,
            found: 0,
            days: Vec::new(),
            days_job: None,
            shots: Vec::new(),
            pick: false,
            eject: false,
            ejected: Arc::default(),
            report: String::new(),
            cards: Vec::new(),
            cards_at: None,
            cards_job: None,
            previews: HashMap::new(),
            previews_job: None,
        }
    }
}

/// Le diaporama dans l'app : depuis quand la photo est montrée, et la pause.
struct Playing {
    since: std::time::Instant,
    paused: bool,
    /// Quand l'image de la photo affichée est arrivée (le début de sa transition).
    arrived: Option<std::time::Instant>,
    /// Le rang de la photo dans le diaporama (le zoom lent change de côté une fois sur
    /// deux).
    count: usize,
    /// La photo qui part, le temps de la transition.
    leaving: Option<Leaving>,
}

/// L'image de la photo qui part, gardée le temps de la transition.
struct Leaving {
    texture: TextureHandle,
    size: [usize; 2],
    arrived: std::time::Instant,
    count: usize,
}

/// L'épreuvage à l'écran : le profil d'imprimante choisi, l'intention de rendu et la
/// table qui en découle. Une vue, pas un réglage : rien n'est écrit dans le XMP.
#[derive(Default)]
struct Proofing {
    on: bool,
    /// L'alerte de gamut : les couleurs que l'imprimante ne saura pas rendre.
    gamut: bool,
    path: Option<PathBuf>,
    description: String,
    profile: Option<luma_color::profile::Profile>,
    relative: bool,
    /// Ce que l'écran montre du papier et de l'encre noire.
    media: luma_color::proof::Media,
    table: Option<Arc<luma_color::proof::Proof>>,
    error: Option<&'static str>,
    version: u64,
}

impl Proofing {
    fn active(&self) -> Option<crate::worker::Proofed> {
        let table = self.table.clone().filter(|_| self.on || self.gamut)?;
        Some(crate::worker::Proofed {
            table,
            simulate: self.on,
            gamut: self.gamut,
        })
    }

    fn intent(&self) -> luma_color::profile::Intent {
        if self.relative {
            luma_color::profile::Intent::Relative
        } else {
            luma_color::profile::Intent::Perceptual
        }
    }

    /// Calcule la table (quelques dizaines de millisecondes).
    fn build(&mut self) {
        self.version += 1;
        self.table = None;
        if let Some(p) = &self.profile {
            match luma_color::proof::Proof::with_media(p, self.intent(), self.media) {
                Ok(t) => {
                    self.table = Some(Arc::new(t));
                    self.error = None;
                }
                Err(_) => {
                    self.error = Some("Ce profil ne sait pas convertir vers l'imprimante");
                    self.on = false;
                    self.gamut = false;
                }
            }
        }
    }
}

pub struct Studio {
    worker: Worker,
    folder: Vec<PathBuf>,
    current: Option<usize>,
    photo: Option<Arc<Photo>>,
    loading: Option<PathBuf>,
    pub settings: Settings,
    history: Vec<(String, Settings)>,
    history_pos: usize,
    clipboard: Option<Settings>,
    /// La fenêtre « Coller une partie des réglages » et les groupes cochés (gardés
    /// d'une fois sur l'autre).
    paste_open: bool,
    /// La fenêtre « Renommer », son modèle (gardé d'une fois sur l'autre) et l'aperçu
    /// des nouveaux noms pour ce modèle.
    rename_open: bool,
    print_open: bool,
    /// La mise en page de l'impression (gardée d'une fois sur l'autre).
    pub print_layout: luma_render::print::Layout,
    /// Convertir les pages vers le profil de l'imprimante (celui de l'épreuvage).
    pub print_convert: bool,
    /// Le PDF en cours : son résultat, les photos posées, leur nombre.
    printing: Option<Printing>,
    outputs_open: bool,
    /// L'ordre des photos du livre, choisi dans « Pages du livre » (vide : celui du film).
    pub book_order: Vec<PathBuf>,
    /// La fenêtre « Pages du livre » est ouverte.
    pub book_pages_open: bool,
    /// Le diaporama : ses réglages (pour l'app comme pour l'export), et la lecture.
    pub show: luma_render::slideshow::Slideshow,
    playing: Option<Playing>,
    import_open: bool,
    pub import: Import,
    /// Les raccourcis clavier, et leur fichier (sinon celui du dossier de LÜMA).
    pub keymap: crate::keys::Keymap,
    keys_path: Option<PathBuf>,
    keys_open: bool,
    /// La commande dont la fenêtre des raccourcis attend les nouvelles touches.
    keys_listening: Option<&'static str>,
    /// Ce qu'a fait le dernier changement (une touche reprise à une autre commande).
    keys_note: Option<String>,
    /// Les panneaux masqués (Tab) : la photo prend toute la place.
    pub hide_panels: bool,
    /// Les espaces de travail enregistrés (après ceux de toujours), leur fichier, et le
    /// nom du prochain à enregistrer.
    pub workspaces: Vec<crate::workspaces::Workspace>,
    workspaces_path: Option<PathBuf>,
    workspace_name: String,
    /// La galerie web.
    pub gallery: luma_render::gallery::Gallery,
    /// Le livre photo.
    pub book: luma_render::book::Book,
    rename_template: String,
    /// Le nom du prochain instantané, en cours de saisie.
    snapshot_name: String,
    /// La recherche dans les films (nom, description), sans tenir compte des accents.
    film_search: String,
    rename_preview: Option<(String, Result<luma_render::rename::Plan, String>)>,
    /// Les champs des informations (IPTC) en cours de saisie, et les photos qu'ils visent.
    info_edit: HashMap<&'static str, String>,
    info_for: Vec<PathBuf>,
    /// Les modèles de métadonnées, et leur fichier.
    templates: Vec<luma_project::xmp::Template>,
    templates_path: Option<PathBuf>,
    template_name: String,
    /// Les réglages par défaut par boîtier et par ISO, et le film donné à l'arrivée.
    defaults: luma_project::defaults::Defaults,
    defaults_path: Option<PathBuf>,
    /// La fenêtre des groupes sert à synchroniser la sélection, pas à coller.
    syncing: bool,
    /// Les photos du lot dont la balance automatique est attendue.
    auto_pending: usize,
    paste_groups: Vec<luma_project::paste::Group>,
    texture: Option<TextureHandle>,
    image_size: [usize; 2],
    histogram: [[u32; 256]; 3],
    /// L'instrument affiché à la place de l'histogramme (Maj+J), et son image.
    scope: Scope,
    scope_texture: Option<TextureHandle>,
    /// La mesure au point : l'aperçu affiché (sans l'écrêtage peint) et l'endroit
    /// survolé de la photo (0 à 1), s'il y en a un.
    probe_image: ([usize; 2], Vec<u8>),
    probe: Option<[f32; 2]>,
    generation: u64,
    shown: u64,
    /// La première génération demandée pour la photo ouverte : les rendus plus anciens,
    /// encore en route, sont ceux de la photo d'avant.
    fresh_from: u64,
    last_request: Option<Request>,
    render_ms: f32,
    render_gpu: bool,
    /// Les aperçus notés dans le journal (iPhone et iPad).
    renders_logged: u32,
    pub before: bool,
    clipping: bool,
    proofing: Proofing,
    zoom: bool,
    compare: Option<Compare>,
    /// Les rafales et séries d'exposition du film, repliées derrière leur première photo.
    stacking: bool,
    /// L'empreinte de chaque photo, prise sur sa vignette (doublons).
    prints: HashMap<PathBuf, u64>,
    /// La vue des doublons : leurs groupes, à la place du film.
    duplicates: Option<Vec<Vec<PathBuf>>>,
    /// La carte des photos géolocalisées, à la place de la grille : sa vue (cadrée sur
    /// les photos à la première image).
    map: Option<Option<luma_library::map::View>>,
    /// Les photos d'un lieu de la carte, à la place du film.
    place: Option<Vec<PathBuf>>,
    tiles: crate::tiles::Tiles,
    /// Les groupes de photos dessinés sur la carte : leur place et leur nombre.
    map_groups: Vec<(egui::Pos2, usize)>,
    stacks: Vec<luma_library::stacks::Stack>,
    /// La pile de chaque photo (sa place dans `stacks`).
    stack_of: HashMap<PathBuf, usize>,
    /// Les piles dépliées, par leur première photo.
    unfolded: std::collections::HashSet<PathBuf>,
    compare_tag: u64,
    /// Le tri en plein écran (F) : la photo seule, notée au clavier.
    culling: bool,
    /// L'écran est étroit (iPhone) : une interface plus simple.
    compact: bool,
    /// Au tri, passer à la suivante après chaque note, drapeau ou libellé (A).
    auto_advance: bool,
    thumbs: HashMap<PathBuf, TextureHandle>,
    /// La dernière vérification de la mémoire laissée par le système (iPhone, iPad).
    memory_check: f64,
    /// Les photos importées depuis Photos déjà vues (voir `watch_imports`).
    imports_seen: usize,
    imports_failed: usize,
    /// Les exports envoyés à la Pellicule (iPhone, iPad), et les réponses de Photos déjà
    /// annoncées : ajoutées, refusées.
    library_sent: usize,
    library_saved: usize,
    library_refused: usize,
    /// Les touchers reçus du système (diagnostic sur iPhone) et le dernier.
    touches: usize,
    last_touch: Option<egui::Pos2>,
    /// Ce qu'a donné la dernière demande d'import (diagnostic sur iPhone).
    import_result: Option<String>,
    /// Jusqu'à quand guetter les photos d'un import Photos (il en arrive une à une).
    imports_until: f64,
    /// Le bouton « Importer des photos » (iPhone, iPad) est affiché : vrai là où le
    /// sélecteur de Photos existe ; les essais l'allument ailleurs.
    pub photos_button: bool,
    /// Combien de fois l'import depuis Photos a été demandé (pour les essais).
    pub import_asks: usize,
    /// Combien de fois l'import depuis l'app Fichiers a été demandé (pour les essais).
    pub files_asks: usize,
    /// Les vignettes libérées faute de mémoire : redemandées quand elles réapparaissent.
    evicted: std::collections::HashSet<PathBuf>,
    /// Les vignettes affichées sans image pendant cette image de l'interface.
    shown_missing: std::cell::RefCell<Vec<PathBuf>>,
    status: String,
    xmp_due: Option<f64>,
    grading_tab: usize,
    hsl_tab: usize,
    /// L'onglet des courbes à points : luminosité, rouge, vert, bleu.
    curve_channel: usize,
    pub module: Module,
    meta: HashMap<PathBuf, Meta>,
    min_rating: u8,
    flag_filter: FlagFilter,
    /// Le mot‑clé choisi dans la Bibliothèque : seules ses photos s'affichent.
    pub keyword_filter: Option<String>,
    /// Le mot‑clé en cours de saisie.
    keyword_input: String,
    /// La photo relue depuis son RAW parce que les corrections du boîtier sont coupées
    /// (son doublon les porte) : demandé une fois.
    original_asked: Option<PathBuf>,
    /// La photothèque : le catalogue de toutes les photos vues, son fichier, et la mise
    /// à jour en cours (un dossier relu dans un fil à part, sans bloquer l'interface).
    catalog: luma_library::Catalog,
    catalog_path: Option<PathBuf>,
    indexing: Option<std::sync::mpsc::Receiver<luma_library::Catalog>>,
    pending_index: Vec<PathBuf>,
    /// Le dossier surveillé, et quand le relever.
    watch: Option<luma_library::watch::Watch>,
    watch_next: f64,
    /// Les photos arrivées depuis le début de la surveillance.
    watch_count: usize,
    /// Chaque photo arrivée est ouverte.
    watch_follow: bool,
    /// Les dossiers et les collections du catalogue, avec leur nombre de photos
    /// (recalculés quand le catalogue change, pas à chaque image).
    catalog_folders: Vec<(PathBuf, usize)>,
    catalog_collections: Vec<(String, usize)>,
    pub view: View,
    catalog_search: String,
    pub export_options: ExportOptions,
    /// Les préréglages d'export (relus à l'ouverture de la fenêtre), ceux qui sont
    /// cochés, et si les réglages de la fenêtre sont exportés eux aussi.
    export_presets: luma_render::export::Presets,
    export_targets: Vec<String>,
    export_current: bool,
    preset_name: String,
    export_open: bool,
    export_all: bool,
    crop_tool: Option<CropTool>,
    /// Le masque sélectionné (ses poignées sont à l'écran).
    pub selected_mask: Option<usize>,
    /// La partie combinée en cours de réglage : (masque, partie) ; aucune pour la forme principale.
    pub selected_part: Option<(usize, usize)>,
    /// Le guide de composition du recadrage, et son quart de tour (0 à 3).
    pub crop_guide: crop::Guide,
    pub crop_turn: u8,
    /// Le redressement guidé : actif, les lignes tracées (coordonnées de la photo avant
    /// la Transformation), et le point de départ de celle qu'on trace.
    pub guiding: bool,
    pub guides: Vec<luma_image::lens::Guide>,
    guide_start: Option<egui::Pos2>,
    /// La couche de sortie montrée par le mélangeur de couches (0 rouge, 1 vert, 2 bleu).
    mixer_tab: usize,
    /// L'outil de retouche actif (Q) : un clic sur la photo pose une retouche.
    pub retouch: Option<luma_project::SpotKind>,
    /// La fluidité : le pinceau choisi (pousser, gonfler, dégonfler), sa taille (en
    /// fraction de la largeur) et sa force.
    pub liquify: Option<luma_project::WarpKind>,
    pub liquify_radius: f32,
    pub liquify_strength: f32,
    /// La retouche choisie.
    pub selected_spot: Option<usize>,
    /// Le rayon des nouvelles retouches, en fraction de la largeur.
    pub spot_radius: f32,
    /// La vue des taches : les poussières du capteur ressortent en points sombres.
    pub show_dust: bool,
    /// Les poussières demandées au prochain rendu.
    dust_pending: bool,
    brush: masking::Brush,
    /// Le masque en rouge (touche O) : le masque dessiné et sa texture.
    show_mask: bool,
    /// La couleur et l'opacité de la zone affichée (O).
    pub mask_style: masking::Overlay,
    mask_overlay: Option<(Mask, TextureHandle)>,
    /// La copie réduite de la base envoyée avec la dernière image, quand un masque a
    /// une plage.
    scene_map: Option<Arc<luma_core::RgbImage>>,
    /// L'outil choisi dans le rail.
    pub tool: Tool,
    /// Les photos choisies dans le film ou la grille (⌘‑clic, Maj‑clic), pour la fusion.
    pub selected: BTreeSet<usize>,
    pub merge_stack: bool,
    pub stack_mode: luma_merge::StackMode,
    stack_align: bool,
    projection: luma_merge::Projection,
    auto_crop: bool,
    /// Panorama : les bords étirés pour remplir le rectangle.
    pub pano_warp: bool,
    merge_full: bool,
    merging: Option<(f32, String, luma_core::Cancel)>,
    /// Films : le film choisi (index dans les préréglages, puis dans `luts`), son
    /// intensité, et les réglages d'avant.
    preset: Option<(usize, f32, Settings)>,
    /// Mes pellicules : les films mesurés et les LUT importées (noms des `.cube` chargés).
    luts: Vec<String>,
    /// Une mesure de pellicule est en cours.
    measuring: bool,
    /// Mes préréglages : enregistrés depuis LÜMA ou importés (fichiers `.xmp`).
    user_presets: Vec<(String, Settings)>,
    /// Le nom du préréglage à enregistrer (champ du panneau Films).
    new_preset_name: String,
    /// Ce qui a changé dans chaque photo du dossier (vignettes, étiquette sous la photo).
    edits: HashMap<PathBuf, presets::Summary>,
    /// La marque : le fond (l'éclipse), l'anneau, le logo avec son nom.
    brand: Option<Brand>,
    /// LÜMA Premium : l'essai, l'abonnement, la clé.
    pub licence: crate::licence::Licence,
    /// L'écran Premium est ouvert (il l'est toujours quand l'essai est fini).
    pub premium_open: bool,
    /// La mise en page pour le doigt (iPhone, iPad) ; réglable pour les essais.
    pub touch_layout: bool,
    previews: Vec<Option<TextureHandle>>,
    previews_tag: u64,
    previews_for: Option<PathBuf>,
    /// La pipette de balance des blancs (W) attend un clic sur la photo.
    eyedropper: bool,
    /// L'outil de ciblage direct du mélangeur : glisser sur une couleur de la photo.
    targeting: bool,
    /// Le geste de ciblage en cours : les bandes du mélangeur saisies au point de départ
    /// et leurs poids (`Some(None)` : une couleur grise, rien à régler).
    target_bands: Option<Option<[(usize, f32); 2]>>,
    /// Une petite copie de l'image affichée (au plus 256 px), pour lire la couleur sous
    /// le doigt : taille et RGBA.
    pick_map: ([usize; 2], Vec<u8>),
    /// Le nom de la dernière balance personnalisée (« Auto » ou « Personnalisée »).
    wb_label: &'static str,
}

/// Une copie réduite (au plus 256 px de côté) d'une image RGBA, pour la lecture des
/// couleurs sous le doigt.
fn pick_map(size: [usize; 2], rgba: &[u8]) -> ([usize; 2], Vec<u8>) {
    let [w, h] = size;
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return ([0, 0], Vec::new());
    }
    let k = (w.max(h) as f32 / 256.0).max(1.0);
    let (sw, sh) = (
        ((w as f32 / k) as usize).max(1),
        ((h as f32 / k) as usize).max(1),
    );
    let mut out = Vec::with_capacity(sw * sh * 4);
    for y in 0..sh {
        for x in 0..sw {
            let (px, py) = (
                ((x as f32 + 0.5) * k) as usize,
                ((y as f32 + 0.5) * k) as usize,
            );
            let i = (py.min(h - 1) * w + px.min(w - 1)) * 4;
            out.extend_from_slice(&rgba[i..i + 4]);
        }
    }
    ([sw, sh], out)
}

/// La couleur d'un point (coordonnées 0 à 1) de l'aperçu affiché, en sRGB 8 bits.
fn probe_color((size, rgba): &([usize; 2], Vec<u8>), nx: f32, ny: f32) -> Option<[u8; 3]> {
    let [w, h] = *size;
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return None;
    }
    if !(0.0..1.0).contains(&nx) || !(0.0..1.0).contains(&ny) {
        return None;
    }
    let (x, y) = ((nx * w as f32) as usize, (ny * h as f32) as usize);
    let i = (y.min(h - 1) * w + x.min(w - 1)) * 4;
    Some([rgba[i], rgba[i + 1], rgba[i + 2]])
}

/// La couleur moyenne autour d'un point (coordonnées 0 à 1) de la copie réduite.
fn pick_color((size, rgba): &([usize; 2], Vec<u8>), nx: f32, ny: f32) -> Option<[u8; 3]> {
    let [w, h] = *size;
    if w == 0 || !(0.0..=1.0).contains(&nx) || !(0.0..=1.0).contains(&ny) {
        return None;
    }
    let (cx, cy) = ((nx * w as f32) as isize, (ny * h as f32) as isize);
    let (mut sum, mut n) = ([0u32; 3], 0u32);
    for y in cy - 1..=cy + 1 {
        for x in cx - 1..=cx + 1 {
            if (0..w as isize).contains(&x) && (0..h as isize).contains(&y) {
                let i = (y as usize * w + x as usize) * 4;
                for c in 0..3 {
                    sum[c] += u32::from(rgba[i + c]);
                }
                n += 1;
            }
        }
    }
    (n > 0).then(|| sum.map(|v| (v / n) as u8))
}

/// L'aperçu pendant un geste : au plus 1024 px sur iPhone et iPad, 1536 ailleurs (et
/// jamais plus que l'écran). Quatre fois moins de pixels qu'un aperçu d'iPad Pro.
fn interactive_size(viewport_px: f32) -> Size {
    let cap = if cfg!(target_os = "ios") {
        1024.0
    } else {
        1536.0
    };
    let side = ((viewport_px / 512.0).ceil().max(1.0) * 512.0).min(cap);
    Size::Fit(side as usize)
}

impl Studio {
    /// L'histogramme du dernier rendu (rouge, vert, bleu).
    pub fn histogram(&self) -> [[u32; 256]; 3] {
        self.histogram
    }

    /// La mesure au point : la couleur sous le pointeur, s'il survole la photo.
    pub fn measured(&self) -> Option<[u8; 3]> {
        self.probe
            .and_then(|[x, y]| probe_color(&self.probe_image, x, y))
    }

    /// L'instrument affiché, et s'il est rendu (son image arrivée avec l'aperçu).
    pub fn scope(&self) -> (Scope, bool) {
        (self.scope, self.scope_texture.is_some())
    }

    /// Une image de photo est affichée.
    pub fn has_image(&self) -> bool {
        self.texture.is_some()
    }

    /// La copie réduite de la base du dernier rendu, s'il en a joint une.
    pub fn scene_map(&self) -> Option<&luma_core::RgbImage> {
        self.scene_map.as_deref()
    }

    pub fn new(ctx: &egui::Context) -> Self {
        crate::style::apply(ctx);
        crate::set_screen(ctx);
        let repaint_ctx = ctx.clone();
        let mut studio = Self {
            worker: Worker::new(move || repaint_ctx.request_repaint()),
            folder: Vec::new(),
            current: None,
            photo: None,
            loading: None,
            settings: Settings::default(),
            history: Vec::new(),
            history_pos: 0,
            clipboard: None,
            paste_open: false,
            rename_open: false,
            print_open: false,
            print_layout: Default::default(),
            print_convert: true,
            printing: None,
            outputs_open: false,
            book_order: Vec::new(),
            book_pages_open: false,
            show: Default::default(),
            playing: None,
            import_open: false,
            import: Default::default(),
            keymap: crate::dialogs::app_dir()
                .map(|d| crate::keys::Keymap::load(&d.join("Raccourcis.txt")))
                .unwrap_or_default(),
            keys_path: None,
            keys_open: false,
            keys_listening: None,
            keys_note: None,
            hide_panels: false,
            workspaces: crate::dialogs::app_dir()
                .map(|d| crate::workspaces::load(&d.join("Espaces de travail.txt")))
                .unwrap_or_default(),
            workspaces_path: None,
            workspace_name: String::new(),
            gallery: Default::default(),
            book: Default::default(),
            rename_template: "{date}_{séquence}".into(),
            snapshot_name: String::new(),
            film_search: String::new(),
            rename_preview: None,
            info_edit: HashMap::new(),
            info_for: Vec::new(),
            templates: crate::dialogs::presets_dir()
                .map(|d| load_templates(&d.join(TEMPLATES_FILE)))
                .unwrap_or_default(),
            templates_path: None,
            template_name: String::new(),
            defaults: crate::dialogs::presets_dir()
                .map(|d| luma_project::defaults::Defaults::load(&d.join(DEFAULTS_FILE)))
                .unwrap_or_default(),
            defaults_path: None,
            syncing: false,
            auto_pending: 0,
            paste_groups: luma_project::paste::default_groups(),
            texture: None,
            image_size: [0, 0],
            histogram: [[0; 256]; 3],
            scope: Scope::default(),
            scope_texture: None,
            probe_image: ([0, 0], Vec::new()),
            probe: None,
            generation: 0,
            shown: 0,
            fresh_from: 0,
            last_request: None,
            render_ms: 0.0,
            render_gpu: false,
            renders_logged: 0,
            before: false,
            clipping: false,
            proofing: Proofing::default(),
            zoom: false,
            compare: None,
            stacking: true,
            prints: HashMap::new(),
            duplicates: None,
            map: None,
            place: None,
            tiles: Default::default(),
            map_groups: Vec::new(),
            stacks: Vec::new(),
            stack_of: HashMap::new(),
            unfolded: Default::default(),
            compare_tag: 0,
            culling: false,
            compact: false,
            auto_advance: true,
            thumbs: HashMap::new(),
            memory_check: 0.0,
            imports_seen: 0,
            imports_failed: 0,
            library_sent: 0,
            library_saved: 0,
            library_refused: 0,
            touches: 0,
            last_touch: None,
            import_result: None,
            imports_until: 0.0,
            photos_button: luma_apple::photos::AVAILABLE,
            import_asks: 0,
            files_asks: 0,
            evicted: Default::default(),
            shown_missing: Default::default(),
            // Vide : l'accueil (traduit à l'affichage, voir `status_text`).
            status: String::new(),
            xmp_due: None,
            grading_tab: 0,
            hsl_tab: 0,
            curve_channel: 0,
            module: Module::Develop,
            meta: HashMap::new(),
            min_rating: 0,
            flag_filter: FlagFilter::All,
            keyword_filter: None,
            keyword_input: String::new(),
            original_asked: None,
            catalog: luma_library::Catalog::default(),
            catalog_path: None,
            indexing: None,
            pending_index: Vec::new(),
            watch: None,
            watch_next: 0.0,
            watch_count: 0,
            watch_follow: true,
            catalog_folders: Vec::new(),
            catalog_collections: Vec::new(),
            view: View::Folder,
            catalog_search: String::new(),
            export_options: ExportOptions::default(),
            export_presets: luma_render::export::Presets::default(),
            export_targets: Vec::new(),
            export_current: true,
            preset_name: String::new(),
            export_open: false,
            export_all: false,
            crop_tool: None,
            selected_mask: None,
            selected_part: None,
            crop_guide: crop::Guide::default(),
            crop_turn: 0,
            guiding: false,
            guides: Vec::new(),
            guide_start: None,
            mixer_tab: 0,
            retouch: None,
            liquify: None,
            liquify_radius: 0.06,
            liquify_strength: 50.0,
            selected_spot: None,
            spot_radius: 0.02,
            show_dust: false,
            dust_pending: false,
            brush: masking::Brush::default(),
            show_mask: false,
            mask_style: masking::Overlay::default(),
            mask_overlay: None,
            scene_map: None,
            tool: Tool::Light,
            selected: BTreeSet::new(),
            merge_stack: true,
            stack_mode: luma_merge::StackMode::Mean,
            stack_align: true,
            projection: luma_merge::Projection::Cylindrical,
            auto_crop: true,
            pano_warp: false,
            merge_full: true,
            merging: None,
            preset: None,
            luts: {
                if let Some(d) = crate::dialogs::films_dir() {
                    luma_image::cube::load_dir(&d);
                }
                luma_image::cube::names()
            },
            measuring: false,
            user_presets: crate::dialogs::presets_dir()
                .map(|d| load_user_presets(&d))
                .unwrap_or_default(),
            new_preset_name: String::new(),
            edits: HashMap::new(),
            brand: None,
            licence: crate::licence::Licence::load(),
            premium_open: false,
            touch_layout: crate::touch(),
            previews: Vec::new(),
            previews_tag: 0,
            previews_for: None,
            eyedropper: false,
            targeting: false,
            target_bands: None,
            pick_map: ([0, 0], Vec::new()),
            wb_label: "Personnalisée",
        };
        if let Some(path) = luma_library::default_path() {
            // Une sauvegarde du catalogue par semaine, sans attendre.
            if luma_library::backup::due(&path, 7) {
                let file = path.clone();
                std::thread::spawn(move || luma_library::backup::backup(&file));
            }
            studio.use_catalog(path);
        }
        studio
    }

    /// Ouvre un fichier (et son dossier dans le film) ou un dossier ; le dossier entre
    /// dans la photothèque (ou y est remis à jour), en arrière‑plan.
    pub fn open_path(&mut self, path: &Path) {
        let (dir, file) = if path.is_dir() {
            (path.to_path_buf(), None)
        } else {
            (
                path.parent().unwrap_or(Path::new(".")).to_path_buf(),
                Some(path.to_path_buf()),
            )
        };
        let mut photos = scan(&dir);
        if let Some(f) = &file
            && !photos.contains(f)
        {
            photos.push(f.clone());
        }
        self.view = View::Folder;
        let empty = if luma_apple::photos::AVAILABLE {
            // Le bouton « Importer des photos » est juste en dessous.
            tr("Vos photos apparaîtront ici.").to_owned()
        } else {
            trf("Aucune photo dans {dir}", &[("dir", &dir.display())])
        };
        self.show_photos(photos, file, empty);
        self.index_folder(dir);
    }

    /// Remplit le film et la grille avec ces photos, et ouvre `file` (sinon la
    /// première).
    fn show_photos(&mut self, photos: Vec<PathBuf>, file: Option<PathBuf>, empty: String) {
        // Les réglages en attente vont à la photo qui les porte, avant que le film change.
        self.flush_xmp();
        self.current = None;
        self.selected.clear();
        self.place = None;
        if self.map.is_some() {
            self.map = Some(None);
        }
        self.folder = photos;
        for p in &self.folder {
            if !self.thumbs.contains_key(p) {
                self.worker.thumbnail(p.clone());
            }
            self.meta.insert(p.clone(), read_meta(p));
            let edits = read_sidecar(p)
                .map(|s| presets::summary(&s))
                .unwrap_or_default();
            self.edits.insert(p.clone(), edits);
        }
        let index = file
            .and_then(|f| self.folder.iter().position(|p| *p == f))
            .or(if self.folder.is_empty() {
                None
            } else {
                Some(0)
            });
        self.find_stacks();
        match index {
            Some(i) => self.select(i),
            None => {
                // Plus de photo : l'image de la précédente ne reste pas affichée.
                self.photo = None;
                self.texture = None;
                self.status = empty;
            }
        }
    }

    /// La photothèque de ce fichier (les tests en prennent une à eux).
    pub fn use_catalog(&mut self, path: PathBuf) {
        self.catalog = luma_library::Catalog::load(&path);
        // iPhone, iPad : après une mise à jour, le dossier de l'app a changé de nom ; ses
        // photos le suivent (sans quoi « Documents » apparaît deux fois).
        if let Some(home) = std::env::var_os("HOME") {
            let before = self.catalog.len();
            let followed = self.catalog.follow_container(Path::new(&home));
            if followed > 0 || self.catalog.len() != before {
                let _ = self.catalog.save(&path);
            }
        }
        self.catalog_path = Some(path);
        self.refresh_catalog_lists();
    }

    /// Le catalogue, en lecture.
    pub fn catalog(&self) -> &luma_library::Catalog {
        &self.catalog
    }

    /// Le catalogue, à modifier (les essais).
    pub fn catalog_mut(&mut self) -> &mut luma_library::Catalog {
        &mut self.catalog
    }

    /// Une mise à jour du catalogue est‑elle en cours ?
    pub fn is_indexing(&self) -> bool {
        self.indexing.is_some() || !self.pending_index.is_empty()
    }

    /// Ajoute un dossier au catalogue, ou le remet à jour, dans un fil à part ; puis
    /// l'enregistre. Une seule mise à jour à la fois, les suivantes attendent leur tour.
    fn index_folder(&mut self, dir: PathBuf) {
        let Some(file) = self.catalog_path.clone() else {
            return;
        };
        if self.indexing.is_some() {
            if !self.pending_index.contains(&dir) {
                self.pending_index.push(dir);
            }
            return;
        }
        let mut catalog = self.catalog.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            catalog.add_folder(&dir, false);
            let _ = catalog.save(&file);
            let _ = tx.send(catalog);
            crate::wake();
        });
        self.indexing = Some(rx);
    }

    /// Surveille un dossier : il s'ouvre, et les photos qui y arriveront entreront dans
    /// la photothèque et dans le film.
    pub fn watch_folder(&mut self, dir: &Path) {
        self.open_path(dir);
        self.watch = Some(luma_library::watch::Watch::new(dir));
        self.watch_count = 0;
    }

    pub fn stop_watching(&mut self) {
        self.watch = None;
        luma_apple::tether::stop();
    }

    /// Le dossier surveillé, et le nombre de photos arrivées.
    pub fn watching(&self) -> Option<(&Path, usize)> {
        self.watch.as_ref().map(|w| (w.dir(), self.watch_count))
    }

    /// Ouvrir chaque photo arrivée, ou seulement l'ajouter au film.
    pub fn set_watch_follow(&mut self, follow: bool) {
        self.watch_follow = follow;
    }

    /// Relève le dossier surveillé (toutes les secondes ; les essais l'appellent
    /// eux‑mêmes) : les photos complètes entrent dans la photothèque et, si le dossier
    /// est à l'écran, dans le film ; la dernière est ouverte.
    pub fn poll_watch(&mut self) {
        let Some(w) = &mut self.watch else { return };
        let new = w.poll();
        if new.is_empty() {
            return;
        }
        let dir = w.dir().to_path_buf();
        self.watch_count += new.len();
        self.index_folder(dir.clone());
        let shown = self.view == View::Folder
            && self
                .folder
                .first()
                .and_then(|p| p.parent())
                .is_none_or(|d| d == dir);
        if !shown {
            return;
        }
        let current = self.current_path().map(Path::to_path_buf);
        // La sélection suit ses photos, pas leurs places dans le film.
        let selected: Vec<PathBuf> = self
            .selected
            .iter()
            .filter_map(|&i| self.folder.get(i).cloned())
            .collect();
        // Le film d'import, aux photos arrivées qui n'ont pas encore de réglages.
        if let Some(settings) = self.import_settings() {
            for p in &new {
                if read_sidecar(p).is_none() {
                    let _ = write_sidecar(p, &settings, &Meta::default());
                }
            }
        }
        for p in &new {
            if self.folder.contains(p) {
                continue;
            }
            self.folder.push(p.clone());
            self.worker.thumbnail(p.clone());
            self.meta.insert(p.clone(), read_meta(p));
            let edits = read_sidecar(p)
                .map(|s| presets::summary(&s))
                .unwrap_or_default();
            self.edits.insert(p.clone(), edits);
        }
        sort_photos(&mut self.folder);
        self.selected = selected
            .iter()
            .filter_map(|s| self.folder.iter().position(|p| p == s))
            .collect();
        let follow = self.watch_follow || current.is_none();
        let target = if follow { new.last().cloned() } else { current };
        if let Some(i) = target.and_then(|t| self.folder.iter().position(|p| *p == t)) {
            if follow {
                // Une autre photo est peut‑être à cette place, désormais.
                self.current = None;
                self.select(i);
            } else {
                // Le film a changé d'ordre : la photo ouverte garde la sienne.
                self.current = Some(i);
            }
        }
    }

    /// Reprend le catalogue mis à jour, s'il est prêt.
    fn poll_catalog(&mut self) {
        let Some(rx) = &self.indexing else { return };
        match rx.try_recv() {
            Ok(catalog) => {
                self.catalog = catalog;
                self.indexing = None;
                self.refresh_catalog_lists();
                if !self.pending_index.is_empty() {
                    let dir = self.pending_index.remove(0);
                    self.index_folder(dir);
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.indexing = None,
        }
    }

    fn refresh_catalog_lists(&mut self) {
        self.find_stacks();
        let mut folders: BTreeMap<PathBuf, usize> = BTreeMap::new();
        for e in self.catalog.entries() {
            if let Some(d) = e.path.parent() {
                *folders.entry(d.to_path_buf()).or_default() += 1;
            }
        }
        self.catalog_folders = folders.into_iter().collect();
        self.catalog_collections = self
            .catalog
            .collections()
            .iter()
            .map(|c| {
                let n = self
                    .catalog
                    .collection_photos(&c.name)
                    .map_or(0, |v| v.len());
                (c.name.clone(), n)
            })
            .collect();
    }

    /// Montre une vue de la photothèque : toutes les photos, une collection ou une
    /// recherche, par date de prise de vue. Une recherche mal écrite le dit et ne
    /// change rien.
    pub fn show_view(&mut self, view: View) {
        let found: Result<Vec<PathBuf>, String> = match &view {
            View::Folder => return,
            View::All => Ok(self
                .catalog
                .by_date()
                .iter()
                .map(|e| e.path.clone())
                .collect()),
            View::Collection(name) => self
                .catalog
                .collection_photos(name)
                .map(|v| v.iter().map(|e| e.path.clone()).collect()),
            View::Search(text) => luma_library::Query::parse(text).map(|q| {
                self.catalog
                    .search(&q)
                    .iter()
                    .map(|e| e.path.clone())
                    .collect()
            }),
        };
        match found {
            Ok(photos) => {
                // Les photos d'un disque débranché ne s'affichent pas ; les copies
                // virtuelles, tant que leur original et leur XMP sont là.
                let photos = photos
                    .into_iter()
                    .filter(|p| luma_library::exists(p))
                    .collect();
                self.view = view;
                self.show_photos(
                    photos,
                    None,
                    tr("Aucune photo ne répond à cette recherche.").to_owned(),
                );
            }
            Err(_) => {
                let text = match &view {
                    View::Search(t) => t.as_str(),
                    _ => "",
                };
                self.status = trf("Recherche mal écrite : {text}", &[("text", &text)]);
            }
        }
    }

    /// La ligne d'état ; vide, l'accueil (dans la langue du moment).
    fn status_text(&self) -> String {
        if self.status.is_empty() && luma_apple::photos::AVAILABLE {
            tr("Choisissez vos photos dans Photos : RAW, ProRAW, HEIC, JPEG.").to_owned()
        } else if self.status.is_empty() {
            tr("Ouvrez une photo (RAW, JPEG, PNG, TIFF…) ou un dossier (Cmd+O), ou déposez-la dans la fenêtre.").to_owned()
        } else {
            self.status.clone()
        }
    }

    /// Crée une copie virtuelle de la photo affichée, avec ses réglages du moment, et
    /// l'ouvre ; elle vient juste après la photo et ses autres copies.
    pub fn create_copy(&mut self) -> Option<PathBuf> {
        let current = self.current_path()?.to_path_buf();
        let settings = if self.photo.is_some() && self.loading.is_none() {
            self.settings.clone()
        } else {
            read_sidecar(&current).unwrap_or_default()
        };
        self.flush_xmp();
        let copy = luma_project::copies::next(&current);
        if let Err(e) = write_sidecar(&copy, &settings, &Meta::default()) {
            self.status = trf("XMP non enregistré : {error}", &[("error", &e)]);
            return None;
        }
        let source = luma_project::copies::source(&current);
        let at = self
            .folder
            .iter()
            .rposition(|p| luma_project::copies::source(p) == source)
            .map_or(self.folder.len(), |i| i + 1);
        self.folder.insert(at, copy.clone());
        self.meta.insert(copy.clone(), Meta::default());
        self.edits.insert(copy.clone(), presets::summary(&settings));
        self.worker.thumbnail(copy.clone());
        // Les rangs choisis après la copie se décalent : la sélection repart de zéro.
        self.selected.clear();
        self.select(at);
        Some(copy)
    }

    /// Supprime la copie virtuelle affichée (son XMP) et revient à son original ; une
    /// photo ordinaire n'est jamais supprimée.
    pub fn delete_copy(&mut self) -> bool {
        let Some(copy) = self.current_path().map(Path::to_path_buf) else {
            return false;
        };
        if luma_project::copies::number(&copy).is_none() {
            return false;
        }
        // Rien ne doit réécrire le XMP de la copie une fois supprimé.
        self.xmp_due = None;
        let _ = std::fs::remove_file(luma_project::copies::sidecar(&copy));
        let source = luma_project::copies::source(&copy);
        self.folder.retain(|p| *p != copy);
        self.meta.remove(&copy);
        self.edits.remove(&copy);
        self.thumbs.remove(&copy);
        self.selected.clear();
        self.current = None;
        if let Some(i) = self
            .folder
            .iter()
            .position(|p| *p == source)
            .or((!self.folder.is_empty()).then_some(0))
        {
            self.select(i);
        } else {
            self.photo = None;
        }
        true
    }

    /// La photo de rang `i` dans le film.
    pub fn photo_at(&self, i: usize) -> Option<&Path> {
        self.folder.get(i).map(PathBuf::as_path)
    }

    /// La photo actuellement développée.
    pub fn current_path(&self) -> Option<&Path> {
        self.current
            .and_then(|i| self.folder.get(i))
            .map(PathBuf::as_path)
    }

    /// Les photos qui passent le filtre, dans l'ordre du dossier.
    /// Les photos du film, dans l'ordre.
    pub fn photos(&self) -> &[PathBuf] {
        &self.folder
    }

    pub fn visible(&self) -> Vec<usize> {
        if let Some(place) = &self.place {
            return place
                .iter()
                .filter_map(|p| self.folder.iter().position(|f| f == p))
                .collect();
        }
        if let Some(groups) = &self.duplicates {
            // Groupe par groupe, les doublons côte à côte.
            return groups
                .iter()
                .flatten()
                .filter_map(|p| self.folder.iter().position(|f| f == p))
                .collect();
        }
        (0..self.folder.len())
            .filter(|i| {
                let m = self.meta.get(&self.folder[*i]).cloned().unwrap_or_default();
                m.rating >= self.min_rating
                    && match self.flag_filter {
                        FlagFilter::All => true,
                        FlagFilter::Picks => m.flag == Flag::Pick,
                        FlagFilter::NotRejected => m.flag != Flag::Reject,
                    }
                    && self
                        .keyword_filter
                        .as_ref()
                        .is_none_or(|k| m.keywords.iter().any(|w| fold(w) == fold(k)))
                    && !self.folded_away(&self.folder[*i])
            })
            .collect()
    }

    /// La pile de la photo ouverte : la déplier, et fusionner une série d'exposition.
    fn stack_panel(&mut self, ui: &mut Ui, path: &Path) {
        let Some(&k) = self.stack_of.get(path) else {
            return;
        };
        let stack = &self.stacks[k];
        let n = stack.photos.len();
        let bracket = stack.kind == luma_library::stacks::Kind::Bracket;
        let text = if bracket {
            trf("Série d'exposition de {n} photos", &[("n", &n)])
        } else {
            trf("Rafale de {n} photos", &[("n", &n)])
        };
        let open = self.unfolded.contains(&stack.photos[0]);
        let members: BTreeSet<usize> = stack
            .photos
            .iter()
            .filter_map(|p| self.folder.iter().position(|f| f == p))
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(text).size(11.5).color(theme::DIM));
            if self.stacking
                && ui
                    .small_button(if open {
                        tr("Replier (S)")
                    } else {
                        tr("Déplier (S)")
                    })
                    .clicked()
            {
                self.toggle_stack(path);
            }
            if bracket
                && ui
                    .small_button(tr("Fusionner la série"))
                    .on_hover_text(tr("Ouvre l'outil Fusion avec les photos de la série"))
                    .clicked()
            {
                self.selected = members;
                self.set_tool(Tool::Merge);
            }
        });
    }

    /// Montre les doublons du film (la même image, à quelques bits d'empreinte près),
    /// groupe par groupe ; rend le nombre de groupes.
    pub fn show_duplicates(&mut self) -> usize {
        let prints: Vec<(PathBuf, u64)> = self
            .folder
            .iter()
            .filter_map(|p| Some((p.clone(), *self.prints.get(p)?)))
            .collect();
        let groups = luma_library::fingerprint::duplicates(&prints);
        let n = groups.len();
        let photos: usize = groups.iter().map(Vec::len).sum();
        self.status = if n == 0 {
            tr("Aucun doublon dans ce dossier").into()
        } else {
            trf(
                "{groups} groupe(s) de doublons, {photos} photos",
                &[("groups", &n), ("photos", &photos)],
            )
        };
        self.duplicates = (n > 0).then_some(groups);
        n
    }

    /// Revient au film entier.
    pub fn hide_duplicates(&mut self) {
        self.duplicates = None;
        self.place = None;
    }

    /// La position GPS des photos du film qui passent le filtre, par rang.
    fn placed(&self) -> Vec<(usize, [f64; 2])> {
        self.visible()
            .into_iter()
            .filter_map(|i| Some((i, self.catalog.get(&self.folder[i])?.gps?)))
            .collect()
    }

    /// Les photos choisies (et celle affichée) qui n'ont pas encore de position : la
    /// carte propose de les placer.
    fn unplaced(&self) -> Vec<PathBuf> {
        self.rename_targets()
            .into_iter()
            .filter(|p| self.catalog.get(p).is_none_or(|e| e.gps.is_none()))
            .collect()
    }

    /// Place à cette position (latitude, longitude) les photos choisies et celle
    /// affichée qui n'en ont pas : écrite dans leur XMP, la photothèque et la carte la
    /// montrent aussitôt. Rend le nombre de photos placées.
    pub fn place_unplaced_at(&mut self, at: [f64; 2]) -> usize {
        let paths = self.unplaced();
        if paths.is_empty() {
            return 0;
        }
        let placed: Vec<(PathBuf, [f64; 2])> = paths.iter().map(|p| (p.clone(), at)).collect();
        self.set_positions(&placed);
        self.status = match paths.as_slice() {
            [one] => trf("{name} placée sur la carte", &[("name", &file_name(one))]),
            _ => trf("{n} photos placées sur la carte", &[("n", &paths.len())]),
        };
        paths.len()
    }

    /// Écrit ces positions (latitude, longitude) dans le XMP des photos et dans la
    /// photothèque.
    fn set_positions(&mut self, placed: &[(PathBuf, [f64; 2])]) {
        let current = self.current_path().map(Path::to_path_buf);
        for (path, at) in placed {
            let position = luma_project::xmp::Position {
                latitude: at[0],
                longitude: at[1],
            };
            self.edit_meta(std::slice::from_ref(path), |m| m.position = Some(position));
            if Some(path) == current.as_ref() {
                self.flush_xmp();
            }
            let mut e = self
                .catalog
                .get(path)
                .cloned()
                .unwrap_or_else(|| luma_library::entry_of(path));
            e.gps = Some(*at);
            self.catalog.insert(e);
        }
        if let Some(file) = &self.catalog_path {
            let _ = self.catalog.save(file);
        }
    }

    /// Place les photos du film qui n'ont pas de position d'après une trace GPX ;
    /// l'heure du boîtier est ramenée à l'heure universelle par l'écart qui en place le
    /// plus. Rend le nombre de photos placées.
    pub fn follow_gpx(&mut self, gpx: &Path) -> usize {
        use luma_library::geotag;
        const GAP: i64 = 3600;
        let track = match std::fs::read_to_string(gpx)
            .map_err(|e| e.to_string())
            .and_then(|t| geotag::read_gpx(&t))
        {
            Ok(t) => t,
            Err(e) => {
                self.status = trf("Trace GPX illisible : {error}", &[("error", &e)]);
                return 0;
            }
        };
        let free: Vec<(PathBuf, i64)> = self
            .folder
            .iter()
            .filter_map(|p| {
                let e = self
                    .catalog
                    .get(p)
                    .cloned()
                    .unwrap_or_else(|| luma_library::entry_of(p));
                if e.gps.is_some() {
                    return None;
                }
                Some((p.clone(), geotag::parse_time(&e.date)?))
            })
            .collect();
        let times: Vec<i64> = free.iter().map(|f| f.1).collect();
        // L'écart se devine serré (cinq minutes au bord de la trace, des trous d'un quart
        // d'heure) ; le placement franchit ensuite les trous d'une heure au plus.
        let Some(offset) = geotag::best_offset(&track, &times, 300, 900) else {
            self.status = tr("Aucune photo de ce film ne tombe dans la trace.").into();
            return 0;
        };
        let placed: Vec<(PathBuf, [f64; 2])> = free
            .into_iter()
            .filter_map(|(p, t)| {
                let at = geotag::position_at(&track, t - offset, GAP)?;
                Some((p, [at.latitude, at.longitude]))
            })
            .collect();
        self.set_positions(&placed);
        let (sign, a) = (if offset < 0 { '-' } else { '+' }, offset.abs());
        let clock = format!("{sign}{:02}:{:02}", a / 3600, a % 3600 / 60);
        self.status = trf(
            "{n} photos placées d'après la trace (horloge du boîtier : UTC{offset})",
            &[("n", &placed.len()), ("offset", &clock)],
        );
        self.map = Some(None);
        placed.len()
    }

    /// Montre la carte (cadrée sur les photos) ou revient à la grille.
    pub fn toggle_map(&mut self) {
        self.map = match self.map {
            Some(_) => None,
            None => {
                self.place = None;
                self.duplicates = None;
                Some(None)
            }
        };
    }

    /// La carte est‑elle montrée ? Et sa vue.
    pub fn map_view(&self) -> Option<Option<luma_library::map::View>> {
        self.map
    }

    /// Les groupes de photos de la carte, tels que dessinés : leur place à l'écran et
    /// leur nombre de photos.
    pub fn map_groups(&self) -> &[(egui::Pos2, usize)] {
        &self.map_groups
    }

    /// Garde les tuiles du fond de carte dans ce dossier (les essais).
    pub fn use_tiles_dir(&mut self, dir: PathBuf) {
        self.tiles.use_dir(dir);
    }

    /// Permet ou non le téléchargement du fond de carte.
    pub fn set_map_online(&mut self, online: bool) {
        self.tiles.set_online(online);
    }

    /// Les tuiles du fond de carte prêtes à dessiner.
    pub fn map_tiles(&self) -> usize {
        self.tiles.ready()
    }

    /// Montre dans la grille les photos d'un lieu de la carte.
    pub fn show_place(&mut self, photos: Vec<PathBuf>) {
        self.status = trf("{n} photos à cet endroit", &[("n", &photos.len())]);
        self.place = Some(photos);
        self.map = None;
    }

    /// Les photos choisies, sauf celle ouverte (la source d'une synchronisation).
    /// Les photos à renommer : celles qui sont choisies (la photo ouverte comprise), sinon
    /// la photo ouverte ; dans l'ordre du film.
    fn rename_targets(&self) -> Vec<PathBuf> {
        let mut rows: BTreeSet<usize> = self.selected.clone();
        rows.extend(self.current);
        rows.iter()
            .filter_map(|i| self.folder.get(*i).cloned())
            .collect()
    }

    /// Le plan du renommage de ces photos selon `template` : ce que la photothèque sait
    /// de chacune (sinon son en‑tête, lu tout de suite), sa note et son libellé du moment.
    fn rename_plan(&self, template: &str) -> Result<luma_render::rename::Plan, String> {
        let fields: Vec<(PathBuf, luma_render::export::NameFields)> = self
            .rename_targets()
            .into_iter()
            .map(|p| {
                let source = luma_project::copies::source(&p);
                let entry = self
                    .catalog
                    .get(&source)
                    .cloned()
                    .unwrap_or_else(|| luma_library::entry_of(&source));
                let meta = self.meta.get(&p).cloned().unwrap_or_default();
                let f = luma_render::export::NameFields::of_details(
                    &p,
                    &entry.date,
                    &entry.make,
                    &entry.model,
                    &entry.lens,
                    entry.iso,
                    meta.rating,
                    &meta.label,
                );
                (p, f)
            })
            .collect();
        luma_render::rename::plan(&fields, template)
    }

    /// Renomme les photos choisies (sinon la photo ouverte) d'après `template` : leurs
    /// fichiers, XMP et copies virtuelles ; le film, la photothèque et les vignettes
    /// suivent. Rend le nombre de photos renommées.
    pub fn rename_photos(&mut self, template: &str) -> Result<usize, String> {
        // Les réglages en attente vont au XMP avant qu'il change de nom.
        self.flush_xmp();
        let plan = self.rename_plan(template)?;
        if plan.files.is_empty() {
            return Ok(0);
        }
        luma_render::rename::apply(&plan)?;
        for m in &plan.photos {
            self.catalog.rename(&m.from, &m.to);
            if let Some(t) = self.thumbs.remove(&m.from) {
                self.thumbs.insert(m.to.clone(), t);
            }
            if let Some(v) = self.meta.remove(&m.from) {
                self.meta.insert(m.to.clone(), v);
            }
            if let Some(v) = self.edits.remove(&m.from) {
                self.edits.insert(m.to.clone(), v);
            }
        }
        if let Some(file) = &self.catalog_path {
            let _ = self.catalog.save(file);
        }
        self.refresh_catalog_lists();
        let current = self.current_path().map(|p| plan.renamed(p));
        let photos: Vec<PathBuf> = self.folder.iter().map(|p| plan.renamed(p)).collect();
        let status = self.status.clone();
        self.show_photos(photos, current, status);
        self.rename_preview = None;
        Ok(plan.photos.len())
    }

    /// La conversion vers l'imprimante : le profil de l'épreuvage, son intention.
    fn print_conversion(&self) -> Result<Option<luma_color::proof::Conversion>, String> {
        let Some(profile) = self
            .proofing
            .profile
            .as_ref()
            .filter(|_| self.print_convert)
        else {
            return Ok(None);
        };
        let intent = if self.proofing.relative {
            luma_color::profile::Intent::Relative
        } else {
            luma_color::profile::Intent::Perceptual
        };
        luma_color::proof::Conversion::new(profile, intent)
            .map(Some)
            .map_err(|e| tr(e.0).to_owned())
    }

    /// Met en page les photos choisies (sinon la photo ouverte) et écrit le PDF dans
    /// `out`, en arrière‑plan : page par page, chaque photo développée avec ses
    /// réglages à la taille de sa case.
    pub fn print_pdf(&mut self, out: PathBuf) {
        use luma_render::print;
        self.flush_xmp();
        let photos = self.rename_targets();
        let layout = self.print_layout.clone();
        let cells = layout.cells();
        if photos.is_empty() || cells.is_empty() {
            self.status = tr("Les marges ne laissent plus de place sur le papier.").into();
            return;
        }
        let printer = match self.print_conversion() {
            Ok(c) => c,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        let (send, done) = std::sync::mpsc::channel();
        let placed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = placed.clone();
        let total = photos.len();
        let job = move || -> Result<(PathBuf, usize), String> {
            let mut pages = Vec::new();
            for chunk in photos.chunks(cells.len()) {
                let mut pictures = Vec::with_capacity(chunk.len());
                for (path, cell) in chunk.iter().zip(&cells) {
                    let photo = crate::dialogs::open_photo(path).map_err(|e| e.to_string())?;
                    let settings = crate::worker::read_sidecar(path)
                        .unwrap_or_else(|| photo.default_settings());
                    pictures.push(print::develop(
                        &photo,
                        &settings,
                        &layout,
                        cell,
                        file_name(path),
                    )?);
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                for page in print::compose(&layout, &pictures, printer.as_ref()) {
                    pages.push(print::encode(&page)?);
                }
            }
            std::fs::write(&out, print::pdf_of(&layout, &pages)).map_err(|e| e.to_string())?;
            Ok((out, pages.len()))
        };
        std::thread::spawn(move || {
            let _ = send.send(job());
        });
        self.printing = Some(Printing {
            done,
            placed,
            total,
            output: Output::Print,
        });
    }

    /// Le PDF est‑il prêt ? (Regardé cinq fois par seconde tant qu'il ne l'est pas.)
    fn poll_printing(&mut self, ctx: &egui::Context) {
        let Some(p) = &self.printing else {
            return;
        };
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
        match p.done.try_recv() {
            Ok(Ok((path, n))) => {
                let file = path.display();
                self.status = match p.output {
                    Output::Print => trf(
                        "PDF prêt : {pages} page(s), {file}",
                        &[("pages", &n), ("file", &file)],
                    ),
                    Output::Book => trf(
                        "Livre prêt : {pages} page(s), {file}",
                        &[("pages", &n), ("file", &file)],
                    ),
                    Output::Slideshow => trf(
                        "Diaporama prêt : {n} photos, {file} (ouvrir index.html)",
                        &[("n", &n), ("file", &file)],
                    ),
                    Output::Import => {
                        let done = trf(
                            "{n} photos importées et vérifiées dans {file}",
                            &[("n", &n), ("file", &file)],
                        );
                        match self.import.ejected.lock().expect("éjection").take() {
                            Some(Ok(())) => format!("{done} · {}", tr("carte éjectée")),
                            Some(Err(e)) => format!(
                                "{done} · {}",
                                trf(
                                    "la carte n'a pas pu être éjectée : {error}",
                                    &[("error", &e)]
                                )
                            ),
                            None => done,
                        }
                    }
                    Output::Gallery => trf(
                        "Galerie prête : {n} photos, {file} (à mettre en ligne telle quelle)",
                        &[("n", &n), ("file", &file)],
                    ),
                };
                if p.output == Output::Print {
                    self.print_open = false;
                }
                let imported = p.output == Output::Import;
                if imported {
                    self.import.report = self.status.clone();
                }
                self.printing = None;
                if imported && n > 0 {
                    self.import_open = false;
                    let status = self.status.clone();
                    self.open_path(&path);
                    self.status = status;
                }
            }
            Ok(Err(e)) => {
                self.status = trf("Impression impossible : {error}", &[("error", &e)]);
                self.printing = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.printing = None,
        }
    }

    /// Les photos d'un livre ou d'un diaporama : celles choisies (deux au moins), sinon
    /// tout le film affiché.
    pub fn output_targets(&self) -> Vec<PathBuf> {
        if self.selected.len() >= 2 {
            return self.rename_targets();
        }
        self.visible()
            .into_iter()
            .map(|i| self.folder[i].clone())
            .collect()
    }

    /// Lance le diaporama dans l'app, depuis la photo ouverte (sinon la première).
    pub fn start_slideshow(&mut self) {
        let visible = self.visible();
        let Some(&first) = visible.first() else {
            return;
        };
        if self.current.is_none_or(|c| !visible.contains(&c)) {
            self.select(first);
        }
        self.culling = true;
        self.compare = None;
        self.playing = Some(Playing {
            since: std::time::Instant::now(),
            paused: false,
            arrived: None,
            count: 0,
            leaving: None,
        });
    }

    /// Une photo du diaporama est‑elle en train de partir (une transition en cours) ?
    pub fn in_transition(&self) -> bool {
        self.playing.as_ref().is_some_and(|p| p.leaving.is_some())
    }

    /// Le diaporama est‑il en cours (et pas en pause) ?
    pub fn is_playing(&self) -> bool {
        self.playing.as_ref().is_some_and(|p| !p.paused)
    }

    /// Passe à la photo suivante quand son temps est écoulé (une fois la photo
    /// affichée : une photo lente à ouvrir n'est pas sautée), et revient au début après
    /// la dernière.
    fn tick_slideshow(&mut self, ctx: &egui::Context) {
        if !self.culling {
            self.playing = None;
        }
        let Some(p) = &self.playing else {
            return;
        };
        if p.paused {
            return;
        }
        let wait = std::time::Duration::from_secs_f32(self.show.seconds.max(0.5));
        let elapsed = p.since.elapsed();
        if elapsed < wait || self.shown != self.generation {
            ctx.request_repaint_after(
                wait.saturating_sub(elapsed)
                    .max(std::time::Duration::from_millis(50)),
            );
            return;
        }
        let visible = self.visible();
        let at = self
            .current
            .and_then(|c| visible.iter().position(|&v| v == c));
        let next = match at {
            Some(k) if k + 1 < visible.len() => visible[k + 1],
            _ => visible.first().copied().unwrap_or(0),
        };
        // L'image de la photo qui part est gardée pour la transition ; la suivante
        // arrive dans une image neuve.
        let leaving = self
            .texture
            .take()
            .map(|texture| (texture, self.image_size));
        self.select(next);
        if let Some(p) = &mut self.playing {
            let now = std::time::Instant::now();
            p.since = now;
            p.leaving = leaving.map(|(texture, size)| Leaving {
                texture,
                size,
                arrived: p.arrived.unwrap_or(now),
                count: p.count,
            });
            p.arrived = None;
            p.count += 1;
        }
        ctx.request_repaint();
    }

    /// Les photos du livre, dans l'ordre choisi dans « Pages du livre » : celles qu'on y
    /// a placées d'abord, à leur place, puis les nouvelles dans l'ordre du film.
    pub fn book_photos(&self) -> Vec<PathBuf> {
        let mut photos = self.output_targets();
        let rank = |p: &PathBuf| {
            self.book_order
                .iter()
                .position(|o| o == p)
                .unwrap_or(usize::MAX)
        };
        // Un tri stable : les photos hors de l'ordre choisi gardent celui du film.
        photos.sort_by_key(rank);
        photos
    }

    /// Échange deux photos du livre (leurs rangs dans [`Studio::book_photos`]).
    pub fn swap_book_photos(&mut self, a: usize, b: usize) {
        let mut photos = self.book_photos();
        if a < photos.len() && b < photos.len() {
            photos.swap(a, b);
            self.book_order = photos;
        }
    }

    /// Fait le livre photo des photos (en arrière‑plan) et l'écrit dans `out`.
    pub fn make_book(&mut self, out: PathBuf) {
        use luma_render::{book, print};
        self.flush_xmp();
        let photos = self.book_photos();
        let b = self.book.clone();
        if photos.is_empty() || b.cells(book::Kind::Four, true).is_empty() {
            self.status = tr("Les marges ne laissent plus de place sur le papier.").into();
            return;
        }
        let printer = match self.print_conversion() {
            Ok(c) => c,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        let (send, done) = std::sync::mpsc::channel();
        let placed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = placed.clone();
        let total = photos.len();
        let job = move || -> Result<(PathBuf, usize), String> {
            let open = |p: &Path| -> Result<(luma_render::Photo, Settings), String> {
                let photo = crate::dialogs::open_photo(p).map_err(|e| e.to_string())?;
                let settings =
                    crate::worker::read_sidecar(p).unwrap_or_else(|| photo.default_settings());
                Ok((photo, settings))
            };
            let mut landscape = Vec::with_capacity(photos.len());
            for p in &photos {
                let (photo, settings) = open(p)?;
                landscape.push(book::is_landscape(&photo, &settings));
            }
            let layout = b.layout();
            let mut pages = Vec::new();
            for sheet in b.sheets(&landscape) {
                let orientation = sheet.photos.first().is_none_or(|&k| landscape[k]);
                let cells = b.cells(sheet.kind, orientation);
                let mut pictures = Vec::with_capacity(sheet.photos.len());
                for (&k, cell) in sheet.photos.iter().zip(&cells) {
                    let (photo, settings) = open(&photos[k])?;
                    pictures.push(print::develop(
                        &photo,
                        &settings,
                        &layout,
                        cell,
                        file_name(&photos[k]),
                    )?);
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                pages.push(print::encode(&b.page(
                    sheet.kind,
                    &cells,
                    &pictures,
                    printer.as_ref(),
                ))?);
            }
            std::fs::write(&out, print::pdf_of(&layout, &pages)).map_err(|e| e.to_string())?;
            Ok((out, pages.len()))
        };
        std::thread::spawn(move || {
            let _ = send.send(job());
        });
        self.printing = Some(Printing {
            done,
            placed,
            total,
            output: Output::Book,
        });
    }

    /// Exporte le diaporama des photos (en arrière‑plan) dans le dossier `dir`.
    pub fn export_slideshow(&mut self, dir: PathBuf) {
        self.flush_xmp();
        let photos = self.output_targets();
        if photos.is_empty() {
            return;
        }
        let show = self.show.clone();
        let (send, done) = std::sync::mpsc::channel();
        let placed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = placed.clone();
        let total = photos.len();
        let job = move || -> Result<(PathBuf, usize), String> {
            let mut frames = Vec::with_capacity(photos.len());
            for p in &photos {
                let photo = crate::dialogs::open_photo(p).map_err(|e| e.to_string())?;
                let settings =
                    crate::worker::read_sidecar(p).unwrap_or_else(|| photo.default_settings());
                let r = photo
                    .render(&settings, Size::Fit(show.size), &luma_core::Cancel::new())
                    .map_err(|e| e.to_string())?;
                let rgb = luma_image::encode8(&r.image, OutputSpace::Srgb);
                let jpeg = luma_render::jpeg::encode(
                    &rgb,
                    r.image.width,
                    r.image.height,
                    show.quality,
                    &[],
                    &[],
                )?;
                frames.push((jpeg, file_name(p)));
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            luma_render::slideshow::write(&dir, &show, &frames)?;
            Ok((dir.join("index.html"), frames.len()))
        };
        std::thread::spawn(move || {
            let _ = send.send(job());
        });
        self.printing = Some(Printing {
            done,
            placed,
            total,
            output: Output::Slideshow,
        });
    }

    /// Exporte la galerie web des photos (en arrière‑plan) dans le dossier `dir` ; la
    /// légende de chaque photo est son titre, sinon son nom.
    pub fn export_gallery(&mut self, dir: PathBuf) {
        self.flush_xmp();
        let photos: Vec<(PathBuf, String)> = self
            .output_targets()
            .into_iter()
            .map(|p| {
                let title = self
                    .meta
                    .get(&p)
                    .map(|m| m.title.trim().to_owned())
                    .unwrap_or_default();
                let caption = if title.is_empty() {
                    file_name(&p)
                } else {
                    title
                };
                (p, caption)
            })
            .collect();
        if photos.is_empty() {
            return;
        }
        let g = self.gallery.clone();
        let (send, done) = std::sync::mpsc::channel();
        let placed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = placed.clone();
        let total = photos.len();
        let job = move || -> Result<(PathBuf, usize), String> {
            let mut entries = Vec::with_capacity(photos.len());
            for (p, caption) in photos {
                let photo = crate::dialogs::open_photo(&p).map_err(|e| e.to_string())?;
                let settings =
                    crate::worker::read_sidecar(&p).unwrap_or_else(|| photo.default_settings());
                let r = photo
                    .render(&settings, Size::Fit(g.size), &luma_core::Cancel::new())
                    .map_err(|e| e.to_string())?;
                entries.push(luma_render::gallery::entry(&r.image, &g, caption)?);
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            luma_render::gallery::write(&dir, &g, &entries)?;
            Ok((dir.join("index.html"), entries.len()))
        };
        std::thread::spawn(move || {
            let _ = send.send(job());
        });
        self.printing = Some(Printing {
            done,
            placed,
            total,
            output: Output::Gallery,
        });
    }

    /// Ouvre la fenêtre d'import, sur la carte montée s'il y en a une (cherchée en
    /// arrière‑plan : la première trouvée devient la source).
    pub fn open_import(&mut self) {
        self.import.cards_at = None;
        self.refresh_cards();
        self.import_open = true;
    }

    /// Cherche les cartes montées en arrière‑plan, au plus toutes les deux secondes, et
    /// prend celles trouvées.
    fn refresh_cards(&mut self) {
        if let Some(rx) = &self.import.cards_job
            && let Ok(cards) = rx.try_recv()
        {
            self.import.cards_job = None;
            if self.import.source.is_none()
                && let Some(first) = cards.first()
            {
                self.set_import_source(first.clone());
            }
            self.import.cards = cards;
        }
        let due = self
            .import
            .cards_at
            .is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(2));
        if due && self.import.cards_job.is_none() {
            self.import.cards_at = Some(std::time::Instant::now());
            let (send, receive) = std::sync::mpsc::channel();
            self.import.cards_job = Some(receive);
            std::thread::spawn(move || {
                let _ = send.send(luma_library::import::cards());
                crate::wake();
            });
        }
    }

    /// Attend la recherche des cartes en cours (pour les essais).
    pub fn wait_cards(&mut self) {
        if let Some(rx) = self.import.cards_job.take()
            && let Ok(cards) = rx.recv()
        {
            self.import.cards = cards;
        }
    }

    /// Choisit la carte (ou le dossier) à importer, et y compte les photos.
    pub fn set_import_source(&mut self, source: PathBuf) {
        let photos = luma_library::import::photos(&source);
        self.import.found = photos.len();
        self.import.source = Some(source);
        // Les jours de prise de vue et les prises, lus en arrière‑plan (l'en‑tête de
        // chaque photo).
        self.import.days.clear();
        self.import.shots.clear();
        self.import.previews.clear();
        self.import.previews_job = None;
        let (send, receive) = std::sync::mpsc::channel();
        self.import.days_job = Some(receive);
        std::thread::spawn(move || {
            let entries: Vec<luma_library::Entry> =
                photos.iter().map(|p| luma_library::entry_of(p)).collect();
            let _ = send.send((luma_library::import::days(&entries), shots_of(&entries)));
            crate::wake();
        });
    }

    /// Attend la fin de la lecture des jours de la source (pour les essais).
    pub fn wait_import_days(&mut self) {
        if let Some(rx) = self.import.days_job.take()
            && let Ok((days, shots)) = rx.recv()
        {
            self.import.days = days.into_iter().map(|(d, n)| (d, n, true)).collect();
            self.import.shots = shots;
        }
    }

    /// Lit en arrière‑plan l'aperçu de chaque prise de la source : celui qui est intégré
    /// au RAW, sinon l'image réduite. S'arrête quand la source change.
    fn start_import_previews(&mut self) {
        if self.import.previews_job.is_some() || !self.import.previews.is_empty() {
            return;
        }
        let paths: Vec<PathBuf> = self
            .import
            .shots
            .iter()
            .map(|s| s.main().to_path_buf())
            .collect();
        let (send, receive) = std::sync::mpsc::channel();
        self.import.previews_job = Some(receive);
        std::thread::spawn(move || {
            for p in paths {
                let img = luma_raw::RawPhoto::embedded_preview(&p, 160).or_else(|| {
                    image::open(&p)
                        .ok()
                        .map(|i| i.thumbnail(160, 160).to_rgb8())
                });
                let Some(img) = img else {
                    continue;
                };
                let size = [img.width() as usize, img.height() as usize];
                if send.send((p, size, img.into_raw())).is_err() {
                    return;
                }
                crate::wake();
            }
        });
    }

    /// Les aperçus arrivés : au plus une trentaine par image de l'écran.
    fn receive_import_previews(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.import.previews_job else {
            return;
        };
        for _ in 0..32 {
            match rx.try_recv() {
                Ok((path, size, rgb)) => {
                    let image = ColorImage::from_rgb(size, &rgb);
                    let name = format!("import {}", path.display());
                    let texture = ctx.load_texture(name, image, TextureOptions::LINEAR);
                    self.import.previews.insert(path, texture);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.import.previews_job = None;
                    return;
                }
            }
        }
        ctx.request_repaint();
    }

    /// Attend tous les aperçus de la source (pour les essais).
    pub fn wait_import_previews(&mut self, ctx: &egui::Context) {
        self.start_import_previews();
        while self.import.previews_job.is_some() {
            self.receive_import_previews(ctx);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// Le nombre d'aperçus lus pour le choix photo par photo.
    pub fn import_previews(&self) -> usize {
        self.import.previews.len()
    }

    /// Importe en arrière‑plan : lit les photos de la source, fait le plan (rangement,
    /// noms, photos déjà là), copie et vérifie ; puis ouvre le dossier des premières
    /// photos importées, qui entre dans la photothèque.
    pub fn start_import(&mut self) {
        use luma_library::import;
        let (Some(source), Some(dest)) = (self.import.source.clone(), self.import.dest.clone())
        else {
            return;
        };
        let options = import::Options {
            dest,
            folders: self.import.folders.clone(),
            backup: self.import.backup.clone(),
            days: self.import.chosen_days(),
            skip: self.import.skipped(),
        };
        let template = Some(self.import.name.trim().to_owned()).filter(|t| !t.is_empty());
        if let Some(t) = &template
            && let Err(e) = luma_render::export::check_template(t)
        {
            self.status = e;
            return;
        }
        let (send, done) = std::sync::mpsc::channel();
        let placed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = placed.clone();
        let total = self.import.chosen();
        // Une carte seulement (un dossier DCIM) : un dossier ordinaire ne s'éjecte pas.
        let eject = self.import.eject && source.join("DCIM").is_dir();
        let ejected = self.import.ejected.clone();
        *ejected.lock().expect("éjection") = None;
        let job = move || -> Result<(PathBuf, usize), String> {
            let photos = import::photos(&source);
            let entries: Vec<luma_library::Entry> =
                photos.iter().map(|p| luma_library::entry_of(p)).collect();
            let plan = import::plan(&entries, &options, |e, n| {
                let t = template.as_ref()?;
                let f = luma_render::export::NameFields::of_details(
                    &e.path, &e.date, &e.make, &e.model, &e.lens, e.iso, e.rating, &e.label,
                );
                Some(luma_render::export::file_name(
                    t,
                    &luma_render::export::NameFields { sequence: n, ..f },
                ))
            });
            // Les photos déjà là comptent comme faites.
            counter.store(plan.skipped.len(), std::sync::atomic::Ordering::Relaxed);
            let skipped = plan.skipped.len();
            let report = import::run(&plan, |n| {
                counter.store(skipped + n, std::sync::atomic::Ordering::Relaxed)
            })?;
            let first = plan
                .items
                .first()
                .and_then(|i| i.target.parent().map(Path::to_path_buf))
                .unwrap_or(options.dest);
            // Tout est copié et relu : la carte peut partir.
            if eject {
                *ejected.lock().expect("éjection") = Some(import::eject(&source));
            }
            Ok((first, report.copied))
        };
        std::thread::spawn(move || {
            let _ = send.send(job());
        });
        self.printing = Some(Printing {
            done,
            placed,
            total,
            output: Output::Import,
        });
    }

    /// La fenêtre « Importer » : la carte, la photothèque, le rangement, le second
    /// disque.
    fn import_window(&mut self, ctx: &egui::Context) {
        let mut open = self.import_open;
        let mut go = false;
        let shown = |p: &Option<PathBuf>, none: &str| {
            p.as_ref()
                .map_or_else(|| tr(none).to_owned(), |p| p.display().to_string())
        };
        egui::Window::new(tr("Importer des photos"))
            .id(egui::Id::unique("fenetre-importer"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(440.0);
                self.refresh_cards();
                ui.ctx().request_repaint_after(std::time::Duration::from_secs(2));
                let cards = self.import.cards.clone();
                egui::Grid::new("importer").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label(RichText::new(tr("Depuis")).color(theme::DIM));
                    ui.vertical(|ui| {
                        for c in &cards {
                            let on = self.import.source.as_ref() == Some(c);
                            // Sous Windows, le lecteur (E:) ; ailleurs, le nom du volume.
                            let name = c.file_name().map_or_else(
                                || c.display().to_string().trim_end_matches('\\').to_owned(),
                                |n| n.to_string_lossy().into_owned(),
                            );
                            if ui.selectable_label(on, trf("Carte {name}", &[("name", &name)])).clicked() {
                                self.set_import_source(c.clone());
                            }
                        }
                        ui.horizontal(|ui| {
                            ui.label(shown(&self.import.source, "Aucune carte trouvée"));
                            if ui.small_button(tr("Choisir…")).clicked()
                                && let Some(d) = crate::dialogs::pick_folder(tr("Importer depuis…"))
                            {
                                self.set_import_source(d);
                            }
                        });
                        if self.import.source.is_some() {
                            ui.label(
                                RichText::new(trf("{n} photos", &[("n", &self.import.found)]))
                                    .size(11.0)
                                    .color(theme::DIM),
                            );
                        }
                    });
                    ui.end_row();
                    // Plusieurs jours sur la carte : ne prendre que certains.
                    self.import.receive_days();
                    if self.import.days.len() > 1 {
                        ui.label(RichText::new(tr("Jours")).color(theme::DIM));
                        ui.horizontal_wrapped(|ui| {
                            for (day, n, on) in &mut self.import.days {
                                ui.checkbox(on, format!("{day} ({n})"));
                            }
                        });
                        ui.end_row();
                    }
                    if !self.import.shots.is_empty() {
                        ui.label(RichText::new(tr("Photos")).color(theme::DIM));
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.import.pick, tr("Choisir photo par photo"));
                            if self.import.pick {
                                if ui.small_button(tr("Toutes")).clicked() {
                                    self.import.shots.iter_mut().for_each(|s| s.on = true);
                                }
                                if ui.small_button(tr("Aucune|photo")).clicked() {
                                    self.import.shots.iter_mut().for_each(|s| s.on = false);
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
                if self.import.pick && !self.import.shots.is_empty() {
                    self.start_import_previews();
                    self.receive_import_previews(ui.ctx());
                    self.import_picker(ui);
                }
                egui::Grid::new("importer-vers").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                    ui.label(RichText::new(tr("Vers")).color(theme::DIM));
                    ui.horizontal(|ui| {
                        ui.label(shown(&self.import.dest, "Choisissez la photothèque"));
                        if ui.small_button(tr("Choisir…")).clicked()
                            && let Some(d) = crate::dialogs::pick_folder(tr("Importer dans…"))
                        {
                            self.import.dest = Some(d);
                        }
                    });
                    ui.end_row();
                    ui.label(RichText::new(tr("Rangement")).color(theme::DIM));
                    ui.add(egui::TextEdit::singleline(&mut self.import.folders).desired_width(f32::INFINITY));
                    ui.end_row();
                    ui.label(RichText::new(tr("Renommer")).color(theme::DIM));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.import.name)
                            .hint_text(tr("Noms d'origine ; ou {date}_{séquence}…"))
                            .desired_width(f32::INFINITY),
                    );
                    ui.end_row();
                    if self.import.source.as_ref().is_some_and(|s| s.join("DCIM").is_dir()) && !cfg!(target_os = "ios") {
                        ui.label("");
                        ui.checkbox(&mut self.import.eject, tr("Éjecter la carte après l'import"));
                        ui.end_row();
                    }
                    ui.label(RichText::new(tr("Second disque")).color(theme::DIM));
                    ui.horizontal(|ui| {
                        ui.label(shown(&self.import.backup, "Aucun"));
                        if ui.small_button(tr("Choisir…")).clicked()
                            && let Some(d) = crate::dialogs::pick_folder(tr("Sauvegarder aussi dans…"))
                        {
                            self.import.backup = Some(d);
                        }
                        if self.import.backup.is_some() && ui.small_button(tr("Aucun")).clicked() {
                            self.import.backup = None;
                        }
                    });
                    ui.end_row();
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new(tr(
                        "Chaque copie est relue et comparée à l'original ; le RAW, son JPEG et son XMP gardent un même nom ; une photo déjà importée est sautée ; la carte n'est jamais modifiée.",
                    ))
                    .size(10.5)
                    .color(theme::DIM),
                );
                ui.add_space(6.0);
                match &self.printing {
                    Some(p) if p.output == Output::Import => {
                        let done = p.placed.load(std::sync::atomic::Ordering::Relaxed);
                        ui.add(egui::ProgressBar::new(done as f32 / p.total.max(1) as f32).text(trf(
                            "Import : {done} sur {n}",
                            &[("done", &done), ("n", &p.total)],
                        )));
                    }
                    _ => {
                        let ready = self.printing.is_none()
                            && self.import.chosen() > 0
                            && self.import.dest.is_some();
                        go = ui
                            .add_enabled(ready, egui::Button::new(trf("Importer {n} photos", &[("n", &self.import.chosen())])))
                            .clicked();
                    }
                }
            });
        if go {
            self.start_import();
        }
        self.import_open = open;
    }

    /// Le choix photo par photo : les aperçus des prises des jours choisis ; un clic
    /// laisse une photo sur la carte, ou la reprend.
    fn import_picker(&mut self, ui: &mut Ui) {
        let tile = vec2(84.0, 64.0);
        let days: Vec<String> = self
            .import
            .days
            .iter()
            .filter(|d| d.2)
            .map(|d| d.0.clone())
            .collect();
        let Import {
            shots, previews, ..
        } = &mut self.import;
        egui::ScrollArea::vertical()
            .id_salt("importer-photos")
            .max_height(240.0)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    for shot in shots.iter_mut() {
                        if !days.is_empty() && !days.contains(&shot.day) {
                            continue;
                        }
                        let (rect, response) = ui.allocate_exact_size(tile, egui::Sense::click());
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, 3.0, Color32::from_gray(0x24));
                        let tint = if shot.on {
                            Color32::WHITE
                        } else {
                            Color32::from_gray(0x50)
                        };
                        if let Some(t) = previews.get(shot.main()) {
                            let [w, h] = t.size();
                            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                            painter.image(t.id(), fit(rect.shrink(2.0), w, h), uv, tint);
                        }
                        // La coche : pleine sur une photo à importer, vide sur une photo
                        // laissée sur la carte.
                        let mark = pos2(rect.max.x - 9.0, rect.min.y + 9.0);
                        if shot.on {
                            painter.circle_filled(mark, 6.0, theme::ACCENT);
                            let stroke = egui::Stroke::new(1.5, Color32::BLACK);
                            painter.line_segment(
                                [mark + vec2(-3.0, 0.0), mark + vec2(-1.0, 2.5)],
                                stroke,
                            );
                            painter.line_segment(
                                [mark + vec2(-1.0, 2.5), mark + vec2(3.0, -2.5)],
                                stroke,
                            );
                        } else {
                            painter.circle_stroke(
                                mark,
                                6.0,
                                egui::Stroke::new(1.2, Color32::from_gray(0xa0)),
                            );
                        }
                        let name = shot
                            .main()
                            .file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        if response.on_hover_text(name).clicked() {
                            shot.on = !shot.on;
                        }
                    }
                });
            });
    }

    /// La fenêtre « Pages du livre » : chaque page en petit, ses photos à leur place
    /// (leurs vignettes) ; glisser une photo sur une autre les échange, et le livre suit
    /// ce nouvel ordre.
    fn book_pages_window(&mut self, ctx: &egui::Context) {
        use luma_render::book::Kind;
        let photos = self.book_photos();
        let landscape: Vec<bool> = photos
            .iter()
            .map(|p| {
                self.thumbs
                    .get(p)
                    .is_none_or(|t| t.size()[0] >= t.size()[1])
            })
            .collect();
        let b = &self.book;
        let sheets = b.sheets(&landscape);
        let (pw, ph) = b.layout().page_px();
        let height = 120.0;
        let scale = height / ph as f32;
        let page_size = vec2(pw as f32 * scale, height);
        let mut open = self.book_pages_open;
        let mut swap = None;
        // Le nombre de photos des pages de photos, dans l'ordre (sans la couverture ni
        // les pages de texte) : de quoi en choisir un pour une page.
        let counts: Vec<usize> = sheets
            .iter()
            .filter(|s| matches!(s.kind, Kind::Single | Kind::Pair | Kind::Four))
            .map(|s| s.photos.len())
            .collect();
        let mut layout: Option<Vec<usize>> = None;
        egui::Window::new(trf("Pages du livre : {pages}", &[("pages", &sheets.len())]))
            .id(egui::Id::unique("fenetre-pages-livre"))
            .open(&mut open)
            .collapsible(false)
            .default_width(640.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(tr("Glissez une photo sur une autre pour les échanger ; 1, 2, 4 : les photos de la page."))
                            .size(11.0)
                            .color(theme::DIM),
                    );
                    if !b.pages.is_empty() && ui.small_button(tr("Disposition automatique")).clicked() {
                        layout = Some(Vec::new());
                    }
                });
                egui::ScrollArea::vertical()
                    .max_height(460.0)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(10.0, 14.0);
                            for (page, sheet) in sheets.iter().enumerate() {
                                ui.vertical(|ui| {
                                    let (rect, _) =
                                        ui.allocate_exact_size(page_size, egui::Sense::hover());
                                    let painter = ui.painter_at(rect);
                                    painter.rect_filled(rect, 2.0, Color32::from_gray(0xf2));
                                    let orientation =
                                        sheet.photos.first().is_none_or(|&k| landscape[k]);
                                    let cells = b.cells(sheet.kind, orientation);
                                    for (&k, cell) in sheet.photos.iter().zip(&cells) {
                                        let area = Rect::from_min_size(
                                            rect.min + vec2(cell.x as f32, cell.y as f32) * scale,
                                            vec2(
                                                cell.width as f32,
                                                (cell.height - cell.caption) as f32,
                                            ) * scale,
                                        );
                                        let id = egui::Id::unique(("photo-livre", k));
                                        let r =
                                            ui.interact(area, id, egui::Sense::click_and_drag());
                                        r.dnd_set_drag_payload(k);
                                        if let Some(from) = r.dnd_release_payload::<usize>()
                                            && *from != k
                                        {
                                            swap = Some((*from, k));
                                        }
                                        let hovered =
                                            r.dnd_hover_payload::<usize>().is_some_and(|f| *f != k);
                                        match self.thumbs.get(&photos[k]) {
                                            Some(t) => {
                                                let [w, h] = t.size();
                                                let uv = Rect::from_min_max(
                                                    pos2(0.0, 0.0),
                                                    pos2(1.0, 1.0),
                                                );
                                                let tint = if r.dragged() {
                                                    Color32::from_gray(0x90)
                                                } else {
                                                    Color32::WHITE
                                                };
                                                painter.image(t.id(), fit(area, w, h), uv, tint);
                                            }
                                            None => {
                                                painter.rect_filled(
                                                    area,
                                                    0.0,
                                                    Color32::from_gray(0xc8),
                                                );
                                            }
                                        }
                                        if hovered {
                                            painter.rect_stroke(
                                                area,
                                                0.0,
                                                egui::Stroke::new(2.0, theme::ACCENT),
                                                egui::StrokeKind::Inside,
                                            );
                                        }
                                        r.on_hover_text(file_name(&photos[k]));
                                    }
                                    // La couverture : son titre sous la photo ; les pages de
                                    // texte : leurs premières lignes.
                                    let text = match sheet.kind {
                                        Kind::Cover => Some(b.title.clone()),
                                        Kind::Opening => Some(b.opening.clone()),
                                        Kind::Closing => Some(b.closing.clone()),
                                        _ => None,
                                    };
                                    if let Some(text) = text.filter(|t| !t.trim().is_empty()) {
                                        let at = if sheet.kind == Kind::Cover {
                                            pos2(rect.center().x, rect.min.y + rect.height() * 0.8)
                                        } else {
                                            rect.center()
                                        };
                                        let line: String = text
                                            .lines()
                                            .next()
                                            .unwrap_or_default()
                                            .chars()
                                            .take(24)
                                            .collect();
                                        painter.text(
                                            at,
                                            egui::Align2::CENTER_CENTER,
                                            line,
                                            egui::FontId::proportional(8.0),
                                            Color32::from_gray(0x40),
                                        );
                                    }
                                    let label = match sheet.kind {
                                        Kind::Cover => tr("Couverture").to_owned(),
                                        _ => trf("Page {n}", &[("n", &page)]),
                                    };
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(label).size(10.0).color(theme::DIM));
                                        // Une page de photos : 1, 2 ou 4 photos ; les pages d'avant gardent les
                                        // leurs, celles d'après suivent.
                                        if matches!(sheet.kind, Kind::Single | Kind::Pair | Kind::Four) {
                                            let rank = sheets[..page]
                                                .iter()
                                                .filter(|s| matches!(s.kind, Kind::Single | Kind::Pair | Kind::Four))
                                                .count();
                                            for n in [1, 2, 4] {
                                                let on = sheet.photos.len() == n;
                                                if ui.selectable_label(on, RichText::new(n.to_string()).size(10.0)).clicked() && !on {
                                                    let mut chosen = counts[..rank].to_vec();
                                                    chosen.push(n);
                                                    layout = Some(chosen);
                                                }
                                            }
                                        }
                                    });
                                });
                            }
                        });
                    });
            });
        if let Some((a, b)) = swap {
            self.swap_book_photos(a, b);
        }
        if let Some(pages) = layout {
            self.book.pages = pages;
        }
        self.book_pages_open = open;
    }

    /// La fenêtre « Livre, diaporama et galerie » : le diaporama (dans l'app ou exporté),
    /// le livre photo et la galerie web.
    fn outputs_window(&mut self, ctx: &egui::Context) {
        use luma_render::book::FORMATS;
        let mut open = self.outputs_open;
        let photos = self.book_photos();
        let n = photos.len();
        let (mut play, mut export, mut make, mut web) = (false, false, false, false);
        egui::Window::new(trf("Livre, diaporama et galerie : {n} photos", &[("n", &n)]))
            .id(egui::Id::unique("fenetre-livre"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(400.0);
                section_title(ui, tr("Diaporama"));
                egui::Grid::new("diaporama").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label(RichText::new(tr("Titre")).color(theme::DIM));
                    ui.text_edit_singleline(&mut self.show.title);
                    ui.end_row();
                    ui.label(RichText::new(tr("Durée")).color(theme::DIM));
                    ui.add(egui::Slider::new(&mut self.show.seconds, 1.0..=30.0).suffix(" s").step_by(0.5));
                    ui.end_row();
                    ui.label(RichText::new(tr("Fondu|durée du diaporama")).color(theme::DIM));
                    ui.add(egui::Slider::new(&mut self.show.fade, 0.0..=3.0).suffix(" s").step_by(0.1));
                    ui.end_row();
                    ui.label(RichText::new(tr("Transition")).color(theme::DIM));
                    use luma_render::slideshow::Transition;
                    let label = |t: Transition| match t {
                        Transition::Fade => tr("Fondu|transition"),
                        Transition::Slide => tr("Glissement"),
                        Transition::Drift => tr("Fondu et zoom lent"),
                    };
                    egui::ComboBox::from_id_salt("transition")
                        .selected_text(label(self.show.transition))
                        .show_ui(ui, |ui| {
                            for t in Transition::ALL {
                                ui.selectable_value(&mut self.show.transition, t, label(t));
                            }
                        });
                    ui.end_row();
                    if crate::dialogs::FOLDER_PICKER {
                        ui.label(RichText::new(tr("Musique")).color(theme::DIM));
                        ui.horizontal(|ui| {
                            let name = self
                                .show
                                .music
                                .as_ref()
                                .and_then(|m| m.file_name())
                                .map(|n| n.to_string_lossy().into_owned());
                            if ui
                                .button(name.as_deref().unwrap_or(tr("Choisir…")))
                                .on_hover_text(tr("Jouée en boucle dans le diaporama exporté ; M la coupe"))
                                .clicked()
                                && let Some(m) = crate::dialogs::pick_music()
                            {
                                self.show.music = Some(m);
                            }
                            if self.show.music.is_some() && ui.small_button("✕").on_hover_text(tr("Sans musique")).clicked() {
                                self.show.music = None;
                            }
                        });
                        ui.end_row();
                    }
                });
                ui.checkbox(&mut self.show.captions, tr("Le nom de chaque photo dessous"));
                ui.horizontal(|ui| {
                    play = ui
                        .button(tr("Lancer le diaporama"))
                        .on_hover_text(tr("En plein écran : Espace pour la pause, flèches pour les photos, Échap pour sortir"))
                        .clicked();
                    export = ui
                        .add_enabled(self.printing.is_none(), egui::Button::new(tr("Exporter le diaporama…")))
                        .on_hover_text(tr("Un dossier qui se suffit à lui-même : index.html et les photos, à ouvrir dans un navigateur"))
                        .clicked();
                });
                ui.add_space(12.0);
                section_title(ui, tr("Livre photo"));
                let b = &mut self.book;
                egui::Grid::new("livre").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label(RichText::new(tr("Titre")).color(theme::DIM));
                    ui.text_edit_singleline(&mut b.title);
                    ui.end_row();
                    ui.label(RichText::new(tr("Sous-titre")).color(theme::DIM));
                    ui.text_edit_singleline(&mut b.subtitle);
                    ui.end_row();
                    ui.label(RichText::new(tr("Format")).color(theme::DIM));
                    let label = |p: &luma_render::print::Paper| match p.name {
                        name if name.starts_with(|c: char| c.is_ascii_digit()) => {
                            format!("{} cm", name.replace('x', " × "))
                        }
                        name => format!("{name} ({:.0} × {:.0} mm)", p.width, p.height),
                    };
                    egui::ComboBox::from_id_salt("format-livre")
                        .selected_text(label(&b.paper))
                        .show_ui(ui, |ui| {
                            for f in FORMATS {
                                ui.selectable_value(&mut b.paper, f, label(&f));
                            }
                        });
                    ui.end_row();
                    ui.label(RichText::new(tr("Orientation")).color(theme::DIM));
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut b.landscape, false, tr("Portrait"));
                        ui.selectable_value(&mut b.landscape, true, tr("Paysage"));
                    });
                    ui.end_row();
                    ui.label(RichText::new(tr("Marges")).color(theme::DIM));
                    ui.add(egui::Slider::new(&mut b.margin, 0.0..=40.0).suffix(" mm").step_by(1.0));
                    ui.end_row();
                });
                ui.checkbox(&mut b.captions, tr("Le nom de chaque photo dessous"));
                ui.label(RichText::new(tr("Page de texte après la couverture")).color(theme::DIM));
                ui.add(
                    egui::TextEdit::multiline(&mut b.opening)
                        .hint_text(tr("une dédicace, un mot d'introduction"))
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                ui.label(RichText::new(tr("Dernière page")).color(theme::DIM));
                ui.add(
                    egui::TextEdit::multiline(&mut b.closing)
                        .hint_text(tr("un lieu, une date, des remerciements"))
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                // La disposition : le nombre de photos de chaque page, au choix ; le texte
                // tapé est gardé tant qu'il n'est pas valable.
                let id = ui.scope_id().with("disposition-livre");
                let mut text: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_else(|| {
                    b.rhythm.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Photos par page")).color(theme::DIM));
                    ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .hint_text(tr("auto, ou 1,2,4 (répété)"))
                            .desired_width(140.0),
                    );
                    match luma_render::book::parse_rhythm(&text) {
                        Ok(r) => b.rhythm = r,
                        Err(_) => {
                            ui.label(RichText::new(tr("1, 2 ou 4 photos par page")).color(theme::ACCENT));
                        }
                    }
                });
                ui.data_mut(|d| d.insert_temp(id, text));
                // Les pages d'après l'orientation des vignettes.
                let landscape: Vec<bool> = photos
                    .iter()
                    .map(|p| self.thumbs.get(p).is_none_or(|t| t.size()[0] >= t.size()[1]))
                    .collect();
                let pages = b.sheets(&landscape).len();
                ui.label(
                    RichText::new(trf(
                        "Environ {pages} pages : la couverture, puis une, deux ou quatre photos par page",
                        &[("pages", &pages)],
                    ))
                    .size(11.0)
                    .color(theme::DIM),
                );
                ui.horizontal(|ui| {
                    make = ui
                        .add_enabled(self.printing.is_none() && n > 0, egui::Button::new(tr("Créer le livre (PDF)…")))
                        .clicked();
                    if ui
                        .add_enabled(n > 0, egui::Button::new(tr("Pages du livre…")))
                        .on_hover_text(tr("Chaque page et ses photos ; glisser une photo sur une autre les échange"))
                        .clicked()
                    {
                        self.book_pages_open = true;
                    }
                });
                ui.add_space(12.0);
                section_title(ui, tr("Galerie web"));
                let g = &mut self.gallery;
                egui::Grid::new("galerie").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label(RichText::new(tr("Titre")).color(theme::DIM));
                    ui.text_edit_singleline(&mut g.title);
                    ui.end_row();
                    ui.label(RichText::new(tr("Description")).color(theme::DIM));
                    ui.text_edit_singleline(&mut g.description);
                    ui.end_row();
                });
                ui.checkbox(&mut g.captions, tr("La légende de chaque photo (son titre, sinon son nom)"));
                web = ui
                    .add_enabled(self.printing.is_none() && n > 0, egui::Button::new(tr("Exporter la galerie…")))
                    .on_hover_text(tr("Une page web et ses photos, allégées pour le web : à mettre en ligne telle quelle"))
                    .clicked();
                if let Some(p) = &self.printing {
                    let done = p.placed.load(std::sync::atomic::Ordering::Relaxed);
                    ui.add(egui::ProgressBar::new(done as f32 / p.total.max(1) as f32).text(trf(
                        "Mise en page : {done} sur {n}",
                        &[("done", &done), ("n", &p.total)],
                    )));
                }
            });
        self.outputs_open = open && !play;
        if play {
            self.start_slideshow();
        }
        let name = |title: &str, fallback: &str| {
            let t = title.trim().replace(['/', '\\', ':'], "-");
            if t.is_empty() {
                tr(fallback).to_owned()
            } else {
                t
            }
        };
        if export
            && let Some(dir) = crate::dialogs::slideshow_dir(&name(&self.show.title, "Diaporama"))
        {
            self.export_slideshow(dir);
        }
        if make
            && let Some(out) =
                crate::dialogs::save_pdf(&format!("{}.pdf", name(&self.book.title, "Livre photo")))
        {
            self.make_book(out);
        }
    }

    /// La fenêtre « Imprimer » : le papier, la disposition, le profil de l'imprimante,
    /// l'aperçu de la première page.
    fn print_window(&mut self, ctx: &egui::Context) {
        use luma_render::print::{PAPERS, Paper};
        let mut open = self.print_open;
        let mut go = false;
        let photos = self.rename_targets();
        let n = photos.len();
        let paper_label = |p: &Paper| -> String {
            match p.name {
                "Letter" => tr("Lettre US").to_owned(),
                name if name.starts_with(|c: char| c.is_ascii_digit()) => {
                    format!("{} cm", name.replace('x', " × "))
                }
                name => format!("{name} ({:.0} × {:.0} mm)", p.width, p.height),
            }
        };
        egui::Window::new(trf("Imprimer {n} photos", &[("n", &n)]))
            .id(egui::Id::unique("fenetre-imprimer"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(420.0);
                let l = &mut self.print_layout;
                egui::Grid::new("imprimer")
                    .num_columns(2)
                    .spacing([10.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new(tr("Papier")).color(theme::DIM));
                        egui::ComboBox::from_id_salt("papier")
                            .selected_text(paper_label(&l.paper))
                            .show_ui(ui, |ui| {
                                for p in PAPERS {
                                    ui.selectable_value(&mut l.paper, p, paper_label(&p));
                                }
                            });
                        ui.end_row();
                        ui.label(RichText::new(tr("Orientation")).color(theme::DIM));
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut l.landscape, false, tr("Portrait"));
                            ui.selectable_value(&mut l.landscape, true, tr("Paysage"));
                        });
                        ui.end_row();
                        ui.label(RichText::new(tr("Disposition")).color(theme::DIM));
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(l.per_page() == 1, tr("Une photo par page"))
                                .clicked()
                            {
                                (l.columns, l.rows, l.captions) = (1, 1, false);
                            }
                            if ui
                                .selectable_label(l.per_page() > 1, tr("Planche contact"))
                                .clicked()
                                && l.per_page() == 1
                            {
                                (l.columns, l.rows, l.gap, l.captions) = (4, 5, 4.0, true);
                            }
                        });
                        ui.end_row();
                        ui.label(RichText::new(tr("Grille")).color(theme::DIM));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut l.columns).range(1..=10));
                            ui.label("×");
                            ui.add(egui::DragValue::new(&mut l.rows).range(1..=10));
                            ui.label(RichText::new(tr("colonnes × lignes")).color(theme::DIM));
                        });
                        ui.end_row();
                        ui.label(RichText::new(tr("Marges")).color(theme::DIM));
                        ui.add(
                            egui::Slider::new(&mut l.margin, 0.0..=40.0)
                                .suffix(" mm")
                                .step_by(1.0),
                        );
                        ui.end_row();
                        if l.per_page() > 1 {
                            ui.label(RichText::new(tr("Écart")).color(theme::DIM));
                            ui.add(
                                egui::Slider::new(&mut l.gap, 0.0..=20.0)
                                    .suffix(" mm")
                                    .step_by(0.5),
                            );
                            ui.end_row();
                        }
                        ui.label(RichText::new(tr("Résolution")).color(theme::DIM));
                        egui::ComboBox::from_id_salt("resolution")
                            .selected_text(trf("{dpi} ppp", &[("dpi", &l.dpi)]))
                            .show_ui(ui, |ui| {
                                for dpi in [150, 240, 300, 360, 600] {
                                    ui.selectable_value(
                                        &mut l.dpi,
                                        dpi,
                                        trf("{dpi} ppp", &[("dpi", &dpi)]),
                                    );
                                }
                            });
                        ui.end_row();
                        ui.label(RichText::new(tr("Accentuation")).color(theme::DIM));
                        let medium = l.sharpen.map(|s| s.medium);
                        egui::ComboBox::from_id_salt("accentuation")
                            .selected_text(match medium {
                                None => tr("Aucune"),
                                Some(luma_render::export::Medium::Glossy) => tr("Papier brillant"),
                                Some(_) => tr("Papier mat"),
                            })
                            .show_ui(ui, |ui| {
                                use luma_render::export::{Medium, OutputSharpen, Strength};
                                let sharpen = |medium| {
                                    Some(OutputSharpen {
                                        medium,
                                        strength: Strength::Standard,
                                    })
                                };
                                ui.selectable_value(&mut l.sharpen, None, tr("Aucune"));
                                ui.selectable_value(
                                    &mut l.sharpen,
                                    sharpen(Medium::Matte),
                                    tr("Papier mat"),
                                );
                                ui.selectable_value(
                                    &mut l.sharpen,
                                    sharpen(Medium::Glossy),
                                    tr("Papier brillant"),
                                );
                            });
                        ui.end_row();
                    });
                ui.checkbox(&mut l.captions, tr("Le nom de chaque photo dessous"));
                ui.checkbox(
                    &mut l.fill,
                    tr("Remplir les cases (photos recadrées au centre)"),
                );
                ui.checkbox(
                    &mut l.crop_marks,
                    tr("Repères de coupe aux coins des photos"),
                );
                ui.horizontal(|ui| {
                    ui.label(tr("Texte au bas de la page"));
                    ui.add(
                        egui::TextEdit::singleline(&mut l.text)
                            .hint_text(tr("un titre, un nom, des droits"))
                            .desired_width(220.0),
                    );
                });
                let (pages, (w, h)) = (l.pages(n), l.page_mm());
                let layout = l.clone();
                ui.add_space(6.0);
                // Le profil de l'imprimante : celui de l'épreuvage.
                ui.horizontal(|ui| {
                    if self.proofing.profile.is_some() {
                        ui.checkbox(
                            &mut self.print_convert,
                            tr("Convertir vers le profil de l'imprimante"),
                        )
                        .on_hover_text(tr("Imprimez alors sans gestion des couleurs du pilote"));
                        ui.label(
                            RichText::new(self.proofing.description.clone()).color(theme::DIM),
                        );
                    } else {
                        ui.label(
                            RichText::new(tr(
                                "Sans profil d'imprimante : les pages restent en sRGB.",
                            ))
                            .color(theme::DIM),
                        );
                    }
                });
                if !crate::touch()
                    && ui.small_button(tr("Choisir un profil ICC…")).clicked()
                    && let Some(path) = crate::dialogs::pick_icc()
                {
                    self.load_proof_profile(&path);
                }
                ui.add_space(8.0);
                self.print_preview(ui, &layout, &photos);
                ui.label(
                    RichText::new(trf(
                        "{pages} page(s) de {w} × {h} mm",
                        &[
                            ("pages", &pages),
                            ("w", &format!("{w:.0}")),
                            ("h", &format!("{h:.0}")),
                        ],
                    ))
                    .size(11.0)
                    .color(theme::DIM),
                );
                ui.add_space(6.0);
                match &self.printing {
                    Some(p) => {
                        let done = p.placed.load(std::sync::atomic::Ordering::Relaxed);
                        ui.add(
                            egui::ProgressBar::new(done as f32 / p.total.max(1) as f32).text(trf(
                                "Mise en page : {done} sur {n}",
                                &[("done", &done), ("n", &p.total)],
                            )),
                        );
                    }
                    None => {
                        go = ui
                            .add_enabled(pages > 0, egui::Button::new(tr("Créer le PDF…")))
                            .clicked();
                    }
                }
            });
        if go && let Some(out) = crate::dialogs::save_pdf(&format!("{}.pdf", tr("Tirage"))) {
            self.print_pdf(out);
        }
        self.print_open = open;
    }

    /// La première page en petit : le papier, les cases, les vignettes des photos.
    fn print_preview(&self, ui: &mut Ui, layout: &luma_render::print::Layout, photos: &[PathBuf]) {
        let (pw, ph) = layout.page_px();
        let scale = (200.0 / pw.max(ph) as f32).min(1.0);
        let size = vec2(pw as f32 * scale, ph as f32 * scale);
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), size.y), egui::Sense::hover());
        let page = Rect::from_center_size(rect.center(), size);
        let painter = ui.painter_at(rect);
        painter.rect_filled(
            page.translate(vec2(2.0, 3.0)),
            2.0,
            Color32::from_black_alpha(90),
        );
        painter.rect_filled(page, 2.0, Color32::from_gray(0xf4));
        for (cell, path) in layout.cells().iter().zip(photos) {
            let r = Rect::from_min_size(
                page.min + vec2(cell.x as f32, cell.y as f32) * scale,
                vec2(cell.width as f32, cell.height as f32) * scale,
            );
            match self.thumbs.get(path) {
                Some(t) => {
                    let [w, h] = t.size();
                    let (tw, th) = layout.target(cell, w, h);
                    let shown = Rect::from_center_size(
                        r.center(),
                        vec2(tw.min(cell.width) as f32, th.min(cell.height) as f32) * scale,
                    );
                    // Remplie : le milieu de la vignette.
                    let uv = Rect::from_center_size(
                        pos2(0.5, 0.5),
                        vec2(
                            (cell.width as f32 / tw as f32).min(1.0),
                            (cell.height as f32 / th as f32).min(1.0),
                        ),
                    );
                    let shown = if cell.caption > 0 {
                        shown.translate(vec2(0.0, r.max.y - shown.max.y))
                    } else {
                        shown
                    };
                    painter.image(t.id(), shown, uv, Color32::WHITE);
                }
                None => {
                    painter.rect_filled(r, 0.0, Color32::from_gray(0xc8));
                }
            }
            if cell.caption > 0 {
                let band = Rect::from_min_size(
                    pos2(
                        r.min.x + r.width() * 0.25,
                        r.max.y + cell.caption as f32 * scale * 0.35,
                    ),
                    vec2(
                        r.width() * 0.5,
                        (cell.caption as f32 * scale * 0.3).max(1.0),
                    ),
                );
                painter.rect_filled(band, 0.0, Color32::from_gray(0x99));
            }
        }
    }

    /// La fenêtre « Renommer » : le modèle, ses repères, les premiers noms obtenus.
    fn rename_window(&mut self, ctx: &egui::Context) {
        let mut open = self.rename_open;
        let mut go = false;
        let count = self.rename_targets().len();
        egui::Window::new(trf("Renommer {n} photos", &[("n", &count)]))
            .id(egui::Id::unique("fenetre-renommer"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(RichText::new(tr("Modèle")).size(11.5).color(theme::DIM));
                ui.add(
                    egui::TextEdit::singleline(&mut self.rename_template)
                        .desired_width(f32::INFINITY),
                );
                let tokens: Vec<String> = luma_render::export::tokens()
                    .map(|t| format!("{{{t}}}"))
                    .collect();
                ui.label(
                    RichText::new(tokens.join(" "))
                        .size(10.5)
                        .color(theme::DIM),
                );
                ui.add_space(6.0);
                let stale = self
                    .rename_preview
                    .as_ref()
                    .is_none_or(|(t, _)| *t != self.rename_template);
                if stale {
                    let plan = self.rename_plan(&self.rename_template);
                    self.rename_preview = Some((self.rename_template.clone(), plan));
                }
                let mut ready = false;
                match self.rename_preview.as_ref().map(|(_, p)| p) {
                    Some(Ok(plan)) => {
                        ready = !plan.files.is_empty();
                        let name = |p: &Path| {
                            p.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        };
                        for m in plan.photos.iter().take(5) {
                            ui.label(
                                RichText::new(format!("{}  →  {}", name(&m.from), name(&m.to)))
                                    .size(11.0)
                                    .monospace(),
                            );
                        }
                        if plan.photos.len() > 5 {
                            ui.label(
                                RichText::new(trf(
                                    "… et {n} autres",
                                    &[("n", &(plan.photos.len() - 5))],
                                ))
                                .size(11.0)
                                .color(theme::DIM),
                            );
                        }
                        if !ready {
                            ui.label(
                                RichText::new(tr("Les noms ne changent pas."))
                                    .size(11.0)
                                    .color(theme::DIM),
                            );
                        }
                    }
                    Some(Err(e)) => {
                        ui.colored_label(Color32::from_rgb(255, 120, 100), e);
                    }
                    None => {}
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new(tr(
                        "Le RAW et le JPEG d'une même prise gardent un même nom ; le XMP et les copies virtuelles suivent.",
                    ))
                    .size(10.5)
                    .color(theme::DIM),
                );
                ui.add_space(6.0);
                go = ui
                    .add_enabled(ready, egui::Button::new(tr("Renommer")))
                    .clicked();
            });
        if go {
            let template = self.rename_template.clone();
            match self.rename_photos(&template) {
                Ok(n) => self.status = trf("{n} photos renommées", &[("n", &n)]),
                Err(e) => self.status = trf("Renommage impossible : {error}", &[("error", &e)]),
            }
        }
        self.rename_open = open && !go;
    }

    fn sync_targets(&self) -> Vec<PathBuf> {
        let current = self.current_path().map(Path::to_path_buf);
        self.selected
            .iter()
            .filter_map(|i| self.folder.get(*i).cloned())
            .filter(|p| Some(p) != current.as_ref())
            .collect()
    }

    /// Donne les réglages de la photo ouverte (ces groupes‑là) aux autres photos
    /// choisies : leur XMP est écrit, leur vignette refaite. Rend le nombre de photos.
    pub fn sync_selected(&mut self, groups: &[luma_project::paste::Group]) -> usize {
        let source = self.settings.clone();
        let mut done = 0;
        for path in self.sync_targets() {
            let before = read_sidecar(&path).unwrap_or_default();
            let after = luma_project::paste::paste(&before, &source, groups);
            let meta = self.meta.get(&path).cloned().unwrap_or_default();
            match write_sidecar(&path, &after, &meta) {
                Ok(()) => {
                    done += 1;
                    self.edits.insert(path.clone(), presets::summary(&after));
                    self.worker.thumbnail(path);
                }
                Err(e) => self.status = trf("XMP non enregistré : {error}", &[("error", &e)]),
            }
        }
        self.status = trf("Réglages synchronisés sur {n} photo(s).", &[("n", &done)]);
        done
    }

    /// La balance automatique de chaque photo choisie (de la photo ouverte sinon), en
    /// arrière‑plan : chacune la sienne.
    pub fn auto_balance_selected(&mut self) {
        let mut paths = self.keyword_targets();
        paths.sort();
        self.auto_pending = paths.len();
        self.status = trf(
            "Balance automatique de {n} photo(s)…",
            &[("n", &paths.len())],
        );
        self.worker.auto_balance(paths);
    }

    /// Les balances automatiques du lot sont‑elles toutes arrivées ?
    pub fn auto_balance_done(&self) -> bool {
        self.auto_pending == 0
    }

    fn apply_auto_balance(&mut self, path: PathBuf, t: luma_color::TempTint) {
        let wb = WhiteBalance::Custom {
            temperature: t.temperature,
            tint: t.tint,
        };
        self.auto_pending = self.auto_pending.saturating_sub(1);
        if self.current_path() == Some(path.as_path()) {
            self.settings.white_balance = wb;
            self.wb_label = "Auto";
            self.commit(tr("Balance : auto").into());
            self.touch();
        } else {
            let mut s = read_sidecar(&path).unwrap_or_default();
            s.white_balance = wb;
            let meta = self.meta.get(&path).cloned().unwrap_or_default();
            if let Err(e) = write_sidecar(&path, &s, &meta) {
                self.status = trf("XMP non enregistré : {error}", &[("error", &e)]);
            }
            self.edits.insert(path.clone(), presets::summary(&s));
            self.worker.thumbnail(path);
        }
        if self.auto_pending == 0 {
            self.status = tr("Balance automatique appliquée.").into();
        }
    }

    /// Plusieurs photos choisies : les synchroniser, ou leur donner chacune sa balance
    /// automatique.
    fn selection_actions(&mut self, ui: &mut Ui) {
        if self.selected.len() < 2 {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(trf("{n} photos choisies", &[("n", &self.selected.len())]))
                    .size(11.5)
                    .color(theme::DIM),
            );
            if ui
                .small_button(tr("Synchroniser…"))
                .on_hover_text(tr(
                    "Les réglages de la photo ouverte, aux autres photos choisies (Maj+Cmd+S)",
                ))
                .clicked()
            {
                self.act("sync");
            }
            if ui
                .small_button(tr("Balance auto"))
                .on_hover_text(tr(
                    "Chaque photo choisie reçoit sa propre balance automatique",
                ))
                .clicked()
            {
                self.auto_balance_selected();
            }
            if ui
                .small_button(tr("Renommer…"))
                .on_hover_text(tr("Renommer les photos choisies d'après un modèle"))
                .clicked()
            {
                self.rename_open = true;
            }
            if ui
                .small_button(tr("Imprimer…"))
                .on_hover_text(tr("Mise en page pour l'imprimante : une photo par page ou une planche contact, en PDF"))
                .clicked()
            {
                self.print_open = true;
            }
            if ui
                .small_button(tr("Livre, diaporama, galerie…"))
                .on_hover_text(tr("Un diaporama (dans l'app ou exporté), un livre photo en PDF et une galerie web"))
                .clicked()
            {
                self.outputs_open = true;
            }
        });
        ui.add_space(6.0);
    }

    /// Une photo cachée dans une pile repliée (toutes sauf la première).
    fn folded_away(&self, path: &Path) -> bool {
        self.stacking
            && self.stack_of.get(path).is_some_and(|&k| {
                let cover = &self.stacks[k].photos[0];
                cover != path && !self.unfolded.contains(cover)
            })
    }

    /// Les piles du film (rafales et séries d'exposition), d'après la photothèque.
    fn find_stacks(&mut self) {
        let entries: Vec<&luma_library::Entry> = self
            .folder
            .iter()
            .filter_map(|p| self.catalog.get(p))
            .collect();
        self.stacks = luma_library::stacks::stacks(entries);
        self.stack_of = self
            .stacks
            .iter()
            .enumerate()
            .flat_map(|(k, s)| s.photos.iter().map(move |p| (p.clone(), k)))
            .collect();
    }

    /// Les piles du film.
    pub fn stacks(&self) -> &[luma_library::stacks::Stack] {
        &self.stacks
    }

    /// Empile (ou non) les rafales et les séries d'exposition.
    pub fn set_stacking(&mut self, on: bool) {
        self.stacking = on;
    }

    /// Déplie ou replie la pile de cette photo.
    pub fn toggle_stack(&mut self, path: &Path) {
        if let Some(&k) = self.stack_of.get(path) {
            let cover = self.stacks[k].photos[0].clone();
            if !self.unfolded.remove(&cover) {
                self.unfolded.insert(cover.clone());
            }
            // Repliée, la photo ouverte devient sa couverture.
            if !self.unfolded.contains(&cover)
                && self.current_path() != Some(cover.as_path())
                && self
                    .stack_of
                    .get(self.current_path().unwrap_or(Path::new("")))
                    == Some(&k)
                && let Some(i) = self.folder.iter().position(|p| *p == cover)
            {
                self.select(i);
            }
        }
    }

    /// Les mots‑clés du dossier et leur nombre de photos, par ordre alphabétique (sans
    /// tenir compte des majuscules ni des accents).
    pub fn folder_keywords(&self) -> Vec<(String, usize)> {
        let mut groups: BTreeMap<String, (String, usize)> = BTreeMap::new();
        for p in &self.folder {
            for k in self
                .meta
                .get(p)
                .map(|m| m.keywords.as_slice())
                .unwrap_or_default()
            {
                groups.entry(fold(k)).or_insert_with(|| (k.clone(), 0)).1 += 1;
            }
        }
        groups.into_values().collect()
    }

    /// Les photos que touche un mot‑clé : la sélection (⌘‑clic, Maj‑clic), sinon la
    /// photo affichée.
    fn keyword_targets(&self) -> Vec<PathBuf> {
        if self.selected.is_empty() {
            self.current_path()
                .map(Path::to_path_buf)
                .into_iter()
                .collect()
        } else {
            self.selected
                .iter()
                .filter_map(|i| self.folder.get(*i).cloned())
                .collect()
        }
    }

    /// Les mots‑clés des photos visées, sans doublon, dans l'ordre où ils viennent.
    pub fn target_keywords(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for p in self.keyword_targets() {
            for k in self
                .meta
                .get(&p)
                .map(|m| m.keywords.as_slice())
                .unwrap_or_default()
            {
                if !out.iter().any(|o| fold(o) == fold(k)) {
                    out.push(k.clone());
                }
            }
        }
        out
    }

    /// Change les métadonnées de plusieurs photos : celle qui est ouverte est
    /// enregistrée avec ses réglages, les autres tout de suite.
    fn edit_meta(&mut self, paths: &[PathBuf], f: impl Fn(&mut Meta)) {
        let current = self.current_path().map(Path::to_path_buf);
        for p in paths {
            let meta = self.meta.entry(p.clone()).or_default();
            f(meta);
            if Some(p) == current.as_ref() {
                self.touch();
            } else if let Err(e) = write_meta(p, meta) {
                self.status = trf("XMP non enregistré : {error}", &[("error", &e)]);
            }
        }
    }

    /// Ajoute des mots‑clés (séparés par des virgules) à la sélection, ou à la photo
    /// affichée ; un mot‑clé déjà là (aux majuscules et accents près) n'est pas doublé.
    /// Renvoie le nombre de photos touchées.
    pub fn add_keywords(&mut self, text: &str) -> usize {
        // « Lieux > France > Paris » (ou « Lieux|France|Paris ») : un mot‑clé hiérarchique.
        let words: Vec<String> = text
            .split([',', ';'])
            .map(|w| {
                w.split(['>', luma_project::xmp::LEVEL])
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join("|")
            })
            .filter(|w| !w.is_empty())
            .collect();
        let targets = self.keyword_targets();
        if words.is_empty() || targets.is_empty() {
            return 0;
        }
        self.edit_meta(&targets, |m| {
            for w in &words {
                if !m.keywords.iter().any(|k| fold(k) == fold(w)) {
                    m.keywords.push(w.clone());
                }
            }
        });
        targets.len()
    }

    /// Retire un mot‑clé de la sélection, ou de la photo affichée.
    pub fn remove_keyword(&mut self, word: &str) {
        let targets = self.keyword_targets();
        self.edit_meta(&targets, |m| m.keywords.retain(|k| fold(k) != fold(word)));
    }

    /// La note et le drapeau de la photo affichée.
    pub fn current_meta(&self) -> Meta {
        self.current_path()
            .and_then(|p| self.meta.get(p))
            .cloned()
            .unwrap_or_default()
    }

    /// Note, drapeau ou libellé : à toutes les photos choisies s'il y en a plusieurs,
    /// sinon à la photo ouverte.
    fn set_meta(&mut self, f: impl Fn(&mut Meta)) {
        let paths = if self.selected.len() >= 2 {
            self.keyword_targets()
        } else {
            self.current_path()
                .map(Path::to_path_buf)
                .into_iter()
                .collect()
        };
        self.edit_meta(&paths, f);
    }

    /// Prête : photo chargée et dernier rendu affiché.
    pub fn is_idle(&self) -> bool {
        self.loading.is_none()
            && !self.is_indexing()
            && self.photo.is_some()
            && self.folder.iter().all(|p| self.thumbs.contains_key(p))
            && self.shown == self.generation
            && self.texture.is_some()
            && !self.tiles.is_loading()
            && self.printing.is_none()
    }

    fn select(&mut self, index: usize) {
        if index >= self.folder.len() || self.current == Some(index) && self.photo.is_some() {
            return;
        }
        self.flush_xmp();
        self.current = Some(index);
        let path = self.folder[index].clone();
        self.status = trf("Ouverture de {file}…", &[("file", &file_name(&path))]);
        self.loading = Some(path.clone());
        self.worker.open(path);
    }

    fn handle_events(&mut self, ctx: &egui::Context) {
        for event in self.worker.poll() {
            match event {
                Event::Loaded { path, result } => {
                    if self.loading.as_ref() != Some(&path) {
                        continue;
                    }
                    self.loading = None;
                    match result {
                        Ok((photo, sidecar)) => {
                            let from_xmp = sidecar.is_some();
                            let defaults = self.defaults_for(&photo);
                            self.settings = sidecar.map(|s| *s).unwrap_or_else(|| defaults.clone());
                            self.history = vec![(tr("Importation").into(), defaults)];
                            if from_xmp {
                                self.history
                                    .push((tr("Réglages XMP").into(), self.settings.clone()));
                            }
                            self.history_pos = self.history.len() - 1;
                            self.selected_mask = None;
                            self.preset = None;
                            // Le recadrage ou le redressement en cours était celui de la
                            // photo d'avant (Échap y remettrait son cadre).
                            self.crop_tool = None;
                            self.guiding = false;
                            self.fresh_from = self.generation + 1;
                            let raw = photo.raw();
                            let e = &raw.exif;
                            let mut info = format!("{} {}", raw.make, raw.model);
                            if let Some(iso) = e.iso {
                                info += &format!(" · ISO {iso}");
                            }
                            if let Some(f) = e.focal_length {
                                info += &format!(" · {f:.0} mm");
                            }
                            if let Some(a) = e.aperture {
                                info += &format!(" · f/{}", format_value(a, 1, false));
                            }
                            if let Some(t) = e.exposure_time {
                                info += &if t < 1.0 {
                                    format!(" · 1/{:.0} s", 1.0 / t)
                                } else {
                                    format!(" · {t:.1} s")
                                };
                            }
                            self.status = info;
                            self.photo = Some(photo);
                            self.original_asked = None;
                            self.zoom = false;
                        }
                        Err(e) => {
                            self.status = trf(
                                "Impossible d'ouvrir {file} : {error}",
                                &[("file", &file_name(&path)), ("error", &e)],
                            )
                        }
                    }
                }
                Event::Reloaded { path, photo } => {
                    // La photo affichée passe sur son doublon (ou revient à l'original) :
                    // mêmes réglages, l'aperçu se refait.
                    if self.loading.is_none()
                        && self.photo.is_some()
                        && self.current_path() == Some(path.as_path())
                    {
                        self.photo = Some(photo);
                        self.last_request = None;
                    }
                }
                Event::Rendered(frame) => {
                    // Pendant l'ouverture, et ensuite tant qu'un rendu de la photo d'avant
                    // arrive : l'aperçu rapide de la nouvelle photo reste.
                    if frame.generation < self.shown.max(self.fresh_from) || self.loading.is_some()
                    {
                        continue;
                    }
                    self.shown = frame.generation;
                    self.render_ms = frame.millis;
                    self.render_gpu = frame.gpu;
                    // Sur iPhone et iPad, la durée des aperçus dans le journal : les 30
                    // premiers, puis un sur vingt.
                    if crate::touch() {
                        self.renders_logged += 1;
                        if self.renders_logged <= 30 || self.renders_logged.is_multiple_of(20) {
                            crate::journal(&format!(
                                "aperçu {} × {} en {:.0} ms ({})",
                                frame.size[0],
                                frame.size[1],
                                frame.millis,
                                if frame.gpu { "GPU" } else { "processeur" }
                            ));
                        }
                    }
                    self.histogram = frame.histogram;
                    match &frame.scope {
                        Some(s) => {
                            let image =
                                ColorImage::from_rgba_unmultiplied([s.width, s.height], &s.rgba);
                            match &mut self.scope_texture {
                                Some(t) => t.set(image, TextureOptions::LINEAR),
                                None => {
                                    self.scope_texture = Some(ctx.load_texture(
                                        "instrument",
                                        image,
                                        TextureOptions::LINEAR,
                                    ))
                                }
                            }
                        }
                        None => self.scope_texture = None,
                    }
                    self.image_size = frame.size;
                    self.pick_map = pick_map(frame.size, &frame.rgba);
                    // Une nouvelle base : la zone d'un masque à plage est à relire.
                    if frame.scene.is_some() {
                        self.mask_overlay = None;
                    }
                    self.scene_map = frame.scene.clone();
                    if let Some(found) = frame.dust.clone()
                        && self.dust_pending
                    {
                        self.dust_pending = false;
                        self.add_dust(found);
                    }
                    let image = ColorImage::from_rgba_unmultiplied(frame.size, &frame.rgba);
                    match &mut self.texture {
                        Some(t) => t.set(image, TextureOptions::LINEAR),
                        None => {
                            self.texture =
                                Some(ctx.load_texture("photo", image, TextureOptions::LINEAR))
                        }
                    }
                    // La mesure au point (Mac, iPad) : l'aperçu gardé tel quel, sans
                    // copie ; sur iPhone, où elle ne s'affiche pas, rien de gardé.
                    if !self.compact {
                        let frame = *frame;
                        self.probe_image = (frame.size, frame.clean.unwrap_or(frame.rgba));
                    }
                }
                Event::Glimpse { path, size, rgba } => {
                    // Seulement pour la photo qu'on attend, et tant que son rendu n'est
                    // pas là.
                    if self.loading.as_ref() == Some(&path) {
                        self.image_size = size;
                        let image = ColorImage::from_rgba_unmultiplied(size, &rgba);
                        match &mut self.texture {
                            Some(t) => t.set(image, TextureOptions::LINEAR),
                            None => {
                                self.texture =
                                    Some(ctx.load_texture("photo", image, TextureOptions::LINEAR))
                            }
                        }
                    }
                }
                Event::Thumbnail { path, size, rgba } => {
                    if let Some(print) =
                        luma_library::fingerprint::fingerprint(&rgba, size[0], size[1])
                    {
                        self.prints.insert(path.clone(), print);
                    }
                    let image = ColorImage::from_rgba_unmultiplied(size, &rgba);
                    let name = format!("vignette {}", path.display());
                    self.thumbs
                        .insert(path, ctx.load_texture(name, image, TextureOptions::LINEAR));
                }
                Event::MergeProgress { fraction, label } => {
                    if let Some(m) = &mut self.merging {
                        m.0 = fraction;
                        m.1 = label;
                    }
                }
                Event::Merged { result } => {
                    self.merging = None;
                    match result {
                        Ok(path) => {
                            self.status = trf(
                                "Fusion enregistrée : {file}",
                                &[("file", &file_name(&path))],
                            );
                            self.selected.clear();
                            self.tool = Tool::Light;
                            self.module = Module::Develop;
                            self.open_path(&path);
                        }
                        Err(e) => {
                            self.status = trf("Fusion impossible : {error}", &[("error", &e)])
                        }
                    }
                }
                Event::AutoBalanced { path, balance } => self.apply_auto_balance(path, balance),
                Event::Compared {
                    tag,
                    index,
                    size,
                    rgba,
                } => {
                    if let Some(c) = &mut self.compare
                        && c.tag == tag
                        && index < c.textures.len()
                    {
                        let image = ColorImage::from_rgba_unmultiplied(size, &rgba);
                        c.textures[index] = Some(ctx.load_texture(
                            format!("comparaison {index}"),
                            image,
                            TextureOptions::LINEAR,
                        ));
                    }
                }
                Event::Preview {
                    tag,
                    index,
                    size,
                    rgba,
                } => {
                    if tag == self.previews_tag && index < self.previews.len() {
                        let image = ColorImage::from_rgba_unmultiplied(size, &rgba);
                        self.previews[index] = Some(ctx.load_texture(
                            format!("préréglage {index}"),
                            image,
                            TextureOptions::LINEAR,
                        ));
                    }
                }
                Event::Measured { result } => {
                    self.measuring = false;
                    match result {
                        Ok((name, before, after)) => {
                            self.status = trf(
                                "Pellicule mesurée : « {name} » (écart du numérique au film {before}, avec le film {after})",
                                &[
                                    ("name", &name),
                                    ("before", &format_value(before, 1, false)),
                                    ("after", &format_value(after, 1, false)),
                                ],
                            );
                            self.refresh_luts();
                            if let Some(k) = self.luts.iter().position(|n| *n == name) {
                                self.choose_film(PRESETS.len() + k);
                            }
                        }
                        Err(e) => {
                            self.status = trf("Mesure impossible : {error}", &[("error", &e)])
                        }
                    }
                }
                Event::Exported {
                    done,
                    total,
                    result,
                } => {
                    let count = if total > 1 {
                        format!(" ({done}/{total})")
                    } else {
                        String::new()
                    };
                    // Sur iPhone et iPad, chaque image exportée va aussi dans la Pellicule.
                    if let Ok(p) = &result
                        && luma_apple::photos::AVAILABLE
                    {
                        luma_apple::photos::save_to_library(p);
                        self.library_sent += 1;
                    }
                    self.status = match result {
                        // Au doigt, le nom du fichier seul : le chemin du dossier de l'app
                        // ne dit rien et ne tient pas sur l'écran.
                        Ok(p) if crate::touch() => trf(
                            "Exporté{count} : {path}",
                            &[("count", &count), ("path", &file_name(&p))],
                        ),
                        Ok(p) => trf(
                            "Exporté{count} : {path}",
                            &[("count", &count), ("path", &p.display())],
                        ),
                        Err(e) => trf(
                            "Échec de l'export{count} : {error}",
                            &[("count", &count), ("error", &e)],
                        ),
                    };
                }
            }
        }
    }

    /// Une étape d'historique (curseur relâché, choix fait).
    /// Le geste de ciblage direct : au début, les bandes du mélangeur de la couleur
    /// sous le doigt ; en glissant, vers le haut pour plus, dans l'onglet ouvert (teinte,
    /// saturation ou luminance), chaque bande selon son poids.
    fn target_drag(&mut self, ui: &Ui, r: &egui::Response, target: Rect) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        if r.dragged() && self.target_bands.is_none() {
            // Au premier instant du geste : la couleur là où le doigt s'est posé.
            let start = ui
                .input(|i| i.pointer.press_origin())
                .or(r.interact_pointer_pos());
            self.target_bands = Some(start.and_then(|p| {
                let (nx, ny) = (
                    (p.x - target.min.x) / target.width(),
                    (p.y - target.min.y) / target.height(),
                );
                pick_color(&self.pick_map, nx, ny).and_then(luma_image::color::mixer_bands_at)
            }));
        }
        if let Some(Some(bands)) = self.target_bands
            && r.dragged()
        {
            // Une demi‑unité par point : 200 points d'écran couvrent toute la plage.
            let dy = -r.drag_delta().y * 0.5;
            if dy != 0.0 {
                for (band, w) in bands {
                    let v = match self.hsl_tab {
                        0 => &mut self.settings.hsl.hue[band],
                        1 => &mut self.settings.hsl.saturation[band],
                        _ => &mut self.settings.hsl.luminance[band],
                    };
                    *v = (*v + dy * w).clamp(-100.0, 100.0);
                }
                self.touch();
            }
        }
        if r.drag_stopped()
            && let Some(grab) = self.target_bands.take()
            && let Some(bands) = grab
        {
            let main = if bands[0].1 >= bands[1].1 {
                bands[0].0
            } else {
                bands[1].0
            };
            let kind = [tr("Teinte|mélangeur"), tr("Saturation"), tr("Luminance")][self.hsl_tab];
            self.commit(trf(
                "Ciblage direct : {kind} {color}",
                &[
                    ("kind", &kind),
                    ("color", &tr(HSL_NAMES[main]).to_lowercase()),
                ],
            ));
        }
    }

    fn commit(&mut self, label: String) {
        // Rien de changé : l'historique à refaire (après une annulation) reste.
        if self
            .history
            .get(self.history_pos)
            .is_some_and(|(_, s)| *s == self.settings)
        {
            return;
        }
        self.history.truncate(self.history_pos + 1);
        self.history.push((label, self.settings.clone()));
        self.history_pos = self.history.len() - 1;
    }

    fn restore(&mut self, pos: usize) {
        if let Some((_, s)) = self.history.get(pos) {
            self.settings = s.clone();
            self.history_pos = pos;
            self.touch();
        }
    }

    /// Les réglages ont changé : le XMP sera écrit dans un instant.
    /// Le menu de la barre du haut : la langue (appliquée tout de suite, gardée au
    /// prochain lancement) et le cache (sa taille, et le bouton pour le vider).
    /// Sur iPhone, un seul menu « ••• » : les commandes d'édition, puis la langue et le
    /// cache (sur Mac, ils sont dans la barre).
    fn more_menu(&mut self, ui: &mut Ui) {
        let mut action = None;
        let mut clear = false;
        let menu = ui.menu_button(
            RichText::new("•••").size(13.0).color(theme::TEXT),
            |ui| {
                for (label, act) in [
                    (tr("Annuler"), "undo"),
                    (tr("Rétablir"), "redo"),
                    (tr("Copier les réglages"), "copy"),
                    (tr("Coller les réglages"), "paste"),
                    (tr("Coller une partie des réglages"), "paste_some"),
                    (tr("Réinitialiser la photo"), "reset"),
                ] {
                    if ui.button(label).clicked() {
                        action = Some(act);
                        ui.close();
                    }
                }
                if ui.button(tr("Livre, diaporama, galerie…")).clicked() {
                    self.outputs_open = true;
                    ui.close();
                }
                if self.photos_button && ui.button(tr("Importer depuis Fichiers")).clicked() {
                    action = Some("open_files");
                    ui.close();
                }
                ui.separator();
                let current = crate::i18n::lang();
                ui.label(RichText::new(tr("Langue")).small().color(theme::DIM));
                for l in crate::i18n::Lang::ALL {
                    if ui.selectable_label(l == current, l.name()).clicked() {
                        crate::i18n::choose(l);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button(tr("Vider le cache")).clicked() {
                    clear = true;
                    ui.close();
                }
            },
        );
        menu.response.accessible_name(tr("Plus"));
        if let Some(a) = action {
            self.act(a);
        }
        if clear {
            self.clear_cache();
        }
    }

    fn settings_menu(&mut self, ui: &mut Ui) {
        let current = crate::i18n::lang();
        let label = RichText::new(current.code().to_uppercase())
            .size(11.0)
            .color(theme::DIM);
        let mut clear = false;
        let menu = ui.menu_button(label, |ui| {
            ui.label(RichText::new(tr("Langue")).small().color(theme::DIM));
            for l in crate::i18n::Lang::ALL {
                if ui.selectable_label(l == current, l.name()).clicked() {
                    crate::i18n::choose(l);
                    ui.close();
                }
            }
            if !crate::touch() {
                ui.separator();
                ui.label(RichText::new(tr("Espaces de travail")).small().color(theme::DIM));
                let builtin = crate::workspaces::builtin().len();
                let mut chosen = None;
                let mut removed = None;
                for (k, w) in self.all_workspaces().iter().enumerate() {
                    ui.horizontal(|ui| {
                        let key = if k < 9 { format!("  Cmd+{}", k + 1) } else { String::new() };
                        if ui.button(format!("{}{key}", tr(&w.name))).clicked() {
                            chosen = Some(k);
                        }
                        if k >= builtin && ui.small_button(tr("Retirer")).clicked() {
                            removed = Some(w.name.clone());
                        }
                    });
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.workspace_name)
                            .hint_text(tr("Nom"))
                            .desired_width(110.0),
                    );
                    if ui
                        .add_enabled(!self.workspace_name.trim().is_empty(), egui::Button::new(tr("Enregistrer l'espace actuel")))
                        .clicked()
                    {
                        let name = std::mem::take(&mut self.workspace_name);
                        self.save_workspace(&name);
                    }
                });
                if let Some(k) = chosen {
                    self.use_workspace(k);
                    ui.close();
                }
                if let Some(n) = removed {
                    self.remove_workspace(&n);
                }
                ui.separator();
                ui.label(RichText::new(tr("Clavier")).small().color(theme::DIM));
                if ui.button(tr("Raccourcis clavier…")).clicked() {
                    self.keys_open = true;
                    ui.close();
                }
            }
            ui.separator();
            ui.label(RichText::new(tr("Cache")).small().color(theme::DIM));
            let used = crate::dialogs::duplicates_dir()
                .map_or(0, |d| luma_render::duplicate::usage(&d))
                + self.photo.as_ref().map_or(0, |p| p.cached_bytes() as u64);
            ui.label(trf("Doublons, vignettes et aperçus : {size}", &[("size", &format_bytes(used))]));
            if ui
                .button(tr("Vider le cache"))
                .on_hover_text(tr(
                    "Supprime les doublons de travail et les vignettes enregistrées, et libère la mémoire des aperçus. Vos photos et vos réglages ne changent pas ; tout se refait à la prochaine ouverture.",
                ))
                .clicked()
            {
                clear = true;
                ui.close();
            }
        });
        menu.response
            .on_hover_text(tr("Langue, espaces de travail, clavier et cache"))
            .accessible_name(tr("Langue, espaces de travail, clavier et cache"));
        if clear {
            self.clear_cache();
        }
    }

    /// Les photos importées depuis Photos arrivent une à une dans le dossier de LÜMA :
    /// on le relit à chaque arrivée, en gardant la photo affichée.
    fn watch_imports(&mut self, ctx: &egui::Context) {
        if !luma_apple::photos::AVAILABLE {
            return;
        }
        // Le sélecteur demandé est‑il bien venu à l'écran ? Sinon, la fenêtre de
        // secours le présente (voir luma_apple::photos::watch).
        if let Some(what) = luma_apple::photos::watch() {
            crate::journal(&format!("importer : {what}"));
            self.import_result = Some(what.to_owned());
        }
        if luma_apple::photos::pending() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        // Les exports ajoutés à la Pellicule : Photos répond plus tard.
        let (saved, refused) = (
            luma_apple::photos::saved(),
            luma_apple::photos::save_failed(),
        );
        if refused != self.library_refused {
            self.library_refused = refused;
            self.status = tr(
                "Photos a refusé l'export : autorisez LÜMA à ajouter des photos (Réglages › LÜMA › Photos). Le fichier est dans LÜMA/, app Fichiers.",
            )
            .into();
        } else if saved != self.library_saved {
            let n = saved - self.library_saved;
            self.library_saved = saved;
            self.status = trf("{n} photo(s) ajoutée(s) à la Pellicule.", &[("n", &n)]);
        }
        if self.library_sent > saved + refused {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        let failed = luma_apple::photos::failed();
        if failed != self.imports_failed {
            crate::journal(&format!("import : {failed} échec(s)"));
            self.imports_failed = failed;
            self.status = trf(
                "{n} photo(s) n'ont pas pu être importée(s).",
                &[("n", &failed)],
            );
        }
        let now = ctx.input(|i| i.time);
        let count = luma_apple::photos::imported();
        if count != self.imports_seen {
            crate::journal(&format!("import : {count} photo(s) copiée(s)"));
            // D'autres suivent peut‑être : on guette encore une minute.
            self.imports_until = self.imports_until.max(now + 60.0);
            let new = count - self.imports_seen;
            self.imports_seen = count;
            let current = self.current_path().map(Path::to_path_buf);
            if let Some(dir) = crate::dialogs::documents() {
                match current {
                    Some(p) => self.open_path(&p),
                    None => self.open_path(&dir),
                }
                self.status = trf("{n} photo(s) importée(s).", &[("n", &new)]);
            }
        }
        // Tant qu'un import peut arriver, on regarde deux fois par seconde ; le reste du
        // temps, l'écran ne se redessine pas pour rien (la batterie).
        if now < self.imports_until {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }

    /// Sur iPhone et iPad, le système ferme une app qui dépasse sa mémoire. Une fois par
    /// seconde : s'il reste moins de 300 Mo, LÜMA libère ses caches d'aperçus et les
    /// vignettes loin de la photo en cours (elles se relisent en moins d'une
    /// milliseconde depuis le disque).
    fn watch_memory(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.memory_check < 1.0 {
            return;
        }
        self.memory_check = now;
        let Some(free) = luma_apple::available_memory() else {
            return;
        };
        if free >= 300 * 1024 * 1024 {
            return;
        }
        if let Some(photo) = &self.photo {
            photo.release_caches();
        }
        let near: std::collections::HashSet<PathBuf> = match self.current {
            Some(c) => self
                .folder
                .iter()
                .skip(c.saturating_sub(20))
                .take(41)
                .cloned()
                .collect(),
            None => Default::default(),
        };
        let far: Vec<PathBuf> = self
            .thumbs
            .keys()
            .filter(|p| !near.contains(*p))
            .cloned()
            .collect();
        for p in far {
            self.thumbs.remove(&p);
            self.evicted.insert(p);
        }
        self.last_request = None;
    }

    /// Vide tout le cache : les doublons de travail sur le disque, les aperçus de la
    /// photo en mémoire, les vignettes. La photo ouverte depuis son doublon est relue
    /// depuis l'original, sans toucher aux réglages.
    fn clear_cache(&mut self) {
        let mut freed =
            crate::dialogs::duplicates_dir().map_or(0, |d| luma_render::duplicate::clear(&d));
        if let Some(photo) = &self.photo {
            freed += photo.cached_bytes() as u64;
            photo.release_caches();
            if photo.from_duplicate()
                && let Some(path) = self.current_path().map(Path::to_path_buf)
            {
                self.worker.reload_original(path);
            }
        }
        self.thumbs.clear();
        self.evicted.clear();
        for p in &self.folder {
            self.worker.thumbnail(p.clone());
        }
        self.last_request = None;
        self.status = trf(
            "Cache vidé : {size} libérés.",
            &[("size", &format_bytes(freed))],
        );
    }

    fn touch(&mut self) {
        self.xmp_due = Some(0.0);
    }

    /// Écrit le XMP à côté du RAW (format XMP).
    pub fn flush_xmp(&mut self) {
        if self.xmp_due.take().is_none() {
            return;
        }
        if let Some(path) = self.current_path().map(Path::to_path_buf) {
            // Pendant l'ouverture, les réglages affichés ne sont pas encore ceux de cette
            // photo : on garde ceux de son XMP.
            let settings = if self.loading.is_some() || self.photo.is_none() {
                read_sidecar(&path).unwrap_or_default()
            } else {
                self.settings.clone()
            };
            let meta = self.meta.get(&path).cloned().unwrap_or_default();
            if let Err(e) = write_sidecar(&path, &settings, &meta) {
                self.status = trf("XMP non enregistré : {error}", &[("error", &e)]);
            } else {
                // La miniature montre la photo avec ses retouches (relue telle quelle si les
                // réglages n'ont pas changé).
                self.worker.thumbnail(path.clone());
            }
            self.edits.insert(path, presets::summary(&settings));
        }
    }

    /// Les réglages de départ de la photo ouverte (ceux d'un RAW sans photo).
    fn default_settings(&self) -> Settings {
        self.photo
            .as_ref()
            .map_or_else(Settings::default, |p| self.defaults_for(p))
    }

    /// Les réglages de départ d'une photo : ceux de son boîtier (et de son ISO) s'il y a
    /// une règle, sinon ceux de LÜMA.
    fn defaults_for(&self, photo: &Photo) -> Settings {
        let raw = photo.raw();
        if raw.rendered {
            return photo.default_settings();
        }
        self.defaults
            .for_photo(&camera_name(&raw.make, &raw.model), raw.exif.iso)
            .cloned()
            .unwrap_or_else(|| photo.default_settings())
    }

    /// Le fichier des réglages par défaut (les essais en prennent un à eux) ; sinon
    /// celui de LÜMA, à côté des préréglages.
    pub fn use_defaults_file(&mut self, path: PathBuf) {
        self.defaults = luma_project::defaults::Defaults::load(&path);
        self.defaults_path = Some(path);
    }

    fn save_defaults(&mut self) {
        let path = self
            .defaults_path
            .clone()
            .or_else(|| crate::dialogs::presets_dir().map(|d| d.join(DEFAULTS_FILE)));
        if let Some(path) = path {
            if let Err(e) = self.defaults.save(&path) {
                self.status = trf(
                    "Réglages par défaut non enregistrés : {error}",
                    &[("error", &e)],
                );
            }
            self.defaults_path = Some(path);
        }
    }

    /// Fait des réglages de la photo ouverte ceux de son boîtier, à partir de son ISO ou
    /// à tout ISO.
    pub fn set_camera_defaults(&mut self, with_iso: bool) {
        let Some(photo) = &self.photo else { return };
        let raw = photo.raw();
        let camera = camera_name(&raw.make, &raw.model);
        let iso = raw.exif.iso.filter(|_| with_iso);
        self.defaults.set(&camera, iso, &self.settings);
        self.status = match iso {
            Some(iso) => trf(
                "Réglages par défaut du {camera} à partir d'ISO {iso}",
                &[("camera", &camera), ("iso", &iso)],
            ),
            None => trf("Réglages par défaut du {camera}", &[("camera", &camera)]),
        };
        self.save_defaults();
    }

    /// Retire les réglages par défaut du boîtier de la photo ouverte.
    pub fn clear_camera_defaults(&mut self) {
        let Some(photo) = &self.photo else { return };
        let raw = photo.raw();
        let camera = camera_name(&raw.make, &raw.model);
        self.defaults.clear(&camera);
        self.status = trf(
            "Réglages par défaut du {camera} retirés",
            &[("camera", &camera)],
        );
        self.save_defaults();
    }

    /// Le film donné aux photos qui arrivent (dossier surveillé, boîtier relié), par son
    /// nom ; `None` : aucun.
    pub fn set_import_preset(&mut self, name: Option<String>) {
        self.defaults.import_preset = name;
        self.save_defaults();
    }

    /// Les réglages de départ d'une photo arrivée, avec le film d'import s'il y en a un.
    fn import_settings(&self) -> Option<Settings> {
        let name = self.defaults.import_preset.as_deref()?;
        let k = (0..self.film_count()).find(|&k| self.film_key(k) == name)?;
        Some(self.film_applied(k, &Settings::default(), 1.0))
    }

    /// Le menu des réglages par défaut du boîtier de la photo ouverte.
    fn defaults_panel(&mut self, ui: &mut Ui, commits: &mut Vec<String>) {
        let Some(photo) = &self.photo else { return };
        let raw = photo.raw();
        if raw.rendered {
            return;
        }
        let camera = camera_name(&raw.make, &raw.model);
        let iso = raw.exif.iso;
        let has = self.defaults.rules.iter().any(|r| r.camera == camera);
        let mut action = None;
        ui.horizontal_wrapped(|ui| {
            ui.menu_button(tr("Réglages par défaut…"), |ui| {
                if ui
                    .button(trf("Pour le {camera}", &[("camera", &camera)]))
                    .clicked()
                {
                    action = Some(0);
                    ui.close();
                }
                if let Some(iso) = iso
                    && ui
                        .button(trf("Pour le {camera} à partir d'ISO {iso}", &[("camera", &camera), ("iso", &iso)]))
                        .clicked()
                {
                    action = Some(1);
                    ui.close();
                }
                if has && ui.button(tr("Retirer ceux de ce boîtier")).clicked() {
                    action = Some(2);
                    ui.close();
                }
            })
            .response
            .on_hover_text(tr(
                "Les réglages actuels deviennent le point de départ des photos de ce boîtier qui n'en ont pas encore (recadrage et masques exceptés)",
            ));
            if has {
                ui.label(RichText::new(tr("Ce boîtier a ses réglages par défaut")).size(11.0).color(theme::DIM));
            }
        });
        match action {
            Some(0) => self.set_camera_defaults(false),
            Some(1) => self.set_camera_defaults(true),
            Some(2) => self.clear_camera_defaults(),
            _ => {}
        }
        if action.is_some() {
            commits.clear();
        }
    }

    fn request_render(&mut self, viewport_px: f32, pressed: bool) {
        // Pendant l'ouverture, `photo` et `settings` sont encore ceux de la photo d'avant.
        if self.loading.is_some() {
            return;
        }
        let (Some(photo), Some(path)) = (&self.photo, self.current_path()) else {
            return;
        };
        let mut settings = if self.before {
            photo.default_settings()
        } else {
            self.settings.clone()
        };
        // Un réglage bouge sous le doigt (ou la souris) : un aperçu plus petit, qui suit
        // le geste ; la pleine netteté revient dès qu'on lâche.
        let moving = pressed
            && self.last_request.as_ref().is_some_and(|r| {
                r.path == path
                    && (r.settings != settings || r.size == interactive_size(viewport_px))
            });
        // Tailles par paliers de 512 px : le cache de sources sert d'un rendu à l'autre.
        let size = if self.zoom {
            Size::Full
        } else if moving {
            interactive_size(viewport_px)
        } else {
            Size::Fit(((viewport_px / 512.0).ceil().max(1.0) * 512.0) as usize)
        };
        if self.crop_tool.is_some() {
            // Pendant le recadrage, toute la photo est visible (redressée).
            settings.crop = Crop {
                angle: settings.crop.angle,
                ..Crop::default()
            };
        }
        if self.guiding {
            // Pendant le redressement guidé, la photo telle qu'elle est (ni perspective ni
            // recadrage) : on y trace les lignes qui doivent être droites.
            settings.crop = Crop::default();
            settings.transform = luma_project::Transform::default();
        }
        let request = Request {
            path: path.to_path_buf(),
            settings,
            size,
            clipping: self.clipping,
            proof: self
                .proofing
                .active()
                .map(|p| (self.proofing.version, p.simulate, p.gamut)),
            // La copie réduite de la photo : pour chercher les sources des retouches, et
            // pour la zone d'un masque qui se lit sur la photo (ciel, plage, bords).
            scene: self.retouch.is_some()
                || self.selected_spot.is_some()
                || self
                    .selected_mask
                    .and_then(|i| self.settings.masks.get(i))
                    .is_some_and(|m| {
                        let sky = |s: &luma_project::Shape| *s == luma_project::Shape::Sky;
                        m.range.is_some()
                            || m.edges
                            || sky(&m.shape)
                            || m.parts.iter().any(|p| sky(&p.shape))
                    }),
            dust_view: self.show_dust && self.tool == Tool::Masks,
            find_dust: self.dust_pending,
            scope: self.scope,
        };
        if self.last_request.as_ref() == Some(&request) {
            return;
        }
        self.generation += 1;
        self.worker.render(
            photo.clone(),
            request.settings.clone(),
            size,
            self.generation,
            crate::worker::Extras {
                clipping: self.clipping,
                scene: request.scene,
                dust_view: request.dust_view,
                find_dust: request.find_dust,
                scope: request.scope,
            },
            self.proofing.active(),
        );
        self.last_request = Some(request);
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        // Une saisie de texte en cours garde ses touches (sauf les raccourcis avec Cmd).
        let typing = ctx.text_edit_focused();
        // La fenêtre des raccourcis attend une touche : elle la prend, rien ne s'exécute.
        if let Some(id) = self.keys_listening {
            let pressed = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
            });
            if let Some((key, m)) = pressed {
                self.keys_listening = None;
                if key != Key::Escape || m.any() {
                    let combo = crate::keys::Combo {
                        key,
                        command: m.command,
                        shift: m.shift,
                        alt: m.alt,
                    };
                    self.keys_note = self.keymap.set(id, combo).map(|lost| {
                        trf(
                            "« {action} » n'a plus ce raccourci",
                            &[("action", &crate::keys::label(lost))],
                        )
                    });
                    self.save_keys();
                }
                ctx.input_mut(|i| i.events.clear());
            }
            return;
        }
        let playing = self.playing.is_some();
        let keymap = &self.keymap;
        let action = ctx.input_mut(|i| keymap.find(i, typing)).map(|a| match a {
            "space" if playing => "slideshow_pause",
            "space" => "next",
            a => a,
        });
        if let Some(a) = action {
            self.act(a);
        }
    }

    /// Le fichier des raccourcis : « Raccourcis.txt » dans le dossier de LÜMA.
    fn keys_file(&self) -> Option<PathBuf> {
        self.keys_path
            .clone()
            .or_else(|| Some(crate::dialogs::app_dir()?.join("Raccourcis.txt")))
    }

    /// Lit les raccourcis de ce fichier, et les y garde (les essais).
    pub fn use_keys_file(&mut self, path: PathBuf) {
        self.keymap = crate::keys::Keymap::load(&path);
        self.keys_path = Some(path);
    }

    fn save_keys(&mut self) {
        if let Some(f) = self.keys_file()
            && let Err(e) = self.keymap.save(&f)
        {
            self.status = e.to_string();
        }
    }

    /// Change le raccourci d'une commande (comme si on avait appuyé sur ces touches dans
    /// la fenêtre des raccourcis).
    pub fn set_shortcut(
        &mut self,
        id: &'static str,
        combo: crate::keys::Combo,
    ) -> Option<&'static str> {
        let lost = self.keymap.set(id, combo);
        self.save_keys();
        lost
    }

    /// Tous les espaces de travail : ceux de toujours, puis ceux enregistrés.
    pub fn all_workspaces(&self) -> Vec<crate::workspaces::Workspace> {
        let mut all = crate::workspaces::builtin();
        all.extend(self.workspaces.iter().cloned());
        all
    }

    /// Passe à l'espace de travail de rang `k` (de 0).
    pub fn use_workspace(&mut self, k: usize) {
        let Some(w) = self.all_workspaces().into_iter().nth(k) else {
            return;
        };
        self.culling = false;
        self.playing = None;
        self.map = None;
        self.set_tool(w.tool);
        self.hide_panels = !w.panels;
        self.status = trf("Espace de travail : {name}", &[("name", &tr(&w.name))]);
    }

    fn workspaces_file(&self) -> Option<PathBuf> {
        self.workspaces_path
            .clone()
            .or_else(|| Some(crate::dialogs::app_dir()?.join("Espaces de travail.txt")))
    }

    /// Lit les espaces de travail de ce fichier, et les y garde (les essais).
    pub fn use_workspaces_file(&mut self, path: PathBuf) {
        self.workspaces = crate::workspaces::load(&path);
        self.workspaces_path = Some(path);
    }

    /// Enregistre l'outil et les panneaux du moment sous ce nom.
    pub fn save_workspace(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let w = crate::workspaces::Workspace {
            name: name.to_owned(),
            tool: self.tool,
            panels: !self.hide_panels,
        };
        crate::workspaces::put(&mut self.workspaces, w);
        if let Some(f) = self.workspaces_file()
            && let Err(e) = crate::workspaces::save(&f, &self.workspaces)
        {
            self.status = e.to_string();
        }
    }

    /// Retire un espace enregistré.
    pub fn remove_workspace(&mut self, name: &str) {
        self.workspaces.retain(|w| w.name != name);
        if let Some(f) = self.workspaces_file() {
            let _ = crate::workspaces::save(&f, &self.workspaces);
        }
    }

    /// Ouvre la fenêtre des raccourcis clavier.
    pub fn open_shortcuts(&mut self) {
        self.keys_open = true;
    }

    /// Attend la prochaine touche pour cette commande.
    pub fn listen_shortcut(&mut self, id: &'static str) {
        self.keys_listening = Some(id);
    }

    /// La fenêtre « Raccourcis clavier » : chaque commande, ses touches ; « Changer »
    /// attend une touche, une touche déjà prise passe à la nouvelle commande.
    fn keys_window(&mut self, ctx: &egui::Context) {
        let mut open = self.keys_open;
        let mut change = None;
        let mut clear = None;
        let mut reset: Option<Option<&'static str>> = None;
        egui::Window::new(tr("Raccourcis clavier"))
            .id(egui::Id::unique("fenetre-raccourcis"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_height(520.0)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(470.0);
                if let Some(id) = self.keys_listening {
                    ui.label(
                        RichText::new(trf(
                            "Appuyez sur les touches pour « {action} » (Échap : annuler)",
                            &[("action", &crate::keys::label(id))],
                        ))
                        .color(theme::ACCENT),
                    );
                } else if let Some(n) = &self.keys_note {
                    ui.label(RichText::new(n.clone()).color(theme::DIM));
                } else {
                    ui.label(
                        RichText::new(tr(
                            "Cliquez sur une commande, puis appuyez sur ses nouvelles touches.",
                        ))
                        .color(theme::DIM),
                    );
                }
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .max_height(440.0)
                    .show(ui, |ui| {
                        egui::Grid::new("raccourcis")
                            .num_columns(3)
                            .striped(true)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                for a in crate::keys::ACTIONS {
                                    ui.label(tr(a.label));
                                    let combos: Vec<String> = self
                                        .keymap
                                        .get(a.id)
                                        .iter()
                                        .map(crate::keys::Combo::label)
                                        .collect();
                                    let text = if combos.is_empty() {
                                        tr("Aucun").to_owned()
                                    } else {
                                        combos.join(&format!(" {} ", tr("ou")))
                                    };
                                    let listening = self.keys_listening == Some(a.id);
                                    if ui
                                        .selectable_label(
                                            listening,
                                            RichText::new(text).monospace(),
                                        )
                                        .on_hover_text(tr("Changer le raccourci"))
                                        .clicked()
                                    {
                                        change = Some(a.id);
                                    }
                                    ui.horizontal(|ui| {
                                        if !self.keymap.get(a.id).is_empty()
                                            && ui.small_button(tr("Retirer")).clicked()
                                        {
                                            clear = Some(a.id);
                                        }
                                        if self.keymap.get(a.id) != a.defaults
                                            && ui.small_button(tr("Par défaut")).clicked()
                                        {
                                            reset = Some(Some(a.id));
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                    });
                ui.add_space(6.0);
                if ui.button(tr("Tout remettre par défaut")).clicked() {
                    reset = Some(None);
                }
            });
        if let Some(id) = change {
            self.keys_note = None;
            self.keys_listening = Some(id);
        }
        if let Some(id) = clear {
            self.keymap.clear(id);
            self.save_keys();
        }
        if let Some(r) = reset {
            self.keymap.reset(r);
            self.keys_note = None;
            self.save_keys();
        }
        if !open {
            self.keys_listening = None;
        }
        self.keys_open = open;
    }

    fn act(&mut self, action: &str) {
        let culled = self.culling
            && self.auto_advance
            && (matches!(action, "pick" | "reject" | "unflag")
                || action.starts_with("rate")
                || action.starts_with("label"));
        self.act_once(action);
        if culled {
            self.act_once("next");
        }
    }

    fn act_once(&mut self, action: &str) {
        match action {
            "undo" if self.history_pos > 0 => self.restore(self.history_pos - 1),
            "redo" if self.history_pos + 1 < self.history.len() => {
                self.restore(self.history_pos + 1)
            }
            "copy" => {
                self.clipboard = Some(self.settings.clone());
                self.status = tr("Réglages copiés.").into();
            }
            "paste" => {
                if let Some(s) = self.clipboard.clone() {
                    self.settings = s;
                    self.commit(tr("Coller les réglages").into());
                    self.touch();
                }
            }
            "sync" => {
                if self.sync_targets().is_empty() {
                    self.status = tr("Choisissez d'abord les photos à synchroniser.").into();
                } else {
                    self.syncing = true;
                    self.paste_open = true;
                }
            }
            "paste_some" => {
                if self.clipboard.is_some() && self.photo.is_some() {
                    self.syncing = false;
                    self.paste_open = true;
                } else {
                    self.status = tr("Copiez d'abord les réglages d'une photo.").into();
                }
            }
            "reset" => {
                self.settings = self.default_settings();
                self.commit(tr("Réinitialiser").into());
                self.touch();
            }
            "export" => {
                if self.current.is_some() {
                    self.export_all = self.module == Module::Library;
                    self.export_presets = luma_render::export::presets_path()
                        .map(|p| luma_render::export::Presets::load(&p))
                        .unwrap_or_default();
                    self.export_open = true;
                }
            }
            "export_now" => self.export_now(),
            "library" => self.set_tool(Tool::Library),
            "develop" => {
                if self.tool == Tool::Library {
                    self.set_tool(Tool::Light);
                }
            }
            "pick" => self.set_meta(|m| m.flag = Flag::Pick),
            "reject" => self.set_meta(|m| m.flag = Flag::Reject),
            "unflag" => self.set_meta(|m| m.flag = Flag::None),
            r if r.starts_with("rate") => {
                let n: u8 = r[4..].parse().unwrap_or(0);
                self.set_meta(|m| m.rating = n);
                self.status = if n == 0 {
                    tr("Note retirée").into()
                } else {
                    trf("Note : {n} sur 5", &[("n", &n)])
                };
            }
            "crop" => {
                if self.crop_tool.is_some() {
                    self.act("crop_done");
                } else if self.photo.is_some() {
                    self.set_tool(Tool::Geometry);
                    self.zoom = false;
                    self.crop_tool = Some(CropTool {
                        saved: self.settings.crop,
                        aspect: 0,
                    });
                }
            }
            "crop_done" => {
                if self.crop_tool.take().is_some() {
                    self.commit(tr("Recadrage").into());
                    self.touch();
                }
            }
            "crop_cancel" if self.culling => {
                self.culling = false;
                self.playing = None;
            }
            "panels" => self.hide_panels = !self.hide_panels,
            a if a.starts_with("workspace") => {
                if let Some(k) = a["workspace".len()..]
                    .parse::<usize>()
                    .ok()
                    .and_then(|k| k.checked_sub(1))
                {
                    self.use_workspace(k);
                }
            }
            "slideshow_pause" => {
                if let Some(p) = &mut self.playing {
                    p.paused = !p.paused;
                    p.since = std::time::Instant::now();
                }
            }
            "crop_cancel" if self.compare.is_some() => self.compare = None,
            "compare" => {
                if self.compare.is_some() {
                    self.compare = None;
                } else {
                    self.start_compare();
                }
            }
            "stack" => {
                if let Some(p) = self.current_path().map(Path::to_path_buf) {
                    self.toggle_stack(&p);
                }
            }
            "loupe" => {
                if let Some(c) = &mut self.compare {
                    c.loupe = !c.loupe;
                }
            }
            "culling" => {
                self.culling = !self.culling && !self.folder.is_empty();
                if self.culling && self.current.is_none() {
                    self.select(0);
                }
            }
            "auto_advance" => {
                self.auto_advance = !self.auto_advance;
                self.status = if self.auto_advance {
                    tr("Avance automatique").into()
                } else {
                    tr("Avance automatique coupée").into()
                };
            }
            l if l.starts_with("label") => {
                let n: usize = l[5..].parse().unwrap_or(0);
                // La même touche retire le libellé.
                let same = luma_project::xmp::label_color(&self.current_meta().label) == Some(n);
                let name = luma_project::xmp::LABELS[n.min(4)];
                self.set_meta(|m| m.label = if same { String::new() } else { name.to_owned() });
                self.status = if same {
                    tr("Libellé retiré").into()
                } else {
                    trf("Libellé : {name}", &[("name", &tr(LABEL_NAMES[n.min(4)]))])
                };
            }
            "crop_cancel" => {
                if let Some(t) = self.crop_tool.take() {
                    self.settings.crop = t.saved;
                } else if self.export_open {
                    self.export_open = false;
                } else if self.paste_open {
                    self.paste_open = false;
                } else if self.liquify.is_some() {
                    self.liquify = None;
                } else if self.retouch.is_some() || self.selected_spot.is_some() || self.show_dust {
                    self.retouch = None;
                    self.selected_spot = None;
                    self.show_dust = false;
                } else {
                    self.selected_mask = None;
                }
            }
            "retouch" | "retouch_heal" | "retouch_clone" | "retouch_red_eye" | "retouch_remove"
                if self.photo.is_some() =>
            {
                use luma_project::SpotKind;
                let kind = match action {
                    "retouch_clone" => SpotKind::Clone,
                    "retouch_red_eye" => SpotKind::RedEye,
                    "retouch_remove" => SpotKind::Remove,
                    "retouch_heal" => SpotKind::Heal,
                    _ => self.retouch.unwrap_or(SpotKind::Heal),
                };
                // Q ou le même bouton une seconde fois : l'outil se range.
                self.retouch = if self.retouch == Some(kind) {
                    None
                } else {
                    Some(kind)
                };
                if self.retouch.is_some() {
                    self.set_tool(Tool::Masks);
                    self.zoom = false;
                    self.selected_mask = None;
                    self.crop_tool = None;
                    self.liquify = None;
                }
            }
            "liquify_push" | "liquify_bloat" | "liquify_pucker" if self.photo.is_some() => {
                use luma_project::WarpKind;
                let kind = match action {
                    "liquify_bloat" => WarpKind::Bloat,
                    "liquify_pucker" => WarpKind::Pucker,
                    _ => WarpKind::Push,
                };
                // Le même bouton une seconde fois : le pinceau se range.
                self.liquify = if self.liquify == Some(kind) {
                    None
                } else {
                    Some(kind)
                };
                if self.liquify.is_some() {
                    self.set_tool(Tool::Masks);
                    self.zoom = false;
                    self.selected_mask = None;
                    self.selected_spot = None;
                    self.retouch = None;
                    self.crop_tool = None;
                }
            }
            "liquify_clear" if !self.settings.liquify.is_empty() => {
                self.settings.liquify.clear();
                self.commit(tr("Fluidité effacée").into());
                self.touch();
            }
            "mask_linear" | "mask_radial" | "mask_brush" | "mask_full" | "mask_sky"
                if self.photo.is_some() =>
            {
                let mask = match action {
                    "mask_linear" => Mask::linear(),
                    "mask_radial" => Mask::radial(),
                    "mask_full" => Mask::full(),
                    "mask_sky" => Mask::sky(),
                    _ => Mask::brush(),
                };
                let label = trf(
                    "Nouveau masque : {mask}",
                    &[("mask", &tr(&mask.name).to_lowercase())],
                );
                self.set_tool(Tool::Masks);
                self.zoom = false;
                self.settings.masks.push(mask);
                self.selected_mask = Some(self.settings.masks.len() - 1);
                self.retouch = None;
                self.selected_spot = None;
                self.liquify = None;
                self.commit(label);
                self.touch();
            }
            // Pendant le recadrage, O change de guide de composition.
            "mask_overlay" if self.crop_tool.is_some() => self.crop_guide = self.crop_guide.next(),
            "guide_turn" => self.crop_turn = (self.crop_turn + 1) % 4,
            "auto_level" => self.auto_level(),
            "auto_vertical" => self.auto_vertical(),
            "mask_overlay" => self.show_mask = !self.show_mask,
            "eyedropper" => self.eyedropper = !self.eyedropper && self.photo.is_some(),
            "targeting" => {
                self.targeting = !self.targeting && self.photo.is_some();
                if self.targeting {
                    // L'outil règle le mélangeur : son panneau s'ouvre.
                    self.tool = Tool::Color;
                    self.eyedropper = false;
                }
            }
            "mask_delete" if self.selected_mask.is_none() && self.selected_spot.is_some() => {
                if let Some(i) = self.selected_spot.take()
                    && i < self.settings.spots.len()
                {
                    self.settings.spots.remove(i);
                    self.commit(tr("Retouche supprimée").into());
                    self.touch();
                }
            }
            "mask_delete" => {
                if let Some(i) = self.selected_mask.take()
                    && i < self.settings.masks.len()
                {
                    self.settings.masks.remove(i);
                    self.selected_part = None;
                    self.commit(tr("Masque supprimé").into());
                    self.touch();
                }
            }
            // Une copie du masque choisi, juste après lui ; inversée, elle prend tout le
            // reste de la photo (le sol après le ciel), ses réglages remis à zéro.
            "mask_duplicate" | "mask_duplicate_inverted" => {
                if let Some(i) = self
                    .selected_mask
                    .filter(|&i| i < self.settings.masks.len())
                {
                    let mut copy = self.settings.masks[i].clone();
                    let inverted = action == "mask_duplicate_inverted";
                    if inverted {
                        copy.invert = !copy.invert;
                        copy.local = luma_project::Local::default();
                        copy.look = None;
                    }
                    self.settings.masks.insert(i + 1, copy);
                    self.selected_mask = Some(i + 1);
                    self.selected_part = None;
                    self.commit(
                        if inverted {
                            tr("Masque dupliqué et inversé")
                        } else {
                            tr("Masque dupliqué")
                        }
                        .into(),
                    );
                    self.touch();
                }
            }
            "open" => {
                // Sur iPhone et iPad : le sélecteur de Photos ; les photos choisies sont
                // copiées dans le dossier de LÜMA, relu dès qu'elles arrivent.
                if self.photos_button {
                    self.import_asks += 1;
                }
                if luma_apple::photos::AVAILABLE {
                    let opened = crate::dialogs::documents()
                        .ok_or("dossier Documents introuvable")
                        .and_then(luma_apple::photos::pick);
                    crate::journal(&format!("importer : {opened:?}"));
                    self.import_result = Some(match &opened {
                        Ok(from) => format!("Photos demandé depuis {from}"),
                        Err(why) => format!("Photos refusé : {why}"),
                    });
                    // Le choix dans Photos prend son temps, et la copie des fichiers aussi.
                    self.imports_until = f64::INFINITY;
                    if let Err(why) = opened {
                        self.status = trf("Photos ne s'ouvre pas ({why}).", &[("why", &why)]);
                    }
                } else if !self.photos_button
                    && let Some(p) = crate::dialogs::pick_raw()
                {
                    self.open_path(&p);
                }
            }
            "open_files" => {
                // Sur iPhone et iPad : l'app Fichiers (carte mémoire branchée, iCloud
                // Drive) ; les photos arrivent dans le dossier de LÜMA, comme celles de
                // Photos.
                if self.photos_button {
                    self.files_asks += 1;
                }
                if luma_apple::photos::AVAILABLE {
                    let opened = crate::dialogs::documents()
                        .ok_or("dossier Documents introuvable")
                        .and_then(luma_apple::photos::pick_files);
                    crate::journal(&format!("importer depuis Fichiers : {opened:?}"));
                    self.imports_until = f64::INFINITY;
                    if let Err(why) = opened {
                        self.status = trf("Fichiers ne s'ouvre pas ({why}).", &[("why", &why)]);
                    }
                }
            }
            "before" => self.before = !self.before,
            "prev" | "next" => {
                let visible = self.visible();
                let next = match self
                    .current
                    .and_then(|c| visible.iter().position(|v| *v == c))
                {
                    Some(k) if action == "prev" => k.checked_sub(1).map(|k| visible[k]),
                    Some(k) => visible.get(k + 1).copied(),
                    None => visible.first().copied(),
                };
                if let Some(i) = next {
                    self.select(i);
                }
            }
            "clipping" => self.clipping = !self.clipping,
            "scope" => {
                self.scope = self.scope.next();
                self.scope_texture = None;
            }
            "zoom" => self.zoom = !self.zoom,
            _ => {}
        }
    }

    /// Lance l'export avec les options de la fenêtre : la photo affichée, ou toutes
    /// celles que le filtre laisse voir.
    fn export_now(&mut self) {
        self.export_open = false;
        self.flush_xmp();
        let current = self.current_path().map(Path::to_path_buf);
        let paths: Vec<PathBuf> = if self.export_all {
            self.visible()
                .into_iter()
                .map(|i| self.folder[i].clone())
                .collect()
        } else {
            current.clone().into_iter().collect()
        };
        let jobs: Vec<ExportJob> = paths
            .into_iter()
            .map(|source| {
                let is_current = Some(&source) == current.as_ref() && self.loading.is_none();
                ExportJob {
                    photo: if is_current { self.photo.clone() } else { None },
                    settings: if is_current {
                        self.settings.clone()
                    } else {
                        read_sidecar(&source).unwrap_or_default()
                    },
                    source,
                }
            })
            .collect();
        if jobs.is_empty() {
            return;
        }
        self.status = if jobs.len() > 1 {
            trf("Export de {n} photos…", &[("n", &jobs.len())])
        } else {
            tr("Export d'une photo…").into()
        };
        let mut targets = Vec::new();
        if self.export_current {
            targets.push(self.export_options.clone());
        }
        for name in &self.export_targets {
            if let Some(p) = self.export_presets.get(name) {
                targets.push(p.options);
            }
        }
        if targets.is_empty() {
            return;
        }
        self.worker.export(jobs, targets);
    }

    /// En tête de la fenêtre Exporter : vers quoi exporter (les réglages de la fenêtre,
    /// des préréglages cochés, plusieurs à la fois), reprendre un préréglage, enregistrer
    /// les réglages de la fenêtre comme préréglage.
    fn presets_editor(&mut self, ui: &mut Ui) {
        ui.label(RichText::new(tr("Exporter vers")).strong());
        ui.checkbox(&mut self.export_current, tr("Les réglages ci-dessous"));
        let mut load = None;
        // Sur iPhone : le préréglage et « Reprendre » sur une ligne, son résumé dessous.
        if self.compact {
            for p in self.export_presets.all() {
                let mut on = self.export_targets.contains(&p.name);
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut on, tr(&p.name)).changed() {
                        self.export_targets.retain(|t| *t != p.name);
                        if on {
                            self.export_targets.push(p.name.clone());
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(tr("Reprendre")).clicked() {
                            load = Some(p.options.clone());
                        }
                    });
                });
                ui.label(
                    RichText::new(export_summary(&p.options))
                        .small()
                        .color(theme::DIM),
                );
                ui.add_space(4.0);
            }
        }
        let grid = !self.compact;
        egui::Grid::new("export-presets")
            .num_columns(3)
            .spacing(vec2(10.0, 4.0))
            .show(ui, |ui| {
                for p in self.export_presets.all().into_iter().filter(|_| grid) {
                    let mut on = self.export_targets.contains(&p.name);
                    if ui.checkbox(&mut on, tr(&p.name)).changed() {
                        self.export_targets.retain(|t| *t != p.name);
                        if on {
                            self.export_targets.push(p.name.clone());
                        }
                    }
                    ui.label(
                        RichText::new(export_summary(&p.options))
                            .small()
                            .color(theme::DIM),
                    );
                    if ui.small_button(tr("Reprendre")).clicked() {
                        load = Some(p.options);
                    }
                    ui.end_row();
                }
            });
        if let Some(o) = load {
            self.export_options = o;
            self.export_current = true;
        }
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.preset_name)
                    .hint_text(tr("Nom du préréglage"))
                    .desired_width(180.0),
            )
            .accessible_name(tr("Nom du préréglage"));
            let name = self.preset_name.trim().to_owned();
            if ui
                .add_enabled(
                    !name.is_empty(),
                    egui::Button::new(tr("Enregistrer ces réglages")),
                )
                .clicked()
            {
                self.export_presets.set(luma_render::export::Preset {
                    name: name.clone(),
                    options: self.export_options.clone(),
                });
                let saved = luma_render::export::presets_path()
                    .ok_or_else(|| "HOME".to_owned())
                    .and_then(|p| self.export_presets.save(&p));
                self.status = match saved {
                    Ok(()) => trf(
                        "Préréglage d'export enregistré : {name}",
                        &[("name", &name)],
                    ),
                    Err(e) => trf("Préréglage non enregistré : {error}", &[("error", &e)]),
                };
                self.preset_name.clear();
            }
        });
    }

    /// La fenêtre Exporter.
    /// « Coller une partie des réglages » : les groupes à coller, cochés (le recadrage
    /// et les masques, propres à chaque photo, ne le sont pas d'abord).
    fn paste_window(&mut self, ctx: &egui::Context) {
        use luma_project::paste::{GROUPS, Group, paste};
        let mut open = self.paste_open;
        let mut go = false;
        let syncing = self.syncing;
        let n_targets = self.sync_targets().len();
        let title = if syncing {
            trf("Synchroniser {n} photos", &[("n", &n_targets)])
        } else {
            tr("Coller une partie des réglages").to_owned()
        };
        egui::Window::new(title)
            .id(egui::Id::unique("fenetre-coller"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .vscroll(true)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                // Le bouton en haut : toujours visible, même quand la liste défile.
                ui.horizontal(|ui| {
                    let label = if syncing {
                        trf("Synchroniser ({n})", &[("n", &self.paste_groups.len())])
                    } else {
                        trf("Coller ({n})", &[("n", &self.paste_groups.len())])
                    };
                    go = ui
                        .add_enabled(!self.paste_groups.is_empty(), egui::Button::new(label))
                        .clicked();
                    if ui.button(tr("Tout")).clicked() {
                        self.paste_groups = GROUPS.iter().map(|g| g.0).collect();
                    }
                    if ui.button(tr("Rien")).clicked() {
                        self.paste_groups.clear();
                    }
                });
                ui.add_space(6.0);
                for (group, name) in GROUPS {
                    let mut on = self.paste_groups.contains(&group);
                    if ui.checkbox(&mut on, tr(name)).changed() {
                        if on {
                            self.paste_groups.push(group);
                        } else {
                            self.paste_groups.retain(|g| *g != group);
                        }
                    }
                }
            });
        if go && syncing {
            let groups: Vec<Group> = self.paste_groups.clone();
            self.sync_selected(&groups);
        } else if go && let Some(source) = self.clipboard.clone() {
            let groups: Vec<Group> = self.paste_groups.clone();
            self.settings = paste(&self.settings, &source, &groups);
            self.commit(trf(
                "Coller une partie des réglages ({n})",
                &[("n", &groups.len())],
            ));
            self.touch();
        }
        self.paste_open = open && !go;
    }

    fn export_window(&mut self, ctx: &egui::Context) {
        let mut open = self.export_open;
        let mut go = false;
        let count = self.visible().len();
        let compact = self.compact;
        let window = egui::Window::new(tr("Exporter"))
            .id(egui::Id::unique("fenetre-export"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            // Plus haute qu'un écran d'iPhone : elle défile.
            .vscroll(true);
        place_window(ctx, window, compact).show(ctx, |ui| {
                self.presets_editor(ui);
                ui.add_space(6.0);
                let o = &mut self.export_options;
                // Sur iPhone, chaque réglage sous son titre ; ailleurs, deux colonnes.
                let mut body = |ui: &mut Ui| {
                        form_row(ui, compact, tr("Photos"));
                        ui.horizontal_wrapped(|ui| {
                            ui.radio_value(&mut self.export_all, false, tr("Photo affichée"));
                            ui.radio_value(
                                &mut self.export_all,
                                true,
                                trf("Les {n} du filtre", &[("n", &count)]),
                            );
                        });
                        form_end(ui, compact);
                        form_row(ui, compact, tr("Format"));
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut o.format, Format::Jpeg, "JPEG");
                            ui.radio_value(&mut o.format, Format::Png, "PNG");
                            ui.radio_value(&mut o.format, Format::Tiff, "TIFF");
                        });
                        form_end(ui, compact);
                        if o.format == Format::Jpeg {
                            form_row(ui, compact, tr("Qualité"));
                            if compact {
                                ui.spacing_mut().slider_width = ui.available_width() - 90.0;
                            }
                            ui.add(egui::Slider::new(&mut o.quality, 1..=100));
                            form_end(ui, compact);
                            form_row(ui, compact, "");
                            ui.checkbox(&mut o.web, tr("Allégé pour le web"))
                                .on_hover_text(tr(
                                    "Couleur à demi-résolution (4:2:0) et codage calculé pour chaque image : de 9 à 29 % plus léger, que tout navigateur lit",
                                ));
                            form_end(ui, compact);
                            form_row(ui, compact, tr("Poids maximal"));
                            ui.horizontal(|ui| max_weight_editor(ui, &mut o.max_kb));
                        } else {
                            form_row(ui, compact, tr("Profondeur"));
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut o.bits, 8, "8 bits");
                                ui.radio_value(&mut o.bits, 16, "16 bits");
                            });
                        }
                        form_end(ui, compact);
                        form_row(ui, compact, tr("Espace colorimétrique"));
                        let spaces = [
                            (OutputSpace::Srgb, "sRGB"),
                            (OutputSpace::DisplayP3, "Display P3"),
                            (OutputSpace::Rec2020, "Rec. 2020"),
                            (OutputSpace::ProPhoto, "ProPhoto RGB"),
                        ];
                        let current = spaces
                            .iter()
                            .find(|s| s.0 == o.space)
                            .map_or("sRGB", |s| s.1);
                        egui::ComboBox::from_id_salt("espace")
                            .selected_text(current)
                            .show_ui(ui, |ui| {
                                for (sp, name) in spaces {
                                    ui.selectable_value(&mut o.space, sp, name);
                                }
                            })
                            .response
                            .accessible_name(tr("Espace colorimétrique"));
                        form_end(ui, compact);
                        form_row(ui, compact, tr("Dimensions"));
                        ui.horizontal_wrapped(|ui| {
                            resize_editor(ui, &mut o.resize);
                            // Au‑delà de la pleine définition : pour les grands tirages.
                            if o.resize.is_some() {
                                ui.checkbox(&mut o.enlarge, tr("Agrandir au besoin"))
                                    .on_hover_text(tr(
                                        "Une taille plus grande que la photo l'agrandit (jusqu'à quatre fois chaque côté), net et sans halo, pour les grands tirages",
                                    ));
                            }
                        });
                        form_end(ui, compact);
                        form_row(ui, compact, tr("Accentuation de sortie"));
                        ui.horizontal_wrapped(|ui| sharpen_editor(ui, &mut o.sharpen));
                        form_end(ui, compact);
                        form_row(ui, compact, tr("Filigrane"));
                        ui.vertical(|ui| watermark_editor(ui, &mut o.watermark));
                        form_end(ui, compact);
                        form_row(ui, compact, tr("Métadonnées"));
                        ui.vertical(|ui| metadata_editor(ui, &mut o.metadata));
                        form_end(ui, compact);
                        if crate::dialogs::FOLDER_PICKER || o.logo.is_some() {
                            form_row(ui, compact, tr("Logo"));
                            ui.vertical(|ui| logo_editor(ui, &mut o.logo));
                            form_end(ui, compact);
                        }
                        form_row(ui, compact, tr("Nom des fichiers"));
                        ui.vertical(|ui| {
                            let mut text = o.name.clone().unwrap_or_default();
                            let edit = ui
                                .add(
                                    egui::TextEdit::singleline(&mut text)
                                        .hint_text("{nom}")
                                        .desired_width(ui.available_width().min(260.0)),
                                )
                                .accessible_name(tr("Nom des fichiers"));
                            if edit.changed() {
                                o.name = Some(text.clone()).filter(|t| !t.trim().is_empty());
                            }
                            // Les repères du modèle, ou ce qui ne va pas.
                            let note = if valid_name(&text) {
                                RichText::new(tr(
                                    "Repères : {nom} {date} {heure} {séquence} {boîtier} {objectif} {iso} {note} {libellé} ; « / » crée un sous-dossier",
                                ))
                                .color(theme::DIM)
                            } else {
                                RichText::new(tr("Repère inconnu ou accolade non fermée")).color(theme::ACCENT)
                            };
                            ui.label(note.small());
                        });
                        form_end(ui, compact);
                        form_row(ui, compact, tr("Dossier"));
                        ui.horizontal_wrapped(|ui| {
                            let text = match &o.folder {
                                Some(d) => d.display().to_string(),
                                // Sur iPhone et iPad, chaque image va aussi dans Photos.
                                None if luma_apple::photos::AVAILABLE => {
                                    tr("La Pellicule de Photos, et LÜMA/ à côté de l'original (app Fichiers)").into()
                                }
                                None => tr("LÜMA/, à côté de l'original").into(),
                            };
                            ui.label(RichText::new(text).color(theme::DIM));
                            if crate::dialogs::FOLDER_PICKER
                                && ui.button(tr("Choisir…")).clicked()
                                && let Some(d) = crate::dialogs::pick_folder(tr("Dossier d'export"))
                            {
                                o.folder = Some(d);
                            }
                            if o.folder.is_some() && ui.button(tr("À côté de l'original")).clicked()
                            {
                                o.folder = None;
                            }
                        });
                        form_end(ui, compact);
                };
                if compact {
                    body(ui);
                } else {
                    egui::Grid::new("export")
                        .num_columns(2)
                        .spacing(vec2(12.0, 8.0))
                        .show(ui, body);
                }
                ui.add_space(8.0);
                let valid = valid_name(self.export_options.name.as_deref().unwrap_or(""))
                    && (self.export_current || !self.export_targets.is_empty());
                ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                    if ui
                        .add_enabled(valid, egui::Button::new(RichText::new(tr("Exporter")).strong()))
                        .clicked()
                    {
                        go = true;
                    }
                });
            });
        self.export_open = open && !go;
        if go {
            self.export_now();
        }
    }

    /// Toute l'interface.
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.handle_events(&ctx);
        self.watch_memory(&ctx);
        self.watch_imports(&ctx);
        if crate::touch() {
            self.track_touches(&ctx);
        }
        // Les vignettes libérées qui se sont affichées vides : on les relit.
        for p in self.shown_missing.take() {
            if self.evicted.remove(&p) {
                self.worker.thumbnail(p);
            }
        }
        self.poll_catalog();
        if self.watch.is_some() {
            let now = ctx.input(|i| i.time);
            if now >= self.watch_next {
                self.watch_next = now + 1.0;
                self.poll_watch();
            }
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
        // Corrections du boîtier coupées sur une photo ouverte depuis son doublon, qui les
        // porte dans ses pixels : la relire depuis le RAW.
        if let (Some(photo), Some(path)) = (&self.photo, self.current_path())
            && photo.needs_original(self.settings.lens.camera)
            && self.original_asked.as_deref() != Some(path)
        {
            let path = path.to_path_buf();
            self.worker.reload_original(path.clone());
            self.original_asked = Some(path);
        }
        if self.licence.poll() {
            // Une clé activée ferme l'écran Premium.
            if matches!(self.licence.access(), luma_licence::Access::Premium { .. })
                && !self.licence.message.is_empty()
            {
                self.status = self.licence.message.clone();
            }
        }
        // Un achat dans l'App Store répond par le fil principal : on regarde. La
        // vérification de la licence, elle, réveille l'écran quand elle a fini.
        if self.licence.store_busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }
        let allowed = self.licence.access().is_allowed();
        if allowed {
            self.shortcuts(&ctx);
        }
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if let Some(p) = dropped.first() {
            self.open_path(p);
        }
        // Le XMP s'écrit une seconde après le dernier changement.
        let now = ctx.input(|i| i.time);
        match self.xmp_due {
            Some(0.0) => self.xmp_due = Some(now + 1.0),
            Some(t) if now >= t => self.flush_xmp(),
            Some(t) => ctx.request_repaint_after(std::time::Duration::from_secs_f64(t - now)),
            None => {}
        }

        let screen = ui.max_rect();
        ui.painter().rect_filled(screen, 0.0, theme::BACKGROUND);
        let brand = self.brand.get_or_insert_with(|| Brand::new(ui.ctx()));
        ui.painter().image(
            brand.backdrop.id(),
            screen,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        // L'interface évite l'encoche, la Dynamic Island et la barre d'accueil : le fond
        // va jusqu'aux bords, le reste dans la zone sûre.
        let full = screen.intersect(ctx.content_rect());
        // iPhone (ou fenêtre étroite) : la photo en haut, la barre d'outils, puis
        // l'inspecteur en feuille.
        // iPhone en portrait, et aussi en paysage (peu de hauteur) : la mise en page
        // compacte. Sur iPad, celle de l'ordinateur, mais la barre du haut compacte
        // (ses commandes dans « ••• ») : au doigt, pas de rangée de petits mots.
        let compact = full.width() < 820.0 || (self.touch_layout && full.height() < 600.0);
        self.compact = compact;
        let compact_top = compact || self.touch_layout;
        let m = if compact { 8.0 } else { 14.0 };
        // La barre du haut contient ses boutons : au doigt, ils ont 40 points de haut, et
        // l'îlot 12 points de marge tout autour.
        let top_h = (ui.spacing().interact_size.y + 24.0).max(50.0);
        let top = Rect::from_min_size(full.min + vec2(m, m), vec2(full.width() - 2.0 * m, top_h));
        // Au tri en plein écran, la photo prend tout, la barre du haut aussi.
        let culling = self.culling && self.current.is_some();
        if !culling {
            self.island(ui, top, |s, ui| s.top_island(ui, compact_top));
        }
        // Rien à retoucher encore : l'accueil prend tout l'écran.
        let welcome = compact && self.folder.is_empty() && self.texture.is_none();
        if culling {
            self.culling_view(ui, full, m);
        } else if self.compare.is_some() && !compact {
            self.compare_view(ui, full, top, m);
        } else if welcome {
            let stage = Rect::from_min_max(pos2(full.min.x, top.max.y + m), full.max);
            self.stage(ui, stage);
        } else if compact && full.width() > full.height() {
            // iPhone en paysage : la photo à gauche, les outils et leur panneau à droite.
            let split = full.min.x + full.width() * 0.56;
            let stage =
                Rect::from_min_max(pos2(full.min.x, top.max.y + m), pos2(split, full.max.y));
            let bar = Rect::from_min_size(
                pos2(split + m, top.max.y + m),
                vec2(full.max.x - split - 2.0 * m, 70.0),
            );
            let sheet = Rect::from_min_max(
                pos2(split + m, bar.max.y + m),
                pos2(full.max.x - m, full.max.y - m),
            );
            self.stage(ui, stage);
            self.island(ui, bar, |s, ui| s.rail(ui, true));
            self.island(ui, sheet, |s, ui| s.inspector(ui));
        } else if compact {
            // La bibliothèque donne plus de place à la grille qu'à la photo seule.
            let share = if self.module == Module::Library {
                0.56
            } else {
                0.38
            };
            let stage = Rect::from_min_max(
                pos2(full.min.x, top.max.y + m),
                pos2(full.max.x, top.max.y + m + full.height() * share),
            );
            let bar = Rect::from_min_size(
                pos2(full.min.x + m, stage.max.y + m),
                vec2(full.width() - 2.0 * m, 70.0),
            );
            let sheet = Rect::from_min_max(
                pos2(full.min.x + m, bar.max.y + m),
                pos2(full.max.x - m, full.max.y - m),
            );
            self.stage(ui, stage);
            static BAR: std::sync::Once = std::sync::Once::new();
            BAR.call_once(|| {
                crate::journal(&format!(
                    "barre des outils : {:.0} {:.0} {:.0} (gauche, milieu, largeur en points)",
                    bar.min.x + 12.0,
                    bar.center().y,
                    bar.width() - 24.0
                ))
            });
            self.island(ui, bar, |s, ui| s.rail(ui, true));
            self.island(ui, sheet, |s, ui| s.inspector(ui));
        } else if self.hide_panels {
            // Tab : la photo seule, sous la barre du haut.
            let stage =
                Rect::from_min_max(pos2(full.min.x + m, top.max.y + m), full.max - vec2(m, m));
            self.stage(ui, stage);
        } else {
            let rail_h = Tool::ALL.len() as f32 * 60.0 + 28.0;
            let rail = Rect::from_min_size(pos2(full.min.x + m, top.max.y + m), vec2(68.0, rail_h));
            let inspector = Rect::from_min_max(
                pos2(full.max.x - m - 366.0, top.max.y + m),
                pos2(full.max.x - m, full.max.y - m),
            );
            let film = Rect::from_min_max(
                pos2(rail.max.x + m, full.max.y - m - 108.0),
                pos2(inspector.min.x - m, full.max.y - m),
            );
            let stage = Rect::from_min_max(
                pos2(rail.max.x + m, top.max.y + m),
                pos2(inspector.min.x - m, film.min.y - m),
            );
            self.stage(ui, stage);
            self.island(ui, rail, |s, ui| s.rail(ui, false));
            self.island(ui, inspector, |s, ui| s.inspector(ui));
            self.island(ui, film, |s, ui| s.filmstrip(ui));
        }
        if self.export_open && allowed {
            self.export_window(&ctx);
        }
        if self.paste_open && allowed {
            self.paste_window(&ctx);
        }
        if self.rename_open && allowed {
            self.rename_window(&ctx);
        }
        self.poll_printing(&ctx);
        self.tick_slideshow(&ctx);
        if self.print_open && allowed {
            self.print_window(&ctx);
        }
        if self.outputs_open && allowed {
            self.outputs_window(&ctx);
        }
        if self.book_pages_open && allowed {
            self.book_pages_window(&ctx);
        }
        if self.import_open && allowed {
            self.import_window(&ctx);
        }
        if self.keys_open {
            self.keys_window(&ctx);
        }
        if !allowed || self.premium_open {
            self.premium_screen(&ctx, full, !allowed);
        }
    }

    /// Compare les photos choisies (2 à 4) ; sans choix, la photo ouverte et la suivante.
    pub fn start_compare(&mut self) {
        let visible = self.visible();
        let mut picked: Vec<usize> = self
            .selected
            .iter()
            .copied()
            .filter(|i| visible.contains(i))
            .collect();
        if picked.len() < 2 {
            picked.clear();
            let at = self
                .current
                .and_then(|c| visible.iter().position(|&v| v == c))
                .unwrap_or(0);
            picked.extend(visible.iter().skip(at).take(2));
            if picked.len() < 2 && at > 0 {
                picked.insert(0, visible[at - 1]);
            }
        }
        picked.truncate(4);
        if picked.len() < 2 {
            self.status = tr("Choisissez au moins deux photos à comparer").into();
            return;
        }
        let paths: Vec<PathBuf> = picked.iter().map(|&i| self.folder[i].clone()).collect();
        self.compare_tag += 1;
        // Une case fait au plus la moitié de l'écran : 2048 px suffisent, même zoomés.
        self.worker.compare(paths.clone(), 2048, self.compare_tag);
        self.compare = Some(Compare {
            textures: vec![None; paths.len()],
            paths,
            tag: self.compare_tag,
            zoom: 1.0,
            center: vec2(0.5, 0.5),
            loupe: false,
        });
    }

    /// Toutes les photos de la comparaison sont‑elles rendues ?
    pub fn compare_ready(&self) -> bool {
        self.compare
            .as_ref()
            .is_some_and(|c| c.textures.iter().all(Option::is_some))
    }

    /// Les photos comparées (vide hors de la comparaison).
    pub fn compared(&self) -> Vec<PathBuf> {
        self.compare
            .as_ref()
            .map_or_else(Vec::new, |c| c.paths.clone())
    }

    /// Le zoom commun de la comparaison et le point au centre de chaque case.
    pub fn compare_view_state(&self) -> Option<(f32, [f32; 2], bool)> {
        self.compare
            .as_ref()
            .map(|c| (c.zoom, [c.center.x, c.center.y], c.loupe))
    }

    /// Agrandit la comparaison (toutes les cases ensemble) autour de `at` (0 à 1).
    pub fn compare_zoom(&mut self, factor: f32, at: [f32; 2]) {
        if let Some(c) = &mut self.compare {
            let before = c.zoom;
            c.zoom = (c.zoom * factor).clamp(1.0, 8.0);
            // Le point sous le pointeur reste sous le pointeur.
            let at = vec2(at[0], at[1]);
            c.center = at + (c.center - at) * (before / c.zoom);
            clamp_center(c);
        }
    }

    /// Déplace la vue de toutes les cases, en fraction de la photo.
    pub fn compare_pan(&mut self, by: [f32; 2]) {
        if let Some(c) = &mut self.compare {
            c.center -= vec2(by[0], by[1]);
            clamp_center(c);
        }
    }

    fn compare_view(&mut self, ui: &mut Ui, full: Rect, top: Rect, m: f32) {
        let area = Rect::from_min_max(
            pos2(full.min.x + m, top.max.y + m),
            full.max - vec2(m, m + 40.0),
        );
        let n = self.compare.as_ref().map_or(0, |c| c.paths.len());
        let (cols, rows) = match n {
            2 => (2, 1),
            3 if area.width() > area.height() * 2.2 => (3, 1),
            _ => (2, 2),
        };
        let gap = 8.0;
        let cell = vec2(
            (area.width() - gap * (cols - 1) as f32) / cols as f32,
            (area.height() - gap * (rows - 1) as f32) / rows as f32,
        );
        let painter = ui.painter_at(area);
        let mut action: Option<(usize, egui::Response, Rect)> = None;
        let mut pointer_at: Option<egui::Vec2> = None;
        for i in 0..n {
            let (cx, cy) = (i % cols, i / cols);
            let rect = Rect::from_min_size(
                area.min + vec2(cx as f32 * (cell.x + gap), cy as f32 * (cell.y + gap)),
                cell,
            );
            painter.rect_filled(rect, 8.0, Color32::from_black_alpha(90));
            let id = ui.scope_id().with(("comparaison", i));
            let response = ui.interact(rect, id, egui::Sense::click_and_drag());
            let Some(c) = &self.compare else { return };
            let fitted = match &c.textures[i] {
                Some(t) => {
                    let size = t.size_vec2();
                    let scale =
                        ((rect.width() - 12.0) / size.x).min((rect.height() - 34.0) / size.y);
                    let fitted =
                        Rect::from_center_size(rect.center() - vec2(0.0, 11.0), size * scale);
                    let half = 0.5 / c.zoom;
                    let uv = Rect::from_min_max(
                        pos2(c.center.x - half, c.center.y - half),
                        pos2(c.center.x + half, c.center.y + half),
                    );
                    painter.image(t.id(), fitted, uv, Color32::WHITE);
                    Some(fitted)
                }
                None => {
                    centered_text(
                        &painter,
                        rect.center(),
                        rect.width() - 20.0,
                        tr("Rendu…").to_owned(),
                    );
                    None
                }
            };
            let path = c.paths[i].clone();
            let meta = self.meta.get(&path).cloned().unwrap_or_default();
            let current = self.current_path() == Some(path.as_path());
            painter.text(
                pos2(rect.min.x + 12.0, rect.max.y - 14.0),
                egui::Align2::LEFT_CENTER,
                file_name(&path),
                egui::FontId::proportional(12.0),
                if current {
                    theme::ACCENT
                } else {
                    Color32::from_gray(0xd8)
                },
            );
            paint_meta(
                &painter,
                Rect::from_min_max(
                    pos2(rect.max.x - 120.0, rect.max.y - 26.0),
                    rect.max - vec2(4.0, 0.0),
                ),
                &meta,
            );
            if current {
                painter.rect_stroke(
                    rect,
                    8.0,
                    egui::Stroke::new(2.0, theme::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
            if let Some(f) = fitted {
                if let Some(p) = response.hover_pos().filter(|p| f.contains(*p)) {
                    let c = &self.compare.as_ref().expect("comparaison");
                    let local = (p - f.min) / f.size();
                    let half = 0.5 / c.zoom;
                    pointer_at = Some(vec2(
                        c.center.x - half + local.x * 2.0 * half,
                        c.center.y - half + local.y * 2.0 * half,
                    ));
                }
                action = action.or_else(|| {
                    (response.clicked() || response.dragged() || response.hovered())
                        .then(|| (i, response.clone(), f))
                });
            }
        }
        // La molette (ou le pincement) agrandit toutes les cases autour du point visé ;
        // le glisser les déplace ensemble ; un double clic passe du tout au 3:1 ; un clic
        // ouvre la photo.
        if let Some((i, response, fitted)) = action {
            let (scroll, pinch) = ui.input(|inp| (inp.smooth_scroll_delta.y, inp.zoom_delta()));
            let factor = pinch * (scroll / 400.0).exp();
            if let Some(at) = pointer_at
                && (factor - 1.0).abs() > 1e-3
            {
                self.compare_zoom(factor, [at.x, at.y]);
            }
            if response.dragged() {
                let zoom = self.compare.as_ref().map_or(1.0, |c| c.zoom);
                let d = response.drag_delta() / fitted.size() / zoom;
                self.compare_pan([d.x, d.y]);
            }
            if response.double_clicked() {
                let at = pointer_at.unwrap_or(vec2(0.5, 0.5));
                let zoom = self.compare.as_ref().map_or(1.0, |c| c.zoom);
                self.compare_zoom(if zoom > 1.0 { 1.0 / zoom } else { 3.0 }, [at.x, at.y]);
            } else if response.clicked() {
                let path = self.compare.as_ref().map(|c| c.paths[i].clone());
                if let Some(k) = path.and_then(|p| self.folder.iter().position(|f| *f == p)) {
                    self.select(k);
                }
            }
        }
        // La loupe : le même point de chaque photo, agrandi quatre fois, sous le pointeur.
        if let (Some(at), Some(c)) = (pointer_at, &self.compare)
            && c.loupe
            && let Some(p) = ui.input(|i| i.pointer.hover_pos())
        {
            let radius = 90.0;
            let lens = Rect::from_center_size(p, vec2(radius * 2.0, radius * 2.0));
            // Une loupe par case, au même endroit de chacune.
            for i in 0..n {
                let Some(t) = &c.textures[i] else { continue };
                let (cx, cy) = (i % cols, i / cols);
                let rect = Rect::from_min_size(
                    area.min + vec2(cx as f32 * (cell.x + gap), cy as f32 * (cell.y + gap)),
                    cell,
                );
                let size = t.size_vec2();
                let scale = ((rect.width() - 12.0) / size.x).min((rect.height() - 34.0) / size.y);
                let fitted = Rect::from_center_size(rect.center() - vec2(0.0, 11.0), size * scale);
                let half = 0.5 / c.zoom;
                let local = (at - vec2(c.center.x - half, c.center.y - half)) / (2.0 * half);
                let spot = fitted.min + local * fitted.size();
                let place = lens.translate(spot - p);
                let span = vec2(radius * 2.0, radius * 2.0) / fitted.size() * (2.0 * half) / 4.0;
                let uv = Rect::from_center_size(pos2(at.x, at.y), span);
                let clip = ui.painter_at(place.intersect(area));
                clip.image(t.id(), place, uv, Color32::WHITE);
                clip.rect_stroke(
                    place,
                    radius,
                    egui::Stroke::new(2.0, Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            }
        }
        let bar = Rect::from_min_max(
            pos2(area.min.x, area.max.y + 6.0),
            pos2(area.max.x, full.max.y - m),
        );
        let zoom = self.compare.as_ref().map_or(1.0, |c| c.zoom);
        let hint = trf(
            "Comparaison · {zoom} · molette ou double clic : zoom · glisser : déplacer · L : loupe · clic : ouvrir · Échap pour sortir",
            &[("zoom", &format!("×{}", (zoom * 10.0).round() / 10.0))],
        );
        centered_text(
            &ui.painter_at(bar),
            pos2(bar.center().x, bar.min.y + 4.0),
            bar.width(),
            hint,
        );
    }

    /// Le tri en plein écran : la photo seule, et en bas sa note, son drapeau, son libellé
    /// et les touches.
    fn culling_view(&mut self, ui: &mut Ui, full: Rect, m: f32) {
        // Le diaporama : la photo seule, tout l'écran ; en pause, une ligne d'aide.
        if let Some(p) = &self.playing {
            let paused = p.paused;
            ui.painter().rect_filled(full, 0.0, Color32::BLACK);
            self.slideshow_frame(ui, full);
            if paused {
                ui.painter().text(
                    pos2(full.center().x, full.max.y - 24.0),
                    egui::Align2::CENTER_BOTTOM,
                    tr("Pause · Espace pour reprendre · flèches : photos · Échap pour sortir"),
                    egui::FontId::proportional(12.5),
                    Color32::from_gray(0xb0),
                );
            }
            return;
        }
        let bar = Rect::from_min_max(
            pos2(full.min.x + m, full.max.y - m - 64.0),
            full.max - vec2(m, m),
        );
        let stage = Rect::from_min_max(full.min, pos2(full.max.x, bar.min.y - m * 0.5));
        ui.scope_builder(egui::UiBuilder::new().max_rect(stage), |ui| {
            ui.set_clip_rect(stage);
            self.viewer(ui);
        });
        self.island(ui, bar, |s, ui| s.culling_bar(ui));
    }

    /// Une image du diaporama : la photo affichée et, pendant la transition, celle qui
    /// part (les mêmes transitions que le diaporama exporté).
    fn slideshow_frame(&mut self, ui: &mut Ui, full: Rect) {
        use luma_render::slideshow::Transition;
        let ppp = ui.ctx().pixels_per_point();
        self.request_render(full.width().max(full.height()) * ppp, false);
        let now = std::time::Instant::now();
        let (transition, fade) = (self.show.transition, self.show.fade.max(0.0));
        let photo_time = self.show.seconds.max(0.5) + 2.0 * fade;
        let has_texture = self.texture.is_some();
        let Some(p) = &mut self.playing else {
            return;
        };
        if has_texture && p.arrived.is_none() {
            p.arrived = Some(now);
        }
        let progress = p.arrived.map_or(0.0, |a| {
            if fade > 0.0 {
                (now - a).as_secs_f32() / fade
            } else {
                1.0
            }
        });
        if progress >= 1.0 {
            p.leaving = None;
        }
        let t = progress.clamp(0.0, 1.0);
        let painter = ui.painter_at(full);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let frame = SlideFrame {
            transition,
            t,
            photo_time,
            now,
            area: full,
        };
        if let Some(l) = &p.leaving {
            let (rect, alpha) = frame.place(l.size, l.arrived, l.count, true);
            painter.image(l.texture.id(), rect, uv, Color32::from_white_alpha(alpha));
        }
        if let (Some(texture), Some(arrived)) = (&self.texture, p.arrived) {
            let (rect, alpha) = frame.place(self.image_size, arrived, p.count, false);
            painter.image(texture.id(), rect, uv, Color32::from_white_alpha(alpha));
        }
        // Une transition en cours, ou le zoom lent : l'image suivante, à 30 images par
        // seconde au plus.
        if p.leaving.is_some() || (transition == Transition::Drift && !p.paused) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
    }

    fn culling_bar(&mut self, ui: &mut Ui) {
        let Some(path) = self.current_path().map(Path::to_path_buf) else {
            return;
        };
        let meta = self.current_meta();
        let visible = self.visible();
        let at = self
            .current
            .and_then(|c| visible.iter().position(|&v| v == c))
            .map_or(0, |p| p + 1);
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new(file_name(&path)).size(13.0).strong());
            ui.label(
                RichText::new(format!("{at} / {}", visible.len()))
                    .size(11.5)
                    .color(theme::DIM),
            );
            ui.add_space(12.0);
            if let Some(n) = widgets::rating(ui, meta.rating, 18.0) {
                self.act(["rate0", "rate1", "rate2", "rate3", "rate4", "rate5"][n as usize]);
            }
            ui.add_space(8.0);
            for (f, name, action) in [
                (Flag::Pick, tr("Retenue"), "pick"),
                (Flag::Reject, tr("Rejetée"), "reject"),
            ] {
                if ui.selectable_label(meta.flag == f, name).clicked() {
                    self.act(if meta.flag == f { "unflag" } else { action });
                }
            }
            ui.add_space(8.0);
            if let Some(n) = label_dots(ui, &meta.label) {
                self.act(["label0", "label1", "label2", "label3", "label4"][n]);
            }
            ui.add_space(8.0);
            let mut auto = self.auto_advance;
            if ui
                .checkbox(&mut auto, tr("Avance automatique (A)"))
                .changed()
            {
                self.auto_advance = auto;
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(tr(
                    "Flèches : photos · 0-5 note · P X U drapeau · 6-9 libellé · Échap pour sortir",
                ))
                .size(11.0)
                .color(theme::DIM),
            );
        });
    }

    /// Ouvre la photo à cette place du film.
    pub fn set_current(&mut self, index: usize) {
        self.select(index);
    }

    /// Le tri en plein écran est‑il ouvert ?
    pub fn is_culling(&self) -> bool {
        self.culling
    }

    /// Charge l'état Premium d'un autre dossier (tests, plusieurs comptes).
    pub fn use_licence_dir(&mut self, dir: PathBuf) {
        self.licence = crate::licence::Licence::load_from(Some(dir));
    }

    /// L'écran Premium, par‑dessus tout : les formules, la clé. `locked` : l'essai est
    /// fini, on ne peut pas le fermer.
    fn premium_screen(&mut self, ctx: &egui::Context, full: Rect, locked: bool) {
        // Le voile : rien derrière ne répond. Il est sur une couche au‑dessous de la
        // carte : toucher le voile le ferait sinon passer devant elle (egui remonte la
        // couche touchée), la carte grisée et ses boutons hors d'atteinte. Le toucher
        // ferme l'écran, sauf quand l'essai est fini.
        egui::Area::new(egui::Id::unique("premium-voile"))
            .order(egui::Order::Middle)
            .fixed_pos(full.min)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(full.size(), egui::Sense::click_and_drag());
                ui.painter().rect_filled(
                    r,
                    0.0,
                    Color32::from_black_alpha(if locked { 215 } else { 170 }),
                );
                if resp.clicked() && !locked {
                    self.premium_open = false;
                }
            });
        let w = (full.width() - 24.0).min(680.0);
        // La feuille prend la hauteur de son contenu (mesurée à l'image d'avant), sans
        // dépasser l'écran : au‑delà, elle défile.
        let tall = egui::Id::unique("premium-hauteur");
        let content = ctx.data(|d| d.get_temp::<f32>(tall)).unwrap_or(600.0);
        let h = (full.height() - 24.0).min(content + 52.0);
        let card = Rect::from_center_size(full.center(), vec2(w, h));
        egui::Area::new(egui::Id::unique("premium"))
            .order(egui::Order::Foreground)
            .fixed_pos(card.min)
            .show(ctx, |ui| {
                ui.set_min_size(card.size());
                ui.set_max_size(card.size());
                let p = ui.painter();
                p.add(
                    egui::Shadow {
                        offset: [0, 20],
                        blur: 60,
                        spread: 0,
                        color: Color32::from_black_alpha(200),
                    }
                    .as_shape(card, egui::CornerRadius::same(26)),
                );
                p.rect(
                    card,
                    26.0,
                    Color32::from_rgb(0x0d, 0x0d, 0x12),
                    egui::Stroke::new(1.0, theme::GLASS_EDGE),
                    egui::StrokeKind::Inside,
                );
                let inner = card.shrink(26.0);
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.premium_contents(ui, locked);
                        let used = ui.min_rect().height().ceil();
                        if (used - content).abs() > 0.5 {
                            ui.ctx().data_mut(|d| d.insert_temp(tall, used));
                            ui.ctx().request_repaint();
                        }
                    });
                });
            });
    }

    fn premium_contents(&mut self, ui: &mut Ui, locked: bool) {
        use crate::licence::{FEATURES, open_url};
        use luma_licence::Access;
        let access = self.licence.access();
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(54.0, 54.0), egui::Sense::hover());
            if let Some(b) = &self.brand {
                ui.painter().image(
                    b.ring.id(),
                    r.expand(6.0),
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("LÜMA Premium")
                        .size(26.0)
                        .extra_letter_spacing(1.5)
                        .color(theme::TEXT),
                );
                let line = match &access {
                    Access::Premium { plan, email, .. } => {
                        let email = email.as_ref().map(|e| format!(" ({e})")).unwrap_or_default();
                        match plan {
                            Some(p) => trf(
                                "Merci ! Votre abonnement {plan} est actif{email}.",
                                &[("plan", &tr(p.name()).to_lowercase()), ("email", &email)],
                            ),
                            None => trf(
                                "Merci ! Votre abonnement est actif{email}.",
                                &[("email", &email)],
                            ),
                        }
                    }
                    Access::Trial { days_left: 1 } => {
                        tr("Il vous reste 1 jour d'essai gratuit.").into()
                    }
                    Access::Trial { days_left } => trf(
                        "Il vous reste {n} jours d'essai gratuit.",
                        &[("n", days_left)],
                    ),
                    Access::Expired { .. } => tr("Votre essai est terminé : abonnez-vous pour continuer à développer vos photos.").into(),
                };
                ui.label(RichText::new(line).size(13.5).color(theme::DIM));
            });
            if !locked {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    if ui.button(tr("Fermer")).clicked() {
                        self.premium_open = false;
                    }
                });
            }
        });
        ui.add_space(14.0);
        for f in FEATURES {
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 18.0), egui::Sense::hover());
                widgets::spectral_ring(ui.painter(), r.center(), 4.0, 2.0, 1.0);
                ui.vertical(|ui| {
                    ui.add(
                        egui::Label::new(RichText::new(tr(f)).size(13.5).color(theme::TEXT)).wrap(),
                    );
                });
            });
            ui.add_space(2.0);
        }
        ui.add_space(18.0);
        let plans = crate::licence::plans();
        // Sur iPhone et iPad, la formule s'achète dans l'App Store ; sur Mac, par le
        // lien de paiement Stripe.
        let app_store = crate::licence::app_store();
        let gap = 12.0;
        // Côte à côte, ou l'une sous l'autre sur un écran étroit ; une seule formule
        // (l'achat unique de l'App Store) prend toute la largeur.
        let stacked = ui.available_width() < 470.0 || plans.len() == 1;
        let card_w = if stacked {
            ui.available_width()
        } else {
            ((ui.available_width() - gap) / 2.0).floor()
        };
        let mut chosen = None;
        let mut buy = None;
        let mut card = |ui: &mut Ui, plan: luma_licence::Plan| {
            let best = plan != luma_licence::Plan::Monthly;
            egui::Frame::new()
                .fill(theme::SECTION)
                .stroke(egui::Stroke::new(
                    if best { 1.5 } else { 1.0 },
                    if best {
                        theme::ACCENT
                    } else {
                        theme::GLASS_EDGE
                    },
                ))
                .corner_radius(18)
                .inner_margin(18)
                .show(ui, |ui| {
                    ui.set_width(card_w - 36.0);
                    let badge = match plan {
                        luma_licence::Plan::Lifetime => Some(tr("Une fois pour toutes")),
                        luma_licence::Plan::Annual => Some(tr("Un seul paiement par an")),
                        luma_licence::Plan::Monthly => None,
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(tr(plan.name()))
                                .size(16.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                        if let Some(badge) = badge {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    widgets::pill(ui, badge);
                                },
                            );
                        }
                    });
                    ui.add_space(6.0);
                    // Le prix de l'App Store est dans la devise de l'acheteur.
                    let price = match self.licence.store_price(plan) {
                        Some(p) if app_store => p.to_owned(),
                        _ => tr(plan.price()).to_owned(),
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(price).size(32.0).strong().color(theme::TEXT));
                        ui.label(
                            RichText::new(tr(plan.period()))
                                .size(14.0)
                                .color(theme::DIM),
                        );
                    });
                    ui.add_space(2.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(tr(plan.note())).size(12.5).color(theme::DIM),
                        )
                        .wrap(),
                    );
                    ui.add_space(14.0);
                    if app_store {
                        let label = if plan == luma_licence::Plan::Lifetime {
                            tr("Acheter")
                        } else {
                            tr("S'abonner")
                        };
                        if widgets::call_to_action(ui, label, !self.licence.busy()) {
                            buy = Some(plan);
                        }
                    } else if let Some(url) = plan.checkout_url() {
                        if widgets::call_to_action(ui, tr("S'abonner"), true) {
                            chosen = Some(url);
                        }
                    } else {
                        widgets::call_to_action(ui, tr("Bientôt disponible"), false);
                    }
                });
        };
        if stacked {
            for (k, plan) in plans.iter().copied().enumerate() {
                if k > 0 {
                    ui.add_space(gap);
                }
                card(ui, plan);
            }
        } else {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for plan in plans.iter().copied() {
                    ui.vertical(|ui| card(ui, plan));
                }
            });
        }
        if let Some(plan) = buy {
            self.licence.buy(plan);
        }
        if let Some(url) = chosen {
            if open_url(url) {
                self.licence.message =
                    tr("La page de paiement s'est ouverte : après le paiement, votre clé s'affiche sur le site.").into();
            } else {
                ui.ctx().copy_text(url.to_owned());
                self.licence.message = tr("Lien de paiement copié : ouvrez-le dans Safari.").into();
            }
        }
        ui.add_space(16.0);
        if app_store {
            // L'App Store : restaurer un abonnement, le gérer dans les Réglages.
            if let Access::Premium {
                email: Some(email), ..
            } = &access
            {
                ui.label(
                    RichText::new(trf(
                        "LÜMA Premium vous est offert ({email}) : sur vos autres iPhone et iPad, « Se connecter avec Apple » avec le même compte.",
                        &[("email", email)],
                    ))
                    .size(12.5)
                    .color(theme::DIM),
                );
            } else if matches!(access, Access::Premium { .. }) {
                ui.label(
                    RichText::new(tr(
                        "LÜMA Premium est à vous, pour toujours : sur vos autres iPhone et iPad, « Restaurer les achats » avec le même compte Apple.",
                    ))
                    .size(12.5)
                    .color(theme::DIM),
                );
            } else {
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(tr("Déjà acheté avec ce compte Apple ?"))
                            .size(12.5)
                            .color(theme::DIM),
                    );
                    if self.licence.busy() {
                        ui.spinner();
                    } else if ui
                        .add(
                            egui::Button::new(
                                RichText::new(tr("Restaurer les achats")).color(theme::ACCENT),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.licence.restore();
                    }
                    // L'accès offert : le compte Apple prouve le courriel, le serveur
                    // donne la clé s'il est dans la liste.
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(tr("LÜMA vous est offert ?"))
                            .size(12.5)
                            .color(theme::DIM),
                    );
                    ui.add_space(4.0);
                    let apple = egui::Button::new(
                        RichText::new(tr("Se connecter avec Apple"))
                            .size(15.0)
                            .strong()
                            .color(egui::Color32::BLACK),
                    )
                    .fill(egui::Color32::WHITE)
                    .corner_radius(8.0)
                    .min_size(egui::vec2(240.0, 44.0));
                    if ui.add_enabled(!self.licence.busy(), apple).clicked() {
                        self.licence.sign_in_with_apple();
                    }
                });
            }
        } else if matches!(access, Access::Premium { .. }) {
            ui.horizontal(|ui| {
                if let Some(url) = luma_licence::billing_portal()
                    && ui.button(tr("Gérer mon abonnement")).clicked()
                    && !open_url(url)
                {
                    ui.ctx().copy_text(url.to_owned());
                }
                if ui
                    .add_enabled(
                        !self.licence.busy(),
                        egui::Button::new(tr("Retirer de cet appareil")),
                    )
                    .on_hover_text(tr("La clé reste valable sur vos autres appareils"))
                    .clicked()
                {
                    self.licence.deactivate();
                }
            });
        } else {
            ui.label(
                RichText::new(tr(
                    "Déjà abonné ? Votre clé de licence s'affiche sur le site après le paiement (gardez ce lien).",
                ))
                .size(12.5)
                .color(theme::DIM),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.licence.key_input)
                        .hint_text(tr("Clé de licence"))
                        .desired_width((ui.available_width() - 90.0).min(330.0)),
                );
                let ready = self.licence.key_input.trim().len() >= 8;
                if ui
                    .add_enabled(ready, egui::Button::new(tr("Activer")))
                    .clicked()
                {
                    self.licence.activate();
                }
                if self.licence.busy() {
                    ui.spinner();
                }
            });
        }
        if !self.licence.message.is_empty() {
            ui.add_space(6.0);
            ui.label(
                RichText::new(&self.licence.message)
                    .size(12.5)
                    .color(theme::ACCENT),
            );
        }
    }

    /// Un îlot de verre fumé à `rect`, son contenu dessiné par `f`.
    fn island(&mut self, ui: &mut Ui, rect: Rect, f: impl FnOnce(&mut Self, &mut Ui)) {
        let radius = egui::CornerRadius::same(18);
        let shadow = egui::Shadow {
            offset: [0, 10],
            blur: 30,
            spread: 0,
            color: Color32::from_black_alpha(150),
        };
        let painter = ui.painter();
        painter.add(shadow.as_shape(rect, radius));
        painter.rect(
            rect,
            radius,
            theme::PANEL,
            egui::Stroke::new(1.0, theme::GLASS_EDGE),
            egui::StrokeKind::Inside,
        );
        let inner = rect.shrink(12.0);
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.set_clip_rect(inner.expand(4.0));
            f(self, ui);
        });
    }

    /// La scène : la photo (ou la grille de la Bibliothèque), sans cadre.
    fn stage(&mut self, ui: &mut Ui, rect: Rect) {
        // La Bibliothèque : la barre de la vue (grille ou carte) au‑dessus.
        let rect = if self.module == Module::Library && !self.folder.is_empty() {
            // Aussi haute que ses boutons (plus hauts au doigt), aux marges de la grille.
            let pad = if self.compact { 8.0 } else { 14.0 };
            let bar = rect.with_max_y(rect.min.y + ui.spacing().interact_size.y + 8.0);
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(bar.shrink2(vec2(pad, 4.0)))
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
                |ui| {
                    ui.set_clip_rect(bar);
                    self.view_bar(ui)
                },
            );
            rect.with_min_y(bar.max.y)
        } else {
            rect
        };
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.set_clip_rect(rect);
            match self.module {
                Module::Develop => self.viewer(ui),
                Module::Library if self.map.is_some() => self.map(ui),
                Module::Library => self.grid(ui),
            }
        });
    }

    /// Grille ou carte ; revenir au film entier après un lieu ou des doublons.
    fn view_bar(&mut self, ui: &mut Ui) {
        let map = self.map.is_some();
        if ui
            .selectable_label(map, tr("Carte"))
            .on_hover_text(tr(
                "Les photos du film à l'endroit de leur prise de vue (position GPS)",
            ))
            .clicked()
            && !map
        {
            self.toggle_map();
        }
        if ui.selectable_label(!map, tr("Grille")).clicked() && map {
            self.toggle_map();
        }
        if map {
            ui.add_space(10.0);
            if ui.button(tr("Cadrer sur les photos")).clicked() {
                self.map = Some(None);
            }
            // Le choix d'un fichier n'existe pas encore sur iPhone et iPad.
            if !cfg!(target_os = "ios")
                && ui
                    .button(tr("Suivre une trace GPX…"))
                    .on_hover_text(tr(
                        "Placer les photos sans position d'après une trace GPX (téléphone, montre, GPS)",
                    ))
                    .clicked()
                && let Some(path) = crate::dialogs::pick_gpx()
            {
                self.follow_gpx(&path);
            }
        } else if self.place.is_some() {
            ui.add_space(10.0);
            if ui.button(tr("Tout afficher")).clicked() {
                self.hide_duplicates();
            }
        }
        ui.add_space(16.0);
        // Sur un écran étroit, il est dans « ••• » : la barre tient sur une ligne.
        if !self.compact
            && ui
                .button(tr("Livre, diaporama, galerie…"))
            .on_hover_text(tr(
                "Un diaporama (dans l'app ou exporté), un livre photo en PDF et une galerie web",
            ))
            .clicked()
        {
            self.outputs_open = true;
        }
        if ui
            .button(tr("Diaporama"))
            .on_hover_text(tr(
                "En plein écran : Espace pour la pause, flèches pour les photos, Échap pour sortir",
            ))
            .clicked()
        {
            self.start_slideshow();
        }
    }

    fn set_tool(&mut self, tool: Tool) {
        if self.tool == tool {
            return;
        }
        if tool != Tool::Geometry && self.crop_tool.is_some() {
            self.act("crop_done");
        }
        if tool != Tool::Masks {
            self.selected_mask = None;
        }
        self.eyedropper = false;
        self.tool = tool;
        self.module = if tool == Tool::Library {
            Module::Library
        } else {
            Module::Develop
        };
    }

    /// L'îlot du haut : la marque, la photo, les gestes d'ensemble.
    fn top_island(&mut self, ui: &mut Ui, compact: bool) {
        ui.horizontal_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), egui::Sense::hover());
            if let Some(b) = &self.brand {
                ui.painter().image(
                    b.ring.id(),
                    r.expand(4.0),
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            ui.add_space(2.0);
            // Sur iPhone, l'anneau suffit : la place va aux boutons.
            if !compact {
                ui.label(
                    RichText::new("LÜMA")
                        .size(15.0)
                        .extra_letter_spacing(5.0)
                        .color(theme::TEXT),
                );
            }
            // La puce Premium : l'essai, ou l'abonnement ; un clic ouvre l'écran Premium.
            let premium = matches!(self.licence.access(), luma_licence::Access::Premium { .. });
            let badge = egui::Button::new(
                RichText::new(self.licence.badge())
                    .size(11.0)
                    .color(if premium { theme::TEXT } else { theme::ACCENT }),
            )
            .fill(if premium {
                theme::SECTION
            } else {
                Color32::from_rgb(0x3a, 0x2a, 0x14)
            })
            .corner_radius(10);
            if ui.add(badge).on_hover_text("LÜMA Premium").clicked() {
                self.premium_open = true;
            }
            if !compact {
                self.settings_menu(ui);
            }
            if !compact {
                ui.add_space(14.0);
                if let Some(p) = self.current_path() {
                    ui.label(RichText::new(file_name(p)).size(12.5).color(theme::DIM));
                }
                if self.render_ms > 0.0 {
                    let chip = format!(
                        "{} ms{}",
                        format_value(self.render_ms, 0, false),
                        if self.render_gpu { " · GPU" } else { "" }
                    );
                    egui::Frame::new()
                        .fill(theme::SECTION)
                        .corner_radius(10)
                        .inner_margin(egui::Margin::symmetric(8, 2))
                        .show(ui, |ui| {
                            ui.label(RichText::new(chip).size(11.0).color(theme::DIM))
                        });
                }
            }
            let has_photo = self.photo.is_some();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Sur iPhone, le reste des commandes est dans « ••• ».
                if compact {
                    self.more_menu(ui);
                    // Sans photo, rien d'autre : l'accueil a son bouton « Importer des
                    // photos », et il n'y a encore rien à exporter ni à annuler.
                    if !has_photo {
                        return;
                    }
                }
                if icon_button(
                    ui,
                    Icon::Export,
                    if compact { "" } else { tr("Exporter") },
                    false,
                )
                .on_hover_text(tr("Maj+Cmd+E"))
                .accessible_name(tr("Exporter"))
                .clicked()
                {
                    self.act("export");
                }
                // Sans photo sur iPhone, l'accueil a son grand bouton « Importer ».
                if (has_photo || !compact)
                    && icon_button(
                        ui,
                        // Sur iPhone et iPad, on ajoute des photos : le « + ».
                        if luma_apple::photos::AVAILABLE {
                            Icon::Add
                        } else {
                            Icon::Open
                        },
                        if compact {
                            ""
                        } else if luma_apple::photos::AVAILABLE {
                            tr("Importer")
                        } else {
                            tr("Ouvrir")
                        },
                        false,
                    )
                    .on_hover_text("Cmd+O")
                    .clicked()
                {
                    self.act("open");
                }
                ui.add_space(6.0);
                if compact
                    && icon_button(ui, Icon::Undo, "", false)
                        .on_hover_text(tr("Annuler"))
                        .accessible_name(tr("Annuler"))
                        .clicked()
                {
                    self.act("undo");
                }
                if !compact {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for (label, action, tip) in [
                        (tr("Réinitialiser"), "reset", tr("Maj+Cmd+R")),
                        (
                            tr("Coller…"),
                            "paste_some",
                            tr("Alt+Maj+Cmd+V : choisir quoi coller"),
                        ),
                        (tr("Coller"), "paste", tr("Maj+Cmd+V")),
                        (tr("Copier"), "copy", tr("Maj+Cmd+C")),
                        (tr("Rétablir"), "redo", tr("Maj+Cmd+Z")),
                        (tr("Annuler"), "undo", "Cmd+Z"),
                    ] {
                        if ui
                            .add(egui::Button::new(RichText::new(label).size(12.0)).frame(false))
                            .on_hover_text(tip)
                            .clicked()
                        {
                            self.act(action);
                        }
                    }
                }
                if compact && !(has_photo && self.module == Module::Develop) {
                    return;
                }
                // Sur iPhone, le bouton dit ce qu'il montre : « Avant », allumé quand on
                // voit la photo sans retouche.
                let label = if self.before || compact {
                    tr("Avant")
                } else {
                    tr("Après")
                };
                if ui
                    .selectable_label(self.before, label)
                    .on_hover_text(tr("\\ : avant / après"))
                    .clicked()
                {
                    self.act("before");
                }
            });
        });
    }

    /// Le rail des outils (vertical ; horizontal sur iPhone).
    fn rail(&mut self, ui: &mut Ui, horizontal: bool) {
        let mut clicked = None;
        // Sur iPhone, la barre est en bas de la photo : chaque outil porte son nom court
        // sous l'icône.
        let (width, size) = if horizontal {
            (
                (ui.available_width() / Tool::ALL.len() as f32).min(64.0),
                36.0,
            )
        } else {
            (44.0, 44.0)
        };
        // Les noms sous les icônes, s'ils ont la place de ne pas se toucher (sinon,
        // en paysage sur iPhone, les icônes seules).
        let named = horizontal && width >= 40.0;
        let font = if width >= 50.0 { 10.5 } else { 9.5 };
        let label = if named { 14.0 } else { 0.0 };
        let mut draw = |ui: &mut Ui| {
            if horizontal {
                // Les outils au milieu de la barre (sur iPad, elle est plus large qu'eux).
                let rest = ui.available_width() - width * Tool::ALL.len() as f32;
                ui.add_space((rest / 2.0).max(0.0).floor());
            }
            for tool in Tool::ALL {
                let (cell, r) =
                    ui.allocate_exact_size(vec2(width, size + label), egui::Sense::click());
                let rect = Rect::from_center_size(
                    pos2(cell.center().x, cell.min.y + size / 2.0),
                    vec2(size, size),
                );
                let active = self.tool == tool;
                let p = ui.painter();
                if active {
                    p.circle_filled(
                        rect.center(),
                        size * 0.46,
                        Color32::from_rgb(0x24, 0x22, 0x1e),
                    );
                    widgets::spectral_ring(p, rect.center(), size * 0.46, 2.2, 1.0);
                } else if r.hovered() {
                    p.circle_filled(rect.center(), size * 0.44, theme::SECTION);
                }
                let color = if active {
                    Color32::WHITE
                } else if r.hovered() {
                    theme::TEXT
                } else {
                    theme::DIM
                };
                icons::paint(p, tool.icon(), rect.shrink(size * 0.27), color);
                if named {
                    p.text(
                        pos2(cell.center().x, cell.max.y),
                        egui::Align2::CENTER_BOTTOM,
                        tool.short_name(),
                        egui::FontId::proportional(font),
                        color,
                    );
                }
                let tip = if tool.key().is_empty() {
                    tool.name().to_owned()
                } else {
                    format!("{} ({})", tool.name(), tool.key())
                };
                // Le nom de l'outil, pour VoiceOver (et les essais).
                r.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::Button, true, active, tool.name())
                });
                if r.on_hover_text(tip).clicked() {
                    clicked = Some(tool);
                }
                if !horizontal {
                    ui.add_space(12.0);
                }
            }
        };
        if horizontal {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                draw(ui)
            });
        } else {
            ui.vertical_centered(draw);
        }
        if let Some(t) = clicked {
            self.set_tool(t);
        }
    }

    /// L'inspecteur : l'outil choisi, et lui seul.
    fn inspector(&mut self, ui: &mut Ui) {
        // Sur iPhone, la barre nomme déjà l'outil : la place va aux réglages.
        if !self.compact {
            self.inspector_title(ui);
            ui.add_space(8.0);
        }
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| match self.tool {
                Tool::Library => {
                    self.library_right(ui);
                    ui.add_space(12.0);
                    self.library_left(ui);
                }
                Tool::Merge => self.merge_panel(ui),
                Tool::Films => {
                    self.scopes(ui, 96.0);
                    ui.add_space(8.0);
                    self.films_panel(ui);
                }
                _ => self.develop_panels(ui),
            });
    }

    /// Le nom et le sous-titre de l'outil, en tête de l'inspecteur.
    fn inspector_title(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), egui::Sense::hover());
            widgets::spectral_ring(ui.painter(), r.center(), 14.0, 2.0, 0.9);
            icons::paint(
                ui.painter(),
                self.tool.icon(),
                r.shrink(8.0),
                Color32::WHITE,
            );
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(self.tool.name())
                        .size(19.0)
                        .strong()
                        .color(theme::TEXT),
                );
                ui.label(
                    RichText::new(self.tool.subtitle())
                        .size(11.5)
                        .color(theme::DIM),
                );
            });
        });
    }

    /// Le mélangeur de couches : un onglet par couche de sortie, et la part de chacune
    /// des trois couches de la photo.
    fn mixer_panel(&mut self, ui: &mut Ui, commits: &mut Vec<String>) {
        egui::CollapsingHeader::new(RichText::new(tr("Mélangeur de couches")).strong())
            .id_salt("Mélangeur de couches")
            .default_open(!self.settings.channel_mixer.is_identity())
            .show(ui, |ui| {
                let outputs = [tr("Rouge"), tr("Vert"), tr("Bleu")];
                ui.horizontal(|ui| {
                    for (k, name) in outputs.iter().enumerate() {
                        ui.selectable_value(&mut self.mixer_tab, k, *name);
                    }
                    if !self.settings.channel_mixer.is_identity()
                        && ui.button(tr("Réinitialiser")).clicked()
                    {
                        self.settings.channel_mixer = luma_project::ChannelMixer::default();
                        commits.push(tr("Mélangeur de couches réinitialisé").into());
                    }
                });
                if ui
                    .add_enabled(self.photo.is_some(), egui::Button::new(tr("Profil depuis une charte")))
                    .on_hover_text(tr(
                        "La photo d'une charte ColorChecker, recadrée au plus juste : LÜMA règle le mélangeur pour en rendre les couleurs exactes. Enregistrez-le ensuite comme préréglage pour les photos de ce boîtier.",
                    ))
                    .clicked()
                    && let Some(label) = self.profile_from_chart()
                {
                    commits.push(label);
                }
                let reference = self.match_reference();
                if ui
                    .add_enabled(
                        reference.is_some(),
                        egui::Button::new(tr("Accorder sur la photo choisie")),
                    )
                    .on_hover_text(tr(
                        "Deux photos de la même scène, prises par deux boîtiers : choisissez aussi celle de référence dans le film, et cette photo en prend les couleurs.",
                    ))
                    .clicked()
                    && let Some(label) = reference.and_then(|r| self.match_colors_to(&r))
                {
                    commits.push(label);
                }
                let out = self.mixer_tab.min(2);
                let mixer = &mut self.settings.channel_mixer;
                let row = match out {
                    0 => &mut mixer.red,
                    1 => &mut mixer.green,
                    _ => &mut mixer.blue,
                };
                let specs = [
                    SliderSpec {
                        label: "Rouge",
                        min: -200.0,
                        max: 200.0,
                        default: 0.0,
                        decimals: 0,
                        track: Track::Plain,
                        log: false,
                    },
                    SliderSpec {
                        label: "Vert",
                        min: -200.0,
                        max: 200.0,
                        default: 0.0,
                        decimals: 0,
                        track: Track::Plain,
                        log: false,
                    },
                    SliderSpec {
                        label: "Bleu",
                        min: -200.0,
                        max: 200.0,
                        default: 0.0,
                        decimals: 0,
                        track: Track::Plain,
                        log: false,
                    },
                ];
                for (k, mut spec) in specs.into_iter().enumerate() {
                    // La couche elle‑même est à 100 % par défaut.
                    if k == out {
                        spec.default = 100.0;
                    }
                    let c = widgets::slider(ui, &spec, &mut row[k]);
                    if c.committed {
                        commits.push(trf(
                            "Mélangeur, couche {out} : {source} {value} %",
                            &[
                                ("out", &outputs[out].to_lowercase()),
                                ("source", &tr(spec.label).to_lowercase()),
                                ("value", &format_value(row[k], 0, false)),
                            ],
                        ));
                    }
                }
            });
    }

    /// L'historique (outil Lumière).
    fn history_panel(&mut self, ui: &mut Ui) {
        let mut restore = None;
        egui::CollapsingHeader::new(RichText::new(tr("Historique")).strong())
            .id_salt("historique")
            .default_open(false)
            .show(ui, |ui| {
                for (i, (label, _)) in self.history.iter().enumerate().rev() {
                    let current = i == self.history_pos;
                    let text = RichText::new(label).size(12.0).color(if current {
                        Color32::WHITE
                    } else if i > self.history_pos {
                        theme::DIM
                    } else {
                        theme::TEXT
                    });
                    if ui.selectable_label(current, text).clicked() {
                        restore = Some(i);
                    }
                }
            });
        if let Some(i) = restore {
            self.restore(i);
        }
    }

    /// Les instantanés de la photo ouverte.
    pub fn snapshots(&self) -> Vec<luma_project::xmp::Snapshot> {
        self.current_meta().snapshots
    }

    /// Un instantané des réglages du moment, rangé dans le XMP de la photo ouverte ; son
    /// nom par défaut est « Instantané 1 », « Instantané 2 »…
    pub fn take_snapshot(&mut self, name: Option<&str>) {
        let Some(path) = self.current_path().map(Path::to_path_buf) else {
            return;
        };
        let n = self.current_meta().snapshots.len() + 1;
        let name = name
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map_or_else(|| trf("Instantané {n}", &[("n", &n)]), str::to_owned);
        let snap = luma_project::xmp::Snapshot::new(&name, &self.settings);
        self.edit_meta(&[path], |m| m.snapshots.push(snap.clone()));
    }

    /// Revient aux réglages d'un instantané (une étape de l'historique, qu'on peut
    /// annuler).
    pub fn apply_snapshot(&mut self, k: usize) {
        let Some(snap) = self.current_meta().snapshots.get(k).cloned() else {
            return;
        };
        if let Some(s) = snap.settings() {
            self.settings = s;
            self.touch();
            self.commit(trf("Instantané : {name}", &[("name", &snap.name)]));
        }
    }

    pub fn delete_snapshot(&mut self, k: usize) {
        if let Some(path) = self.current_path().map(Path::to_path_buf) {
            self.edit_meta(&[path], |m| {
                if k < m.snapshots.len() {
                    m.snapshots.remove(k);
                }
            });
        }
    }

    /// Le panneau des instantanés : en créer un, y revenir d'un clic, le retirer.
    fn snapshots_panel(&mut self, ui: &mut Ui) {
        let snaps = self.snapshots();
        let (mut apply, mut delete, mut take) = (None, None, false);
        egui::CollapsingHeader::new(RichText::new(tr("Instantanés")).strong())
            .id_salt("instantanes")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.snapshot_name)
                            .hint_text(tr("Nom de l'instantané"))
                            .desired_width(ui.available_width() - 70.0),
                    );
                    take = ui
                        .small_button(tr("Créer"))
                        .on_hover_text(tr("Garde les réglages du moment, pour y revenir d'un clic"))
                        .clicked();
                });
                for (k, s) in snaps.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .small_button("×")
                            .on_hover_text(tr("Retirer cet instantané"))
                            .clicked()
                        {
                            delete = Some(k);
                        }
                        if ui
                            .selectable_label(false, RichText::new(&s.name).size(12.0))
                            .on_hover_text(tr("Revenir à ces réglages"))
                            .clicked()
                        {
                            apply = Some(k);
                        }
                    });
                }
                if snaps.is_empty() {
                    ui.label(
                        RichText::new(tr("Aucun instantané pour cette photo."))
                            .size(11.0)
                            .color(theme::DIM),
                    );
                }
            });
        if take {
            let name = std::mem::take(&mut self.snapshot_name);
            self.take_snapshot(Some(&name));
        }
        if let Some(k) = apply {
            self.apply_snapshot(k);
        }
        if let Some(k) = delete {
            self.delete_snapshot(k);
        }
    }

    /// Le nombre de films : les préréglages intégrés, mes pellicules, mes préréglages.
    fn film_count(&self) -> usize {
        PRESETS.len() + self.luts.len() + self.user_presets.len()
    }

    /// Ce qu'est le film `k`.
    fn film(&self, k: usize) -> Film<'_> {
        if let Some(p) = PRESETS.get(k) {
            return Film::Builtin(p);
        }
        let k = k - PRESETS.len();
        match self.luts.get(k) {
            Some(name) => Film::Lut(name),
            None => {
                let (name, look) = &self.user_presets[k - self.luts.len()];
                Film::Mine(name, look)
            }
        }
    }

    /// Le nom du film `k`, tel qu'enregistré (celui des films intégrés n'est pas traduit).
    fn film_key(&self, k: usize) -> &str {
        match self.film(k) {
            Film::Builtin(p) => p.name,
            Film::Lut(n) | Film::Mine(n, _) => n,
        }
    }

    /// Le nom affiché du film `k` (les films intégrés aux noms français sont traduits).
    fn film_name(&self, k: usize) -> &str {
        match self.film(k) {
            Film::Builtin(p) => tr(p.name),
            Film::Lut(n) | Film::Mine(n, _) => n,
        }
    }

    /// Les réglages `base` avec le film `k`, à `amount` (0 à 1).
    fn film_applied(&self, k: usize, base: &Settings, amount: f32) -> Settings {
        match self.film(k) {
            Film::Builtin(p) => p.applied(base, amount),
            Film::Lut(n) => presets::with_lut(base, n, amount),
            Film::Mine(n, look) => presets::with_look(base, n, look, amount),
        }
    }

    /// Le film `k` en préréglage XMP (`.xmp`) dans `path` ; une pellicule mesurée
    /// emporte sa LUT (`.cube`) à côté.
    fn export_film(&self, k: usize, path: &Path) -> Result<(), String> {
        let (group, look) = match self.film(k) {
            Film::Builtin(p) => (format!("LÜMA — {}", tr(p.group.name())), p.look()),
            Film::Lut(n) => (
                format!("LÜMA — {}", tr("Mes pellicules")),
                presets::with_lut(&Settings::default(), n, 1.0),
            ),
            Film::Mine(_, look) => (format!("LÜMA — {}", tr("Mes préréglages")), look.clone()),
        };
        let name = self.film_name(k);
        luma_project::xmp::save(path, &luma_project::xmp::write_preset(name, &group, &look))
            .map_err(|e| format!("{} : {e}", path.display()))?;
        if let Film::Lut(n) = self.film(k)
            && let Some(cube) = luma_image::cube::get(n)
        {
            let lut = path.with_file_name(format!("{}.cube", safe_name(n)));
            std::fs::write(&lut, cube.to_text()).map_err(|e| format!("{} : {e}", lut.display()))?;
        }
        Ok(())
    }

    /// Tous les films, rangés par famille, dans `dir/Préréglages LÜMA`.
    fn export_all_films(&self, dir: &Path) -> Result<(PathBuf, usize), String> {
        let root = dir.join(tr("Préréglages LÜMA"));
        for k in 0..self.film_count() {
            let group = match self.film(k) {
                Film::Builtin(p) => tr(p.group.name()),
                Film::Lut(_) => tr("Mes pellicules"),
                Film::Mine(..) => tr("Mes préréglages"),
            };
            let folder = root.join(group);
            std::fs::create_dir_all(&folder).map_err(|e| format!("{} : {e}", folder.display()))?;
            self.export_film(
                k,
                &folder.join(format!("{}.xmp", safe_name(self.film_name(k)))),
            )?;
        }
        Ok((root, self.film_count()))
    }

    /// Le profil du boîtier mesuré sur la photo affichée (une charte recadrée au plus
    /// juste) : posé dans le mélangeur, par‑dessus celui qui y est. Rend le libellé de
    /// l'historique, ou `None` si la photo ne se développe pas ou n'est pas une charte.
    pub fn profile_from_chart(&mut self) -> Option<String> {
        let photo = self.photo.clone()?;
        // La couleur seule, comme pour `luma colorchecker`.
        let s = Settings {
            white_balance: self.settings.white_balance,
            crop: self.settings.crop,
            transform: self.settings.transform.clone(),
            lens: self.settings.lens.clone(),
            channel_mixer: self.settings.channel_mixer,
            ..Settings::default()
        };
        let mut t = luma_core::Timings::default();
        let base = photo
            .base(
                &s,
                luma_render::Size::Fit(1600),
                &luma_core::Cancel::new(),
                &mut t,
            )
            .ok()?;
        let profile = luma_render::chart::fit_profile(&base.image);
        let mean = |e: &[f32; 24]| e.iter().sum::<f32>() / 24.0;
        // Même corrigée, une photo qui n'est pas une charte reste loin de ses couleurs :
        // rien n'est changé.
        if mean(&profile.after) > 6.0 {
            self.status = trf(
                "Pas une charte recadrée au plus juste (écart de couleur {error}) : mélangeur inchangé",
                &[("error", &format_value(mean(&profile.after), 1, false))],
            );
            return None;
        }
        let total = luma_color::mul(&profile.matrix, &s.channel_mixer.matrix());
        let row = |r: usize| total[r].map(|v| (v * 100.0).clamp(-200.0, 200.0));
        self.settings.channel_mixer = luma_project::ChannelMixer {
            red: row(0),
            green: row(1),
            blue: row(2),
        };
        let label = trf(
            "Profil de charte : écart de couleur de {before} à {after}",
            &[
                ("before", &format_value(mean(&profile.before), 1, false)),
                ("after", &format_value(mean(&profile.after), 1, false)),
            ],
        );
        self.status = label.clone();
        Some(label)
    }

    /// Le recadrage qui retire les bords vides de la Transformation, aux dimensions de la
    /// photo entière (celles du rendu en cours si la photo n'est pas recadrée).
    pub fn fill_crop(&mut self) {
        let (w, h) = self
            .photo
            .as_ref()
            .map(|p| p.full_size())
            .unwrap_or((self.image_size[0], self.image_size[1]));
        self.settings.crop = luma_image::lens::filled_crop(
            &self.settings.lens,
            &self.settings.transform,
            w.max(1),
            h.max(1),
            self.settings.crop.angle,
        );
    }

    /// Les lignes du redressement guidé sur `target` (la photo affichée sans
    /// Transformation, `iw × ih` pixels) : on en trace une en glissant, chacune redresse
    /// aussitôt la photo. Rend le libellé de l'historique quand une ligne est ajoutée.
    fn guide_lines(&mut self, ui: &mut Ui, target: Rect, iw: usize, ih: usize) -> Option<String> {
        // Coordonnées normalisées par la demi‑diagonale, centre en 0.
        let half = (((iw * iw + ih * ih) as f32).sqrt() / 2.0).max(1.0);
        let to_norm = |p: egui::Pos2| {
            (
                ((p.x - target.min.x) / target.width() - 0.5) * iw as f32 / half,
                ((p.y - target.min.y) / target.height() - 0.5) * ih as f32 / half,
            )
        };
        let to_screen = |(x, y): (f32, f32)| {
            pos2(
                target.min.x + (x * half / iw as f32 + 0.5) * target.width(),
                target.min.y + (y * half / ih as f32 + 0.5) * target.height(),
            )
        };
        let response = ui.interact(target, ui.scope_id().with("guides"), egui::Sense::drag());
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        let painter = ui.painter_at(target);
        let stroke = |vertical: bool| {
            egui::Stroke::new(
                2.0,
                if vertical {
                    theme::ACCENT
                } else {
                    Color32::from_rgb(120, 200, 255)
                },
            )
        };
        for g in &self.guides {
            painter.line_segment([to_screen(g.a), to_screen(g.b)], stroke(g.vertical));
        }
        if response.drag_started() {
            self.guide_start = response.interact_pointer_pos();
        }
        let current = ui.ctx().pointer_latest_pos();
        if let (Some(a), Some(b)) = (self.guide_start, current) {
            let vertical = (b.y - a.y).abs() > (b.x - a.x).abs();
            painter.line_segment([a, b], stroke(vertical));
        }
        if response.drag_stopped()
            && let (Some(a), Some(b)) = (self.guide_start.take(), current)
            && a.distance(b) > 20.0
        {
            let vertical = (b.y - a.y).abs() > (b.x - a.x).abs();
            self.guides.push(luma_image::lens::Guide {
                a: to_norm(a),
                b: to_norm(b),
                vertical,
            });
            if self.guides.len() > 4 {
                self.guides.remove(0);
            }
            self.settings.transform =
                luma_image::lens::guided(&self.guides, &self.settings.transform);
            return Some(tr("Redressement guidé").into());
        }
        None
    }

    /// La photo de référence pour accorder les couleurs : l'autre photo choisie dans le
    /// film, quand il y en a exactement deux (celle affichée et elle).
    fn match_reference(&self) -> Option<PathBuf> {
        let current = self.current?;
        let others: Vec<usize> = self
            .selected
            .iter()
            .copied()
            .filter(|&i| i != current)
            .collect();
        match others.as_slice() {
            [one] if self.selected.contains(&current) || self.selected.len() == 1 => {
                self.folder.get(*one).cloned()
            }
            _ => None,
        }
    }

    /// Accorde les couleurs de la photo affichée sur celles de `reference` (la même
    /// scène, un autre boîtier) : la correction va dans le mélangeur. Rend le libellé de
    /// l'historique, ou `None` (raison dans la barre d'état).
    pub fn match_colors_to(&mut self, reference: &Path) -> Option<String> {
        let photo = self.photo.clone()?;
        let develop = |p: &Photo, s: &Settings| {
            let mut t = luma_core::Timings::default();
            p.base(s, Size::Fit(1600), &luma_core::Cancel::new(), &mut t)
                .ok()
        };
        let colour_only = |s: &Settings| Settings {
            white_balance: s.white_balance,
            crop: s.crop,
            transform: s.transform.clone(),
            lens: s.lens.clone(),
            ..Settings::default()
        };
        let other = Photo::open(reference).ok()?;
        let reference_settings = crate::worker::read_sidecar(reference).unwrap_or_default();
        let (Some(a), Some(b)) = (
            develop(&other, &colour_only(&reference_settings)),
            develop(&photo, &colour_only(&self.settings)),
        ) else {
            return None;
        };
        match luma_merge::matching::match_colors(&a.image, &b.image) {
            Ok(m) => {
                let row = |r: usize| m.matrix[r].map(|v| (v * 100.0).clamp(-200.0, 200.0));
                self.settings.channel_mixer = luma_project::ChannelMixer {
                    red: row(0),
                    green: row(1),
                    blue: row(2),
                };
                let label = trf(
                    "Couleurs accordées : écart de {before} à {after}",
                    &[
                        ("before", &format_value(m.before, 1, false)),
                        ("after", &format_value(m.after, 1, false)),
                    ],
                );
                self.status = label.clone();
                Some(label)
            }
            Err(e) => {
                // La raison seule, sans le « décodage impossible » général.
                let reason = match e {
                    luma_core::Error::Decode(why) => why,
                    other => other.to_string(),
                };
                self.status = trf("Couleurs non accordées : {error}", &[("error", &reason)]);
                None
            }
        }
    }

    /// Enregistre le look des réglages actuels comme préréglage.
    fn save_user_preset(&mut self, name: &str) -> Result<(), String> {
        let dir = crate::dialogs::presets_dir().ok_or(tr("pas de dossier Préréglages"))?;
        let path = dir.join(format!("{}.xmp", safe_name(name)));
        let group = format!("LÜMA — {}", tr("Mes préréglages"));
        let text = luma_project::xmp::write_preset(name, &group, &self.settings);
        std::fs::write(&path, text).map_err(|e| format!("{} : {e}", path.display()))?;
        self.reload_user_presets();
        Ok(())
    }

    fn reload_user_presets(&mut self) {
        self.user_presets = crate::dialogs::presets_dir()
            .map(|d| load_user_presets(&d))
            .unwrap_or_default();
        self.previews_for = None;
    }

    /// L'étiquette sous la photo : son film et ses retouches, ou « Avant ».
    fn edit_label(&self, painter: &egui::Painter, photo: Rect) {
        let text = if self.before {
            tr("Avant : sans retouche").to_owned()
        } else {
            let e = presets::summary(&self.settings);
            if !e.is_edited() {
                return;
            }
            summary_text(&e)
        };
        let galley = painter.layout_no_wrap(text, egui::FontId::proportional(12.0), theme::TEXT);
        let size = galley.size() + vec2(34.0, 10.0);
        let w = size.x.min(photo.width() - 16.0);
        let chip = Rect::from_center_size(
            pos2(photo.center().x, photo.max.y - size.y / 2.0 - 10.0),
            vec2(w, size.y),
        );
        painter.rect_filled(chip, size.y / 2.0, Color32::from_black_alpha(170));
        widgets::spectral_ring(
            painter,
            pos2(chip.min.x + 13.0, chip.center().y),
            4.5,
            2.0,
            1.0,
        );
        painter
            .with_clip_rect(
                chip.with_min_x(chip.min.x + 22.0)
                    .with_max_x(chip.max.x - 8.0),
            )
            .galley(
                pos2(chip.min.x + 24.0, chip.center().y - galley.size().y / 2.0),
                galley,
                theme::TEXT,
            );
    }

    /// Relit le dossier des films (de nouvelles LUT ont pu y arriver).
    fn refresh_luts(&mut self) {
        if let Some(d) = crate::dialogs::films_dir() {
            luma_image::cube::load_dir(&d);
        }
        self.luts = luma_image::cube::names();
        // Les aperçus sont refaits avec la nouvelle liste.
        self.previews_for = None;
    }

    /// Choisit le film `k` (à l'intensité en cours).
    fn choose_film(&mut self, k: usize) {
        let base = self
            .preset
            .as_ref()
            .map_or_else(|| self.settings.clone(), |p| p.2.clone());
        let amount = self.preset.as_ref().map_or(1.0, |p| p.1);
        self.settings = self.film_applied(k, &base, amount);
        self.preset = Some((k, amount, base));
        self.commit(trf("Film : {name}", &[("name", &self.film_name(k))]));
        self.touch();
    }

    /// Films : les pellicules et les looks, avec leur aperçu sur la photo.
    fn films_panel(&mut self, ui: &mut Ui) {
        let Some(photo) = self.photo.clone() else {
            ui.label(
                RichText::new(tr("Ouvrez une photo pour essayer les films.")).color(theme::DIM),
            );
            return;
        };
        // Les réglages sans film : ceux d'avant le préréglage choisi.
        let base = self
            .preset
            .as_ref()
            .map_or_else(|| self.settings.clone(), |p| p.2.clone());
        if self.previews_for.as_deref() != self.current_path()
            || self.previews.len() != self.film_count()
        {
            self.previews_tag += 1;
            self.previews = vec![None; self.film_count()];
            self.previews_for = self.current_path().map(Path::to_path_buf);
            let all = (0..self.film_count())
                .map(|k| self.film_applied(k, &base, 1.0))
                .collect();
            self.worker.previews(photo.clone(), all, self.previews_tag);
        }
        if let Some((i, amount, base)) = self.preset.clone() {
            let mut percent = amount * 100.0;
            let spec = SliderSpec {
                label: "Intensité",
                min: 0.0,
                max: 100.0,
                default: 100.0,
                decimals: 0,
                track: Track::Gradient(&theme::SPECTRUM),
                log: false,
            };
            let c = widgets::slider(ui, &spec, &mut percent);
            if c.changed {
                let amount = percent / 100.0;
                self.settings = self.film_applied(i, &base, amount);
                self.preset = Some((i, amount, base.clone()));
                self.touch();
            }
            if c.committed {
                self.commit(trf(
                    "{name} à {n} %",
                    &[
                        ("name", &self.film_name(i)),
                        ("n", &format_value(percent, 0, false)),
                    ],
                ));
            }
            if ui.button(tr("Retirer le film")).clicked() {
                self.settings = base;
                self.preset = None;
                self.commit(tr("Sans film").into());
                self.touch();
            }
            ui.add_space(6.0);
        }
        let mut choose = None;
        // 275 films : une recherche par nom ou par description (« Kodak », « chaud »,
        // « noir et blanc »), sans accents ni majuscules.
        ui.add(
            egui::TextEdit::singleline(&mut self.film_search)
                .hint_text(tr("Rechercher un film"))
                .desired_width(f32::INFINITY),
        )
        .accessible_name(tr("Rechercher un film"));
        let wanted = fold(self.film_search.trim());
        ui.add_space(4.0);
        let mut sections: Vec<(&str, Vec<usize>)> = presets::Group::ALL
            .iter()
            .map(|&g| {
                (
                    g.name(),
                    (0..PRESETS.len())
                        .filter(|&k| PRESETS[k].group == g)
                        .collect(),
                )
            })
            .collect();
        // Mes préréglages, puis mes pellicules (mesurées sur de vraies pellicules, ou
        // importées).
        // Sur iPhone (ou panneau étroit), les films d'abord : les siens et leur gestion
        // (enregistrer, importer, exporter) viennent après.
        let mine = PRESETS.len() + self.luts.len();
        let own = [
            ("Mes préréglages", (mine..self.film_count()).collect()),
            ("Mes pellicules", (PRESETS.len()..mine).collect()),
        ];
        if crate::touch() || ui.available_width() < 420.0 {
            sections.extend(own);
        } else {
            sections.splice(0..0, own);
        }
        let mut export = None;
        // Chaque famille se plie et se déplie (son titre dit combien elle compte de
        // films) ; au départ, seule celle du film appliqué est ouverte, sinon la première.
        let active = self.preset.as_ref().map(|p| p.0);
        // En recherche : seuls les films trouvés, dans leurs familles toutes ouvertes.
        if !wanted.is_empty() {
            for (_, items) in &mut sections {
                items.retain(|&k| {
                    let hint = match self.film(k) {
                        Film::Builtin(p) => tr(p.description),
                        _ => "",
                    };
                    fold(self.film_name(k)).contains(&wanted) || fold(hint).contains(&wanted)
                });
            }
            sections.retain(|(_, items)| !items.is_empty());
            if sections.is_empty() {
                ui.label(RichText::new(tr("Aucun film ne correspond.")).color(theme::DIM));
            }
        }
        let first = sections.iter().position(|(_, items)| !items.is_empty());
        for (n, (title, items)) in sections.into_iter().enumerate() {
            let open = match active {
                Some(k) => items.contains(&k),
                None => Some(n) == first,
            };
            let header = RichText::new(format!("{} ({})", tr(title), items.len())).strong();
            egui::CollapsingHeader::new(header)
                .id_salt(("famille", title))
                .default_open(open)
                .open((!wanted.is_empty()).then_some(true))
                .show(ui, |ui| {
                    let width = ((ui.available_width() - 10.0) / 2.0).floor();
                    if title == "Mes préréglages" {
                        self.my_presets(ui);
                    }
                    if title == "Mes pellicules" {
                        self.my_films(ui, &photo);
                    }
                    for row in items.chunks(2) {
                        ui.horizontal(|ui| {
                            for &k in row {
                                let (rect, r) = ui.allocate_exact_size(
                                    vec2(width, width * 0.72 + 20.0),
                                    egui::Sense::click(),
                                );
                                let image =
                                    Rect::from_min_size(rect.min, vec2(width, width * 0.72));
                                let p = ui.painter();
                                match &self.previews[k] {
                                    Some(t) => {
                                        let [w, h] = t.size();
                                        p.rect_filled(image, 10.0, Color32::from_gray(0x10));
                                        p.image(
                                            t.id(),
                                            fit(image, w, h),
                                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                            Color32::WHITE,
                                        );
                                    }
                                    None => {
                                        p.rect_filled(image, 10.0, theme::SECTION);
                                    }
                                }
                                let active = self.preset.as_ref().is_some_and(|x| x.0 == k);
                                if active {
                                    p.rect_stroke(
                                        image.expand(2.0),
                                        11.0,
                                        egui::Stroke::new(2.0, theme::ACCENT),
                                        egui::StrokeKind::Outside,
                                    );
                                } else if r.hovered() {
                                    p.rect_stroke(
                                        image,
                                        10.0,
                                        egui::Stroke::new(1.0, theme::TEXT),
                                        egui::StrokeKind::Outside,
                                    );
                                }
                                p.text(
                                    pos2(rect.min.x + 2.0, image.max.y + 4.0),
                                    egui::Align2::LEFT_TOP,
                                    self.film_name(k),
                                    egui::FontId::proportional(12.0),
                                    if active { theme::ACCENT } else { theme::TEXT },
                                );
                                let hint = match self.film(k) {
                                    Film::Builtin(p) => tr(p.description),
                                    Film::Lut(_) => {
                                        tr("Pellicule mesurée ou LUT 3D importée (.cube).")
                                    }
                                    Film::Mine(..) => tr("Mon préréglage."),
                                };
                                let r = r.on_hover_text(format!(
                                    "{hint}\n{}",
                                    tr("Clic droit : exporter.")
                                ));
                                r.context_menu(|ui| {
                                    if ui.button(tr("Exporter ce préréglage…")).clicked() {
                                        export = Some(k);
                                        ui.close();
                                    }
                                });
                                if r.clicked() {
                                    choose = Some(k);
                                }
                            }
                        });
                        ui.add_space(4.0);
                    }
                });
        }
        if let Some(k) = choose {
            self.choose_film(k);
        }
        if let Some(k) = export
            && let Some(path) = crate::dialogs::save_preset(&safe_name(self.film_name(k)))
        {
            self.status = match self.export_film(k, &path) {
                Ok(()) => trf("Préréglage exporté : {path}", &[("path", &path.display())]),
                Err(e) => trf("Export impossible : {error}", &[("error", &e)]),
            };
        }
    }

    /// Mes préréglages : enregistrer le look actuel, importer, tout exporter.
    fn my_presets(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(if crate::touch() {
                tr(
                    "En préréglages XMP (.xmp) : touchez longuement un film pour l'exporter seul, \
                    ou tous d'un coup, rangés par famille.",
                )
            } else {
                tr(
                    "En préréglages XMP (.xmp) : clic droit sur un film pour l'exporter seul, \
                    ou tous d'un coup, rangés par famille.",
                )
            })
            .size(11.5)
            .color(theme::DIM),
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_preset_name)
                    .hint_text(tr("Nom du préréglage"))
                    .desired_width(150.0),
            );
            let name = self.new_preset_name.trim().to_owned();
            if ui
                .add_enabled(!name.is_empty(), egui::Button::new(tr("Enregistrer")))
                .on_hover_text(tr(
                    "Le look des réglages actuels (sans l'exposition, la balance ni le recadrage)",
                ))
                .clicked()
            {
                self.status = match self.save_user_preset(&name) {
                    Ok(()) => {
                        self.new_preset_name.clear();
                        trf("Préréglage enregistré : « {name} »", &[("name", &name)])
                    }
                    Err(e) => trf("Préréglage non enregistré : {error}", &[("error", &e)]),
                };
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr("Exporter tous les préréglages…")).clicked()
                && let Some(dir) = crate::dialogs::pick_export_folder()
            {
                self.status = match self.export_all_films(&dir) {
                    Ok((root, n)) => trf(
                        "{n} préréglages exportés dans {path}",
                        &[("n", &n), ("path", &root.display())],
                    ),
                    Err(e) => trf("Export impossible : {error}", &[("error", &e)]),
                };
            }
            if crate::dialogs::FOLDER_PICKER {
                if ui
                    .button(tr("Importer…"))
                    .on_hover_text(tr("Des préréglages XMP (.xmp)"))
                    .clicked()
                {
                    self.import_presets(&crate::dialogs::pick_presets());
                }
            } else if ui.button(tr("Actualiser")).clicked() {
                self.reload_user_presets();
            }
        });
        ui.add_space(4.0);
    }

    /// Copie des préréglages `.xmp` dans mon dossier Préréglages.
    fn import_presets(&mut self, files: &[PathBuf]) {
        if files.is_empty() {
            return;
        }
        let dir = crate::dialogs::presets_dir();
        let mut n = 0;
        for f in files {
            if let (Some(dir), Some(name)) = (&dir, f.file_name())
                && let Ok(text) = std::fs::read_to_string(f)
                && luma_project::xmp::read_preset(&text).is_ok()
                && std::fs::write(dir.join(name), text).is_ok()
            {
                n += 1;
            }
        }
        self.status = trf("{n} préréglage(s) importé(s)", &[("n", &n)]);
        self.reload_user_presets();
    }

    /// Les boutons de « Mes pellicules » : mesurer une vraie pellicule, importer une LUT.
    fn my_films(&mut self, ui: &mut Ui, photo: &Arc<Photo>) {
        ui.label(
            RichText::new(tr(
                "Le rendu exact d'une vraie pellicule : photographiez la même scène en numérique \
                 (cette photo) et sur film, puis choisissez le scan. LÜMA aligne les deux images \
                 et mesure le film, couleur par couleur.",
            ))
            .size(11.5)
            .color(theme::DIM),
        );
        ui.horizontal_wrapped(|ui| {
            if crate::dialogs::FOLDER_PICKER {
                let measure = ui.add_enabled(
                    !self.measuring,
                    egui::Button::new(tr("Mesurer une pellicule…")),
                );
                if measure.clicked()
                    && let Some(scan) = crate::dialogs::pick_scan()
                    && let Some(dir) = crate::dialogs::films_dir()
                {
                    self.measuring = true;
                    self.status = trf(
                        "Mesure de la pellicule : {file}…",
                        &[("file", &file_name(&scan))],
                    );
                    self.worker.measure_film(photo.clone(), scan, dir);
                }
                if ui.button(tr("Importer une LUT…")).clicked()
                    && let Some(path) = crate::dialogs::pick_lut()
                {
                    self.import_lut(&path);
                }
            } else if ui.button(tr("Actualiser")).clicked() {
                self.refresh_luts();
            }
        });
        if self.measuring {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(tr("Alignement et mesure…")).color(theme::DIM));
            });
        }
        if !crate::dialogs::FOLDER_PICKER {
            ui.label(
                RichText::new(tr(
                    "Déposez vos LUT .cube dans LÜMA › Films (app Fichiers).",
                ))
                .size(11.5)
                .color(theme::DIM),
            );
        }
        ui.add_space(4.0);
    }

    /// Copie une LUT dans le dossier des films et la charge.
    fn import_lut(&mut self, path: &Path) {
        let result = (|| -> Result<String, String> {
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            luma_image::cube::Cube::parse(&text).map_err(|e| e.to_string())?;
            let dir = crate::dialogs::films_dir().ok_or(tr("pas de dossier Films"))?;
            let dest = dir.join(path.file_name().ok_or(tr("nom de fichier"))?);
            if dest != path {
                std::fs::write(&dest, text).map_err(|e| e.to_string())?;
            }
            luma_image::cube::load(&dest).map_err(|e| e.to_string())
        })();
        match result {
            Ok(name) => {
                self.status = trf("LUT importée : « {name} »", &[("name", &name)]);
                self.refresh_luts();
                if let Some(k) = self.luts.iter().position(|n| *n == name) {
                    self.choose_film(PRESETS.len() + k);
                }
            }
            Err(e) => self.status = trf("LUT illisible : {error}", &[("error", &e)]),
        }
    }

    /// Fusion : empilements et panoramas des photos choisies.
    fn merge_panel(&mut self, ui: &mut Ui) {
        let n = self.selected.len();
        ui.label(
            RichText::new(match n {
                0 if crate::touch() => tr("Dans la bibliothèque, touchez longuement les photos à fusionner, ou prenez tout le dossier.").to_owned(),
                1 if crate::touch() => tr("Une photo choisie : touchez longuement les autres dans la bibliothèque.").to_owned(),
                0 => tr("Choisissez les photos dans le film : Cmd-clic pour en ajouter une, Maj-clic pour une suite.").to_owned(),
                1 => tr("Une photo choisie : ajoutez-en d'autres (Cmd-clic, Maj-clic).").to_owned(),
                n => trf("{n} photos choisies.", &[("n", &n)]),
            })
            .color(theme::DIM),
        );
        ui.horizontal(|ui| {
            if ui.button(tr("Tout le dossier")).clicked() {
                self.selected = self.visible().into_iter().collect();
            }
            if n > 0 && ui.button(tr("Vider")).clicked() {
                self.selected.clear();
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            for (stack, icon, label) in [
                (true, Icon::Stack, tr("Empilement")),
                (false, Icon::Panorama, tr("Panorama")),
            ] {
                if big_choice(ui, icon, label, self.merge_stack == stack).clicked() {
                    self.merge_stack = stack;
                }
            }
        });
        ui.add_space(8.0);
        if self.merge_stack {
            use luma_merge::StackMode as S;
            for (mode, text) in [
                (S::Mean, tr("Moins de bruit, poses longues simulées")),
                (S::Median, tr("Ce qui bouge disparaît (passants, voitures)")),
                (
                    S::Maximum,
                    tr("Filé d'étoiles, traînées de phares, éclairs"),
                ),
                (
                    S::Stars,
                    tr("Suit le ciel qui tourne, rejette satellites et avions"),
                ),
                (
                    S::Hdr,
                    tr("Bracketing : ombres et ciel détaillés, sans fantômes"),
                ),
                (S::Focus, tr("Tout net, du premier plan au fond")),
            ] {
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.stack_mode,
                        mode,
                        RichText::new(tr(mode.name())).strong(),
                    );
                    ui.label(RichText::new(text).size(11.5).color(theme::DIM));
                });
            }
            ui.add_space(4.0);
            // Les étoiles, le HDR et la mise au point s'alignent toujours.
            let stars = matches!(
                self.stack_mode,
                luma_merge::StackMode::Stars
                    | luma_merge::StackMode::Hdr
                    | luma_merge::StackMode::Focus
            );
            ui.add_enabled_ui(!stars, |ui| {
                let mut align = self.stack_align || stars;
                ui.checkbox(&mut align, tr("Aligner les poses (à main levée)"));
                if !stars {
                    self.stack_align = align;
                }
            });
        } else {
            use luma_merge::Projection as P;
            ui.horizontal(|ui| {
                for p in [P::Perspective, P::Cylindrical, P::Spherical] {
                    ui.selectable_value(&mut self.projection, p, tr(p.name()));
                }
            });
            ui.label(
                RichText::new(match self.projection {
                    P::Perspective => {
                        tr("Les droites restent droites (architecture) ; jusqu'à 120° environ.")
                    }
                    P::Cylindrical => {
                        tr("Les verticales restent droites : le choix des panoramas horizontaux.")
                    }
                    P::Spherical => {
                        tr("Pour les très grands angles et les panoramas sur plusieurs rangées.")
                    }
                })
                .size(11.5)
                .color(theme::DIM),
            );
            ui.checkbox(&mut self.pano_warp, tr("Déformer les bords"))
                .on_hover_text(tr(
                    "Les bords irréguliers du panorama sont étirés pour remplir le rectangle, au lieu d'être rognés",
                ));
        }
        ui.add_space(8.0);
        ui.checkbox(&mut self.auto_crop, tr("Rogner automatiquement"));
        ui.label(
            RichText::new(tr("Le rognage est un recadrage ordinaire : on peut le modifier ou le retirer ensuite (Géométrie)."))
                .size(11.0)
                .color(theme::DIM),
        );
        ui.checkbox(
            &mut self.merge_full,
            tr("Pleine définition (sinon 2048 px, plus rapide)"),
        );
        ui.add_space(10.0);
        if let Some((fraction, label, cancel)) = &self.merging {
            ui.add(
                egui::ProgressBar::new(*fraction)
                    .text(tr(label))
                    .fill(theme::ACCENT),
            );
            if ui.button(tr("Annuler la fusion")).clicked() {
                cancel.cancel();
            }
            return;
        }
        let go = ui.add_enabled(
            n >= 2,
            egui::Button::new(
                RichText::new(trf("Fusionner {n} photos", &[("n", &n.max(2))]))
                    .strong()
                    .color(Color32::BLACK),
            )
            .fill(theme::ACCENT)
            .min_size(vec2(ui.available_width(), 36.0))
            .corner_radius(12),
        );
        if go.clicked() {
            let paths: Vec<PathBuf> = self
                .selected
                .iter()
                .filter_map(|&i| self.folder.get(i).cloned())
                .collect();
            let kind = if self.merge_stack {
                MergeKind::Stack(self.stack_mode, self.stack_align)
            } else {
                MergeKind::Panorama(self.projection, self.pano_warp)
            };
            let cancel = luma_core::Cancel::new();
            self.merging = Some((0.0, tr("Préparation").into(), cancel.clone()));
            self.flush_xmp();
            self.worker.merge(
                MergeJob {
                    paths,
                    kind,
                    auto_crop: self.auto_crop,
                    full_size: self.merge_full,
                },
                cancel,
            );
        }
    }

    /// Bibliothèque, à gauche : le dossier et le filtre.
    fn library_left(&mut self, ui: &mut Ui) {
        self.catalog_panel(ui);
        ui.add_space(14.0);
        self.watch_panel(ui);
        ui.add_space(14.0);
        section_title(ui, tr("Dossier"));
        let dir = match &self.view {
            View::Folder => self
                .folder
                .first()
                .and_then(|p| p.parent())
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| {
                    if crate::touch() {
                        tr("Aucun")
                    } else {
                        tr("Aucun (Cmd+O)")
                    }
                    .into()
                }),
            View::All => tr("Toutes les photos").to_owned(),
            View::Collection(name) => name.clone(),
            View::Search(text) => trf("Recherche : {text}", &[("text", text)]),
        };
        ui.label(RichText::new(dir).size(12.0));
        ui.label(
            RichText::new(trf(
                "{n} photos, {shown} affichées",
                &[("n", &self.folder.len()), ("shown", &self.visible().len())],
            ))
            .size(11.5)
            .color(theme::DIM),
        );
        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            if self.duplicates.is_some() || self.place.is_some() {
                if ui.button(tr("Tout afficher")).clicked() {
                    self.hide_duplicates();
                }
            } else if ui
                .button(tr("Chercher les doublons"))
                .on_hover_text(tr(
                    "La même image dans plusieurs fichiers : copiée, exportée à une autre taille, recompressée",
                ))
                .clicked()
            {
                self.show_duplicates();
            }
        });
        if self.map.is_some() {
            let placed = self.placed().len();
            let missing = self.visible().len() - placed;
            ui.label(
                RichText::new(trf(
                    "{n} photos sur la carte, {m} sans position",
                    &[("n", &placed), ("m", &missing)],
                ))
                .size(11.5)
                .color(theme::DIM),
            );
            let mut online = self.tiles.online;
            if ui
                .checkbox(&mut online, tr("Fond de carte en ligne"))
                .on_hover_text(tr(
                    "Télécharge les tuiles libres de la carte et les garde sur l'appareil ; sinon, celles déjà gardées",
                ))
                .changed()
            {
                self.tiles.set_online(online);
            }
        }
        let mut stacking = self.stacking;
        if ui
            .checkbox(&mut stacking, tr("Empiler les rafales et les séries (S)"))
            .on_hover_text(tr(
                "Les photos d'un même boîtier à moins de deux secondes d'écart, repliées derrière la première",
            ))
            .changed()
        {
            self.stacking = stacking;
        }
        ui.add_space(10.0);
        section_title(ui, tr("Filtre"));
        ui.label(
            RichText::new(tr("Note minimale"))
                .color(theme::DIM)
                .size(11.5),
        );
        if let Some(n) = widgets::rating(ui, self.min_rating, 18.0) {
            self.min_rating = n;
        }
        ui.add_space(8.0);
        ui.label(RichText::new(tr("Drapeau")).color(theme::DIM).size(11.5));
        for (f, name) in [
            (FlagFilter::All, tr("Toutes")),
            (FlagFilter::Picks, tr("Retenues (P)")),
            (FlagFilter::NotRejected, tr("Sans les rejetées (X)")),
        ] {
            ui.radio_value(&mut self.flag_filter, f, name);
        }
        let keywords = self.folder_keywords();
        if !keywords.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new(tr("Mots-clés")).color(theme::DIM).size(11.5));
            if ui
                .selectable_label(self.keyword_filter.is_none(), tr("Tous les mots-clés"))
                .clicked()
            {
                self.keyword_filter = None;
            }
            for (k, n) in keywords {
                let on = self
                    .keyword_filter
                    .as_ref()
                    .is_some_and(|f| fold(f) == fold(&k));
                if ui.selectable_label(on, format!("{k} ({n})")).clicked() {
                    self.keyword_filter = if on { None } else { Some(k) };
                }
            }
        }
        ui.add_space(14.0);
        section_title(ui, tr("Raccourcis"));
        for line in [
            tr("0 à 5 : note"),
            tr("P, X, U : retenir, rejeter, sans drapeau"),
            tr("Flèches gauche et droite : photo voisine"),
            tr("D ou double-clic : Développement"),
        ] {
            ui.label(RichText::new(line).size(11.5).color(theme::DIM));
        }
    }

    /// La photothèque : la recherche, toutes les photos, les collections et les dossiers
    /// du catalogue, chacun avec son nombre de photos.
    fn watch_panel(&mut self, ui: &mut Ui) {
        section_title(ui, tr("Dossier surveillé"));
        let mut stop = false;
        let mut follow = self.watch_follow;
        if let Some((dir, n)) = self.watching().map(|(d, n)| (d.to_path_buf(), n)) {
            let name = dir.file_name().map_or_else(
                || dir.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            ui.label(RichText::new(name).size(12.0))
                .on_hover_text(dir.display().to_string());
            ui.label(
                RichText::new(if n == 1 {
                    tr("1 nouvelle photo").to_owned()
                } else {
                    trf("{n} nouvelles photos", &[("n", &n)])
                })
                .size(11.5)
                .color(theme::DIM),
            );
            ui.checkbox(&mut follow, tr("Ouvrir chaque nouvelle photo"));
            // Enregistré sous son nom propre, affiché traduit.
            let names: Vec<(String, String)> = (0..self.film_count())
                .map(|k| (self.film_key(k).to_owned(), self.film_name(k).to_owned()))
                .collect();
            let mut chosen = self.defaults.import_preset.clone();
            let shown = chosen
                .as_ref()
                .and_then(|c| names.iter().find(|(key, _)| key == c))
                .map_or_else(|| tr("Aucun film").to_owned(), |(_, name)| name.clone());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tr("À l'arrivée"))
                        .size(11.5)
                        .color(theme::DIM),
                );
                egui::ComboBox::from_id_salt("film-import")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut chosen, None, tr("Aucun film"));
                        for (key, name) in &names {
                            ui.selectable_value(&mut chosen, Some(key.clone()), name);
                        }
                    });
            });
            if chosen != self.defaults.import_preset {
                self.set_import_preset(chosen);
            }
            let dir = dir.to_path_buf();
            self.tether_panel(ui, &dir);
            stop = ui.button(tr("Arrêter la surveillance")).clicked();
        } else {
            ui.label(
                RichText::new(tr(
                    "Les photos qui arrivent dans le dossier (boîtier relié, carte copiée) s'affichent aussitôt",
                ))
                .size(11.0)
                .color(theme::DIM),
            );
            let current = (self.view == View::Folder)
                .then(|| self.folder.first().and_then(|p| p.parent()))
                .flatten()
                .map(Path::to_path_buf);
            ui.horizontal_wrapped(|ui| {
                if let Some(dir) = current
                    && ui.button(tr("Surveiller ce dossier")).clicked()
                {
                    self.watch_folder(&dir);
                }
                if ui.button(tr("Choisir un dossier…")).clicked()
                    && let Some(dir) = crate::dialogs::pick_folder(tr("Dossier à surveiller"))
                {
                    self.watch_folder(&dir);
                }
            });
        }
        self.watch_follow = follow;
        if stop {
            self.stop_watching();
        }
    }

    /// Sur Mac : les boîtiers reliés, dont les photos arrivent dans le dossier surveillé.
    fn tether_panel(&mut self, ui: &mut Ui, dir: &Path) {
        use luma_apple::tether;
        if !tether::AVAILABLE {
            return;
        }
        let status = tether::status();
        if !status.running {
            if ui
                .button(tr("Relier un boîtier"))
                .on_hover_text(tr(
                    "Les photos prises boîtier relié (USB) arrivent dans ce dossier",
                ))
                .clicked()
            {
                tether::start(dir.to_path_buf());
            }
            return;
        }
        let text = if status.cameras.is_empty() {
            tr("Branchez le boîtier et allumez-le").to_owned()
        } else {
            trf(
                "Boîtier relié : {name}",
                &[("name", &status.cameras.join(", "))],
            )
        };
        ui.label(RichText::new(text).size(11.5).color(theme::DIM));
        ui.horizontal_wrapped(|ui| {
            if status.can_trigger && ui.button(tr("Déclencher")).clicked() {
                tether::trigger();
            }
            if ui.button(tr("Déconnecter le boîtier")).clicked() {
                tether::stop();
            }
        });
    }

    fn catalog_panel(&mut self, ui: &mut Ui) {
        section_title(ui, tr("Photothèque"));
        if crate::dialogs::FOLDER_PICKER
            && ui
                .button(tr("Importer une carte…"))
                .on_hover_text(tr(
                    "Copie les photos d'une carte mémoire dans la photothèque, rangées par date, et sur un second disque",
                ))
                .clicked()
        {
            self.open_import();
        }
        let edit = ui.add(
            egui::TextEdit::singleline(&mut self.catalog_search)
                .hint_text(tr("Rechercher : mer note:3 date:2024 iso:..800"))
                .desired_width(f32::INFINITY),
        );
        if edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            let text = self.catalog_search.trim().to_owned();
            self.show_view(if text.is_empty() {
                View::All
            } else {
                View::Search(text)
            });
        }
        let mut chosen = None;
        let all = trf("Toutes les photos ({n})", &[("n", &self.catalog.len())]);
        if ui.selectable_label(self.view == View::All, all).clicked() {
            chosen = Some(View::All);
        }
        if !self.catalog_collections.is_empty() {
            ui.label(
                RichText::new(tr("Collections"))
                    .color(theme::DIM)
                    .size(11.5),
            );
            for (name, n) in &self.catalog_collections {
                let on = self.view == View::Collection(name.clone());
                if ui.selectable_label(on, format!("{name} ({n})")).clicked() {
                    chosen = Some(View::Collection(name.clone()));
                }
            }
        }
        let mut open = None;
        if !self.catalog_folders.is_empty() {
            ui.label(RichText::new(tr("Dossiers")).color(theme::DIM).size(11.5));
            let current = self
                .folder
                .first()
                .and_then(|p| p.parent())
                .map(Path::to_path_buf);
            for (dir, n) in &self.catalog_folders {
                let on = self.view == View::Folder && current.as_ref() == Some(dir);
                let name = dir.file_name().map_or_else(
                    || dir.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                if ui
                    .selectable_label(on, format!("{name} ({n})"))
                    .on_hover_text(dir.display().to_string())
                    .clicked()
                {
                    open = Some(dir.clone());
                }
            }
        }
        if self.is_indexing() {
            ui.label(
                RichText::new(tr("Mise à jour de la photothèque…"))
                    .color(theme::DIM)
                    .size(11.0),
            );
        }
        if let Some(v) = chosen {
            self.show_view(v);
        } else if let Some(dir) = open {
            self.open_path(&dir);
        }
    }

    /// L'histogramme, ou l'instrument choisi à sa place (forme d'onde, parade RVB,
    /// vectorscope), avec le sélecteur au‑dessus (Maj+J : le suivant).
    fn scopes(&mut self, ui: &mut Ui, height: f32) {
        // Sur iPhone, la place manque : l'histogramme seul, plus bas.
        if self.compact {
            widgets::histogram_sized(ui, &self.histogram, &mut self.clipping, 52.0)
                .on_hover_text(tr("Clic ou J : afficher l'écrêtage"));
            return;
        }
        let names: Vec<&str> = Scope::ALL
            .iter()
            .map(|s| match s {
                Scope::Histogram => tr("Histogramme"),
                Scope::Waveform => tr("Forme d'onde"),
                Scope::Parade => tr("Parade RVB"),
                Scope::Vectorscope => tr("Vectorscope"),
            })
            .collect();
        let on: Vec<bool> = Scope::ALL.iter().map(|&s| s == self.scope).collect();
        if let Some(k) = widgets::segments(
            ui,
            &names,
            &on,
            &[tr("Maj+J : instrument suivant"); 4],
            10.5,
        ) && Scope::ALL[k] != self.scope
        {
            self.scope = Scope::ALL[k];
            self.scope_texture = None;
        }
        ui.add_space(2.0);
        match (&self.scope_texture, self.scope) {
            (Some(texture), scope) if scope != Scope::Histogram => {
                widgets::scope(ui, scope, texture, height);
            }
            _ => {
                widgets::histogram_sized(ui, &self.histogram, &mut self.clipping, height)
                    .on_hover_text(tr("Clic ou J : afficher l'écrêtage"));
            }
        }
        // La mesure au point : sous le pointeur, en RVB (0 à 255, sRGB) et en Lab.
        let measured = self
            .probe
            .and_then(|[x, y]| probe_color(&self.probe_image, x, y));
        ui.add_space(2.0);
        let line = match measured {
            Some(rgb) => {
                let [l, a, b] = luma_color::profile::srgb8_to_lab(rgb);
                format!(
                    "{} {:>3}  {} {:>3}  {} {:>3}   L {:.0}  a {:.0}  b {:.0}",
                    tr("R|mesure"),
                    rgb[0],
                    tr("V|mesure"),
                    rgb[1],
                    tr("B|mesure"),
                    rgb[2],
                    l,
                    a,
                    b
                )
            }
            // Au doigt, rien ne survole la photo : pas de mesure à proposer.
            None if crate::touch() => return,
            None => tr("Survolez la photo : RVB et Lab du point").to_owned(),
        };
        ui.label(RichText::new(line).size(10.5).monospace().color(theme::DIM));
    }

    /// Bibliothèque, à droite : la note, le drapeau et les informations de la photo.
    fn library_right(&mut self, ui: &mut Ui) {
        self.scopes(ui, 96.0);
        ui.add_space(10.0);
        let Some(path) = self.current_path().map(Path::to_path_buf) else {
            return;
        };
        section_title(ui, tr("Métadonnées"));
        ui.label(RichText::new(file_name(&path)).strong());
        // À la ligne quand la place manque (iPhone, panneau étroit).
        ui.horizontal_wrapped(|ui| {
            if ui
                .small_button(tr("Copie virtuelle"))
                .on_hover_text(tr(
                    "Un autre développement de cette photo, sans dupliquer le fichier",
                ))
                .clicked()
            {
                self.create_copy();
            }
            if luma_project::copies::number(&path).is_some()
                && ui
                    .small_button(tr("Supprimer la copie"))
                    .on_hover_text(tr("Supprime ses réglages ; l'original ne change pas"))
                    .clicked()
            {
                self.delete_copy();
            }
            if ui
                .small_button(tr("Renommer…"))
                .on_hover_text(tr("Renommer les photos choisies d'après un modèle"))
                .clicked()
            {
                self.rename_open = true;
            }
            if ui
                .small_button(tr("Imprimer…"))
                .on_hover_text(tr("Mise en page pour l'imprimante : une photo par page ou une planche contact, en PDF"))
                .clicked()
            {
                self.print_open = true;
            }
        });
        let meta = self.current_meta();
        if let Some(n) = widgets::rating(ui, meta.rating, 20.0) {
            self.act(["rate0", "rate1", "rate2", "rate3", "rate4", "rate5"][n as usize]);
        }
        ui.horizontal(|ui| {
            for (f, name, action) in [
                (Flag::Pick, tr("Retenue"), "pick"),
                (Flag::None, tr("Sans drapeau"), "unflag"),
                (Flag::Reject, tr("Rejetée"), "reject"),
            ] {
                if ui.selectable_label(meta.flag == f, name).clicked() {
                    self.act(action);
                }
            }
        });
        self.selection_actions(ui);
        self.stack_panel(ui, &path);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Libellé")).color(theme::DIM).size(11.5));
            if let Some(n) = label_dots(ui, &meta.label) {
                self.act(["label0", "label1", "label2", "label3", "label4"][n]);
            }
        });
        ui.add_space(8.0);
        self.keywords_editor(ui);
        self.info_editor(ui);
        ui.add_space(8.0);
        if let Some(photo) = &self.photo {
            let raw = photo.raw();
            let e = &raw.exif;
            let (w, h) = photo.full_size();
            let mut rows = vec![
                (tr("Appareil"), format!("{} {}", raw.make, raw.model)),
                (tr("Dimensions"), format!("{w} × {h}")),
            ];
            if let Some(iso) = e.iso {
                rows.push(("ISO", iso.to_string()));
            }
            if let Some(f) = e.focal_length {
                rows.push((tr("Focale"), format!("{f:.0} mm")));
            }
            if let Some(a) = e.aperture {
                rows.push((tr("Ouverture"), format!("f/{}", format_value(a, 1, false))));
            }
            if let Some(t) = e.exposure_time {
                rows.push((
                    tr("Vitesse"),
                    if t < 1.0 {
                        format!("1/{:.0} s", 1.0 / t)
                    } else {
                        format!("{t:.1} s")
                    },
                ));
            }
            egui::Grid::new("exif").num_columns(2).show(ui, |ui| {
                for (k, v) in rows {
                    ui.label(RichText::new(k).color(theme::DIM));

                    ui.label(v);
                    ui.end_row();
                }
            });
        }
    }

    /// Les mots‑clés des photos visées (la sélection, sinon la photo affichée), × pour en
    /// retirer un, et la saisie, qui les ajoute à toutes.
    /// Change une information (titre, légende, auteur, droits, lieu) des photos visées.
    pub fn set_info(&mut self, field: &str, value: &str) {
        let Some(&(_, _, _, get)) = INFO_FIELDS.iter().find(|f| f.0 == field) else {
            return;
        };
        let targets = self.keyword_targets();
        let value = value.trim().to_owned();
        self.edit_meta(&targets, |m| *get(m) = value.clone());
    }

    /// Le fichier des modèles de métadonnées (les essais en prennent un à eux).
    pub fn use_templates_file(&mut self, path: PathBuf) {
        self.templates = load_templates(&path);
        self.templates_path = Some(path);
    }

    /// Les modèles de métadonnées, par nom.
    pub fn templates(&self) -> &[luma_project::xmp::Template] {
        &self.templates
    }

    /// Enregistre les informations de la photo ouverte comme modèle (un modèle du même
    /// nom est remplacé).
    pub fn save_template(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let t = luma_project::xmp::Template::of(name, &self.current_meta());
        self.templates.retain(|o| o.name != name);
        self.templates.push(t);
        self.templates.sort_by(|a, b| a.name.cmp(&b.name));
        let path = self
            .templates_path
            .clone()
            .or_else(|| crate::dialogs::presets_dir().map(|d| d.join(TEMPLATES_FILE)));
        if let Some(path) = path {
            if let Err(e) = luma_project::xmp::save_templates(&path, &self.templates) {
                self.status = trf("Modèle non enregistré : {error}", &[("error", &e)]);
            } else {
                self.status = trf("Modèle « {name} » enregistré", &[("name", &name)]);
            }
            self.templates_path = Some(path);
        }
    }

    /// Donne un modèle aux photos visées (la sélection, ou la photo ouverte).
    pub fn apply_template(&mut self, name: &str) {
        let Some(t) = self.templates.iter().find(|t| t.name == name).cloned() else {
            return;
        };
        let targets = self.keyword_targets();
        self.edit_meta(&targets, |m| t.apply(m));
        self.status = trf(
            "Modèle « {name} » appliqué à {n} photo(s)",
            &[("name", &name), ("n", &targets.len())],
        );
    }

    /// Les informations de la photo (IPTC) : titre, légende, auteur, droits, lieu ; et les
    /// modèles. Plusieurs photos visées : un champ commun s'affiche, un champ qui
    /// diffère reste vide.
    fn info_editor(&mut self, ui: &mut Ui) {
        let targets = self.keyword_targets();
        if targets.is_empty() {
            return;
        }
        let metas: Vec<Meta> = targets
            .iter()
            .map(|p| self.meta.get(p).cloned().unwrap_or_default())
            .collect();
        let common = |get: fn(&mut Meta) -> &mut String| -> (String, bool) {
            let mut values = metas.iter().cloned().map(|mut m| get(&mut m).clone());
            let first = values.next().unwrap_or_default();
            let same = values.all(|v| v == first);
            (if same { first } else { String::new() }, same)
        };
        if self.info_for != targets {
            self.info_for = targets.clone();
            self.info_edit.clear();
        }
        let mut changes: Vec<(&'static str, String)> = Vec::new();
        let mut apply = None;
        let mut save = false;
        egui::CollapsingHeader::new(RichText::new(tr("Informations")).strong())
            .id_salt("Informations IPTC")
            .default_open(false)
            .show(ui, |ui| {
                for &(key, label, multiline, get) in INFO_FIELDS {
                    let (value, same) = common(get);
                    let buffer = self.info_edit.entry(key).or_insert_with(|| value.clone());
                    ui.label(RichText::new(tr(label)).size(11.0).color(theme::DIM));
                    let edit = if multiline {
                        egui::TextEdit::multiline(buffer).desired_rows(2)
                    } else {
                        egui::TextEdit::singleline(buffer)
                    };
                    let hint = if same { "" } else { tr("Plusieurs valeurs") };
                    let r = ui.add(edit.hint_text(hint).desired_width(f32::INFINITY));
                    if r.lost_focus() && *buffer != value {
                        changes.push((key, buffer.clone()));
                    }
                }
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if !self.templates.is_empty() {
                        ui.menu_button(tr("Appliquer un modèle"), |ui| {
                            for t in &self.templates {
                                if ui.button(&t.name).clicked() {
                                    apply = Some(t.name.clone());
                                    ui.close();
                                }
                            }
                        });
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.template_name)
                            .hint_text(tr("Nom du modèle"))
                            .desired_width(110.0),
                    );
                    save = ui
                        .add_enabled(
                            !self.template_name.trim().is_empty(),
                            egui::Button::new(tr("Enregistrer comme modèle")),
                        )
                        .on_hover_text(tr(
                            "L'auteur, les droits, le lieu et les mots-clés de la photo ouverte",
                        ))
                        .clicked();
                });
            });
        for (key, value) in changes {
            self.set_info(key, &value);
        }
        if let Some(name) = apply {
            self.apply_template(&name);
            self.info_edit.clear();
        }
        if save {
            let name = std::mem::take(&mut self.template_name);
            self.save_template(&name);
        }
    }

    fn keywords_editor(&mut self, ui: &mut Ui) {
        ui.label(RichText::new(tr("Mots-clés")).color(theme::DIM).size(11.5));
        let mut removed = None;
        ui.horizontal_wrapped(|ui| {
            for k in self.target_keywords() {
                if ui
                    .small_button(format!("{}  ×", k.replace('|', " > ")))
                    .on_hover_text(tr("Retirer ce mot-clé"))
                    .clicked()
                {
                    removed = Some(k);
                }
            }
        });
        if let Some(k) = removed {
            self.remove_keyword(&k);
        }
        let hint = if self.selected.len() > 1 {
            trf(
                "Ajouter aux {n} photos choisies",
                &[("n", &self.selected.len())],
            )
        } else {
            tr("Ajouter un mot-clé (virgule entre deux)").to_owned()
        };
        let edit = ui.add(
            egui::TextEdit::singleline(&mut self.keyword_input)
                .hint_text(hint)
                .desired_width(f32::INFINITY),
        );
        if edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            let text = std::mem::take(&mut self.keyword_input);
            self.add_keywords(&text);
            edit.request_focus();
        }
    }

    /// Bibliothèque, au centre : la grille des vignettes.
    /// La carte : le fond de tuiles libres, les photos posées à l'endroit de leur prise
    /// de vue, regroupées quand elles sont trop proches. Glisser pour se déplacer, la
    /// molette ou deux doigts pour zoomer, toucher un groupe pour voir ses photos.
    fn map(&mut self, ui: &mut Ui) {
        use luma_library::map::{self, View};
        let rect = ui.max_rect();
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        let placed = self.placed();
        let unplaced = self.unplaced();
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, Color32::from_gray(0x1a));
        if placed.is_empty() && unplaced.is_empty() {
            self.map_groups.clear();
            centered_text(
                &painter,
                rect.center() - vec2(0.0, 20.0),
                rect.width() - 40.0,
                tr("Aucune photo de ce film n'a de position GPS.").into(),
            );
            return;
        }
        let mut view = self
            .map
            .flatten()
            .unwrap_or_else(|| View::fit(placed.iter().map(|p| p.1), size));
        let ctx = ui.ctx().clone();
        let response = ui.interact(
            rect,
            ui.scope_id().with("carte"),
            egui::Sense::click_and_drag(),
        );
        let local = |p: egui::Pos2| [f64::from(p.x - rect.min.x), f64::from(p.y - rect.min.y)];
        if response.dragged() {
            let d = response.drag_delta();
            view.pan([f64::from(d.x), f64::from(d.y)]);
        }
        if response.hovered() || response.dragged() {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = f64::from(pinch) * (f64::from(scroll) / 240.0).exp2();
            if (factor - 1.0).abs() > 1e-4 {
                let at = ui
                    .input(|i| i.pointer.hover_pos())
                    .map_or([size[0] / 2.0, size[1] / 2.0], local);
                view.zoom_at(factor, at, size);
            }
        }
        if response.double_clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            view.zoom_at(2.0, local(p), size);
        }

        // Le fond : les tuiles prêtes, un quadrillage sous celles qui manquent.
        self.tiles.receive(&ctx);
        let to_rect = |at: [f64; 2], side: f64| {
            Rect::from_min_size(
                rect.min + vec2(at[0] as f32, at[1] as f32),
                vec2(side as f32, side as f32),
            )
        };
        for (tile, at, side) in view.tiles(size) {
            let r = to_rect(at, side);
            match self.tiles.get(&ctx, tile) {
                Some(t) => {
                    painter.image(
                        t.id(),
                        r,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        // Un peu assombri : la carte reste derrière les photos.
                        Color32::from_gray(0xd8),
                    );
                }
                None => {
                    painter.rect_stroke(
                        r,
                        0.0,
                        egui::Stroke::new(1.0, Color32::from_gray(0x2c)),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }

        // Les photos, en groupes : la vignette de la première et leur nombre.
        let points: Vec<[f64; 2]> = placed.iter().map(|p| p.1).collect();
        let groups = map::clusters(&points, &view, size, 56.0);
        let mut chosen = None;
        self.map_groups.clear();
        let click = response
            .clicked()
            .then(|| response.interact_pointer_pos())
            .flatten();
        for group in &groups {
            let center = rect.min + vec2(group.at[0] as f32, group.at[1] as f32);
            let r = Rect::from_center_size(center, vec2(44.0, 44.0));
            painter.rect_filled(r.expand(2.0), 5.0, Color32::WHITE);
            let first = &self.folder[placed[group.members[0]].0];
            match self.thumbs.get(first) {
                Some(t) => {
                    let [w, h] = t.size();
                    // Le carré du milieu de la vignette.
                    let (w, h) = (w as f32, h as f32);
                    let side = w.min(h);
                    let uv = Rect::from_center_size(pos2(0.5, 0.5), vec2(side / w, side / h));
                    painter.image(t.id(), r, uv, Color32::WHITE);
                }
                None => {
                    painter.rect_filled(r, 3.0, Color32::from_gray(0x44));
                }
            }
            let n = group.members.len();
            self.map_groups.push((center, n));
            if n > 1 {
                let text = n.to_string();
                let galley =
                    painter.layout_no_wrap(text, egui::FontId::proportional(11.0), Color32::WHITE);
                let chip = Rect::from_min_size(
                    pos2(r.max.x - galley.size().x / 2.0 - 4.0, r.min.y - 10.0),
                    galley.size(),
                )
                .expand2(vec2(5.0, 1.5));
                painter.rect_filled(chip, 8.0, theme::ACCENT);
                painter.galley(chip.min + vec2(5.0, 1.5), galley, Color32::WHITE);
            }
            if let Some(p) = click
                && r.expand(6.0).contains(p)
            {
                chosen = Some(group.members.clone());
            }
        }
        // Le crédit des tuiles libres, lisible sur tous les fonds.
        let credit = painter.layout_no_wrap(
            map::ATTRIBUTION.to_owned(),
            egui::FontId::proportional(10.5),
            Color32::from_gray(0xe0),
        );
        let chip = Rect::from_min_size(rect.max - credit.size() - vec2(10.0, 7.0), credit.size())
            .expand2(vec2(5.0, 2.0));
        painter.rect_filled(chip, 4.0, Color32::from_black_alpha(150));
        painter.galley(chip.min + vec2(5.0, 2.0), credit, Color32::from_gray(0xe0));
        // Des photos sans position (celle affichée, ou les photos choisies) : un viseur au
        // centre, la carte glisse dessous, un bouton les place là.
        let mut place_at = None;
        if let Some(path) = unplaced.first() {
            let c = rect.center();
            let stroke = egui::Stroke::new(2.0, theme::ACCENT);
            painter.circle_stroke(
                c,
                14.0,
                egui::Stroke::new(4.0, Color32::from_black_alpha(120)),
            );
            painter.circle_stroke(c, 14.0, stroke);
            for d in [vec2(1.0, 0.0), vec2(0.0, 1.0)] {
                painter.line_segment([c - d * 24.0, c - d * 8.0], stroke);
                painter.line_segment([c + d * 8.0, c + d * 24.0], stroke);
            }
            let text = match unplaced.len() {
                1 => trf("Placer « {name} » ici", &[("name", &file_name(path))]),
                n => trf("Placer les {n} photos ici", &[("n", &n)]),
            };
            let button = egui::Button::new(RichText::new(text).size(14.0).color(theme::BACKGROUND))
                .fill(theme::ACCENT)
                .corner_radius(18.0);
            let w = (rect.width() - 40.0).min(420.0);
            let place = Rect::from_center_size(pos2(c.x, rect.max.y - 56.0), vec2(w, 36.0));
            if ui
                .put(place, button)
                .on_hover_text(tr(
                    "Glisser la carte pour mettre le viseur à l'endroit de la prise de vue",
                ))
                .clicked()
            {
                place_at = Some(map::unproject(view.center));
            }
        }
        self.map = Some(Some(view));
        if let Some(at) = place_at {
            self.place_unplaced_at(at);
        }
        if let Some(members) = chosen {
            let photos = members
                .into_iter()
                .map(|k| self.folder[placed[k].0].clone())
                .collect();
            self.show_place(photos);
        }
    }

    fn grid(&mut self, ui: &mut Ui) {
        let visible = self.visible();
        if visible.is_empty() {
            let rect = ui.max_rect();
            let text = if self.folder.is_empty() {
                self.status_text()
            } else {
                tr("Aucune photo ne passe le filtre.").into()
            };
            let shown = centered_text(
                ui.painter(),
                rect.center() - vec2(0.0, 20.0),
                rect.width() - 40.0,
                text,
            );
            if self.folder.is_empty() {
                self.import_button(ui, pos2(rect.center().x, shown.max.y + 20.0));
            }
            return;
        }
        // Les marges des îlots, pour que la grille tombe dans leur alignement ; deux
        // colonnes au moins sur un écran étroit (iPhone en portrait).
        let (pad, gap) = if self.compact {
            (8.0, 8.0)
        } else {
            (14.0, 10.0)
        };
        let width = ui.available_width() - 2.0 * pad;
        let target = if self.compact { 160.0 } else { 200.0 };
        let cols = ((width + gap) / (target + gap)).floor().max(2.0) as usize;
        let cell = ((width - gap * (cols - 1) as f32) / cols as f32).max(60.0);
        let mut clicked = None;
        let mut open = None;
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .id_salt("grille")
            .auto_shrink(false)
            .show(ui, |ui| {
                // Tout est placé au point près : l'espacement des éléments (plus grand au
                // doigt) ne s'ajoute pas aux marges.
                ui.spacing_mut().item_spacing = vec2(0.0, gap);
                ui.add_space(gap);
                // Sur iPhone et iPad, ajouter des photos se fait d'ici aussi.
                if luma_apple::photos::AVAILABLE {
                    let (row, _) = ui.allocate_exact_size(
                        vec2(ui.available_width(), 44.0),
                        egui::Sense::hover(),
                    );
                    let place = Rect::from_min_size(row.min + vec2(pad, 0.0), vec2(width, 44.0));
                    let button = egui::Button::new(
                        RichText::new(format!("+  {}", tr("Importer des photos")))
                            .size(15.0)
                            .color(theme::BACKGROUND),
                    )
                    .fill(theme::ACCENT)
                    .corner_radius(22.0);
                    if ui.put(place, button).clicked() {
                        self.act("open");
                    }
                }
                for row in visible.chunks(cols) {
                    let (line, _) = ui.allocate_exact_size(
                        vec2(ui.available_width(), cell),
                        egui::Sense::hover(),
                    );
                    {
                        for (k, &i) in row.iter().enumerate() {
                            let path = &self.folder[i];
                            let rect = Rect::from_min_size(
                                line.min + vec2(pad + k as f32 * (cell + gap), 0.0),
                                vec2(cell, cell),
                            );
                            let r = ui.interact(
                                rect,
                                ui.scope_id().with(("case", i)),
                                egui::Sense::click(),
                            );
                            let current = Some(i) == self.current;
                            let painter = ui.painter();
                            painter.rect_filled(
                                rect,
                                4.0,
                                if current {
                                    Color32::from_gray(0x4a)
                                } else {
                                    theme::SECTION
                                },
                            );
                            // Le nom en haut, les étoiles et le drapeau en bas, la photo entre
                            // les deux : rien ne se chevauche.
                            let area = Rect::from_min_max(
                                rect.min + vec2(8.0, 24.0),
                                rect.max - vec2(8.0, 26.0),
                            );
                            match self.thumbs.get(path) {
                                Some(t) => {
                                    let [w, h] = t.size();
                                    painter.image(
                                        t.id(),
                                        fit(area, w, h),
                                        egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                        Color32::WHITE,
                                    );
                                }
                                None => {
                                    painter.rect_filled(area, 2.0, Color32::from_gray(0x22));
                                    if self.evicted.contains(path) {
                                        self.shown_missing.borrow_mut().push(path.clone());
                                        ui.ctx().request_repaint();
                                    }
                                }
                            }
                            let meta = self.meta.get(path).cloned().unwrap_or_default();
                            if let Some(e) = self.edits.get(path) {
                                let photo = match self.thumbs.get(path) {
                                    Some(t) => fit(area, t.size()[0], t.size()[1]),
                                    None => area,
                                };
                                paint_edits(painter, photo, e, true);
                            }
                            paint_meta(painter, rect, &meta);
                            if self.stacking
                                && let Some(&k) = self.stack_of.get(path)
                                && self.stacks[k].photos[0] == *path
                            {
                                let stack = &self.stacks[k];
                                let text = match stack.kind {
                                    luma_library::stacks::Kind::Burst => {
                                        trf("Rafale ×{n}", &[("n", &stack.photos.len())])
                                    }
                                    luma_library::stacks::Kind::Bracket => {
                                        trf("Série ×{n}", &[("n", &stack.photos.len())])
                                    }
                                };
                                // Ouverte, la pastille prend le liseré de l'accent.
                                let open = self.unfolded.contains(path);
                                let galley = painter.layout_no_wrap(
                                    text,
                                    egui::FontId::proportional(10.5),
                                    theme::TEXT,
                                );
                                let chip = Rect::from_min_size(
                                    pos2(rect.max.x - 14.0 - galley.size().x, rect.min.y + 30.0),
                                    galley.size(),
                                )
                                .expand2(vec2(5.0, 2.0));
                                painter.rect_filled(chip, 6.0, Color32::from_black_alpha(160));
                                if open {
                                    painter.rect_stroke(
                                        chip,
                                        6.0,
                                        egui::Stroke::new(1.0, theme::ACCENT),
                                        egui::StrokeKind::Outside,
                                    );
                                }
                                painter.galley(chip.min + vec2(5.0, 2.0), galley, theme::TEXT);
                            }
                            // Le nom, coupé avant la pastille du libellé.
                            painter
                                .with_clip_rect(rect.shrink(4.0).with_max_x(rect.max.x - 20.0))
                                .text(
                                    pos2(rect.min.x + 10.0, rect.min.y + 12.0),
                                    egui::Align2::LEFT_CENTER,
                                    file_name(path),
                                    egui::FontId::proportional(10.5),
                                    theme::DIM,
                                );
                            if meta.flag == Flag::Reject {
                                painter.rect_filled(rect, 4.0, Color32::from_black_alpha(120));
                            }
                            if self.selected.contains(&i) {
                                painter.rect_stroke(
                                    rect,
                                    4.0,
                                    egui::Stroke::new(2.0, Color32::from_rgb(0x8b, 0x7b, 0xff)),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            if crate::touch() && r.long_touched() {
                                chosen = Some(i);
                            } else if r.double_clicked() || (crate::touch() && r.clicked()) {
                                open = Some(i);
                            } else if r.clicked() {
                                clicked = Some(i);
                            }
                        }
                    }
                }
                ui.add_space(gap);
            });
        if let Some(i) = chosen {
            // Toucher longuement : ajouter à la sélection (ou en retirer), comme ⌘‑clic.
            if !self.selected.remove(&i) {
                self.selected.insert(i);
                if let Some(c) = self.current {
                    self.selected.insert(c);
                }
            }
        } else if let Some(i) = open {
            self.selected.clear();
            self.select(i);
            self.set_tool(Tool::Light);
        } else if let Some(i) = clicked {
            self.click_photo(i, ui);
        }
    }

    fn filmstrip(&mut self, ui: &mut Ui) {
        let mut clicked = None;
        let mut chosen = None;
        let thumb_h = (ui.available_height() - 4.0).clamp(36.0, 84.0);
        egui::ScrollArea::horizontal()
            .id_salt("film")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for i in self.visible() {
                        let path = &self.folder[i];
                        let h = thumb_h;
                        let (w, tex) = match self.thumbs.get(path) {
                            Some(t) => {
                                let s = t.size();
                                (h * s[0] as f32 / s[1].max(1) as f32, Some(t))
                            }
                            None => {
                                if self.evicted.contains(path) {
                                    self.shown_missing.borrow_mut().push(path.clone());
                                    ui.ctx().request_repaint();
                                }
                                (h * 1.5, None)
                            }
                        };
                        let (rect, r) = ui.allocate_exact_size(vec2(w, h), egui::Sense::click());
                        let painter = ui.painter();
                        match tex {
                            Some(t) => {
                                painter.image(
                                    t.id(),
                                    rect,
                                    egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            None => {
                                painter.rect_filled(rect, 2.0, theme::SECTION);
                            }
                        }
                        if let Some(e) = self.edits.get(path) {
                            paint_edits(painter, rect, e, false);
                        }
                        if let Some(m) = self.meta.get(path) {
                            paint_meta(painter, rect.expand2(vec2(0.0, 0.0)), m);
                            if m.flag == Flag::Reject {
                                painter.rect_filled(rect, 2.0, Color32::from_black_alpha(130));
                            }
                        }
                        if self.selected.contains(&i) {
                            painter.rect_stroke(
                                rect.expand(1.0),
                                6.0,
                                egui::Stroke::new(1.5, Color32::from_rgb(0x8b, 0x7b, 0xff)),
                                egui::StrokeKind::Outside,
                            );
                            painter.circle_filled(
                                rect.right_top() + vec2(-9.0, 9.0),
                                6.0,
                                Color32::from_rgb(0x8b, 0x7b, 0xff),
                            );
                        }
                        if Some(i) == self.current {
                            painter.rect_stroke(
                                rect.expand(3.0),
                                7.0,
                                egui::Stroke::new(2.0, theme::ACCENT),
                                egui::StrokeKind::Outside,
                            );
                        }
                        if crate::touch() && r.long_touched() {
                            chosen = Some(i);
                        } else if r.on_hover_text(file_name(path)).clicked() {
                            clicked = Some(i);
                        }
                        ui.add_space(8.0);
                    }
                });
            });
        if let Some(i) = chosen {
            // Toucher longuement : ajouter à la sélection (ou en retirer), comme dans
            // la grille.
            if !self.selected.remove(&i) {
                self.selected.insert(i);
                if let Some(c) = self.current {
                    self.selected.insert(c);
                }
            }
        } else if let Some(i) = clicked {
            self.click_photo(i, ui);
        }
    }

    /// Un clic sur une vignette : ⌘ l'ajoute à la sélection (ou l'en retire), Maj
    /// sélectionne la suite depuis la photo affichée, sinon on l'affiche.
    fn click_photo(&mut self, i: usize, ui: &Ui) {
        let m = ui.input(|i| i.modifiers);
        if m.command {
            if !self.selected.remove(&i) {
                self.selected.insert(i);
                if let Some(c) = self.current {
                    self.selected.insert(c);
                }
            }
        } else if m.shift
            && let Some(c) = self.current
        {
            let visible = self.visible();
            let (a, b) = (c.min(i), c.max(i));
            self.selected
                .extend(visible.into_iter().filter(|v| (a..=b).contains(v)));
        } else {
            self.selected.clear();
            self.select(i);
        }
    }

    /// Sur iPhone et iPad, sans photo : le bouton qui ouvre Photos, sous l'accueil.
    fn import_button(&mut self, ui: &mut Ui, top: egui::Pos2) {
        if !self.photos_button {
            return;
        }
        let size = vec2(240.0, 48.0);
        let rect = Rect::from_min_size(top - vec2(size.x / 2.0, 0.0), size);
        let button = egui::Button::new(
            RichText::new(tr("Importer des photos"))
                .size(17.0)
                .color(theme::BACKGROUND),
        )
        .fill(theme::ACCENT)
        .corner_radius(24.0);
        let r = ui.put(rect, button);
        // Un doigt levé sur le bouton suffit, même si egui n'y a pas vu de clic (un
        // doigt bouge un peu, ou l'appui a duré) : le toucher brut du système compte.
        let lifted = ui.input(|i| {
            i.events.iter().any(|e| match e {
                egui::Event::Touch {
                    phase: egui::TouchPhase::End,
                    pos,
                    ..
                } => rect.expand(12.0).contains(*pos),
                _ => false,
            })
        });
        // L'autre source : l'app Fichiers (un lecteur de carte branché, iCloud Drive).
        let files = ui.put(
            Rect::from_center_size(
                pos2(rect.center().x, rect.max.y + 30.0),
                vec2(size.x + 120.0, 28.0),
            ),
            egui::Button::new(
                RichText::new(tr("Depuis Fichiers (carte mémoire, iCloud Drive)"))
                    .size(14.0)
                    .color(theme::ACCENT),
            )
            .frame(false),
        );
        if files.clicked() {
            self.act("open_files");
        }
        if crate::touch() {
            // Diagnostic, le temps de vérifier l'import sur un vrai iPhone.
            let text = match self.last_touch {
                Some(p) => format!(
                    "touchers reçus : {} · dernier à {:.0}, {:.0}",
                    self.touches, p.x, p.y
                ),
                None => "touchers reçus : 0".to_owned(),
            };
            let text = match &self.import_result {
                Some(result) => format!("{text}\n{result}"),
                None => format!("{text}\nimport pas encore demandé"),
            };
            ui.painter().text(
                pos2(rect.center().x, rect.max.y + 52.0),
                egui::Align2::CENTER_TOP,
                text,
                egui::FontId::proportional(11.0),
                theme::DIM,
            );
        }
        // Pour les essais sur simulateur : où toucher (en points), une fois.
        static SHOWN: std::sync::Once = std::sync::Once::new();
        SHOWN.call_once(|| {
            crate::journal(&format!(
                "bouton importer : centre {:.0} {:.0} (points)",
                rect.center().x,
                rect.center().y
            ))
        });
        if r.clicked() || lifted {
            crate::journal(&format!(
                "bouton importer : touché (clic {}, doigt levé {lifted})",
                r.clicked()
            ));
            self.act("open");
        }
    }

    /// Note les touchers que le système transmet (les 40 premiers dans le journal).
    fn track_touches(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        let ppp = ctx.pixels_per_point();
        ctx.input(|i| {
            for e in &i.events {
                let (what, pos) = match e {
                    egui::Event::Touch { phase, pos, .. } => (format!("toucher {phase:?}"), *pos),
                    egui::Event::PointerButton { pressed, pos, .. } => (
                        format!("pointeur {}", if *pressed { "appuyé" } else { "relâché" }),
                        *pos,
                    ),
                    _ => continue,
                };
                if matches!(
                    e,
                    egui::Event::Touch {
                        phase: egui::TouchPhase::Start,
                        ..
                    }
                ) {
                    self.touches += 1;
                    self.last_touch = Some(pos);
                }
                if self.touches <= 40 {
                    crate::journal(&format!(
                        "{what} à {:.0}, {:.0} (écran {:.0}×{:.0} points, ×{ppp})",
                        pos.x,
                        pos.y,
                        screen.width(),
                        screen.height()
                    ));
                }
            }
        });
    }

    fn viewer(&mut self, ui: &mut Ui) {
        let rect = ui.max_rect();
        let ppp = ui.ctx().pixels_per_point();
        // Pincer (iPad, pavé tactile) : écarter pour le 1:1, rapprocher pour l'image entière.
        if self.crop_tool.is_none() && ui.rect_contains_pointer(rect) {
            let pinch = ui.input(|i| i.zoom_delta());
            if pinch > 1.02 {
                self.zoom = true;
            } else if pinch < 0.98 {
                self.zoom = false;
            }
        }
        let pressed = ui.input(|i| i.pointer.any_down());
        self.request_render(rect.width().max(rect.height()) * ppp, pressed);
        self.probe = None;
        let painter = ui.painter_at(rect);
        let Some(t) = &self.texture else {
            // L'accueil : le logo, et ce qu'on peut faire.
            let side = (rect.width().min(rect.height()) * 0.5).clamp(120.0, 420.0);
            let logo =
                Rect::from_center_size(rect.center() - vec2(0.0, side * 0.12), vec2(side, side));
            if let Some(b) = &self.brand {
                painter.image(
                    b.logo.id(),
                    logo,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            let text = centered_text(
                &painter,
                pos2(rect.center().x, logo.max.y + 18.0),
                rect.width() - 40.0,
                self.status_text(),
            );
            self.import_button(ui, pos2(rect.center().x, text.max.y + 20.0));
            return;
        };
        let [iw, ih] = self.image_size;
        let uv = egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        if self.zoom && matches!(self.last_request.as_ref().map(|r| r.size), Some(Size::Full)) {
            // 1:1 : un pixel de l'image par pixel de l'écran, on se déplace en glissant.
            let size = vec2(iw as f32, ih as f32) / ppp;
            egui::ScrollArea::both().id_salt("zoom").show(ui, |ui| {
                let (r, resp) = ui.allocate_exact_size(size, egui::Sense::click());
                ui.painter().image(t.id(), r, uv, Color32::WHITE);
                if resp.clicked() {
                    self.zoom = false;
                }
            });
        } else if let Some(tool) = &self.crop_tool {
            let target = fit(rect.shrink(36.0), iw, ih);
            painter.image(t.id(), target, uv, Color32::WHITE);
            let aspect = crop::ASPECTS[tool.aspect].1;
            // Le rendu peut avoir un temps de retard : le cadre suit l'image affichée.
            let (guide, turn) = (self.crop_guide, self.crop_turn);
            let c = crop::edit(ui, target, &mut self.settings.crop, aspect, guide, turn);
            if c.changed {
                self.touch();
            }
        } else if self.guiding && self.tool == Tool::Geometry {
            let target = fit(rect.shrink(36.0), iw, ih);
            painter.image(t.id(), target, uv, Color32::WHITE);
            if let Some(label) = self.guide_lines(ui, target, iw, ih) {
                self.commit(label);
            }
        } else if let Some(i) = self
            .selected_mask
            .filter(|i| *i < self.settings.masks.len() && self.tool == Tool::Masks)
        {
            let target = fit(rect.shrink(18.0), iw, ih);
            painter.image(t.id(), target, uv, Color32::WHITE);
            self.edit_label(&painter, target);
            if self.show_mask {
                let mask = &self.settings.masks[i];
                let stale = self.mask_overlay.as_ref().is_none_or(|(m, _)| m != mask);
                if stale {
                    let (w, h) = luma_core::RgbImage::fit(iw, ih, 400);
                    let image = masking::overlay(mask, w, h, self.scene_map.as_deref());
                    match &mut self.mask_overlay {
                        Some((m, tex)) => {
                            tex.set(image, TextureOptions::LINEAR);
                            *m = mask.clone();
                        }
                        None => {
                            let tex =
                                ui.ctx()
                                    .load_texture("masque", image, TextureOptions::LINEAR);
                            self.mask_overlay = Some((mask.clone(), tex));
                        }
                    }
                }
                if let Some((_, tex)) = &self.mask_overlay {
                    if let Some(black) = self.mask_style.backdrop() {
                        painter.rect_filled(target, 0.0, black);
                    }
                    painter.image(tex.id(), target, uv, self.mask_style.tint());
                }
            }
            let brush = self.brush;
            // Une partie combinée choisie : ses poignées à la place de la forme principale.
            let part = self
                .selected_part
                .filter(|&(m, p)| m == i && p < self.settings.masks[i].parts.len())
                .map(|(_, p)| p);
            let c = match part {
                Some(j) => {
                    let mut edited = Mask::full();
                    let shape = &mut self.settings.masks[i].parts[j].shape;
                    std::mem::swap(&mut edited.shape, shape);
                    let c = masking::edit(ui, target, &mut edited, &brush);
                    std::mem::swap(&mut edited.shape, shape);
                    c
                }
                None => masking::edit(ui, target, &mut self.settings.masks[i], &brush),
            };
            if c.changed {
                self.touch();
            }
            if c.committed {
                let name = tr(&self.settings.masks[i].name).to_lowercase();
                self.commit(trf("Masque : {mask}", &[("mask", &name)]));
            }
        } else if let Some(kind) = self.liquify.filter(|_| self.tool == Tool::Masks) {
            let target = fit(rect.shrink(18.0), iw, ih);
            painter.image(t.id(), target, uv, Color32::WHITE);
            self.edit_label(&painter, target);
            let c = liquifying::edit(
                ui,
                target,
                &mut self.settings.liquify,
                kind,
                self.liquify_radius,
                self.liquify_strength,
            );
            if c.changed {
                self.touch();
            }
            if c.committed {
                self.commit(tr("Fluidité").into());
            }
        } else if self.tool == Tool::Masks
            && (self.retouch.is_some() || self.selected_spot.is_some())
        {
            let target = fit(rect.shrink(18.0), iw, ih);
            painter.image(t.id(), target, uv, Color32::WHITE);
            self.edit_label(&painter, target);
            let c = retouching::edit(
                ui,
                target,
                &mut self.settings.spots,
                &mut self.selected_spot,
                self.retouch,
                self.spot_radius,
                self.scene_map.as_deref(),
            );
            if c.changed {
                self.touch();
            }
            if c.committed {
                self.commit(tr("Retouche").into());
            }
        } else {
            let target = fit(rect.shrink(18.0), iw, ih);
            painter.image(t.id(), target, uv, Color32::WHITE);
            self.edit_label(&painter, target);
            let sense = if self.targeting || crate::touch() {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::click()
            };
            let r = ui.interact(target, ui.scope_id().with("photo"), sense);
            self.probe = r.hover_pos().map(|p| {
                [
                    (p.x - target.min.x) / target.width(),
                    (p.y - target.min.y) / target.height(),
                ]
            });
            if self.targeting && !self.eyedropper {
                self.target_drag(ui, &r, target);
            } else if self.eyedropper {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                if r.clicked()
                    && let (Some(p), Some(photo)) = (r.interact_pointer_pos(), &self.photo)
                {
                    let (nx, ny) = (
                        (p.x - target.min.x) / target.width(),
                        (p.y - target.min.y) / target.height(),
                    );
                    let t =
                        photo.white_balance_for(photo.camera_color_at(nx, ny, &self.settings.crop));
                    self.settings.white_balance = WhiteBalance::Custom {
                        temperature: t.temperature,
                        tint: t.tint,
                    };
                    self.wb_label = "Personnalisée";
                    self.eyedropper = false;
                    self.commit(trf(
                        "Pipette : {temperature} K, {tint}",
                        &[
                            ("temperature", &format_value(t.temperature, 0, false)),
                            ("tint", &format_value(t.tint, 0, true)),
                        ],
                    ));
                    self.touch();
                }
            } else if crate::touch() && r.drag_stopped() {
                // Glisser du doigt : la photo suivante ou la précédente.
                let moved = ui.input(|i| {
                    i.pointer
                        .press_origin()
                        .zip(i.pointer.latest_pos())
                        .map(|(a, b)| b - a)
                });
                if let Some(d) = moved
                    && d.x.abs() > 60.0
                    && d.x.abs() > 2.0 * d.y.abs()
                {
                    self.act(if d.x < 0.0 { "next" } else { "prev" });
                }
            } else if r.on_hover_text(tr("Clic ou Z : zoom 1:1")).clicked() {
                self.zoom = true;
            }
        }
        let mut badges = Vec::new();
        if self.before {
            badges.push(tr("Avant"));
        }
        if self.watch.is_some() {
            badges.push(tr("Dossier surveillé"));
        }
        if self.loading.is_some() || self.shown != self.generation {
            badges.push(tr("Rendu…"));
        }
        if self.clipping {
            badges.push(if crate::touch() {
                tr("Écrêtage")
            } else {
                tr("Écrêtage (J)")
            });
        }
        if self.proofing.active().is_some_and(|p| p.simulate) {
            badges.push(tr("Épreuvage à l'écran"));
        }
        if self.proofing.active().is_some_and(|p| p.gamut) {
            badges.push(tr("Alerte de gamut"));
        }
        if self.eyedropper {
            badges.push(tr("Pipette : cliquez un gris ou un blanc de la photo"));
        } else if self.targeting {
            badges.push(tr(
                "Ciblage direct : glissez sur une couleur, vers le haut pour plus",
            ));
        }
        if self.crop_tool.is_some() {
            badges.push(tr("Recadrage : Entrée pour valider, Échap pour annuler"));
        } else if let Some(m) = self.selected_mask.and_then(|i| self.settings.masks.get(i)) {
            badges.push(match m.shape {
                Shape::Brush { .. } => {
                    tr("Pinceau : peindre en glissant, Option pour gommer, O pour voir le masque")
                }
                _ => tr("Masque : tirer les poignées, O pour voir le masque, Échap pour quitter"),
            });
        } else if self.tool == Tool::Masks && self.liquify.is_some() {
            badges.push(tr("Fluidité : glissez sur la photo, Échap pour quitter"));
        } else if self.tool == Tool::Masks && self.retouch.is_some() {
            badges.push(tr(
                "Retouche : cliquez une tache, tirez la zone ou sa source, Échap pour quitter",
            ));
        }
        if self.tool == Tool::Masks && self.show_dust {
            badges.push(tr(
                "Vue des taches : les poussières ressortent en points sombres",
            ));
        }
        for (k, b) in badges.iter().enumerate() {
            let p = rect.min + vec2(24.0, 24.0 + k as f32 * 26.0);
            let galley = painter.layout_no_wrap(
                b.to_string(),
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );
            let bg = egui::Rect::from_min_size(p, galley.size() + vec2(16.0, 8.0));
            painter.rect_filled(bg, 10.0, Color32::from_black_alpha(170));
            painter.galley(p + vec2(8.0, 4.0), galley, Color32::WHITE);
        }
        let status = painter.layout_no_wrap(
            self.status_text(),
            egui::FontId::proportional(11.5),
            theme::DIM,
        );
        painter.galley(
            pos2(rect.min.x + 20.0, rect.max.y - 20.0),
            status,
            theme::DIM,
        );
    }

    fn fields(&mut self, ui: &mut Ui, fields: &[Field]) -> Option<String> {
        // Un groupe à part : deux groupes d'un même panneau ont des libellés communs
        // (« Gain » du vignetage et du grain), qui ne doivent pas partager leur id.
        let group: Vec<&str> = fields.iter().map(|f| f.spec.label).collect();
        ui.scope_builder(egui::UiBuilder::new().id_salt(group), |ui| {
            let mut label = None;
            for f in fields {
                let value = (f.get)(&mut self.settings);
                let change = widgets::slider(ui, &f.spec, value);
                let v = *value;
                if change.changed {
                    self.touch();
                }
                if change.committed {
                    label = Some(format!(
                        "{} {}",
                        tr(f.spec.label),
                        format_value(v, f.spec.decimals, f.spec.min < 0.0)
                    ));
                }
            }
            label
        })
        .inner
    }

    /// Redresse la photo d'après ses lignes (l'horizon, les verticales), lues sur une
    /// copie de 1024 px sans recadrage ; rien si aucune ligne n'est assez sûre.
    pub fn auto_level(&mut self) {
        let Some(photo) = self.photo.clone() else {
            return;
        };
        let settings = Settings {
            crop: Crop::default(),
            ..self.settings.clone()
        };
        let mut timings = luma_core::Timings::default();
        let found = photo
            .base(
                &settings,
                Size::Fit(1024),
                &luma_core::Cancel::new(),
                &mut timings,
            )
            .ok()
            .and_then(|base| luma_image::horizon::angle(&base.image));
        match found {
            Some(a) => {
                self.settings.crop.angle = (a * 10.0).round() / 10.0;
                let label = trf(
                    "Redressement automatique : {angle}°",
                    &[("angle", &format_value(self.settings.crop.angle, 1, true))],
                );
                self.status = label.clone();
                self.commit(label);
                self.touch();
            }
            None => {
                self.status = tr("Pas de ligne assez sûre pour redresser la photo.").into();
            }
        }
    }

    /// Rend parallèles les verticales de la photo (la Verticale de la Transformation),
    /// lues sur une copie de 1024 px sans recadrage ni transformation ; rien sans
    /// longues verticales des deux côtés.
    pub fn auto_vertical(&mut self) {
        let Some(photo) = self.photo.clone() else {
            return;
        };
        let settings = Settings {
            crop: Crop::default(),
            transform: Default::default(),
            ..self.settings.clone()
        };
        let mut timings = luma_core::Timings::default();
        let found = photo
            .base(
                &settings,
                Size::Fit(1024),
                &luma_core::Cancel::new(),
                &mut timings,
            )
            .ok()
            .and_then(|base| luma_image::horizon::auto_vertical(&base.image));
        match found {
            Some(v) => {
                self.settings.transform.vertical = v.round();
                let label = trf(
                    "Verticales automatiques : {value}",
                    &[(
                        "value",
                        &format_value(self.settings.transform.vertical, 0, true),
                    )],
                );
                self.status = label.clone();
                self.commit(label);
                self.touch();
            }
            None => {
                self.status =
                    tr("Pas de verticales assez nettes des deux côtés de la photo.").into();
            }
        }
    }

    /// Demande au prochain rendu de chercher les poussières du capteur.
    pub fn find_dust(&mut self) {
        if self.photo.is_some() {
            self.dust_pending = true;
        }
    }

    /// Ajoute les poussières trouvées par le rendu, sauf celles déjà sous une retouche.
    fn add_dust(&mut self, found: Vec<luma_project::Spot>) {
        let aspect = self.image_size[1].max(1) as f32 / self.image_size[0].max(1) as f32;
        let covered = |s: &luma_project::Spot| {
            self.settings.spots.iter().any(|o| {
                let (dx, dy) = (s.center.0 - o.center.0, (s.center.1 - o.center.1) * aspect);
                (dx * dx + dy * dy).sqrt() < o.radius
            })
        };
        let new: Vec<_> = found.into_iter().filter(|s| !covered(s)).collect();
        if new.is_empty() {
            self.status = tr("Aucune poussière trouvée.").into();
            return;
        }
        let n = new.len();
        self.settings.spots.extend(new);
        self.selected_spot = None;
        self.status = trf("{n} poussières retirées", &[("n", &n)]);
        self.commit(trf("{n} poussières retirées", &[("n", &n)]));
        self.touch();
    }

    /// Panneau Retouche : correcteur et tampon, la liste des retouches et les réglages de
    /// celle qui est choisie.
    fn retouch_panel(&mut self, ui: &mut Ui, commits: &mut Vec<String>) {
        use luma_project::SpotKind;
        let open = self.retouch.is_some() || self.selected_spot.is_some() || self.liquify.is_some();
        egui::CollapsingHeader::new(RichText::new(tr("Retouche")).strong())
            .id_salt("Retouche")
            .open(open.then_some(true))
            .default_open(false)
            .show(ui, |ui| {
                // Quatre outils : à la ligne si le panneau est étroit.
                ui.horizontal_wrapped(|ui| {
                    for (kind, label, hint, action) in [
                        (
                            SpotKind::Heal,
                            tr("Correcteur"),
                            tr("Q : la texture d'ailleurs, la lumière du bord de la zone ; un clic, ou un trait le long d'une ride ou d'une rayure"),
                            "retouch_heal",
                        ),
                        (
                            SpotKind::Clone,
                            tr("Tampon"),
                            tr("Une copie exacte d'une autre partie de la photo"),
                            "retouch_clone",
                        ),
                        (
                            SpotKind::RedEye,
                            tr("Yeux rouges"),
                            tr("Un cercle sur chaque œil : la pupille rouge redevient noire"),
                            "retouch_red_eye",
                        ),
                        (
                            SpotKind::Remove,
                            tr("Supprimer|retouche"),
                            tr("Un clic sur ce qui gêne (passant, déchet), ou un trait le long d'un fil : la photo autour le remplace, texture comprise"),
                            "retouch_remove",
                        ),
                    ] {
                        if ui
                            .selectable_label(self.retouch == Some(kind), label)
                            .on_hover_text(hint)
                            .clicked()
                        {
                            self.act(action);
                        }
                    }
                });
                // Les taches de capteur : les voir, puis les retirer d'un coup.
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.show_dust, tr("Voir les taches"))
                        .on_hover_text(tr(
                            "Les poussières du capteur ressortent en points sombres sur du gris",
                        ));
                    if ui
                        .add_enabled(!self.dust_pending, egui::Button::new(tr("Retirer les poussières")))
                        .on_hover_text(tr(
                            "Trouve les taches de capteur dans les zones unies et pose un correcteur sur chacune",
                        ))
                        .clicked()
                    {
                        self.find_dust();
                    }
                });
                let spec = |label, min, max, default, decimals| SliderSpec {
                    label,
                    min,
                    max,
                    default,
                    decimals,
                    track: Track::Plain,
                    log: false,
                };
                // La fluidité : trois pinceaux qui déforment la photo.
                ui.label(RichText::new(tr("Fluidité")).size(12.0).color(theme::DIM));
                ui.horizontal_wrapped(|ui| {
                    use luma_project::WarpKind;
                    for (kind, label, hint, action) in [
                        (
                            WarpKind::Push,
                            tr("Pousser"),
                            tr("Glissez : la photo suit le pinceau"),
                            "liquify_push",
                        ),
                        (
                            WarpKind::Bloat,
                            tr("Gonfler"),
                            tr("Cliquez ou glissez : la zone grandit depuis son centre"),
                            "liquify_bloat",
                        ),
                        (
                            WarpKind::Pucker,
                            tr("Dégonfler"),
                            tr("Cliquez ou glissez : la zone se resserre vers son centre"),
                            "liquify_pucker",
                        ),
                    ] {
                        if ui
                            .selectable_label(self.liquify == Some(kind), label)
                            .on_hover_text(hint)
                            .clicked()
                        {
                            self.act(action);
                        }
                    }
                    let n = self.settings.liquify.len();
                    if n > 0
                        && ui
                            .button(tr("Effacer|fluidité"))
                            .on_hover_text(trf(
                                "Retire les {n} coups de pinceau de la fluidité",
                                &[("n", &n.to_string())],
                            ))
                            .clicked()
                    {
                        self.act("liquify_clear");
                    }
                });
                if self.liquify.is_some() {
                    let mut size = self.liquify_radius * 100.0;
                    widgets::slider(ui, &spec("Taille", 1.0, 30.0, 6.0, 1), &mut size);
                    self.liquify_radius = size / 100.0;
                    widgets::slider(ui, &spec("Force", 1.0, 100.0, 50.0, 0), &mut self.liquify_strength);
                }
                let selected = self
                    .selected_spot
                    .filter(|&i| i < self.settings.spots.len());
                if self.retouch.is_some() && selected.is_none() {
                    let mut size = self.spot_radius * 100.0;
                    widgets::slider(ui, &spec("Taille", 0.2, 20.0, 2.0, 1), &mut size);
                    self.spot_radius = size / 100.0;
                }
                let mut select = None;
                for (i, s) in self.settings.spots.iter().enumerate() {
                    let name = match s.kind {
                        SpotKind::Heal => tr("Correcteur"),
                        SpotKind::Clone => tr("Tampon"),
                        SpotKind::RedEye => tr("Yeux rouges"),
                        SpotKind::Remove => tr("Supprimer|retouche"),
                    };
                    let text = RichText::new(format!("{}. {name}", i + 1)).size(12.0);
                    if ui.selectable_label(selected == Some(i), text).clicked() {
                        select = Some((selected != Some(i)).then_some(i));
                    }
                }
                if let Some(s) = select {
                    self.selected_spot = s;
                    if s.is_some() {
                        self.selected_mask = None;
                    }
                }
                let Some(i) = self
                    .selected_spot
                    .filter(|&i| i < self.settings.spots.len())
                else {
                    return;
                };
                let scene = self.scene_map.clone();
                let mut delete = false;
                let s = &mut self.settings.spots[i];
                // Les yeux rouges et la suppression n'ont pas de source : ni choix du type,
                // ni nouvelle source.
                let red_eye = !s.kind.has_source();
                ui.horizontal(|ui| {
                    if red_eye {
                        return;
                    }
                    for (kind, label) in [
                        (SpotKind::Heal, tr("Correcteur")),
                        (SpotKind::Clone, tr("Tampon")),
                    ] {
                        if ui.selectable_label(s.kind == kind, label).clicked() && s.kind != kind {
                            s.kind = kind;
                            commits
                                .push(trf("Retouche : {tool}", &[("tool", &label.to_lowercase())]));
                        }
                    }
                });
                let mut size = s.radius * 100.0;
                let mut c = widgets::slider(ui, &spec("Taille", 0.2, 20.0, 2.0, 1), &mut size);
                s.radius = size / 100.0;
                c.merge(widgets::slider(
                    ui,
                    &spec("Contour progressif", 0.0, 100.0, 50.0, 0),
                    &mut s.feather,
                ));
                c.merge(widgets::slider(
                    ui,
                    &spec("Opacité", 0.0, 100.0, 100.0, 0),
                    &mut s.opacity,
                ));
                if c.committed {
                    commits.push(tr("Retouche réglée").into());
                }
                ui.horizontal(|ui| {
                    if !red_eye
                        && ui
                        .button(tr("Nouvelle source"))
                        .on_hover_text(tr("Chercher une autre partie de la photo qui convient"))
                        .clicked()
                        && let Some(src) = scene.as_deref().and_then(|img| {
                            luma_image::retouch::find_source(img, s.center, s.radius)
                        })
                    {
                        s.source = src;
                        commits.push(tr("Nouvelle source").into());
                    }
                    delete = ui.button(tr("Supprimer")).clicked();
                });
                if delete {
                    self.act("mask_delete");
                }
            });
    }

    /// La couleur et l'opacité de la zone affichée (O).
    fn mask_style_row(&mut self, ui: &mut Ui) {
        let style = &mut self.mask_style;
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Zone")).size(11.5).color(theme::DIM));
            let current = masking::COLORS[style.color.min(masking::COLORS.len() - 1)].0;
            egui::ComboBox::from_id_salt("couleur-zone")
                .selected_text(tr(current))
                .show_ui(ui, |ui| {
                    for (k, (name, color)) in masking::COLORS.iter().enumerate() {
                        let text = RichText::new(tr(name)).color(*color);
                        ui.selectable_value(&mut style.color, k, text);
                    }
                })
                .response
                .on_hover_text(tr("Couleur de la zone affichée"));
            ui.add(
                egui::DragValue::new(&mut style.opacity)
                    .range(10.0..=100.0)
                    .speed(0.5)
                    .max_decimals(0)
                    .suffix(" %"),
            )
            .on_hover_text(tr("Opacité"))
            .accessible_name(tr("Opacité de la zone affichée"));
        });
    }

    /// Les combinaisons du masque `i` : d'autres formes ajoutées, retirées ou en
    /// intersection ; la partie choisie se règle sur la photo.
    fn parts_panel(&mut self, ui: &mut Ui, i: usize, commits: &mut Vec<String>) {
        use luma_project::{Combine, Part};
        sub_title(ui, tr("Combiner"));
        ui.horizontal(|ui| {
            for combine in [Combine::Add, Combine::Subtract, Combine::Intersect] {
                ui.menu_button(tr(combine_name(combine)), |ui| {
                    for mask in [Mask::linear(), Mask::radial(), Mask::brush()] {
                        if ui.button(tr(&mask.name)).clicked() {
                            let parts = &mut self.settings.masks[i].parts;
                            parts.push(Part {
                                combine,
                                shape: mask.shape,
                                invert: false,
                            });
                            self.selected_part = Some((i, parts.len() - 1));
                            commits.push(trf(
                                "Masque combiné : {shape}",
                                &[("shape", &tr(&mask.name).to_lowercase())],
                            ));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text(tr(combine_hint(combine)));
            }
        });
        if self.settings.masks[i].parts.is_empty() {
            return;
        }
        let current = self.selected_part.filter(|&(m, _)| m == i).map(|(_, p)| p);
        let main = RichText::new(tr("Forme principale")).size(12.0);
        if ui.selectable_label(current.is_none(), main).clicked() {
            self.selected_part = None;
        }
        let mut remove = None;
        for (j, part) in self.settings.masks[i].parts.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                let text = format!(
                    "{} · {}",
                    tr(combine_name(part.combine)),
                    tr(shape_name(&part.shape))
                );
                let text = RichText::new(text).size(12.0);
                if ui.selectable_label(current == Some(j), text).clicked() {
                    self.selected_part = (current != Some(j)).then_some((i, j));
                }
                if ui.checkbox(&mut part.invert, tr("Inverser")).changed() {
                    commits.push(tr("Partie du masque inversée").into());
                }
                if ui
                    .small_button("×")
                    .on_hover_text(tr("Supprimer"))
                    .clicked()
                {
                    remove = Some(j);
                }
            });
        }
        if let Some(j) = remove {
            self.settings.masks[i].parts.remove(j);
            self.selected_part = None;
            commits.push(tr("Partie du masque supprimée").into());
        }
    }

    /// Panneau Masquage : la liste des masques, et les curseurs du masque choisi.
    fn masks_panel(&mut self, ui: &mut Ui, commits: &mut Vec<String>) {
        let editing = self.selected_mask.is_some();
        egui::CollapsingHeader::new(RichText::new(tr("Masquage")).strong())
            .id_salt("Masquage")
            .open(editing.then_some(true))
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let width = ((ui.available_width() - 18.0) / 4.0).floor();
                    for (icon, label, key, action) in [
                        (
                            Icon::Radial,
                            tr("Radial"),
                            tr("Maj+M : un cercle, plein effet au centre"),
                            "mask_radial",
                        ),
                        (
                            Icon::Linear,
                            tr("Linéaire"),
                            tr("M : trois traits, plein effet, milieu, plus d'effet"),
                            "mask_linear",
                        ),
                        (
                            Icon::Brush,
                            tr("Pinceau"),
                            tr("K : peindre, Option pour gommer"),
                            "mask_brush",
                        ),
                        (
                            Icon::Films,
                            tr("Image entière|bouton"),
                            tr("Toute l'image : pour empiler un film de plus"),
                            "mask_full",
                        ),
                    ] {
                        if big_choice_sized(ui, icon, label, false, width)
                            .on_hover_text(key)
                            .clicked()
                        {
                            self.act(action);
                        }
                    }
                });
                if ui
                    .button(tr("Sélectionner le ciel"))
                    .on_hover_text(tr(
                        "Un masque sur le ciel de la photo, trouvé sur la photo elle-même (clair, uni, relié au haut), ses bords sur les branches et les toits",
                    ))
                    .clicked()
                {
                    self.act("mask_sky");
                }
                let mut select = None;
                for (i, m) in self.settings.masks.iter().enumerate() {
                    let current = self.selected_mask == Some(i);
                    let film = m
                        .look
                        .as_ref()
                        .map(|l| format!(" · {}", tr(&l.name)))
                        .unwrap_or_default();
                    let mut text =
                        RichText::new(format!("{}. {}{film}", i + 1, tr(&m.name))).size(12.0);
                    if !m.is_active() && m.active_look().is_none() {
                        text = text.color(theme::DIM);
                    }
                    if ui.selectable_label(current, text).clicked() {
                        select = Some(if current { None } else { Some(i) });
                    }
                }
                if let Some(s) = select {
                    self.selected_mask = s;
                    if s.is_some() {
                        self.retouch = None;
                        self.selected_spot = None;
                        self.liquify = None;
                    }
                }
                let Some(i) = self
                    .selected_mask
                    .filter(|i| *i < self.settings.masks.len())
                else {
                    if !self.settings.masks.is_empty() {
                        ui.label(
                            RichText::new(tr("Choisissez un masque pour le régler."))
                                .size(11.5)
                                .color(theme::DIM),
                        );
                    }
                    return;
                };
                ui.horizontal(|ui| {
                    let m = &mut self.settings.masks[i];
                    if ui.checkbox(&mut m.invert, tr("Inverser")).changed() {
                        commits.push(tr("Masque inversé").into());
                    }
                    // Le pinceau qui suit les bords de la photo.
                    let brush =
                        |s: &luma_project::Shape| matches!(s, luma_project::Shape::Brush { .. });
                    if (brush(&m.shape) || m.parts.iter().any(|p| brush(&p.shape)))
                        && ui
                            .checkbox(&mut m.edges, tr("Suivre les bords"))
                            .on_hover_text(tr("La zone peinte s'arrête aux contours de la photo"))
                            .changed()
                    {
                        commits.push(tr("Pinceau qui suit les bords").into());
                    }
                    if ui
                        .selectable_label(self.show_mask, tr("Voir (O)"))
                        .clicked()
                    {
                        self.show_mask = !self.show_mask;
                    }
                    if ui.button(tr("Supprimer")).clicked() {
                        self.act("mask_delete");
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .button(tr("Dupliquer|masque"))
                        .on_hover_text(tr("Une copie du masque, ses réglages compris"))
                        .clicked()
                    {
                        self.act("mask_duplicate");
                    }
                    if ui
                        .button(tr("Dupliquer inversé"))
                        .on_hover_text(tr(
                            "Une copie inversée, réglages à zéro : tout le reste de la photo (le sol après le ciel)",
                        ))
                        .clicked()
                    {
                        self.act("mask_duplicate_inverted");
                    }
                });
                if self.show_mask {
                    self.mask_style_row(ui);
                }
                self.parts_panel(ui, i, commits);
                let luts = self.luts.clone();
                let Some(m) = self.settings.masks.get_mut(i) else {
                    return;
                };
                let name = tr(&m.name).to_lowercase();
                let spec = |label, min, max, decimals| SliderSpec {
                    label,
                    min,
                    max,
                    default: if label == "Intensité" { 100.0 } else { 0.0 },
                    decimals,
                    track: Track::Plain,
                    log: false,
                };
                // Le film du masque : il s'empile sur le reste, dans la zone du masque.
                sub_title(ui, tr("Film du masque"));
                let current = m.look.as_ref().map(|l| l.name.clone());
                let mut pick: Option<Option<String>> = None;
                egui::ComboBox::from_id_salt("film-masque")
                    .selected_text(current.as_deref().map_or(tr("Aucun"), tr))
                    .height(440.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(current.is_none(), tr("Aucun"))
                            .clicked()
                        {
                            pick = Some(None);
                        }
                        if !luts.is_empty() {
                            ui.label(
                                RichText::new(tr("Mes pellicules"))
                                    .size(11.0)
                                    .color(theme::DIM),
                            );
                            for n in &luts {
                                if ui
                                    .selectable_label(current.as_deref() == Some(n), n)
                                    .clicked()
                                {
                                    pick = Some(Some(n.clone()));
                                }
                            }
                        }
                        for group in presets::Group::ALL {
                            ui.label(RichText::new(tr(group.name())).size(11.0).color(theme::DIM));
                            for p in PRESETS.iter().filter(|p| p.group == group) {
                                if ui
                                    .selectable_label(
                                        current.as_deref() == Some(p.name),
                                        tr(p.name),
                                    )
                                    .on_hover_text(tr(p.description))
                                    .clicked()
                                {
                                    pick = Some(Some(p.name.to_owned()));
                                }
                            }
                        }
                    })
                    .response
                    .accessible_name(tr("Film du masque"));
                let mut film_changed = false;
                if let Some(choice) = pick {
                    let amount = m.look.as_ref().map_or(100.0, |l| l.amount);
                    commits.push(trf(
                        "Masque {mask} : film {film}",
                        &[
                            ("mask", &name),
                            ("film", &choice.as_deref().map_or(tr("aucun"), tr)),
                        ],
                    ));
                    m.look = choice.map(|n| luma_project::Look { name: n, amount });
                    film_changed = true;
                }
                if let Some(look) = &mut m.look {
                    let film = SliderSpec {
                        default: 100.0,
                        track: Track::Gradient(&theme::SPECTRUM),
                        ..spec("Intensité du film", 0.0, 100.0, 0)
                    };
                    let c = widgets::slider(ui, &film, &mut look.amount);
                    film_changed |= c.changed;
                    if c.committed {
                        commits.push(trf(
                            "Masque {mask} : {film} à {n} %",
                            &[
                                ("mask", &name),
                                ("film", &tr(&look.name)),
                                ("n", &format_value(look.amount, 0, false)),
                            ],
                        ));
                    }
                }
                sub_title(ui, tr("Réglages du masque"));
                let l = &mut m.local;
                let rows: [(SliderSpec, &mut f32); 17] = [
                    (spec("Intensité", 0.0, 100.0, 0), &mut m.amount),
                    (spec("Température", -100.0, 100.0, 0), &mut l.temperature),
                    (spec("Teinte", -100.0, 100.0, 0), &mut l.tint),
                    (spec("Exposition", -4.0, 4.0, 2), &mut l.exposure),
                    (spec("Contraste", -100.0, 100.0, 0), &mut l.contrast),
                    (spec("Hautes lumières", -100.0, 100.0, 0), &mut l.highlights),
                    (spec("Ombres", -100.0, 100.0, 0), &mut l.shadows),
                    (spec("Blancs", -100.0, 100.0, 0), &mut l.whites),
                    (spec("Noirs", -100.0, 100.0, 0), &mut l.blacks),
                    (spec("Saturation", -100.0, 100.0, 0), &mut l.saturation),
                    (spec("Clarté", -100.0, 100.0, 0), &mut l.clarity),
                    (spec("Texture", -100.0, 100.0, 0), &mut l.texture),
                    (spec("Netteté", -100.0, 100.0, 0), &mut l.sharpness),
                    (spec("Réduction du bruit", 0.0, 100.0, 0), &mut l.noise),
                    (spec("Correction du voile", -100.0, 100.0, 0), &mut l.dehaze),
                    // Au pinceau sur un tissu, une grille, un contour à liserés.
                    (spec("Moiré", 0.0, 100.0, 0), &mut l.moire),
                    (spec("Frange", 0.0, 100.0, 0), &mut l.defringe),
                ];
                let mut changed = film_changed;
                for (spec, value) in rows {
                    let c = widgets::slider(ui, &spec, value);
                    changed |= c.changed;
                    if c.committed {
                        commits.push(trf(
                            "Masque {mask} : {setting} {value}",
                            &[
                                ("mask", &name),
                                ("setting", &tr(spec.label).to_lowercase()),
                                (
                                    "value",
                                    &format_value(*value, spec.decimals, spec.min < 0.0),
                                ),
                            ],
                        ));
                    }
                }
                // La plage : la zone ne garde que certains tons, ou certaines couleurs.
                sub_title(ui, tr("Plage"));
                let kinds = [tr("Aucune"), tr("Luminance"), tr("Couleur")];
                let current = match m.range {
                    None => 0,
                    Some(Range::Luminance { .. }) => 1,
                    Some(Range::Color { .. }) => 2,
                };
                let mut pick = current;
                ui.horizontal(|ui| {
                    for (k, label) in kinds.iter().enumerate() {
                        if ui.selectable_label(pick == k, *label).clicked() {
                            pick = k;
                        }
                    }
                });
                if pick != current {
                    m.range = match pick {
                        1 => Some(Range::luminance()),
                        2 => Some(Range::color()),
                        _ => None,
                    };
                    changed = true;
                    commits.push(trf(
                        "Masque {mask} : plage {kind}",
                        &[("mask", &name), ("kind", &kinds[pick].to_lowercase())],
                    ));
                }
                let range_rows: Vec<(SliderSpec, &mut f32)> = match &mut m.range {
                    Some(Range::Luminance { low, high, feather }) => vec![
                        (
                            SliderSpec {
                                default: 60.0,
                                track: Track::Gradient(&widgets::GREYS),
                                ..spec("Minimum", 0.0, 100.0, 0)
                            },
                            low,
                        ),
                        (
                            SliderSpec {
                                default: 100.0,
                                track: Track::Gradient(&widgets::GREYS),
                                ..spec("Maximum", 0.0, 100.0, 0)
                            },
                            high,
                        ),
                        (
                            SliderSpec {
                                default: 30.0,
                                ..spec("Fondu", 0.0, 100.0, 0)
                            },
                            feather,
                        ),
                    ],
                    Some(Range::Color {
                        hue,
                        width,
                        feather,
                    }) => vec![
                        (
                            SliderSpec {
                                default: 255.0,
                                track: Track::Gradient(&widgets::HUE),
                                ..spec("Teinte|mélangeur", 0.0, 360.0, 0)
                            },
                            hue,
                        ),
                        (
                            SliderSpec {
                                default: 30.0,
                                ..spec("Largeur", 0.0, 180.0, 0)
                            },
                            width,
                        ),
                        (
                            SliderSpec {
                                default: 50.0,
                                ..spec("Fondu", 0.0, 100.0, 0)
                            },
                            feather,
                        ),
                    ],
                    None => Vec::new(),
                };
                let mut range_committed = false;
                for (spec, value) in range_rows {
                    let c = widgets::slider(ui, &spec, value);
                    changed |= c.changed;
                    range_committed |= c.committed;
                }
                if range_committed {
                    if let Some(r) = &mut m.range {
                        r.clamp();
                    }
                    commits.push(trf("Masque {mask} : plage", &[("mask", &name)]));
                }
                if matches!(m.shape, Shape::Brush { .. }) {
                    sub_title(ui, tr("Pinceau"));
                    let b = &mut self.brush;
                    let mut size = b.radius * 100.0;
                    for (spec, value) in [
                        (
                            SliderSpec {
                                default: 4.0,
                                ..spec("Taille", 0.5, 25.0, 1)
                            },
                            &mut size,
                        ),
                        (
                            SliderSpec {
                                default: 50.0,
                                ..spec("Contour progressif", 0.0, 100.0, 0)
                            },
                            &mut b.feather,
                        ),
                        (
                            SliderSpec {
                                default: 100.0,
                                ..spec("Débit", 1.0, 100.0, 0)
                            },
                            &mut b.flow,
                        ),
                    ] {
                        widgets::slider(ui, &spec, value);
                    }
                    b.radius = size / 100.0;
                }
                if let Shape::Radial { feather, .. } = &mut m.shape {
                    let c = widgets::slider(
                        ui,
                        &SliderSpec {
                            default: 50.0,
                            ..spec("Contour progressif", 0.0, 100.0, 0)
                        },
                        feather,
                    );
                    changed |= c.changed;
                    if c.committed {
                        commits.push(trf(
                            "Masque {mask} : contour progressif",
                            &[("mask", &name)],
                        ));
                    }
                }
                if changed {
                    self.xmp_due = Some(0.0);
                }
            });
    }

    /// Choisit le profil d'imprimante de l'épreuvage et l'active.
    pub fn load_proof_profile(&mut self, path: &Path) {
        let parsed = std::fs::read(path)
            .ok()
            .and_then(|b| luma_color::profile::Profile::parse(&b).ok());
        let Some(profile) = parsed else {
            self.proofing.error = Some("Profil ICC illisible");
            return;
        };
        self.proofing.description = if profile.description.is_empty() {
            file_name(path)
        } else {
            profile.description.clone()
        };
        self.proofing.path = Some(path.to_path_buf());
        self.proofing.profile = Some(profile);
        self.proofing.on = true;
        self.proofing.build();
    }

    /// Active ou coupe l'épreuvage, et choisit l'intention de rendu.
    pub fn set_proofing(&mut self, on: bool, relative: bool) {
        let rebuild = relative != self.proofing.relative;
        self.proofing.relative = relative;
        self.proofing.on = on && self.proofing.profile.is_some();
        if rebuild {
            self.proofing.build();
        }
    }

    /// Montre le ton du papier (l'encre noire suit) ou seulement le noir de l'encre.
    pub fn set_proof_media(&mut self, paper: bool, ink: bool) {
        let media = luma_color::proof::Media {
            paper,
            ink: ink || paper,
        };
        if media != self.proofing.media {
            self.proofing.media = media;
            self.proofing.build();
        }
    }

    pub fn proofing_active(&self) -> bool {
        self.proofing.active().is_some_and(|p| p.simulate)
    }

    /// Montre ou cache l'alerte de gamut (il faut un profil d'imprimante).
    pub fn set_gamut_alert(&mut self, on: bool) {
        self.proofing.gamut = on && self.proofing.table.is_some();
    }

    pub fn gamut_alert(&self) -> bool {
        self.proofing.active().is_some_and(|p| p.gamut)
    }

    fn proofing_panel(&mut self, ui: &mut Ui) {
        egui::CollapsingHeader::new(RichText::new(tr("Épreuvage à l'écran")).strong())
            .id_salt("Épreuvage à l'écran")
            .default_open(self.proofing.profile.is_some())
            .show(ui, |ui| {
                let (mut on, mut relative) = (self.proofing.on, self.proofing.relative);
                let mut gamut = self.proofing.gamut;
                ui.add_enabled_ui(self.proofing.table.is_some(), |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut on, tr("Simuler l'impression"));
                        ui.checkbox(&mut gamut, tr("Alerte de gamut")).on_hover_text(tr(
                            "En bleu : les couleurs que l'imprimante ne saura pas rendre",
                        ));
                    });
                });
                let mut media = self.proofing.media;
                ui.add_enabled_ui(self.proofing.table.is_some() && on, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut media.paper, tr("Couleur du papier"))
                            .on_hover_text(tr(
                                "Le ton et la luminosité du papier ; le noir de l'encre suit",
                            ));
                        ui.add_enabled_ui(!media.paper, |ui| {
                            let mut ink = media.ink || media.paper;
                            ui.checkbox(&mut ink, tr("Encre noire")).on_hover_text(tr(
                                "Le vrai noir de l'encre, plus clair que celui de l'écran",
                            ));
                            media.ink = ink;
                        });
                    });
                });
                if media != self.proofing.media {
                    self.set_proof_media(media.paper, media.ink);
                }
                ui.horizontal(|ui| {
                    if ui.button(tr("Choisir un profil ICC…")).clicked()
                        && let Some(path) = crate::dialogs::pick_icc()
                    {
                        self.load_proof_profile(&path);
                        on = self.proofing.on;
                    }
                    let name = if self.proofing.profile.is_some() {
                        self.proofing.description.clone()
                    } else {
                        tr("Aucun profil choisi").to_owned()
                    };
                    ui.label(RichText::new(name).color(theme::DIM));
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr("Intention")).color(theme::DIM));
                    ui.selectable_value(&mut relative, false, tr("Perceptuelle"))
                        .on_hover_text(tr(
                            "Toutes les couleurs sont rapprochées pour que celles hors de portée de l'encre gardent leurs nuances",
                        ));
                    ui.selectable_value(&mut relative, true, tr("Colorimétrique relative"))
                        .on_hover_text(tr(
                            "Les couleurs imprimables restent exactes, les autres sont ramenées au plus près",
                        ));
                });
                if let Some(e) = self.proofing.error {
                    ui.label(RichText::new(tr(e)).color(theme::DIM));
                }
                if (on, relative) != (self.proofing.on, self.proofing.relative) {
                    self.set_proofing(on, relative);
                }
                if gamut != self.proofing.gamut {
                    self.set_gamut_alert(gamut);
                }
            });
    }

    fn develop_panels(&mut self, ui: &mut Ui) {
        let mut commits: Vec<String> = Vec::new();
        let keep = |l: Option<String>, commits: &mut Vec<String>| {
            if let Some(l) = l {
                commits.push(l);
            }
        };

        // Sur iPhone, l'histogramme est plus petit, sans titre, et l'épreuvage (pour
        // l'impression) n'apparaît qu'avec un profil d'imprimante.
        if self.compact {
            widgets::histogram_sized(ui, &self.histogram, &mut self.clipping, 52.0)
                .on_hover_text(tr("Clic ou J : afficher l'écrêtage"));
        } else {
            self.scopes(ui, 96.0);
        }
        ui.add_space(6.0);
        self.selection_actions(ui);
        if !self.compact || self.proofing.profile.is_some() {
            self.proofing_panel(ui);
        }

        if self.tool == Tool::Masks {
            self.retouch_panel(ui, &mut commits);
            self.masks_panel(ui, &mut commits);
        }

        if self.tool == Tool::Light {
            egui::CollapsingHeader::new(RichText::new(tr("Base")).strong())
                .id_salt("Base")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr("Profil")).color(theme::DIM));
                        let mut names = vec![
                            (Profile::Color, tr("Couleur")),
                            (Profile::Neutral, tr("Neutre")),
                            (Profile::Vivid, tr("Vif")),
                            (Profile::Monochrome, tr("Monochrome")),
                        ];
                        // Le rendu du fichier lui‑même : pour un JPEG, un PNG, un TIFF…
                        if self.photo.as_ref().is_some_and(|p| p.raw().rendered) {
                            names.insert(0, (Profile::Original, tr("Original")));
                        }
                        let current = names
                            .iter()
                            .find(|n| n.0 == self.settings.profile)
                            .map_or(names[0].1, |n| n.1);
                        egui::ComboBox::from_id_salt("profil")
                            .selected_text(current)
                            .show_ui(ui, |ui| {
                                for (p, name) in names {
                                    if ui
                                        .selectable_value(&mut self.settings.profile, p, name)
                                        .changed()
                                    {
                                        commits.push(trf("Profil : {name}", &[("name", &name)]));
                                    }
                                }
                            })
                            .response
                            .accessible_name(tr("Profil"));
                    });
                    ui.add_space(4.0);
                    let as_shot = self.photo.as_ref().map(|p| p.as_shot());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(tr("Balance des blancs")).color(theme::DIM));
                        let current = match self.settings.white_balance {
                            WhiteBalance::AsShot => "Telle quelle",
                            WhiteBalance::Custom { temperature, tint } => WB_PRESETS
                                .iter()
                                .find(|p| p.1 == temperature && p.2 == tint)
                                .map_or(self.wb_label, |p| p.0),
                        };
                        let mut choice: Option<&'static str> = None;
                        egui::ComboBox::from_id_salt("balance")
                            .selected_text(tr(current))
                            .show_ui(ui, |ui| {
                                for name in ["Telle quelle", "Auto"]
                                    .into_iter()
                                    .chain(WB_PRESETS.iter().map(|p| p.0))
                                {
                                    if ui.selectable_label(current == name, tr(name)).clicked() {
                                        choice = Some(name);
                                    }
                                }
                            })
                            .response
                            .accessible_name(tr("Balance des blancs"));
                        let picking = self.eyedropper;
                        if ui
                            .selectable_label(picking, tr("Pipette"))
                            .on_hover_text(tr("W : cliquez un gris de la photo"))
                            .clicked()
                        {
                            self.eyedropper = !picking;
                        }
                        match choice {
                            Some("Telle quelle") => {
                                self.settings.white_balance = WhiteBalance::AsShot;
                                commits.push(tr("Balance : telle quelle").into());
                            }
                            Some("Auto") => {
                                if let Some(photo) = &self.photo {
                                    let t = photo.auto_white_balance();
                                    self.settings.white_balance = WhiteBalance::Custom {
                                        temperature: t.temperature,
                                        tint: t.tint,
                                    };
                                    self.wb_label = "Auto";
                                    commits.push(tr("Balance : auto").into());
                                }
                            }
                            Some(name) => {
                                if let Some(p) = WB_PRESETS.iter().find(|p| p.0 == name) {
                                    self.settings.white_balance = WhiteBalance::Custom {
                                        temperature: p.1,
                                        tint: p.2,
                                    };
                                    commits.push(trf(
                                        "Balance : {name}",
                                        &[("name", &tr(name).to_lowercase())],
                                    ));
                                }
                            }
                            None => {}
                        }
                    });
                    let (mut temperature, mut tint) = match (self.settings.white_balance, as_shot) {
                        (WhiteBalance::Custom { temperature, tint }, _) => (temperature, tint),
                        (WhiteBalance::AsShot, Some(t)) => (t.temperature, t.tint),
                        (WhiteBalance::AsShot, None) => (5500.0, 0.0),
                    };
                    let default_t = as_shot.map_or(5500.0, |t| t.temperature.round());
                    let default_tint = as_shot.map_or(0.0, |t| t.tint.round());
                    let ts = SliderSpec {
                        label: "Temp.",
                        min: 2000.0,
                        max: 50000.0,
                        default: default_t,
                        decimals: 0,
                        track: Track::Gradient(&widgets::TEMPERATURE),
                        log: true,
                    };
                    let tn = SliderSpec {
                        label: "Teinte",
                        min: -150.0,
                        max: 150.0,
                        default: default_tint,
                        decimals: 0,
                        track: Track::Gradient(&widgets::TINT),
                        log: false,
                    };
                    let mut c = widgets::slider(ui, &ts, &mut temperature);
                    let c2 = widgets::slider(ui, &tn, &mut tint);
                    c.merge(c2);
                    if c.changed {
                        self.wb_label = "Personnalisée";
                        self.settings.white_balance = WhiteBalance::Custom { temperature, tint };
                        self.touch();
                    }
                    if c.committed {
                        commits.push(trf(
                            "Balance : {temperature} K, {tint}",
                            &[
                                ("temperature", &format_value(temperature, 0, false)),
                                ("tint", &format_value(tint, 0, true)),
                            ],
                        ));
                    }
                    sub_title(ui, tr("Tonalité"));
                    // Les tons lus sur la photo : un point de départ, réglable ensuite.
                    if ui
                        .button(tr("Tons auto"))
                        .on_hover_text(tr(
                            "Exposition, contraste, hautes lumières et ombres lus sur l'histogramme de la photo",
                        ))
                        .clicked()
                        && let Some(photo) = &self.photo
                    {
                        let t = photo.auto_tones(&self.settings);
                        let s = &mut self.settings;
                        s.exposure = t.exposure;
                        s.contrast = t.contrast;
                        s.highlights = t.highlights;
                        s.shadows = t.shadows;
                        s.whites = 0.0;
                        s.blacks = 0.0;
                        commits.push(tr("Tons automatiques").into());
                    }
                    let l = self.fields(ui, &TONE);
                    keep(l, &mut commits);
                    if ui
                        .checkbox(
                            &mut self.settings.highlight_reconstruction,
                            tr("Reconstruire les hautes lumières"),
                        )
                        .on_hover_text(tr(
                            "Un canal saturé par le capteur reprend la couleur de ses voisins : ciels, peaux et tissus clairs retrouvent teinte et texture",
                        ))
                        .changed()
                    {
                        commits.push(if self.settings.highlight_reconstruction {
                            tr("Hautes lumières reconstruites").into()
                        } else {
                            tr("Hautes lumières telles que mesurées").into()
                        });
                    }
                    sub_title(ui, tr("Présence"));
                    let l = self.fields(ui, &PRESENCE);
                    keep(l, &mut commits);
                });
            self.defaults_panel(ui, &mut commits);
        }

        if self.tool == Tool::Light {
            egui::CollapsingHeader::new(RichText::new(tr("Courbe des tonalités")).strong())
                .id_salt("Courbe des tonalités")
                .default_open(true)
                .show(ui, |ui| {
                    let names = [tr("Luminosité"), tr("Rouge"), tr("Vert"), tr("Bleu")];
                    let on: [bool; 4] = std::array::from_fn(|i| self.curve_channel == i);
                    let tip = tr(
                        "Un clic sur la courbe ajoute un point, glissez-le pour le régler ; un double-clic le retire",
                    );
                    if let Some(i) = widgets::segments(ui, &names, &on, &[tip; 4], 12.5) {
                        self.curve_channel = i;
                    }
                    // Sous la courbe de luminosité, en pâle, la paramétrique qui la précède.
                    let parametric = luma_image::curve::parametric(&self.settings.tone_curve);
                    let c = &mut self.settings.tone_curve;
                    let (points, color, under) = match self.curve_channel {
                        1 => (&mut c.red, Color32::from_rgb(0xf0, 0x6a, 0x6a), None),
                        2 => (&mut c.green, Color32::from_rgb(0x6a, 0xd8, 0x7a), None),
                        3 => (&mut c.blue, Color32::from_rgb(0x6a, 0x9a, 0xf0), None),
                        _ => (&mut c.point, Color32::from_gray(0xe0), Some(&parametric)),
                    };
                    let edit = curving::editor(ui, "courbe à points", points, color, under);
                    if edit.changed {
                        self.touch();
                    }
                    if edit.committed {
                        commits.push(tr("Courbe à points").into());
                    }
                    let l = self.fields(ui, &CURVE);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Color {
            egui::CollapsingHeader::new(RichText::new(tr("Mélangeur de couleurs")).strong())
                .id_salt("Mélangeur de couleurs")
                .default_open(true)
                .show(ui, |ui| {
                    let names = [
                        tr("Teinte|mélangeur"),
                        tr("Saturation"),
                        tr("Luminance"),
                        tr("Ciblage"),
                    ];
                    let on = [
                        self.hsl_tab == 0,
                        self.hsl_tab == 1,
                        self.hsl_tab == 2,
                        self.targeting,
                    ];
                    let tips = [
                        "",
                        "",
                        "",
                        tr(
                            "Ciblage direct (T) : glissez sur une couleur de la photo, vers le haut ou le bas, pour régler cette couleur dans l'onglet choisi.",
                        ),
                    ];
                    match widgets::segments(ui, &names, &on, &tips, 12.5) {
                        Some(3) => {
                            self.targeting = !self.targeting;
                            self.eyedropper = false;
                        }
                        Some(i) => self.hsl_tab = i,
                        None => {}
                    }
                    let kind =
                        [tr("Teinte|mélangeur"), tr("Saturation"), tr("Luminance")][self.hsl_tab];
                    for (i, name) in HSL_NAMES.iter().enumerate() {
                        let value = match self.hsl_tab {
                            0 => &mut self.settings.hsl.hue[i],
                            1 => &mut self.settings.hsl.saturation[i],
                            _ => &mut self.settings.hsl.luminance[i],
                        };
                        let colors: &'static [Color32] = hsl_track(self.hsl_tab, i);
                        let spec = SliderSpec {
                            label: name,
                            min: -100.0,
                            max: 100.0,
                            default: 0.0,
                            decimals: 0,
                            track: Track::Gradient(colors),
                            log: false,
                        };
                        let c = widgets::slider(ui, &spec, value);
                        let v = *value;
                        if c.changed {
                            self.touch();
                        }
                        if c.committed {
                            commits.push(format!(
                                "{kind} {} {}",
                                tr(name).to_lowercase(),
                                format_value(v, 0, true)
                            ));
                        }
                    }
                });
        }

        if self.tool == Tool::Color {
            egui::CollapsingHeader::new(RichText::new(tr("Étalonnage des couleurs")).strong())
                .id_salt("Étalonnage des couleurs")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, name) in [
                            tr("Ombres"),
                            tr("Tons moyens"),
                            tr("Hautes lum."),
                            tr("Global"),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            ui.selectable_value(&mut self.grading_tab, i, name);
                        }
                    });
                    let names = [
                        tr("Ombres"),
                        tr("Tons moyens"),
                        tr("Hautes lumières"),
                        tr("Global"),
                    ];
                    let g = &mut self.settings.color_grading;
                    let wheel = match self.grading_tab {
                        0 => &mut g.shadows,
                        1 => &mut g.midtones,
                        2 => &mut g.highlights,
                        _ => &mut g.global,
                    };
                    let mut c = Change::default();
                    ui.vertical_centered(|ui| {
                        c.merge(widgets::color_wheel(
                            ui,
                            names[self.grading_tab],
                            &mut wheel.hue,
                            &mut wheel.saturation,
                            150.0,
                        ));
                    });
                    let lum = SliderSpec {
                        label: "Luminance",
                        min: -100.0,
                        max: 100.0,
                        default: 0.0,
                        decimals: 0,
                        track: Track::Plain,
                        log: false,
                    };
                    c.merge(widgets::slider(ui, &lum, &mut wheel.luminance));
                    let (h, s, l) = (wheel.hue, wheel.saturation, wheel.luminance);
                    let blend = SliderSpec {
                        label: "Fusion|étalonnage",
                        min: 0.0,
                        max: 100.0,
                        default: 50.0,
                        decimals: 0,
                        track: Track::Plain,
                        log: false,
                    };
                    let balance = SliderSpec {
                        label: "Balance",
                        min: -100.0,
                        max: 100.0,
                        default: 0.0,
                        decimals: 0,
                        track: Track::Plain,
                        log: false,
                    };
                    c.merge(widgets::slider(ui, &blend, &mut g.blending));
                    c.merge(widgets::slider(ui, &balance, &mut g.balance));
                    if c.changed {
                        self.touch();
                    }
                    if c.committed {
                        commits.push(trf(
                            "Étalonnage {zone} : {hue}°, {saturation}, {luminance}",
                            &[
                                ("zone", &names[self.grading_tab].to_lowercase()),
                                ("hue", &format!("{h:.0}")),
                                ("saturation", &format!("{s:.0}")),
                                ("luminance", &format_value(l, 0, true)),
                            ],
                        ));
                    }
                });
        }

        if self.tool == Tool::Detail {
            egui::CollapsingHeader::new(RichText::new(tr("Détail")).strong())
                .id_salt("Détail")
                .default_open(true)
                .show(ui, |ui| {
                    sub_title(ui, tr("Accentuation"));
                    let l = self.fields(ui, &SHARPEN);
                    keep(l, &mut commits);
                    if ui
                        .checkbox(&mut self.settings.detail.sharpen_deconvolution, tr("Déconvolution"))
                        .on_hover_text(tr(
                            "Défait le flou de l'objectif au lieu d'en creuser les contours : plus de vrai détail, sans halo ; visible à 100 % et à l'export",
                        ))
                        .changed()
                    {
                        commits.push(if self.settings.detail.sharpen_deconvolution {
                            tr("Accentuation par déconvolution").into()
                        } else {
                            tr("Accentuation par masque flou").into()
                        });
                    }
                    sub_title(ui, tr("Réduction du bruit"));
                    let l = self.fields(ui, &NOISE);
                    keep(l, &mut commits);
                    if ui
                        .checkbox(&mut self.settings.detail.luminance_nr_wavelets, tr("Ondelettes"))
                        .on_hover_text(tr(
                            "Lisse la luminance échelle par échelle : plus de détail gardé à force égale, environ deux fois plus lent",
                        ))
                        .changed()
                    {
                        commits.push(if self.settings.detail.luminance_nr_wavelets {
                            tr("Bruit par ondelettes").into()
                        } else {
                            tr("Bruit par filtre guidé").into()
                        });
                    }
                });
        }

        if self.tool == Tool::Geometry {
            egui::CollapsingHeader::new(RichText::new(tr("Corrections de l'objectif")).strong())
                .id_salt("Corrections de l'objectif")
                .default_open(true)
                .show(ui, |ui| {
                    let recorded = self.photo.as_ref().is_some_and(|p| p.has_camera_corrections());
                    if recorded {
                        if ui
                            .checkbox(&mut self.settings.lens.camera, tr("Corrections du boîtier"))
                            .on_hover_text(tr(
                                "La distorsion, l'aberration chromatique et le vignetage enregistrés dans le fichier par le boîtier",
                            ))
                            .changed()
                        {
                            commits.push(if self.settings.lens.camera {
                                tr("Corrections du boîtier appliquées").into()
                            } else {
                                tr("Corrections du boîtier retirées").into()
                            });
                        }
                    } else {
                        ui.label(
                            RichText::new(tr("Aucune correction enregistrée dans ce fichier"))
                                .size(11.5)
                                .color(theme::DIM),
                        );
                    }
                    if ui
                        .checkbox(
                            &mut self.settings.lens.remove_ca,
                            tr("Supprimer l'aberration chromatique"),
                        )
                        .changed()
                    {
                        commits.push(if self.settings.lens.remove_ca {
                            tr("Aberration chromatique supprimée").into()
                        } else {
                            tr("Aberration chromatique gardée").into()
                        });
                    }
                    let l = self.fields(ui, &LENS);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Geometry {
            egui::CollapsingHeader::new(RichText::new(tr("Transformation")).strong())
                .id_salt("Transformation")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .button(tr("Verticales automatiques"))
                            .on_hover_text(tr(
                                "Rend parallèles les verticales d'un bâtiment photographié en visant vers le haut (Maj+V)",
                            ))
                            .clicked()
                        {
                            self.act("auto_vertical");
                        }
                        let label = if self.guiding {
                            tr("Terminer le tracé")
                        } else {
                            tr("Redressement guidé")
                        };
                        if ui
                            .selectable_label(self.guiding, label)
                            .on_hover_text(tr(
                                "Tracez sur la photo deux à quatre lignes qui doivent être droites (arêtes, horizon) : LÜMA règle la perspective et la rotation",
                            ))
                            .clicked()
                        {
                            self.guiding = !self.guiding;
                            self.guide_start = None;
                            if !self.guiding {
                                self.guides.clear();
                            }
                            self.touch();
                        }
                    });
                    let empty = !self.settings.transform.is_identity() || self.settings.lens.distortion != 0.0;
                    if ui
                        .add_enabled(
                            empty && self.image_size[0] > 0,
                            egui::Button::new(tr("Rogner les bords vides")),
                        )
                        .on_hover_text(tr(
                            "Recadre au plus grand cadre sans bord vide, aux proportions de la photo (un recadrage ordinaire, modifiable ensuite)",
                        ))
                        .clicked()
                    {
                        self.fill_crop();
                        commits.push(tr("Bords vides rognés").into());
                    }
                    if self.guiding {
                        ui.label(
                            RichText::new(trf(
                                "{n} ligne(s) sur 4 : glissez le long d'une arête verticale ou horizontale.",
                                &[("n", &self.guides.len())],
                            ))
                            .size(11.5)
                            .color(theme::DIM),
                        );
                        if !self.guides.is_empty() && ui.button(tr("Effacer les lignes")).clicked() {
                            self.guides.clear();
                        }
                    }
                    let l = self.fields(ui, &TRANSFORM);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Detail {
            egui::CollapsingHeader::new(RichText::new(tr("Effets")).strong())
                .id_salt("Effets")
                .default_open(true)
                .show(ui, |ui| {
                    sub_title(ui, tr("Vignetage après recadrage"));
                    let l = self.fields(ui, &VIGNETTE);
                    keep(l, &mut commits);
                    sub_title(ui, tr("Grain"));
                    let l = self.fields(ui, &GRAIN);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Color {
            egui::CollapsingHeader::new(
                RichText::new(tr("Étalonnage de l'appareil photo")).strong(),
            )
            .id_salt("Étalonnage de l'appareil photo")
            .default_open(true)
            .show(ui, |ui| {
                let l = self.fields(ui, &CALIBRATION);
                keep(l, &mut commits);
            });
            self.mixer_panel(ui, &mut commits);
        }

        if self.tool == Tool::Geometry {
            let cropping = self.crop_tool.is_some();
            egui::CollapsingHeader::new(RichText::new(tr("Recadrage")).strong())
                .id_salt("Recadrage")
                .open(cropping.then_some(true))
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let label = if cropping {
                            tr("Terminer (R)")
                        } else {
                            tr("Outil Recadrage (R)")
                        };
                        if ui.selectable_label(cropping, label).clicked() {
                            self.act("crop");
                        }
                        if ui.button(tr("Réinitialiser")).clicked() {
                            self.settings.crop = Crop::default();
                            commits.push(tr("Recadrage réinitialisé").into());
                        }
                    });
                    if let Some(tool) = &mut self.crop_tool {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("Proportions")).color(theme::DIM));
                            let before = tool.aspect;
                            egui::ComboBox::from_id_salt("proportions")
                                .selected_text(tr(crop::ASPECTS[tool.aspect].0))
                                .show_ui(ui, |ui| {
                                    for (k, (name, _)) in crop::ASPECTS.iter().enumerate() {
                                        ui.selectable_value(&mut tool.aspect, k, tr(name));
                                    }
                                })
                                .response
                                .accessible_name(tr("Proportions"));
                            if tool.aspect != before
                                && let Some(photo) = &self.photo
                            {
                                let (w, h) = photo.full_size();
                                crop::constrain(
                                    &mut self.settings.crop,
                                    crop::ASPECTS[tool.aspect].1,
                                    w as f32 / h as f32,
                                );
                                self.xmp_due = Some(0.0);
                            }
                        });
                        // Le guide de composition (O ; Maj+O tourne la spirale et les
                        // triangles).
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(tr("Guide")).color(theme::DIM));
                            egui::ComboBox::from_id_salt("guide")
                                .selected_text(tr(self.crop_guide.name()))
                                .show_ui(ui, |ui| {
                                    for g in crop::Guide::ALL {
                                        ui.selectable_value(&mut self.crop_guide, g, tr(g.name()));
                                    }
                                })
                                .response
                                .on_hover_text(tr("O : guide suivant, Maj+O : tourner"))
                                .accessible_name(tr("Guide"));
                            if matches!(
                                self.crop_guide,
                                crop::Guide::Spiral | crop::Guide::Triangles
                            ) && ui.button(tr("Tourner")).clicked()
                            {
                                self.crop_turn = (self.crop_turn + 1) % 4;
                            }
                        });
                    }
                    let angle = SliderSpec {
                        label: "Angle",
                        min: -45.0,
                        max: 45.0,
                        default: 0.0,
                        decimals: 1,
                        track: Track::Plain,
                        log: false,
                    };
                    let c = widgets::slider(ui, &angle, &mut self.settings.crop.angle);
                    if c.changed {
                        self.touch();
                    }
                    if ui
                        .button(tr("Redresser automatiquement"))
                        .on_hover_text(tr("Lit l'horizon et les verticales de la photo (Maj+R)"))
                        .clicked()
                    {
                        self.act("auto_level");
                    }
                    if c.committed {
                        commits.push(trf(
                            "Angle {value}",
                            &[("value", &format_value(self.settings.crop.angle, 1, true))],
                        ));
                    }
                });
        }

        if self.tool == Tool::Light {
            ui.add_space(8.0);
            self.history_panel(ui);
            self.snapshots_panel(ui);
        }
        for label in commits {
            self.touch();
            self.commit(label);
        }
    }
}

/// Le nom d'une combinaison de masques (à traduire).
fn combine_name(c: luma_project::Combine) -> &'static str {
    match c {
        luma_project::Combine::Add => "Ajouter",
        luma_project::Combine::Subtract => "Soustraire",
        luma_project::Combine::Intersect => "Intersection",
    }
}

/// L'aide d'une combinaison de masques (à traduire).
fn combine_hint(c: luma_project::Combine) -> &'static str {
    match c {
        luma_project::Combine::Add => "Étendre le masque à une autre forme",
        luma_project::Combine::Subtract => "Retirer une forme du masque",
        luma_project::Combine::Intersect => "Ne garder que la partie commune avec une autre forme",
    }
}

/// Le nom d'une forme de masque (à traduire).
fn shape_name(s: &luma_project::Shape) -> &'static str {
    match s {
        luma_project::Shape::Linear { .. } => "Dégradé linéaire",
        luma_project::Shape::Radial { .. } => "Dégradé radial",
        luma_project::Shape::Brush { .. } => "Pinceau",
        luma_project::Shape::Full => "Image entière",
        luma_project::Shape::Sky => "Ciel",
    }
}

/// Le dégradé d'un curseur du mélangeur : la teinte voisine, la couleur désaturée ou
/// assombrie.
fn hsl_track(tab: usize, band: usize) -> &'static [Color32] {
    use std::sync::OnceLock;
    static TRACKS: OnceLock<Vec<Vec<Color32>>> = OnceLock::new();
    let tracks = TRACKS.get_or_init(|| {
        let mut all = Vec::new();
        for t in 0..3 {
            for (b, hue) in HSL_HUES.iter().enumerate() {
                let _ = b;
                let v = match t {
                    0 => vec![
                        widgets::hue_color(hue - 30.0, 0.8, 0.9),
                        widgets::hue_color(*hue, 0.8, 0.9),
                        widgets::hue_color(hue + 30.0, 0.8, 0.9),
                    ],
                    1 => vec![
                        widgets::hue_color(*hue, 0.0, 0.7),
                        widgets::hue_color(*hue, 0.9, 0.95),
                    ],
                    _ => vec![
                        widgets::hue_color(*hue, 0.8, 0.25),
                        widgets::hue_color(*hue, 0.5, 1.0),
                    ],
                };
                all.push(v);
            }
        }
        all
    });
    &tracks[tab * 8 + band]
}

/// Un bouton icône (et texte), sans cadre lourd.
fn icon_button(ui: &mut Ui, icon: Icon, label: &str, active: bool) -> egui::Response {
    let galley = (!label.is_empty()).then(|| {
        ui.painter().layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(13.0),
            theme::TEXT,
        )
    });
    let w = 30.0 + galley.as_ref().map_or(0.0, |g| g.size().x + 8.0);
    let (rect, r) = ui.allocate_exact_size(vec2(w, 30.0), egui::Sense::click());
    let p = ui.painter();
    if r.hovered() || active {
        p.rect_filled(rect, 10.0, theme::SECTION);
    }
    icons::paint(
        p,
        icon,
        Rect::from_min_size(rect.min + vec2(7.0, 7.0), vec2(16.0, 16.0)),
        if active { theme::ACCENT } else { theme::TEXT },
    );
    if let Some(g) = galley {
        p.galley(
            pos2(rect.min.x + 30.0, rect.center().y - g.size().y / 2.0),
            g,
            theme::TEXT,
        );
    }
    r
}

/// Un grand choix illustré (Empilement / Panorama, masques).
fn big_choice(ui: &mut Ui, icon: Icon, label: &str, active: bool) -> egui::Response {
    big_choice_sized(ui, icon, label, active, 96.0)
}

fn big_choice_sized(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    active: bool,
    width: f32,
) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(vec2(width, 74.0), egui::Sense::click());
    let p = ui.painter();
    p.rect_filled(
        rect,
        14.0,
        if active {
            Color32::from_rgb(0x2a, 0x24, 0x1a)
        } else {
            theme::SECTION
        },
    );
    if active {
        p.rect_stroke(
            rect,
            14.0,
            egui::Stroke::new(1.5, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    } else if r.hovered() {
        p.rect_stroke(
            rect,
            14.0,
            egui::Stroke::new(1.0, theme::DIM),
            egui::StrokeKind::Inside,
        );
    }
    let color = if active { theme::ACCENT } else { theme::TEXT };
    icons::paint(
        p,
        icon,
        Rect::from_center_size(rect.center() - vec2(0.0, 10.0), vec2(30.0, 30.0)),
        color,
    );
    p.text(
        pos2(rect.center().x, rect.max.y - 10.0),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::proportional(12.5),
        color,
    );
    r
}

/// Les étoiles et le drapeau en bas d'une vignette.
/// Les images de la marque (luma-brand), chargées une fois.
struct Brand {
    backdrop: TextureHandle,
    ring: TextureHandle,
    logo: TextureHandle,
}

impl Brand {
    fn new(ctx: &egui::Context) -> Brand {
        let tex = |name: &str, w: usize, h: usize, rgba: Vec<u8>| {
            ctx.load_texture(
                name,
                ColorImage::from_rgba_unmultiplied([w, h], &rgba),
                TextureOptions::LINEAR,
            )
        };
        Brand {
            backdrop: tex("fond", 960, 600, luma_brand::backdrop(960, 600)),
            ring: tex(
                "anneau",
                96,
                96,
                luma_brand::logo(96, luma_brand::Backdrop::None, false),
            ),
            logo: tex(
                "logo",
                512,
                512,
                luma_brand::logo(512, luma_brand::Backdrop::None, true),
            ),
        }
    }
}

/// Un film du panneau Films.
enum Film<'a> {
    Builtin(&'static presets::Preset),
    Lut(&'a str),
    Mine(&'a str, &'a Settings),
}

/// Le signe « modifiée » d'une vignette : un point spectral en haut à gauche et, en bas,
/// le nom du film (ou « Modifiée »).
fn paint_edits(painter: &egui::Painter, rect: egui::Rect, e: &presets::Summary, large: bool) {
    if !e.is_edited() {
        return;
    }
    let r = if large { 5.0 } else { 4.0 };
    let c = rect.left_top() + vec2(r + 5.0, r + 5.0);
    painter.circle_filled(c, r + 1.5, Color32::from_black_alpha(150));
    widgets::spectral_ring(painter, c, r, 2.0, 1.0);
    let label = match &e.film {
        Some((name, _)) => tr(name).to_owned(),
        None => tr("Modifiée").to_owned(),
    };
    let size = if large { 11.0 } else { 9.5 };
    if rect.height() < 30.0 {
        return;
    }
    let band = egui::Rect::from_min_max(pos2(rect.min.x, rect.max.y - size - 6.0), rect.max);
    painter.rect_filled(
        band,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: 4,
            se: 4,
        },
        Color32::from_black_alpha(150),
    );
    painter.with_clip_rect(band).text(
        pos2(band.min.x + 5.0, band.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(size),
        theme::TEXT,
    );
}

/// Le résumé des modifications, traduit : « Portra 400 · 80 % — Lumière, Couleur, 2
/// masques » (comme `Summary::text`).
fn summary_text(e: &presets::Summary) -> String {
    let mut parts = Vec::new();
    if let Some((name, amount)) = &e.film {
        let name = tr(name);
        parts.push(if *amount >= 99.5 {
            name.to_owned()
        } else {
            trf(
                "{name} · {n} %",
                &[("name", &name), ("n", &format_value(*amount, 0, false))],
            )
        });
    }
    let mut rest: Vec<String> = e.panels.iter().map(|p| tr(p).to_owned()).collect();
    if e.masks > 0 {
        let mut masks = if e.masks > 1 {
            trf("{n} masques", &[("n", &e.masks)])
        } else {
            tr("1 masque").to_owned()
        };
        if e.mask_films > 0 {
            masks += " ";
            masks += &trf("({n} avec film)", &[("n", &e.mask_films)]);
        }
        rest.push(masks);
    }
    if !rest.is_empty() {
        parts.push(rest.join(", "));
    }
    parts.join(" — ")
}

/// Une taille en octets, lisible : « 3,2 Go », « 640 Mo ».
fn format_bytes(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        format!(
            "{} {}",
            format_value((mb / 1024.0) as f32, 1, false),
            tr("Go")
        )
    } else {
        format!("{} {}", format_value(mb as f32, 0, false), tr("Mo"))
    }
}

/// Les préréglages `.xmp` d'un dossier (nom tiré du fichier s'il n'en a pas), triés.
fn load_user_presets(dir: &Path) -> Vec<(String, Settings)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<(String, Settings)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xmp")))
        .filter_map(|p| {
            let (name, s) =
                luma_project::xmp::read_preset(&std::fs::read_to_string(&p).ok()?).ok()?;
            Some((
                name.unwrap_or_else(|| {
                    p.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default()
                }),
                s,
            ))
        })
        .collect();
    v.sort_by_key(|a| a.0.to_lowercase());
    v
}

/// Un nom de fichier sûr pour un préréglage.
fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '-'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// La vue de la comparaison ne sort pas de la photo.
fn clamp_center(c: &mut Compare) {
    let half = 0.5 / c.zoom;
    c.center.x = c.center.x.clamp(half, 1.0 - half);
    c.center.y = c.center.y.clamp(half, 1.0 - half);
}

/// Les informations de la photo : nom, libellé, sur plusieurs lignes, et le champ.
type InfoField = (
    &'static str,
    &'static str,
    bool,
    fn(&mut Meta) -> &mut String,
);
const INFO_FIELDS: &[InfoField] = &[
    ("title", "Titre", false, |m| &mut m.title),
    ("caption", "Légende", true, |m| &mut m.caption),
    ("creator", "Auteur", false, |m| &mut m.creator),
    ("rights", "Droits", false, |m| &mut m.rights),
    ("sublocation", "Lieu", false, |m| {
        &mut m.location.sublocation
    }),
    ("city", "Ville", false, |m| &mut m.location.city),
    ("state", "Région", false, |m| &mut m.location.state),
    ("country", "Pays", false, |m| &mut m.location.country),
];

/// Le fichier des modèles de métadonnées, dans le dossier des préréglages.
const TEMPLATES_FILE: &str = "Modèles de métadonnées.json";

fn load_templates(path: &Path) -> Vec<luma_project::xmp::Template> {
    luma_project::xmp::load_templates(path)
}

/// Le fichier des réglages par défaut, dans le dossier des préréglages.
const DEFAULTS_FILE: &str = "Réglages par défaut.json";

/// Le boîtier, marque comprise (« Sony ILCE-7M3 »), comme dans la photothèque.
fn camera_name(make: &str, model: &str) -> String {
    if model.starts_with(make) {
        model.to_owned()
    } else {
        format!("{make} {model}").trim().to_owned()
    }
}

/// Les cinq pastilles de libellé ; rend celle cliquée (un clic sur la pastille choisie
/// retire le libellé).
fn label_dots(ui: &mut Ui, label: &str) -> Option<usize> {
    let current = luma_project::xmp::label_color(label);
    let mut clicked = None;
    for (i, color) in LABEL_COLORS.iter().enumerate() {
        let (rect, r) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::click());
        let on = current == Some(i);
        let painter = ui.painter();
        painter.circle_filled(rect.center(), if on { 7.0 } else { 5.5 }, *color);
        if on {
            painter.circle_stroke(rect.center(), 8.5, egui::Stroke::new(1.5, Color32::WHITE));
        }
        let key = if i < 4 {
            format!(" ({})", i + 6)
        } else {
            String::new()
        };
        if r.on_hover_text(format!("{}{key}", tr(LABEL_NAMES[i])))
            .clicked()
        {
            clicked = Some(i);
        }
    }
    clicked
}

fn paint_meta(painter: &egui::Painter, rect: egui::Rect, meta: &Meta) {
    let y = rect.max.y - 13.0;
    if let Some(color) = label_color32(&meta.label) {
        painter.circle_filled(pos2(rect.max.x - 11.0, rect.min.y + 11.0), 5.0, color);
    }
    for k in 0..meta.rating {
        let c = pos2(rect.min.x + 14.0 + f32::from(k) * 13.0, y);
        widgets::paint_star(painter, c, 5.5, Color32::from_gray(0xe8));
    }
    let flag = match meta.flag {
        Flag::Pick => Some(Color32::WHITE),
        Flag::Reject => Some(Color32::from_rgb(0xe0, 0x50, 0x50)),
        Flag::None => None,
    };
    if let Some(color) = flag {
        let r = egui::Rect::from_center_size(pos2(rect.max.x - 16.0, y), vec2(10.0, 10.0));
        painter.rect_filled(r, 2.0, color);
        if meta.flag == Flag::Reject {
            painter.line_segment(
                [
                    r.left_top() + vec2(2.0, 2.0),
                    r.right_bottom() - vec2(2.0, 2.0),
                ],
                egui::Stroke::new(1.5, Color32::BLACK),
            );
        }
    }
}

/// Une fenêtre de réglages (Exporter…) : au centre de l'écran ; sur iPhone, tout l'écran
/// utile, sur un fond opaque (rien du panneau ne se lit au travers).
fn place_window<'a>(
    ctx: &egui::Context,
    window: egui::Window<'a>,
    compact: bool,
) -> egui::Window<'a> {
    if compact {
        let style = ctx.global_style();
        window
            .fixed_rect(ctx.content_rect().shrink(8.0))
            .frame(egui::Frame::window(&style).fill(Color32::from_rgb(0x14, 0x14, 0x1a)))
    } else {
        window.anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
    }
}

/// Le titre d'une ligne de formulaire : à gauche dans une grille, ou au‑dessus du
/// réglage sur iPhone (rien pour un titre vide).
fn form_row(ui: &mut Ui, compact: bool, title: &str) {
    if !compact {
        ui.label(title);
    } else if !title.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new(title).size(12.0).color(theme::DIM));
    }
}

/// La fin d'une ligne de formulaire (voir [`form_row`]).
fn form_end(ui: &mut Ui, compact: bool) {
    if compact {
        ui.add_space(6.0);
    } else {
        ui.end_row();
    }
}

fn section_title(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).size(11.0).color(theme::DIM).strong());
    ui.add_space(4.0);
}

fn sub_title(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(11.5).color(theme::DIM));
}

fn file_name(p: &Path) -> String {
    let name = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    match luma_project::copies::number(p) {
        Some(n) => trf(
            "{file} (copie {n})",
            &[("file", &name(&luma_project::copies::source(p))), ("n", &n)],
        ),
        None => name(p),
    }
}

/// Le rectangle de taille `w × h` qui tient dans `area`, centré.
/// Le moment d'une transition du diaporama : `t` va de 0 (la photo arrive) à 1 (elle
/// est en place).
struct SlideFrame {
    transition: luma_render::slideshow::Transition,
    t: f32,
    /// Le temps d'une photo, fondus compris (la durée du zoom lent).
    photo_time: f32,
    now: std::time::Instant,
    area: Rect,
}

impl SlideFrame {
    /// La place et l'opacité d'une photo : celle qui part (`leaving`) ou celle qui
    /// arrive.
    fn place(
        &self,
        size: [usize; 2],
        arrived: std::time::Instant,
        count: usize,
        leaving: bool,
    ) -> (Rect, u8) {
        use luma_render::slideshow::Transition;
        let rect = fit(self.area, size[0], size[1]);
        let t = self.t;
        match self.transition {
            Transition::Fade => (rect, alpha(if leaving { 1.0 } else { t })),
            Transition::Slide => {
                // Lent au départ et à l'arrivée, comme dans le navigateur.
                let e = t * t * (3.0 - 2.0 * t);
                let shift = self.area.width() * if leaving { -e } else { 1.0 - e };
                (rect.translate(vec2(shift, 0.0)), 255)
            }
            Transition::Drift => {
                let u = ((self.now - arrived).as_secs_f32() / self.photo_time).min(1.0);
                let scale = 1.0 + 0.08 * u;
                // Vers le haut à gauche, puis vers le bas à droite, une photo sur deux.
                let (ox, oy) = if count % 2 == 1 {
                    (0.7, 0.6)
                } else {
                    (0.3, 0.4)
                };
                let origin = pos2(
                    rect.min.x + rect.width() * ox,
                    rect.min.y + rect.height() * oy,
                );
                let zoomed = Rect::from_min_max(
                    origin + (rect.min - origin) * scale,
                    origin + (rect.max - origin) * scale,
                );
                (zoomed, alpha(if leaving { 1.0 } else { t }))
            }
        }
    }
}

fn alpha(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn fit(area: egui::Rect, w: usize, h: usize) -> egui::Rect {
    if w == 0 || h == 0 {
        return area;
    }
    let s = (area.width() / w as f32).min(area.height() / h as f32);
    egui::Rect::from_center_size(area.center(), vec2(w as f32 * s, h as f32 * s))
}

/// Les métadonnées d'un export : ce qui est recopié du RAW, l'auteur et le copyright.
fn metadata_editor(ui: &mut Ui, m: &mut luma_render::export::MetadataOptions) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut m.copy_exif, tr("EXIF de la prise de vue"));
        ui.add_enabled(
            m.copy_exif,
            egui::Checkbox::new(&mut m.keep_gps, tr("Position GPS")),
        );
        ui.checkbox(&mut m.keywords, tr("Mots-clés et note"));
    });
    let field = |ui: &mut Ui, value: &mut Option<String>, hint: &str, name: &str| {
        let mut text = value.clone().unwrap_or_default();
        if ui
            .add(
                egui::TextEdit::singleline(&mut text)
                    .hint_text(hint)
                    .desired_width(260.0),
            )
            .accessible_name(name)
            .changed()
        {
            *value = Some(text.trim().to_owned()).filter(|t| !t.is_empty());
        }
    };
    field(ui, &mut m.author, tr("Auteur"), tr("Auteur"));
    field(
        ui,
        &mut m.copyright,
        tr("© 2026 votre nom"),
        tr("Copyright"),
    );
}

/// Le logo en filigrane d'un export : le fichier PNG, puis son emplacement, sa taille
/// et son opacité.
fn logo_editor(ui: &mut Ui, logo: &mut Option<luma_render::export::Logo>) {
    use luma_render::export::{Logo, Position};
    ui.horizontal(|ui| {
        let name = logo.as_ref().map_or_else(
            || tr("Aucun").to_owned(),
            |l| {
                l.path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            },
        );
        ui.label(RichText::new(name).color(theme::DIM));
        if crate::dialogs::FOLDER_PICKER
            && ui.button(tr("Choisir un logo")).clicked()
            && let Some(path) = crate::dialogs::pick_logo()
        {
            *logo = Some(match logo.take() {
                Some(l) => Logo { path, ..l },
                None => Logo::new(path),
            });
        }
        if logo.is_some() && ui.button(tr("Retirer")).clicked() {
            *logo = None;
        }
    });
    let Some(l) = logo else {
        return;
    };
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("logo-position")
            .selected_text(tr(l.position.label()))
            .show_ui(ui, |ui| {
                for p in Position::ALL {
                    ui.selectable_value(&mut l.position, p, tr(p.label()));
                }
            })
            .response
            .accessible_name(tr("Emplacement du logo"));
        ui.add(
            egui::DragValue::new(&mut l.size)
                .range(1.0..=50.0)
                .speed(0.2)
                .suffix(" %"),
        )
        .on_hover_text(tr("Taille, en pour cent du petit côté"))
        .accessible_name(tr("Taille du logo"));
        let mut opacity = (l.opacity * 100.0).round();
        if ui
            .add(
                egui::DragValue::new(&mut opacity)
                    .range(0.0..=100.0)
                    .suffix(" %"),
            )
            .on_hover_text(tr("Opacité"))
            .accessible_name(tr("Opacité du logo"))
            .changed()
        {
            l.opacity = opacity / 100.0;
        }
    });
}

/// Le filigrane texte d'un export : le texte (vide : aucun), puis son emplacement, sa
/// taille, son opacité et sa couleur.
fn watermark_editor(ui: &mut Ui, watermark: &mut Option<luma_render::export::Watermark>) {
    use luma_render::export::{Position, Watermark};
    let mut text = watermark
        .as_ref()
        .map(|w| w.text.clone())
        .unwrap_or_default();
    let edit = ui
        .add(
            egui::TextEdit::singleline(&mut text)
                .hint_text(tr("© votre nom"))
                .desired_width(260.0),
        )
        .accessible_name(tr("Filigrane"));
    if edit.changed() {
        *watermark = if text.trim().is_empty() {
            None
        } else {
            Some(match watermark.take() {
                Some(w) => Watermark { text, ..w },
                None => Watermark::new(&text),
            })
        };
    }
    let Some(w) = watermark else {
        return;
    };
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("filigrane-position")
            .selected_text(tr(w.position.label()))
            .show_ui(ui, |ui| {
                for p in Position::ALL {
                    ui.selectable_value(&mut w.position, p, tr(p.label()));
                }
            })
            .response
            .accessible_name(tr("Emplacement du filigrane"));
        ui.add(
            egui::DragValue::new(&mut w.size)
                .range(0.5..=25.0)
                .speed(0.1)
                .suffix(" %"),
        )
        .on_hover_text(tr("Taille, en pour cent du petit côté"))
        .accessible_name(tr("Taille du filigrane"));
        let mut opacity = (w.opacity * 100.0).round();
        if ui
            .add(
                egui::DragValue::new(&mut opacity)
                    .range(0.0..=100.0)
                    .suffix(" %"),
            )
            .on_hover_text(tr("Opacité"))
            .accessible_name(tr("Opacité du filigrane"))
            .changed()
        {
            w.opacity = opacity / 100.0;
        }
        ui.checkbox(&mut w.dark, tr("Noir"));
    });
}

/// L'accentuation de sortie d'un export : le support, puis l'intensité.
fn sharpen_editor(ui: &mut Ui, sharpen: &mut Option<luma_render::export::OutputSharpen>) {
    use luma_render::export::{Medium, OutputSharpen, Strength};
    let media = [
        (None, tr("Aucune")),
        (Some(Medium::Screen), tr("Écran")),
        (Some(Medium::Matte), tr("Papier mat")),
        (Some(Medium::Glossy), tr("Papier brillant")),
    ];
    let current = sharpen.map(|s| s.medium);
    let mut chosen = current;
    egui::ComboBox::from_id_salt("accentuation-sortie")
        .selected_text(media.iter().find(|m| m.0 == current).map_or("", |m| m.1))
        .show_ui(ui, |ui| {
            for (m, name) in media {
                ui.selectable_value(&mut chosen, m, name);
            }
        })
        .response
        .accessible_name(tr("Accentuation de sortie"));
    if chosen != current {
        *sharpen = chosen.map(|medium| OutputSharpen {
            medium,
            strength: sharpen.map_or(Strength::Standard, |s| s.strength),
        });
    }
    if let Some(s) = sharpen {
        for (k, name) in [
            (Strength::Low, tr("Faible")),
            (Strength::Standard, tr("Normale")),
            (Strength::High, tr("Forte")),
        ] {
            ui.radio_value(&mut s.strength, k, name);
        }
    }
}

/// Un nombre de pixels d'export.
fn pixels(v: &mut usize) -> egui::DragValue<'_> {
    egui::DragValue::new(v).range(64..=20000).suffix(" px")
}

/// Les dimensions d'un export : pleine définition, grand ou petit côté, cadre,
/// mégapixels ; le nombre garde sa valeur quand on change de mode.
fn resize_editor(ui: &mut Ui, resize: &mut Option<luma_render::export::Resize>) {
    use luma_render::export::Resize;
    let modes = [
        tr("Pleine définition"),
        tr("Grand côté"),
        tr("Petit côté"),
        tr("Largeur × hauteur"),
        tr("Mégapixels"),
        tr("Tirage (cm)"),
    ];
    let mode = match resize {
        None => 0,
        Some(Resize::LongEdge(_)) => 1,
        Some(Resize::ShortEdge(_)) => 2,
        Some(Resize::Fit { .. }) => 3,
        Some(Resize::Megapixels(_)) => 4,
        Some(Resize::Print { .. }) => 5,
    };
    let mut chosen = mode;
    egui::ComboBox::from_id_salt("dimensions")
        .selected_text(modes[mode])
        .show_ui(ui, |ui| {
            for (k, name) in modes.iter().enumerate() {
                ui.selectable_value(&mut chosen, k, *name);
            }
        })
        .response
        .accessible_name(tr("Dimensions"));
    // Le nombre affiché, d'un mode à l'autre.
    let edge = match *resize {
        Some(Resize::LongEdge(n) | Resize::ShortEdge(n)) => n,
        Some(Resize::Fit { width, .. }) => width,
        _ => 2048,
    };
    if chosen != mode {
        *resize = match chosen {
            0 => None,
            1 => Some(Resize::LongEdge(edge)),
            2 => Some(Resize::ShortEdge(edge.min(8000))),
            3 => Some(Resize::Fit {
                width: edge,
                height: edge,
            }),
            4 => Some(Resize::Megapixels(12.0)),
            _ => Some(Resize::Print {
                width_cm: 30.0,
                height_cm: 40.0,
                dpi: 300,
            }),
        };
    }
    match resize {
        None => {}
        Some(Resize::LongEdge(n)) => {
            ui.add(pixels(n))
                .accessible_name(tr("Grand côté en pixels"));
        }
        Some(Resize::ShortEdge(n)) => {
            ui.add(pixels(n))
                .accessible_name(tr("Petit côté en pixels"));
        }
        Some(Resize::Fit { width, height }) => {
            ui.add(pixels(width))
                .accessible_name(tr("Largeur en pixels"));
            ui.label("×");
            ui.add(pixels(height))
                .accessible_name(tr("Hauteur en pixels"));
        }
        Some(Resize::Megapixels(m)) => {
            ui.add(
                egui::DragValue::new(m)
                    .range(0.1..=200.0)
                    .speed(0.1)
                    .suffix(" Mpx"),
            )
            .accessible_name(tr("Mégapixels"));
        }
        Some(Resize::Print {
            width_cm,
            height_cm,
            dpi,
        }) => {
            fn cm(v: &mut f32) -> egui::DragValue<'_> {
                // Les décimales comme ailleurs dans l'app (virgule en français).
                egui::DragValue::new(v)
                    .range(1.0..=500.0)
                    .speed(0.5)
                    .custom_formatter(|v, _| format!("{} cm", format_value(v as f32, 1, false)))
                    .custom_parser(|t| {
                        t.trim()
                            .trim_end_matches("cm")
                            .trim()
                            .replace(',', ".")
                            .parse()
                            .ok()
                    })
            }
            ui.add(cm(width_cm))
                .accessible_name(tr("Largeur du tirage en centimètres"));
            ui.label("×");
            ui.add(cm(height_cm))
                .accessible_name(tr("Hauteur du tirage en centimètres"));
            ui.add(
                egui::DragValue::new(dpi)
                    .range(72..=1200)
                    .speed(1.0)
                    .suffix(" ppp"),
            )
            .accessible_name(tr("Points par pouce"));
        }
    }
}

/// Le poids maximal d'un JPEG : une case, puis le poids en kilo‑octets (500 par
/// défaut, assez pour la plupart des sites).
fn max_weight_editor(ui: &mut Ui, max_kb: &mut Option<u32>) {
    let mut on = max_kb.is_some();
    if ui
        .checkbox(&mut on, tr("Au plus"))
        .on_hover_text(tr(
            "La meilleure qualité qui tient dans ce poids, métadonnées comprises : pour un site ou un formulaire qui limite la taille des fichiers",
        ))
        .changed()
    {
        *max_kb = on.then_some(500);
    }
    if let Some(k) = max_kb {
        ui.add(
            egui::DragValue::new(k)
                .range(20..=100_000)
                .speed(10.0)
                .suffix(format!(" {}", tr("Ko"))),
        )
        .accessible_name(tr("Poids maximal en kilo-octets"));
    }
}

/// Le résumé d'un préréglage d'export dans la langue de l'interface : format, espace,
/// taille.
fn export_summary(o: &ExportOptions) -> String {
    use luma_render::export::Resize;
    let format = match o.format {
        Format::Jpeg => match o.max_kb {
            Some(k) => trf("JPEG {q}, {k} Ko au plus", &[("q", &o.quality), ("k", &k)]),
            None => format!("JPEG {}", o.quality),
        },
        Format::Png => format!("PNG {} bits", o.bits),
        Format::Tiff => format!("TIFF {} bits", o.bits),
    };
    let space = match o.space {
        OutputSpace::Srgb => "sRGB",
        OutputSpace::DisplayP3 => "Display P3",
        OutputSpace::Rec2020 => "Rec. 2020",
        OutputSpace::ProPhoto => "ProPhoto RGB",
    };
    let size = match o.resize {
        None => tr("Pleine définition").to_owned(),
        Some(Resize::LongEdge(n)) => format!("{n} px"),
        Some(Resize::ShortEdge(n)) => trf("petit côté {n} px", &[("n", &n)]),
        Some(Resize::Fit { width, height }) => format!("{width} × {height} px"),
        Some(Resize::Megapixels(m)) => format!("{m} Mpx"),
        Some(Resize::Print {
            width_cm,
            height_cm,
            dpi,
        }) => trf(
            "{w} × {h} cm à {dpi} ppp",
            &[
                ("w", &format_value(width_cm, 1, false)),
                ("h", &format_value(height_cm, 1, false)),
                ("dpi", &dpi),
            ],
        ),
    };
    if o.enlarge && o.resize.is_some() {
        return format!("{format}, {space}, {size}, {}", tr("agrandie au besoin"));
    }
    format!("{format}, {space}, {size}")
}

/// Le modèle du nom des fichiers exportés est‑il valable ?
fn valid_name(template: &str) -> bool {
    luma_render::export::check_template(template).is_ok()
}

/// Un texte centré sous `top`, revenu à la ligne au‑delà de `width` ; rend sa place.
fn centered_text(painter: &egui::Painter, top: egui::Pos2, width: f32, text: String) -> Rect {
    let mut job = egui::text::LayoutJob::simple(
        text,
        egui::FontId::proportional(15.0),
        theme::DIM,
        width.max(100.0),
    );
    job.halign = egui::Align::Center;
    let galley = painter.layout_job(job);
    let rect = galley.rect.translate(top.to_vec2());
    painter.galley(top, galley, theme::DIM);
    rect
}

#[cfg(test)]
mod tests {
    use luma_render::slideshow::Transition;

    #[test]
    fn slideshow_transitions_move_the_photos() {
        let area = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 500.0));
        let start = std::time::Instant::now();
        let frame = |transition, t, now| SlideFrame {
            transition,
            t,
            photo_time: 6.0,
            now,
            area,
        };
        // Glissement : la photo qui arrive vient de la droite, celle qui part sort à gauche.
        let f = frame(Transition::Slide, 0.0, start);
        assert_eq!(f.place([200, 100], start, 0, false).0.min.x, 1000.0);
        assert_eq!(f.place([200, 100], start, 0, true).0.min.x, 0.0);
        let f = frame(Transition::Slide, 0.5, start);
        assert_eq!(f.place([200, 100], start, 0, false).0.min.x, 500.0);
        assert_eq!(f.place([200, 100], start, 0, true).0.min.x, -500.0);
        let f = frame(Transition::Slide, 1.0, start);
        assert_eq!(f.place([200, 100], start, 0, false), (area, 255));
        // Fondu : la photo qui arrive apparaît sur celle qui part, sans bouger.
        let f = frame(Transition::Fade, 0.5, start);
        assert_eq!(f.place([200, 100], start, 0, false), (area, 128));
        assert_eq!(f.place([200, 100], start, 0, true), (area, 255));
        // Zoom lent : 8 % sur tout le temps de la photo, vers un côté puis l'autre.
        let later = start + std::time::Duration::from_secs(6);
        let f = frame(Transition::Drift, 1.0, later);
        let (even, _) = f.place([200, 100], start, 0, false);
        let (odd, _) = f.place([200, 100], start, 1, false);
        assert!((even.width() - 1080.0).abs() < 0.01 && (odd.width() - 1080.0).abs() < 0.01);
        assert!((even.min.x - -24.0).abs() < 0.01 && (odd.min.x - -56.0).abs() < 0.01);
        let f = frame(Transition::Drift, 1.0, start);
        assert_eq!(f.place([200, 100], start, 0, false).0, area);
    }

    use super::*;

    #[test]
    fn raw_extensions() {
        assert!(is_raw(Path::new("a/IMG_1.CR3")));
        assert!(is_raw(Path::new("b.dng")));
        assert!(!is_raw(Path::new("c.jpg")));
        assert!(is_photo(Path::new("c.JPG")) && is_photo(Path::new("d.webp")));
        assert!(!is_photo(Path::new("e.txt")));
    }

    #[test]
    fn defaults_of_the_fields_match_the_settings() {
        let mut s = Settings::default();
        for f in TONE
            .iter()
            .chain(&LENS)
            .chain(&TRANSFORM)
            .chain(&PRESENCE)
            .chain(&CURVE)
            .chain(&SHARPEN)
            .chain(&NOISE)
            .chain(&VIGNETTE)
            .chain(&GRAIN)
            .chain(&CALIBRATION)
        {
            assert_eq!(*(f.get)(&mut s), f.spec.default, "{}", f.spec.label);
        }
    }

    /// Les textes des tableaux constants sont traduits à l'affichage : ils doivent être
    /// dans le catalogue (les `tr("…")` du code sont vérifiés par `tests/i18n.rs`).
    #[test]
    fn constant_texts_are_in_the_catalog() {
        let c = crate::i18n::catalog();
        let fields = TONE
            .iter()
            .chain(&LENS)
            .chain(&TRANSFORM)
            .chain(&PRESENCE)
            .chain(&CURVE)
            .chain(&SHARPEN)
            .chain(&NOISE)
            .chain(&VIGNETTE)
            .chain(&GRAIN)
            .chain(&CALIBRATION)
            .map(|f| f.spec.label);
        let texts: Vec<&str> = fields
            .chain(HSL_NAMES)
            .chain(WB_PRESETS.iter().map(|p| p.0))
            .chain(["Telle quelle", "Auto", "Personnalisée"])
            .chain(crop::ASPECTS.iter().map(|a| a.0))
            .chain(crate::licence::FEATURES)
            .chain(Tool::ALL.map(Tool::key))
            .filter(|t| !t.is_empty() && !t.chars().all(|c| !c.is_alphabetic()))
            .collect();
        let missing: Vec<&&str> = texts
            .iter()
            .filter(|t| !c.has(t) && !NOT_TRANSLATED.contains(t))
            .collect();
        assert!(missing.is_empty(), "absents du catalogue : {missing:?}");
    }

    /// Les mots identiques dans les trois langues (et les touches seules).
    const NOT_TRANSLATED: [&str; 3] = ["G", "D", "R"];

    #[test]
    fn a_gesture_renders_a_smaller_preview() {
        // Un écran d'iPad Pro (2732 px) : pendant le geste, au plus 1536 px ici (1024
        // sur iPhone et iPad) ; un petit écran garde sa taille.
        let cap = if cfg!(target_os = "ios") { 1024 } else { 1536 };
        assert_eq!(interactive_size(2732.0), Size::Fit(cap));
        assert_eq!(interactive_size(700.0), Size::Fit(1024));
    }

    #[test]
    fn fit_keeps_the_aspect_ratio() {
        let r = fit(
            egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0)),
            400,
            400,
        );
        assert_eq!(r.size(), vec2(100.0, 100.0));
        assert_eq!(r.center(), pos2(100.0, 50.0));
    }
}
