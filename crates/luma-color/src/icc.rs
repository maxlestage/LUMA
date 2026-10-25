//! Profils ICC (version 2, matrice et courbes) des espaces de sortie, à incorporer dans
//! les JPEG, PNG et TIFF exportés. Sans profil, un JPEG en Display P3 serait lu comme
//! du sRGB et ses couleurs paraîtraient ternes et faussées.

use crate::{OutputSpace, apply, bradford, rgb_to_xyz};

fn s15f16(v: f32) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn xyz_tag(xyz: [f32; 3]) -> Vec<u8> {
    let mut t = b"XYZ \0\0\0\0".to_vec();
    for v in xyz {
        t.extend(s15f16(v));
    }
    t
}

fn text_tag(text: &str) -> Vec<u8> {
    let mut t = b"text\0\0\0\0".to_vec();
    t.extend(text.as_bytes());
    t.push(0);
    t
}

fn desc_tag(text: &str) -> Vec<u8> {
    let mut t = b"desc\0\0\0\0".to_vec();
    t.extend(((text.len() + 1) as u32).to_be_bytes());
    t.extend(text.as_bytes());
    t.push(0);
    // Pas de version Unicode ni ScriptCode.
    t.extend([0u8; 4 + 4 + 2 + 1 + 67]);
    t
}

fn curve_tag(space: OutputSpace) -> Vec<u8> {
    const N: usize = 1024;
    let mut t = b"curv\0\0\0\0".to_vec();
    t.extend((N as u32).to_be_bytes());
    for i in 0..N {
        let v = i as f32 / (N - 1) as f32;
        let linear = match space {
            OutputSpace::Srgb | OutputSpace::DisplayP3 => crate::srgb_decode(v),
            OutputSpace::Rec2020 => {
                if v < 0.081_242_86 {
                    v / 4.5
                } else {
                    ((v + 0.099_296_8) / 1.099_296_8).powf(1.0 / 0.45)
                }
            }
            OutputSpace::ProPhoto => {
                if v < 16.0 / 512.0 {
                    v / 16.0
                } else {
                    v.powf(1.8)
                }
            }
        };
        t.extend(((linear.clamp(0.0, 1.0) * 65535.0).round() as u16).to_be_bytes());
    }
    t
}

/// Le profil ICC d'un espace de sortie.
pub fn profile(space: OutputSpace) -> Vec<u8> {
    let name = match space {
        OutputSpace::Srgb => "sRGB (LÜMA)",
        OutputSpace::DisplayP3 => "Display P3 (LÜMA)",
        OutputSpace::Rec2020 => "Rec. 2020 (LÜMA)",
        OutputSpace::ProPhoto => "ProPhoto RGB (LÜMA)",
    };
    let p = space.primaries();
    // Les colorants sont exprimés en D50 (espace de connexion ICC).
    let m = crate::mul(&bradford(p.white, crate::D50), &rgb_to_xyz(&p));
    let col = |i: usize| [m[0][i], m[1][i], m[2][i]];
    let white = apply(&m, [1.0, 1.0, 1.0]);
    let trc = curve_tag(space);
    let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", desc_tag(name)),
        (b"cprt", text_tag("No copyright, use freely")),
        (b"wtpt", xyz_tag(white)),
        (b"rXYZ", xyz_tag(col(0))),
        (b"gXYZ", xyz_tag(col(1))),
        (b"bXYZ", xyz_tag(col(2))),
        (b"rTRC", trc.clone()),
        (b"gTRC", trc.clone()),
        (b"bTRC", trc),
    ];
    let table_len = 4 + 12 * tags.len();
    let mut offset = 128 + table_len;
    let mut table = (tags.len() as u32).to_be_bytes().to_vec();
    let mut data = Vec::new();
    for (sig, body) in &tags {
        table.extend(*sig);
        table.extend((offset as u32).to_be_bytes());
        table.extend((body.len() as u32).to_be_bytes());
        data.extend(body);
        let pad = (4 - body.len() % 4) % 4;
        data.extend(std::iter::repeat_n(0u8, pad));
        offset += body.len() + pad;
    }
    let size = 128 + table.len() + data.len();
    let mut header = Vec::with_capacity(128);
    header.extend((size as u32).to_be_bytes());
    header.extend(b"\0\0\0\0"); // module de gestion des couleurs
    header.extend([2, 0x10, 0, 0]); // version 2.1
    header.extend(b"mntr");
    header.extend(b"RGB ");
    header.extend(b"XYZ ");
    header.extend([0u8; 12]); // date
    header.extend(b"acsp");
    header.extend(b"APPL");
    header.extend([0u8; 4 + 4 + 4 + 8]); // drapeaux, fabricant, modèle, attributs
    header.extend(0u32.to_be_bytes()); // intention perceptuelle
    for v in [0.9642, 1.0, 0.8249] {
        header.extend(s15f16(v));
    }
    header.extend([0u8; 4]); // créateur
    header.resize(128, 0);
    let mut out = header;
    out.extend(table);
    out.extend(data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_are_well_formed() {
        for space in [
            OutputSpace::Srgb,
            OutputSpace::DisplayP3,
            OutputSpace::Rec2020,
            OutputSpace::ProPhoto,
        ] {
            let p = profile(space);
            let size = u32::from_be_bytes([p[0], p[1], p[2], p[3]]) as usize;
            assert_eq!(size, p.len());
            assert_eq!(&p[36..40], b"acsp");
            let count = u32::from_be_bytes([p[128], p[129], p[130], p[131]]) as usize;
            assert_eq!(count, 9);
            for i in 0..count {
                let e = 132 + i * 12;
                let off = u32::from_be_bytes([p[e + 4], p[e + 5], p[e + 6], p[e + 7]]) as usize;
                let len = u32::from_be_bytes([p[e + 8], p[e + 9], p[e + 10], p[e + 11]]) as usize;
                assert!(off.is_multiple_of(4) && off + len <= p.len());
            }
        }
    }

    #[test]
    fn srgb_colorants_match_the_standard_profile() {
        let p = profile(OutputSpace::Srgb);
        // rXYZ du profil sRGB IEC61966‑2.1 : 0,4361 0,2225 0,0139.
        let pos = p.windows(4).position(|w| w == b"rXYZ").unwrap();
        let off = u32::from_be_bytes(p[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let x = i32::from_be_bytes(p[off + 8..off + 12].try_into().unwrap()) as f32 / 65536.0;
        let y = i32::from_be_bytes(p[off + 12..off + 16].try_into().unwrap()) as f32 / 65536.0;
        assert!((x - 0.4361).abs() < 2e-3 && (y - 0.2225).abs() < 2e-3, "{x} {y}");
    }
}
