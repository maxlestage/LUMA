//! L'encodeur JPEG des exports, sur tous les cœurs : un JPEG de base (4:4:4, tables
//! standard), octet pour octet celui de l'encodeur de la bibliothèque `image`, que LÜMA
//! utilisait avant, mais calculé par lignes de blocs en parallèle.
//!
//! Chaque ligne de blocs de 8 pixels (couleur, DCT, quantification, codage de Huffman)
//! est codée à part dans son propre flux de bits ; la prédiction du coefficient continu
//! d'une ligne part du dernier bloc de la précédente, calculé d'abord. Les flux sont
//! ensuite mis bout à bout, avec l'échappement des octets 0xFF.
//!
//! La DCT entière est celle de jfdctint.c, du logiciel de l'Independent JPEG Group
//! (libjpeg 9a), telle que portée dans la bibliothèque `image` ; ce logiciel est fondé en
//! partie sur le travail de l'Independent JPEG Group.

use rayon::prelude::*;

const SOF0: u8 = 0xC0;
const DHT: u8 = 0xC4;
const SOI: u8 = 0xD8;
const EOI: u8 = 0xD9;
const SOS: u8 = 0xDA;
const DQT: u8 = 0xDB;
const APP0: u8 = 0xE0;
const APP1: u8 = 0xE1;
const APP2: u8 = 0xE2;

#[rustfmt::skip]
const STD_LUMA_QTABLE: [u8; 64] = [
    16, 11, 10, 16,  24,  40,  51,  61,
    12, 12, 14, 19,  26,  58,  60,  55,
    14, 13, 16, 24,  40,  57,  69,  56,
    14, 17, 22, 29,  51,  87,  80,  62,
    18, 22, 37, 56,  68, 109, 103,  77,
    24, 35, 55, 64,  81, 104, 113,  92,
    49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103,  99,
];

#[rustfmt::skip]
const STD_CHROMA_QTABLE: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99,
    18, 21, 26, 66, 99, 99, 99, 99,
    24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
];

const LUMA_DC_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const LUMA_DC_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const CHROMA_DC_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const CHROMA_DC_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const LUMA_AC_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7D];
#[rustfmt::skip]
const LUMA_AC_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
    0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
    0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
    0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];
const CHROMA_AC_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
#[rustfmt::skip]
const CHROMA_AC_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0,
    0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26,
    0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5,
    0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3,
    0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA,
    0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];

/// L'ordre en zigzag des coefficients.
#[rustfmt::skip]
const UNZIGZAG: [u8; 64] = [
     0,  1,  8, 16,  9,  2,  3, 10,
    17, 24, 32, 25, 18, 11,  4,  5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13,  6,  7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63,
];

/// La table de Huffman d'un symbole : (longueur, code), selon l'annexe C de la norme.
type Huff = [(u8, u16); 256];

fn huffman(bits: &[u8; 16], values: &[u8]) -> Huff {
    let mut sizes = Vec::new();
    for (i, &n) in bits.iter().enumerate() {
        sizes.extend(std::iter::repeat_n(i as u8 + 1, usize::from(n)));
    }
    let mut lut = [(17u8, 0u16); 256];
    let mut code = 0u16;
    let mut size = sizes.first().copied().unwrap_or(0);
    for (k, &s) in sizes.iter().enumerate() {
        if s != size {
            code <<= s - size;
            size = s;
        }
        lut[usize::from(values[k])] = (s, code);
        code = code.wrapping_add(1);
    }
    lut
}

/// Les tables de quantification d'une qualité (1 à 100), par la formule de libjpeg.
fn qtables(quality: u8) -> [[u8; 64]; 2] {
    let q = u32::from(quality.clamp(1, 100));
    let scale = if q < 50 { 5000 / q } else { 200 - q * 2 };
    [STD_LUMA_QTABLE, STD_CHROMA_QTABLE].map(|t| t.map(|v| ((u32::from(v) * scale + 50) / 100).clamp(1, 255) as u8))
}

/// Un flux de bits sans échappement (ajouté à l'assemblage).
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    acc: u32,
    n: u8,
}

impl Bits {
    #[inline]
    fn put(&mut self, bits: u16, size: u8) {
        if size == 0 {
            return;
        }
        self.n += size;
        self.acc |= u32::from(bits) << (32 - u32::from(self.n));
        while self.n >= 8 {
            self.bytes.push((self.acc >> 24) as u8);
            self.n -= 8;
            self.acc <<= 8;
        }
    }

