//! L'EXIF des images déjà développées (JPEG, HEIC et AVIF, PNG, WebP, TIFF) : la date de
//! prise de vue, le boîtier et l'objectif, les réglages et la position GPS, comme pour
//! un RAW. Les photos d'un téléphone trouvent ainsi leur vraie date dans la photothèque
//! et leur place sur la carte.
//!
//! Seul le début du fichier est lu (l'EXIF d'un JPEG est au tout début), et pour le HEIC
//! le seul morceau où l'EXIF est rangé : indexer un dossier reste rapide.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::{Exif, Gps, Info};

/// Ce qu'on lit d'abord de chaque fichier.
const HEAD: usize = 256 << 10;

fn read_at(path: &Path, offset: u64, len: usize) -> Option<Vec<u8>> {
    let mut f = std::fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(offset)).ok()?;
    let mut b = Vec::with_capacity(len.min(16 << 20));
    f.take(len as u64).read_to_end(&mut b).ok()?;
    Some(b)
}

/// Les métadonnées d'une image développée, si elle a un EXIF.
pub fn read(path: &Path) -> Option<Info> {
    let head = read_at(path, 0, HEAD)?;
    if let Some((offset, len)) = heif_exif(&head) {
        // Le HEIC range son EXIF dans un élément à part : quatre octets donnent le
        // décalage de l'en‑tête TIFF.
        let item = read_at(path, offset, len.min(1 << 20))?;
        let skip = u32::from_be_bytes(item.get(..4)?.try_into().ok()?) as usize;
        return parse(item.get(4 + skip..)?);
    }
    let tiff = find(&head)?;
    parse(tiff)
}

/// Le bloc TIFF de l'EXIF dans le début d'un fichier JPEG, PNG, WebP ou TIFF.
pub fn find(b: &[u8]) -> Option<&[u8]> {
    if b.starts_with(b"II*\0") || b.starts_with(b"MM\0*") {
        return Some(b);
    }
    if b.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 4 <= b.len() && b[i] == 0xFF {
            let marker = b[i + 1];
            if marker == 0xFF {
                i += 1;
                continue;
            }
            // Le début des données : l'EXIF n'est plus à venir.
            if marker == 0xDA || marker == 0xD9 {
                return None;
            }
            let n = usize::from(u16::from_be_bytes([b[i + 2], b[i + 3]]));
            let data = b.get(i + 4..(i + 2 + n).min(b.len()))?;
            if marker == 0xE1 && data.starts_with(b"Exif\0\0") {
                return Some(&data[6..]);
            }
            i += 2 + n;
        }
        return None;
    }
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        let mut i = 8;
        while i + 8 <= b.len() {
            let n = u32::from_be_bytes(b[i..i + 4].try_into().ok()?) as usize;
            let data = b.get(i + 8..(i + 8).saturating_add(n).min(b.len()))?;
            match &b[i + 4..i + 8] {
                b"eXIf" => return Some(data),
                b"IDAT" | b"IEND" => return None,
                _ => {}
            }
            i = i.saturating_add(12 + n);
        }
        return None;
    }
    if b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        let mut i = 12;
        while i + 8 <= b.len() {
            let n = u32::from_le_bytes(b[i + 4..i + 8].try_into().ok()?) as usize;
            let data = b.get(i + 8..(i + 8).saturating_add(n).min(b.len()))?;
            if &b[i..i + 4] == b"EXIF" {
                return Some(data.strip_prefix(b"Exif\0\0").unwrap_or(data));
            }
            i = i.saturating_add(8 + n + (n & 1));
        }
    }
    None
}

// ----- HEIC et AVIF (ISOBMFF) ---------------------------------------------------------

/// Les boîtes d'un bloc : (type, contenu).
fn boxes(b: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut i = 0;
    std::iter::from_fn(move || {
        if i + 8 > b.len() {
            return None;
        }
        let size = u32::from_be_bytes(b[i..i + 4].try_into().ok()?) as u64;
        let kind: [u8; 4] = b[i + 4..i + 8].try_into().ok()?;
        let (start, size) = match size {
            0 => (i + 8, (b.len() - i) as u64),
            1 => (i + 16, u64::from_be_bytes(b.get(i + 8..i + 16)?.try_into().ok()?)),
            s => (i + 8, s),
        };
        let end = i.saturating_add(usize::try_from(size).ok()?);
        // La taille compte l'en-tête (8 octets, 16 avec la grande taille).
        if size < (start - i) as u64 || start > b.len() {
            return None;
        }
        let body = &b[start..end.min(b.len())];
        i = end;
        Some((kind, body))
    })
}

