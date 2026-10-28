//! Les tuiles du fond de carte : lues dans le cache sur le disque, sinon téléchargées
//! (sur Mac, iPhone et iPad, par le système) et gardées pour la fois suivante. Deux
//! fils de fond s'en chargent ; l'interface demande les tuiles de l'écran à chaque
//! image et dessine celles qui sont prêtes, sans jamais attendre.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use luma_library::map::Tile;

/// Les tuiles gardées en mémoire (une tuile : 256 Kio sur le GPU).
const KEPT: usize = 192;

/// Les demandes en route au plus : une carte qu'on fait glisser vite ne laisse pas
/// derrière elle une file de tuiles qu'on ne regarde plus.
const IN_FLIGHT: usize = 24;

/// Une tuile lue : sa taille et ses pixels RGBA, ou rien (absente, illisible).
type Read = (Tile, Option<([usize; 2], Vec<u8>)>);

/// Une demande : la tuile, le téléchargement permis, le cache.
type Job = (Tile, bool, Option<PathBuf>);

pub struct Tiles {
    textures: HashMap<Tile, (TextureHandle, u64)>,
    /// Demandées, pas encore reçues.
    asked: HashSet<Tile>,
    /// Sans réponse : pas redemandées avant le prochain changement de réglage.
    missing: HashSet<Tile>,
    jobs: Option<Sender<Job>>,
    done: Option<Receiver<Read>>,
    frame: u64,
    /// Le téléchargement est permis (sinon, le cache seul).
    pub online: bool,
    /// Le cache sur le disque.
    dir: Option<PathBuf>,
}

impl Default for Tiles {
    fn default() -> Self {
        Tiles {
            textures: HashMap::new(),
            asked: HashSet::new(),
            missing: HashSet::new(),
            jobs: None,
            done: None,
            frame: 0,
            online: true,
            dir: cache_dir(),
        }
    }
}

/// Le cache des tuiles : sur Mac `~/Library/Caches/LÜMA/Tuiles`, sur iPhone et iPad
/// `Library/Caches/Tuiles` (le système peut le vider), ailleurs `~/.cache/luma/tuiles`.
/// `LUMA_TILES` le remplace.
pub fn cache_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("LUMA_TILES") {
        return Some(PathBuf::from(d));
    }
    if cfg!(windows) {
        return Some(luma_core::windows_dir(true)?.join("Tuiles"));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    Some(if cfg!(target_os = "ios") {
        home.join("Library/Caches/Tuiles")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Caches/LÜMA/Tuiles")
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map_or_else(|| home.join(".cache"), PathBuf::from)
            .join("luma/tuiles")
    })
}

/// Une tuile, du cache ou du réseau (si permis) ; gardée dans le cache si elle se lit.
fn load(tile: Tile, online: bool, dir: Option<&std::path::Path>) -> Option<([usize; 2], Vec<u8>)> {
    let file = dir.map(|d| d.join(tile.file()));
    let decode = |bytes: &[u8]| {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .ok()?
            .into_rgba8();
        Some((
            [img.width() as usize, img.height() as usize],
            img.into_raw(),
        ))
    };
    if let Some(bytes) = file.as_ref().and_then(|f| std::fs::read(f).ok())
        && let Some(img) = decode(&bytes)
    {
        return Some(img);
    }
    if !online {
        return None;
    }
    let bytes = luma_apple::net::fetch(&tile.url())?;
    let img = decode(&bytes)?;
    if let Some(f) = file {
        // Écrite à côté puis renommée : jamais une tuile à moitié écrite.
        if let Some(dir) = f.parent()
            && std::fs::create_dir_all(dir).is_ok()
        {
            let tmp = f.with_extension("tmp");
            if std::fs::write(&tmp, &bytes).is_ok() {
                let _ = std::fs::rename(&tmp, &f);
            }
        }
    }
    Some(img)
}

impl Tiles {
    /// Les fils de fond, au premier besoin.
    fn start(&mut self, ctx: &Context) -> Option<&Sender<Job>> {
        if self.jobs.is_none() {
            let (jobs, queue) = channel::<Job>();
            let (send, done) = channel::<Read>();
            let queue = Arc::new(Mutex::new(queue));
            for _ in 0..2 {
                let (queue, send, ctx) = (queue.clone(), send.clone(), ctx.clone());
                let spawned = std::thread::Builder::new()
                    .name("tuiles".into())
                    .spawn(move || {
                        loop {
                            let job = queue.lock().ok().and_then(|q| q.recv().ok());
                            let Some((tile, online, dir)) = job else {
                                break;
                            };
                            if send
                                .send((tile, load(tile, online, dir.as_deref())))
                                .is_err()
                            {
                                break;
                            }
                            ctx.request_repaint();
                        }
                    });
                if spawned.is_err() {
                    return None;
                }
            }
            self.jobs = Some(jobs);
            self.done = Some(done);
        }
        self.jobs.as_ref()
    }

    /// Garde les tuiles dans ce dossier (les essais).
    pub fn use_dir(&mut self, dir: PathBuf) {
        self.dir = Some(dir);
        self.missing.clear();
    }

    /// Change le réglage du téléchargement : les tuiles absentes seront redemandées.
    pub fn set_online(&mut self, online: bool) {
        self.online = online;
        self.missing.clear();
    }

    /// Reçoit les tuiles prêtes ; rend `true` si l'une est arrivée.
    pub fn receive(&mut self, ctx: &Context) -> bool {
        let Some(done) = &self.done else {
            return false;
        };
        let mut any = false;
        while let Ok((tile, img)) = done.try_recv() {
            self.asked.remove(&tile);
            match img {
                Some((size, rgba)) => {
                    let image = ColorImage::from_rgba_unmultiplied(size, &rgba);
                    let name = format!("tuile {}/{}/{}", tile.z, tile.x, tile.y);
                    let texture = ctx.load_texture(name, image, TextureOptions::LINEAR);
                    self.textures.insert(tile, (texture, self.frame));
                    any = true;
                }
                None => {
                    self.missing.insert(tile);
                }
            }
        }
        // Les moins récemment vues partent d'abord.
        if self.textures.len() > KEPT {
            let mut ages: Vec<(u64, Tile)> =
                self.textures.iter().map(|(t, (_, f))| (*f, *t)).collect();
            ages.sort();
            for (_, t) in &ages[..self.textures.len() - KEPT] {
                self.textures.remove(t);
            }
        }
        any
    }

    /// La tuile si elle est prête ; sinon demandée (une fois). Les demandes partent
    /// dans l'ordre des appels : d'abord celles du centre de l'écran.
    pub fn get(&mut self, ctx: &Context, tile: Tile) -> Option<&TextureHandle> {
        self.frame += 1;
        let frame = self.frame;
        if self.textures.contains_key(&tile) {
            let entry = self.textures.get_mut(&tile)?;
            entry.1 = frame;
            return Some(&entry.0);
        }
        if self.asked.len() < IN_FLIGHT
            && !self.asked.contains(&tile)
            && !self.missing.contains(&tile)
        {
            let job = (tile, self.online, self.dir.clone());
            if let Some(jobs) = self.start(ctx)
                && jobs.send(job).is_ok()
            {
                self.asked.insert(tile);
            }
        }
        None
    }

    /// Les tuiles prêtes à dessiner.
    pub fn ready(&self) -> usize {
        self.textures.len()
    }

    /// Des tuiles sont‑elles en route ?
    pub fn is_loading(&self) -> bool {
        !self.asked.is_empty()
    }
}