    #[inline]
    fn huff(&mut self, symbol: u8, table: &Huff) {
        let (size, code) = table[usize::from(symbol)];
        self.put(code, size);
    }
}

/// Le flux final : chaque octet 0xFF est suivi d'un 0x00.
struct Stuffed {
    out: Vec<u8>,
    acc: u32,
    n: u8,
}

impl Stuffed {
    #[inline]
    fn put(&mut self, bits: u16, size: u8) {
        if size == 0 {
            return;
        }
        self.n += size;
        self.acc |= u32::from(bits) << (32 - u32::from(self.n));
        while self.n >= 8 {
            let byte = (self.acc >> 24) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0);
            }
            self.n -= 8;
            self.acc <<= 8;
        }
    }

    fn append(&mut self, b: &Bits) {
        for &byte in &b.bytes {
            self.put(u16::from(byte), 8);
        }
        if b.n > 0 {
            self.put((b.acc >> (32 - u32::from(b.n))) as u16, b.n);
        }
    }
}

/// La taille et les bits d'un coefficient (catégorie de magnitude, puis valeur).
#[inline]
fn coefficient(c: i32) -> (u8, u16) {
    let num_bits = (32 - c.unsigned_abs().leading_zeros()) as u8;
    let mask = ((1u32 << num_bits) - 1) as u16;
    let val = if c < 0 { (c - 1) as u16 & mask } else { c as u16 & mask };
    (num_bits, val)
}

/// Code un bloc quantifié ; rend son coefficient continu, la prédiction du suivant.
fn write_block(w: &mut Bits, block: &[i32; 64], prev: i32, dc: &Huff, ac: &Huff) -> i32 {
    let (size, value) = coefficient(block[0] - prev);
    w.huff(size, dc);
    w.put(value, size);
    let mut zeros = 0u8;
    for &k in &UNZIGZAG[1..] {
        let c = block[usize::from(k)];
        if c == 0 {
            zeros += 1;
        } else {
            while zeros > 15 {
                w.huff(0xF0, ac);
                zeros -= 16;
            }
            let (size, value) = coefficient(c);
            w.huff((zeros << 4) | size, ac);
            w.put(value, size);
            zeros = 0;
        }
    }
    if block[usize::from(UNZIGZAG[63])] == 0 {
        w.huff(0x00, ac);
    }
    block[0]
}

const CONST_BITS: i32 = 13;
const PASS1_BITS: i32 = 2;
const FIX_0_298631336: i32 = 2446;
const FIX_0_390180644: i32 = 3196;
const FIX_0_541196100: i32 = 4433;
const FIX_0_765366865: i32 = 6270;
const FIX_0_899976223: i32 = 7373;
const FIX_1_175875602: i32 = 9633;
const FIX_1_501321110: i32 = 12_299;
const FIX_1_847759065: i32 = 15_137;
const FIX_1_961570560: i32 = 16_069;
const FIX_2_053119869: i32 = 16_819;
const FIX_2_562915447: i32 = 20_995;
const FIX_3_072711026: i32 = 25_172;

