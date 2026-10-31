//! Le travail de fond : ouvrir un RAW, rendre l'aperçu, fabriquer les vignettes du
//! film, exporter. L'interface ne fait jamais ce travail elle‑même : elle envoie une
//! demande et reçoit un [`Event`] quand c'est prêt, sans jamais bloquer.
//!
//! Le rendu suit la règle « seule la dernière demande compte » : quand un curseur
//! bouge, le rendu en cours est annulé à sa prochaine étape et le nouveau démarre.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use luma_color::OutputSpace;
use luma_core::{Cancel, RgbImage};
use luma_project::{Settings, xmp};
use luma_render::{Photo, Size};

/// Une image prête à afficher (sRGB, 8 bits, RGBA).
pub struct Frame {
    pub generation: u64,
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
    pub histogram: [[u32; 256]; 3],
    pub millis: f32,
    /// Fini sur le GPU (sinon sur le processeur).
    pub gpu: bool,
    /// Une copie réduite de la base (linéaire), quand un masque a une plage : l'écran
    /// y lit les tons et les couleurs pour montrer sa zone (touche O).
    pub scene: Option<Arc<RgbImage>>,
    /// Les poussières trouvées sur la base, quand on les a demandées.
    pub dust: Option<Vec<luma_project::Spot>>,
    /// L'instrument choisi (forme d'onde, parade, vectorscope), rendu sur l'aperçu.
    pub scope: Option<luma_image::scopes::ScopeImage>,
    /// L'aperçu sans ce qu'on a peint dessus (écrêtage, alerte de gamut), quand on y a
    /// peint quelque chose : la mesure au point y lit les vraies couleurs.
    pub clean: Option<Vec<u8>>,
}

/// Les corrections d'objectif mesurées sur le JPEG du boîtier.
#[derive(Clone, Copy, Debug)]
pub struct LensMeasure {
    pub distortion: f32,
    pub vignetting: f32,
    /// Le recadrage qui retire les bords vides (avec la Transformation de la photo).
    pub crop: luma_project::Crop,
}

/// Ce qu'une mesure de film a mesuré : une pellicule, le rendu du boîtier, ou l'accord
/// sur une photo de référence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasureKind {
    Film,
    Camera,
    Reference,
}

/// Ce qu'on mesure contre le rendu neutre d'une photo.
pub enum FilmSource {
    /// Le scan de la même scène sur pellicule.
    Scan(PathBuf),
    /// Le JPEG que le boîtier a rangé dans le RAW `raw` (et dans `others`, d'autres RAW
    /// choisis : ceux d'un autre boîtier sont laissés de côté) ; `camera` nomme le film.
    Camera {
        raw: PathBuf,
        others: Vec<PathBuf>,
        camera: String,
    },
    /// La photo de référence `path` (développée avec ses réglages), à laquelle on
    /// accorde la photo développée avec `settings`.
    Reference {
        path: PathBuf,
        settings: Box<Settings>,
    },
}

pub enum Event {
    Loaded {
        path: PathBuf,
        result: Result<(Arc<Photo>, Option<Box<Settings>>), String>,
    },
    /// La même photo, relue : depuis son doublon de travail une fois écrit (plus rapide,
    /// bien plus légère en mémoire), ou depuis l'original quand le cache est vidé.
    Reloaded {
        path: PathBuf,
        photo: Arc<Photo>,
    },
    Rendered(Box<Frame>),
    /// Un premier coup d'œil sur la photo qui s'ouvre, avant son rendu : l'aperçu
    /// intégré au RAW, ou la vignette enregistrée d'une photo retouchée.
    Glimpse {
        path: PathBuf,
        size: [usize; 2],
        rgba: Vec<u8>,
    },
    Thumbnail {
        path: PathBuf,
        size: [usize; 2],
        rgba: Vec<u8>,
    },
    /// L'avancement d'une fusion (empilement, panorama).
    MergeProgress {
        fraction: f32,
        label: String,
    },
    /// La fusion est enregistrée (son DNG) ou a échoué.
    Merged {
        result: Result<PathBuf, String>,
    },
    /// Les corrections d'objectif d'une photo d'un lot, mesurées sur le JPEG du boîtier :
    /// distorsion, vignetage et le recadrage sans bords vides ; ou pourquoi pas.
    LensMeasured {
        path: PathBuf,
        result: Result<LensMeasure, String>,
    },
    /// La balance automatique d'une photo d'un lot.
    AutoBalanced {
        path: PathBuf,
        balance: luma_color::TempTint,
    },
    /// Une photo de la comparaison, développée avec ses réglages.
    Compared {
        tag: u64,
        index: usize,
        size: [usize; 2],
        rgba: Vec<u8>,
    },
    /// La photo sans ses retouches (sa géométrie gardée), pour l'avant / après côte à
    /// côte.
    Before {
        tag: u64,
        size: [usize; 2],
        rgba: Vec<u8>,
    },
    /// La photo de référence, développée avec ses réglages.
    /// La photo de référence n'a pas pu être développée : pourquoi.
    ReferenceFailed {
        path: PathBuf,
        error: String,
    },
    Reference {
        path: PathBuf,
        size: [usize; 2],
        rgba: Vec<u8>,
    },
    /// L'aperçu d'un préréglage sur la photo affichée.
    Preview {
        tag: u64,
        index: usize,
        size: [usize; 2],
        rgba: Vec<u8>,
    },
    /// Une pellicule (ou le rendu du boîtier, `camera`) est mesurée : le nom du film,
    /// l'écart (OKLab × 100) entre le numérique et la pellicule, avant et après, et le
    /// nombre de photos mesurées.
    Measured {
        result: Result<(String, f32, f32, usize), String>,
        kind: MeasureKind,
    },
    /// Une photo de l'export est écrite (ou a échoué) : `done` sur `total`.
    Exported {
        done: usize,
        total: usize,
        result: Result<PathBuf, String>,
    },
}

/// Ce qu'on fusionne.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MergeKind {
    Stack(luma_merge::StackMode, bool),
    /// La projection, et les bords étirés pour remplir le rectangle (au lieu d'être rognés).
    Panorama(luma_merge::Projection, bool),
}

