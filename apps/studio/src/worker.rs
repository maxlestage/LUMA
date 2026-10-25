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
use luma_core::Cancel;
use luma_project::{Settings, xmp};
use luma_render::{Photo, Size};

/// Une image prête à afficher (sRGB, 8 bits, RGBA).
pub struct Frame {
    pub generation: u64,
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
    pub histogram: [[u32; 256]; 3],
    pub millis: f32,
}

pub enum Event {
    Loaded {
        path: PathBuf,
        result: Result<(Arc<Photo>, Option<Box<Settings>>), String>,
    },
    Rendered(Box<Frame>),
    Thumbnail {
        path: PathBuf,
        size: [usize; 2],
        rgba: Vec<u8>,
    },
    Exported {
        result: Result<PathBuf, String>,
    },
}

struct RenderJob {
    photo: Arc<Photo>,
    settings: Settings,
    size: Size,
    generation: u64,
    clipping: bool,
}

/// Le sidecar XMP d'un RAW (`IMG_0001.xmp` ou `IMG_0001.CR3.xmp`, comme Lightroom).
pub fn sidecar_path(raw: &Path) -> PathBuf {
    let long = PathBuf::from(format!("{}.xmp", raw.display()));
    if long.is_file() {
        long
    } else {
        raw.with_extension("xmp")
    }
}

/// Les réglages enregistrés à côté d'un RAW, s'il y en a.
pub fn read_sidecar(raw: &Path) -> Option<Settings> {
    let text = std::fs::read_to_string(sidecar_path(raw)).ok()?;
    xmp::read(&text).ok()
}

/// RGB 8 bits → RGBA, avec en option l'indicateur d'écrêtage de Lightroom (touche J) :
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
            let result = Photo::open(&path)
                .map(|p| (Arc::new(p), read_sidecar(&path).map(Box::new)))
                .map_err(|e| e.to_string());
            let _ = sender.send(Event::Loaded { path, result });
            repaint();
        });
    }

    /// Demande un rendu ; celui en cours, devenu inutile, est annulé.
    pub fn render(
        &self,
        photo: Arc<Photo>,
        settings: Settings,
        size: Size,
        generation: u64,
        clipping: bool,
    ) {
        self.running.lock().expect("annulation").cancel();
        let _ = self.render.send(RenderJob {
            photo,
            settings,
            size,
            generation,
            clipping,
        });
    }

    pub fn thumbnail(&self, path: PathBuf) {
        let _ = self.thumbs.send(path);
    }

    /// Export pleine définition, JPEG sRGB, dans `<dossier>/LÜMA/`.
    pub fn export(&self, photo: Arc<Photo>, settings: Settings, source: PathBuf) {
        let (sender, repaint) = (self.sender.clone(), self.repaint.clone());
        std::thread::spawn(move || {
            let result = (|| {
                let dir = source.parent().unwrap_or(Path::new(".")).join("LÜMA");
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let out = dir.join(source.with_extension("jpg").file_name().unwrap_or_default());
                let rendered = photo
                    .render(&settings, Size::Full, &Cancel::new())
                    .map_err(|e| e.to_string())?;
                luma_render::export::write(&out, &rendered.image, OutputSpace::Srgb, 92, 8)?;
                Ok(out)
            })();
            let _ = sender.send(Event::Exported { result });
            repaint();
        });
    }
}

fn render_loop(
    jobs: Receiver<RenderJob>,
    sender: Sender<Event>,
    running: Arc<Mutex<Cancel>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
) {
    while let Ok(mut job) = jobs.recv() {
        // Seule la dernière demande compte.
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        let cancel = Cancel::new();
        *running.lock().expect("annulation") = cancel.clone();
        let start = Instant::now();
        let Ok(rendered) = job.photo.render(&job.settings, job.size, &cancel) else {
            continue;
        };
        let rgb = luma_image::encode8(&rendered.image, OutputSpace::Srgb);
        let histogram = luma_image::histogram(&rgb);
        let frame = Frame {
            generation: job.generation,
            size: [rendered.image.width, rendered.image.height],
            rgba: to_rgba(&rgb, job.clipping),
            histogram,
            millis: start.elapsed().as_secs_f32() * 1000.0,
        };
        let _ = sender.send(Event::Rendered(Box::new(frame)));
        repaint();
    }
}

fn thumbnail_loop(
    queue: Receiver<PathBuf>,
    sender: Sender<Event>,
    repaint: Arc<dyn Fn() + Send + Sync>,
) {
    while let Ok(path) = queue.recv() {
        let Ok(photo) = Photo::open(&path) else {
            continue;
        };
        let settings = read_sidecar(&path).unwrap_or_default();
        let Ok(r) = photo.render(&settings, Size::Fit(320), &Cancel::new()) else {
            continue;
        };
        let rgb = luma_image::encode8(&r.image, OutputSpace::Srgb);
        let _ = sender.send(Event::Thumbnail {
            path,
            size: [r.image.width, r.image.height],
            rgba: to_rgba(&rgb, false),
        });
        repaint();
    }
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
    fn sidecar_names() {
        assert_eq!(
            sidecar_path(Path::new("/x/IMG_1.CR3")),
            PathBuf::from("/x/IMG_1.xmp")
        );
    }
}
