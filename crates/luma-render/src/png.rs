//! L'écriture des PNG, sur tous les cœurs : l'image est coupée en bandes de lignes,
//! chaque bande filtrée (pour chaque ligne, le meilleur des cinq filtres du format) et
//! compressée de son côté, puis les bandes mises bout à bout en un seul flux : un
//! PNG ordinaire, que tout lecteur ouvre, aux mêmes pixels.

use miniz_oxide::deflate::core::{
    CompressorOxide, TDEFLFlush, TDEFLStatus, compress_to_output, create_comp_flags_from_zip_params,
};
use rayon::prelude::*;

/// Les lignes d'une bande : assez pour que la compression y trouve ses répétitions,
/// assez peu pour occuper tous les cœurs.
const BAND: usize = 32;

/// Le niveau de compression : le plus rapide, comme avant (« rapide » du codeur
/// d'images), le poids voisin.
const LEVEL: i32 = 1;

/// Le PNG d'une image RVB, en mémoire (voir [`write`]).
pub fn encode(bytes: &[u8], width: usize, height: usize, sixteen: bool, icc: &[u8], exif: &[u8]) -> Vec<u8> {
    let mut png = Vec::new();
    write(&mut png, bytes, width, height, sixteen, icc, exif, None).expect("écrire en mémoire");
    png
}

/// Écrit le PNG d'une image RVB (`bytes` : 8 bits, ou 16 bits dans l'ordre de la
/// machine), avec son profil ICC, son EXIF et la résolution d'un tirage s'il y en a.
/// Chaque bande devient son propre bloc IDAT, somme comprise, dans son fil : rien n'est
/// recopié d'un bout à l'autre (les blocs IDAT se suivent comme un seul flux).
#[allow(clippy::too_many_arguments)]
pub fn write(
    out: &mut impl std::io::Write,
    bytes: &[u8],
    width: usize,
    height: usize,
    sixteen: bool,
    icc: &[u8],
    exif: &[u8],
    dpi: Option<u32>,
) -> std::io::Result<()> {
    let bpp = if sixteen { 6 } else { 3 };
    let stride = width * bpp;
    // Le PNG range les valeurs de 16 bits poids fort d'abord.
    let row = |y: usize, out: &mut [u8]| {
        let src = &bytes[y * stride..(y + 1) * stride];
        if sixteen {
            for (o, s) in out.as_chunks_mut::<2>().0.iter_mut().zip(src.as_chunks::<2>().0) {
                *o = u16::from_ne_bytes(*s).to_be_bytes();
            }
        } else {
            out.copy_from_slice(src);
        }
    };
    let bands: Vec<(Vec<u8>, u32, usize)> = (0..height.div_ceil(BAND))
        .into_par_iter()
        .map(|b| {
            let (y0, y1) = (b * BAND, ((b + 1) * BAND).min(height));
            let mut filtered = Vec::with_capacity((y1 - y0) * (stride + 1));
            let (mut prev, mut cur) = (vec![0u8; stride], vec![0u8; stride]);
            if y0 > 0 {
                row(y0 - 1, &mut prev);
            }
            for y in y0..y1 {
                row(y, &mut cur);
                filter_row(&cur, &prev, bpp, &mut filtered);
                std::mem::swap(&mut prev, &mut cur);
            }
            let mut idat = Vec::new();
            chunk(&mut idat, b"IDAT", &deflate(&filtered, y1 == height));
            (idat, adler32(&filtered), filtered.len())
        })
        .collect();
    let mut adler = 1u32;
    for (_, a, len) in &bands {
        adler = adler32_combine(adler, *a, *len);
    }

    let mut head = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend((width as u32).to_be_bytes());
    ihdr.extend((height as u32).to_be_bytes());
    ihdr.extend([if sixteen { 16 } else { 8 }, 2, 0, 0, 0]);
    chunk(&mut head, b"IHDR", &ihdr);
    if let Some(dpi) = dpi {
        let per_metre = (f64::from(dpi) / 0.0254).round() as u32;
        let mut phys = Vec::with_capacity(9);
        phys.extend(per_metre.to_be_bytes());
        phys.extend(per_metre.to_be_bytes());
        phys.push(1);
        chunk(&mut head, b"pHYs", &phys);
    }
    if !icc.is_empty() {
        let mut iccp = b"ICC Profile\0\0".to_vec();
        iccp.extend(miniz_oxide::deflate::compress_to_vec_zlib(icc, 6));
        chunk(&mut head, b"iCCP", &iccp);
    }
    if !exif.is_empty() {
        chunk(&mut head, b"eXIf", exif);
    }
    // Le flux zlib : son en‑tête, les bandes, la somme de contrôle de l'ensemble.
    chunk(&mut head, b"IDAT", &[0x78, 0x01]);
    out.write_all(&head)?;
    for (idat, _, _) in &bands {
        out.write_all(idat)?;
    }
    let mut tail = Vec::new();
    let mut last = Vec::new();
    if bands.is_empty() {
        last.extend(deflate(&[], true));
    }
    last.extend(adler.to_be_bytes());
    chunk(&mut tail, b"IDAT", &last);
    chunk(&mut tail, b"IEND", &[]);
    out.write_all(&tail)
}