/// Une fusion à faire : les RAW, le type, le rognage automatique, la taille.
pub struct MergeJob {
    pub paths: Vec<PathBuf>,
    pub kind: MergeKind,
    pub auto_crop: bool,
    pub full_size: bool,
}

// Les réglages de la fenêtre Exporter : ceux du moteur, partagés avec la ligne de
// commande et ses préréglages.
pub use luma_render::export::{
    Format, NameFields, Options as ExportOptions, output_path as export_path,
};

/// Une photo à exporter : déjà ouverte (la photo affichée) ou à ouvrir.
pub struct ExportJob {
    pub source: PathBuf,
    pub photo: Option<Arc<Photo>>,
    pub settings: Settings,
}

/// Développe et écrit une photo selon les options.
pub fn export_one(
    photo: &Photo,
    settings: &Settings,
    out: &Path,
    options: &ExportOptions,
    meta: &xmp::Meta,
) -> Result<(), String> {
    luma_render::export::develop(photo, settings, out, options, meta)
}

struct RenderJob {
    photo: Arc<Photo>,
    settings: Settings,
    size: Size,
    generation: u64,
    clipping: bool,
    /// L'épreuvage à l'écran, s'il est actif.
    proof: Option<Proofed>,
    /// Joindre une copie réduite de la base (la retouche y cherche ses sources).
    scene: bool,
    /// Montrer la vue des taches à la place de la photo.
    dust_view: bool,
    /// Chercher les poussières sur la base.
    find_dust: bool,
    scope: luma_image::scopes::Scope,
}

/// Ce qu'un rendu joint à l'image : l'écrêtage peint par‑dessus, une copie réduite de
/// la base, la vue des taches, les poussières trouvées.
#[derive(Clone, Copy, Debug, Default)]
pub struct Extras {
    pub clipping: bool,
    pub scene: bool,
    /// La vue des taches (les poussières du capteur en points sombres).
    pub dust_view: bool,
    /// Les poussières à chercher, jointes au rendu.
    pub find_dust: bool,
    /// L'instrument à rendre avec l'aperçu (l'histogramme est toujours joint).
    pub scope: luma_image::scopes::Scope,
}

/// L'épreuvage demandé : la table, la simulation de l'impression, l'alerte de gamut.
#[derive(Clone)]
pub struct Proofed {
    pub table: Arc<luma_color::proof::Proof>,
    pub simulate: bool,
    pub gamut: bool,
}

/// Le sidecar XMP d'un RAW (`IMG_0001.xmp` ou `IMG_0001.CR3.xmp`).
pub fn sidecar_path(raw: &Path) -> PathBuf {
    xmp::sidecar_path(raw)
}

/// Les réglages enregistrés à côté d'un RAW, s'il y en a.
pub fn read_sidecar(raw: &Path) -> Option<Settings> {
    // Sans XMP à côté : les réglages qu'un RAW ou un DNG porte en lui.
    luma_library::photo_settings(raw)
}

/// La note, le drapeau et le libellé enregistrés à côté d'un RAW ; sans XMP à côté,
/// ceux que le fichier porte en lui (posés par un autre logiciel ou par le boîtier).
pub fn read_meta(raw: &Path) -> xmp::Meta {
    luma_library::photo_meta(raw)
}

/// Écrit le sidecar d'un RAW : réglages, note et drapeau.
pub fn write_sidecar(raw: &Path, settings: &Settings, meta: &xmp::Meta) -> Result<(), String> {
    let target = sidecar_path(raw);
    xmp::save(&target, &xmp::write_with_meta(settings, meta))
        .map_err(|e| format!("{} : {e}", target.display()))
}

/// Change la note, le drapeau ou les mots‑clés d'une photo qui n'est pas ouverte :
/// ses réglages enregistrés sont gardés tels quels ; sans réglages, seul un XMP de
/// métadonnées est écrit, et la photo garde son rendu par défaut.
pub fn write_meta(raw: &Path, meta: &xmp::Meta) -> Result<(), String> {
    luma_library::write_photo_meta(raw, meta)
}

/// La couleur de l'indicateur d'écrêtage (touche J) pour un pixel, couche par couche :
/// les couches brûlées gardent leur couleur (rouge seul, jaune pour rouge et vert…,
/// blanc quand les trois le sont) ; bleu où tout est noir.
pub fn clip_color(p: [u8; 3]) -> Option<[u8; 3]> {
    if p.contains(&255) {
        Some(p.map(|v| if v == 255 { 255 } else { 0 }))
    } else if p == [0, 0, 0] {
        Some([40, 90, 255])
    } else {
        None
    }
}

/// RGB 8 bits → RGBA, avec en option l'indicateur d'écrêtage ([`clip_color`]).
pub fn to_rgba(rgb: &[u8], clipping: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgb.len() / 3 * 4);
    for p in rgb.as_chunks::<3>().0 {
        let px = if clipping {
            clip_color(*p).unwrap_or(*p)
        } else {
            *p
        };
        out.extend_from_slice(&[px[0], px[1], px[2], 255]);
    }
    out
}

pub struct Worker {
    events: Receiver<Event>,
    sender: Sender<Event>,
    render: Sender<RenderJob>,
    thumbs: Sender<PathBuf>,
    running: Arc<Mutex<Cancel>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
}

impl Worker {
    /// `repaint` réveille l'interface quand un résultat arrive.
    pub fn new(repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(repaint);
        let (sender, events) = channel();
        let running = Arc::new(Mutex::new(Cancel::new()));

        let (render, jobs) = channel::<RenderJob>();
        {
            let (sender, running, repaint) = (sender.clone(), running.clone(), repaint.clone());
            std::thread::Builder::new()
                .name("luma-rendu".into())
                .spawn(move || render_loop(jobs, sender, running, repaint))
                .expect("fil de rendu");
        }
        let (thumbs, queue) = channel::<PathBuf>();
        {
            let (sender, repaint) = (sender.clone(), repaint.clone());
            std::thread::Builder::new()
                .name("luma-vignettes".into())
                .spawn(move || thumbnail_loop(queue, sender, repaint))
                .expect("fil des vignettes");
        }
        Self {
            events,
            sender,
            render,
            thumbs,
            running,
            repaint,
        }
    }

