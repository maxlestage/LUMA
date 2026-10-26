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
    /// L'aperçu d'un préréglage sur la photo affichée.
    Preview {
        tag: u64,
        index: usize,
        size: [usize; 2],
        rgba: Vec<u8>,
    },
    /// Une pellicule est mesurée : le nom du film, et l'écart (OKLab × 100) entre le
    /// numérique et la pellicule, avant et après.
    Measured {
        result: Result<(String, f32, f32), String>,
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
    Panorama(luma_merge::Projection),
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
    let text = std::fs::read_to_string(sidecar_path(raw)).ok()?;
    xmp::read(&text).ok()
}

/// La note, le drapeau et le libellé enregistrés à côté d'un RAW.
pub fn read_meta(raw: &Path) -> xmp::Meta {
    std::fs::read_to_string(sidecar_path(raw))
        .map(|t| xmp::read_meta(&t))
        .unwrap_or_default()
}

/// Écrit le sidecar d'un RAW : réglages, note et drapeau.
pub fn write_sidecar(raw: &Path, settings: &Settings, meta: &xmp::Meta) -> Result<(), String> {
    let target = sidecar_path(raw);
    std::fs::write(&target, xmp::write_with_meta(settings, meta))
        .map_err(|e| format!("{} : {e}", target.display()))
}

/// Change la note, le drapeau ou les mots‑clés d'une photo qui n'est pas ouverte :
/// ses réglages enregistrés sont gardés tels quels ; sans réglages, seul un XMP de
/// métadonnées est écrit, et la photo garde son rendu par défaut.
pub fn write_meta(raw: &Path, meta: &xmp::Meta) -> Result<(), String> {
    match read_sidecar(raw) {
        Some(settings) => write_sidecar(raw, &settings, meta),
        None => {
            let target = sidecar_path(raw);
            std::fs::write(&target, xmp::write_meta(meta))
                .map_err(|e| format!("{} : {e}", target.display()))
        }
    }
}

