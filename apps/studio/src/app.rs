//! LÜMA Studio : l'état de l'application et son interface.
//!
//! L'interface ne ressemble à aucune autre : la photo occupe tout l'écran, et les
//! outils flottent dessus dans des îlots de verre fumé. À gauche, le rail des outils
//! (Bibliothèque, Lumière, Couleur, Détail, Masques, Films, Géométrie, Fusion) ; à
//! droite, l'inspecteur de l'outil choisi, et lui seul ; en bas, la table lumineuse du
//! dossier. L'anneau spectral de LÜMA marque ce qui est actif.
//!
//! Les gestes et les réglages restent ceux que connaissent les photographes : mêmes
//! curseurs que Lightroom, historique, XMP enregistré à côté du RAW, raccourcis (`\`
//! avant/après, ⌘Z, ⇧⌘C et ⇧⌘V, ⇧⌘E exporter, ← →, J écrêtage, Z zoom 1:1, G, D, M,
//! Maj+M, K, R, W, O).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{
    Color32, ColorImage, Key, Modifiers, Rect, RichText, TextureHandle, TextureOptions, Ui, pos2,
    vec2,
};
use luma_color::OutputSpace;
use luma_project::presets::{self, PRESETS};
use luma_project::xmp::{Flag, Meta};
use luma_project::{Crop, Mask, Profile, Settings, Shape, WhiteBalance};
use luma_render::{Photo, Size};

use crate::crop;
use crate::icons::{self, Icon};
use crate::masking;
use crate::widgets::{self, Change, SliderSpec, Track, format_value, theme};
use crate::worker::{
    Event, ExportJob, ExportOptions, Format, MergeJob, MergeKind, Worker, read_meta, read_sidecar,
    write_sidecar,
};

/// Les extensions RAW reconnues dans un dossier.
pub const RAW_EXTENSIONS: [&str; 20] = [
    "arw", "cr2", "cr3", "crw", "nef", "nrw", "raf", "dng", "orf", "rw2", "pef", "srw", "3fr",
    "iiq", "erf", "kdc", "dcr", "mos", "mef", "rwl",
];

pub fn is_raw(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| RAW_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Les RAW d'un dossier, triés par nom.
pub fn scan(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_raw(p))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
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

/// Les préréglages de balance des blancs de Lightroom (température, teinte).
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

/// Les modules de la barre du haut (G et D dans Lightroom).
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

    pub fn name(self) -> &'static str {
        match self {
            Tool::Library => "Bibliothèque",
            Tool::Light => "Lumière",
            Tool::Color => "Couleur",
            Tool::Detail => "Détail",
            Tool::Masks => "Masques",
            Tool::Films => "Films",
            Tool::Geometry => "Géométrie",
            Tool::Merge => "Fusion",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Tool::Library => "Trier, noter, choisir",
            Tool::Light => "Profil, balance, tonalité, courbe",
            Tool::Color => "Mélangeur, étalonnage, calibration",
            Tool::Detail => "Netteté, bruit, vignetage, grain",
            Tool::Masks => "Retouches locales",
            Tool::Films => "Pellicules et looks",
            Tool::Geometry => "Recadrage, objectif, perspective",
            Tool::Merge => "Empilements et panoramas",
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
            Tool::Masks => "M, Maj+M, K",
            Tool::Geometry => "R",
            _ => "",
        }
    }
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
    texture: Option<TextureHandle>,
    image_size: [usize; 2],
    histogram: [[u32; 256]; 3],
    generation: u64,
    shown: u64,
    last_request: Option<Request>,
    render_ms: f32,
    render_gpu: bool,
    before: bool,
    clipping: bool,
    zoom: bool,
    thumbs: HashMap<PathBuf, TextureHandle>,
    status: String,
    xmp_due: Option<f64>,
    grading_tab: usize,
    hsl_tab: usize,
    pub module: Module,
    meta: HashMap<PathBuf, Meta>,
    min_rating: u8,
    flag_filter: FlagFilter,
    export_options: ExportOptions,
    export_open: bool,
    export_all: bool,
    crop_tool: Option<CropTool>,
    /// Le masque sélectionné (ses poignées sont à l'écran).
    pub selected_mask: Option<usize>,
    brush: masking::Brush,
    /// Le masque en rouge (touche O) : le masque dessiné et sa texture.
    show_mask: bool,
    mask_overlay: Option<(Mask, TextureHandle)>,
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
    /// Films : le préréglage choisi, son intensité, et les réglages d'avant.
    preset: Option<(usize, f32, Settings)>,
    previews: Vec<Option<TextureHandle>>,
    previews_tag: u64,
    previews_for: Option<PathBuf>,
    /// La pipette de balance des blancs (W) attend un clic sur la photo.
    eyedropper: bool,
    /// Le nom de la dernière balance personnalisée (« Auto » ou « Personnalisée »).
    wb_label: &'static str,
}

impl Studio {
    pub fn new(ctx: &egui::Context) -> Self {
        crate::style::apply(ctx);
        let repaint_ctx = ctx.clone();
        Self {
            worker: Worker::new(move || repaint_ctx.request_repaint()),
            folder: Vec::new(),
            current: None,
            photo: None,
            loading: None,
            settings: Settings::default(),
            history: Vec::new(),
            history_pos: 0,
            clipboard: None,
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
            zoom: false,
            thumbs: HashMap::new(),
            status: "Ouvrez un RAW ou un dossier (Cmd+O), ou déposez-le dans la fenêtre.".into(),
            xmp_due: None,
            grading_tab: 0,
            hsl_tab: 0,
            module: Module::Develop,
            meta: HashMap::new(),
            min_rating: 0,
            flag_filter: FlagFilter::All,
            export_options: ExportOptions::default(),
            export_open: false,
            export_all: false,
            crop_tool: None,
            selected_mask: None,
            brush: masking::Brush::default(),
            show_mask: false,
            mask_overlay: None,
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
            previews: Vec::new(),
            previews_tag: 0,
            previews_for: None,
            eyedropper: false,
            wb_label: "Personnalisée",
        }
    }

