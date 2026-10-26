//! Le doublon de travail d'une photo : la même image, en pleine définition, prête à
//! retoucher.
//!
//! Un RAW se décode (rawler) puis se dématrice à chaque ouverture, et il reste en
//! mémoire tant que la photo est ouverte. Le doublon, écrit une fois en arrière‑plan,
//! garde l'image déjà dématricée, en RGB caméra linéaire, en flottants 24 bits (les 24
//! bits forts d'un flottant 32 bits, arrondis : 17 bits de précision, un écart relatif
//! d'au plus 0,0008 %, bien en deçà des 14 bits d'un capteur ; vérifié par les tests au
//! 1/255 près, même aux hautes lumières), avec ses réductions successives par deux (une
//! pyramide). Chaque niveau est découpé en bandes compressées sans perte (écarts au
//! pixel de gauche, octets séparés par poids, LZ4), décompressées en parallèle.
//!
//! À la retouche, seul le niveau utile est lu : l'aperçu n'a besoin que d'une image de
//! la taille de l'écran ; la pleine définition n'est lue que pour le zoom 100 % et
//! l'export. Le RAW n'est plus en mémoire.
//!
//! Le doublon porte la taille et la date du fichier d'origine : modifié, l'original
//! est relu et le doublon refait.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use luma_core::{Error, Result, RgbImage};
use luma_raw::{Exif, Mosaic, Orientation, RawPhoto};
use rayon::prelude::*;
use serde_json::{Value, json};

// 3 : l'auteur, le copyright et la position GPS (recopiés à l'export) ; 4 : les écarts
// au pixel de gauche (moitié moins gros).
const MAGIC: &[u8; 8] = b"LUMADUP4";
/// Lignes par bande compressée.
const BAND: usize = 64;
/// Le plus petit niveau de la pyramide (grand côté), sous lequel on s'arrête.
const SMALLEST: usize = 512;
/// L'aperçu de l'app (grand côté) : un niveau de cette taille exacte quand la
/// réduction par deux passerait dessous, pour ne pas relire la pleine définition.
const PREVIEW: usize = 2048;

/// La signature du fichier d'origine : taille et date de modification.
pub(crate) fn signature(source: &Path) -> Option<(u64, u128)> {
    let m = std::fs::metadata(source).ok()?;
    let t = m
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((m.len(), t))
}

/// Le nom du doublon d'un fichier, dans le dossier des doublons : son chemin, sa
/// taille et sa date (FNV‑1a 64 bits).
pub fn path_for(source: &Path, dir: &Path) -> Option<PathBuf> {
    let (len, time) = signature(source)?;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let key = format!("{}|{len}|{time}", source.display());
    for b in key.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    Some(dir.join(format!("{h:016x}.lumadup")))
}

fn orientation_code(o: Orientation) -> u8 {
    use Orientation as O;
    match o {
        O::Normal => 1,
        O::FlipHorizontal => 2,
        O::Rotate180 => 3,
        O::FlipVertical => 4,
        O::Transpose => 5,
        O::Rotate90 => 6,
        O::Transverse => 7,
        O::Rotate270 => 8,
    }
}

fn orientation_of(code: u64) -> Orientation {
    use Orientation as O;
    match code {
        2 => O::FlipHorizontal,
        3 => O::Rotate180,
        4 => O::FlipVertical,
        5 => O::Transpose,
        6 => O::Rotate90,
        7 => O::Transverse,
        8 => O::Rotate270,
        _ => O::Normal,
    }
}

