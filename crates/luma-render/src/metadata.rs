//! Les métadonnées des exports : l'EXIF de la prise de vue (boîtier, objectif,
//! réglages, date), l'auteur et le copyright, la position GPS seulement si on la garde,
//! et un paquet XMP (créateur, droits, mots‑clés, note, libellé) dans les JPEG.
//!
//! L'EXIF est écrit ici, en TIFF petit‑boutiste : une poignée de balises, sans
//! bibliothèque.

use luma_project::xmp::Meta;
use luma_raw::{Exif, Gps};

/// Ce que l'export recopie et ajoute.
#[derive(Clone, Debug, PartialEq)]
pub struct MetadataOptions {
    /// L'EXIF de la prise de vue (boîtier, objectif, réglages, date).
    pub copy_exif: bool,
    /// La position GPS. Retirée par défaut : une photo partagée ne dit pas où l'on vit.
    pub keep_gps: bool,
    /// Les mots‑clés, la note et le libellé (paquet XMP des JPEG).
    pub keywords: bool,
    /// L'auteur et le copyright ; à défaut, ceux que le boîtier a inscrits.
    pub author: Option<String>,
    pub copyright: Option<String>,
}

impl Default for MetadataOptions {
    fn default() -> Self {
        MetadataOptions {
            copy_exif: true,
            keep_gps: false,
            keywords: true,
            author: None,
            copyright: None,
        }
    }
}

impl MetadataOptions {
    /// En clair, seulement ce qui diffère d'un export ordinaire : « sans EXIF, GPS
    /// gardé, auteur Maxime ».
    pub fn describe(&self) -> Option<String> {
        let mut parts = Vec::new();
        if !self.copy_exif {
            parts.push("sans EXIF".to_owned());
        }
        if self.keep_gps && self.copy_exif {
            parts.push("GPS gardé".to_owned());
        }
        if !self.keywords {
            parts.push("sans mots‑clés".to_owned());
        }
        if let Some(a) = &self.author {
            parts.push(format!("auteur {a}"));
        }
        if let Some(c) = &self.copyright {
            parts.push(format!("copyright {c}"));
        }
        (!parts.is_empty()).then(|| parts.join(", "))
    }
}

/// Ce que l'on sait d'une photo pour ses métadonnées.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PhotoMetadata {
    pub make: String,
    pub model: String,
    pub exif: Exif,
    pub meta: Meta,
}

/// Les blocs à incorporer au fichier exporté.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Embedded {
    /// L'EXIF en TIFF (sans l'en‑tête `Exif\0\0`) ; vide : aucun.
    pub exif: Vec<u8>,
    /// Le paquet XMP ; `None` : aucun.
    pub xmp: Option<String>,
}

impl Embedded {
    /// Les blocs d'une image de `width × height` pixels.
    pub fn new(photo: &PhotoMetadata, o: &MetadataOptions, width: usize, height: usize) -> Embedded {
        let author = o
            .author
            .clone()
            .or_else(|| photo.exif.artist.clone().filter(|_| o.copy_exif));
        let copyright = o
            .copyright
            .clone()
            .or_else(|| photo.exif.copyright.clone().filter(|_| o.copy_exif));
        let exif = exif_tiff(photo, o, author.as_deref(), copyright.as_deref(), width, height);
        let m = &photo.meta;
        let keywords: &[String] = if o.keywords { &m.keywords } else { &[] };
        let (rating, label) = if o.keywords {
            (m.rating, m.label.as_str())
        } else {
            (0, "")
        };
        let xmp = (author.is_some() || copyright.is_some() || !keywords.is_empty() || rating > 0 || !label.is_empty())
            .then(|| xmp_packet(author.as_deref(), copyright.as_deref(), keywords, rating, label));
        Embedded { exif, xmp }
    }
}

/// Une balise TIFF : numéro, type, nombre de valeurs, octets.
struct Tag(u16, u16, u32, Vec<u8>);

const ASCII: u16 = 2;
const SHORT: u16 = 3;
const LONG: u16 = 4;
const RATIONAL: u16 = 5;
const UNDEFINED: u16 = 7;
const BYTE: u16 = 1;

fn ascii(tag: u16, s: &str) -> Tag {
    let mut b = s.trim().as_bytes().to_vec();
    b.push(0);
    Tag(tag, ASCII, b.len() as u32, b)
}

fn short(tag: u16, v: u16) -> Tag {
    Tag(tag, SHORT, 1, v.to_le_bytes().to_vec())
}

