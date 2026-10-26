//! Lecture des profils ICC (versions 2 et 4) : les profils à matrice et courbes des
//! écrans, et les profils à tables (lut8, lut16, A→B et B→A) des imprimantes et des
//! papiers. Un profil se lit une fois ; ses transformations vont du périphérique vers
//! l'espace de connexion (XYZ sous D50) et retour, pour chaque intention de rendu.
//!
//! Rien n'est supposé sur le contenu du fichier : un profil tronqué ou faux donne une
//! erreur, jamais une panique ni une allocation démesurée.

use crate::{Mat3, Vec3, apply, inverse};

/// Le blanc de l'espace de connexion ICC (D50, Y = 1).
pub const PCS_WHITE: Vec3 = [0.9642, 1.0, 0.8249];

/// Le nombre maximal de canaux d'un profil (15 couleurs dans la norme).
pub const MAX_CHANNELS: usize = 15;

/// Au‑delà, une table de couleurs est refusée (64 Mo de flottants).
const MAX_CLUT: usize = 1 << 24;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub &'static str);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

const TRUNCATED: Error = Error("profil ICC tronqué");

/// La classe du profil : à quel périphérique il se rapporte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Input,
    Display,
    Output,
    Link,
    ColorSpace,
    Abstract,
    Named,
}

/// L'espace des données du périphérique.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Space {
    Gray,
    Rgb,
    Cmy,
    Cmyk,
    Lab,
    Xyz,
    /// Un espace à n couleurs (2 à 15) : encres spéciales, hexachromie…
    Colors(u8),
}

impl Space {
    pub fn channels(self) -> usize {
        match self {
            Space::Gray => 1,
            Space::Rgb | Space::Cmy | Space::Lab | Space::Xyz => 3,
            Space::Cmyk => 4,
            Space::Colors(n) => usize::from(n),
        }
    }

    fn parse(sig: &[u8]) -> Option<Self> {
        Some(match sig {
            b"GRAY" => Space::Gray,
            b"RGB " => Space::Rgb,
            b"CMY " => Space::Cmy,
            b"CMYK" => Space::Cmyk,
            b"Lab " => Space::Lab,
            b"XYZ " => Space::Xyz,
            [n, b'C', b'L', b'R'] => {
                let n = (*n as char).to_digit(16)?;
                if !(2..=15).contains(&n) {
                    return None;
                }
                Space::Colors(n as u8)
            }
            _ => return None,
        })
    }
}

/// L'espace de connexion du profil.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pcs {
    Xyz,
    Lab,
}

/// L'intention de rendu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Perceptual,
    Relative,
    Saturation,
    Absolute,
}

impl Intent {
    pub const ALL: [Intent; 4] = [
        Intent::Perceptual,
        Intent::Relative,
        Intent::Saturation,
        Intent::Absolute,
    ];

    /// La table qui porte l'intention (A2B0, A2B1, A2B2) ; l'absolue se déduit de la
    /// relative et du blanc du support.
    fn table(self) -> usize {
        match self {
            Intent::Perceptual => 0,
            Intent::Relative | Intent::Absolute => 1,
            Intent::Saturation => 2,
        }
    }
}

/// Une courbe de tonalité : gamma, table échantillonnée ou fonction paramétrique.
#[derive(Clone, Debug, PartialEq)]
pub enum Curve {
    Gamma(f32),
    /// Des échantillons de 0 à 1, régulièrement espacés.
    Table(Vec<f32>),
    /// Les paramètres g, a, b, c, d, e, f et le numéro de la fonction (0 à 4).
    Parametric([f32; 7], u16),
}

impl Curve {
    pub fn eval(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match self {
            Curve::Gamma(g) => x.powf(*g),
            Curve::Table(t) => {
                let pos = x * (t.len() - 1) as f32;
                let i = (pos as usize).min(t.len() - 2);
                let f = pos - i as f32;
                t[i] + (t[i + 1] - t[i]) * f
            }
            Curve::Parametric([g, a, b, c, d, e, f], kind) => {
                let pow = |v: f32| v.max(0.0).powf(*g);
                match kind {
                    0 => pow(x),
                    1 => {
                        if a * x + b >= 0.0 {
                            pow(a * x + b)
                        } else {
                            0.0
                        }
                    }
                    2 => {
                        if a * x + b >= 0.0 {
                            pow(a * x + b) + c
                        } else {
                            *c
                        }
                    }
                    3 => {
                        if x >= *d {
                            pow(a * x + b)
                        } else {
                            c * x
                        }
                    }
                    _ => {
                        if x >= *d {
                            pow(a * x + b) + e
                        } else {
                            c * x + f
                        }
                    }
                }
            }
        }
    }