/// La DCT entière d'un bloc (coefficients multipliés par 8), telle que l'encodeur de
/// `image` la calcule : même arrondi dans la partie impaire des colonnes, pour des
/// fichiers identiques.
fn fdct(s: &[u8; 64], c: &mut [i32; 64]) {
    for y in 0..8 {
        let y0 = y * 8;
        let p = |i: usize| i32::from(s[y0 + i]);
        let (t0, t1, t2, t3) = (p(0) + p(7), p(1) + p(6), p(2) + p(5), p(3) + p(4));
        let (t10, t12, t11, t13) = (t0 + t3, t0 - t3, t1 + t2, t1 - t2);
        let (t0, t1, t2, t3) = (p(0) - p(7), p(1) - p(6), p(2) - p(5), p(3) - p(4));
        c[y0] = (t10 + t11 - 8 * 128) << PASS1_BITS;
        c[y0 + 4] = (t10 - t11) << PASS1_BITS;
        let z1 = (t12 + t13) * FIX_0_541196100 + (1 << (CONST_BITS - PASS1_BITS - 1));
        c[y0 + 2] = (z1 + t12 * FIX_0_765366865) >> (CONST_BITS - PASS1_BITS);
        c[y0 + 6] = (z1 - t13 * FIX_1_847759065) >> (CONST_BITS - PASS1_BITS);
        let (t12, t13) = (t0 + t2, t1 + t3);
        let z1 = (t12 + t13) * FIX_1_175875602 + (1 << (CONST_BITS - PASS1_BITS - 1));
        let t12 = t12 * -FIX_0_390180644 + z1;
        let t13 = t13 * -FIX_1_961570560 + z1;
        let z1 = (t0 + t3) * -FIX_0_899976223;
        let a0 = t0 * FIX_1_501321110 + z1 + t12;
        let a3 = t3 * FIX_0_298631336 + z1 + t13;
        let z1 = (t1 + t2) * -FIX_2_562915447;
        let a1 = t1 * FIX_3_072711026 + z1 + t13;
        let a2 = t2 * FIX_2_053119869 + z1 + t12;
        c[y0 + 1] = a0 >> (CONST_BITS - PASS1_BITS);
        c[y0 + 3] = a1 >> (CONST_BITS - PASS1_BITS);
        c[y0 + 5] = a2 >> (CONST_BITS - PASS1_BITS);
        c[y0 + 7] = a3 >> (CONST_BITS - PASS1_BITS);
    }
    for x in (0..8).rev() {
        let q = |i: usize| c[x + 8 * i];
        let (t0, t1, t2, t3) = (q(0) + q(7), q(1) + q(6), q(2) + q(5), q(3) + q(4));
        let t10 = t0 + t3 + (1 << (PASS1_BITS - 1));
        let (t12, t11, t13) = (t0 - t3, t1 + t2, t1 - t2);
        let (t0, t1, t2, t3) = (q(0) - q(7), q(1) - q(6), q(2) - q(5), q(3) - q(4));
        c[x] = (t10 + t11) >> PASS1_BITS;
        c[x + 32] = (t10 - t11) >> PASS1_BITS;
        let z1 = (t12 + t13) * FIX_0_541196100 + (1 << (CONST_BITS + PASS1_BITS - 1));
        c[x + 16] = (z1 + t12 * FIX_0_765366865) >> (CONST_BITS + PASS1_BITS);
        c[x + 48] = (z1 - t13 * FIX_1_847759065) >> (CONST_BITS + PASS1_BITS);
        let (t12, t13) = (t0 + t2, t1 + t3);
        // (Cet arrondi est celui de l'encodeur d'origine, pas celui de libjpeg.)
        let z1 = (t12 + t13) * FIX_1_175875602 + (1 << (CONST_BITS - PASS1_BITS - 1));
        let t12 = t12 * -FIX_0_390180644 + z1;
        let t13 = t13 * -FIX_1_961570560 + z1;
        let z1 = (t0 + t3) * -FIX_0_899976223;
        let a0 = t0 * FIX_1_501321110 + z1 + t12;
        let a3 = t3 * FIX_0_298631336 + z1 + t13;
        let z1 = (t1 + t2) * -FIX_2_562915447;
        let a1 = t1 * FIX_3_072711026 + z1 + t13;
        let a2 = t2 * FIX_2_053119869 + z1 + t12;
        c[x + 8] = a0 >> (CONST_BITS + PASS1_BITS);
        c[x + 24] = a1 >> (CONST_BITS + PASS1_BITS);
        c[x + 40] = a2 >> (CONST_BITS + PASS1_BITS);
        c[x + 56] = a3 >> (CONST_BITS + PASS1_BITS);
    }
}

/// RVB → YCbCr (BT.601 pleine échelle, en virgule fixe, arrondi au plus près).
#[inline]
fn ycbcr(p: &[u8]) -> (u8, u8, u8) {
    let (r, g, b) = (i32::from(p[0]), i32::from(p[1]), i32::from(p[2]));
    const ROUND: i32 = (1 << 15) - 1;
    const BIAS: i32 = (128 << 16) + ROUND;
    let y = (19595 * r + 38469 * g + 7471 * b + ROUND) >> 16;
    let cb = (-11059 * r - 21709 * g + 32768 * b + BIAS) >> 16;
    let cr = (32768 * r - 27439 * g - 5329 * b + BIAS) >> 16;
    (y as u8, cb as u8, cr as u8)
}