fn long(tag: u16, v: u32) -> Tag {
    Tag(tag, LONG, 1, v.to_le_bytes().to_vec())
}

fn rationals(tag: u16, v: &[(u32, u32)]) -> Tag {
    let b = v
        .iter()
        .flat_map(|(n, d)| [n.to_le_bytes(), d.to_le_bytes()].concat())
        .collect();
    Tag(tag, RATIONAL, v.len() as u32, b)
}

/// Un nombre en fraction de dénominateur `d`.
fn frac(v: f64, d: u32) -> (u32, u32) {
    ((v.abs() * f64::from(d)).round().min(f64::from(u32::MAX)) as u32, d)
}

/// Un temps de pose : `1/250` plutôt que `0,004`.
fn exposure(t: f32) -> (u32, u32) {
    let t = f64::from(t);
    if t > 0.0 && t < 1.0 && ((1.0 / t) - (1.0 / t).round()).abs() < 0.05 {
        (1, (1.0 / t).round() as u32)
    } else {
        frac(t, 1000)
    }
}

/// Degrés décimaux en degrés, minutes, secondes.
fn dms(v: f64) -> [(u32, u32); 3] {
    let v = v.abs();
    let d = v.floor();
    let m = ((v - d) * 60.0).floor();
    let s = (v - d - m / 60.0) * 3600.0;
    [(d as u32, 1), (m as u32, 1), frac(s, 1000)]
}

/// La taille d'un IFD et de ses valeurs longues.
fn ifd_size(tags: &[Tag]) -> u32 {
    let data: usize = tags
        .iter()
        .map(|t| {
            if t.3.len() > 4 {
                t.3.len().next_multiple_of(2)
            } else {
                0
            }
        })
        .sum();
    (2 + 12 * tags.len() + 4 + data) as u32
}

/// Écrit un IFD à `offset` (ses valeurs longues juste derrière).
fn write_ifd(out: &mut Vec<u8>, mut tags: Vec<Tag>, offset: u32) {
    tags.sort_by_key(|t| t.0);
    let mut data_at = offset + 2 + 12 * tags.len() as u32 + 4;
    let mut data = Vec::new();
    out.extend_from_slice(&(tags.len() as u16).to_le_bytes());
    for t in &tags {
        out.extend_from_slice(&t.0.to_le_bytes());
        out.extend_from_slice(&t.1.to_le_bytes());
        out.extend_from_slice(&t.2.to_le_bytes());
        if t.3.len() <= 4 {
            let mut v = t.3.clone();
            v.resize(4, 0);
            out.extend_from_slice(&v);
        } else {
            out.extend_from_slice(&data_at.to_le_bytes());
            data.extend_from_slice(&t.3);
            if t.3.len() % 2 == 1 {
                data.push(0);
            }
            data_at += t.3.len().next_multiple_of(2) as u32;
        }
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&data);
}