/// RGB 8 bits → RGBA, avec en option l'indicateur d'écrêtage (touche J) :
/// rouge où un canal est blanc pur, bleu où tout est noir.
pub fn to_rgba(rgb: &[u8], clipping: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgb.len() / 3 * 4);
    for p in rgb.as_chunks::<3>().0 {
        let px = if clipping && p.contains(&255) {
            [255, 40, 40]
        } else if clipping && *p == [0, 0, 0] {
            [40, 90, 255]
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
            let result = crate::dialogs::open_photo(&path)
                .map(|p| (Arc::new(p), read_sidecar(&path).map(Box::new)))
                .map_err(|e| e.to_string());
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
                && photo.write_duplicate(&source, &dir).is_ok()
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
        });
    }

    pub fn thumbnail(&self, path: PathBuf) {
        let _ = self.thumbs.send(path);
    }

    /// Fusionne des RAW en un DNG, à côté du premier.
    /// Mesure une pellicule : `photo` en numérique (son rendu neutre) et `scan`, la même
    /// scène sur film. La LUT est enregistrée dans `dir` et chargée.
    pub fn measure_film(&self, photo: Arc<Photo>, scan: PathBuf, dir: PathBuf) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::Builder::new()
            .name("luma-mesure".into())
            .spawn(move || {
                let result = (|| -> Result<(String, f32, f32), String> {
                    use luma_merge::film;
                    let pair = film::Pair {
                        digital: film::reference(&photo).map_err(|e| e.to_string())?,
                        film: film::load_scan(&scan).map_err(|e| e.to_string())?,
                        aligned: false,
                    };
                    let stem = scan.file_stem().map_or_else(
                        || "Pellicule".to_owned(),
                        |s| s.to_string_lossy().into_owned(),
                    );
                    // Un nom libre : « Portra », « Portra 2 »…
                    let mut name = stem.clone();
                    let mut k = 2;
                    while dir.join(format!("{name}.cube")).exists()
                        || luma_image::cube::get(&name).is_some()
                    {
                        name = format!("{stem} {k}");
                        k += 1;
                    }
                    let m = film::measure(&[pair], 33, &name).map_err(|e| e.to_string())?;
                    let path = dir.join(format!("{name}.cube"));
                    std::fs::write(&path, m.cube.to_text())
                        .map_err(|e| format!("{} : {e}", path.display()))?;
                    luma_image::cube::register(&name, m.cube);
                    Ok((name, m.before, m.after))
                })();
                let _ = sender.send(Event::Measured { result });
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
                let result = (|| -> Result<PathBuf, String> {
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
                            },
                        ),
                        MergeKind::Panorama(p) => (
                            luma_merge::panorama(&frames, p, &progress, &cancel)
                                .map_err(|e| e.to_string())?,
                            "Pano",
                        ),
                    };
                    let reference = &job.paths[job.paths.len() / 2];
                    let template =
                        luma_raw::RawPhoto::open(reference).map_err(|e| e.to_string())?;
                    let out = luma_merge::output_path(&job.paths[0], suffix);
                    merged
                        .save(&out, &template, job.auto_crop)
                        .map_err(|e| e.to_string())?;
                    Ok(out)
                })();
                let _ = sender.send(Event::Merged { result });
                repaint();
            })
            .expect("fil de fusion");
    }

    /// Les aperçus des préréglages (petits rendus), dans l'ordre.
    pub fn previews(&self, photo: Arc<Photo>, settings: Vec<Settings>, tag: u64) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::spawn(move || {
            for (index, s) in settings.iter().enumerate() {
                let Ok(r) = photo.render(s, Size::Fit(240), &Cancel::new()) else {
                    continue;
                };
                let rgb = luma_image::encode8(&r.image, OutputSpace::Srgb);
                let _ = sender.send(Event::Preview {
                    tag,
                    index,
                    size: [r.image.width, r.image.height],
                    rgba: to_rgba(&rgb, false),
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
            let mut done = 0;
            for (k, job) in jobs.into_iter().enumerate() {
                let photo = match job.photo {
                    Some(p) => Ok(p),
                    None => crate::dialogs::open_photo(&job.source)
                        .map(Arc::new)
                        .map_err(|e| e.to_string()),
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
                            export_one(&photo, &job.settings, &out, options, &meta).map(|()| out)
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
    let gpu = if std::env::var("LUMA_GPU").is_ok_and(|v| v == "0") {
        None
    } else {
        luma_gpu::Gpu::new()
    };
    while let Ok(mut job) = jobs.recv() {
        // Seule la dernière demande compte.
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        let cancel = Cancel::new();
        *running.lock().expect("annulation") = cancel.clone();
        let start = Instant::now();
        let mut timings = luma_core::Timings::default();
        let Ok(base) = job
            .photo
            .base(&job.settings, job.size, &cancel, &mut timings)
        else {
            continue;
        };
        let (w, h) = (base.image.width, base.image.height);
        let layers = luma_image::look_layers(&job.settings);
        let (mut rgba, on_gpu) = match &gpu {
            Some(g)
                if g.can_finish(&job.settings, w, h, base.scale)
                    && layers
                        .iter()
                        .all(|(_, s, _)| g.can_finish(s, w, h, base.scale)) =>
            {
                let mut rgba = g.finish(
                    &base.image,
                    base.id,
                    base.scale,
                    &job.settings,
                    OutputSpace::Srgb,
                );
                // Les films des masques : chacun rendu sur le GPU (depuis sa propre base
                // s'il touche aux hautes lumières, aux ombres ou à la clarté), puis fondu
                // selon la zone du masque.
                for (mask, look, k) in &layers {
                    let own = if luma_image::needs_own_base(&job.settings, look) {
                        job.photo.base(look, job.size, &cancel, &mut timings).ok()
                    } else {
                        None
                    };
                    let b = own
                        .as_ref()
                        .filter(|b| (b.image.width, b.image.height) == (w, h))
                        .unwrap_or(&base);
                    let alt = g.finish(&b.image, b.id, b.scale, look, OutputSpace::Srgb);
                    let weights = luma_image::local::weights_on(mask, &base.image);
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
                                o[c] = (f32::from(o[c]) + (f32::from(a[c]) - f32::from(o[c])) * t)
                                    .round() as u8;
                            }
                        }
                    }
                }
                (rgba, true)
            }
            _ => {
                let photo = &job.photo;
                let Ok(image) = luma_image::finish_layers(
                    &base.image,
                    base.scale,
                    &job.settings,
                    &mut |look, t| photo.base(look, job.size, &cancel, t).map(|b| b.image),
                    &cancel,
                    &mut timings,
                ) else {
                    continue;
                };
                (
                    to_rgba(&luma_image::encode8(&image, OutputSpace::Srgb), false),
                    false,
                )
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
        if p[..3].contains(&255) {
            p[..3].copy_from_slice(&[255, 40, 40]);
        } else if p[..3] == [0, 0, 0] {
            p[..3].copy_from_slice(&[40, 90, 255]);
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
                    render_thumbnail(&p, &sender, &repaint);
                    continue;
                }
                match queue.recv() {
                    Ok(p) => p,
                    Err(_) => return,
                }
            }
        };
        if saved_thumbnail(&path, &sender, &repaint) {
            continue;
        }
        // Une photo retouchée attend son vrai rendu : l'aperçu intégré au RAW la
        // montrerait sans ses retouches.
        let edited =
            read_sidecar(&path).is_some_and(|s| luma_project::presets::summary(&s).is_edited());
        if !edited && quick_thumbnail(&path, &sender, &repaint) {
            pending.push_back(path);
        } else {
            render_thumbnail(&path, &sender, &repaint);
        }
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
fn render_thumbnail(path: &Path, sender: &Sender<Event>, repaint: &Arc<dyn Fn() + Send + Sync>) {
    let Ok(photo) = crate::dialogs::open_photo(path) else {
        return;
    };
    let settings = read_sidecar(path).unwrap_or_else(|| photo.default_settings());
    let Ok(r) = photo.render(&settings, Size::Fit(320), &Cancel::new()) else {
        return;
    };
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
    repaint();
}

/// L'écriture d'un doublon dématrice toute l'image d'un coup : sur iPhone et iPad, où
/// le système ferme une app trop gourmande, seulement jusqu'à 25 millions de pixels.
fn duplicate_fits(photo: &Photo) -> bool {
    let (w, h) = photo.full_size();
    !cfg!(target_os = "ios") || w * h <= 25_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipping_overlay() {
        let rgb = [255, 10, 10, 0, 0, 0, 100, 100, 100];
        assert_eq!(to_rgba(&rgb, false)[..4], [255, 10, 10, 255]);
        let c = to_rgba(&rgb, true);
        assert_eq!(c[..4], [255, 40, 40, 255]);
        assert_eq!(c[4..8], [40, 90, 255, 255]);
        assert_eq!(c[8..12], [100, 100, 100, 255]);
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
