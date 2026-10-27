//! Le fichier du catalogue, en binaire compact.
//!
//! Un catalogue répète les mêmes textes des milliers de fois : le dossier d'une journée,
//! le boîtier, l'objectif, les mots‑clés. Chacun n'est écrit qu'une fois, dans une table
//! en tête du fichier ; une photo n'y garde que des numéros. Les nombres sont écrits en
//! longueur variable (LEB128) : une note ou un ISO tiennent en un ou deux octets.
//!
//! ```text
//! « LUMACAT » 0 · version · table des textes · photos · collections (JSON)
//! ```
//!
//! Une photo dont le chemin n'est pas de l'Unicode valide n'est pas enregistrée : elle
//! sera relue à la prochaine mise à jour de son dossier.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crate::{Catalog, Collection, Entry};

pub(crate) const MAGIC: &[u8; 8] = b"LUMACAT\0";
const VERSION: u64 = 2;

/// La version d'avant la position GPS : encore lue, ses photos relues à la prochaine
/// mise à jour de leur dossier.
const WITHOUT_GPS: u64 = 1;

/// Les textes du fichier, chacun une fois.
#[derive(Default)]
struct Table<'a> {
    index: HashMap<&'a str, u64>,
    texts: Vec<&'a str>,
}

impl<'a> Table<'a> {
    fn id(&mut self, s: &'a str) -> u64 {
        *self.index.entry(s).or_insert_with(|| {
            self.texts.push(s);
            self.texts.len() as u64 - 1
        })
    }
}

