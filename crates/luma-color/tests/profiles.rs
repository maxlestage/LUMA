//! Lecture de profils ICC réels (libres, CC0) et de profils construits ici, aux tables
//! connues, pour vérifier chaque type de table dans les deux sens.

use luma_color::profile::{Class, Intent, Pcs, Profile, Space, lab_to_xyz, xyz_to_lab};
use luma_color::{SRGB, Vec3, apply, bradford, inverse, mul, rgb_to_xyz, srgb_decode, srgb_encode};

const CMYK: &[u8] = include_bytes!("profils/CGATS001Compat-v2-micro.icc");
const LAB: &[u8] = include_bytes!("profils/LAB.icc");

fn delta_e(a: Vec3, b: Vec3) -> f32 {
    let (a, b) = (xyz_to_lab(a), xyz_to_lab(b));
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn cmyk_press_profile() {
    let p = Profile::parse(CMYK).unwrap();
    assert_eq!((p.space, p.pcs, p.version), (Space::Cmyk, Pcs::Lab, (2, 1)));
    assert!(p.reads() && !p.writes());
    // Valeurs de référence d'un module de gestion des couleurs indépendant (à 1 près,
    // l'arrondi de sa sortie sur 8 bits).
    for (cmyk, lab) in [
        ([0.0, 0.0, 0.0, 0.0], [100.0, 0.0, 0.0]),
        ([1.0, 0.0, 0.0, 0.0], [62.35, -44.0, -50.0]),
        ([0.0, 1.0, 0.0, 0.0], [52.16, 81.0, -7.0]),
        ([0.0, 0.0, 1.0, 0.0], [94.9, -6.0, 96.0]),
        ([0.0, 0.0, 0.0, 1.0], [12.16, 2.0, 0.0]),
        ([0.3, 0.6, 0.2, 0.1], [53.33, 26.0, -7.0]),
        ([1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0]),
    ] {
        let got = xyz_to_lab(p.to_pcs(&cmyk, Intent::Perceptual).unwrap());
        for i in 0..3 {
            assert!((got[i] - lab[i]).abs() < 1.0, "{cmyk:?} : {got:?} au lieu de {lab:?}");
        }
    }
}

#[test]
fn version_4_lab_identity() {
    let p = Profile::parse(LAB).unwrap();
    assert_eq!((p.version, p.class, p.space), ((4, 4), Class::Abstract, Space::Lab));
    assert_eq!(p.description, "Lab identity built-in");
    let lab = xyz_to_lab(p.to_pcs(&[0.8, 0.3, 0.7], Intent::Relative).unwrap());
    let want = [80.0, 0.3 * 255.0 - 128.0, 0.7 * 255.0 - 128.0];
    for i in 0..3 {
        assert!((lab[i] - want[i]).abs() < 0.01, "{lab:?}");
    }
}

// Construction de profils.

fn u16be(v: f32) -> [u8; 2] {
    ((v.clamp(0.0, 1.0) * 65535.0).round() as u16).to_be_bytes()
}

fn s15f16(v: f32) -> [u8; 4] {
    ((v * 65536.0).round() as i32).to_be_bytes()
}

fn identity_matrix() -> Vec<u8> {
    let mut t = Vec::new();
    for i in 0..9 {
        t.extend(s15f16(if i % 4 == 0 { 1.0 } else { 0.0 }));
    }
    t
}

/// Le XYZ sous D50 d'une couleur sRGB.
fn srgb_to_pcs(rgb: [f32; 3]) -> Vec3 {
    let m = mul(&bradford(SRGB.white, luma_color::D50), &rgb_to_xyz(&SRGB));
    apply(&m, rgb.map(srgb_decode))
}

fn pcs_to_srgb(xyz: Vec3) -> [f32; 3] {
    let m = mul(&bradford(SRGB.white, luma_color::D50), &rgb_to_xyz(&SRGB));
    apply(&inverse(&m).unwrap(), xyz).map(|v| srgb_encode(v.clamp(0.0, 1.0)))
}

/// Une lut16 3 → n, la grille remplie par `f`.
fn lut16(grid: usize, outputs: usize, f: impl Fn([f32; 3]) -> Vec<f32>) -> Vec<u8> {
    let mut t = b"mft2\0\0\0\0".to_vec();
    t.extend([3, outputs as u8, grid as u8, 0]);
    t.extend(identity_matrix());
    t.extend(2u16.to_be_bytes());
    t.extend(2u16.to_be_bytes());
    for _ in 0..3 {
        t.extend(u16be(0.0));
        t.extend(u16be(1.0));
    }
    let g = (grid - 1) as f32;
    for i in 0..grid {
        for j in 0..grid {
            for k in 0..grid {
                for v in f([i as f32 / g, j as f32 / g, k as f32 / g]) {
                    t.extend(u16be(v));
                }
            }
        }
    }
    for _ in 0..outputs {
        t.extend(u16be(0.0));
        t.extend(u16be(1.0));
    }
    t
}

/// Le Lab en codage lut16 (L* = 100 pour 0xFF00).
fn lab16(xyz: Vec3) -> Vec<f32> {
    let lab = xyz_to_lab(xyz);
    let k = 65280.0 / 65535.0;
    vec![
        lab[0] / 100.0 * k,
        (lab[1] + 128.0) / 255.0 * k,
        (lab[2] + 128.0) / 255.0 * k,
    ]
}

fn unlab16(v: [f32; 3]) -> Vec3 {
    let k = 65535.0 / 65280.0;
    lab_to_xyz([v[0] * k * 100.0, v[1] * k * 255.0 - 128.0, v[2] * k * 255.0 - 128.0])
}

fn xyz_tag(v: Vec3) -> Vec<u8> {
    let mut t = b"XYZ \0\0\0\0".to_vec();
    for c in v {
        t.extend(s15f16(c));
    }
    t
}

fn assemble(class: &[u8; 4], space: &[u8; 4], pcs: &[u8; 4], version: u8, tags: Vec<(&[u8; 4], Vec<u8>)>) -> Vec<u8> {
    let mut table = (tags.len() as u32).to_be_bytes().to_vec();
    let mut data = Vec::new();
    let mut offset = 128 + 4 + 12 * tags.len();
    for (sig, mut body) in tags {
        body.resize(body.len().next_multiple_of(4), 0);
        table.extend(sig);
        table.extend((offset as u32).to_be_bytes());
        table.extend((body.len() as u32).to_be_bytes());
        offset += body.len();
        data.extend(body);
    }
    let mut out = vec![0u8; 128];
    out[8] = version;
    out[12..16].copy_from_slice(class);
    out[16..20].copy_from_slice(space);
    out[20..24].copy_from_slice(pcs);
    out[36..40].copy_from_slice(b"acsp");
    out.extend(table);
    out.extend(data);
    let size = (out.len() as u32).to_be_bytes();
    out[..4].copy_from_slice(&size);
    out
}

#[test]
fn printer_tables_in_both_directions() {
    // Une « imprimante RVB » dont les tables sont celles du sRGB, sur un papier crème.
    let paper = [0.93, 0.96, 0.75];
    let a2b = lut16(17, 3, |rgb| lab16(srgb_to_pcs(rgb)));
    let b2a = lut16(33, 3, |lab| pcs_to_srgb(unlab16(lab)).to_vec());
    let mut desc = b"desc\0\0\0\0".to_vec();
    desc.extend(9u32.to_be_bytes());
    desc.extend(b"Brillant\0");
    desc.extend([0u8; 79]);
    let bytes = assemble(
        b"prtr",
        b"RGB ",
        b"Lab ",
        2,
        vec![
            (b"desc", desc),
            (b"wtpt", xyz_tag(paper)),
            (b"A2B0", a2b.clone()),
            (b"A2B1", a2b),
            (b"B2A0", b2a.clone()),
            (b"B2A1", b2a),
        ],
    );
    let p = Profile::parse(&bytes).unwrap();
    assert_eq!((p.class, p.description.as_str()), (Class::Output, "Brillant"));
    assert!(p.writes());
    assert_eq!(p.tables(), [Intent::Perceptual, Intent::Relative]);
    for rgb in [
        [0.2, 0.5, 0.8],
        [0.9, 0.1, 0.4],
        [0.5, 0.5, 0.5],
        [1.0, 1.0, 1.0],
        [0.1, 0.12, 0.08],
    ] {
        let xyz = p.to_pcs(&rgb, Intent::Relative).unwrap();
        let e = delta_e(xyz, srgb_to_pcs(rgb));
        assert!(e < 1.0, "{rgb:?} : ΔE {e}");
        // Aller et retour : un écart invisible (la table B→A échantillonne le Lab, où le
        // sRGB n'est pas linéaire).
        let back = p.from_pcs(xyz, Intent::Relative).unwrap();
        let e = delta_e(srgb_to_pcs([back[0], back[1], back[2]]), xyz);
        assert!(e < 1.5, "{rgb:?} → {:?} : ΔE {e}", &*back);
    }
    // Les mêmes tables dans un module de gestion des couleurs indépendant : (51, 128,
    // 204) revient en (44, 128, 204), et (230, 26, 102) en (230, 25, 102).
    for (rgb, want) in [
        ([51.0, 128.0, 204.0], [44.0, 128.0, 204.0]),
        ([230.0, 26.0, 102.0], [230.0, 25.0, 102.0]),
    ] {
        let xyz = srgb_to_pcs(rgb.map(|v: f32| v / 255.0));
        let back = p.from_pcs(xyz, Intent::Relative).unwrap();
        for i in 0..3 {
            assert!((back[i] * 255.0 - want[i]).abs() < 1.0, "{rgb:?} → {:?}", &*back);
        }
    }
    // L'absolue garde le ton du papier : le blanc du papier a sa couleur, et c'est sa
    // couleur (pas le blanc de l'espace de connexion) qui s'imprime sans encre.
    let white = p.to_pcs(&[1.0, 1.0, 1.0], Intent::Absolute).unwrap();
    for i in 0..3 {
        assert!((white[i] - paper[i]).abs() < 2e-3, "{white:?}");
    }
    let blank = p.from_pcs(paper, Intent::Absolute).unwrap();
    assert!(blank.iter().all(|v| *v > 0.995), "{:?}", &*blank);
    let d50 = p.from_pcs(luma_color::profile::PCS_WHITE, Intent::Absolute).unwrap();
    assert!(d50[0] < 0.99, "plus bleu que le papier : {:?}", &*d50);
    // La saturation, sans table propre, se rabat sur la perceptuelle.
    assert_eq!(
        p.to_pcs(&[0.3, 0.3, 0.3], Intent::Saturation),
        p.to_pcs(&[0.3, 0.3, 0.3], Intent::Perceptual)
    );
}

#[test]
fn cmyk_output_with_lut8() {
    // lut8 B→A : du Lab vers une encre noire seule, K = 1 − L*/100.
    let mut t = b"mft1\0\0\0\0".to_vec();
    t.extend([3, 4, 2, 0]);
    t.extend(identity_matrix());
    for _ in 0..3 {
        t.extend((0..=255u8).collect::<Vec<_>>());
    }
    for l in 0..2 {
        for _ in 0..4 {
            t.extend([0, 0, 0, if l == 0 { 255 } else { 0 }]);
        }
    }
    for _ in 0..4 {
        t.extend((0..=255u8).collect::<Vec<_>>());
    }
    // A→B : l'encre noire seule assombrit, les autres sont ignorées (grille 2⁴).
    let mut a = b"mft1\0\0\0\0".to_vec();
    a.extend([4, 3, 2, 0]);
    a.extend(identity_matrix());
    for _ in 0..4 {
        a.extend((0..=255u8).collect::<Vec<_>>());
    }
    for i in 0..16 {
        let k = i & 1;
        a.extend([if k == 1 { 0 } else { 255 }, 128, 128]);
    }
    for _ in 0..3 {
        a.extend((0..=255u8).collect::<Vec<_>>());
    }
    let bytes = assemble(b"prtr", b"CMYK", b"Lab ", 2, vec![(b"A2B0", a), (b"B2A0", t)]);
    let p = Profile::parse(&bytes).unwrap();
    let ink = p.from_pcs(lab_to_xyz([25.0, 0.0, 0.0]), Intent::Relative).unwrap();
    assert_eq!(ink.len(), 4);
    assert!(
        ink[..3].iter().all(|v| v.abs() < 1e-6) && (ink[3] - 0.75).abs() < 0.01,
        "{:?}",
        &*ink
    );
    let l = xyz_to_lab(p.to_pcs(&[0.0, 0.0, 0.0, 0.4], Intent::Relative).unwrap())[0];
    assert!((l - 60.0).abs() < 0.5, "{l}");
}

/// Les étapes d'une lutAtoB / lutBtoA en version 4 : courbes, matrice, courbes.
fn curves(curves: &[Vec<u8>]) -> Vec<u8> {
    let mut t = Vec::new();
    for c in curves {
        t.extend(c);
        t.resize(t.len().next_multiple_of(4), 0);
    }
    t
}

fn para_gamma(g: f32) -> Vec<u8> {
    let mut t = b"para\0\0\0\0\0\0\0\0".to_vec();
    t.extend(s15f16(g));
    t
}

fn modern(sig: &[u8; 4], b: Vec<u8>, matrix: Option<([[f32; 3]; 3], Vec3)>, m: Vec<u8>) -> Vec<u8> {
    let mut t = sig.to_vec();
    t.extend([0, 0, 0, 0, 3, 3, 0, 0]);
    let mut body = Vec::new();
    let mut at = 32;
    let mut place = |part: Vec<u8>| -> u32 {
        let o = at as u32;
        at += part.len();
        body.extend(part);
        o
    };
    let b_at = place(b);
    let mat_at = matrix.map_or(0, |(mat, off)| {
        let mut v = Vec::new();
        for row in mat {
            for c in row {
                v.extend(s15f16(c));
            }
        }
        for c in off {
            v.extend(s15f16(c));
        }
        place(v)
    });
    let m_at = place(m);
    for o in [b_at, mat_at, m_at, 0, 0] {
        t.extend(o.to_be_bytes());
    }
    t.extend(body);
    t
}

#[test]
fn version_4_matrix_tables() {
    // Le Display P3 en tables de version 4 (gamma 2,2 pour simplifier), XYZ codé sur
    // u1.15 : 1,0 vaut 0x8000.
    let enc = 32768.0 / 65535.0;
    let m = mul(
        &bradford(luma_color::DISPLAY_P3.white, luma_color::D50),
        &rgb_to_xyz(&luma_color::DISPLAY_P3),
    );
    let forward = m.map(|row| row.map(|v| v * enc));
    let back = inverse(&forward).unwrap();
    let identity = curves(&[para_gamma(1.0), para_gamma(1.0), para_gamma(1.0)]);
    let a2b = modern(
        b"mAB ",
        identity.clone(),
        Some((forward, [0.0; 3])),
        curves(&[para_gamma(2.2), para_gamma(2.2), para_gamma(2.2)]),
    );
    let b2a = modern(
        b"mBA ",
        identity,
        Some((back, [0.0; 3])),
        curves(&[para_gamma(1.0 / 2.2), para_gamma(1.0 / 2.2), para_gamma(1.0 / 2.2)]),
    );
    let mut mluc = b"mluc\0\0\0\0".to_vec();
    mluc.extend(2u32.to_be_bytes());
    mluc.extend(12u32.to_be_bytes());
    let fr: Vec<u8> = "Écran large".encode_utf16().flat_map(u16::to_be_bytes).collect();
    let en: Vec<u8> = "Wide display".encode_utf16().flat_map(u16::to_be_bytes).collect();
    mluc.extend(b"frFR");
    mluc.extend((fr.len() as u32).to_be_bytes());
    mluc.extend(40u32.to_be_bytes());
    mluc.extend(b"enUS");
    mluc.extend((en.len() as u32).to_be_bytes());
    mluc.extend((40 + fr.len() as u32).to_be_bytes());
    mluc.extend(fr);
    mluc.extend(en);
    let bytes = assemble(
        b"mntr",
        b"RGB ",
        b"XYZ ",
        4,
        vec![(b"desc", mluc), (b"A2B0", a2b), (b"B2A0", b2a)],
    );
    let p = Profile::parse(&bytes).unwrap();
    assert_eq!((p.version.0, p.description.as_str()), (4, "Wide display"));
    for rgb in [[0.2f32, 0.5, 0.8], [0.9, 0.1, 0.4], [1.0, 1.0, 1.0]] {
        let xyz = p.to_pcs(&rgb, Intent::Perceptual).unwrap();
        let want = apply(&m, rgb.map(|v| v.powf(2.2)));
        for i in 0..3 {
            assert!((xyz[i] - want[i]).abs() < 1e-3, "{rgb:?} {xyz:?} {want:?}");
        }
        let back = p.from_pcs(xyz, Intent::Perceptual).unwrap();
        for i in 0..3 {
            assert!((back[i] - rgb[i]).abs() < 2e-3, "{rgb:?} → {back:?}");
        }
    }
}

#[test]
fn not_a_profile() {
    assert!(Profile::parse(b"").is_err());
    assert!(Profile::parse(&[0u8; 200]).is_err());
    // Une table annoncée plus grande que le fichier.
    let mut huge = lut16(2, 3, |_| vec![0.5; 3]);
    huge[10] = 255;
    let bytes = assemble(b"prtr", b"RGB ", b"Lab ", 2, vec![(b"A2B0", huge)]);
    assert!(Profile::parse(&bytes).is_err());
}

#[test]
fn proofing_follows_the_intent() {
    use luma_color::proof::Proof;
    // Une imprimante dont la table perceptuelle réduit la saturation de moitié, et dont
    // la relative la garde : l'épreuvage montre l'une ou l'autre.
    let a2b = lut16(17, 3, |rgb| lab16(srgb_to_pcs(rgb)));
    let relative = lut16(33, 3, |lab| pcs_to_srgb(unlab16(lab)).to_vec());
    let perceptual = lut16(33, 3, |lab| {
        let k = 65535.0 / 65280.0;
        let mid = 128.0 / 255.0 / k;
        pcs_to_srgb(unlab16([
            lab[0],
            mid + (lab[1] - mid) * 0.5,
            mid + (lab[2] - mid) * 0.5,
        ]))
        .to_vec()
    });
    let bytes = assemble(
        b"prtr",
        b"RGB ",
        b"Lab ",
        2,
        vec![
            (b"A2B0", a2b.clone()),
            (b"A2B1", a2b),
            (b"B2A0", perceptual),
            (b"B2A1", relative),
        ],
    );
    let p = Profile::parse(&bytes).unwrap();
    let chroma = |rgb: [f32; 3]| {
        let lab = xyz_to_lab(srgb_to_pcs(rgb));
        lab[1].hypot(lab[2])
    };
    let red = [0.85, 0.2, 0.15];
    let rel = Proof::new(&p, Intent::Relative).unwrap().apply(red);
    let per = Proof::new(&p, Intent::Perceptual).unwrap().apply(red);
    assert!((chroma(rel) - chroma(red)).abs() < 2.0, "{rel:?}");
    let ratio = chroma(per) / chroma(red);
    assert!((ratio - 0.5).abs() < 0.06, "perceptuelle : {per:?}, rapport {ratio}");
}

#[test]
fn paper_and_black_ink() {
    use luma_color::proof::{Media, Proof};
    // Un papier mat : son noir ne descend qu'à L* 15, son blanc est crème.
    let (black, span) = (15.0, 85.0);
    let a2b = lut16(17, 3, |rgb| {
        let lab = xyz_to_lab(srgb_to_pcs(rgb));
        let xyz = lab_to_xyz([black + lab[0] * span / 100.0, lab[1], lab[2]]);
        lab16(xyz)
    });
    let b2a = lut16(33, 3, |v| {
        let lab = xyz_to_lab(unlab16(v));
        let l = ((lab[0] - black) / span * 100.0).clamp(0.0, 100.0);
        pcs_to_srgb(lab_to_xyz([l, lab[1], lab[2]])).to_vec()
    });
    let paper = [0.90, 0.93, 0.70];
    let bytes = assemble(
        b"prtr",
        b"RGB ",
        b"Lab ",
        2,
        vec![(b"wtpt", xyz_tag(paper)), (b"A2B0", a2b), (b"B2A0", b2a)],
    );
    let p = Profile::parse(&bytes).unwrap();
    let proof = |media| Proof::with_media(&p, Intent::Relative, media).unwrap();

    // Par défaut : le noir et le blanc de l'écran, le gris moyen à peine déplacé.
    let plain = proof(Media::default());
    for (c, want) in [(0.0, 0.0), (1.0, 1.0), (0.5, 0.5)] {
        let v = plain.apply([c; 3]);
        for x in v {
            assert!((x - want).abs() < 0.04, "{c} → {v:?}");
        }
    }
    // L'encre noire : le noir imprimé est un gris foncé (L* 15), le blanc reste blanc.
    let ink = proof(Media {
        paper: false,
        ink: true,
    });
    let k = ink.apply([0.0; 3]);
    let l = xyz_to_lab(srgb_to_pcs(k))[0];
    assert!((l - black).abs() < 1.5, "noir de l'encre : L* {l}");
    assert!(ink.apply([1.0; 3]).iter().all(|v| *v > 0.99));
    // Le papier : son blanc est crème (moins de bleu), plus sombre que l'écran, et le
    // noir de l'encre suit.
    let on_paper = proof(Media {
        paper: true,
        ink: false,
    });
    let w = on_paper.apply([1.0; 3]);
    assert!(w[2] < w[0] - 0.04 && w[1] < 0.99, "blanc du papier : {w:?}");
    assert!(on_paper.apply([0.0; 3])[0] > 0.1);
    // L'alerte de gamut ne dépend pas de ce que l'écran montre du papier.
    for c in [[0.9, 0.1, 0.1], [0.5, 0.5, 0.5], [0.0, 0.0, 0.0]] {
        assert_eq!(plain.out_of_gamut(c), on_paper.out_of_gamut(c), "{c:?}");
    }
    assert!(
        plain.out_of_gamut([0.0, 0.0, 0.0]),
        "le noir de l'écran est hors de portée du papier mat"
    );
}
