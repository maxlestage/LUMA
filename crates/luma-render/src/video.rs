//! La vidéo de base : un film ou un préréglage appliqué à une vidéo courte, image par
//! image, puis réenregistrée. Le format est l'AVI en Motion JPEG (chaque image est un
//! JPEG), celui de nombreux appareils photo et de bien des logiciels de montage : il se
//! lit et s'écrit ici en Rust pur, sans rien d'autre que le décodeur JPEG déjà utilisé
//! pour les photos et l'encodeur de LÜMA. Le son (et tout autre flux) est recopié tel
//! quel, à sa place : la vidéo garde sa durée, son rythme et sa bande‑son.
//!
//! Le fichier écrit est un AVI 1.0 avec son index (`idx1`) : il se limite à 1 Go, ce
//! qui suffit à une vidéo courte. À la lecture, les fichiers étendus (OpenDML, au‑delà
//! d'1 Go) sont acceptés : leurs suites `AVIX` sont parcourues comme le début.

use luma_core::{Cancel, RgbImage};

/// Une vidéo AVI lue : ses en‑têtes et ses morceaux, dans l'ordre du fichier.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub width: usize,
    pub height: usize,
    /// Le rythme : `rate / scale` images par seconde.
    pub rate: u32,
    pub scale: u32,
    /// Le numéro du flux vidéo (les morceaux `00dc`, `01dc`…).
    pub video: usize,
    /// Les listes `strl` de chaque flux, telles que lues (celle de la vidéo est récrite).
    streams: Vec<Vec<u8>>,
    pub chunks: Vec<Chunk>,
}

/// Un morceau de données : une image (un JPEG, vide si l'image répète la précédente)
/// ou un morceau d'un autre flux (le son), gardé tel quel.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub id: [u8; 4],
    pub data: Vec<u8>,
}

impl Clip {
    /// Une vidéo sans son, de ces images JPEG, à `rate / scale` images par seconde.
    pub fn from_frames(width: usize, height: usize, rate: u32, scale: u32, frames: Vec<Vec<u8>>) -> Clip {
        Clip {
            width,
            height,
            rate,
            scale: scale.max(1),
            video: 0,
            streams: vec![b"strl".to_vec()],
            chunks: frames.into_iter().map(|data| Chunk { id: *b"00dc", data }).collect(),
        }
    }

    /// Les images par seconde.
    pub fn fps(&self) -> f64 {
        f64::from(self.rate) / f64::from(self.scale.max(1))
    }

    /// Les images de la vidéo, dans l'ordre (les répétitions comprises).
    pub fn frames(&self) -> impl Iterator<Item = &Chunk> {
        self.chunks.iter().filter(|c| self.is_frame(c))
    }

    pub fn frame_count(&self) -> usize {
        self.frames().count()
    }

    /// La durée, en secondes.
    pub fn seconds(&self) -> f64 {
        self.frame_count() as f64 / self.fps().max(1e-6)
    }

    fn is_frame(&self, c: &Chunk) -> bool {
        stream_of(&c.id) == Some(self.video) && matches!(&c.id[2..], b"dc" | b"db")
    }
}