/// Un entier de `n` octets (0, 4 ou 8), gros‑boutiste, lu à `*i`.
fn be(b: &[u8], i: &mut usize, n: usize) -> Option<u64> {
    let v = b.get(*i..*i + n)?.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x));
    *i += n;
    Some(v)
}

/// La place de l'élément EXIF d'un HEIC ou d'un AVIF : (décalage, longueur).
fn heif_exif(b: &[u8]) -> Option<(u64, usize)> {
    let (kind, ftyp) = boxes(b).next()?;
    if &kind != b"ftyp" || ftyp.len() < 4 {
        return None;
    }
    let (_, meta) = boxes(b).find(|(k, _)| k == b"meta")?;
    let children = meta.get(4..)?;
    // L'élément de type « Exif ».
    let (_, iinf) = boxes(children).find(|(k, _)| k == b"iinf")?;
    let mut i = 4;
    be(iinf, &mut i, if *iinf.first()? == 0 { 2 } else { 4 })?;
    let id = boxes(iinf.get(i..)?).find_map(|(k, infe)| {
        if &k != b"infe" || infe.first().is_none_or(|&v| v < 2) {
            return None;
        }
        let mut j = 4;
        let id = be(infe, &mut j, if infe[0] == 2 { 2 } else { 4 })?;
        j += 2;
        (infe.get(j..j + 4)? == b"Exif").then_some(id)
    })?;
    // Sa place dans le fichier.
    let (_, iloc) = boxes(children).find(|(k, _)| k == b"iloc")?;
    let version = *iloc.first()?;
    let mut i = 4;
    let sizes = be(iloc, &mut i, 1)? as usize;
    let more = be(iloc, &mut i, 1)? as usize;
    let (offset_size, length_size, base_size) = (sizes >> 4, sizes & 15, more >> 4);
    let index_size = if version >= 1 { more & 15 } else { 0 };
    let count = be(iloc, &mut i, if version < 2 { 2 } else { 4 })?;
    for _ in 0..count {
        let item = be(iloc, &mut i, if version < 2 { 2 } else { 4 })?;
        let method = if version >= 1 { be(iloc, &mut i, 2)? & 15 } else { 0 };
        be(iloc, &mut i, 2)?;
        let base = be(iloc, &mut i, base_size)?;
        let extents = be(iloc, &mut i, 2)?;
        let mut first = None;
        for _ in 0..extents {
            be(iloc, &mut i, index_size)?;
            let off = be(iloc, &mut i, offset_size)?;
            let len = be(iloc, &mut i, length_size)?;
            first.get_or_insert((base.checked_add(off)?, len as usize));
        }
        if item == id {
            // Seule la place dans le fichier est lue (pas les données dans `idat`).
            return first.filter(|_| method == 0);
        }
    }
    None
}

// ----- TIFF ---------------------------------------------------------------------------

struct Tiff<'a> {
    b: &'a [u8],
    little: bool,
}

/// Une entrée d'IFD : son type, son nombre de valeurs, leur place et leurs octets.
struct Tag<'a> {
    kind: u16,
    count: usize,
    at: usize,
    data: &'a [u8],
}

impl<'a> Tiff<'a> {
    fn u16(&self, i: usize) -> Option<u16> {
        let v: [u8; 2] = self.b.get(i..i + 2)?.try_into().ok()?;
        Some(if self.little {
            u16::from_le_bytes(v)
        } else {
            u16::from_be_bytes(v)
        })
    }

    fn u32(&self, i: usize) -> Option<u32> {
        let v: [u8; 4] = self.b.get(i..i + 4)?.try_into().ok()?;
        Some(if self.little {
            u32::from_le_bytes(v)
        } else {
            u32::from_be_bytes(v)
        })
    }

    fn ifd(&self, at: usize) -> Vec<(u16, Tag<'a>)> {
        let mut out = Vec::new();
        let Some(n) = self.u16(at) else {
            return out;
        };
        for k in 0..usize::from(n).min(512) {
            let e = at + 2 + 12 * k;
            let (Some(tag), Some(kind), Some(count)) = (self.u16(e), self.u16(e + 2), self.u32(e + 4)) else {
                break;
            };
            let unit = match kind {
                1 | 2 | 6 | 7 => 1,
                3 | 8 => 2,
                4 | 9 | 11 => 4,
                5 | 10 | 12 => 8,
                _ => continue,
            };
            let len = unit * count as usize;
            let start = if len <= 4 {
                e + 8
            } else {
                match self.u32(e + 8) {
                    Some(o) => o as usize,
                    None => continue,
                }
            };
            if let Some(data) = self.b.get(start..start.saturating_add(len)) {
                out.push((
                    tag,
                    Tag {
                        kind,
                        count: count as usize,
                        at: start,
                        data,
                    },
                ));
            }
        }
        out
    }

