//! Couleur : Vibrance, Saturation, Mélangeur de couleurs (TSL), Étalonnage des couleurs
//! et Étalonnage de l'appareil photo.
//!
//! La vibrance, la saturation, le mélangeur et l'étalonnage travaillent en OKLCh, un
//! espace perceptuel : tourner une teinte ou changer une saturation n'y modifie pas la
//! clarté perçue. Les huit couleurs du mélangeur et les teintes des roues sont les huit
//! teintes usuelles (rouge 0°, orange 30°, jaune 60°, vert 120°, bleu‑vert 180°, bleu 240°,
//! violet 270°, magenta 300°), placées dans OKLCh.

use luma_color::{self as color, Mat3, OkLab, Vec3};
use luma_project::{Calibration, Profile, Settings, Wheel};

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Les centres des huit bandes du mélangeur, croissants sur le cercle à partir du rouge.
fn band_centers(ok: &OkLab) -> [f32; 8] {
    let mut centers = BAND_DEGREES.map(|d| wheel_hue(ok, d));
    for i in 1..8 {
        while centers[i] < centers[i - 1] {
            centers[i] += std::f32::consts::TAU;
        }
    }
    centers
}

/// Les deux bandes qui encadrent la teinte `h`, et le poids de la première.
fn bands_around(centers: &[f32; 8], h: f32) -> (usize, usize, f32) {
    let tau = std::f32::consts::TAU;
    let mut h = h;
    while h < centers[0] {
        h += tau;
    }
    while h >= centers[0] + tau {
        h -= tau;
    }
    for i in 0..8 {
        let (c0, c1) = (centers[i], if i == 7 { centers[0] + tau } else { centers[i + 1] });
        if h < c1 {
            // Poids lisse, sans trigonométrie (même allure que cos²).
            let w = 1.0 - smoothstep(0.0, 1.0, (h - c0) / (c1 - c0));
            return (i, (i + 1) % 8, w);
        }
    }
    (7, 0, 1.0)
}

/// L'outil de ciblage direct : les bandes du mélangeur qu'une couleur affichée (sRGB,
/// 0 à 255) touche, avec leur poids (somme 1), comme le rendu les pondère. `None` pour
/// un gris, que le mélangeur ne touche pas.
pub fn mixer_bands_at(srgb: [u8; 3]) -> Option<[(usize, f32); 2]> {
    let lin = srgb.map(|v| color::srgb_decode(f32::from(v) / 255.0));
    let to_working = color::rgb_to_rgb(&color::SRGB, &color::WORKING);
    let ok = OkLab::for_working();
    let [_, a, b] = ok.from_rgb(color::apply(&to_working, lin));
    // Une teinte à peine visible suffit (un vert de pin terne, un ciel blanc bleuté).
    if (a * a + b * b).sqrt() < 0.005 {
        return None;
    }
    let (i, j, w) = bands_around(&band_centers(&ok), color::fast_atan2(b, a));
    Some([(i, w), (j, 1.0 - w)])
}

/// La teinte OKLCh (radians) d'une teinte de roue chromatique (degrés, TSV du sRGB).
pub fn wheel_hue(ok: &OkLab, degrees: f32) -> f32 {
    let h = degrees.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    let srgb_to_working = color::rgb_to_rgb(&color::SRGB, &color::WORKING);
    let lin = color::apply(
        &srgb_to_working,
        [color::srgb_decode(r), color::srgb_decode(g), color::srgb_decode(b)],
    );
    let lab = ok.from_rgb(lin);
    lab[2].atan2(lab[1])
}

const BAND_DEGREES: [f32; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 300.0];

/// Les réglages de couleur, préparés une fois par rendu.
pub struct ColorStage {
    ok: OkLab,
    saturation: f32,
    /// Chroma ajouté par le profil de rendu (0 en noir et blanc).
    profile_chroma: f32,
    mono: bool,
    simple: bool,
    chroma_only: bool,
    vibrance: f32,
    skin_hue: f32,
    /// Centres des bandes (radians, croissants) et réglages TSL de chaque bande.
    centers: [f32; 8],
    hsl: [[f32; 3]; 8],
    grading: Option<Grading>,
}