    /// La réciproque, par dichotomie (les courbes d'un profil sont monotones, croissantes
    /// ou décroissantes).
    pub fn invert(&self, y: f32) -> f32 {
        if let Curve::Gamma(g) = self
            && *g > 0.0
        {
            return y.clamp(0.0, 1.0).powf(1.0 / g);
        }
        let rising = self.eval(1.0) >= self.eval(0.0);
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if (self.eval(mid) < y) == rising {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    }
}

/// Une table de couleurs à n entrées, interpolée linéairement sur chaque axe.
#[derive(Clone, Debug, PartialEq)]
struct Clut {
    grid: Vec<usize>,
    outputs: usize,
    data: Vec<f32>,
}

impl Clut {
    fn eval(&self, input: &[f32], out: &mut [f32]) {
        let n = self.grid.len();
        let mut base = 0;
        let mut frac = [0.0f32; MAX_CHANNELS];
        let mut step = [0usize; MAX_CHANNELS];
        let mut stride = self.outputs;
        for i in (0..n).rev() {
            let g = self.grid[i];
            let pos = input[i].clamp(0.0, 1.0) * (g - 1) as f32;
            let k = (pos as usize).min(g.saturating_sub(2));
            frac[i] = if g > 1 { pos - k as f32 } else { 0.0 };
            step[i] = if g > 1 { stride } else { 0 };
            base += k * stride;
            stride *= g;
        }
        out[..self.outputs].fill(0.0);
        for corner in 0..1usize << n {
            let mut w = 1.0;
            let mut at = base;
            for i in 0..n {
                if corner >> i & 1 == 1 {
                    w *= frac[i];
                    at += step[i];
                } else {
                    w *= 1.0 - frac[i];
                }
            }
            if w != 0.0 {
                for (o, v) in out[..self.outputs].iter_mut().zip(&self.data[at..]) {
                    *o += w * v;
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Stage {
    Curves(Vec<Curve>),
    Matrix(Mat3, Vec3),
    Clut(Clut),
}

/// Le codage des valeurs de l'espace de connexion dans une table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    /// lut16 : L* = 100 pour 0xFF00.
    Legacy,
    /// lut8, A→B, B→A : L* = 100 pour 0xFFFF.
    Modern,
}

/// Une transformation à tables : suite d'étapes, entrées et sorties de 0 à 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Lut {
    inputs: usize,
    outputs: usize,
    stages: Vec<Stage>,
    encoding: Encoding,
}

impl Lut {
    fn eval(&self, input: &[f32]) -> [f32; MAX_CHANNELS] {
        let mut a = [0.0f32; MAX_CHANNELS];
        let mut b = [0.0f32; MAX_CHANNELS];
        a[..self.inputs].copy_from_slice(&input[..self.inputs]);
        for stage in &self.stages {
            match stage {
                Stage::Curves(curves) => {
                    for (v, c) in a.iter_mut().zip(curves) {
                        *v = c.eval(*v);
                    }
                }
                Stage::Matrix(m, offset) => {
                    let v = apply(m, [a[0], a[1], a[2]]);
                    for i in 0..3 {
                        a[i] = (v[i] + offset[i]).clamp(0.0, 1.0);
                    }
                }
                Stage::Clut(clut) => {
                    clut.eval(&a, &mut b);
                    std::mem::swap(&mut a, &mut b);
                }
            }
        }
        a
    }
}

/// Une couleur du périphérique : ses canaux, de 0 à 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    values: [f32; MAX_CHANNELS],
    len: usize,
}

impl std::ops::Deref for Color {
    type Target = [f32];
    fn deref(&self) -> &[f32] {
        &self.values[..self.len]
    }
}

/// Un profil ICC lu.
#[derive(Clone, Debug)]
pub struct Profile {
    pub version: (u8, u8),
    pub class: Class,
    pub space: Space,
    pub pcs: Pcs,
    /// L'intention de rendu inscrite dans l'en‑tête.
    pub intent: Intent,
    pub description: String,
    /// Le blanc du support (balise wtpt), D50 s'il manque.
    pub white: Vec3,
    matrix: Option<(Mat3, Mat3, [Curve; 3])>,
    gray: Option<Curve>,
    a2b: [Option<Lut>; 3],
    b2a: [Option<Lut>; 3],
}

fn be16(d: &[u8], at: usize) -> Result<u16> {
    let b = d.get(at..at + 2).ok_or(TRUNCATED)?;
    Ok(u16::from_be_bytes([b[0], b[1]]))
}

fn be32(d: &[u8], at: usize) -> Result<u32> {
    let b = d.get(at..at + 4).ok_or(TRUNCATED)?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn s15f16(d: &[u8], at: usize) -> Result<f32> {
    Ok(be32(d, at)? as i32 as f32 / 65536.0)
}

fn xyz(d: &[u8]) -> Result<Vec3> {
    if d.get(..4) != Some(b"XYZ ") {
        return Err(Error("balise XYZ invalide"));
    }
    Ok([s15f16(d, 8)?, s15f16(d, 12)?, s15f16(d, 16)?])
}

/// Une courbe (curv ou para) et la place qu'elle occupe, alignée sur 4 octets.
fn curve(d: &[u8]) -> Result<(Curve, usize)> {
    let len;
    let c = match d.get(..4).ok_or(TRUNCATED)? {
        b"curv" => {
            let n = be32(d, 8)? as usize;
            len = 12 + n.checked_mul(2).ok_or(TRUNCATED)?;
            if d.len() < len {
                return Err(TRUNCATED);
            }
            match n {
                0 => Curve::Gamma(1.0),
                1 => Curve::Gamma(f32::from(be16(d, 12)?) / 256.0),
                _ => Curve::Table(
                    d[12..len]
                        .chunks_exact(2)
                        .map(|b| f32::from(u16::from_be_bytes([b[0], b[1]])) / 65535.0)
                        .collect(),
                ),
            }
        }
        b"para" => {
            let kind = be16(d, 8)?;
            let count = match kind {
                0 => 1,
                1 => 3,
                2 => 4,
                3 => 5,
                4 => 7,
                _ => return Err(Error("courbe paramétrique inconnue")),
            };
            let mut p = [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
            for (i, v) in p.iter_mut().enumerate().take(count) {
                *v = s15f16(d, 12 + 4 * i)?;
            }
            if kind != 0 && p[1] == 0.0 {
                return Err(Error("courbe paramétrique invalide"));
            }
            len = 12 + 4 * count;
            Curve::Parametric(p, kind)
        }
        _ => return Err(Error("courbe de type inconnu")),
    };
    Ok((c, len.next_multiple_of(4)))
}

fn curves(d: &[u8], at: usize, n: usize) -> Result<Vec<Curve>> {
    let mut at = at;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let (c, len) = curve(d.get(at..).ok_or(TRUNCATED)?)?;
        out.push(c);
        at += len;
    }
    Ok(out)
}

fn clut(d: &[u8], grid: Vec<usize>, outputs: usize, bytes: usize) -> Result<Clut> {
    if grid.contains(&0) {
        return Err(Error("table de couleurs vide"));
    }
    let count = grid
        .iter()
        .try_fold(outputs, |n, &g| n.checked_mul(g))
        .filter(|&n| n <= MAX_CLUT)
        .ok_or(Error("table de couleurs démesurée"))?;
    let raw = d.get(..count * bytes).ok_or(TRUNCATED)?;
    let data = if bytes == 1 {
        raw.iter().map(|&v| f32::from(v) / 255.0).collect()
    } else {
        raw.chunks_exact(2)
            .map(|b| f32::from(u16::from_be_bytes([b[0], b[1]])) / 65535.0)
            .collect()
    };
    Ok(Clut { grid, outputs, data })
}

fn matrix3(d: &[u8], at: usize) -> Result<Mat3> {
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = s15f16(d, at + 4 * (3 * i + j))?;
        }
    }
    Ok(m)
}

/// lut8 (mft1) et lut16 (mft2). La matrice ne sert que si l'entrée est en XYZ.
fn lut_legacy(d: &[u8], xyz_input: bool) -> Result<Lut> {
    let wide = d.get(..4) == Some(b"mft2");
    let (inputs, outputs, grid) = (
        usize::from(*d.get(8).ok_or(TRUNCATED)?),
        usize::from(*d.get(9).ok_or(TRUNCATED)?),
        usize::from(*d.get(10).ok_or(TRUNCATED)?),
    );
    if !(1..=MAX_CHANNELS).contains(&inputs) || !(1..=MAX_CHANNELS).contains(&outputs) || grid < 2 {
        return Err(Error("table LUT invalide"));
    }
    let m = matrix3(d, 12)?;
    let (n_in, n_out, bytes, mut at) = if wide {
        (usize::from(be16(d, 48)?), usize::from(be16(d, 50)?), 2, 52)
    } else {
        (256, 256, 1, 48)
    };
    if n_in < 2 || n_out < 2 {
        return Err(Error("table LUT invalide"));
    }
    let table = |at: usize, n: usize| -> Result<Curve> {
        let raw = d.get(at..at + n * bytes).ok_or(TRUNCATED)?;
        Ok(Curve::Table(if bytes == 1 {
            raw.iter().map(|&v| f32::from(v) / 255.0).collect()
        } else {
            raw.chunks_exact(2)
                .map(|b| f32::from(u16::from_be_bytes([b[0], b[1]])) / 65535.0)
                .collect()
        }))
    };
    let mut stages = Vec::new();
    if xyz_input && inputs == 3 && m != crate::IDENTITY {
        stages.push(Stage::Matrix(m, [0.0; 3]));
    }
    let mut input_curves = Vec::with_capacity(inputs);
    for _ in 0..inputs {
        input_curves.push(table(at, n_in)?);
        at += n_in * bytes;
    }
    stages.push(Stage::Curves(input_curves));
    let c = clut(d.get(at..).ok_or(TRUNCATED)?, vec![grid; inputs], outputs, bytes)?;
    at += c.data.len() * bytes;
    stages.push(Stage::Clut(c));
    let mut output_curves = Vec::with_capacity(outputs);
    for _ in 0..outputs {
        output_curves.push(table(at, n_out)?);
        at += n_out * bytes;
    }
    stages.push(Stage::Curves(output_curves));
    Ok(Lut {
        inputs,
        outputs,
        stages,
        encoding: if wide { Encoding::Legacy } else { Encoding::Modern },
    })
}

/// lutAtoB (mAB) et lutBtoA (mBA) de la version 4.
fn lut_modern(d: &[u8]) -> Result<Lut> {
    let a_to_b = d.get(..4) == Some(b"mAB ");
    let (inputs, outputs) = (
        usize::from(*d.get(8).ok_or(TRUNCATED)?),
        usize::from(*d.get(9).ok_or(TRUNCATED)?),
    );
    if !(1..=MAX_CHANNELS).contains(&inputs) || !(1..=MAX_CHANNELS).contains(&outputs) {
        return Err(Error("table LUT invalide"));
    }
    let offset = |at: usize| be32(d, at).map(|v| v as usize);
    let (b, m, mat, c, a) = (offset(12)?, offset(20)?, offset(16)?, offset(24)?, offset(28)?);
    // Les courbes A sont du côté du périphérique, B du côté de l'espace de connexion.
    let (device, pcs) = if a_to_b { (inputs, outputs) } else { (outputs, inputs) };
    let mut a_stage = Vec::new();
    if a != 0 {
        a_stage.push(Stage::Curves(curves(d, a, device)?));
    }
    if c != 0 {
        let (from, to) = if a_to_b { (inputs, outputs) } else { (pcs, device) };
        let mut grid = Vec::with_capacity(from);
        for i in 0..from {
            grid.push(usize::from(*d.get(c + i).ok_or(TRUNCATED)?));
        }
        let bytes = usize::from(*d.get(c + 16).ok_or(TRUNCATED)?);
        if bytes != 1 && bytes != 2 {
            return Err(Error("table de couleurs invalide"));
        }
        a_stage.push(Stage::Clut(clut(d.get(c + 20..).ok_or(TRUNCATED)?, grid, to, bytes)?));
    } else if a != 0 && device != pcs {
        return Err(Error("table LUT invalide"));
    }
    let mut b_stage = Vec::new();
    if m != 0 {
        if pcs != 3 {
            return Err(Error("table LUT invalide"));
        }
        b_stage.push(Stage::Curves(curves(d, m, 3)?));
    }
    if mat != 0 {
        if pcs != 3 {
            return Err(Error("table LUT invalide"));
        }
        let offset = [s15f16(d, mat + 36)?, s15f16(d, mat + 40)?, s15f16(d, mat + 44)?];
        b_stage.push(Stage::Matrix(matrix3(d, mat)?, offset));
    }
    if b == 0 {
        return Err(Error("table LUT sans courbes B"));
    }
    b_stage.push(Stage::Curves(curves(d, b, pcs)?));
    let stages = if a_to_b {
        a_stage.into_iter().chain(b_stage).collect()
    } else {
        // B→A : les mêmes étapes, parcourues de l'espace de connexion vers le périphérique.
        b_stage.into_iter().rev().chain(a_stage.into_iter().rev()).collect()
    };
    Ok(Lut {
        inputs,
        outputs,
        stages,
        encoding: Encoding::Modern,
    })
}

fn lut(d: &[u8], xyz_input: bool) -> Result<Lut> {
    match d.get(..4).ok_or(TRUNCATED)? {
        b"mft1" | b"mft2" => lut_legacy(d, xyz_input),
        b"mAB " | b"mBA " => lut_modern(d),
        _ => Err(Error("table LUT de type inconnu")),
    }
}

fn text(d: &[u8]) -> Option<String> {
    let latin = |b: &[u8]| -> String {
        // De l'UTF‑8 s'il en est (bien des profils l'y mettent), sinon du Latin‑1.
        let b = &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())];
        match std::str::from_utf8(b) {
            Ok(s) => s.trim().to_owned(),
            Err(_) => b.iter().map(|&c| c as char).collect::<String>().trim().to_owned(),
        }
    };
    match d.get(..4)? {
        b"desc" => {
            let n = be32(d, 8).ok()? as usize;
            Some(latin(d.get(12..12 + n).unwrap_or(d.get(12..)?)))
        }
        b"text" => Some(latin(d.get(8..)?)),
        b"mluc" => {
            let (count, size) = (be32(d, 8).ok()? as usize, be32(d, 12).ok()? as usize);
            if size < 12 {
                return None;
            }
            let records = (0..count.min(256)).filter_map(|i| {
                let r = d.get(16 + i * size..16 + i * size + 12)?;
                let (len, at) = (be32(r, 4).ok()? as usize, be32(r, 8).ok()? as usize);
                let units: Vec<u16> = d
                    .get(at..at.checked_add(len)?)?
                    .chunks_exact(2)
                    .map(|b| u16::from_be_bytes([b[0], b[1]]))
                    .collect();
                Some((&r[..2] == b"en", String::from_utf16_lossy(&units)))
            });
            // L'anglais s'il y est, sinon la première langue.
            let mut first = None;
            for (english, s) in records {
                if english {
                    return Some(s.trim_end_matches('\0').trim().to_owned());
                }
                first.get_or_insert(s);
            }
            first.map(|s| s.trim_end_matches('\0').trim().to_owned())
        }
        _ => None,
    }
}