    fn text(&self, t: &Tag) -> Option<String> {
        // Jusqu'au premier zéro : certains boîtiers laissent des octets au‑delà.
        let end = t.data.iter().position(|&c| c == 0).unwrap_or(t.data.len());
        let s = String::from_utf8_lossy(&t.data[..end]);
        let s = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
        (t.kind == 2 && !s.is_empty()).then(|| s.to_owned())
    }

    /// La `k`‑ième valeur numérique.
    fn number(&self, t: &Tag, k: usize) -> Option<f64> {
        if k >= t.count {
            return None;
        }
        let at = |n: usize| t.at + n;
        Some(match t.kind {
            1 => f64::from(*t.data.get(k)?),
            3 => f64::from(self.u16(at(2 * k))?),
            4 => f64::from(self.u32(at(4 * k))?),
            9 => f64::from(self.u32(at(4 * k))? as i32),
            5 | 10 => {
                let (n, d) = (self.u32(at(8 * k))?, self.u32(at(8 * k + 4))?);
                if d == 0 {
                    return None;
                }
                if t.kind == 5 {
                    f64::from(n) / f64::from(d)
                } else {
                    f64::from(n as i32) / f64::from(d as i32)
                }
            }
            _ => return None,
        })
    }
}

fn get<'t, 'a>(ifd: &'t [(u16, Tag<'a>)], tag: u16) -> Option<&'t Tag<'a>> {
    ifd.iter().find(|(t, _)| *t == tag).map(|(_, v)| v)
}