    /// Les résultats arrivés depuis le dernier appel.
    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }

    pub fn open(&self, path: PathBuf) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::spawn(move || {
            // Aussitôt, un coup d'œil, pendant que le RAW se décode.
            if let Some((size, rgba)) = glimpse(&path) {
                let _ = sender.send(Event::Glimpse {
                    path: path.clone(),
                    size,
                    rgba,
                });
                repaint();
            }
            let result = caught(|| {
                crate::dialogs::open_photo(&path)
                    .map(|p| (Arc::new(p), read_sidecar(&path).map(Box::new)))
                    .map_err(|e| e.to_string())
            });
            let photo = result.as_ref().ok().map(|(p, _)| p.clone());
            let _ = sender.send(Event::Loaded {
                path: path.clone(),
                result,
            });
            repaint();
            // Un RAW ouvert depuis l'original : son doublon de travail s'écrit
            // maintenant, puis la photo passe dessus.
            // Une copie virtuelle partage le doublon de son original.
            let source = luma_project::copies::source(&path);
            if let (Some(photo), Some(dir)) = (photo, crate::dialogs::duplicates_dir())
                && photo.wants_duplicate()
                && duplicate_fits(&photo)
                && background()
                    .install(|| photo.write_duplicate(&source, &dir))
                    .is_ok()
            {
                drop(photo);
                if let Ok(next) = Photo::open_with_duplicates(&source, &dir)
                    && next.from_duplicate()
                {
                    let _ = sender.send(Event::Reloaded {
                        path,
                        photo: Arc::new(next),
                    });
                    repaint();
                }
            }
        });
    }

    /// Relit la photo depuis l'original (le cache vient d'être vidé) ; les réglages
    /// restent ceux de l'app.
    pub fn reload_original(&self, path: PathBuf) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::spawn(move || {
            if let Ok(photo) = Photo::open(luma_project::copies::source(&path)) {
                let _ = sender.send(Event::Reloaded {
                    path,
                    photo: Arc::new(photo),
                });
                repaint();
            }
        });
    }

    /// Demande un rendu ; celui en cours, devenu inutile, est annulé.
    pub fn render(
        &self,
        photo: Arc<Photo>,
        settings: Settings,
        size: Size,
        generation: u64,
        extras: Extras,
        proof: Option<Proofed>,
    ) {
        self.running.lock().expect("annulation").cancel();
        let _ = self.render.send(RenderJob {
            photo,
            settings,
            size,
            generation,
            clipping: extras.clipping,
            proof,
            scene: extras.scene,
            dust_view: extras.dust_view,
            find_dust: extras.find_dust,
            scope: extras.scope,
        });
    }

    pub fn thumbnail(&self, path: PathBuf) {
        let _ = self.thumbs.send(path);
    }

    /// Fusionne des RAW en un DNG, à côté du premier.
    /// Mesure une pellicule : `photo` en numérique (son rendu neutre) et `source`, la
    /// même scène sur film (son scan), ou le rendu du boîtier (le JPEG rangé dans le RAW,
    /// nommé d'après le boîtier `camera`). La LUT est enregistrée dans `dir` et chargée.
    pub fn measure_film(&self, photo: Arc<Photo>, source: FilmSource, dir: PathBuf) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::Builder::new()
            .name("luma-mesure".into())
            .spawn(move || {
                let result = caught(|| -> Result<(String, f32, f32, usize), String> {
                    use luma_merge::film;
                    use rayon::prelude::*;
                    let (pairs, stem) = match &source {
                        FilmSource::Scan(scan) => (
                            vec![film::Pair {
                                digital: film::reference(&photo).map_err(|e| e.to_string())?,
                                film: film::load_scan(scan).map_err(|e| e.to_string())?,
                                aligned: false,
                            }],
                            scan.file_stem().map_or_else(
                                || "Pellicule".to_owned(),
                                |s| s.to_string_lossy().into_owned(),
                            ),
                        ),
                        FilmSource::Camera {
                            raw,
                            others,
                            camera,
                        } => {
                            let mut pairs =
                                vec![film::camera_pair(&photo, raw).map_err(|e| e.to_string())?];
                            // Les autres photos choisies du même boîtier, ouvertes ensemble.
                            let (make, model) = (&photo.raw().make, &photo.raw().model);
                            pairs.extend(
                                others
                                    .par_iter()
                                    .filter_map(|p| {
                                        let other = Photo::open(p).ok()?;
                                        let r = other.raw();
                                        (r.make == *make && r.model == *model)
                                            .then(|| film::camera_pair(&other, p).ok())
                                            .flatten()
                                    })
                                    .collect::<Vec<_>>(),
                            );
                            (pairs, format!("{camera} JPEG"))
                        }
                        FilmSource::Reference { path, .. } => (
                            Vec::new(),
                            format!(
                                "Accord {}",
                                path.file_stem()
                                    .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
                            ),
                        ),
                    };
                    // Un nom libre : « Portra », « Portra 2 »…
                    let mut name = stem.clone();
                    let mut k = 2;
                    while dir.join(format!("{name}.cube")).exists()
                        || luma_image::cube::get(&name).is_some()
                    {
                        name = format!("{stem} {k}");
                        k += 1;
                    }
                    let (m, kept) = match &source {
                        FilmSource::Scan(_) => (
                            film::measure(&pairs, 33, &name).map_err(|e| e.to_string())?,
                            1,
                        ),
                        FilmSource::Camera { .. } => {
                            film::measure_tolerant(&pairs, 33, &name).map_err(|e| e.to_string())?
                        }
                        FilmSource::Reference { path, settings } => {
                            let source = film::reference_with(&photo, settings)
                                .map_err(|e| e.to_string())?;
                            let other =
                                crate::dialogs::open_photo(path).map_err(|e| e.to_string())?;
                            let own =
                                read_sidecar(path).unwrap_or_else(|| other.default_settings());
                            let reference =
                                film::reference_with(&other, &own).map_err(|e| e.to_string())?;
                            (
                                film::transfer(&source, &reference, 33, &name)
                                    .map_err(|e| e.to_string())?,
                                1,
                            )
                        }
                    };
                    let path = dir.join(format!("{name}.cube"));
                    luma_core::save(&path, m.cube.to_text())
                        .map_err(|e| format!("{} : {e}", path.display()))?;
                    luma_image::cube::register(&name, m.cube);
                    Ok((name, m.before, m.after, kept))
                });
                let kind = match source {
                    FilmSource::Scan(_) => MeasureKind::Film,
                    FilmSource::Camera { .. } => MeasureKind::Camera,
                    FilmSource::Reference { .. } => MeasureKind::Reference,
                };
                let _ = sender.send(Event::Measured { result, kind });
                repaint();
            })
            .expect("fil de mesure");
    }

    pub fn merge(&self, mut job: MergeJob, cancel: Cancel) {
        // Une copie virtuelle fusionne les pixels de son original, une seule fois.
        let mut sources: Vec<PathBuf> = Vec::new();
        for p in &job.paths {
            let source = luma_project::copies::source(p);
            if !sources.contains(&source) {
                sources.push(source);
            }
        }
        job.paths = sources;
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::Builder::new()
            .name("luma-fusion".into())
            .spawn(move || {
                let progress_sender = sender.clone();
                let progress_repaint = repaint.clone();
                let last = Mutex::new(Instant::now());
                let progress = move |fraction: f32, label: &str| {
                    // Au plus vingt nouvelles par seconde.
                    let mut l = last.lock().expect("avancement");
                    if l.elapsed().as_millis() > 50 || fraction >= 1.0 {
                        *l = Instant::now();
                        let _ = progress_sender.send(Event::MergeProgress {
                            fraction,
                            label: label.to_owned(),
                        });
                        progress_repaint();
                    }
                };
                let result = caught(|| -> Result<PathBuf, String> {
                    let size = if job.full_size {
                        Size::Full
                    } else {
                        Size::Fit(2048)
                    };
                    let frames = luma_merge::RawFiles {
                        paths: job.paths.clone(),
                        size,
                    };
                    let (merged, suffix) = match job.kind {
                        MergeKind::Stack(mode, align) => (
                            luma_merge::stack(&frames, mode, align, &progress, &cancel)
                                .map_err(|e| e.to_string())?,
                            match mode {
                                luma_merge::StackMode::Stars => "Etoiles",
                                luma_merge::StackMode::Median => "Mediane",
                                luma_merge::StackMode::Maximum => "Maximum",
                                luma_merge::StackMode::Mean => "Empilement",
                                luma_merge::StackMode::Hdr => "HDR",
                                luma_merge::StackMode::Focus => "Mise-au-point",
                            },
                        ),
                        MergeKind::Panorama(p, warp) => {
                            let mut merged = luma_merge::panorama(&frames, p, &progress, &cancel)
                                .map_err(|e| e.to_string())?;
                            if warp {
                                merged.warp_edges();
                            }
                            (merged, "Pano")
                        }
                    };
                    let reference = &job.paths[job.paths.len() / 2];
                    let template =
                        luma_raw::RawPhoto::open(reference).map_err(|e| e.to_string())?;
                    let out = luma_merge::output_path(&job.paths[0], suffix);
                    merged
                        .save(&out, &template, job.auto_crop)
                        .map_err(|e| e.to_string())?;
                    Ok(out)
                });
                let _ = sender.send(Event::Merged { result });
                repaint();
            })
            .expect("fil de fusion");
    }

    /// Les aperçus des préréglages (petits rendus), dans l'ordre.
    /// La balance automatique de chaque photo du lot, l'une après l'autre (sur un aperçu
    /// réduit : quelques dizaines de millisecondes par photo depuis son doublon).
    pub fn auto_balance(&self, paths: Vec<PathBuf>) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::Builder::new()
            .name("luma-balance-auto".into())
            .spawn(move || {
                for path in paths {
                    let Ok(balance) = caught(|| {
                        let photo = crate::dialogs::open_photo(&path).map_err(|e| e.to_string())?;
                        Ok(photo.auto_white_balance())
                    }) else {
                        continue;
                    };
                    let _ = sender.send(Event::AutoBalanced { path, balance });
                    repaint();
                }
            })
            .expect("fil de la balance automatique");
    }

    /// Les corrections d'objectif de chaque photo d'après le JPEG que son boîtier a rangé
    /// dans le RAW, l'une après l'autre (chacune la sienne : la focale change tout) ; une
    /// photo dont le fichier porte déjà ses corrections est laissée de côté.
    pub fn lens_from_camera(&self, paths: Vec<PathBuf>) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::Builder::new()
            .name("luma-objectif-boitier".into())
            .spawn(move || {
                for path in paths {
                    let result = caught(|| -> Result<LensMeasure, String> {
                        use luma_merge::camera_lens;
                        let photo = crate::dialogs::open_photo(&path).map_err(|e| e.to_string())?;
                        if photo.has_camera_corrections() {
                            return Err("corrections du boîtier déjà dans le fichier".into());
                        }
                        // Le JPEG décodé une fois, ses détails cherchés une fois.
                        let (distortion, vignetting) =
                            camera_lens::corrections(&photo, &path).map_err(|e| e.to_string())?;
                        let (distortion, vignetting) = (distortion.round(), vignetting.round());
                        let mut s = Settings::default();
                        s.lens.distortion = distortion;
                        s.lens.vignetting = vignetting;
                        s.lens.vignetting_midpoint = camera_lens::MIDPOINT;
                        let own = read_sidecar(&path).unwrap_or_else(|| photo.default_settings());
                        let (w, h) = photo.full_size();
                        Ok(LensMeasure {
                            distortion: s.lens.distortion,
                            vignetting,
                            crop: luma_image::lens::filled_crop(
                                &s.lens,
                                &own.transform,
                                w.max(1),
                                h.max(1),
                                own.crop.angle,
                            ),
                        })
                    });
                    let _ = sender.send(Event::LensMeasured { path, result });
                    repaint();
                }
            })
            .expect("fil des corrections d'objectif");
    }

    /// Développe les photos à comparer, chacune avec ses réglages, au grand côté `side`,
    /// l'une après l'autre (la première arrive vite).
    pub fn compare(&self, paths: Vec<PathBuf>, side: usize, tag: u64) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::Builder::new()
            .name("luma-comparaison".into())
            .spawn(move || {
                for (index, path) in paths.iter().enumerate() {
                    let Ok(r) = caught(|| {
                        let photo = crate::dialogs::open_photo(path).map_err(|e| e.to_string())?;
                        let settings = read_sidecar(path).unwrap_or_default();
                        photo
                            .render(&settings, Size::Fit(side), &Cancel::new())
                            .map_err(|e| e.to_string())
                    }) else {
                        continue;
                    };
                    let _ = sender.send(Event::Compared {
                        tag,
                        index,
                        size: [r.image.width, r.image.height],
                        rgba: luma_image::encode8_rgba(&r.image, OutputSpace::Srgb),
                    });
                    repaint();
                }
            })
            .expect("fil de la comparaison");
    }

    /// Développe `photo` avec `settings` (l'« avant » de la vue côte à côte), à `size`.
    pub fn render_before(&self, photo: Arc<Photo>, settings: Settings, size: Size, tag: u64) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::Builder::new()
            .name("luma-avant".into())
            .spawn(move || {
                let Ok(r) = caught(|| {
                    photo
                        .render(&settings, size, &Cancel::new())
                        .map_err(|e| e.to_string())
                }) else {
                    return;
                };
                let _ = sender.send(Event::Before {
                    tag,
                    size: [r.image.width, r.image.height],
                    rgba: luma_image::encode8_rgba(&r.image, OutputSpace::Srgb),
                });
                repaint();
            })
            .expect("fil de l'avant");
    }

    /// Développe la photo de référence `path` avec ses propres réglages, au grand côté
    /// `side`.
    pub fn reference(&self, path: PathBuf, side: usize) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::Builder::new()
            .name("luma-reference".into())
            .spawn(move || {
                let rendered = caught(|| {
                    let photo = crate::dialogs::open_photo(&path).map_err(|e| e.to_string())?;
                    let settings = read_sidecar(&path).unwrap_or_else(|| photo.default_settings());
                    photo
                        .render(&settings, Size::Fit(side), &Cancel::new())
                        .map_err(|e| e.to_string())
                });
                // Une référence illisible le dit : sans réponse, l'app l'attendrait
                // toujours.
                let event = match rendered {
                    Ok(r) => Event::Reference {
                        path,
                        size: [r.image.width, r.image.height],
                        rgba: luma_image::encode8_rgba(&r.image, OutputSpace::Srgb),
                    },
                    Err(error) => Event::ReferenceFailed { path, error },
                };
                let _ = sender.send(event);
                repaint();
            })
            .expect("fil de la référence");
    }

    pub fn previews(&self, photo: Arc<Photo>, settings: Vec<Settings>, tag: u64) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::spawn(move || {
            for (index, s) in settings.iter().enumerate() {
                let Ok(r) = caught(|| {
                    photo
                        .render(s, Size::Fit(240), &Cancel::new())
                        .map_err(|e| e.to_string())
                }) else {
                    continue;
                };
                let _ = sender.send(Event::Preview {
                    tag,
                    index,
                    size: [r.image.width, r.image.height],
                    rgba: luma_image::encode8_rgba(&r.image, OutputSpace::Srgb),
                });
                repaint();
            }
        });
    }

    /// Exporte les photos l'une après l'autre, en arrière-plan.
    /// Exporte chaque photo vers chacun des réglages (ceux de la fenêtre, des
    /// préréglages) : chaque photo est ouverte une fois ; deux fichiers du même nom sont
    /// numérotés.
    pub fn export(&self, jobs: Vec<ExportJob>, targets: Vec<ExportOptions>) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::spawn(move || {
            let total = jobs.len() * targets.len();
            let mut taken = std::collections::HashSet::new();
            luma_render::export::reserve_sources(
                &mut taken,
                jobs.iter().map(|j| j.source.as_path()),
            );
            let mut done = 0;
            for (k, job) in jobs.into_iter().enumerate() {
                let photo = match job.photo {
                    Some(p) => Ok(p),
                    None => caught(|| {
                        crate::dialogs::open_photo(&job.source)
                            .map(Arc::new)
                            .map_err(|e| e.to_string())
                    }),
                };
                // Le nom vient du modèle : date, boîtier, note… et le rang dans l'export.
                let meta = read_meta(&job.source);
                for options in &targets {
                    let result = photo
                        .clone()
                        .and_then(|photo| {
                            let fields = NameFields::of_photo(&job.source, &photo, &meta, k + 1);
                            let out = luma_render::export::unique(
                                export_path(&job.source, options, &fields),
                                &mut taken,
                            );
                            caught(|| export_one(&photo, &job.settings, &out, options, &meta))
                                .map(|()| out)
                        })
                        .map_err(|e| format!("{} : {e}", job.source.display()));
                    done += 1;
                    let _ = sender.send(Event::Exported {
                        done,
                        total,
                        result,
                    });
                    repaint();
                }
            }
        });
    }
}