/// CIE Lab sous D50, depuis le XYZ de l'espace de connexion.
pub fn xyz_to_lab(v: Vec3) -> Vec3 {
    let f = |t: f32| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let [fx, fy, fz] = [0, 1, 2].map(|i| f(v[i] / PCS_WHITE[i]));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

pub fn lab_to_xyz(lab: Vec3) -> Vec3 {
    let fy = (lab[0] + 16.0) / 116.0;
    let f = [fy + lab[1] / 500.0, fy, fy - lab[2] / 200.0];
    let g = |t: f32| {
        if t > 6.0 / 29.0 {
            t * t * t
        } else {
            (116.0 * t - 16.0) * 27.0 / 24389.0
        }
    };
    [0, 1, 2].map(|i| g(f[i]) * PCS_WHITE[i])
}

/// Les valeurs de l'espace de connexion, de 0 à 1 dans une table, en XYZ.
fn decode_pcs(pcs: Pcs, encoding: Encoding, v: &[f32]) -> Vec3 {
    match pcs {
        Pcs::Xyz => [v[0], v[1], v[2]].map(|c| c * 65535.0 / 32768.0),
        Pcs::Lab => {
            let k = if encoding == Encoding::Legacy {
                65535.0 / 65280.0
            } else {
                1.0
            };
            lab_to_xyz([v[0] * k * 100.0, v[1] * k * 255.0 - 128.0, v[2] * k * 255.0 - 128.0])
        }
    }
}

fn encode_pcs(pcs: Pcs, encoding: Encoding, xyz: Vec3) -> [f32; 3] {
    match pcs {
        Pcs::Xyz => xyz.map(|c| (c * 32768.0 / 65535.0).clamp(0.0, 1.0)),
        Pcs::Lab => {
            let k = if encoding == Encoding::Legacy {
                65280.0 / 65535.0
            } else {
                1.0
            };
            let lab = xyz_to_lab(xyz);
            [lab[0] / 100.0, (lab[1] + 128.0) / 255.0, (lab[2] + 128.0) / 255.0].map(|c| (c * k).clamp(0.0, 1.0))
        }
    }
}

impl Profile {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < 132 {
            return Err(TRUNCATED);
        }
        if &data[36..40] != b"acsp" {
            return Err(Error("ce fichier n'est pas un profil ICC"));
        }
        let size = be32(data, 0)? as usize;
        let d = data.get(..size).ok_or(TRUNCATED)?;
        let class = match &d[12..16] {
            b"scnr" => Class::Input,
            b"mntr" => Class::Display,
            b"prtr" => Class::Output,
            b"link" => Class::Link,
            b"spac" => Class::ColorSpace,
            b"abst" => Class::Abstract,
            b"nmcl" => Class::Named,
            _ => return Err(Error("classe de profil inconnue")),
        };
        let space = Space::parse(&d[16..20]).ok_or(Error("espace de couleurs inconnu"))?;
        let pcs = match &d[20..24] {
            b"XYZ " => Pcs::Xyz,
            b"Lab " => Pcs::Lab,
            _ => return Err(Error("espace de connexion inconnu")),
        };
        let intent = Intent::ALL[usize::from(be16(d, 66)? & 3)];

        let count = be32(d, 128)? as usize;
        if count > (d.len() - 132) / 12 {
            return Err(TRUNCATED);
        }
        let tag = |sig: &[u8; 4]| -> Option<&[u8]> {
            (0..count).find_map(|i| {
                let e = 132 + 12 * i;
                if &d[e..e + 4] != sig {
                    return None;
                }
                let (at, len) = (be32(d, e + 4).ok()? as usize, be32(d, e + 8).ok()? as usize);
                d.get(at..at.checked_add(len)?)
            })
        };
        let required = |sig: &[u8; 4]| tag(sig).ok_or(Error("balise hors du fichier"));
        let tables = |prefix: &[u8; 3], xyz_input: bool| -> Result<[Option<Lut>; 3]> {
            let mut out = [None, None, None];
            for (i, slot) in out.iter_mut().enumerate() {
                let sig = [prefix[0], prefix[1], prefix[2], b'0' + i as u8];
                if let Some(t) = tag(&sig) {
                    *slot = Some(lut(t, xyz_input)?);
                }
            }
            Ok(out)
        };
        let a2b = tables(b"A2B", space == Space::Xyz)?;
        let b2a = tables(b"B2A", pcs == Pcs::Xyz)?;
        for l in a2b.iter().flatten() {
            if l.inputs != space.channels() || l.outputs != 3 {
                return Err(Error("table A→B incohérente avec le profil"));
            }
        }
        for l in b2a.iter().flatten() {
            if l.inputs != 3 || l.outputs != space.channels() {
                return Err(Error("table B→A incohérente avec le profil"));
            }
        }

        let matrix = match (space, tag(b"rXYZ"), tag(b"rTRC")) {
            (Space::Rgb, Some(_), Some(_)) => {
                let cols = [
                    xyz(required(b"rXYZ")?)?,
                    xyz(required(b"gXYZ")?)?,
                    xyz(required(b"bXYZ")?)?,
                ];
                let m = [0, 1, 2].map(|i| [cols[0][i], cols[1][i], cols[2][i]]);
                let inv = inverse(&m).ok_or(Error("matrice du profil non inversible"))?;
                let trc = |sig: &[u8; 4]| -> Result<Curve> { Ok(curve(required(sig)?)?.0) };
                Some((m, inv, [trc(b"rTRC")?, trc(b"gTRC")?, trc(b"bTRC")?]))
            }
            _ => None,
        };
        let gray = match (space, tag(b"kTRC")) {
            (Space::Gray, Some(t)) => Some(curve(t)?.0),
            _ => None,
        };
        if a2b.iter().all(Option::is_none) && matrix.is_none() && gray.is_none() && class != Class::Named {
            return Err(Error("profil sans transformation utilisable"));
        }
        let white = match tag(b"wtpt") {
            Some(t) => xyz(t)?,
            None => PCS_WHITE,
        };
        let description = tag(b"desc").and_then(text).unwrap_or_default();
        Ok(Profile {
            version: (d[8], d[9] >> 4),
            class,
            space,
            pcs,
            intent,
            description,
            white,
            matrix,
            gray,
            a2b,
            b2a,
        })
    }

    pub fn channels(&self) -> usize {
        self.space.channels()
    }

    /// Le profil sait‑il aller du périphérique vers l'espace de connexion ?
    pub fn reads(&self) -> bool {
        self.a2b.iter().any(Option::is_some) || self.matrix.is_some() || self.gray.is_some()
    }

    /// Le profil sait‑il aller de l'espace de connexion vers le périphérique (pour
    /// l'impression et l'épreuvage) ?
    pub fn writes(&self) -> bool {
        self.b2a.iter().any(Option::is_some) || self.matrix.is_some() || self.gray.is_some()
    }

    /// Les intentions que portent des tables propres, dans un sens ou dans l'autre (les
    /// autres se rabattent sur la perceptuelle ou sur la matrice ; l'absolue se déduit
    /// de la relative).
    pub fn tables(&self) -> Vec<Intent> {
        Intent::ALL
            .into_iter()
            .filter(|i| *i != Intent::Absolute && (self.a2b[i.table()].is_some() || self.b2a[i.table()].is_some()))
            .collect()
    }

    fn pick(tables: &[Option<Lut>; 3], intent: Intent) -> Option<&Lut> {
        tables[intent.table()].as_ref().or(tables[0].as_ref())
    }

    /// Le blanc du support ramené au blanc de l'espace de connexion : l'intention
    /// colorimétrique absolue garde le ton du papier.
    fn absolute(&self, intent: Intent) -> Vec3 {
        if intent == Intent::Absolute {
            [0, 1, 2].map(|i| self.white[i] / PCS_WHITE[i])
        } else {
            [1.0; 3]
        }
    }

    /// Du périphérique (canaux de 0 à 1 ; un Lab en L/100, (a+128)/255…) vers le XYZ
    /// sous D50 de l'espace de connexion.
    pub fn to_pcs(&self, device: &[f32], intent: Intent) -> Option<Vec3> {
        if device.len() < self.channels() {
            return None;
        }
        let xyz = if let Some(l) = Self::pick(&self.a2b, intent) {
            let out = l.eval(device);
            decode_pcs(self.pcs, l.encoding, &out)
        } else if let Some((m, _, trc)) = &self.matrix {
            apply(m, [0, 1, 2].map(|i| trc[i].eval(device[i])))
        } else {
            let y = self.gray.as_ref()?.eval(device[0]);
            PCS_WHITE.map(|w| w * y)
        };
        let k = self.absolute(intent);
        Some([0, 1, 2].map(|i| xyz[i] * k[i]))
    }

    /// Du XYZ sous D50 de l'espace de connexion vers le périphérique.
    pub fn from_pcs(&self, xyz: Vec3, intent: Intent) -> Option<Color> {
        let k = self.absolute(intent);
        let xyz = [0, 1, 2].map(|i| xyz[i] / k[i]);
        let mut values = [0.0; MAX_CHANNELS];
        if let Some(l) = Self::pick(&self.b2a, intent) {
            let out = l.eval(&encode_pcs(self.pcs, l.encoding, xyz));
            values[..l.outputs].copy_from_slice(&out[..l.outputs]);
        } else if let Some((_, inv, trc)) = &self.matrix {
            let lin = apply(inv, xyz);
            for i in 0..3 {
                values[i] = trc[i].invert(lin[i]);
            }
        } else {
            values[0] = self.gray.as_ref()?.invert(xyz[1]);
        }
        Some(Color {
            values,
            len: self.channels(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OutputSpace, icc};

    #[test]
    fn our_output_profiles_read_back() {
        for space in [
            OutputSpace::Srgb,
            OutputSpace::DisplayP3,
            OutputSpace::Rec2020,
            OutputSpace::ProPhoto,
        ] {
            let p = Profile::parse(&icc::profile(space)).unwrap();
            assert_eq!((p.class, p.space, p.pcs), (Class::Display, Space::Rgb, Pcs::Xyz));
            assert!(p.description.ends_with("(LÜMA)"), "{}", p.description);
            let white = p.to_pcs(&[1.0, 1.0, 1.0], Intent::Relative).unwrap();
            for i in 0..3 {
                assert!((white[i] - PCS_WHITE[i]).abs() < 2e-3, "{space:?} {white:?}");
            }
            for c in [[0.2, 0.5, 0.8], [0.9, 0.1, 0.4], [0.5, 0.5, 0.5]] {
                let xyz = p.to_pcs(&c, Intent::Perceptual).unwrap();
                let back = p.from_pcs(xyz, Intent::Perceptual).unwrap();
                for i in 0..3 {
                    assert!((back[i] - c[i]).abs() < 2e-3, "{space:?} {c:?} {back:?}");
                }
            }
        }
    }

    #[test]
    fn lab_round_trip() {
        for lab in [[50.0, 20.0, -30.0], [95.0, -2.0, 4.0], [3.0, 0.5, -0.5]] {
            let back = xyz_to_lab(lab_to_xyz(lab));
            for i in 0..3 {
                assert!((back[i] - lab[i]).abs() < 1e-3, "{lab:?} {back:?}");
            }
        }
        let w = xyz_to_lab(PCS_WHITE);
        assert!((w[0] - 100.0).abs() < 1e-3 && w[1].abs() < 1e-3 && w[2].abs() < 1e-3);
    }

    #[test]
    fn parametric_curves() {
        // La courbe du sRGB (fonction 3).
        let srgb = Curve::Parametric([2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045, 0.0, 0.0], 3);
        for x in [0.0, 0.02, 0.2, 0.5, 1.0] {
            assert!((srgb.eval(x) - crate::srgb_decode(x)).abs() < 1e-5);
            assert!((srgb.invert(srgb.eval(x)) - x).abs() < 1e-4);
        }
        let falling = Curve::Table(vec![1.0, 0.5, 0.0]);
        assert!((falling.invert(0.25) - 0.75).abs() < 1e-4);
    }

    #[test]
    fn clut_interpolates_every_axis() {
        // f(x, y, z) = (x, y, z) sur une grille 2×3×2 : l'interpolation multilinéaire
        // le rend exactement.
        let grid = vec![2, 3, 2];
        let mut data = Vec::new();
        for x in 0..2 {
            for y in 0..3 {
                for z in 0..2 {
                    data.extend([x as f32, y as f32 / 2.0, z as f32]);
                }
            }
        }
        let c = Clut { grid, outputs: 3, data };
        let mut out = [0.0; MAX_CHANNELS];
        c.eval(&[0.3, 0.8, 0.6], &mut out);
        for (o, e) in out.iter().zip([0.3, 0.8, 0.6]) {
            assert!((o - e).abs() < 1e-5, "{out:?}");
        }
    }

    #[test]
    fn garbage_never_panics() {
        let good = icc::profile(OutputSpace::Srgb);
        // Tronqué à chaque longueur, puis abîmé octet par octet.
        for n in 0..good.len() {
            let mut cut = good[..n].to_vec();
            if cut.len() >= 4 {
                cut[..4].copy_from_slice(&(n as u32).to_be_bytes());
            }
            let _ = Profile::parse(&cut);
        }
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..4000 {
            let mut bad = good.clone();
            for _ in 0..8 {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let at = 128 + (seed as usize) % (bad.len() - 128);
                bad[at] = (seed >> 32) as u8;
            }
            if let Ok(p) = Profile::parse(&bad) {
                let _ = p.to_pcs(&[0.5, 0.5, 0.5], Intent::Relative);
                let _ = p.from_pcs([0.4, 0.4, 0.4], Intent::Perceptual);
            }
        }
    }
}