/// Tout ce que la photo sait d'elle‑même, sauf ses pixels.
fn meta(raw: &RawPhoto, source: (u64, u128)) -> Value {
    json!({
        "source_len": source.0,
        "source_time": source.1.to_string(),
        "make": raw.make,
        "model": raw.model,
        "as_shot_neutral": raw.as_shot_neutral,
        "matrices": raw.matrices.iter().map(|(t, m)| json!([t, m])).collect::<Vec<_>>(),
        "orientation": orientation_code(raw.orientation),
        "baseline_exposure": raw.baseline_exposure,
        "rendered": raw.rendered,
        // Les corrections d'objectif du fichier sont déjà dans les pixels du doublon.
        "lens_corrected": raw.opcodes.is_some(),
        "exif": {
            "iso": raw.exif.iso,
            "exposure_time": raw.exif.exposure_time,
            "aperture": raw.exif.aperture,
            "focal_length": raw.exif.focal_length,
            "lens": raw.exif.lens,
            "date": raw.exif.date,
            "artist": raw.exif.artist,
            "copyright": raw.exif.copyright,
            "gps": raw.exif.gps.map(|g| [Some(g.latitude), Some(g.longitude), g.altitude]),
        },
    })
}

fn f32s<const N: usize>(v: &Value) -> Option<[f32; N]> {
    let a = v.as_array()?;
    (a.len() == N).then(|| std::array::from_fn(|i| a[i].as_f64().unwrap_or(0.0) as f32))
}

/// La photo (sans pixels) décrite par l'en‑tête d'un doublon.
fn photo_of(m: &Value, width: usize, height: usize) -> Option<RawPhoto> {
    let matrices = m["matrices"]
        .as_array()?
        .iter()
        .filter_map(|e| {
            let t = e.get(0)?.as_f64()? as f32;
            let rows = e.get(1)?.as_array()?;
            (rows.len() == 3).then(|| (t, std::array::from_fn(|i| f32s::<3>(&rows[i]).unwrap_or_default())))
        })
        .collect::<Vec<_>>();
    if matrices.is_empty() {
        return None;
    }
    let e = &m["exif"];
    let f = |k: &str| e[k].as_f64().map(|v| v as f32);
    let s = |k: &str| e[k].as_str().map(str::to_owned);
    Some(RawPhoto {
        make: m["make"].as_str().unwrap_or_default().to_owned(),
        model: m["model"].as_str().unwrap_or_default().to_owned(),
        width,
        height,
        mosaic: Mosaic::Linear,
        data: Vec::new(),
        as_shot_neutral: f32s::<3>(&m["as_shot_neutral"])?,
        matrices,
        orientation: orientation_of(m["orientation"].as_u64().unwrap_or(1)),
        baseline_exposure: m["baseline_exposure"].as_f64().unwrap_or(0.0) as f32,
        exif: Exif {
            iso: e["iso"].as_u64().map(|v| v as u32),
            exposure_time: f("exposure_time"),
            aperture: f("aperture"),
            focal_length: f("focal_length"),
            lens: s("lens"),
            date: s("date"),
            artist: s("artist"),
            copyright: s("copyright"),
            gps: e["gps"].as_array().and_then(|g| {
                Some(luma_raw::Gps {
                    latitude: g.first()?.as_f64()?,
                    longitude: g.get(1)?.as_f64()?,
                    altitude: g.get(2).and_then(Value::as_f64),
                })
            }),
        },
        rendered: m["rendered"].as_bool().unwrap_or(false),
        opcodes: None,
    })
}

/// Un flottant réduit à ses 24 bits forts (signe, exposant, 15 bits de mantisse),
/// arrondi au plus près.
fn to_f24(v: f32) -> u32 {
    let b = v.to_bits();
    // Pas d'arrondi qui déborderait sur l'exposant des infinis et NaN.
    if (b >> 23) & 0xff == 0xff {
        b >> 8
    } else {
        (b.saturating_add(0x80)) >> 8
    }
}

fn from_f24(b: u32) -> f32 {
    f32::from_bits(b << 8)
}

/// Une valeur 24 bits signée repliée sur les naturels : 0, −1, 1, −2… deviennent 0,
/// 1, 2, 3… (les petits écarts, positifs ou négatifs, gardent des octets forts nuls).
fn fold24(d: u32) -> u32 {
    let d = ((d << 8) as i32) >> 8;
    (((d << 1) ^ (d >> 31)) as u32) & 0xff_ffff
}

fn unfold24(z: u32) -> u32 {
    ((z >> 1) ^ (z & 1).wrapping_neg()) & 0xff_ffff
}