struct Grading {
    wheels: [(f32, f32, f32); 4],
    mid: f32,
    blend: f32,
}

fn wheel_offset(ok: &OkLab, w: &Wheel) -> (f32, f32, f32) {
    let h = wheel_hue(ok, w.hue);
    let s = w.saturation / 100.0 * 0.07;
    (s * h.cos(), s * h.sin(), w.luminance / 100.0)
}

/// Les décalages (a, b, L) des quatre roues de l'étalonnage, son milieu et sa fusion.
pub type GradingParts = ([(f32, f32, f32); 4], f32, f32);

/// Les paramètres préparés de [`ColorStage`], pour refaire le même calcul ailleurs
/// (shaders du GPU).
pub struct ColorParts {
    pub ok: OkLab,
    pub simple: bool,
    pub saturation: f32,
    pub profile_chroma: f32,
    pub mono: bool,
    pub vibrance: f32,
    pub skin_hue: f32,
    pub centers: [f32; 8],
    pub hsl: [[f32; 3]; 8],
    pub grading: Option<GradingParts>,
}

impl ColorStage {
    pub fn parts(&self) -> ColorParts {
        ColorParts {
            ok: self.ok,
            simple: self.simple,
            saturation: self.saturation,
            profile_chroma: self.profile_chroma,
            mono: self.mono,
            vibrance: self.vibrance,
            skin_hue: self.skin_hue,
            centers: self.centers,
            hsl: self.hsl,
            grading: self.grading.as_ref().map(|g| (g.wheels, g.mid, g.blend)),
        }
    }

    /// `None` si aucun réglage de couleur n'est actif (passe sautée).
    pub fn new(s: &Settings) -> Option<Self> {
        let g = &s.color_grading;
        let wheels = [&g.shadows, &g.midtones, &g.highlights, &g.global];
        let grading_active = wheels.iter().any(|w| w.saturation != 0.0 || w.luminance != 0.0);
        let hsl_active = s
            .hsl
            .hue
            .iter()
            .chain(&s.hsl.saturation)
            .chain(&s.hsl.luminance)
            .any(|v| *v != 0.0);
        let chroma = s.profile.look().1;
        if s.saturation == 0.0 && s.vibrance == 0.0 && !hsl_active && !grading_active && chroma == 1.0 {
            return None;
        }
        let ok = OkLab::for_working();
        let centers = band_centers(&ok);
        let mut hsl = [[0.0; 3]; 8];
        for (i, band) in hsl.iter_mut().enumerate() {
            *band = [
                s.hsl.hue[i] / 100.0,
                s.hsl.saturation[i] / 100.0,
                s.hsl.luminance[i] / 100.0,
            ];
        }
        let grading = grading_active.then(|| Grading {
            wheels: wheels.map(|w| wheel_offset(&ok, w)),
            mid: 0.5 - g.balance / 100.0 * 0.25,
            blend: g.blending / 100.0,
        });
        let mut stage = Self {
            skin_hue: wheel_hue(&ok, 25.0),
            ok,
            saturation: s.saturation / 100.0,
            profile_chroma: chroma,
            mono: s.profile == Profile::Monochrome,
            vibrance: s.vibrance / 100.0,
            centers,
            hsl,
            grading,
            simple: false,
            chroma_only: false,
        };
        stage.simple = stage.is_simple();
        stage.chroma_only = stage.is_chroma_only();
        Some(stage)
    }

    /// Les deux bandes qui encadrent une teinte, et le poids de la première.
    #[inline]
    fn bands(&self, h: f32) -> (usize, usize, f32) {
        bands_around(&self.centers, h)
    }

    /// L'écart angulaire entre une bande et sa voisine (sens positif ou négatif).
    fn gap(&self, i: usize, forward: bool) -> f32 {
        let tau = std::f32::consts::TAU;
        if forward {
            let next = if i == 7 {
                self.centers[0] + tau
            } else {
                self.centers[i + 1]
            };
            next - self.centers[i]
        } else {
            let prev = if i == 0 {
                self.centers[7] - tau
            } else {
                self.centers[i - 1]
            };
            self.centers[i] - prev
        }
    }