/// Une ligne filtrée, son octet de filtre en tête : des cinq filtres du format, celui
/// dont la somme des écarts (en octets signés) est la plus petite.
fn filter_row(cur: &[u8], prev: &[u8], bpp: usize, out: &mut Vec<u8>) {
    let n = cur.len();
    let left = |i: usize| if i >= bpp { cur[i - bpp] } else { 0 };
    let up_left = |i: usize| if i >= bpp { prev[i - bpp] } else { 0 };
    let paeth = |a: u8, b: u8, c: u8| {
        let p = i16::from(a) + i16::from(b) - i16::from(c);
        let (pa, pb, pc) = (
            (p - i16::from(a)).abs(),
            (p - i16::from(b)).abs(),
            (p - i16::from(c)).abs(),
        );
        if pa <= pb && pa <= pc {
            a
        } else if pb <= pc {
            b
        } else {
            c
        }
    };
    let filtered = |kind: u8, i: usize| -> u8 {
        let x = cur[i];
        match kind {
            0 => x,
            1 => x.wrapping_sub(left(i)),
            2 => x.wrapping_sub(prev[i]),
            3 => x.wrapping_sub(((u16::from(left(i)) + u16::from(prev[i])) / 2) as u8),
            _ => x.wrapping_sub(paeth(left(i), prev[i], up_left(i))),
        }
    };
    let cost = |kind: u8| -> u64 {
        (0..n)
            .map(|i| u64::from((filtered(kind, i) as i8).unsigned_abs()))
            .sum()
    };
    let best = (0..5u8).min_by_key(|&k| cost(k)).unwrap_or(0);
    out.push(best);
    out.extend((0..n).map(|i| filtered(best, i)));
}

/// `data` compressée en blocs deflate bruts : terminés par une synchronisation (un
/// autre morceau suivra), ou par le bloc final pour la dernière bande.
fn deflate(data: &[u8], last: bool) -> Vec<u8> {
    let mut c = CompressorOxide::new(create_comp_flags_from_zip_params(LEVEL, -15, 0));
    let mut out = Vec::with_capacity(data.len() / 2 + 64);
    let flush = if last { TDEFLFlush::Finish } else { TDEFLFlush::Sync };
    let (status, _) = compress_to_output(&mut c, data, flush, |chunk| {
        out.extend_from_slice(chunk);
        true
    });
    debug_assert!(matches!(status, TDEFLStatus::Okay | TDEFLStatus::Done));
    out
}

const ADLER: u32 = 65_521;

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    // Par paquets de 5552 octets : les sommes ne débordent pas avant le modulo.
    for block in data.chunks(5552) {
        for &d in block {
            a += u32::from(d);
            b += a;
        }
        a %= ADLER;
        b %= ADLER;
    }
    (b << 16) | a
}

/// La somme de contrôle de `A` puis `B`, d'après celles de `A` et de `B` (`len` :
/// la longueur de `B`), comme le fait zlib.
fn adler32_combine(a: u32, b: u32, len: usize) -> u32 {
    let rem = (len % ADLER as usize) as u64;
    let m = u64::from(ADLER);
    let (a1, b1) = (u64::from(a & 0xffff), u64::from(a >> 16));
    let (a2, b2) = (u64::from(b & 0xffff), u64::from(b >> 16));
    let sum1 = (a1 + a2 + m - 1) % m;
    let sum2 = (b1 + b2 + rem * a1 % m + m - rem) % m;
    ((sum2 as u32) << 16) | sum1 as u32
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend((data.len() as u32).to_be_bytes());
    let start = png.len();
    png.extend(kind);
    png.extend(data);
    let crc = crc32(&png[start..]);
    png.extend(crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        std::array::from_fn(|n| {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            c
        })
    });
    !data
        .iter()
        .fold(!0u32, |c, &b| table[((c ^ u32::from(b)) & 0xff) as usize] ^ (c >> 8))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relu par le décodeur d'images : les mêmes octets, en 8 et en 16 bits, sur des
    /// tailles qui ne tombent pas juste (une ligne, une colonne, une bande et demie),
    /// avec profil et EXIF ; la somme de contrôle par morceaux est celle d'un seul tenant.
    #[test]
    fn a_png_reads_back_with_the_same_pixels() {
        for (w, h) in [(1, 1), (1, 40), (37, 1), (53, 49), (130, 70)] {
            for sixteen in [false, true] {
                let bpp = if sixteen { 6 } else { 3 };
                let bytes: Vec<u8> = (0..w * h * bpp)
                    .map(|i| ((i * 7919 + (i / bpp) % 13 * 3) % 251) as u8)
                    .collect();
                let png = encode(&bytes, w, h, sixteen, b"profil", b"MM\0*exif");
                let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
                assert_eq!((img.width() as usize, img.height() as usize), (w, h));
                if sixteen {
                    let back: Vec<u8> = img.to_rgb16().into_raw().iter().flat_map(|v| v.to_ne_bytes()).collect();
                    assert_eq!(back, bytes, "{w} × {h}, 16 bits");
                } else {
                    assert_eq!(img.to_rgb8().into_raw(), bytes, "{w} × {h}, 8 bits");
                }
            }
        }
        // Le CRC des blocs, sur la valeur de référence du format.
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        let data: Vec<u8> = (0..100_000u32).map(|i| (i * 31 % 256) as u8).collect();
        let (a, b) = data.split_at(33_333);
        assert_eq!(adler32_combine(adler32(a), adler32(b), b.len()), adler32(&data));
    }
}
