//! Les LUT 3D (`.cube`, le format texte courant des LUT) : un film mesuré sur
//! une vraie pellicule (voir `luma film-lut`) ou une LUT importée. Elle agit après la
//! couleur, en sRGB encodé (l'espace des LUT créatives), avant le grain et le
//! vignetage.
//!
//! Les LUT sont rangées dans un registre, par nom : les réglages (et les films des
//! masques) ne gardent que le nom et l'intensité ; l'application charge les fichiers
//! `.cube` de son dossier Films au démarrage.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use luma_color::{self as lc, EncodeLut, Mat3};
use luma_core::{Error, Result, RgbImage};
use luma_project::Settings;
use rayon::prelude::*;

use crate::curve::Lut;

/// Une LUT 3D : `size`³ couleurs, le rouge variant le plus vite (ordre du `.cube`).
#[derive(Clone, Debug, PartialEq)]
pub struct Cube {
    pub title: String,
    pub size: usize,
    pub data: Vec<[f32; 3]>,
    /// Le domaine d'entrée (0 à 1 en général).
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
}

impl Cube {
    /// La LUT qui ne change rien.
    pub fn identity(size: usize) -> Cube {
        let n = size.max(2);
        let k = 1.0 / (n - 1) as f32;
        let data = (0..n * n * n)
            .map(|i| [(i % n) as f32 * k, (i / n % n) as f32 * k, (i / (n * n)) as f32 * k])
            .collect();
        Cube {
            title: String::new(),
            size: n,
            data,
            domain_min: [0.0; 3],
            domain_max: [1.0; 3],
        }
    }

    /// Lit un fichier `.cube` (LUT 3D ; les LUT 1D ne sont pas des films).
    pub fn parse(text: &str) -> Result<Cube> {
        let bad = |m: String| Error::Format(format!("LUT .cube : {m}"));
        let mut cube = Cube {
            title: String::new(),
            size: 0,
            data: Vec::new(),
            domain_min: [0.0; 3],
            domain_max: [1.0; 3],
        };
        let triple = |parts: &[&str]| -> Option<[f32; 3]> {
            let v: Vec<f32> = parts.iter().filter_map(|p| p.parse().ok()).collect();
            (v.len() == 3 && parts.len() == 3 && v.iter().all(|x| x.is_finite())).then(|| [v[0], v[1], v[2]])
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts[0] {
                "TITLE" => cube.title = line[5..].trim().trim_matches('"').to_string(),
                "LUT_3D_SIZE" => {
                    cube.size = parts
                        .get(1)
                        .and_then(|v| v.parse().ok())
                        .filter(|n| (2..=256).contains(n))
                        .ok_or_else(|| bad(format!("taille invalide : {line}")))?;
                }
                "LUT_1D_SIZE" => return Err(bad("LUT 1D, pas 3D".into())),
                "DOMAIN_MIN" => cube.domain_min = triple(&parts[1..]).ok_or_else(|| bad(line.into()))?,
                "DOMAIN_MAX" => cube.domain_max = triple(&parts[1..]).ok_or_else(|| bad(line.into()))?,
                _ if parts[0].chars().next().is_some_and(|c| c.is_ascii_alphabetic()) => {
                    // Mots‑clés facultatifs (LUT_3D_INPUT_RANGE…) : ignorés.
                }
                _ => cube
                    .data
                    .push(triple(&parts).ok_or_else(|| bad(format!("ligne illisible : {line}")))?),
            }
        }
        if cube.size == 0 {
            return Err(bad("LUT_3D_SIZE manquant".into()));
        }
        if cube.data.len() != cube.size.pow(3) {
            return Err(bad(format!(
                "{} couleurs pour une taille {} ({} attendues)",
                cube.data.len(),
                cube.size,
                cube.size.pow(3)
            )));
        }
        if (0..3).any(|c| cube.domain_max[c] <= cube.domain_min[c]) {
            return Err(bad("domaine vide".into()));
        }
        Ok(cube)
    }