/// Le numéro de flux d'un morceau (`01wb` → 1).
fn stream_of(id: &[u8; 4]) -> Option<usize> {
    let d = |b: u8| b.is_ascii_digit().then(|| usize::from(b - b'0'));
    Some(d(id[0])? * 10 + d(id[1])?)
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// Les morceaux RIFF d'un bloc : (identifiant, contenu), sans s'arrêter sur un dernier
/// morceau tronqué (il est coupé à ce qui reste).
fn riff_chunks(b: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut i = 0;
    std::iter::from_fn(move || {
        if i + 8 > b.len() {
            return None;
        }
        let id: [u8; 4] = b[i..i + 4].try_into().ok()?;
        let n = u32_at(b, i + 4)? as usize;
        let start = i + 8;
        let end = start.saturating_add(n).min(b.len());
        i = start.saturating_add(n).saturating_add(n & 1);
        Some((id, &b[start..end]))
    })
}

/// Lit une vidéo AVI en Motion JPEG.
pub fn read(bytes: &[u8]) -> Result<Clip, String> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"AVI " {
        return Err("ce n'est pas une vidéo AVI".into());
    }
    let mut clip = Clip {
        width: 0,
        height: 0,
        rate: 0,
        scale: 1,
        video: usize::MAX,
        streams: Vec::new(),
        chunks: Vec::new(),
    };
    let mut motion_jpeg = false;
    // Le fichier, puis ses suites (`RIFF AVIX`) au‑delà d'1 Go.
    for (id, body) in riff_chunks(bytes) {
        if &id != b"RIFF" || body.len() < 4 {
            continue;
        }
        for (id, data) in riff_chunks(&body[4..]) {
            if &id != b"LIST" || data.len() < 4 {
                continue;
            }
            match &data[..4] {
                b"hdrl" => {
                    for (id, d) in riff_chunks(&data[4..]) {
                        if &id == b"LIST" && d.starts_with(b"strl") {
                            let k = clip.streams.len();
                            clip.streams.push(d.to_vec());
                            let mut strh = None;
                            let mut strf = None;
                            for (id, s) in riff_chunks(&d[4..]) {
                                match &id {
                                    b"strh" => strh = Some(s),
                                    b"strf" => strf = Some(s),
                                    _ => {}
                                }
                            }
                            if let Some(h) = strh
                                && h.starts_with(b"vids")
                                && clip.video == usize::MAX
                            {
                                clip.video = k;
                                clip.scale = u32_at(h, 20).unwrap_or(1).max(1);
                                clip.rate = u32_at(h, 24).unwrap_or(0);
                                let handler = h.get(4..8).unwrap_or_default();
                                let compression = strf.and_then(|f| f.get(16..20)).unwrap_or_default();
                                motion_jpeg = [handler, compression]
                                    .iter()
                                    .any(|c| c.eq_ignore_ascii_case(b"MJPG") || c.eq_ignore_ascii_case(b"JPEG"));
                                if let Some(f) = strf {
                                    clip.width = u32_at(f, 4).unwrap_or(0) as usize;
                                    // Une hauteur négative : image de haut en bas, même taille.
                                    clip.height = (u32_at(f, 8).unwrap_or(0) as i32).unsigned_abs() as usize;
                                }
                            }
                        }
                    }
                }
                b"movi" => collect(&data[4..], &mut clip.chunks),
                _ => {}
            }
        }
    }
    if clip.video == usize::MAX {
        return Err("aucune image dans cette vidéo".into());
    }
    // Un en‑tête muet sur le codec : la première image dit si c'est du JPEG.
    if !motion_jpeg
        && !clip
            .frames()
            .find(|c| !c.data.is_empty())
            .is_some_and(|c| c.data.starts_with(&[0xFF, 0xD8]))
    {
        return Err("seules les vidéos en Motion JPEG sont prises en charge".into());
    }
    if clip.rate == 0 {
        return Err("vidéo sans rythme d'images".into());
    }
    if clip.frame_count() == 0 {
        return Err("aucune image dans cette vidéo".into());
    }
    Ok(clip)
}

/// Les morceaux d'une liste `movi` (et de ses sous‑listes `rec `), dans l'ordre.
fn collect(b: &[u8], out: &mut Vec<Chunk>) {
    for (id, data) in riff_chunks(b) {
        if &id == b"LIST" {
            if data.len() >= 4 {
                collect(&data[4..], out);
            }
        } else if stream_of(&id).is_some() {
            out.push(Chunk {
                id,
                data: data.to_vec(),
            });
        }
    }
}

fn put_chunk(out: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
}

fn put_list(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
    let mut data = kind.to_vec();
    data.extend_from_slice(body);
    put_chunk(out, b"LIST", &data);
}