/// L'EXIF de l'export, en TIFF : vide si rien n'est à écrire.
fn exif_tiff(
    p: &PhotoMetadata,
    o: &MetadataOptions,
    author: Option<&str>,
    copyright: Option<&str>,
    width: usize,
    height: usize,
) -> Vec<u8> {
    let e = &p.exif;
    let mut ifd0 = vec![short(0x0112, 1), ascii(0x0131, "LÜMA")];
    if let Some(a) = author {
        ifd0.push(ascii(0x013B, a));
    }
    if let Some(c) = copyright {
        ifd0.push(ascii(0x8298, c));
    }
    let mut sub = Vec::new();
    let mut gps = Vec::new();
    if o.copy_exif {
        for (tag, v) in [(0x010F, &p.make), (0x0110, &p.model)] {
            if !v.trim().is_empty() {
                ifd0.push(ascii(tag, v));
            }
        }
        sub.push(Tag(0x9000, UNDEFINED, 4, b"0232".to_vec()));
        sub.push(long(0xA002, width as u32));
        sub.push(long(0xA003, height as u32));
        if let Some(t) = e.exposure_time.filter(|t| *t > 0.0) {
            sub.push(rationals(0x829A, &[exposure(t)]));
        }
        if let Some(f) = e.aperture.filter(|f| *f > 0.0) {
            sub.push(rationals(0x829D, &[frac(f64::from(f), 10)]));
        }
        if let Some(iso) = e.iso {
            sub.push(short(0x8827, iso.min(u32::from(u16::MAX)) as u16));
        }
        if let Some(d) = e.date.as_deref().filter(|d| !d.trim().is_empty()) {
            sub.push(ascii(0x9003, d));
        }
        if let Some(f) = e.focal_length.filter(|f| *f > 0.0) {
            sub.push(rationals(0x920A, &[frac(f64::from(f), 10)]));
        }
        if let Some(l) = e.lens.as_deref().filter(|l| !l.trim().is_empty()) {
            sub.push(ascii(0xA434, l));
        }
        if o.keep_gps
            && let Some(Gps {
                latitude,
                longitude,
                altitude,
            }) = e.gps
        {
            gps.push(Tag(0x0000, BYTE, 4, vec![2, 3, 0, 0]));
            gps.push(ascii(0x0001, if latitude < 0.0 { "S" } else { "N" }));
            gps.push(rationals(0x0002, &dms(latitude)));
            gps.push(ascii(0x0003, if longitude < 0.0 { "W" } else { "E" }));
            gps.push(rationals(0x0004, &dms(longitude)));
            if let Some(a) = altitude {
                gps.push(Tag(0x0005, BYTE, 1, vec![u8::from(a < 0.0)]));
                gps.push(rationals(0x0006, &[frac(a, 100)]));
            }
        }
    }
    if !o.copy_exif && author.is_none() && copyright.is_none() {
        return Vec::new();
    }
    // Les pointeurs vers les sous‑IFD, puis les décalages de chacun.
    if !sub.is_empty() {
        ifd0.push(long(0x8769, 0));
    }
    if !gps.is_empty() {
        ifd0.push(long(0x8825, 0));
    }
    let sub_at = 8 + ifd_size(&ifd0);
    let gps_at = sub_at + if sub.is_empty() { 0 } else { ifd_size(&sub) };
    for t in &mut ifd0 {
        match t.0 {
            0x8769 => t.3 = sub_at.to_le_bytes().to_vec(),
            0x8825 => t.3 = gps_at.to_le_bytes().to_vec(),
            _ => {}
        }
    }
    let mut out = b"II*\0".to_vec();
    out.extend_from_slice(&8u32.to_le_bytes());
    write_ifd(&mut out, ifd0, 8);
    if !sub.is_empty() {
        write_ifd(&mut out, sub, sub_at);
    }
    if !gps.is_empty() {
        write_ifd(&mut out, gps, gps_at);
    }
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Le paquet XMP : créateur, droits, mots‑clés, note et libellé.
fn xmp_packet(author: Option<&str>, copyright: Option<&str>, keywords: &[String], rating: u8, label: &str) -> String {
    let mut x = String::from(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"LÜMA\">\n \
         <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  \
         <rdf:Description rdf:about=\"\"\n    \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n    \
         xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"",
    );
    if rating > 0 {
        x += &format!("\n    xmp:Rating=\"{}\"", rating.min(5));
    }
    if !label.trim().is_empty() {
        x += &format!("\n    xmp:Label=\"{}\"", escape(label.trim()));
    }
    x += ">\n";
    if let Some(a) = author {
        x += &format!(
            "   <dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator>\n",
            escape(a.trim())
        );
    }
    if let Some(c) = copyright {
        x += &format!(
            "   <dc:rights><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:rights>\n",
            escape(c.trim())
        );
    }
    if !keywords.is_empty() {
        x += "   <dc:subject><rdf:Bag>";
        for k in keywords {
            x += &format!("<rdf:li>{}</rdf:li>", escape(k.trim()));
        }
        x += "</rdf:Bag></dc:subject>\n";
    }
    x += "  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>";
    x
}

/// L'identifiant du segment XMP des JPEG, fixé par le format.
const XMP_ID: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// Ajoute le paquet XMP à un JPEG, après ses premiers segments d'application.
pub fn insert_jpeg_xmp(jpeg: &mut Vec<u8>, xmp: &str) {
    let len = 2 + XMP_ID.len() + xmp.len();
    if len > 0xFFFF || jpeg.len() < 4 || jpeg[..2] != [0xFF, 0xD8] {
        return;
    }
    let mut at = 2;
    while at + 4 <= jpeg.len() && jpeg[at] == 0xFF && (0xE0..=0xEF).contains(&jpeg[at + 1]) {
        at += 2 + usize::from(u16::from_be_bytes([jpeg[at + 2], jpeg[at + 3]]));
    }
    let mut seg = vec![0xFF, 0xE1];
    seg.extend_from_slice(&(len as u16).to_be_bytes());
    seg.extend_from_slice(XMP_ID);
    seg.extend_from_slice(xmp.as_bytes());
    jpeg.splice(at..at, seg);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Un IFD lu : (numéro, type, nombre, octets) de chaque balise.
    pub(crate) type Ifd = Vec<(u16, u16, u32, Vec<u8>)>;

    /// Les balises d'un IFD de ce TIFF : numéro → (type, nombre, octets de la valeur).
    pub(crate) fn read_ifd(tiff: &[u8], offset: usize) -> Ifd {
        let u16_at = |i: usize| u16::from_le_bytes([tiff[i], tiff[i + 1]]);
        let u32_at = |i: usize| u32::from_le_bytes(tiff[i..i + 4].try_into().unwrap());
        let n = usize::from(u16_at(offset));
        (0..n)
            .map(|k| {
                let e = offset + 2 + 12 * k;
                let (tag, kind, count) = (u16_at(e), u16_at(e + 2), u32_at(e + 4));
                let size = match kind {
                    1 | 2 | 7 => 1,
                    3 => 2,
                    4 => 4,
                    _ => 8,
                } * count as usize;
                let at = if size <= 4 { e + 8 } else { u32_at(e + 8) as usize };
                (tag, kind, count, tiff[at..at + size].to_vec())
            })
            .collect()
    }

    pub(crate) fn text(v: &[u8]) -> String {
        String::from_utf8_lossy(v.strip_suffix(&[0]).unwrap_or(v)).into_owned()
    }

    pub(crate) fn photo() -> PhotoMetadata {
        PhotoMetadata {
            make: "SONY".into(),
            model: "ILCE-7M3".into(),
            exif: Exif {
                iso: Some(400),
                exposure_time: Some(1.0 / 250.0),
                aperture: Some(4.0),
                focal_length: Some(32.0),
                lens: Some("FE 24-105mm F4 G OSS".into()),
                date: Some("2026:03:09 08:05:01".into()),
                artist: Some("Boîtier".into()),
                copyright: None,
                gps: Some(Gps {
                    latitude: 48.8584,
                    longitude: -2.2945,
                    altitude: Some(35.5),
                }),
            },
            meta: Meta {
                rating: 4,
                label: "Rouge".into(),
                keywords: vec!["mer".into(), "Bretagne & été".into()],
                ..Meta::default()
            },
        }
    }

    fn tags(e: &Embedded) -> (Ifd, Ifd, Ifd) {
        let t = &e.exif;
        assert_eq!(&t[..4], b"II*\0");
        let ifd0 = read_ifd(t, 8);
        let ptr = |tag| {
            ifd0.iter()
                .find(|x| x.0 == tag)
                .map(|x| u32::from_le_bytes(x.3[..4].try_into().unwrap()) as usize)
        };
        let sub = ptr(0x8769).map(|o| read_ifd(t, o)).unwrap_or_default();
        let gps = ptr(0x8825).map(|o| read_ifd(t, o)).unwrap_or_default();
        (ifd0, sub, gps)
    }

    fn get(ifd: &[(u16, u16, u32, Vec<u8>)], tag: u16) -> Option<&Vec<u8>> {
        ifd.iter().find(|x| x.0 == tag).map(|x| &x.3)
    }

    #[test]
    fn exif_is_copied_without_gps_by_default() {
        let e = Embedded::new(&photo(), &MetadataOptions::default(), 1600, 1067);
        let (ifd0, sub, gps) = tags(&e);
        assert_eq!(text(get(&ifd0, 0x010F).unwrap()), "SONY");
        assert_eq!(text(get(&ifd0, 0x0110).unwrap()), "ILCE-7M3");
        assert_eq!(text(get(&ifd0, 0x0131).unwrap()), "LÜMA");
        // L'auteur du boîtier, faute d'un auteur donné.
        assert_eq!(text(get(&ifd0, 0x013B).unwrap()), "Boîtier");
        assert_eq!(get(&ifd0, 0x0112).unwrap()[..2], 1u16.to_le_bytes());
        assert_eq!(
            get(&sub, 0x829A).unwrap()[..],
            [1u32.to_le_bytes(), 250u32.to_le_bytes()].concat()[..]
        );
        assert_eq!(
            get(&sub, 0x829D).unwrap()[..],
            [40u32.to_le_bytes(), 10u32.to_le_bytes()].concat()[..]
        );
        assert_eq!(get(&sub, 0x8827).unwrap()[..2], 400u16.to_le_bytes());
        assert_eq!(text(get(&sub, 0x9003).unwrap()), "2026:03:09 08:05:01");
        assert_eq!(text(get(&sub, 0xA434).unwrap()), "FE 24-105mm F4 G OSS");
        assert_eq!(get(&sub, 0xA002).unwrap()[..], 1600u32.to_le_bytes());
        // Les balises sont rangées par numéro, et la position n'est pas là.
        assert!(ifd0.windows(2).all(|w| w[0].0 < w[1].0) && sub.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(gps.is_empty() && get(&ifd0, 0x8825).is_none());
    }

    #[test]
    fn gps_is_kept_on_request() {
        let o = MetadataOptions {
            keep_gps: true,
            ..MetadataOptions::default()
        };
        let (_, _, gps) = tags(&Embedded::new(&photo(), &o, 100, 100));
        assert_eq!(text(get(&gps, 0x0001).unwrap()), "N");
        assert_eq!(text(get(&gps, 0x0003).unwrap()), "W");
        let r = |v: &[u8], i: usize| {
            let n = u32::from_le_bytes(v[i * 8..i * 8 + 4].try_into().unwrap()) as f64;
            let d = u32::from_le_bytes(v[i * 8 + 4..i * 8 + 8].try_into().unwrap()) as f64;
            n / d
        };
        let lat = get(&gps, 0x0002).unwrap();
        assert!((r(lat, 0) + r(lat, 1) / 60.0 + r(lat, 2) / 3600.0 - 48.8584).abs() < 1e-6);
        let lon = get(&gps, 0x0004).unwrap();
        assert!((r(lon, 0) + r(lon, 1) / 60.0 + r(lon, 2) / 3600.0 - 2.2945).abs() < 1e-6);
        assert!((r(get(&gps, 0x0006).unwrap(), 0) - 35.5).abs() < 1e-9);
    }

    #[test]
    fn author_copyright_and_no_exif() {
        let o = MetadataOptions {
            copy_exif: false,
            keep_gps: true,
            author: Some("Maxime Lestage".into()),
            copyright: Some("© 2026 Maxime Lestage".into()),
            ..MetadataOptions::default()
        };
        let e = Embedded::new(&photo(), &o, 100, 100);
        let (ifd0, sub, gps) = tags(&e);
        assert_eq!(text(get(&ifd0, 0x013B).unwrap()), "Maxime Lestage");
        assert_eq!(text(get(&ifd0, 0x8298).unwrap()), "© 2026 Maxime Lestage");
        // Sans EXIF : ni boîtier, ni réglages, ni position, même demandée.
        assert!(get(&ifd0, 0x010F).is_none() && sub.is_empty() && gps.is_empty());
        let x = e.xmp.unwrap();
        assert!(x.contains("<rdf:li>Maxime Lestage</rdf:li>") && x.contains("© 2026 Maxime Lestage"));
        assert!(x.contains("<rdf:li>Bretagne &amp; été</rdf:li>") && x.contains("xmp:Rating=\"4\""));
        // Rien du tout : ni EXIF, ni XMP.
        let none = MetadataOptions {
            copy_exif: false,
            keywords: false,
            ..MetadataOptions::default()
        };
        assert_eq!(Embedded::new(&photo(), &none, 10, 10), Embedded::default());
    }

    #[test]
    fn xmp_goes_after_the_first_segments() {
        // SOI, APP0 (4 octets), puis l'image.
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0xAA, 0xBB, 0xFF, 0xDB, 0x00];
        insert_jpeg_xmp(&mut jpeg, "<x/>");
        assert_eq!(&jpeg[..8], &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0xAA, 0xBB]);
        assert_eq!(&jpeg[8..10], &[0xFF, 0xE1]);
        let len = u16::from_be_bytes([jpeg[10], jpeg[11]]) as usize;
        assert_eq!(len, 2 + XMP_ID.len() + 4);
        assert_eq!(&jpeg[12 + XMP_ID.len()..12 + XMP_ID.len() + 4], b"<x/>");
        assert_eq!(&jpeg[jpeg.len() - 3..], &[0xFF, 0xDB, 0x00]);
        // Ce qui n'est pas un JPEG ne bouge pas.
        let mut other = vec![1, 2, 3, 4];
        insert_jpeg_xmp(&mut other, "<x/>");
        assert_eq!(other, [1, 2, 3, 4]);
    }

    #[test]
    fn exposure_times_read_as_fractions() {
        assert_eq!(exposure(1.0 / 250.0), (1, 250));
        assert_eq!(exposure(1.0 / 3.0), (1, 3));
        assert_eq!(exposure(2.5), (2500, 1000));
        assert_eq!(exposure(0.3), (300, 1000));
        let d = dms(48.8584);
        assert_eq!((d[0], d[1]), ((48, 1), (51, 1)));
    }
}