    /// Le texte `.cube`.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        if !self.title.is_empty() {
            out += &format!("TITLE \"{}\"\n", self.title.replace('"', "'"));
        }
        out += "# LÜMA\n";
        out += &format!("LUT_3D_SIZE {}\n", self.size);
        if self.domain_min != [0.0; 3] || self.domain_max != [1.0; 3] {
            let [a, b, c] = self.domain_min;
            out += &format!("DOMAIN_MIN {a} {b} {c}\n");
            let [a, b, c] = self.domain_max;
            out += &format!("DOMAIN_MAX {a} {b} {c}\n");
        }
        for p in &self.data {
            out += &format!("{:.6} {:.6} {:.6}\n", p[0], p[1], p[2]);
        }
        out
    }

    /// La couleur de la LUT pour `rgb` (encodé, dans le domaine), par interpolation
    /// tétraédrique : le gris reste sur l'axe des gris, sans les teintes parasites de
    /// l'interpolation trilinéaire.
    pub fn sample(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = self.size;
        let m = (n - 1) as f32;
        let mut i = [0usize; 3];
        let mut f = [0.0f32; 3];
        for c in 0..3 {
            let t = ((rgb[c] - self.domain_min[c]) / (self.domain_max[c] - self.domain_min[c])).clamp(0.0, 1.0) * m;
            let k = (t as usize).min(n - 2);
            i[c] = k;
            f[c] = t - k as f32;
        }
        let at = |dr: usize, dg: usize, db: usize| self.data[(i[0] + dr) + (i[1] + dg) * n + (i[2] + db) * n * n];
        tetrahedral(f, at)
    }
}

/// L'interpolation tétraédrique dans une cellule : `f` la position (0 à 1 sur chaque
/// axe), `at` les huit sommets. Le même calcul que le shader.
fn tetrahedral(f: [f32; 3], at: impl Fn(usize, usize, usize) -> [f32; 3]) -> [f32; 3] {
    let [fr, fg, fb] = f;
    let c000 = at(0, 0, 0);
    let c111 = at(1, 1, 1);
    // Les quatre sommets du tétraèdre qui contient le point, et leurs poids.
    let (a, b, wa, wb, wc) = if fr >= fg {
        if fg >= fb {
            (at(1, 0, 0), at(1, 1, 0), fr - fg, fg - fb, fb)
        } else if fr >= fb {
            (at(1, 0, 0), at(1, 0, 1), fr - fb, fb - fg, fg)
        } else {
            (at(0, 0, 1), at(1, 0, 1), fb - fr, fr - fg, fg)
        }
    } else if fb >= fg {
        (at(0, 0, 1), at(0, 1, 1), fb - fg, fg - fr, fr)
    } else if fb >= fr {
        (at(0, 1, 0), at(0, 1, 1), fg - fb, fb - fr, fr)
    } else {
        (at(0, 1, 0), at(1, 1, 0), fg - fr, fr - fb, fb)
    };
    let w0 = 1.0 - wa - wb - wc;
    std::array::from_fn(|c| w0 * c000[c] + wa * a[c] + wb * b[c] + wc * c111[c])
}

fn registry() -> &'static RwLock<HashMap<String, Arc<Cube>>> {
    static REGISTRY: std::sync::OnceLock<RwLock<HashMap<String, Arc<Cube>>>> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}

/// Range une LUT sous `name` (remplace celle du même nom).
pub fn register(name: &str, cube: Cube) {
    registry()
        .write()
        .expect("registre des LUT")
        .insert(name.to_string(), Arc::new(cube));
}

/// La LUT de ce nom, si elle est chargée.
pub fn get(name: &str) -> Option<Arc<Cube>> {
    registry().read().expect("registre des LUT").get(name).cloned()
}

/// Les noms des LUT chargées, triés.
pub fn names() -> Vec<String> {
    let mut v: Vec<String> = registry().read().expect("registre des LUT").keys().cloned().collect();
    v.sort_unstable();
    v
}

/// Charge un fichier `.cube` et le range sous son nom de fichier (sans extension).
pub fn load(path: &std::path::Path) -> Result<String> {
    let text = std::fs::read_to_string(path)?;
    let cube = Cube::parse(&text).map_err(|e| Error::Format(format!("{} : {e}", path.display())))?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "LUT".into());
    register(&name, cube);
    Ok(name)
}

/// Charge toutes les LUT `.cube` d'un dossier ; rend les noms chargés.
pub fn load_dir(dir: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cube")))
        .filter_map(|p| load(&p).ok())
        .collect();
    names.sort_unstable();
    names
}

/// Ce qu'il faut pour appliquer la LUT des réglages, s'il y en a une : la LUT, son
/// intensité (0 à 1) et les tables communes au processeur et au GPU.
pub struct Stage {
    pub cube: Arc<Cube>,
    pub amount: f32,
    /// Travail → sRGB linéaire, et retour.
    pub to_srgb: Mat3,
    pub from_srgb: Mat3,
    encode: EncodeLut,
    decode: Lut,
}

impl Stage {
    pub fn new(s: &Settings) -> Option<Stage> {
        let look = s.lut.as_ref()?;
        let amount = (look.amount / 100.0).clamp(0.0, 1.0);
        let cube = get(&look.name)?;
        (amount > 0.0).then(|| Stage {
            cube,
            amount,
            to_srgb: lc::rgb_to_rgb(&lc::WORKING, &lc::SRGB),
            from_srgb: lc::rgb_to_rgb(&lc::SRGB, &lc::WORKING),
            encode: EncodeLut::new(lc::srgb_encode),
            decode: Lut::from_fn(lc::srgb_decode),
        })
    }