fn le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Écrit la vidéo : les images de `clip` (déjà remplacées), son rythme, ses autres flux.
pub fn write(clip: &Clip) -> Result<Vec<u8>, String> {
    let frames = clip.frame_count() as u32;
    let biggest = clip.frames().map(|c| c.data.len()).max().unwrap_or(0) as u32;
    let total: usize = clip.chunks.iter().map(|c| c.data.len() + 8 + 16).sum();
    if total > 1 << 30 {
        return Err("vidéo trop longue (1 Go au plus)".into());
    }
    let (w, h) = (clip.width as u32, clip.height as u32);
    let micro = (1e6 / clip.fps().max(1e-6)).round() as u32;

    // Le flux vidéo, récrit : même rythme, nouvelles images JPEG.
    let mut strh = b"vidsMJPG".to_vec();
    for x in [0, 0, 0, clip.scale, clip.rate, 0, frames, biggest, u32::MAX, 0] {
        le(&mut strh, x);
    }
    // Le rectangle de l'image (quatre entiers de 16 bits).
    for x in [0u16, 0, w as u16, h as u16] {
        strh.extend_from_slice(&x.to_le_bytes());
    }
    let mut strf = Vec::new();
    for x in [40, w, h] {
        le(&mut strf, x);
    }
    strf.extend_from_slice(&1u16.to_le_bytes());
    strf.extend_from_slice(&24u16.to_le_bytes());
    strf.extend_from_slice(b"MJPG");
    for x in [w * h * 3, 0, 0, 0, 0] {
        le(&mut strf, x);
    }
    let mut video = Vec::new();
    put_chunk(&mut video, b"strh", &strh);
    put_chunk(&mut video, b"strf", &strf);

    let mut hdrl = Vec::new();
    let mut avih = Vec::new();
    let streams = clip.streams.len().max(clip.video + 1) as u32;
    for x in [micro, 0, 0, 0x10, frames, 0, streams, biggest, w, h, 0, 0, 0, 0] {
        le(&mut avih, x);
    }
    put_chunk(&mut hdrl, b"avih", &avih);
    for k in 0..streams as usize {
        if k == clip.video {
            put_list(&mut hdrl, b"strl", &video);
        } else if let Some(s) = clip.streams.get(k) {
            // Les autres flux (le son) : leur liste telle quelle, sans l'index OpenDML
            // (`indx`), dont les positions comptent dans l'ancien fichier.
            let mut kept = Vec::new();
            for (id, d) in riff_chunks(&s[4..]).filter(|(id, _)| id != b"indx") {
                put_chunk(&mut kept, &id, d);
            }
            put_list(&mut hdrl, &[s[0], s[1], s[2], s[3]], &kept);
        }
    }

    // Les données, puis l'index : chaque position compte depuis le mot `movi`.
    let mut movi = Vec::with_capacity(total);
    let mut index = Vec::with_capacity(clip.chunks.len() * 16);
    for c in &clip.chunks {
        index.extend_from_slice(&c.id);
        le(&mut index, 0x10);
        le(&mut index, movi.len() as u32 + 4);
        le(&mut index, c.data.len() as u32);
        put_chunk(&mut movi, &c.id, &c.data);
    }
    let mut body = b"AVI ".to_vec();
    put_list(&mut body, b"hdrl", &hdrl);
    put_list(&mut body, b"movi", &movi);
    put_chunk(&mut body, b"idx1", &index);
    let mut out = Vec::with_capacity(body.len() + 8);
    put_chunk(&mut out, b"RIFF", &body);
    Ok(out)
}