/// Les trois blocs quantifiés (Y, Cb, Cr) du bloc `(bx, by)` ; les pixels au‑delà du
/// bord répètent le dernier.
fn blocks(rgb: &[u8], w: usize, h: usize, bx: usize, by: usize, q: &[[u8; 64]; 2]) -> [[i32; 64]; 3] {
    let mut samples = [[0u8; 64]; 3];
    for y in 0..8 {
        let row = (by * 8 + y).min(h - 1) * w;
        for x in 0..8 {
            let px = (bx * 8 + x).min(w - 1);
            let (yc, cb, cr) = ycbcr(&rgb[(row + px) * 3..]);
            samples[0][y * 8 + x] = yc;
            samples[1][y * 8 + x] = cb;
            samples[2][y * 8 + x] = cr;
        }
    }
    let mut out = [[0i32; 64]; 3];
    for (k, (s, o)) in samples.iter().zip(out.iter_mut()).enumerate() {
        fdct(s, o);
        let table = &q[usize::from(k > 0)];
        for (v, &t) in o.iter_mut().zip(table.iter()) {
            *v = ((*v / 8) as f32 / f32::from(t)).round() as i32;
        }
    }
    out
}

fn segment(out: &mut Vec<u8>, marker: u8, data: &[u8]) {
    out.extend_from_slice(&[0xFF, marker]);
    out.extend_from_slice(&(data.len() as u16 + 2).to_be_bytes());
    out.extend_from_slice(data);
}

fn huffman_segment(out: &mut Vec<u8>, class: u8, dest: u8, bits: &[u8; 16], values: &[u8]) {
    let mut m = vec![(class << 4) | dest];
    m.extend_from_slice(bits);
    m.extend_from_slice(values);
    segment(out, DHT, &m);
}