    #[inline]
    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        // Vers le sRGB, dans son gamut (rapprochement du gris, comme à l'export).
        let lin = crate::to_output(&self.to_srgb, p);
        let enc = lin.map(|v| self.encode.get(v));
        let out = self.cube.sample(enc).map(|v| self.decode.get(v.clamp(0.0, 1.0)));
        let film = lc::apply(&self.from_srgb, out);
        std::array::from_fn(|c| p[c] + (film[c] - p[c]) * self.amount)
    }
}

/// Applique la LUT des réglages (rien si elle n'est pas chargée).
pub fn apply(img: &mut RgbImage, s: &Settings) {
    if let Some(stage) = Stage::new(s) {
        img.data.par_iter_mut().for_each(|p| *p = stage.apply(*p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_writes_and_samples_like_resolve() {
        let text = "TITLE \"Essai\"\n# commentaire\nLUT_3D_SIZE 2\n\
            0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";
        let c = Cube::parse(text).unwrap();
        assert_eq!((c.title.as_str(), c.size), ("Essai", 2));
        assert_eq!(c.data, Cube::identity(2).data);
        assert_eq!(Cube::parse(&c.to_text()).unwrap(), c);
        // L'identité rend l'entrée, partout dans le cube.
        let id = Cube::identity(17);
        for p in [[0.3, 0.7, 0.1], [0.95, 0.02, 0.5], [0.5, 0.5, 0.5]] {
            let q = id.sample(p);
            assert!((0..3).all(|k| (q[k] - p[k]).abs() < 1e-5), "{q:?}");
        }
        // Une LUT qui inverse : 1 − x.
        let mut inv = Cube::identity(9);
        inv.data.iter_mut().for_each(|p| *p = p.map(|v| 1.0 - v));
        let q = inv.sample([0.2, 0.6, 0.9]);
        assert!((q[0] - 0.8).abs() < 1e-5 && (q[1] - 0.4).abs() < 1e-5 && (q[2] - 0.1).abs() < 1e-5);
        assert!(Cube::parse("LUT_3D_SIZE 2\n0 0 0\n").is_err());
        assert!(Cube::parse("LUT_1D_SIZE 2\n0 0 0\n1 1 1\n").is_err());
    }

    #[test]
    fn the_identity_lut_leaves_the_image_and_a_real_one_acts() {
        let mut s = Settings::default();
        register("Identité (essai)", Cube::identity(33));
        s.lut = Some(luma_project::Look {
            name: "Identité (essai)".into(),
            amount: 100.0,
        });
        let stage = Stage::new(&s).unwrap();
        // Des couleurs du gamut sRGB (celui des LUT) : elles reviennent telles quelles.
        let from_srgb = lc::rgb_to_rgb(&lc::SRGB, &lc::WORKING);
        for q in [[0.18, 0.18, 0.18], [0.5, 0.2, 0.05], [0.02, 0.04, 0.3]] {
            let p = lc::apply(&from_srgb, q);
            let q = stage.apply(p);
            assert!((0..3).all(|k| (q[k] - p[k]).abs() < 2e-3), "{p:?} → {q:?}");
        }
        // Une vraie LUT agit, dans l'image développée, à son intensité.
        let mut inv = Cube::identity(9);
        inv.data.iter_mut().for_each(|p| *p = p.map(|v| 1.0 - v));
        register("Inverse (essai)", inv);
        let gray = |amount: f32| {
            let s = Settings {
                lut: Some(luma_project::Look {
                    name: "Inverse (essai)".into(),
                    amount,
                }),
                ..Settings::default()
            };
            let mut img = RgbImage::from_fn(2, 2, |_, _| [0.05, 0.05, 0.05]);
            crate::finish(
                &mut img,
                1.0,
                &s,
                &luma_core::Cancel::new(),
                &mut luma_core::Timings::default(),
            )
            .unwrap();
            img.data[0][1]
        };
        let (none, half, full) = (gray(0.0), gray(50.0), gray(100.0));
        assert!(full > 0.3 && none < 0.1, "{none} {full}");
        assert!((half - (none + full) / 2.0).abs() < 1e-4);
        // Une LUT inconnue ne fait rien (le fichier manque sur cette machine).
        s.lut = Some(luma_project::Look {
            name: "Introuvable".into(),
            amount: 100.0,
        });
        assert!(Stage::new(&s).is_none());
    }
}