    /// Seuls le profil et la Saturation agissent : un simple gain sur (a, b).
    fn is_simple(&self) -> bool {
        self.vibrance == 0.0 && self.grading.is_none() && !self.mono && self.hsl.iter().all(|b| *b == [0.0; 3])
    }

    /// Seuls le profil, la Saturation et la Vibrance agissent : le chroma change, la
    /// teinte non (pas de mélangeur, d'étalonnage ni de noir et blanc).
    fn is_chroma_only(&self) -> bool {
        self.grading.is_none() && !self.mono && self.hsl.iter().all(|b| *b == [0.0; 3])
    }

    /// Le gain de chroma de la Vibrance et de la Saturation, pour un pixel de chroma `c0`
    /// et de teinte `h` : le calcul de [`Self::apply`], dans le même ordre.
    #[inline]
    fn chroma_gain(&self, a: f32, b: f32) -> f32 {
        let c0 = (a * a + b * b).sqrt();
        let mut c = c0;
        if self.vibrance != 0.0 {
            let h = color::fast_atan2(b, a);
            let mut d = h - self.skin_hue;
            if d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            } else if d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            let skin = (1.0 - (d / 0.8).powi(2)).max(0.0).powi(2);
            let w = (1.0 - smoothstep(0.0, 0.2, c)) * (1.0 - 0.6 * skin);
            c *= 1.0 + self.vibrance * w;
        }
        c *= (1.0 + self.saturation) * self.profile_chroma;
        if c0 > 1e-9 { c / c0 } else { 0.0 }
    }

    /// [`Self::apply`] sur `px` (au plus [`crate::LANES`] pixels) : les passages vers
    /// OKLab et retour en colonnes, plusieurs fois plus vite, mêmes bits. Quand seul le
    /// chroma change (le profil, la Saturation, la Vibrance), tout le calcul ; sinon, le
    /// réglage de chaque pixel entre les deux ([`Self::adjust`]). Un bloc incomplet passe
    /// pixel par pixel.
    pub fn apply_lanes(&self, px: &mut [Vec3]) {
        const N: usize = crate::LANES;
        if px.len() < N {
            for p in px.iter_mut() {
                *p = self.apply(*p);
            }
            return;
        }
        let ([to_lms, from_lms, lms_to_lab, lab_to_lms], white) = self.ok.parts();
        let c = crate::columns(px);
        // Vers OKLab, huit pixels à la fois (les calculs de `OkLab::from_rgb`).
        let mut lab = [[0.0f32; 3]; N];
        for (i, out) in lab.iter_mut().enumerate() {
            let lms = color::apply(&to_lms, [c[0][i], c[1][i], c[2][i]]);
            let v = color::apply(&lms_to_lab, lms.map(color::fast_cbrt));
            *out = [v[0], v[1] - v[0] * white.0, v[2] - v[0] * white.1];
        }
        // Le gain de chroma de chacun ; ou, pour les autres réglages, chaque pixel réglé
        // dans OKLab (gain 1 : le retour ne change rien de plus).
        let mut k = [0.0f32; N];
        if self.simple {
            k = [(1.0 + self.saturation) * self.profile_chroma; N];
        } else if self.chroma_only {
            for (g, [_, a, b]) in k.iter_mut().zip(&lab) {
                *g = self.chroma_gain(*a, *b);
            }
        } else {
            for (g, v) in k.iter_mut().zip(&mut lab) {
                *v = self.adjust(*v);
                *g = 1.0;
            }
        }
        // Et retour (les calculs de `OkLab::to_rgb`).
        let mut out = [[0.0f32; N]; 3];
        for i in 0..N {
            let [l, a, b] = lab[i];
            let q = color::apply(&lab_to_lms, [l, a * k[i] + l * white.0, b * k[i] + l * white.1]);
            let rgb = color::apply(&from_lms, q.map(|v| v * v * v));
            for ch in 0..3 {
                out[ch][i] = rgb[ch];
            }
        }
        for (i, p) in px.iter_mut().enumerate() {
            *p = [out[0][i], out[1][i], out[2][i]];
        }
    }

    #[inline]
    pub fn apply(&self, rgb: Vec3) -> Vec3 {
        if self.simple {
            let [l, a, b] = self.ok.from_rgb(rgb);
            let k = (1.0 + self.saturation) * self.profile_chroma;
            return self.ok.to_rgb([l, a * k, b * k]);
        }
        self.ok.to_rgb(self.adjust(self.ok.from_rgb(rgb)))
    }

    /// Les réglages de couleur d'un pixel dans OKLab : Vibrance, Saturation, mélangeur,
    /// noir et blanc, étalonnage.
    #[inline]
    fn adjust(&self, [mut l, a, b]: Vec3) -> Vec3 {
        let c0 = (a * a + b * b).sqrt();
        let mut c = c0;
        let h = color::fast_atan2(b, a);
        let mut dh = 0.0;
        let chroma0 = c;

        // Vibrance (couleurs ternes surtout, tons chair épargnés) et Saturation.
        if self.vibrance != 0.0 {
            // Proximité des tons chair : écart d'angle ramené à [−π, π], cloche douce.
            let mut d = h - self.skin_hue;
            if d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            } else if d < -std::f32::consts::PI {
                d += std::f32::consts::TAU;
            }
            let skin = (1.0 - (d / 0.8).powi(2)).max(0.0).powi(2);
            let w = (1.0 - smoothstep(0.0, 0.2, c)) * (1.0 - 0.6 * skin);
            c *= 1.0 + self.vibrance * w;
        }
        c *= (1.0 + self.saturation) * self.profile_chroma;

        // Mélangeur : chaque bande pèse selon la proximité de sa teinte.
        let (i, j, wi) = self.bands(h);
        let wj = 1.0 - wi;
        let (bi, bj) = (self.hsl[i], self.hsl[j]);
        if bi != [0.0; 3] || bj != [0.0; 3] {
            let shift = |band: usize, v: f32| v * 0.5 * self.gap(band, v > 0.0);
            dh = wi * shift(i, bi[0]) + wj * shift(j, bj[0]);
            c *= (1.0 + wi * bi[1] + wj * bj[1]).max(0.0);
            // La luminance ne touche que les pixels colorés : le gris reste gris.
            let colored = smoothstep(0.01, 0.06, chroma0);
            // En noir et blanc, la luminance du mélangeur devient le mélange noir et blanc,
            // plus ample : L ×2 ou ÷2, soit ±3 IL, la force d'un filtre
            // rouge qui noircit un ciel bleu.
            let gain = if self.mono { 1.0 } else { 0.6 };
            l *= (gain * (wi * bi[2] + wj * bj[2]) * colored).exp2();
        }
        if self.mono {
            c = 0.0;
        }
        // Pas de trigonométrie par pixel : (a, b) est mis à l'échelle, et tourné
        // seulement si une teinte du mélangeur a bougé.
        let k = if c0 > 1e-9 { c / c0 } else { 0.0 };
        let (mut a, mut b) = (a * k, b * k);
        if dh != 0.0 {
            let (s, co) = dh.sin_cos();
            (a, b) = (a * co - b * s, a * s + b * co);
        }

        // Étalonnage : ombres, tons moyens, hautes lumières, global.
        if let Some(g) = &self.grading {
            let width = 0.2 + 0.5 * g.blend;
            let sh = 1.0 - smoothstep(0.0, g.mid + width * 0.5, l);
            let hi = smoothstep(g.mid - width * 0.5, 1.0, l);
            let mid = (-((l - g.mid) / (0.12 + 0.25 * g.blend)).powi(2)).exp();
            let weights = [sh, mid, hi, 1.0];
            let floor = smoothstep(0.0, 0.08, l);
            for (w, (da, db, dl)) in weights.iter().zip(g.wheels) {
                a += w * da * floor;
                b += w * db * floor;
                l += w * dl * 0.12;
            }
            l = l.max(0.0);
        }
        [l, a, b]
    }
}