fn render_loop(
    jobs: Receiver<RenderJob>,
    sender: Sender<Event>,
    running: Arc<Mutex<Cancel>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
) {
    // Le GPU, s'il y en a un (LUMA_GPU=0 pour tout faire sur le processeur).
    let mut gpu = if std::env::var("LUMA_GPU").is_ok_and(|v| v == "0") {
        None
    } else {
        luma_gpu::Gpu::new()
    };
    // L'image du dernier aperçu rendu sur le processeur, reprise par le suivant.
    let mut spare = Vec::new();
    serve(jobs, |job| {
        render_job(job, &mut gpu, &mut spare, &sender, &running, &repaint)
    });
}

/// `f`, une panique devenant une erreur : un calcul qui échoue rend un message à l'app,
/// qui sinon attendrait sans fin un résultat qui ne viendrait pas (une photo qui
/// s'ouvre, un export, une fusion).
fn caught<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err(crate::i18n::tr("Erreur inattendue pendant le calcul").to_owned()))
}

/// Sert les demandes une à une, seule la dernière arrivée comptant. Une demande qui
/// panique (le défaut d'une étape, une photo singulière) est abandonnée et le fil
/// continue : sans cela, plus aucun aperçu n'arriverait de la séance.
fn serve<J>(jobs: Receiver<J>, mut each: impl FnMut(J)) {
    while let Ok(mut job) = jobs.recv() {
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| each(job))).is_err() {
            crate::journal("un rendu a échoué : l'aperçu suivant repart de zéro");
        }
    }
}