/// Une bande : chaque flottant 24 bits remplacé par son écart au pixel de gauche (même
/// couche, même ligne ; les bits d'un flottant positif croissent avec lui), replié ;
/// puis les octets forts des écarts, les moyens, les faibles (le bruit est dans les
/// faibles : les forts, presque tous nuls, se compressent très bien), en LZ4.
/// Sans perte : les flottants relus sont ceux écrits, au bit près.
fn encode_band(px: &[[f32; 3]], width: usize) -> Vec<u8> {
    let n = px.len() * 3;
    let mut planes = vec![0u8; n * 3];
    let mut i = 0;
    for row in px.chunks(width) {
        let mut left = [0u32; 3];
        for p in row {
            for c in 0..3 {
                let b = to_f24(p[c]);
                let z = fold24(b.wrapping_sub(left[c]));
                left[c] = b;
                planes[i] = (z >> 16) as u8;
                planes[n + i] = (z >> 8) as u8;
                planes[2 * n + i] = z as u8;
                i += 1;
            }
        }
    }
    lz4_flex::compress_prepend_size(&planes)
}

fn decode_band(bytes: &[u8], out: &mut [[f32; 3]], width: usize) -> Result<()> {
    let planes = lz4_flex::decompress_size_prepended(bytes).map_err(|e| Error::Decode(e.to_string()))?;
    let n = out.len() * 3;
    if planes.len() != n * 3 {
        return Err(Error::Decode("doublon : bande de mauvaise taille".into()));
    }
    let (high, rest) = planes.split_at(n);
    let (mid, low) = rest.split_at(n);
    let mut i = 0;
    for row in out.chunks_mut(width) {
        let mut left = [0u32; 3];
        for p in row {
            for c in 0..3 {
                let z = (u32::from(high[i]) << 16) | (u32::from(mid[i]) << 8) | u32::from(low[i]);
                let b = left[c].wrapping_add(unfold24(z)) & 0xff_ffff;
                left[c] = b;
                p[c] = from_f24(b);
                i += 1;
            }
        }
    }
    Ok(())
}

/// Écrit le doublon de `raw` (lu depuis `source`) dans `path`. Long (un dématriçage
/// complet) : à faire en arrière‑plan. Écrit à côté puis renommé : jamais de doublon à
/// moitié écrit.
pub fn write(raw: &RawPhoto, source: &Path, path: &Path) -> Result<()> {
    let sig = signature(source).ok_or_else(|| Error::Decode("original introuvable".into()))?;
    // La pyramide : la pleine définition, puis des réductions par deux (avec un arrêt à
    // la taille de l'aperçu).
    let mut levels = vec![raw.demosaic()];
    while levels.last().is_some_and(|l| l.width.max(l.height) > SMALLEST * 2) {
        let l = levels.last().expect("niveau");
        let long = l.width.max(l.height);
        let (w, h) = if long > PREVIEW && long.div_ceil(2) < PREVIEW {
            RgbImage::fit(l.width, l.height, PREVIEW)
        } else {
            (l.width.div_ceil(2), l.height.div_ceil(2))
        };
        let next = l.resize_area(w, h);
        levels.push(next);
    }
    let header = serde_json::to_vec(&meta(raw, sig)).map_err(|e| Error::Decode(e.to_string()))?;
    // Chaque niveau : ses bandes compressées en parallèle.
    let bodies: Vec<(usize, usize, Vec<Vec<u8>>)> = levels
        .iter()
        .map(|l| {
            let bands = l
                .data
                .par_chunks(l.width * BAND)
                .map(|b| encode_band(b, l.width))
                .collect();
            (l.width, l.height, bands)
        })
        .collect();

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(Error::Io)?;
    }
    let tmp = path.with_extension("tmp");
    let mut f = std::io::BufWriter::new(File::create(&tmp).map_err(Error::Io)?);
    let mut put = |b: &[u8]| f.write_all(b).map_err(Error::Io);
    put(MAGIC)?;
    put(&(header.len() as u32).to_le_bytes())?;
    put(&header)?;
    put(&(bodies.len() as u32).to_le_bytes())?;
    // La table des niveaux : taille, puis la longueur de chaque bande.
    for (w, h, bands) in &bodies {
        put(&(*w as u32).to_le_bytes())?;
        put(&(*h as u32).to_le_bytes())?;
        put(&(bands.len() as u32).to_le_bytes())?;
        for b in bands {
            put(&(b.len() as u32).to_le_bytes())?;
        }
    }
    for (_, _, bands) in &bodies {
        for b in bands {
            put(b)?;
        }
    }
    f.flush().map_err(Error::Io)?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(Error::Io)
}