/// Applique `look` (le développement d'une image, reçue comme une photo déjà
/// développée, en sRGB linéaire) à chaque image de la vidéo, qui en ressort en JPEG à `quality`. `progress` reçoit le rang de l'image faite
/// et le total. Une image vide (répétition) reste vide, une image illisible arrête tout.
pub fn develop(
    clip: &mut Clip,
    quality: u8,
    cancel: &Cancel,
    look: &mut dyn FnMut(luma_raw::RawPhoto) -> Result<RgbImage, String>,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<(), String> {
    let video = clip.video;
    let total = clip.frame_count();
    let mut done = 0;
    let mut size = None;
    for c in &mut clip.chunks {
        if stream_of(&c.id) != Some(video) || !matches!(&c.id[2..], b"dc" | b"db") || c.data.is_empty() {
            continue;
        }
        if cancel.is_cancelled() {
            return Err("annulé".into());
        }
        let jpeg = crate::jpeg::with_standard_tables(&c.data);
        let decoded = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg)
            .map_err(|e| format!("image {} : {e}", done + 1))?;
        let raw = luma_raw::raster::from_image(&decoded, luma_raw::raster::Space::SRGB, luma_raw::Orientation::Normal);
        let out = look(raw)?;
        let rgb = luma_image::encode8(&out, luma_color::OutputSpace::Srgb);
        c.data = crate::jpeg::encode(&rgb, out.width, out.height, quality, &[], &[])?;
        c.id = [c.id[0], c.id[1], b'd', b'c'];
        size.get_or_insert((out.width, out.height));
        done += 1;
        progress(done, total);
    }
    if let Some((w, h)) = size {
        clip.width = w;
        clip.height = h;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une vidéo de synthèse : `n` images JPEG (un dégradé qui glisse) et un morceau de
    /// son entre chaque image.
    pub(crate) fn sample(n: usize, w: usize, h: usize) -> Vec<u8> {
        let mut chunks = Vec::new();
        for k in 0..n {
            let rgb: Vec<u8> = (0..w * h)
                .flat_map(|i| {
                    let (x, y) = (i % w, i / w);
                    [((x + k * 4) * 255 / w) as u8, (y * 255 / h) as u8, 90]
                })
                .collect();
            chunks.push(Chunk {
                id: *b"00dc",
                data: crate::jpeg::encode(&rgb, w, h, 90, &[], &[]).unwrap(),
            });
            chunks.push(Chunk {
                id: *b"01wb",
                data: vec![k as u8; 7],
            });
        }
        // Le flux son : un en‑tête quelconque, recopié tel quel.
        let mut audio = b"strl".to_vec();
        put_chunk(&mut audio, b"strh", b"audsaudio-header");
        put_chunk(&mut audio, b"strf", &[1, 0, 1, 0]);
        let clip = Clip {
            width: w,
            height: h,
            rate: 25,
            scale: 1,
            video: 0,
            streams: vec![b"strl".to_vec(), audio],
            chunks,
        };
        write(&clip).unwrap()
    }

    #[test]
    fn written_and_read_back() {
        let bytes = sample(5, 48, 32);
        let clip = read(&bytes).unwrap();
        assert_eq!((clip.width, clip.height, clip.fps()), (48, 32, 25.0));
        assert_eq!(clip.frame_count(), 5);
        assert!((clip.seconds() - 0.2).abs() < 1e-9);
        // Le son est là, dans l'ordre, et son en‑tête aussi.
        let sound: Vec<_> = clip.chunks.iter().filter(|c| &c.id == b"01wb").collect();
        assert_eq!(sound.len(), 5);
        assert_eq!(sound[3].data, vec![3; 7]);
        assert_eq!(write(&clip).unwrap(), bytes);
        // Chaque image se relit en JPEG à la bonne taille.
        for f in clip.frames() {
            let img = image::load_from_memory(&f.data).unwrap();
            assert_eq!((img.width(), img.height()), (48, 32));
        }
    }

    #[test]
    fn index_points_at_each_chunk() {
        let bytes = sample(3, 16, 16);
        let movi = bytes.windows(4).position(|w| w == b"movi").unwrap();
        let idx = bytes.windows(4).rposition(|w| w == b"idx1").unwrap();
        let entries = &bytes[idx + 8..];
        assert_eq!(entries.len(), 6 * 16);
        for e in entries.chunks(16) {
            let at = movi + u32_at(e, 8).unwrap() as usize;
            assert_eq!(&bytes[at..at + 4], &e[..4]);
            assert_eq!(u32_at(&bytes, at + 4), u32_at(e, 12));
        }
    }

    #[test]
    fn develop_changes_every_frame_and_keeps_the_sound() {
        let mut clip = read(&sample(4, 32, 24)).unwrap();
        let before: Vec<_> = clip.frames().cloned().collect();
        let mut seen = Vec::new();
        // Un « look » simple : l'image deux fois plus sombre.
        develop(
            &mut clip,
            85,
            &Cancel::new(),
            &mut |raw| {
                Ok(RgbImage {
                    width: raw.width,
                    height: raw.height,
                    data: raw.data.as_chunks::<3>().0.iter().map(|p| p.map(|v| v * 0.5)).collect(),
                })
            },
            &mut |k, n| seen.push((k, n)),
        )
        .unwrap();
        assert_eq!(seen, vec![(1, 4), (2, 4), (3, 4), (4, 4)]);
        let mean = |c: &Chunk| {
            let img = image::load_from_memory(&c.data).unwrap().to_rgb8();
            img.as_raw().iter().map(|&v| f64::from(v)).sum::<f64>() / img.as_raw().len() as f64
        };
        for (a, b) in before.iter().zip(clip.frames()) {
            assert!(mean(b) < mean(a) - 20.0, "{} → {}", mean(a), mean(b));
        }
        let back = read(&write(&clip).unwrap()).unwrap();
        assert_eq!(back.frame_count(), 4);
        assert_eq!(back.chunks.iter().filter(|c| &c.id == b"01wb").count(), 4);
    }

    #[test]
    fn the_sound_index_of_the_old_file_is_dropped() {
        // Un fichier OpenDML : le son porte un `indx` aux positions de l'ancien fichier.
        let mut clip = read(&sample(2, 16, 16)).unwrap();
        put_chunk(&mut clip.streams[1], b"indx", &[0xAB; 24]);
        let bytes = write(&clip).unwrap();
        assert!(!bytes.windows(4).any(|w| w == b"indx"));
        let back = read(&bytes).unwrap();
        assert!(
            back.streams[1].windows(4).any(|w| w == b"strf"),
            "le reste de l'en‑tête reste"
        );
        assert_eq!(back.chunks.iter().filter(|c| &c.id == b"01wb").count(), 2);
    }

    #[test]
    fn refuses_what_it_cannot_read() {
        assert!(read(b"RIFF\0\0\0\0WAVE").is_err());
        assert!(read(b"pas une video").is_err());
        // Une vidéo dont les images ne sont pas des JPEG.
        let mut bytes = sample(2, 16, 16);
        let at = bytes.windows(8).position(|w| w == b"vidsMJPG").unwrap();
        bytes[at + 4..at + 8].copy_from_slice(b"H264");
        let f = bytes.windows(4).position(|w| w == b"MJPG").unwrap();
        bytes[f..f + 4].copy_from_slice(b"H264");
        for i in 0..bytes.len() - 6 {
            if &bytes[i..i + 4] == b"00dc" {
                bytes[i + 8] = 0;
            }
        }
        assert!(read(&bytes).unwrap_err().contains("Motion JPEG"));
    }
}