    /// Ouvre un fichier (et son dossier dans le film) ou un dossier.
    pub fn open_path(&mut self, path: &Path) {
        let (dir, file) = if path.is_dir() {
            (path.to_path_buf(), None)
        } else {
            (
                path.parent().unwrap_or(Path::new(".")).to_path_buf(),
                Some(path.to_path_buf()),
            )
        };
        self.folder = scan(&dir);
        if let Some(f) = &file
            && !self.folder.contains(f)
        {
            self.folder.push(f.clone());
        }
        for p in &self.folder {
            if !self.thumbs.contains_key(p) {
                self.worker.thumbnail(p.clone());
            }
            self.meta.insert(p.clone(), read_meta(p));
        }
        let index = file
            .and_then(|f| self.folder.iter().position(|p| *p == f))
            .or(if self.folder.is_empty() {
                None
            } else {
                Some(0)
            });
        match index {
            Some(i) => self.select(i),
            None => self.status = format!("Aucun RAW dans {}", dir.display()),
        }
    }

    /// La photo actuellement développée.
    pub fn current_path(&self) -> Option<&Path> {
        self.current
            .and_then(|i| self.folder.get(i))
            .map(PathBuf::as_path)
    }

    /// Les photos qui passent le filtre, dans l'ordre du dossier.
    pub fn visible(&self) -> Vec<usize> {
        (0..self.folder.len())
            .filter(|i| {
                let m = self.meta.get(&self.folder[*i]).cloned().unwrap_or_default();
                m.rating >= self.min_rating
                    && match self.flag_filter {
                        FlagFilter::All => true,
                        FlagFilter::Picks => m.flag == Flag::Pick,
                        FlagFilter::NotRejected => m.flag != Flag::Reject,
                    }
            })
            .collect()
    }

    /// La note et le drapeau de la photo affichée.
    pub fn current_meta(&self) -> Meta {
        self.current_path()
            .and_then(|p| self.meta.get(p))
            .cloned()
            .unwrap_or_default()
    }

    fn set_meta(&mut self, f: impl FnOnce(&mut Meta)) {
        let Some(path) = self.current_path().map(Path::to_path_buf) else {
            return;
        };
        let meta = self.meta.entry(path).or_default();
        f(meta);
        self.touch();
    }