/// La matrice du panneau Étalonnage de l'appareil photo : chaque primaire tourne
/// autour de l'axe des gris (teinte) et s'en éloigne ou s'en rapproche (saturation).
/// Le blanc reste blanc.
pub fn calibration_matrix(c: &Calibration) -> Mat3 {
    let params = [
        (c.red_hue, c.red_saturation),
        (c.green_hue, c.green_saturation),
        (c.blue_hue, c.blue_saturation),
    ];
    if params.iter().all(|p| *p == (0.0, 0.0)) {
        return color::IDENTITY;
    }
    let u = 1.0 / 3f32.sqrt();
    let rotation = |theta: f32| -> Mat3 {
        let (s, co) = theta.sin_cos();
        let k = 1.0 - co;
        [
            [co + u * u * k, u * u * k - u * s, u * u * k + u * s],
            [u * u * k + u * s, co + u * u * k, u * u * k - u * s],
            [u * u * k - u * s, u * u * k + u * s, co + u * u * k],
        ]
    };
    let mut m = [[0.0; 3]; 3];
    for (i, &(hue, sat)) in params.iter().enumerate() {
        let mut e = [0.0; 3];
        e[i] = 1.0;
        let chroma = [e[0] - 1.0 / 3.0, e[1] - 1.0 / 3.0, e[2] - 1.0 / 3.0];
        let r = color::apply(&rotation(hue / 100.0 * 30f32.to_radians()), chroma);
        let s = 1.0 + sat / 100.0 * 0.5;
        for (row, line) in m.iter_mut().enumerate() {
            line[i] = 1.0 / 3.0 + s * r[row];
        }
    }
    for row in &mut m {
        let sum: f32 = row.iter().sum();
        if sum.abs() > 1e-6 {
            row.iter_mut().for_each(|v| *v /= sum);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La couleur en colonnes donne exactement les bits du calcul pixel par pixel, pour
    /// le cas simple (profils, Saturation) comme pour les autres (qui passent pixel par
    /// pixel), blocs incomplets compris.
    #[test]
    fn colour_lanes_match_pixel_by_pixel() {
        let pixels: Vec<Vec3> = (0..1003)
            .map(|i| {
                let t = i as f32 / 97.0;
                [
                    (t * 1.7).sin().abs() * 1.4,
                    (t * 0.9).cos().abs(),
                    ((t * 2.3).sin() * 0.5 + 0.5).powi(2) - 0.02,
                ]
            })
            .chain([[0.0; 3], [1.0; 3], [-0.1, 0.2, 0.0]])
            .collect();
        let mut settings = vec![
            Settings {
                saturation: 30.0,
                ..Settings::default()
            },
            Settings {
                profile: luma_project::Profile::Vivid,
                saturation: -60.0,
                ..Settings::default()
            },
            Settings {
                vibrance: 40.0,
                ..Settings::default()
            },
            Settings {
                profile: luma_project::Profile::Vivid,
                vibrance: -70.0,
                saturation: 25.0,
                ..Settings::default()
            },
            Settings {
                profile: luma_project::Profile::Monochrome,
                vibrance: 30.0,
                ..Settings::default()
            },
        ];
        settings.push(Settings::default());
        // Le mélangeur (teinte, saturation, luminance), l'étalonnage, et tout à la fois.
        let mut mixer = Settings::default();
        mixer.hsl.hue[0] = 30.0;
        mixer.hsl.saturation[5] = -20.0;
        mixer.hsl.luminance[4] = 25.0;
        let mut grading = Settings::default();
        grading.color_grading.shadows.saturation = 20.0;
        grading.color_grading.shadows.hue = 220.0;
        grading.color_grading.highlights.saturation = 15.0;
        grading.color_grading.highlights.hue = 40.0;
        let mut all = mixer.clone();
        all.color_grading = grading.color_grading.clone();
        all.vibrance = 20.0;
        all.saturation = -10.0;
        let mut mono_mix = mixer.clone();
        mono_mix.profile = luma_project::Profile::Monochrome;
        settings.extend([mixer, grading, all, mono_mix]);
        for s in &settings {
            let Some(stage) = ColorStage::new(s) else {
                continue;
            };
            let one: Vec<Vec3> = pixels.iter().map(|p| stage.apply(*p)).collect();
            let mut lanes = pixels.clone();
            lanes.chunks_mut(crate::LANES).for_each(|c| stage.apply_lanes(c));
            for (k, (a, b)) in one.iter().zip(&lanes).enumerate() {
                assert!(
                    a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()),
                    "{k} : {a:?} {b:?}"
                );
            }
        }
    }

    fn close3(a: Vec3, b: Vec3, eps: f32) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() <= eps)
    }

    #[test]
    fn inactive_settings_skip_the_stage() {
        let neutral = Settings {
            profile: Profile::Neutral,
            ..Settings::default()
        };
        assert!(ColorStage::new(&neutral).is_none());
        assert!(
            ColorStage::new(&Settings::default()).is_some(),
            "le profil Couleur sature un peu"
        );
    }

    #[test]
    fn monochrome_removes_color() {
        let s = Settings {
            profile: Profile::Monochrome,
            ..Settings::default()
        };
        let p = ColorStage::new(&s).unwrap().apply([0.4, 0.1, 0.05]);
        assert!((p[0] - p[1]).abs() < 1e-4 && (p[1] - p[2]).abs() < 1e-4, "{p:?}");
    }

    #[test]
    fn saturation_leaves_grays_alone() {
        let s = Settings {
            saturation: 80.0,
            vibrance: 50.0,
            ..Settings::default()
        };
        let stage = ColorStage::new(&s).unwrap();
        assert!(close3(stage.apply([0.3, 0.3, 0.3]), [0.3, 0.3, 0.3], 1e-4));
        let red = stage.apply([0.5, 0.1, 0.1]);
        assert!(red[0] - red[1] > 0.4, "{red:?}");
    }

    #[test]
    fn the_targeted_tool_finds_the_color_under_the_pointer() {
        let main = |rgb: [u8; 3]| {
            let b = mixer_bands_at(rgb).unwrap();
            if b[0].1 >= b[1].1 { b[0].0 } else { b[1].0 }
        };
        // Rouge, orange, jaune, vert, bleu (bandes 0, 1, 2, 3, 5).
        assert_eq!(main([220, 30, 30]), 0);
        assert_eq!(main([240, 140, 30]), 1);
        assert_eq!(main([230, 220, 40]), 2);
        assert_eq!(main([40, 180, 60]), 3);
        assert_eq!(main([40, 80, 220]), 5);
        // Un gris : rien à cibler.
        assert!(mixer_bands_at([128, 128, 128]).is_none());
        let b = mixer_bands_at([200, 120, 60]).unwrap();
        assert!((b[0].1 + b[1].1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hsl_targets_one_color() {
        let mut s = Settings {
            profile: Profile::Neutral,
            ..Settings::default()
        };
        s.hsl.saturation[5] = -100.0; // bleu
        let stage = ColorStage::new(&s).unwrap();
        let ok = OkLab::for_working();
        let chroma = |c: Vec3| {
            let l = ok.from_rgb(c);
            (l[1] * l[1] + l[2] * l[2]).sqrt()
        };
        let blue = color::apply(&color::rgb_to_rgb(&color::SRGB, &color::WORKING), [0.05, 0.1, 0.6]);
        let green = color::apply(&color::rgb_to_rgb(&color::SRGB, &color::WORKING), [0.1, 0.5, 0.1]);
        assert!(
            chroma(stage.apply(blue)) < chroma(blue) * 0.3,
            "le bleu perd sa couleur"
        );
        assert!(
            (chroma(stage.apply(green)) - chroma(green)).abs() < 0.01,
            "le vert ne bouge pas"
        );
    }

    #[test]
    fn grading_tints_shadows_but_keeps_black() {
        let mut s = Settings::default();
        s.color_grading.shadows = Wheel {
            hue: 220.0,
            saturation: 80.0,
            luminance: 0.0,
        };
        let stage = ColorStage::new(&s).unwrap();
        let dark = stage.apply([0.02, 0.02, 0.02]);
        assert!(dark[2] > dark[0], "ombres bleutées : {dark:?}");
        assert!(close3(stage.apply([0.0, 0.0, 0.0]), [0.0, 0.0, 0.0], 1e-4));
    }

    #[test]
    fn calibration_keeps_white() {
        let c = Calibration {
            red_hue: 40.0,
            green_saturation: -30.0,
            blue_hue: -20.0,
            ..Calibration::default()
        };
        let m = calibration_matrix(&c);
        assert!(close3(color::apply(&m, [1.0, 1.0, 1.0]), [1.0, 1.0, 1.0], 1e-5));
        assert_eq!(calibration_matrix(&Calibration::default()), color::IDENTITY);
    }
}