/// Un aperçu : la base, puis la fin sur le GPU ou le processeur, l'épreuvage, les
/// histogrammes et les surimpressions.
/// La plus grande image gardée d'un rendu au suivant : 5,6 Mpx (64 Mo).
const SPARE_PIXELS: usize = (64 << 20) / 12;

fn render_job(
    job: RenderJob,
    gpu: &mut Option<luma_gpu::Gpu>,
    spare: &mut Vec<[f32; 3]>,
    sender: &Sender<Event>,
    running: &Mutex<Cancel>,
    repaint: &Arc<dyn Fn() + Send + Sync>,
) {
    {
        let cancel = Cancel::new();
        *running.lock().expect("annulation") = cancel.clone();
        let start = Instant::now();
        let mut timings = luma_core::Timings::default();
        let Ok(base) = job
            .photo
            .base(&job.settings, job.size, &cancel, &mut timings)
        else {
            return;
        };
        let (w, h) = (base.image.width, base.image.height);
        let layers = luma_image::look_layers(&job.settings);
        let usable = gpu.as_ref().filter(|g| {
            g.can_finish(&job.settings, w, h, base.scale)
                && layers
                    .iter()
                    .all(|(_, s, _)| g.can_finish(s, w, h, base.scale))
        });
        let finished = usable.and_then(|g| {
            finish_on_gpu(
                g,
                &job.photo,
                &job.settings,
                job.size,
                &base,
                &layers,
                &cancel,
                &mut timings,
            )
        });
        if usable.is_some() && finished.is_none() {
            // Le GPU ne répond plus : le processeur finit, cette fois et les suivantes.
            crate::journal("le GPU ne répond plus : rendu sur le processeur");
            *gpu = None;
        }
        let (mut rgba, on_gpu) = match finished {
            Some(rgba) => (rgba, true),
            None => {
                let photo = &job.photo;
                let Ok(image) = luma_image::finish_layers_in(
                    &base.image,
                    std::mem::take(spare),
                    base.scale,
                    &job.settings,
                    &mut |look, t| photo.base(look, job.size, &cancel, t).map(|b| b.image),
                    &cancel,
                    &mut timings,
                ) else {
                    return;
                };
                let rgba = luma_image::encode8_rgba(&image, OutputSpace::Srgb);
                // Gardée seulement à la taille d'un aperçu : la pleine définition (le
                // zoom 1:1, 288 Mo pour 24 Mpx) ne reste pas en mémoire entre deux rendus.
                if image.data.len() <= SPARE_PIXELS {
                    *spare = image.data;
                }
                (rgba, false)
            }
        };
        // L'épreuvage : l'aperçu tel qu'il sera imprimé (l'histogramme le suit), et les
        // couleurs d'origine hors de portée de l'imprimante, signalées par-dessus.
        let gamut = job
            .proof
            .as_ref()
            .filter(|p| p.gamut)
            .map(|p| p.table.gamut_mask(&rgba));
        if let Some(p) = job.proof.as_ref().filter(|p| p.simulate) {
            p.table.apply_rgba(&mut rgba);
        }
        let histogram = histogram_rgba(&rgba);
        let scope = luma_image::scopes::render(job.scope, &rgba, w, h);
        let clean = (gamut.is_some() || job.clipping || job.dust_view).then(|| rgba.clone());
        if let Some(mask) = &gamut {
            luma_color::proof::paint_alert(&mut rgba, mask);
        }
        if job.clipping {
            clip_overlay(&mut rgba);
        }
        if job.dust_view {
            rgba = luma_image::dust::view(&base.image);
        }
        let dust = job
            .find_dust
            .then(|| luma_image::dust::find(&base.image, 60));
        let scene = (job.scene
            || job
                .settings
                .masks
                .iter()
                .any(|m| m.range.is_some() || m.edges))
        .then(|| {
            let (sw, sh) = RgbImage::fit(w, h, 256);
            Arc::new(base.image.resize_area(sw, sh))
        });
        let frame = Frame {
            generation: job.generation,
            size: [w, h],
            rgba,
            histogram,
            millis: start.elapsed().as_secs_f32() * 1000.0,
            gpu: on_gpu,
            scene,
            dust,
            scope,
            clean,
        };
        let _ = sender.send(Event::Rendered(Box::new(frame)));
        repaint();
    }
}