/// Un niveau de la pyramide, dans le fichier.
#[derive(Clone, Debug)]
struct Level {
    width: usize,
    height: usize,
    offset: u64,
    bands: Vec<u32>,
}

/// Un doublon ouvert : son en‑tête et la table de ses niveaux ; les pixels restent sur
/// le disque jusqu'à ce qu'un niveau soit demandé.
#[derive(Debug)]
pub struct Duplicate {
    path: PathBuf,
    levels: Vec<Level>,
    /// Les pixels portent déjà les corrections d'objectif du fichier.
    pub lens_corrected: bool,
}

fn read_u32(f: &mut impl Read) -> Result<u32> {
    let mut b = [0u8; 4];
    f.read_exact(&mut b).map_err(Error::Io)?;
    Ok(u32::from_le_bytes(b))
}

impl Duplicate {
    /// Ouvre le doublon de `source` s'il existe et correspond encore à l'original ;
    /// rend aussi la photo (sans pixels) qu'il décrit.
    pub fn open(source: &Path, path: &Path) -> Option<(Duplicate, RawPhoto)> {
        let sig = signature(source)?;
        let mut f = std::io::BufReader::new(File::open(path).ok()?);
        let mut magic = [0u8; 8];
        f.read_exact(&mut magic).ok()?;
        if &magic != MAGIC {
            return None;
        }
        let n = read_u32(&mut f).ok()? as usize;
        let mut header = vec![0u8; n];
        f.read_exact(&mut header).ok()?;
        let m: Value = serde_json::from_slice(&header).ok()?;
        if m["source_len"].as_u64() != Some(sig.0) || m["source_time"].as_str() != Some(&sig.1.to_string()) {
            return None;
        }
        let count = read_u32(&mut f).ok()? as usize;
        let mut levels = Vec::with_capacity(count);
        for _ in 0..count {
            let width = read_u32(&mut f).ok()? as usize;
            let height = read_u32(&mut f).ok()? as usize;
            let nb = read_u32(&mut f).ok()? as usize;
            let bands = (0..nb).map(|_| read_u32(&mut f)).collect::<Result<Vec<_>>>().ok()?;
            levels.push(Level {
                width,
                height,
                offset: 0,
                bands,
            });
        }
        let mut offset = f.stream_position().ok()?;
        for l in &mut levels {
            l.offset = offset;
            offset += l.bands.iter().map(|&b| u64::from(b)).sum::<u64>();
        }
        let full = levels.first()?;
        let raw = photo_of(&m, full.width, full.height)?;
        // Marque d'usage, pour que le ménage garde les doublons servis récemment.
        if let Ok(file) = File::options().append(true).open(path) {
            let _ = file.set_modified(std::time::SystemTime::now());
        }
        Some((
            Duplicate {
                path: path.to_owned(),
                levels,
                lens_corrected: m["lens_corrected"].as_bool().unwrap_or(false),
            },
            raw,
        ))
    }

    /// Les tailles des niveaux, du plus grand au plus petit.
    pub fn sizes(&self) -> Vec<(usize, usize)> {
        self.levels.iter().map(|l| (l.width, l.height)).collect()
    }

    /// Le niveau le plus petit dont le grand côté atteint `side` (sinon le plus grand).
    pub fn level_for(&self, side: usize) -> usize {
        self.levels
            .iter()
            .rposition(|l| l.width.max(l.height) >= side)
            .unwrap_or(0)
    }