/// Un JPEG de pixels RVB 8 bits (`width × height`, au plus 65 535 de côté), à la qualité
/// donnée, avec l'EXIF (sans son en‑tête `Exif\0\0`) et le profil ICC s'ils ne sont pas
/// vides.
pub fn encode(
    rgb: &[u8],
    width: usize,
    height: usize,
    quality: u8,
    exif: &[u8],
    icc: &[u8],
) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 || width > 65_535 || height > 65_535 || rgb.len() < width * height * 3 {
        return Err(format!("JPEG : taille impossible ({width} × {height})"));
    }
    let q = qtables(quality);
    let tables = [
        huffman(&LUMA_DC_BITS, &LUMA_DC_VALUES),
        huffman(&LUMA_AC_BITS, &LUMA_AC_VALUES),
        huffman(&CHROMA_DC_BITS, &CHROMA_DC_VALUES),
        huffman(&CHROMA_AC_BITS, &CHROMA_AC_VALUES),
    ];
    let (bw, bh) = (width.div_ceil(8), height.div_ceil(8));
    // La prédiction de chaque ligne de blocs : le coefficient continu du dernier bloc de
    // la ligne d'avant.
    let last: Vec<[i32; 3]> = (0..bh)
        .into_par_iter()
        .map(|by| blocks(rgb, width, height, bw - 1, by, &q).map(|b| b[0]))
        .collect();
    let rows: Vec<Bits> = (0..bh)
        .into_par_iter()
        .map(|by| {
            let mut w = Bits {
                bytes: Vec::with_capacity(bw * 48),
                ..Bits::default()
            };
            let mut prev = if by == 0 { [0; 3] } else { last[by - 1] };
            for bx in 0..bw {
                let b = blocks(rgb, width, height, bx, by, &q);
                prev[0] = write_block(&mut w, &b[0], prev[0], &tables[0], &tables[1]);
                prev[1] = write_block(&mut w, &b[1], prev[1], &tables[2], &tables[3]);
                prev[2] = write_block(&mut w, &b[2], prev[2], &tables[2], &tables[3]);
            }
            w
        })
        .collect();

    let mut out = vec![0xFF, SOI];
    segment(&mut out, APP0, &[b'J', b'F', b'I', b'F', 0, 1, 2, 0, 0, 1, 0, 1, 0, 0]);
    if !exif.is_empty() {
        let mut e = b"Exif\0\0".to_vec();
        e.extend_from_slice(exif);
        segment(&mut out, APP1, &e);
    }
    if !icc.is_empty() {
        const CHUNK: usize = 65_533 - 14;
        let n = icc.len().div_ceil(CHUNK);
        if n > 255 {
            return Err("JPEG : profil ICC trop grand".into());
        }
        for (i, chunk) in icc.chunks(CHUNK).enumerate() {
            let mut s = b"ICC_PROFILE\0".to_vec();
            s.extend_from_slice(&[i as u8 + 1, n as u8]);
            s.extend_from_slice(chunk);
            segment(&mut out, APP2, &s);
        }
    }
    let mut sof = vec![8];
    sof.extend_from_slice(&(height as u16).to_be_bytes());
    sof.extend_from_slice(&(width as u16).to_be_bytes());
    sof.extend_from_slice(&[3, 1, 0x11, 0, 2, 0x11, 1, 3, 0x11, 1]);
    segment(&mut out, SOF0, &sof);
    for (i, t) in q.iter().enumerate() {
        let mut m = vec![i as u8];
        m.extend(UNZIGZAG.iter().map(|&k| t[usize::from(k)]));
        segment(&mut out, DQT, &m);
    }
    huffman_segment(&mut out, 0, 0, &LUMA_DC_BITS, &LUMA_DC_VALUES);
    huffman_segment(&mut out, 1, 0, &LUMA_AC_BITS, &LUMA_AC_VALUES);
    huffman_segment(&mut out, 0, 1, &CHROMA_DC_BITS, &CHROMA_DC_VALUES);
    huffman_segment(&mut out, 1, 1, &CHROMA_AC_BITS, &CHROMA_AC_VALUES);
    segment(&mut out, SOS, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);
    let size: usize = rows.iter().map(|r| r.bytes.len() + 1).sum();
    let mut s = Stuffed {
        out: Vec::with_capacity(size + size / 64),
        acc: 0,
        n: 0,
    };
    for r in &rows {
        s.append(r);
    }
    // Le bourrage final : sept bits à 1 (un octet incomplet est laissé, comme avant).
    s.put(0x7F, 7);
    out.extend_from_slice(&s.out);
    out.extend_from_slice(&[0xFF, EOI]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    /// Le JPEG de l'encodeur de `image`, tel que LÜMA l'écrivait avant.
    fn reference(rgb: &[u8], w: usize, h: usize, quality: u8, exif: &[u8], icc: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
        if !icc.is_empty() {
            enc.set_icc_profile(icc.to_vec()).unwrap();
        }
        if !exif.is_empty() {
            enc.set_exif_metadata(exif.to_vec()).unwrap();
        }
        enc.write_image(rgb, w as u32, h as u32, image::ExtendedColorType::Rgb8)
            .unwrap();
        out
    }

    /// Des pixels pseudo‑aléatoires, avec des aplats et des dégradés : tous les cas de
    /// codage (longues suites de zéros, grands coefficients, octets 0xFF).
    fn pixels(w: usize, h: usize, seed: u32) -> Vec<u8> {
        let mut x = seed.wrapping_mul(2_654_435_761).max(1);
        (0..w * h * 3)
            .map(|i| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                let p = i / 3;
                match (p / w) % 3 {
                    0 => (x >> 24) as u8,
                    1 => ((p % w) * 255 / w.max(1)) as u8,
                    _ => 200,
                }
            })
            .collect()
    }

    #[test]
    fn same_bytes_as_before() {
        let icc = luma_color::icc::profile(luma_color::OutputSpace::DisplayP3);
        let exif = b"II*\0\x08\0\0\0\0\0\0\0\0\0".to_vec();
        for (w, h) in [(1, 1), (7, 13), (8, 8), (64, 64), (100, 37), (257, 129)] {
            for quality in [1, 30, 50, 75, 92, 100] {
                let rgb = pixels(w, h, (w * 31 + h) as u32 + u32::from(quality));
                let mine = encode(&rgb, w, h, quality, &[], &[]).unwrap();
                assert_eq!(
                    mine,
                    reference(&rgb, w, h, quality, &[], &[]),
                    "{w} × {h}, qualité {quality}"
                );
            }
        }
        let rgb = pixels(90, 70, 7);
        assert_eq!(
            encode(&rgb, 90, 70, 92, &exif, &icc).unwrap(),
            reference(&rgb, 90, 70, 92, &exif, &icc)
        );
        assert!(encode(&rgb, 0, 70, 92, &[], &[]).is_err());
    }
}