/// L'histogramme des trois couches d'une image RGBA.
pub fn histogram_rgba(rgba: &[u8]) -> [[u32; 256]; 3] {
    let mut h = [[0u32; 256]; 3];
    for p in rgba.as_chunks::<4>().0 {
        for c in 0..3 {
            h[c][usize::from(p[c])] += 1;
        }
    }
    h
}

/// L'indicateur d'écrêtage sur une image RGBA (voir [`to_rgba`]).
pub fn clip_overlay(rgba: &mut [u8]) {
    for p in rgba.as_chunks_mut::<4>().0 {
        if let Some(c) = clip_color([p[0], p[1], p[2]]) {
            p[..3].copy_from_slice(&c);
        }
    }
}

/// Les vignettes, en deux temps : pour chaque photo demandée, tout de suite l'aperçu que
/// le boîtier a enregistré dans le RAW (quelques millisecondes) ; puis, quand plus rien
/// n'attend, le vrai rendu de LÜMA avec ses réglages, qui le remplace.
fn thumbnail_loop(
    queue: Receiver<PathBuf>,
    sender: Sender<Event>,
    repaint: Arc<dyn Fn() + Send + Sync>,
) {
    use std::sync::mpsc::TryRecvError;
    let mut pending: std::collections::VecDeque<PathBuf> = std::collections::VecDeque::new();
    loop {
        let path = match queue.try_recv() {
            Ok(p) => p,
            Err(TryRecvError::Disconnected) => return,
            Err(TryRecvError::Empty) => {
                if let Some(p) = pending.pop_front() {
                    guarded(&p, || render_thumbnail(&p, &sender, &repaint));
                    continue;
                }
                match queue.recv() {
                    Ok(p) => p,
                    Err(_) => return,
                }
            }
        };
        guarded(&path, || {
            if saved_thumbnail(&path, &sender, &repaint) {
                return;
            }
            // Une photo retouchée attend son vrai rendu : l'aperçu intégré au RAW la
            // montrerait sans ses retouches.
            let edited =
                read_sidecar(&path).is_some_and(|s| luma_project::presets::summary(&s).is_edited());
            if !edited && quick_thumbnail(&path, &sender, &repaint) {
                pending.push_back(path.clone());
            } else {
                render_thumbnail(&path, &sender, &repaint);
            }
        });
    }
}