/// Les métadonnées d'un bloc TIFF d'EXIF.
pub fn parse(b: &[u8]) -> Option<Info> {
    let little = match b.get(..4)? {
        b"II*\0" => true,
        b"MM\0*" => false,
        _ => return None,
    };
    let t = Tiff { b, little };
    let ifd0 = t.ifd(t.u32(4)? as usize);
    let sub_ifd = |tag| get(&ifd0, tag).and_then(|v| t.number(v, 0)).map(|o| t.ifd(o as usize));
    let sub = sub_ifd(0x8769).unwrap_or_default();
    let gps_ifd = sub_ifd(0x8825).unwrap_or_default();
    let text = |ifd: &[(u16, Tag)], tag| get(ifd, tag).and_then(|v| t.text(v));
    let number = |ifd: &[(u16, Tag)], tag| get(ifd, tag).and_then(|v| t.number(v, 0));

    let coordinate = |value: u16, reference: u16, negative: char| -> Option<f64> {
        let v = get(&gps_ifd, value)?;
        let deg = t.number(v, 0)? + t.number(v, 1).unwrap_or(0.0) / 60.0 + t.number(v, 2).unwrap_or(0.0) / 3600.0;
        let neg = text(&gps_ifd, reference).is_some_and(|r| r.starts_with(negative));
        Some(if neg { -deg } else { deg })
    };
    let gps = match (coordinate(2, 1, 'S'), coordinate(4, 3, 'W')) {
        (Some(latitude), Some(longitude))
            if latitude.is_finite() && longitude.is_finite() && (latitude, longitude) != (0.0, 0.0) =>
        {
            let below = get(&gps_ifd, 5).and_then(|v| t.number(v, 0)) == Some(1.0);
            Some(Gps {
                latitude,
                longitude,
                altitude: number(&gps_ifd, 6).map(|a| if below { -a } else { a }),
            })
        }
        _ => None,
    };
    let exif = Exif {
        iso: number(&sub, 0x8827).map(|v| v as u32).filter(|&v| v > 0),
        exposure_time: number(&sub, 0x829A).map(|v| v as f32).filter(|&v| v > 0.0),
        aperture: number(&sub, 0x829D).map(|v| v as f32).filter(|&v| v > 0.0),
        focal_length: number(&sub, 0x920A).map(|v| v as f32).filter(|&v| v > 0.0),
        lens: text(&sub, 0xA434),
        date: text(&sub, 0x9003).or_else(|| text(&ifd0, 0x0132)),
        artist: text(&ifd0, 0x013B),
        copyright: text(&ifd0, 0x8298),
        gps,
    };
    let make = text(&ifd0, 0x010F).unwrap_or_default();
    let model = text(&ifd0, 0x0110).unwrap_or_default();
    (exif != Exif::default() || !make.is_empty() || !model.is_empty()).then_some(Info { make, model, exif })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un bloc TIFF d'EXIF, petit‑boutiste ou gros‑boutiste, comme l'écrit un téléphone.
    pub(crate) fn sample(little: bool) -> Vec<u8> {
        // Chaque IFD : (étiquette, type, nombre, valeur ou données).
        type Entry = (u16, u16, u32, Vec<u8>);
        let w16 = |v: u16| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let w32 = |v: u32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let rational = |n: u32, d: u32| [w32(n), w32(d)].concat();
        let ascii = |s: &str| {
            let mut v = s.as_bytes().to_vec();
            v.push(0);
            v
        };
        let ifd0: Vec<Entry> = vec![
            (0x010F, 2, 6, ascii("Apple")),
            (0x0110, 2, 14, ascii("iPhone 15 Pro")),
            (0x0132, 2, 20, ascii("2026:10:01 09:00:00")),
            (0x8769, 4, 1, vec![]),
            (0x8825, 4, 1, vec![]),
        ];
        let sub: Vec<Entry> = vec![
            (0x829A, 5, 1, rational(1, 120)),
            (0x829D, 5, 1, rational(178, 100)),
            (0x8827, 3, 1, w16(64).to_vec()),
            (0x9003, 2, 20, ascii("2026:09:30 18:42:07")),
            (0x920A, 5, 1, rational(6765, 1000)),
            (0xA434, 2, 26, ascii("iPhone 15 Pro back camera")),
        ];
        let gps: Vec<Entry> = vec![
            (1, 2, 2, ascii("N")),
            (
                2,
                5,
                3,
                [rational(48, 1), rational(51, 1), rational(3024, 100)].concat(),
            ),
            (3, 2, 2, ascii("W")),
            (4, 5, 3, [rational(2, 1), rational(17, 1), rational(4020, 100)].concat()),
            (5, 1, 1, vec![0]),
            (6, 5, 1, rational(355, 10)),
        ];
        let size = |ifd: &[Entry]| 2 + 12 * ifd.len() + 4;
        let extra = |ifd: &[Entry]| -> usize {
            ifd.iter()
                .map(|e| if e.3.len() > 4 { e.3.len() + (e.3.len() & 1) } else { 0 })
                .sum()
        };
        let at0 = 8;
        let at_sub = at0 + size(&ifd0) + extra(&ifd0);
        let at_gps = at_sub + size(&sub) + extra(&sub);
        let mut out = if little { b"II*\0".to_vec() } else { b"MM\0*".to_vec() };
        out.extend_from_slice(&w32(at0 as u32));
        let write = |out: &mut Vec<u8>, ifd: &[Entry], start: usize| {
            let mut data_at = start + size(ifd);
            let mut data = Vec::new();
            out.extend_from_slice(&w16(ifd.len() as u16));
            for (tag, kind, count, v) in ifd {
                out.extend_from_slice(&w16(*tag));
                out.extend_from_slice(&w16(*kind));
                out.extend_from_slice(&w32(*count));
                let v = match *tag {
                    0x8769 => w32(at_sub as u32).to_vec(),
                    0x8825 => w32(at_gps as u32).to_vec(),
                    _ => v.clone(),
                };
                if v.len() > 4 {
                    out.extend_from_slice(&w32(data_at as u32));
                    data_at += v.len() + (v.len() & 1);
                    data.extend_from_slice(&v);
                    if v.len() % 2 == 1 {
                        data.push(0);
                    }
                } else {
                    let mut inline = v.clone();
                    inline.resize(4, 0);
                    out.extend_from_slice(&inline);
                }
            }
            out.extend_from_slice(&[0; 4]);
            out.extend_from_slice(&data);
        };
        write(&mut out, &ifd0, at0);
        write(&mut out, &sub, at_sub);
        write(&mut out, &gps, at_gps);
        out
    }

    fn check(info: &Info) {
        assert_eq!((info.make.as_str(), info.model.as_str()), ("Apple", "iPhone 15 Pro"));
        let e = &info.exif;
        assert_eq!(
            e.date.as_deref(),
            Some("2026:09:30 18:42:07"),
            "la prise de vue, pas l'écriture"
        );
        assert_eq!(
            (e.iso, e.lens.as_deref()),
            (Some(64), Some("iPhone 15 Pro back camera"))
        );
        assert!((e.exposure_time.unwrap() - 1.0 / 120.0).abs() < 1e-6);
        assert!((e.aperture.unwrap() - 1.78).abs() < 1e-6);
        assert!((e.focal_length.unwrap() - 6.765).abs() < 1e-5);
        let g = e.gps.unwrap();
        assert!((g.latitude - 48.8584).abs() < 1e-4, "{}", g.latitude);
        assert!((g.longitude + 2.2945).abs() < 1e-4, "ouest : négatif, {}", g.longitude);
        assert_eq!(g.altitude, Some(35.5));
    }

    #[test]
    fn both_byte_orders() {
        check(&parse(&sample(true)).unwrap());
        check(&parse(&sample(false)).unwrap());
        assert!(parse(b"II*\0\x08\0\0\0\0\0").is_none(), "un EXIF vide");
        assert!(parse(b"pas un TIFF").is_none());
    }

    fn temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("luma-exif-{}-{name}", std::process::id()));
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn found_in_each_format() {
        let tiff = sample(false);
        // JPEG : un segment APP1 après le JFIF.
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xE1];
        jpeg.extend_from_slice(&(tiff.len() as u16 + 8).to_be_bytes());
        jpeg.extend_from_slice(b"Exif\0\0");
        jpeg.extend_from_slice(&tiff);
        jpeg.extend_from_slice(&[0xFF, 0xDA, 0, 2, 1, 2, 3]);
        // PNG : un morceau eXIf avant les données.
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        for (kind, data) in [(&b"IHDR"[..], &[0u8; 13][..]), (b"eXIf", &tiff), (b"IDAT", &[1, 2])] {
            png.extend_from_slice(&(data.len() as u32).to_be_bytes());
            png.extend_from_slice(kind);
            png.extend_from_slice(data);
            png.extend_from_slice(&[0; 4]);
        }
        // WebP : un morceau EXIF.
        let mut webp = b"RIFF\0\0\0\0WEBPVP8X".to_vec();
        webp.extend_from_slice(&10u32.to_le_bytes());
        webp.extend_from_slice(&[0; 10]);
        webp.extend_from_slice(b"EXIF");
        webp.extend_from_slice(&(tiff.len() as u32).to_le_bytes());
        webp.extend_from_slice(&tiff);
        for (name, bytes) in [
            ("a.jpg", jpeg),
            ("a.png", png),
            ("a.webp", webp),
            ("a.tif", tiff.clone()),
        ] {
            let p = temp(name, &bytes);
            check(&read(&p).unwrap_or_else(|| panic!("{name}")));
            check(&crate::info(&p).unwrap());
            let _ = std::fs::remove_file(p);
        }
        // Un JPEG sans EXIF.
        let p = temp("b.jpg", &[0xFF, 0xD8, 0xFF, 0xDA, 0, 2, 0, 0]);
        assert!(read(&p).is_none());
        let _ = std::fs::remove_file(p);
    }

    /// Une boîte ISOBMFF.
    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn found_in_a_heic() {
        let tiff = sample(true);
        let mut item = 6u32.to_be_bytes().to_vec();
        item.extend_from_slice(b"Exif\0\0");
        item.extend_from_slice(&tiff);
        let infe = |id: u16, kind: &[u8; 4]| {
            let mut b = vec![2, 0, 0, 0];
            b.extend_from_slice(&id.to_be_bytes());
            b.extend_from_slice(&[0, 0]);
            b.extend_from_slice(kind);
            b.push(0);
            bx(b"infe", &b)
        };
        let mut iinf = vec![0, 0, 0, 0, 0, 2];
        iinf.extend(infe(1, b"hvc1"));
        iinf.extend(infe(7, b"Exif"));
        // L'élément 7 se trouve après tout l'en‑tête : sa place s'écrit une fois connue.
        let build = |offset: u32| {
            let mut iloc = vec![1, 0, 0, 0, 0x44, 0x00, 0, 2];
            for (id, off, len) in [(1u16, 0u32, 10u32), (7, offset, item.len() as u32)] {
                iloc.extend_from_slice(&id.to_be_bytes());
                iloc.extend_from_slice(&[0, 0, 0, 0, 0, 1]);
                iloc.extend_from_slice(&off.to_be_bytes());
                iloc.extend_from_slice(&len.to_be_bytes());
            }
            let mut meta = vec![0, 0, 0, 0];
            meta.extend(bx(b"hdlr", &[0; 24]));
            meta.extend(bx(b"iinf", &iinf));
            meta.extend(bx(b"iloc", &iloc));
            let mut file = bx(b"ftyp", b"heic\0\0\0\0mif1heic");
            file.extend(bx(b"meta", &meta));
            file
        };
        let len = build(0).len() + 8;
        let mut file = build(len as u32);
        let mut mdat = item.clone();
        mdat.extend_from_slice(&[0; 32]);
        file.extend(bx(b"mdat", &mdat));
        let p = temp("a.heic", &file);
        check(&read(&p).unwrap());
        let _ = std::fs::remove_file(p);
    }
}