fn varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn text(out: &mut Vec<u8>, s: &str) {
    varint(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

/// Un nombre facultatif : 0 s'il manque, sinon la valeur plus un.
fn optional(out: &mut Vec<u8>, v: Option<u32>) {
    varint(out, v.map_or(0, |v| u64::from(v) + 1));
}

/// Un réel facultatif : un octet de présence, puis ses quatre octets.
fn real(out: &mut Vec<u8>, v: Option<f32>) {
    match v {
        Some(v) => {
            out.push(1);
            out.extend_from_slice(&v.to_le_bytes());
        }
        None => out.push(0),
    }
}

pub(crate) fn encode(cat: &Catalog) -> Vec<u8> {
    encode_as(cat, VERSION)
}

/// Le catalogue écrit dans cette version (l'ancienne, pour les essais).
fn encode_as(cat: &Catalog, version: u64) -> Vec<u8> {
    let entries: Vec<(&str, &str, &Entry)> = cat
        .entries
        .values()
        .filter_map(|e| {
            let dir = e.path.parent()?.to_str()?;
            let name = e.path.file_name()?.to_str()?;
            Some((dir, name, e))
        })
        .collect();
    let mut table = Table::default();
    let mut body = Vec::with_capacity(entries.len() * 48);
    varint(&mut body, entries.len() as u64);
    for (dir, name, e) in &entries {
        varint(&mut body, table.id(dir));
        text(&mut body, name);
        for v in [e.size, e.modified, e.sidecar] {
            varint(&mut body, v);
        }
        text(&mut body, &e.date);
        for s in [&e.make, &e.model, &e.lens, &e.label] {
            varint(&mut body, table.id(s));
        }
        optional(&mut body, e.iso);
        real(&mut body, e.aperture);
        real(&mut body, e.exposure_time);
        real(&mut body, e.focal_length);
        match e.gps.filter(|_| version == VERSION) {
            Some([lat, lon]) => {
                body.push(1);
                body.extend_from_slice(&lat.to_le_bytes());
                body.extend_from_slice(&lon.to_le_bytes());
            }
            None if version == VERSION => body.push(0),
            None => {}
        }
        body.push(e.rating);
        body.push(e.flag as u8);
        varint(&mut body, e.keywords.len() as u64);
        for k in &e.keywords {
            varint(&mut body, table.id(k));
        }
    }
    let mut out = Vec::with_capacity(body.len() + table.texts.len() * 24 + 64);
    out.extend_from_slice(MAGIC);
    varint(&mut out, version);
    varint(&mut out, table.texts.len() as u64);
    for t in &table.texts {
        text(&mut out, t);
    }
    out.extend_from_slice(&body);
    text(&mut out, &serde_json::to_string(&cat.collections).unwrap_or_default());
    out
}

/// La lecture, octet par octet ; `None` dès qu'un fichier est tronqué ou abîmé.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Option<u8> {
        let b = *self.bytes.get(self.at)?;
        self.at += 1;
        Some(b)
    }

    fn varint(&mut self) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            v |= u64::from(b & 0x7f) << shift;
            if b < 0x80 {
                return Some(v);
            }
        }
        None
    }

    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.bytes.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }

    fn text(&mut self) -> Option<&'a str> {
        let n = usize::try_from(self.varint()?).ok()?;
        std::str::from_utf8(self.bytes(n)?).ok()
    }

    fn optional(&mut self) -> Option<Option<u32>> {
        Some(match self.varint()? {
            0 => None,
            v => Some(u32::try_from(v - 1).ok()?),
        })
    }

    fn gps(&mut self) -> Option<Option<[f64; 2]>> {
        Some(match self.byte()? {
            0 => None,
            _ => {
                let mut f = || Some(f64::from_le_bytes(self.bytes(8)?.try_into().ok()?));
                Some([f()?, f()?])
            }
        })
    }

    fn real(&mut self) -> Option<Option<f32>> {
        Some(match self.byte()? {
            0 => None,
            _ => Some(f32::from_le_bytes(self.bytes(4)?.try_into().ok()?)),
        })
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Option<Catalog> {
    let mut r = Reader { bytes, at: 0 };
    if r.bytes(MAGIC.len())? != MAGIC {
        return None;
    }
    let version = r.varint()?;
    if version != VERSION && version != WITHOUT_GPS {
        return None;
    }
    let count = usize::try_from(r.varint()?).ok()?;
    // Une taille annoncée plus grande que le fichier : fichier abîmé.
    if count > bytes.len() {
        return None;
    }
    let mut texts = Vec::with_capacity(count);
    for _ in 0..count {
        texts.push(r.text()?);
    }
    let get = |i: u64| texts.get(usize::try_from(i).ok()?).copied();
    let photos = usize::try_from(r.varint()?).ok()?;
    if photos > bytes.len() {
        return None;
    }
    // Le dossier, joint une fois par texte et non une fois par photo.
    let mut dirs: HashMap<u64, PathBuf> = HashMap::new();
    let mut entries = Vec::with_capacity(photos.min(1 << 20));
    for _ in 0..photos {
        let dir = r.varint()?;
        let name = r.text()?;
        if let std::collections::hash_map::Entry::Vacant(v) = dirs.entry(dir) {
            v.insert(PathBuf::from(get(dir)?));
        }
        let path = dirs[&dir].join(name);
        let (size, modified, sidecar) = (r.varint()?, r.varint()?, r.varint()?);
        let date = r.text()?.to_owned();
        let mut owned = || get(r.varint()?).map(str::to_owned);
        let (make, model, lens, label) = (owned()?, owned()?, owned()?, owned()?);
        let iso = r.optional()?;
        let (aperture, exposure_time, focal_length) = (r.real()?, r.real()?, r.real()?);
        let gps = if version == VERSION { r.gps()? } else { None };
        let (rating, flag) = (r.byte()?, r.byte()? as i8);
        let n = usize::try_from(r.varint()?).ok()?;
        let mut keywords = Vec::with_capacity(n.min(64));
        for _ in 0..n {
            keywords.push(get(r.varint()?)?.to_owned());
        }
        let e = Entry {
            path: path.clone(),
            size,
            // Sans date, une photo d'avant la position GPS est relue (et sa position
            // avec elle).
            modified: if version == VERSION { modified } else { 0 },
            sidecar,
            date,
            make,
            model,
            lens,
            iso,
            aperture,
            exposure_time,
            focal_length,
            gps,
            rating,
            flag,
            label,
            keywords,
        };
        entries.push((path, e));
    }
    let collections: Vec<Collection> = serde_json::from_str(r.text()?).ok()?;
    // Écrites dans l'ordre des chemins : l'arbre se construit d'un bloc.
    let entries: BTreeMap<PathBuf, Entry> = entries.into_iter().collect();
    Some(Catalog { entries, collections })
}

/// Le catalogue est‑il au format binaire ?
pub(crate) fn is_binary(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn photo(dir: &str, name: &str) -> PathBuf {
        Path::new(dir).join(name)
    }

    fn sample() -> Catalog {
        let mut cat = Catalog::default();
        for (i, dir) in ["/photos/2024-06-01", "/photos/2024-06-02", "/photos/2024-06-01"]
            .iter()
            .enumerate()
        {
            cat.insert(Entry {
                path: photo(dir, &format!("IMG_{i}.ARW")),
                size: 25_000_000 + i as u64,
                modified: 1_700_000_000,
                sidecar: if i == 1 { 1_700_000_100 } else { 0 },
                date: format!("2024-06-0{}T10:00:00", i + 1),
                make: "Sony".into(),
                model: "ILCE-7M3".into(),
                lens: if i == 2 {
                    String::new()
                } else {
                    "FE 24-70mm F2.8 GM".into()
                },
                iso: (i != 2).then_some(100 << i),
                aperture: Some(2.8),
                exposure_time: (i == 0).then_some(1.0 / 250.0),
                focal_length: Some(35.0),
                gps: (i == 1).then_some([48.8584, 2.2945]),
                rating: i as u8 + 3,
                flag: [1, -1, 0][i],
                label: if i == 0 { "Rouge".into() } else { String::new() },
                keywords: vec!["Été".into(), format!("jour {i}")],
            });
        }
        cat.add_to_collection("Tirages", &[photo("/photos/2024-06-01", "IMG_0.ARW")])
            .unwrap();
        cat.set_collection(Collection::smart("Meilleures", "note:4").unwrap())
            .unwrap();
        cat
    }

    #[test]
    fn round_trip() {
        let cat = sample();
        let bytes = encode(&cat);
        assert!(is_binary(&bytes));
        assert_eq!(decode(&bytes).unwrap(), cat);
        assert_eq!(decode(&encode(&Catalog::default())).unwrap(), Catalog::default());
    }

    #[test]
    fn a_catalog_without_gps_is_read_and_its_photos_reread() {
        let cat = sample();
        let read = decode(&encode_as(&cat, WITHOUT_GPS)).unwrap();
        assert_eq!(read.len(), cat.len());
        assert_eq!(read.collections, cat.collections);
        for e in read.entries() {
            let was = cat.get(&e.path).unwrap();
            assert_eq!((e.gps, e.modified), (None, 0), "relue à la prochaine mise à jour");
            assert_eq!((e.rating, &e.keywords, &e.lens), (was.rating, &was.keywords, &was.lens));
        }
    }

    #[test]
    fn repeated_texts_are_written_once() {
        let bytes = encode(&sample());
        let count = |needle: &[u8]| bytes.windows(needle.len()).filter(|w| *w == needle).count();
        assert_eq!(count(b"FE 24-70mm F2.8 GM"), 1);
        assert_eq!(count(b"/photos/2024-06-02"), 1, "un dossier, une fois");
        assert_eq!(count("Été".as_bytes()), 1);
    }

    #[test]
    fn damaged_files_are_refused() {
        let bytes = encode(&sample());
        for cut in [0, 5, 9, bytes.len() / 2, bytes.len() - 1] {
            assert!(decode(&bytes[..cut]).is_none(), "tronqué à {cut} octets");
        }
        let mut other = bytes.clone();
        other[8] = 9;
        assert!(decode(&other).is_none(), "autre version");
        // Une taille démesurée n'alloue rien.
        let mut huge = MAGIC.to_vec();
        varint(&mut huge, VERSION);
        varint(&mut huge, u64::MAX >> 1);
        assert!(decode(&huge).is_none());
    }

    #[test]
    fn varints() {
        for v in [0, 1, 127, 128, 300, u64::from(u32::MAX), u64::MAX] {
            let mut out = Vec::new();
            varint(&mut out, v);
            assert_eq!(Reader { bytes: &out, at: 0 }.varint(), Some(v));
        }
    }
}