/// Une vignette à l'abri : un fichier qui fait échouer le décodeur n'arrête pas le fil
/// des vignettes (sinon plus aucune n'arriverait jusqu'à la fin de la séance).
fn guarded(path: &Path, f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        eprintln!("LÜMA : vignette de {} : échec du décodage", path.display());
    }
}

/// Le coup d'œil sur une photo qui s'ouvre : l'aperçu intégré au RAW (2048 px) si elle
/// n'est pas retouchée ; sinon sa vignette enregistrée, qui montre ses retouches.
fn glimpse(path: &Path) -> Option<([usize; 2], Vec<u8>)> {
    let edited = read_sidecar(path).is_some_and(|s| luma_project::presets::summary(&s).is_edited());
    let img = if edited {
        thumbnail_file(path).and_then(|f| luma_render::thumbnail::load(&f))
    } else {
        luma_raw::RawPhoto::embedded_preview(path, 2048)
    }?;
    Some((
        [img.width() as usize, img.height() as usize],
        to_rgba(img.as_raw(), false),
    ))
}

/// La vignette instantanée : l'aperçu intégré au RAW. `false` s'il n'y en a pas.
fn quick_thumbnail(
    path: &Path,
    sender: &Sender<Event>,
    repaint: &Arc<dyn Fn() + Send + Sync>,
) -> bool {
    let source = luma_project::copies::source(path);
    let Some(img) = luma_raw::RawPhoto::embedded_preview(&source, 320) else {
        return false;
    };
    let size = [img.width() as usize, img.height() as usize];
    let _ = sender.send(Event::Thumbnail {
        path: path.to_path_buf(),
        size,
        rgba: to_rgba(img.as_raw(), false),
    });
    repaint();
    true
}

/// La vignette déjà rendue avec les réglages actuels de la photo, enregistrée à une
/// visite précédente. `false` s'il n'y en a pas.
fn saved_thumbnail(
    path: &Path,
    sender: &Sender<Event>,
    repaint: &Arc<dyn Fn() + Send + Sync>,
) -> bool {
    let Some(img) = thumbnail_file(path).and_then(|f| luma_render::thumbnail::load(&f)) else {
        return false;
    };
    let _ = sender.send(Event::Thumbnail {
        path: path.to_path_buf(),
        size: [img.width() as usize, img.height() as usize],
        rgba: to_rgba(img.as_raw(), false),
    });
    repaint();
    true
}

/// Le fichier de la vignette d'une photo avec ses réglages actuels.
fn thumbnail_file(path: &Path) -> Option<PathBuf> {
    let dir = crate::dialogs::duplicates_dir()?;
    luma_render::thumbnail::path_for(path, read_sidecar(path).as_ref(), 320, &dir)
}

/// La vignette rendue par LÜMA, avec les réglages de la photo ; enregistrée pour les
/// visites suivantes.
/// Les étapes d'une vignette rendue, avec leur temps, quand `LUMA_TRACE_VIGNETTES` est
/// posée : de quoi savoir où une vignette qui n'arrive pas s'est arrêtée.
fn trace(path: &Path, step: &str, start: std::time::Instant) {
    if std::env::var_os("LUMA_TRACE_VIGNETTES").is_some() {
        eprintln!(
            "LÜMA : vignette de {} : {step} ({:.0} ms)",
            path.display(),
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

fn render_thumbnail(path: &Path, sender: &Sender<Event>, repaint: &Arc<dyn Fn() + Send + Sync>) {
    let start = std::time::Instant::now();
    trace(path, "rendu demandé", start);
    // Une vignette impossible (fichier illisible, rendu en échec) le dit, au lieu de
    // laisser la case vide sans raison.
    let photo = match crate::dialogs::open_photo(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "LÜMA : vignette de {} : ouverture impossible ({e})",
                path.display()
            );
            return;
        }
    };
    trace(path, "photo ouverte", start);
    let settings = read_sidecar(path).unwrap_or_else(|| photo.default_settings());
    let r = match photo.render(&settings, Size::Fit(320), &Cancel::new()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "LÜMA : vignette de {} : rendu impossible ({e})",
                path.display()
            );
            return;
        }
    };
    trace(path, "rendue", start);
    let rgb = luma_image::encode8(&r.image, OutputSpace::Srgb);
    if let Some(file) = thumbnail_file(path) {
        let _ =
            luma_render::thumbnail::store(&file, &rgb, r.image.width as u32, r.image.height as u32);
    }
    let _ = sender.send(Event::Thumbnail {
        path: path.to_path_buf(),
        size: [r.image.width, r.image.height],
        rgba: to_rgba(&rgb, false),
    });
    trace(path, "envoyée", start);
    repaint();
}

/// Les fils du travail de fond (l'écriture des doublons) : la moitié des cœurs, pour
/// que l'aperçu et les vignettes, calculés sur les fils communs, n'attendent pas
/// derrière. Sur un ordinateur à trois cœurs, une photo ouverte faisait sinon attendre
/// les vignettes plus de trente secondes.
fn background() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let n = std::thread::available_parallelism().map_or(2, |n| n.get());
        rayon::ThreadPoolBuilder::new()
            .num_threads((n / 2).max(1))
            .thread_name(|i| format!("luma-fond-{i}"))
            .build()
            .expect("fils du travail de fond")
    })
}

