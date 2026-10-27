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
    Color32, ColorImage, Key, Modifiers, Rect, RichText, TextureHandle, TextureOptions, Ui, pos2,
    vec2,
};
use luma_color::OutputSpace;
use luma_library::query::fold;
use luma_project::presets::{self, PRESETS};
use luma_project::xmp::{Flag, Meta};
use luma_project::{Crop, Mask, Profile, Range, Settings, Shape, WhiteBalance};
use luma_render::{Photo, Size};

use crate::crop;
use crate::i18n::{tr, trf};
use crate::icons::{self, Icon};
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

/// Un écran tactile sans clavier (iPhone, iPad) : toucher pour ouvrir, glisser d'une
/// photo à l'autre, toucher longuement pour choisir ; pas de raccourcis à annoncer.
const TOUCH: bool = cfg!(target_os = "ios");

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
    /// La fenêtre des groupes sert à synchroniser la sélection, pas à coller.
    syncing: bool,
    /// Les photos du lot dont la balance automatique est attendue.
    auto_pending: usize,
    paste_groups: Vec<luma_project::paste::Group>,
    texture: Option<TextureHandle>,
    image_size: [usize; 2],
    histogram: [[u32; 256]; 3],
    generation: u64,
    shown: u64,
    last_request: Option<Request>,
    render_ms: f32,
    render_gpu: bool,
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
    /// Jusqu'à quand guetter les photos d'un import Photos (il en arrive une à une).
    imports_until: f64,
    /// Les vignettes libérées faute de mémoire : redemandées quand elles réapparaissent.
    evicted: std::collections::HashSet<PathBuf>,
    /// Les vignettes affichées sans image pendant cette image de l'interface.
    shown_missing: std::cell::RefCell<Vec<PathBuf>>,
    status: String,
    xmp_due: Option<f64>,
    grading_tab: usize,
    hsl_tab: usize,
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
    export_options: ExportOptions,
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
    /// La couche de sortie montrée par le mélangeur de couches (0 rouge, 1 vert, 2 bleu).
    mixer_tab: usize,
    /// L'outil de retouche actif (Q) : un clic sur la photo pose une retouche.
    pub retouch: Option<luma_project::SpotKind>,
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
    merge_stack: bool,
    stack_mode: luma_merge::StackMode,
    stack_align: bool,
    projection: luma_merge::Projection,
    auto_crop: bool,
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
            syncing: false,
            auto_pending: 0,
            paste_groups: luma_project::paste::default_groups(),
            texture: None,
            image_size: [0, 0],
            histogram: [[0; 256]; 3],
            generation: 0,
            shown: 0,
            last_request: None,
            render_ms: 0.0,
            render_gpu: false,
            before: false,
            clipping: false,
            proofing: Proofing::default(),
            zoom: false,
            compare: None,
            stacking: true,
            prints: HashMap::new(),
            duplicates: None,
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
            imports_until: 0.0,
            evicted: Default::default(),
            shown_missing: Default::default(),
            // Vide : l'accueil (traduit à l'affichage, voir `status_text`).
            status: String::new(),
            xmp_due: None,
            grading_tab: 0,
            hsl_tab: 0,
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
            mixer_tab: 0,
            retouch: None,
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
        self.catalog_path = Some(path);
        self.refresh_catalog_lists();
    }

    /// Le catalogue, en lecture.
    pub fn catalog(&self) -> &luma_library::Catalog {
        &self.catalog
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
                // Les photos d'un disque débranché ne s'affichent pas.
                let photos = photos.into_iter().filter(|p| p.is_file()).collect();
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
    }

    /// Les photos choisies, sauf celle ouverte (la source d'une synchronisation).
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
        let words: Vec<String> = text
            .split([',', ';'])
            .map(str::trim)
            .filter(|w| !w.is_empty())
            .map(str::to_owned)
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
                            let defaults = photo.default_settings();
                            self.settings = sidecar.map(|s| *s).unwrap_or_else(|| defaults.clone());
                            self.history = vec![(tr("Importation").into(), defaults)];
                            if from_xmp {
                                self.history
                                    .push((tr("Réglages XMP").into(), self.settings.clone()));
                            }
                            self.history_pos = self.history.len() - 1;
                            self.selected_mask = None;
                            self.preset = None;
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
                    if frame.generation < self.shown {
                        continue;
                    }
                    self.shown = frame.generation;
                    self.render_ms = frame.millis;
                    self.render_gpu = frame.gpu;
                    self.histogram = frame.histogram;
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
                                "Pellicule mesurée : « {name} » (écart numérique → film {before}, avec le film {after})",
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
                    self.status = match result {
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
        self.history.truncate(self.history_pos + 1);
        if self
            .history
            .last()
            .is_some_and(|(_, s)| *s == self.settings)
        {
            return;
        }
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
            .on_hover_text(tr("Langue et cache"))
            .accessible_name(tr("Langue et cache"));
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
            .map_or_else(Settings::default, |p| p.default_settings())
    }

    fn request_render(&mut self, viewport_px: f32, pressed: bool) {
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
        let request = Request {
            path: path.to_path_buf(),
            settings,
            size,
            clipping: self.clipping,
            proof: self
                .proofing
                .active()
                .map(|p| (self.proofing.version, p.simulate, p.gamut)),
            scene: self.retouch.is_some() || self.selected_spot.is_some(),
            dust_view: self.show_dust && self.tool == Tool::Masks,
            find_dust: self.dust_pending,
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
            },
            self.proofing.active(),
        );
        self.last_request = Some(request);
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let typing = ctx.memory(|m| m.focused().is_some());
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let mut action: Option<&str> = None;
        ctx.input_mut(|i| {
            if i.consume_key(cmd_shift, Key::Z) {
                action = Some("redo");
            } else if i.consume_key(Modifiers::COMMAND, Key::Z) {
                action = Some("undo");
            } else if i.consume_key(cmd_shift, Key::C) {
                action = Some("copy");
            } else if i.consume_key(cmd_shift | Modifiers::ALT, Key::V) {
                action = Some("paste_some");
            } else if i.consume_key(cmd_shift, Key::V) {
                action = Some("paste");
            } else if i.consume_key(cmd_shift, Key::S) {
                action = Some("sync");
            } else if i.consume_key(cmd_shift, Key::R) {
                action = Some("reset");
            } else if i.consume_key(cmd_shift, Key::E) {
                action = Some("export");
            } else if i.consume_key(Modifiers::COMMAND, Key::O) {
                action = Some("open");
            } else if !typing {
                let digits = [
                    Key::Num0,
                    Key::Num1,
                    Key::Num2,
                    Key::Num3,
                    Key::Num4,
                    Key::Num5,
                ];
                let labels = [Key::Num6, Key::Num7, Key::Num8, Key::Num9];
                if let Some(n) = digits
                    .iter()
                    .position(|k| i.consume_key(Modifiers::NONE, *k))
                {
                    action = Some(["rate0", "rate1", "rate2", "rate3", "rate4", "rate5"][n]);
                } else if let Some(n) = labels
                    .iter()
                    .position(|k| i.consume_key(Modifiers::NONE, *k))
                {
                    action = Some(["label0", "label1", "label2", "label3"][n]);
                } else if i.consume_key(Modifiers::NONE, Key::F) {
                    action = Some("culling");
                } else if i.consume_key(Modifiers::NONE, Key::C) {
                    action = Some("compare");
                } else if i.consume_key(Modifiers::NONE, Key::S) {
                    action = Some("stack");
                } else if i.consume_key(Modifiers::NONE, Key::L) {
                    action = Some("loupe");
                } else if i.consume_key(Modifiers::NONE, Key::A) {
                    action = Some("auto_advance");
                } else if i.consume_key(Modifiers::NONE, Key::Space) {
                    action = Some("next");
                } else if i.consume_key(Modifiers::SHIFT, Key::M) {
                    action = Some("mask_radial");
                } else if i.consume_key(Modifiers::NONE, Key::M) {
                    action = Some("mask_linear");
                } else if i.consume_key(Modifiers::NONE, Key::K) {
                    action = Some("mask_brush");
                } else if i.consume_key(Modifiers::NONE, Key::Q) {
                    action = Some("retouch");
                } else if i.consume_key(Modifiers::NONE, Key::W) {
                    action = Some("eyedropper");
                } else if i.consume_key(Modifiers::NONE, Key::T) {
                    action = Some("targeting");
                } else if i.consume_key(Modifiers::SHIFT, Key::O) {
                    action = Some("guide_turn");
                } else if i.consume_key(Modifiers::NONE, Key::O) {
                    action = Some("mask_overlay");
                } else if i.consume_key(Modifiers::NONE, Key::Delete)
                    || i.consume_key(Modifiers::NONE, Key::Backspace)
                {
                    action = Some("mask_delete");
                } else if i.consume_key(Modifiers::NONE, Key::G) {
                    action = Some("library");
                } else if i.consume_key(Modifiers::NONE, Key::D)
                    || i.consume_key(Modifiers::NONE, Key::E)
                {
                    action = Some("develop");
                } else if i.consume_key(Modifiers::NONE, Key::P) {
                    action = Some("pick");
                } else if i.consume_key(Modifiers::NONE, Key::X) {
                    action = Some("reject");
                } else if i.consume_key(Modifiers::NONE, Key::U) {
                    action = Some("unflag");
                } else if i.consume_key(Modifiers::NONE, Key::R) {
                    action = Some("crop");
                } else if i.consume_key(Modifiers::NONE, Key::Enter) {
                    action = Some("crop_done");
                } else if i.consume_key(Modifiers::NONE, Key::Escape) {
                    action = Some("crop_cancel");
                } else if i.consume_key(Modifiers::NONE, Key::Backslash) {
                    action = Some("before");
                } else if i.consume_key(Modifiers::NONE, Key::ArrowLeft) {
                    action = Some("prev");
                } else if i.consume_key(Modifiers::NONE, Key::ArrowRight) {
                    action = Some("next");
                } else if i.consume_key(Modifiers::NONE, Key::J) {
                    action = Some("clipping");
                } else if i.consume_key(Modifiers::NONE, Key::Z) {
                    action = Some("zoom");
                }
            }
        });
        if let Some(a) = action {
            self.act(a);
        }
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
            "crop_cancel" if self.culling => self.culling = false,
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
                } else if self.retouch.is_some() || self.selected_spot.is_some() || self.show_dust {
                    self.retouch = None;
                    self.selected_spot = None;
                    self.show_dust = false;
                } else {
                    self.selected_mask = None;
                }
            }
            "retouch" | "retouch_heal" | "retouch_clone" | "retouch_red_eye"
                if self.photo.is_some() =>
            {
                use luma_project::SpotKind;
                let kind = match action {
                    "retouch_clone" => SpotKind::Clone,
                    "retouch_red_eye" => SpotKind::RedEye,
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
                }
            }
            "mask_linear" | "mask_radial" | "mask_brush" | "mask_full" if self.photo.is_some() => {
                let mask = match action {
                    "mask_linear" => Mask::linear(),
                    "mask_radial" => Mask::radial(),
                    "mask_full" => Mask::full(),
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
                self.commit(label);
                self.touch();
            }
            // Pendant le recadrage, O change de guide de composition.
            "mask_overlay" if self.crop_tool.is_some() => self.crop_guide = self.crop_guide.next(),
            "guide_turn" => self.crop_turn = (self.crop_turn + 1) % 4,
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
            "open" => {
                // Sur iPhone et iPad : le sélecteur de Photos ; les photos choisies sont
                // copiées dans le dossier de LÜMA, relu dès qu'elles arrivent.
                if luma_apple::photos::AVAILABLE {
                    let opened = crate::dialogs::documents()
                        .ok_or("dossier Documents introuvable")
                        .and_then(luma_apple::photos::pick);
                    crate::journal(&format!("importer : {opened:?}"));
                    // Le choix dans Photos prend son temps, et la copie des fichiers aussi.
                    self.imports_until = f64::INFINITY;
                    if let Err(why) = opened {
                        self.status = trf("Photos ne s'ouvre pas ({why}).", &[("why", &why)]);
                    }
                } else if let Some(p) = crate::dialogs::pick_raw() {
                    self.open_path(&p);
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
        egui::Grid::new("export-presets")
            .num_columns(3)
            .spacing(vec2(10.0, 4.0))
            .show(ui, |ui| {
                for p in self.export_presets.all() {
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
        ui.horizontal(|ui| {
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
        egui::Window::new(tr("Exporter"))
            .id(egui::Id::unique("fenetre-export"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            // Plus haute qu'un écran d'iPhone : elle défile.
            .vscroll(true)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                self.presets_editor(ui);
                ui.add_space(6.0);
                let o = &mut self.export_options;
                egui::Grid::new("export")
                    .num_columns(2)
                    .spacing(vec2(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.label(tr("Photos"));
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.export_all, false, tr("Photo affichée"));
                            ui.radio_value(
                                &mut self.export_all,
                                true,
                                trf("Les {n} du filtre", &[("n", &count)]),
                            );
                        });
                        ui.end_row();
                        ui.label(tr("Format"));
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut o.format, Format::Jpeg, "JPEG");
                            ui.radio_value(&mut o.format, Format::Png, "PNG");
                            ui.radio_value(&mut o.format, Format::Tiff, "TIFF");
                        });
                        ui.end_row();
                        if o.format == Format::Jpeg {
                            ui.label(tr("Qualité"));
                            ui.add(egui::Slider::new(&mut o.quality, 1..=100));
                        } else {
                            ui.label(tr("Profondeur"));
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut o.bits, 8, "8 bits");
                                ui.radio_value(&mut o.bits, 16, "16 bits");
                            });
                        }
                        ui.end_row();
                        ui.label(tr("Espace colorimétrique"));
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
                        ui.end_row();
                        ui.label(tr("Dimensions"));
                        ui.horizontal(|ui| resize_editor(ui, &mut o.resize));
                        ui.end_row();
                        ui.label(tr("Accentuation de sortie"));
                        ui.horizontal(|ui| sharpen_editor(ui, &mut o.sharpen));
                        ui.end_row();
                        ui.label(tr("Filigrane"));
                        ui.vertical(|ui| watermark_editor(ui, &mut o.watermark));
                        ui.end_row();
                        ui.label(tr("Métadonnées"));
                        ui.vertical(|ui| metadata_editor(ui, &mut o.metadata));
                        ui.end_row();
                        if crate::dialogs::FOLDER_PICKER || o.logo.is_some() {
                            ui.label(tr("Logo"));
                            ui.vertical(|ui| logo_editor(ui, &mut o.logo));
                            ui.end_row();
                        }
                        ui.label(tr("Nom des fichiers"));
                        ui.vertical(|ui| {
                            let mut text = o.name.clone().unwrap_or_default();
                            let edit = ui
                                .add(
                                    egui::TextEdit::singleline(&mut text)
                                        .hint_text("{nom}")
                                        .desired_width(260.0),
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
                        ui.end_row();
                        ui.label(tr("Dossier"));
                        ui.horizontal(|ui| {
                            let text = match &o.folder {
                                Some(d) => d.display().to_string(),
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
                        ui.end_row();
                    });
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
        if self.indexing.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if self.licence.poll() {
            // Une clé activée ferme l'écran Premium.
            if matches!(self.licence.access(), luma_licence::Access::Premium { .. })
                && !self.licence.message.is_empty()
            {
                self.status = self.licence.message.clone();
            }
        }
        if self.licence.busy() {
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
        let compact = full.width() < 820.0;
        self.compact = compact;
        let m = if compact { 8.0 } else { 14.0 };
        let top = Rect::from_min_size(full.min + vec2(m, m), vec2(full.width() - 2.0 * m, 50.0));
        // Au tri en plein écran, la photo prend tout, la barre du haut aussi.
        let culling = self.culling && self.current.is_some();
        if !culling {
            self.island(ui, top, |s, ui| s.top_island(ui, compact));
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
        // Le voile : rien derrière ne répond.
        egui::Area::new(egui::Id::unique("premium-voile"))
            .order(egui::Order::Foreground)
            .fixed_pos(full.min)
            .show(ctx, |ui| {
                let (r, _) = ui.allocate_exact_size(full.size(), egui::Sense::click_and_drag());
                ui.painter().rect_filled(
                    r,
                    0.0,
                    Color32::from_black_alpha(if locked { 215 } else { 170 }),
                );
            });
        let w = (full.width() - 24.0).min(680.0);
        let h = (full.height() - 24.0).min(560.0);
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
                    egui::ScrollArea::vertical().show(ui, |ui| self.premium_contents(ui, locked));
                });
            });
    }

    fn premium_contents(&mut self, ui: &mut Ui, locked: bool) {
        use crate::licence::{FEATURES, PLANS, open_url};
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
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
                widgets::spectral_ring(ui.painter(), r.center(), 4.0, 2.0, 1.0);
                ui.label(RichText::new(tr(f)).size(13.0).color(theme::TEXT));
            });
        }
        ui.add_space(16.0);
        let gap = 14.0;
        let (row, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), 172.0), egui::Sense::hover());
        let card_w = ((row.width() - gap) / 2.0).floor();
        // Sur iPhone et iPad, l'abonnement s'achète dans l'App Store ; sur Mac, par le
        // lien de paiement Stripe.
        let app_store = luma_apple::store::AVAILABLE;
        let mut chosen = None;
        let mut buy = None;
        for (k, plan) in PLANS.into_iter().enumerate() {
            let best = plan == luma_licence::Plan::Annual;
            let rect = Rect::from_min_size(
                row.min + vec2(k as f32 * (card_w + gap), 0.0),
                vec2(card_w, row.height()),
            );
            ui.painter().rect(
                rect,
                18.0,
                theme::SECTION,
                egui::Stroke::new(
                    if best { 1.5 } else { 1.0 },
                    if best {
                        theme::ACCENT
                    } else {
                        theme::GLASS_EDGE
                    },
                ),
                egui::StrokeKind::Inside,
            );
            let mut card = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect.shrink(16.0))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            card.horizontal(|ui| {
                ui.label(RichText::new(tr(plan.name())).size(15.0).color(theme::TEXT));
                if best {
                    ui.label(
                        RichText::new(tr("Un seul paiement par an"))
                            .size(11.0)
                            .color(theme::ACCENT),
                    );
                }
            });
            // Le prix de l'App Store est dans la devise de l'acheteur.
            let price = match self.licence.store_price(plan) {
                Some(p) if app_store => p.to_owned(),
                _ => tr(plan.price()).to_owned(),
            };
            card.horizontal(|ui| {
                ui.label(RichText::new(price).size(28.0).strong().color(theme::TEXT));
                ui.label(
                    RichText::new(tr(plan.period()))
                        .size(13.0)
                        .color(theme::DIM),
                );
            });
            card.label(RichText::new(tr(plan.note())).size(12.0).color(theme::DIM));
            card.add_space(8.0);
            let size = vec2(card.available_width(), 30.0);
            if app_store {
                if card
                    .add_enabled(
                        !self.licence.busy(),
                        egui::Button::new(tr("S'abonner")).min_size(size),
                    )
                    .clicked()
                {
                    buy = Some(plan);
                }
                continue;
            }
            match plan.checkout_url() {
                Some(url) => {
                    if card
                        .add_sized(size, egui::Button::new(tr("S'abonner")))
                        .clicked()
                    {
                        chosen = Some(url);
                    }
                }
                None => {
                    card.add_enabled(
                        false,
                        egui::Button::new(tr("Bientôt disponible")).min_size(size),
                    )
                    .on_disabled_hover_text(tr("Le paiement en ligne ouvre bientôt."));
                }
            }
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
            if matches!(access, Access::Premium { .. }) {
                ui.label(
                    RichText::new(tr(
                        "Pour gérer ou résilier votre abonnement : Réglages › votre nom › Abonnements.",
                    ))
                    .size(12.5)
                    .color(theme::DIM),
                );
            } else {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(tr("Déjà abonné avec ce compte Apple ?"))
                            .size(12.5)
                            .color(theme::DIM),
                    );
                    if ui
                        .add_enabled(
                            !self.licence.busy(),
                            egui::Button::new(tr("Restaurer les achats")),
                        )
                        .clicked()
                    {
                        self.licence.restore();
                    }
                    if self.licence.busy() {
                        ui.spinner();
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
                        .desired_width(330.0),
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
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.set_clip_rect(rect);
            match self.module {
                Module::Develop => self.viewer(ui),
                Module::Library => self.grid(ui),
            }
        });
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
                        Icon::Open,
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
        let label = if horizontal { 14.0 } else { 0.0 };
        let mut draw = |ui: &mut Ui| {
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
                if horizontal {
                    p.text(
                        pos2(cell.center().x, cell.max.y),
                        egui::Align2::CENTER_BOTTOM,
                        tool.short_name(),
                        egui::FontId::proportional(10.0),
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
                    widgets::histogram(ui, &self.histogram, &mut self.clipping);
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
        std::fs::write(path, luma_project::xmp::write_preset(name, &group, &look))
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
        let width = ((ui.available_width() - 10.0) / 2.0).floor();
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
        if TOUCH || ui.available_width() < 420.0 {
            sections.extend(own);
        } else {
            sections.splice(0..0, own);
        }
        let mut export = None;
        for (title, items) in sections {
            sub_title(ui, tr(title));
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
                        let image = Rect::from_min_size(rect.min, vec2(width, width * 0.72));
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
                            Film::Lut(_) => tr("Pellicule mesurée ou LUT 3D importée (.cube)."),
                            Film::Mine(..) => tr("Mon préréglage."),
                        };
                        let r =
                            r.on_hover_text(format!("{hint}\n{}", tr("Clic droit : exporter.")));
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
            RichText::new(if TOUCH {
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
                0 if TOUCH => tr("Dans la bibliothèque, touchez longuement les photos à fusionner, ou prenez tout le dossier.").to_owned(),
                1 if TOUCH => tr("Une photo choisie : touchez longuement les autres dans la bibliothèque.").to_owned(),
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
            let stars = self.stack_mode == luma_merge::StackMode::Stars;
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
                MergeKind::Panorama(self.projection)
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
                    if TOUCH {
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
            if self.duplicates.is_some() {
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
        if let Some((dir, n)) = self.watching() {
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

    /// Bibliothèque, à droite : la note, le drapeau et les informations de la photo.
    fn library_right(&mut self, ui: &mut Ui) {
        section_title(ui, tr("Histogramme"));
        widgets::histogram(ui, &self.histogram, &mut self.clipping);
        ui.add_space(10.0);
        let Some(path) = self.current_path().map(Path::to_path_buf) else {
            return;
        };
        section_title(ui, tr("Métadonnées"));
        ui.label(RichText::new(file_name(&path)).strong());
        ui.horizontal(|ui| {
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
    fn keywords_editor(&mut self, ui: &mut Ui) {
        ui.label(RichText::new(tr("Mots-clés")).color(theme::DIM).size(11.5));
        let mut removed = None;
        ui.horizontal_wrapped(|ui| {
            for k in self.target_keywords() {
                if ui
                    .small_button(format!("{k}  ×"))
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
        // Deux colonnes au moins sur un écran étroit (iPhone en portrait).
        let cell = ((ui.available_width() - 14.0) / 2.0 - 10.0).clamp(120.0, 210.0);
        let mut clicked = None;
        let mut open = None;
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .id_salt("grille")
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_space(14.0);
                let cols = ((ui.available_width() - 14.0) / (cell + 10.0))
                    .floor()
                    .max(1.0) as usize;
                for row in visible.chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        for &i in row {
                            let path = &self.folder[i];
                            let (rect, r) =
                                ui.allocate_exact_size(vec2(cell, cell), egui::Sense::click());
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
                            let area = rect.shrink2(vec2(12.0, 12.0)).with_max_y(rect.max.y - 30.0);
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
                                    pos2(rect.max.x - 12.0 - galley.size().x, rect.min.y + 22.0),
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
                            painter.with_clip_rect(rect.shrink(6.0)).text(
                                pos2(rect.min.x + 12.0, rect.min.y + 6.0),
                                egui::Align2::LEFT_TOP,
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
                            if TOUCH && r.long_touched() {
                                chosen = Some(i);
                            } else if r.double_clicked() || (TOUCH && r.clicked()) {
                                open = Some(i);
                            } else if r.clicked() {
                                clicked = Some(i);
                            }
                            ui.add_space(10.0);
                        }
                    });
                    ui.add_space(10.0);
                }
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
                        if r.on_hover_text(file_name(path)).clicked() {
                            clicked = Some(i);
                        }
                        ui.add_space(8.0);
                    }
                });
            });
        if let Some(i) = clicked {
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
        if !luma_apple::photos::AVAILABLE {
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
        // Pour les essais sur simulateur : où toucher (en points), une fois.
        static SHOWN: std::sync::Once = std::sync::Once::new();
        SHOWN.call_once(|| {
            crate::journal(&format!(
                "bouton importer : centre {:.0} {:.0} (points)",
                rect.center().x,
                rect.center().y
            ))
        });
        if r.clicked() {
            crate::journal("bouton importer : touché");
            self.act("open");
        }
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
            let sense = if self.targeting || TOUCH {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::click()
            };
            let r = ui.interact(target, ui.scope_id().with("photo"), sense);
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
            } else if TOUCH && r.drag_stopped() {
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
            badges.push(tr("Écrêtage (J)"));
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
        let open = self.retouch.is_some() || self.selected_spot.is_some();
        egui::CollapsingHeader::new(RichText::new(tr("Retouche")).strong())
            .id_salt("Retouche")
            .open(open.then_some(true))
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (kind, label, hint, action) in [
                        (
                            SpotKind::Heal,
                            tr("Correcteur"),
                            tr("Q : la texture d'ailleurs, la lumière du bord de la zone"),
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
                // Les yeux rouges n'ont pas de source : ni choix du type, ni nouvelle source.
                let red_eye = s.kind == SpotKind::RedEye;
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
                let rows: [(SliderSpec, &mut f32); 13] = [
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
        if !self.compact {
            section_title(ui, tr("Histogramme"));
        }
        widgets::histogram_sized(
            ui,
            &self.histogram,
            &mut self.clipping,
            if self.compact { 52.0 } else { 96.0 },
        )
        .on_hover_text(tr("Clic ou J : afficher l'écrêtage"));
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
                    let l = self.fields(ui, &TONE);
                    keep(l, &mut commits);
                    sub_title(ui, tr("Présence"));
                    let l = self.fields(ui, &PRESENCE);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Light {
            egui::CollapsingHeader::new(RichText::new(tr("Courbe des tonalités")).strong())
                .id_salt("Courbe des tonalités")
                .default_open(true)
                .show(ui, |ui| {
                    curve_preview(ui, &self.settings);
                    let l = self.fields(ui, &CURVE);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Color {
            egui::CollapsingHeader::new(RichText::new(tr("Mélangeur de couleurs")).strong())
                .id_salt("Mélangeur de couleurs")
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, name) in [tr("Teinte|mélangeur"), tr("Saturation"), tr("Luminance")]
                            .into_iter()
                            .enumerate()
                        {
                            ui.selectable_value(&mut self.hsl_tab, i, name);
                        }
                        let on = self.targeting;
                        if ui
                            .selectable_label(on, tr("Ciblage"))
                            .on_hover_text(tr(
                                "Ciblage direct (T) : glissez sur une couleur de la photo, vers le haut ou le bas, pour régler cette couleur dans l'onglet choisi.",
                            ))
                            .clicked()
                        {
                            self.targeting = !on;
                            self.eyedropper = false;
                        }
                    });
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
                    sub_title(ui, tr("Réduction du bruit"));
                    let l = self.fields(ui, &NOISE);
                    keep(l, &mut commits);
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

/// L'aperçu de la courbe paramétrique et des courbes à points.
fn curve_preview(ui: &mut Ui, s: &Settings) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, w * 0.6), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, Color32::from_gray(0x16));
    for k in 1..4 {
        let x = rect.min.x + rect.width() * k as f32 / 4.0;
        let y = rect.min.y + rect.height() * k as f32 / 4.0;
        p.line_segment(
            [pos2(x, rect.min.y), pos2(x, rect.max.y)],
            egui::Stroke::new(1.0, Color32::from_gray(0x2a)),
        );
        p.line_segment(
            [pos2(rect.min.x, y), pos2(rect.max.x, y)],
            egui::Stroke::new(1.0, Color32::from_gray(0x2a)),
        );
    }
    let lut = luma_image::curve::parametric(&s.tone_curve)
        .then(&luma_image::curve::spline(&s.tone_curve.point));
    let points: Vec<egui::Pos2> = (0..=64)
        .map(|i| {
            let x = i as f32 / 64.0;
            pos2(
                rect.min.x + x * rect.width(),
                rect.max.y - lut.get(x) * rect.height(),
            )
        })
        .collect();
    p.add(egui::Shape::line(
        points,
        egui::Stroke::new(1.5, Color32::from_gray(0xe0)),
    ));
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
    ];
    let mode = match resize {
        None => 0,
        Some(Resize::LongEdge(_)) => 1,
        Some(Resize::ShortEdge(_)) => 2,
        Some(Resize::Fit { .. }) => 3,
        Some(Resize::Megapixels(_)) => 4,
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
            _ => Some(Resize::Megapixels(12.0)),
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
    }
}

/// Le résumé d'un préréglage d'export dans la langue de l'interface : format, espace,
/// taille.
fn export_summary(o: &ExportOptions) -> String {
    use luma_render::export::Resize;
    let format = match o.format {
        Format::Jpeg => format!("JPEG {}", o.quality),
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
    };
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