    /// Lit un niveau (RGB caméra linéaire, non orienté).
    pub fn read(&self, index: usize) -> Result<RgbImage> {
        let l = &self.levels[index];
        let total: u64 = l.bands.iter().map(|&b| u64::from(b)).sum();
        let mut f = File::open(&self.path).map_err(Error::Io)?;
        f.seek(SeekFrom::Start(l.offset)).map_err(Error::Io)?;
        let mut bytes = vec![0u8; total as usize];
        f.read_exact(&mut bytes).map_err(Error::Io)?;
        let mut starts = Vec::with_capacity(l.bands.len());
        let mut at = 0usize;
        for &b in &l.bands {
            starts.push((at, b as usize));
            at += b as usize;
        }
        let mut img = RgbImage {
            width: l.width,
            height: l.height,
            data: vec![[0.0; 3]; l.width * l.height],
        };
        img.data
            .par_chunks_mut(l.width * BAND)
            .zip(starts.par_iter())
            .try_for_each(|(out, &(s, n))| decode_band(&bytes[s..s + n], out, l.width))?;
        Ok(img)
    }
}

/// Le ménage : au‑delà de `budget` octets, les doublons et les vignettes servis il y a
/// le plus longtemps partent.
pub fn trim(dir: &Path, budget: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x == "lumadup" || x == crate::thumbnail::EXTENSION)
        })
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort();
    for (_, len, path) in files {
        if total <= budget {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

/// La place que prennent les doublons, en octets.
pub fn usage(dir: &Path) -> u64 {
    std::fs::read_dir(dir).map_or(0, |entries| {
        entries
            .flatten()
            .filter_map(|e| e.metadata().ok().filter(|m| m.is_file()).map(|m| m.len()))
            .sum()
    })
}

/// Vide le dossier des doublons, vignettes comprises (et ce qui est à moitié écrit) ;
/// rend la place libérée, en octets.
pub fn clear(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x == "lumadup" || x == "tmp" || x == crate::thumbnail::EXTENSION)
        })
        .filter_map(|e| {
            let len = e.metadata().ok()?.len();
            std::fs::remove_file(e.path()).ok().map(|()| len)
        })
        .sum()
}

/// La place que les doublons peuvent prendre : `LUMA_DUPLICATES_GB` Go, 20 par défaut
/// (8 sur iPhone et iPad).
pub fn budget() -> u64 {
    let gb = std::env::var("LUMA_DUPLICATES_GB")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(if cfg!(target_os = "ios") { 8 } else { 20 });
    gb * 1024 * 1024 * 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_come_back_bit_for_bit() {
        // Des pentes douces, des sauts, des négatifs, zéro, l'infini, du bruit : les
        // flottants 24 bits relus sont exactement ceux écrits.
        let width = 37;
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut noise = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        let px: Vec<[f32; 3]> = (0..width * 5)
            .map(|i| match i % 9 {
                0 => [0.0, -0.0, f32::INFINITY],
                1 => [-0.25, 1e-30, 65504.0],
                2 => [noise(), -noise(), noise() * 1000.0],
                _ => [i as f32 * 0.001, 0.5 + noise() * 0.01, 1.0 - i as f32 * 1e-4],
            })
            .collect();
        let mut back = vec![[1.0f32; 3]; px.len()];
        decode_band(&encode_band(&px, width), &mut back, width).unwrap();
        for (a, b) in px.iter().zip(&back) {
            for c in 0..3 {
                assert_eq!(from_f24(to_f24(a[c])).to_bits(), b[c].to_bits(), "{a:?} → {b:?}");
            }
        }
    }

    #[test]
    fn folding_is_reversible() {
        for d in [0u32, 1, 2, 0xff_ffff, 0x80_0000, 0x7f_ffff, 0x12_3456] {
            assert_eq!(unfold24(fold24(d)), d);
        }
        // Les petits écarts, dans un sens ou dans l'autre, restent petits.
        assert_eq!(fold24(0xff_ffff), 1);
        assert_eq!(fold24(1), 2);
    }
}