/// L'écriture d'un doublon dématrice toute l'image d'un coup : sur iPhone et iPad, où
/// le système ferme une app trop gourmande, seulement jusqu'à 25 millions de pixels.
fn duplicate_fits(photo: &Photo) -> bool {
    let (w, h) = photo.full_size();
    !cfg!(target_os = "ios") || w * h <= 25_000_000
}

/// Finit sur le GPU : le développement, puis les films des masques, chacun rendu sur le
/// GPU (depuis sa propre base s'il touche aux hautes lumières, aux ombres ou à la
/// clarté) et fondu selon la zone du masque. `None` si le GPU ne répond plus.
#[allow(clippy::too_many_arguments)]
fn finish_on_gpu(
    g: &luma_gpu::Gpu,
    photo: &Photo,
    settings: &Settings,
    size: Size,
    base: &luma_render::Base,
    layers: &[(&luma_project::Mask, Settings, f32)],
    cancel: &Cancel,
    timings: &mut luma_core::Timings,
) -> Option<Vec<u8>> {
    let (w, h) = (base.image.width, base.image.height);
    let mut rgba = g.finish(
        &base.image,
        base.id,
        base.scale,
        settings,
        OutputSpace::Srgb,
    )?;
    for (mask, look, k) in layers {
        let own = if luma_image::needs_own_base(settings, look) {
            photo.base(look, size, cancel, timings).ok()
        } else {
            None
        };
        let b = own
            .as_ref()
            .filter(|b| (b.image.width, b.image.height) == (w, h))
            .unwrap_or(base);
        let alt = g.finish(&b.image, b.id, b.scale, look, OutputSpace::Srgb)?;
        let weights = luma_image::local::weights_on(mask, &base.image);
        // Fondu en lumière linéaire, comme `finish_layers` (l'export) : les octets sRGB
        // décodés, mélangés, réencodés.
        let (decode, encode) = srgb_luts();
        for ((o, a), wt) in rgba
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(alt.as_chunks::<4>().0)
            .zip(&weights.data)
        {
            let t = (wt * k).clamp(0.0, 1.0);
            if t > 0.0 {
                for c in 0..3 {
                    let (lo, la) = (decode[o[c] as usize], decode[a[c] as usize]);
                    o[c] = (encode.get(lo + (la - lo) * t) * 255.0).round() as u8;
                }
            }
        }
    }
    Some(rgba)
}

/// Les tables du sRGB : décodage des 256 octets, encodage interpolé.
fn srgb_luts() -> &'static ([f32; 256], luma_color::EncodeLut) {
    static LUTS: std::sync::OnceLock<([f32; 256], luma_color::EncodeLut)> =
        std::sync::OnceLock::new();
    LUTS.get_or_init(|| {
        (
            std::array::from_fn(|i| luma_color::srgb_decode(i as f32 / 255.0)),
            luma_color::EncodeLut::new(luma_color::srgb_encode),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_render_that_panics_does_not_stop_the_next_ones() {
        // La première demande panique ; le fil continue et sert la suivante.
        let (send, jobs) = channel::<u32>();
        let (done, served) = channel::<u32>();
        let (ack, started) = channel::<()>();
        let thread = std::thread::spawn(move || {
            serve(jobs, |job| {
                let _ = ack.send(());
                if job == 1 {
                    panic!("rendu impossible");
                }
                let _ = done.send(job);
            })
        });
        send.send(1).unwrap();
        started.recv().unwrap();
        send.send(2).unwrap();
        started.recv().unwrap();
        drop(send);
        thread.join().expect("le fil de rendu survit");
        assert_eq!(served.try_iter().collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn a_panic_becomes_an_error_the_app_can_show() {
        assert_eq!(caught(|| Ok::<_, String>(3)), Ok(3));
        assert_eq!(
            caught(|| Err::<u8, _>("illisible".to_owned())),
            Err("illisible".to_owned())
        );
        let failed = caught(|| -> Result<u8, String> { panic!("indice hors bornes") });
        assert_eq!(
            failed,
            Err("Erreur inattendue pendant le calcul".to_owned())
        );
    }

    #[test]
    fn clipping_overlay() {
        let rgb = [255, 10, 10, 0, 0, 0, 100, 100, 100];
        assert_eq!(to_rgba(&rgb, false)[..4], [255, 10, 10, 255]);
        let c = to_rgba(&rgb, true);
        assert_eq!(c[..4], [255, 0, 0, 255]);
        assert_eq!(c[4..8], [40, 90, 255, 255]);
        assert_eq!(c[8..12], [100, 100, 100, 255]);
        // Couche par couche : rouge et vert brûlés en jaune, les trois en blanc.
        assert_eq!(clip_color([255, 255, 30]), Some([255, 255, 0]));
        assert_eq!(clip_color([40, 255, 255]), Some([0, 255, 255]));
        assert_eq!(clip_color([255; 3]), Some([255; 3]));
        assert_eq!(clip_color([1, 0, 0]), None);
    }

    #[test]
    fn export_paths() {
        let mut o = ExportOptions::default();
        let src = Path::new("/x/IMG_1.CR3");
        let f = NameFields::of_file(src);
        assert_eq!(export_path(src, &o, &f), PathBuf::from("/x/LÜMA/IMG_1.jpg"));
        o.format = Format::Tiff;
        o.folder = Some("/sortie".into());
        assert_eq!(export_path(src, &o, &f), PathBuf::from("/sortie/IMG_1.tif"));
    }

    #[test]
    fn overlay_on_rgba_matches_rgb() {
        let rgb = [255, 10, 10, 0, 0, 0, 100, 100, 100];
        let mut rgba = to_rgba(&rgb, false);
        clip_overlay(&mut rgba);
        assert_eq!(rgba, to_rgba(&rgb, true));
        assert_eq!(histogram_rgba(&to_rgba(&rgb, false))[0][255], 1);
    }

    #[test]
    fn sidecar_names() {
        assert_eq!(
            sidecar_path(Path::new("/x/IMG_1.CR3")),
            PathBuf::from("/x/IMG_1.xmp")
        );
    }
}