    /// Prête : photo chargée et dernier rendu affiché.
    pub fn is_idle(&self) -> bool {
        self.loading.is_none()
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
        self.status = format!("Ouverture de {}…", file_name(&path));
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
                            self.settings = sidecar.map(|s| *s).unwrap_or_default();
                            self.history = vec![("Importation".into(), Settings::default())];
                            if from_xmp {
                                self.history
                                    .push(("Réglages XMP".into(), self.settings.clone()));
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
                            self.zoom = false;
                        }
                        Err(e) => {
                            self.status = format!("Impossible d'ouvrir {} : {e}", file_name(&path))
                        }
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
                    let image = ColorImage::from_rgba_unmultiplied(frame.size, &frame.rgba);
                    match &mut self.texture {
                        Some(t) => t.set(image, TextureOptions::LINEAR),
                        None => {
                            self.texture =
                                Some(ctx.load_texture("photo", image, TextureOptions::LINEAR))
                        }
                    }
                }
                Event::Thumbnail { path, size, rgba } => {
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
                            self.status = format!("Fusion enregistrée : {}", file_name(&path));
                            self.selected.clear();
                            self.tool = Tool::Light;
                            self.module = Module::Develop;
                            self.open_path(&path);
                        }
                        Err(e) => self.status = format!("Fusion impossible : {e}"),
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
                        Ok(p) => format!("Exporté{count} : {}", p.display()),
                        Err(e) => format!("Échec de l'export{count} : {e}"),
                    };
                }
            }
        }
    }

    /// Une étape d'historique (curseur relâché, choix fait).
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
    fn touch(&mut self) {
        self.xmp_due = Some(0.0);
    }

    /// Écrit le XMP à côté du RAW (format Lightroom).
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
                self.status = format!("XMP non enregistré : {e}");
            }
        }
    }

    fn request_render(&mut self, viewport_px: f32) {
        let (Some(photo), Some(path)) = (&self.photo, self.current_path()) else {
            return;
        };
        // Tailles par paliers de 512 px : le cache de sources sert d'un rendu à l'autre.
        let size = if self.zoom {
            Size::Full
        } else {
            Size::Fit(((viewport_px / 512.0).ceil().max(1.0) * 512.0) as usize)
        };
        let mut settings = if self.before {
            Settings::default()
        } else {
            self.settings.clone()
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
            self.clipping,
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
            } else if i.consume_key(cmd_shift, Key::V) {
                action = Some("paste");
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
                if let Some(n) = digits
                    .iter()
                    .position(|k| i.consume_key(Modifiers::NONE, *k))
                {
                    action = Some(["rate0", "rate1", "rate2", "rate3", "rate4", "rate5"][n]);
                } else if i.consume_key(Modifiers::SHIFT, Key::M) {
                    action = Some("mask_radial");
                } else if i.consume_key(Modifiers::NONE, Key::M) {
                    action = Some("mask_linear");
                } else if i.consume_key(Modifiers::NONE, Key::K) {
                    action = Some("mask_brush");
                } else if i.consume_key(Modifiers::NONE, Key::W) {
                    action = Some("eyedropper");
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
        match action {
            "undo" if self.history_pos > 0 => self.restore(self.history_pos - 1),
            "redo" if self.history_pos + 1 < self.history.len() => {
                self.restore(self.history_pos + 1)
            }
            "copy" => {
                self.clipboard = Some(self.settings.clone());
                self.status = "Réglages copiés.".into();
            }
            "paste" => {
                if let Some(s) = self.clipboard.clone() {
                    self.settings = s;
                    self.commit("Coller les réglages".into());
                    self.touch();
                }
            }
            "reset" => {
                self.settings = Settings::default();
                self.commit("Réinitialiser".into());
                self.touch();
            }
            "export" => {
                if self.current.is_some() {
                    self.export_all = self.module == Module::Library;
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
                    "Note retirée".into()
                } else {
                    format!("Note : {n} sur 5")
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
                    self.commit("Recadrage".into());
                    self.touch();
                }
            }
            "crop_cancel" => {
                if let Some(t) = self.crop_tool.take() {
                    self.settings.crop = t.saved;
                } else if self.export_open {
                    self.export_open = false;
                } else {
                    self.selected_mask = None;
                }
            }
            "mask_linear" | "mask_radial" | "mask_brush" if self.photo.is_some() => {
                let mask = match action {
                    "mask_linear" => Mask::linear(),
                    "mask_radial" => Mask::radial(),
                    _ => Mask::brush(),
                };
                let label = format!("Nouveau masque : {}", mask.name.to_lowercase());
                self.set_tool(Tool::Masks);
                self.zoom = false;
                self.settings.masks.push(mask);
                self.selected_mask = Some(self.settings.masks.len() - 1);
                self.commit(label);
                self.touch();
            }
            "mask_overlay" => self.show_mask = !self.show_mask,
            "eyedropper" => self.eyedropper = !self.eyedropper && self.photo.is_some(),
            "mask_delete" => {
                if let Some(i) = self.selected_mask.take()
                    && i < self.settings.masks.len()
                {
                    self.settings.masks.remove(i);
                    self.commit("Masque supprimé".into());
                    self.touch();
                }
            }
            "open" => {
                if let Some(p) = crate::dialogs::pick_raw() {
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
        self.status = format!(
            "Export de {} photo{}…",
            jobs.len(),
            if jobs.len() > 1 { "s" } else { "" }
        );
        self.worker.export(jobs, self.export_options.clone());
    }

    /// La fenêtre Exporter.
    fn export_window(&mut self, ctx: &egui::Context) {
        let mut open = self.export_open;
        let mut go = false;
        let count = self.visible().len();
        egui::Window::new("Exporter")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                let o = &mut self.export_options;
                egui::Grid::new("export")
                    .num_columns(2)
                    .spacing(vec2(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.label("Photos");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.export_all, false, "Photo affichée");
                            ui.radio_value(
                                &mut self.export_all,
                                true,
                                format!("Les {count} du filtre"),
                            );
                        });
                        ui.end_row();
                        ui.label("Format");
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut o.format, Format::Jpeg, "JPEG");
                            ui.radio_value(&mut o.format, Format::Png, "PNG");
                            ui.radio_value(&mut o.format, Format::Tiff, "TIFF");
                        });
                        ui.end_row();
                        if o.format == Format::Jpeg {
                            ui.label("Qualité");
                            ui.add(egui::Slider::new(&mut o.quality, 1..=100));
                        } else {
                            ui.label("Profondeur");
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut o.bits, 8, "8 bits");
                                ui.radio_value(&mut o.bits, 16, "16 bits");
                            });
                        }
                        ui.end_row();
                        ui.label("Espace colorimétrique");
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
                            .accessible_name("Espace colorimétrique");
                        ui.end_row();
                        ui.label("Dimensions");
                        ui.horizontal(|ui| {
                            let mut resize = o.long_edge.is_some();
                            ui.checkbox(&mut resize, "Grand côté");
                            let mut n = o.long_edge.unwrap_or(2048);
                            ui.add_enabled(
                                resize,
                                egui::DragValue::new(&mut n).range(64..=20000).suffix(" px"),
                            )
                            .accessible_name("Grand côté en pixels");
                            o.long_edge = resize.then_some(n);
                        });
                        ui.end_row();
                        ui.label("Dossier");
                        ui.horizontal(|ui| {
                            let text = match &o.folder {
                                Some(d) => d.display().to_string(),
                                None => "LÜMA/, à côté du RAW".into(),
                            };
                            ui.label(RichText::new(text).color(theme::DIM));
                            if crate::dialogs::FOLDER_PICKER
                                && ui.button("Choisir…").clicked()
                                && let Some(d) = crate::dialogs::pick_folder("Dossier d'export")
                            {
                                o.folder = Some(d);
                            }
                            if o.folder.is_some() && ui.button("À côté du RAW").clicked() {
                                o.folder = None;
                            }
                        });
                        ui.end_row();
                    });
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                    if ui.button(RichText::new("Exporter").strong()).clicked() {
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
        self.shortcuts(&ctx);
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

        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, theme::BACKGROUND);
        // iPhone (ou fenêtre étroite) : la photo en haut, la barre d'outils, puis
        // l'inspecteur en feuille.
        let compact = full.width() < 820.0;
        let m = if compact { 8.0 } else { 14.0 };
        let top = Rect::from_min_size(full.min + vec2(m, m), vec2(full.width() - 2.0 * m, 50.0));
        self.island(ui, top, |s, ui| s.top_island(ui, compact));
        if compact {
            let stage = Rect::from_min_max(
                pos2(full.min.x, top.max.y + m),
                pos2(full.max.x, top.max.y + m + full.height() * 0.38),
            );
            let bar = Rect::from_min_size(
                pos2(full.min.x + m, stage.max.y + m),
                vec2(full.width() - 2.0 * m, 60.0),
            );
            let sheet = Rect::from_min_max(
                pos2(full.min.x + m, bar.max.y + m),
                pos2(full.max.x - m, full.max.y - m),
            );
            self.stage(ui, stage);
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
        if self.export_open {
            self.export_window(&ctx);
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
            let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), egui::Sense::hover());
            widgets::spectral_ring(ui.painter(), r.center(), 10.0, 3.5, 1.0);
            ui.painter()
                .circle_filled(r.center(), 4.0, Color32::from_rgb(0xff, 0xf3, 0xd6));
            ui.label(RichText::new("LÜMA").strong().size(17.0).color(theme::TEXT));
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(
                    ui,
                    Icon::Export,
                    if compact { "" } else { "Exporter" },
                    false,
                )
                .on_hover_text("Maj+Cmd+E")
                .clicked()
                {
                    self.act("export");
                }
                if icon_button(ui, Icon::Open, if compact { "" } else { "Ouvrir" }, false)
                    .on_hover_text("Cmd+O")
                    .clicked()
                {
                    self.act("open");
                }
                ui.add_space(6.0);
                if !compact {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    for (label, action, tip) in [
                        ("Réinitialiser", "reset", "Maj+Cmd+R"),
                        ("Coller", "paste", "Maj+Cmd+V"),
                        ("Copier", "copy", "Maj+Cmd+C"),
                        ("Rétablir", "redo", "Maj+Cmd+Z"),
                        ("Annuler", "undo", "Cmd+Z"),
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
                let label = if self.before { "Avant" } else { "Après" };
                if ui
                    .selectable_label(self.before, label)
                    .on_hover_text("\\ : avant / après")
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
        let size = if horizontal {
            (ui.available_width() / Tool::ALL.len() as f32).min(52.0)
        } else {
            44.0
        };
        let mut draw = |ui: &mut Ui| {
            for tool in Tool::ALL {
                let (rect, r) = ui.allocate_exact_size(vec2(size, size), egui::Sense::click());
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
                let tip = if tool.key().is_empty() {
                    tool.name().to_owned()
                } else {
                    format!("{} ({})", tool.name(), tool.key())
                };
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
        ui.add_space(8.0);
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

    /// L'historique (outil Lumière).
    fn history_panel(&mut self, ui: &mut Ui) {
        let mut restore = None;
        egui::CollapsingHeader::new(RichText::new("Historique").strong())
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

    /// Films : les pellicules et les looks, avec leur aperçu sur la photo.
    fn films_panel(&mut self, ui: &mut Ui) {
        let Some(photo) = self.photo.clone() else {
            ui.label(RichText::new("Ouvrez une photo pour essayer les films.").color(theme::DIM));
            return;
        };
        // Les réglages sans film : ceux d'avant le préréglage choisi.
        let base = self
            .preset
            .as_ref()
            .map_or_else(|| self.settings.clone(), |p| p.2.clone());
        if self.previews_for.as_deref() != self.current_path()
            || self.previews.len() != PRESETS.len()
        {
            self.previews_tag += 1;
            self.previews = vec![None; PRESETS.len()];
            self.previews_for = self.current_path().map(Path::to_path_buf);
            let all = PRESETS.iter().map(|p| p.applied(&base, 1.0)).collect();
            self.worker.previews(photo, all, self.previews_tag);
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
                self.settings = PRESETS[i].applied(&base, amount);
                self.preset = Some((i, amount, base.clone()));
                self.touch();
            }
            if c.committed {
                self.commit(format!(
                    "{} à {} %",
                    PRESETS[i].name,
                    format_value(percent, 0, false)
                ));
            }
            if ui.button("Retirer le film").clicked() {
                self.settings = base;
                self.preset = None;
                self.commit("Sans film".into());
                self.touch();
            }
            ui.add_space(6.0);
        }
        let mut choose = None;
        let width = ((ui.available_width() - 10.0) / 2.0).floor();
        for group in [
            presets::Group::ColorFilm,
            presets::Group::BlackAndWhiteFilm,
            presets::Group::Creative,
        ] {
            sub_title(ui, group.name());
            let items: Vec<usize> = (0..PRESETS.len())
                .filter(|&k| PRESETS[k].group == group)
                .collect();
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
                            PRESETS[k].name,
                            egui::FontId::proportional(12.0),
                            if active { theme::ACCENT } else { theme::TEXT },
                        );
                        if r.on_hover_text(PRESETS[k].description).clicked() {
                            choose = Some(k);
                        }
                    }
                });
                ui.add_space(4.0);
            }
        }
        if let Some(k) = choose {
            let amount = self.preset.as_ref().map_or(1.0, |p| p.1);
            self.settings = PRESETS[k].applied(&base, amount);
            self.preset = Some((k, amount, base));
            self.commit(format!("Film : {}", PRESETS[k].name));
            self.touch();
        }
    }

    /// Fusion : empilements et panoramas des photos choisies.
    fn merge_panel(&mut self, ui: &mut Ui) {
        let n = self.selected.len();
        ui.label(
            RichText::new(match n {
                0 => "Choisissez les photos dans le film : Cmd‑clic pour en ajouter une, Maj‑clic pour une suite.".to_owned(),
                1 => "Une photo choisie : ajoutez‑en d'autres (Cmd‑clic, Maj‑clic).".to_owned(),
                n => format!("{n} photos choisies."),
            })
            .color(theme::DIM),
        );
        ui.horizontal(|ui| {
            if ui.button("Tout le dossier").clicked() {
                self.selected = self.visible().into_iter().collect();
            }
            if n > 0 && ui.button("Vider").clicked() {
                self.selected.clear();
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            for (stack, icon, label) in [
                (true, Icon::Stack, "Empilement"),
                (false, Icon::Panorama, "Panorama"),
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
                (S::Mean, "Moins de bruit, poses longues simulées"),
                (S::Median, "Ce qui bouge disparaît (passants, voitures)"),
                (S::Maximum, "Filé d'étoiles, traînées de phares, éclairs"),
                (
                    S::Stars,
                    "Suit le ciel qui tourne, rejette satellites et avions",
                ),
            ] {
                ui.horizontal(|ui| {
                    ui.radio_value(
                        &mut self.stack_mode,
                        mode,
                        RichText::new(mode.name()).strong(),
                    );
                    ui.label(RichText::new(text).size(11.5).color(theme::DIM));
                });
            }
            ui.add_space(4.0);
            let stars = self.stack_mode == luma_merge::StackMode::Stars;
            ui.add_enabled_ui(!stars, |ui| {
                let mut align = self.stack_align || stars;
                ui.checkbox(&mut align, "Aligner les poses (à main levée)");
                if !stars {
                    self.stack_align = align;
                }
            });
        } else {
            use luma_merge::Projection as P;
            ui.horizontal(|ui| {
                for p in [P::Perspective, P::Cylindrical, P::Spherical] {
                    ui.selectable_value(&mut self.projection, p, p.name());
                }
            });
            ui.label(
                RichText::new(match self.projection {
                    P::Perspective => {
                        "Les droites restent droites (architecture) ; jusqu'à 120° environ."
                    }
                    P::Cylindrical => {
                        "Les verticales restent droites : le choix des panoramas horizontaux."
                    }
                    P::Spherical => {
                        "Pour les très grands angles et les panoramas sur plusieurs rangées."
                    }
                })
                .size(11.5)
                .color(theme::DIM),
            );
        }
        ui.add_space(8.0);
        ui.checkbox(&mut self.auto_crop, "Rogner automatiquement");
        ui.label(
            RichText::new("Le rognage est un recadrage ordinaire : on peut le modifier ou le retirer ensuite (Géométrie).")
                .size(11.0)
                .color(theme::DIM),
        );
        ui.checkbox(
            &mut self.merge_full,
            "Pleine définition (sinon 2048 px, plus rapide)",
        );
        ui.add_space(10.0);
        if let Some((fraction, label, cancel)) = &self.merging {
            ui.add(
                egui::ProgressBar::new(*fraction)
                    .text(label.clone())
                    .fill(theme::ACCENT),
            );
            if ui.button("Annuler la fusion").clicked() {
                cancel.cancel();
            }
            return;
        }
        let go = ui.add_enabled(
            n >= 2,
            egui::Button::new(
                RichText::new(format!("Fusionner {} photos", n.max(2)))
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
            self.merging = Some((0.0, "Préparation".into(), cancel.clone()));
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
        section_title(ui, "Dossier");
        let dir = self
            .folder
            .first()
            .and_then(|p| p.parent())
            .map(|d| d.display().to_string())
            .unwrap_or_else(|| "Aucun (Cmd+O)".into());
        ui.label(RichText::new(dir).size(12.0));
        ui.label(
            RichText::new(format!(
                "{} photos, {} affichées",
                self.folder.len(),
                self.visible().len()
            ))
            .size(11.5)
            .color(theme::DIM),
        );
        ui.add_space(14.0);
        section_title(ui, "Filtre");
        ui.label(RichText::new("Note minimale").color(theme::DIM).size(11.5));
        if let Some(n) = widgets::rating(ui, self.min_rating, 18.0) {
            self.min_rating = n;
        }
        ui.add_space(8.0);
        ui.label(RichText::new("Drapeau").color(theme::DIM).size(11.5));
        for (f, name) in [
            (FlagFilter::All, "Toutes"),
            (FlagFilter::Picks, "Retenues (P)"),
            (FlagFilter::NotRejected, "Sans les rejetées (X)"),
        ] {
            ui.radio_value(&mut self.flag_filter, f, name);
        }
        ui.add_space(14.0);
        section_title(ui, "Raccourcis");
        for line in [
            "0 à 5 : note",
            "P, X, U : retenir, rejeter, sans drapeau",
            "Flèches gauche et droite : photo voisine",
            "D ou double-clic : Développement",
        ] {
            ui.label(RichText::new(line).size(11.5).color(theme::DIM));
        }
    }

    /// Bibliothèque, à droite : la note, le drapeau et les informations de la photo.
    fn library_right(&mut self, ui: &mut Ui) {
        section_title(ui, "Histogramme");
        widgets::histogram(ui, &self.histogram, &mut self.clipping);
        ui.add_space(10.0);
        let Some(path) = self.current_path().map(Path::to_path_buf) else {
            return;
        };
        section_title(ui, "Métadonnées");
        ui.label(RichText::new(file_name(&path)).strong());
        let meta = self.current_meta();
        if let Some(n) = widgets::rating(ui, meta.rating, 20.0) {
            self.act(["rate0", "rate1", "rate2", "rate3", "rate4", "rate5"][n as usize]);
        }
        ui.horizontal(|ui| {
            for (f, name, action) in [
                (Flag::Pick, "Retenue", "pick"),
                (Flag::None, "Sans drapeau", "unflag"),
                (Flag::Reject, "Rejetée", "reject"),
            ] {
                if ui.selectable_label(meta.flag == f, name).clicked() {
                    self.act(action);
                }
            }
        });
        ui.add_space(8.0);
        if let Some(photo) = &self.photo {
            let raw = photo.raw();
            let e = &raw.exif;
            let (w, h) = photo.full_size();
            let mut rows = vec![
                ("Appareil", format!("{} {}", raw.make, raw.model)),
                ("Dimensions", format!("{w} × {h}")),
            ];
            if let Some(iso) = e.iso {
                rows.push(("ISO", iso.to_string()));
            }
            if let Some(f) = e.focal_length {
                rows.push(("Focale", format!("{f:.0} mm")));
            }
            if let Some(a) = e.aperture {
                rows.push(("Ouverture", format!("f/{}", format_value(a, 1, false))));
            }
            if let Some(t) = e.exposure_time {
                rows.push((
                    "Vitesse",
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

    /// Bibliothèque, au centre : la grille des vignettes.
    fn grid(&mut self, ui: &mut Ui) {
        let visible = self.visible();
        if visible.is_empty() {
            let rect = ui.max_rect();
            let text = if self.folder.is_empty() {
                self.status.clone()
            } else {
                "Aucune photo ne passe le filtre.".into()
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(15.0),
                theme::DIM,
            );
            return;
        }
        let cell = 210.0;
        let mut clicked = None;
        let mut open = None;
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
                                }
                            }
                            let meta = self.meta.get(path).cloned().unwrap_or_default();
                            paint_meta(painter, rect, &meta);
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
                            if r.double_clicked() {
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
        if let Some(i) = open {
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
                            None => (h * 1.5, None),
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
        self.request_render(rect.width().max(rect.height()) * ppp);
        let painter = ui.painter_at(rect);
        let Some(t) = &self.texture else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &self.status,
                egui::FontId::proportional(15.0),
                theme::DIM,
            );
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
            let c = crop::edit(ui, target, &mut self.settings.crop, aspect);
            if c.changed {
                self.touch();
            }
        } else if let Some(i) = self
            .selected_mask
            .filter(|i| *i < self.settings.masks.len() && self.tool == Tool::Masks)
        {
            let target = fit(rect.shrink(18.0), iw, ih);
            painter.image(t.id(), target, uv, Color32::WHITE);
            if self.show_mask {
                let mask = &self.settings.masks[i];
                let stale = self.mask_overlay.as_ref().is_none_or(|(m, _)| m != mask);
                if stale {
                    let (w, h) = luma_core::RgbImage::fit(iw, ih, 400);
                    let image = masking::overlay(mask, w, h);
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
                    painter.image(tex.id(), target, uv, Color32::WHITE);
                }
            }
            let brush = self.brush;
            let c = masking::edit(ui, target, &mut self.settings.masks[i], &brush);
            if c.changed {
                self.touch();
            }
            if c.committed {
                let name = self.settings.masks[i].name.clone();
                self.commit(format!("Masque : {}", name.to_lowercase()));
            }
        } else {
            let target = fit(rect.shrink(18.0), iw, ih);
            painter.image(t.id(), target, uv, Color32::WHITE);
            let r = ui.interact(target, ui.scope_id().with("photo"), egui::Sense::click());
            if self.eyedropper {
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
                    self.commit(format!(
                        "Pipette : {} K, {}",
                        format_value(t.temperature, 0, false),
                        format_value(t.tint, 0, true)
                    ));
                    self.touch();
                }
            } else if r.on_hover_text("Clic ou Z : zoom 1:1").clicked() {
                self.zoom = true;
            }
        }
        let mut badges = Vec::new();
        if self.before {
            badges.push("Avant");
        }
        if self.loading.is_some() || self.shown != self.generation {
            badges.push("Rendu…");
        }
        if self.clipping {
            badges.push("Écrêtage (J)");
        }
        if self.eyedropper {
            badges.push("Pipette : cliquez un gris ou un blanc de la photo");
        }
        if self.crop_tool.is_some() {
            badges.push("Recadrage : Entrée pour valider, Échap pour annuler");
        } else if let Some(m) = self.selected_mask.and_then(|i| self.settings.masks.get(i)) {
            badges.push(match m.shape {
                Shape::Brush { .. } => {
                    "Pinceau : peindre en glissant, Option pour gommer, O pour voir le masque"
                }
                _ => "Masque : tirer les poignées, O pour voir le masque, Échap pour quitter",
            });
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
            self.status.clone(),
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
                    f.spec.label,
                    format_value(v, f.spec.decimals, f.spec.min < 0.0)
                ));
            }
        }
        label
    }

    /// Panneau Masquage : la liste des masques, et les curseurs du masque choisi.
    fn masks_panel(&mut self, ui: &mut Ui, commits: &mut Vec<String>) {
        let editing = self.selected_mask.is_some();
        egui::CollapsingHeader::new(RichText::new("Masquage").strong())
            .open(editing.then_some(true))
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (icon, label, key, action) in [
                        (
                            Icon::Radial,
                            "Radial",
                            "Maj+M : un cercle, plein effet au centre",
                            "mask_radial",
                        ),
                        (
                            Icon::Linear,
                            "Linéaire",
                            "M : trois traits, plein effet, milieu, plus d'effet",
                            "mask_linear",
                        ),
                        (
                            Icon::Brush,
                            "Pinceau",
                            "K : peindre, Option pour gommer",
                            "mask_brush",
                        ),
                    ] {
                        if big_choice(ui, icon, label, false)
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
                    let mut text = RichText::new(format!("{}. {}", i + 1, m.name)).size(12.0);
                    if !m.is_active() {
                        text = text.color(theme::DIM);
                    }
                    if ui.selectable_label(current, text).clicked() {
                        select = Some(if current { None } else { Some(i) });
                    }
                }
                if let Some(s) = select {
                    self.selected_mask = s;
                }
                let Some(i) = self
                    .selected_mask
                    .filter(|i| *i < self.settings.masks.len())
                else {
                    if !self.settings.masks.is_empty() {
                        ui.label(
                            RichText::new("Choisissez un masque pour le régler.")
                                .size(11.5)
                                .color(theme::DIM),
                        );
                    }
                    return;
                };
                ui.horizontal(|ui| {
                    let m = &mut self.settings.masks[i];
                    if ui.checkbox(&mut m.invert, "Inverser").changed() {
                        commits.push("Masque inversé".into());
                    }
                    if ui.selectable_label(self.show_mask, "Voir (O)").clicked() {
                        self.show_mask = !self.show_mask;
                    }
                    if ui.button("Supprimer").clicked() {
                        self.act("mask_delete");
                    }
                });
                let Some(m) = self.settings.masks.get_mut(i) else {
                    return;
                };
                let name = m.name.to_lowercase();
                let spec = |label, min, max, decimals| SliderSpec {
                    label,
                    min,
                    max,
                    default: if label == "Intensité" { 100.0 } else { 0.0 },
                    decimals,
                    track: Track::Plain,
                    log: false,
                };
                let l = &mut m.local;
                let rows: [(SliderSpec, &mut f32); 11] = [
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
                ];
                let mut changed = false;
                for (spec, value) in rows {
                    let c = widgets::slider(ui, &spec, value);
                    changed |= c.changed;
                    if c.committed {
                        commits.push(format!(
                            "Masque {name} : {} {}",
                            spec.label.to_lowercase(),
                            format_value(*value, spec.decimals, spec.min < 0.0)
                        ));
                    }
                }
                if matches!(m.shape, Shape::Brush { .. }) {
                    sub_title(ui, "Pinceau");
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
                        commits.push(format!("Masque {name} : contour progressif"));
                    }
                }
                if changed {
                    self.xmp_due = Some(0.0);
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

        section_title(ui, "Histogramme");
        widgets::histogram(ui, &self.histogram, &mut self.clipping)
            .on_hover_text("Clic ou J : afficher l'écrêtage");
        ui.add_space(6.0);

        if self.tool == Tool::Masks {
            self.masks_panel(ui, &mut commits);
        }

        if self.tool == Tool::Light {
            egui::CollapsingHeader::new(RichText::new("Base").strong())
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Profil").color(theme::DIM));
                        let names = [
                            (Profile::Color, "Couleur"),
                            (Profile::Neutral, "Neutre"),
                            (Profile::Vivid, "Vif"),
                            (Profile::Monochrome, "Monochrome"),
                        ];
                        let current = names
                            .iter()
                            .find(|n| n.0 == self.settings.profile)
                            .map_or("Couleur", |n| n.1);
                        egui::ComboBox::from_id_salt("profil")
                            .selected_text(current)
                            .show_ui(ui, |ui| {
                                for (p, name) in names {
                                    if ui
                                        .selectable_value(&mut self.settings.profile, p, name)
                                        .changed()
                                    {
                                        commits.push(format!("Profil : {name}"));
                                    }
                                }
                            })
                            .response
                            .accessible_name("Profil");
                    });
                    ui.add_space(4.0);
                    let as_shot = self.photo.as_ref().map(|p| p.as_shot());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Balance des blancs").color(theme::DIM));
                        let current = match self.settings.white_balance {
                            WhiteBalance::AsShot => "Telle quelle",
                            WhiteBalance::Custom { temperature, tint } => WB_PRESETS
                                .iter()
                                .find(|p| p.1 == temperature && p.2 == tint)
                                .map_or(self.wb_label, |p| p.0),
                        };
                        let mut choice: Option<&'static str> = None;
                        egui::ComboBox::from_id_salt("balance")
                            .selected_text(current)
                            .show_ui(ui, |ui| {
                                for name in ["Telle quelle", "Auto"]
                                    .into_iter()
                                    .chain(WB_PRESETS.iter().map(|p| p.0))
                                {
                                    if ui.selectable_label(current == name, name).clicked() {
                                        choice = Some(name);
                                    }
                                }
                            })
                            .response
                            .accessible_name("Balance des blancs");
                        let picking = self.eyedropper;
                        if ui
                            .selectable_label(picking, "Pipette")
                            .on_hover_text("W : cliquez un gris de la photo")
                            .clicked()
                        {
                            self.eyedropper = !picking;
                        }
                        match choice {
                            Some("Telle quelle") => {
                                self.settings.white_balance = WhiteBalance::AsShot;
                                commits.push("Balance : telle quelle".into());
                            }
                            Some("Auto") => {
                                if let Some(photo) = &self.photo {
                                    let t = photo.auto_white_balance();
                                    self.settings.white_balance = WhiteBalance::Custom {
                                        temperature: t.temperature,
                                        tint: t.tint,
                                    };
                                    self.wb_label = "Auto";
                                    commits.push("Balance : auto".into());
                                }
                            }
                            Some(name) => {
                                if let Some(p) = WB_PRESETS.iter().find(|p| p.0 == name) {
                                    self.settings.white_balance = WhiteBalance::Custom {
                                        temperature: p.1,
                                        tint: p.2,
                                    };
                                    commits.push(format!("Balance : {}", name.to_lowercase()));
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
                        commits.push(format!(
                            "Balance : {} K, {}",
                            format_value(temperature, 0, false),
                            format_value(tint, 0, true)
                        ));
                    }
                    sub_title(ui, "Tonalité");
                    let l = self.fields(ui, &TONE);
                    keep(l, &mut commits);
                    sub_title(ui, "Présence");
                    let l = self.fields(ui, &PRESENCE);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Light {
            egui::CollapsingHeader::new(RichText::new("Courbe des tonalités").strong())
                .default_open(true)
                .show(ui, |ui| {
                    curve_preview(ui, &self.settings);
                    let l = self.fields(ui, &CURVE);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Color {
            egui::CollapsingHeader::new(RichText::new("Mélangeur de couleurs").strong())
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, name) in ["Teinte", "Saturation", "Luminance"].iter().enumerate() {
                            ui.selectable_value(&mut self.hsl_tab, i, *name);
                        }
                    });
                    let kind = ["Teinte", "Saturation", "Luminance"][self.hsl_tab];
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
                                name.to_lowercase(),
                                format_value(v, 0, true)
                            ));
                        }
                    }
                });
        }

        if self.tool == Tool::Color {
            egui::CollapsingHeader::new(RichText::new("Étalonnage des couleurs").strong())
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, name) in ["Ombres", "Tons moyens", "Hautes lum.", "Global"]
                            .iter()
                            .enumerate()
                        {
                            ui.selectable_value(&mut self.grading_tab, i, *name);
                        }
                    });
                    let names = ["Ombres", "Tons moyens", "Hautes lumières", "Global"];
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
                        label: "Fusion",
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
                        commits.push(format!(
                            "Étalonnage {} : {h:.0}°, {s:.0}, {}",
                            names[self.grading_tab].to_lowercase(),
                            format_value(l, 0, true)
                        ));
                    }
                });
        }

        if self.tool == Tool::Detail {
            egui::CollapsingHeader::new(RichText::new("Détail").strong())
                .default_open(true)
                .show(ui, |ui| {
                    sub_title(ui, "Accentuation");
                    let l = self.fields(ui, &SHARPEN);
                    keep(l, &mut commits);
                    sub_title(ui, "Réduction du bruit");
                    let l = self.fields(ui, &NOISE);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Geometry {
            egui::CollapsingHeader::new(RichText::new("Corrections de l'objectif").strong())
                .default_open(true)
                .show(ui, |ui| {
                    if ui
                        .checkbox(
                            &mut self.settings.lens.remove_ca,
                            "Supprimer l'aberration chromatique",
                        )
                        .changed()
                    {
                        commits.push(if self.settings.lens.remove_ca {
                            "Aberration chromatique supprimée".into()
                        } else {
                            "Aberration chromatique gardée".into()
                        });
                    }
                    let l = self.fields(ui, &LENS);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Geometry {
            egui::CollapsingHeader::new(RichText::new("Transformation").strong())
                .default_open(true)
                .show(ui, |ui| {
                    let l = self.fields(ui, &TRANSFORM);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Detail {
            egui::CollapsingHeader::new(RichText::new("Effets").strong())
                .default_open(true)
                .show(ui, |ui| {
                    sub_title(ui, "Vignetage après recadrage");
                    let l = self.fields(ui, &VIGNETTE);
                    keep(l, &mut commits);
                    sub_title(ui, "Grain");
                    let l = self.fields(ui, &GRAIN);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Color {
            egui::CollapsingHeader::new(RichText::new("Étalonnage de l'appareil photo").strong())
                .default_open(true)
                .show(ui, |ui| {
                    let l = self.fields(ui, &CALIBRATION);
                    keep(l, &mut commits);
                });
        }

        if self.tool == Tool::Geometry {
            let cropping = self.crop_tool.is_some();
            egui::CollapsingHeader::new(RichText::new("Recadrage").strong())
                .open(cropping.then_some(true))
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let label = if cropping {
                            "Terminer (R)"
                        } else {
                            "Outil Recadrage (R)"
                        };
                        if ui.selectable_label(cropping, label).clicked() {
                            self.act("crop");
                        }
                        if ui.button("Réinitialiser").clicked() {
                            self.settings.crop = Crop::default();
                            commits.push("Recadrage réinitialisé".into());
                        }
                    });
                    if let Some(tool) = &mut self.crop_tool {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Proportions").color(theme::DIM));
                            let before = tool.aspect;
                            egui::ComboBox::from_id_salt("proportions")
                                .selected_text(crop::ASPECTS[tool.aspect].0)
                                .show_ui(ui, |ui| {
                                    for (k, (name, _)) in crop::ASPECTS.iter().enumerate() {
                                        ui.selectable_value(&mut tool.aspect, k, *name);
                                    }
                                })
                                .response
                                .accessible_name("Proportions");
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
                        commits.push(format!(
                            "Angle {}",
                            format_value(self.settings.crop.angle, 1, true)
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
    let (rect, r) = ui.allocate_exact_size(vec2(96.0, 74.0), egui::Sense::click());
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
fn paint_meta(painter: &egui::Painter, rect: egui::Rect, meta: &Meta) {
    let y = rect.max.y - 13.0;
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
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Le rectangle de taille `w × h` qui tient dans `area`, centré.
fn fit(area: egui::Rect, w: usize, h: usize) -> egui::Rect {
    if w == 0 || h == 0 {
        return area;
    }
    let s = (area.width() / w as f32).min(area.height() / h as f32);
    egui::Rect::from_center_size(area.center(), vec2(w as f32 * s, h as f32 * s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_extensions() {
        assert!(is_raw(Path::new("a/IMG_1.CR3")));
        assert!(is_raw(Path::new("b.dng")));
        assert!(!is_raw(Path::new("c.jpg")));
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
