//! Panneau Masquage : des retouches locales. Chaque masque a une
//! forme (dégradé linéaire, dégradé radial, pinceau) et ses propres curseurs
//! (exposition, contraste, hautes lumières, ombres, blancs, noirs, température, teinte,
//! saturation, clarté).
//!
//! Les positions sont en fractions de l'image recadrée (0 à 1 en largeur et en
//! hauteur) : le masque suit la photo à toutes les tailles de rendu.

use serde::{Deserialize, Serialize};

/// La forme d'un masque.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Shape {
    /// Plein effet du côté de `start`, plus rien après `end`, fondu entre les deux.
    Linear { start: (f32, f32), end: (f32, f32) },
    /// Une ellipse de centre `center`, de demi‑axes `radius` (en fractions de la
    /// largeur et de la hauteur), tournée de `angle` degrés ; `feather` (0 à 100) est
    /// la largeur du fondu vers l'intérieur.
    Radial {
        center: (f32, f32),
        radius: (f32, f32),
        angle: f32,
        feather: f32,
    },
    /// Des touches de pinceau.
    Brush { dabs: Vec<Dab> },
    /// Toute l'image : pour empiler des films (chacun avec son intensité).
    Full,
}

/// Un film (préréglage) appliqué dans la zone d'un masque : son nom et son intensité
/// (0 à 100). Plusieurs masques portent plusieurs films : ils s'empilent dans l'ordre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Look {
    pub name: String,
    pub amount: f32,
}

/// Une touche de pinceau : position, rayon (en fraction de la largeur), contour
/// progressif (0 à 100), et gomme ou non.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dab {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub feather: f32,
    pub flow: f32,
    #[serde(default)]
    pub erase: bool,
}

/// Les curseurs d'un masque (plages : exposition ±4, le reste ±100).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Local {
    pub exposure: f32,
    pub contrast: f32,
    pub highlights: f32,
    pub shadows: f32,
    pub whites: f32,
    pub blacks: f32,
    pub temperature: f32,
    pub tint: f32,
    pub saturation: f32,
    pub clarity: f32,
}

impl Local {
    pub fn is_neutral(&self) -> bool {
        *self == Local::default()
    }

    pub fn clamp(&mut self) {
        let fix = |v: &mut f32, lim: f32| *v = if v.is_finite() { v.clamp(-lim, lim) } else { 0.0 };
        fix(&mut self.exposure, 4.0);
        for v in [
            &mut self.contrast,
            &mut self.highlights,
            &mut self.shadows,
            &mut self.whites,
            &mut self.blacks,
            &mut self.temperature,
            &mut self.tint,
            &mut self.saturation,
            &mut self.clarity,
        ] {
            fix(v, 100.0);
        }
    }
}

/// Un masque : sa forme, ses curseurs, son intensité (0 à 100) ; `invert` retourne la
/// zone touchée.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mask {
    pub name: String,
    pub shape: Shape,
    #[serde(default)]
    pub invert: bool,
    #[serde(default = "full")]
    pub amount: f32,
    #[serde(default)]
    pub local: Local,
    /// Un film dans la zone du masque.
    #[serde(default)]
    pub look: Option<Look>,
    /// Une plage de luminance : le masque n'agit que sur ces tons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<LumaRange>,
}

/// Une plage de luminance, du noir (0) au blanc (100), en clarté perçue : le masque
/// agit pleinement entre `low` et `high`, et s'éteint en douceur sur `feather` (0 à 100)
/// de part et d'autre.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LumaRange {
    pub low: f32,
    pub high: f32,
    pub feather: f32,
}

impl Default for LumaRange {
    /// Les hautes lumières : le ciel, les reflets.
    fn default() -> Self {
        LumaRange {
            low: 60.0,
            high: 100.0,
            feather: 30.0,
        }
    }
}

impl LumaRange {
    /// La part du masque (0 à 1) pour une clarté perçue `l` (0 à 1).
    pub fn factor(&self, l: f32) -> f32 {
        let (mut lo, mut hi) = (self.low.clamp(0.0, 100.0) / 100.0, self.high.clamp(0.0, 100.0) / 100.0);
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        // Le fondu : jusqu'à un quart de l'échelle de chaque côté.
        let f = self.feather.clamp(0.0, 100.0) / 100.0 * 0.25;
        let rise = |a: f32, b: f32, x: f32| {
            if b <= a {
                if x < a { 0.0 } else { 1.0 }
            } else {
                let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            }
        };
        // Les extrêmes de l'échelle sont pleins : « jusqu'au blanc » inclut le blanc.
        let below = if lo <= 0.0 { 1.0 } else { rise(lo - f, lo, l) };
        let above = if hi >= 1.0 { 1.0 } else { 1.0 - rise(hi, hi + f, l) };
        below * above
    }
}

fn full() -> f32 {
    100.0
}

impl Mask {
    /// Un dégradé linéaire, du haut (plein effet) vers le milieu : le ciel.
    pub fn linear() -> Mask {
        Mask {
            name: "Dégradé linéaire".into(),
            shape: Shape::Linear {
                start: (0.5, 0.15),
                end: (0.5, 0.5),
            },
            invert: false,
            amount: 100.0,
            local: Local::default(),
            look: None,
            range: None,
        }
    }

    /// Un dégradé radial au centre.
    pub fn radial() -> Mask {
        Mask {
            name: "Dégradé radial".into(),
            shape: Shape::Radial {
                center: (0.5, 0.5),
                radius: (0.25, 0.3),
                angle: 0.0,
                feather: 50.0,
            },
            invert: false,
            amount: 100.0,
            local: Local::default(),
            look: None,
            range: None,
        }
    }

    /// Un pinceau vide.
    pub fn brush() -> Mask {
        Mask {
            name: "Pinceau".into(),
            shape: Shape::Brush { dabs: Vec::new() },
            invert: false,
            amount: 100.0,
            local: Local::default(),
            look: None,
            range: None,
        }
    }

    /// Toute l'image, pour un film empilé.
    pub fn full() -> Mask {
        Mask {
            name: "Image entière".into(),
            shape: Shape::Full,
            invert: false,
            amount: 100.0,
            local: Local::default(),
            look: None,
            range: None,
        }
    }

    /// Les curseurs locaux agissent‑ils ?
    pub fn is_active(&self) -> bool {
        self.amount > 0.0 && !self.local.is_neutral()
    }

    /// Le film du masque, s'il en a un et qu'il agit (intensité du film, de 0 à 1).
    pub fn active_look(&self) -> Option<(&str, f32)> {
        let l = self.look.as_ref()?;
        // L'intensité du masque (et son inversion) est déjà dans sa zone.
        let k = l.amount / 100.0;
        (k > 0.0).then_some((l.name.as_str(), k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_luminance_range_keeps_its_tones() {
        let r = LumaRange {
            low: 40.0,
            high: 70.0,
            feather: 40.0,
        };
        // Plein dans la plage, rien loin dehors, un fondu entre les deux.
        assert_eq!(r.factor(0.55), 1.0);
        assert_eq!(r.factor(0.40), 1.0);
        assert_eq!(r.factor(0.70), 1.0);
        assert_eq!(r.factor(0.1), 0.0);
        assert_eq!(r.factor(0.95), 0.0);
        let soft = r.factor(0.35);
        assert!(soft > 0.0 && soft < 1.0);
        assert!(r.factor(0.36) > r.factor(0.33));
        // Sans adoucissement : une coupure nette.
        let sharp = LumaRange { feather: 0.0, ..r };
        assert_eq!(sharp.factor(0.399), 0.0);
        assert_eq!(sharp.factor(0.401), 1.0);
        // Les extrêmes de l'échelle sont pleins, et des bornes à l'envers se remettent.
        let whites = LumaRange::default();
        assert_eq!(whites.factor(1.0), 1.0);
        assert_eq!(whites.factor(0.0), 0.0);
        let all = LumaRange {
            low: 0.0,
            high: 100.0,
            feather: 50.0,
        };
        assert!([0.0, 0.3, 1.0].iter().all(|&l| all.factor(l) == 1.0));
        let swapped = LumaRange {
            low: 70.0,
            high: 40.0,
            feather: 40.0,
        };
        assert_eq!(swapped.factor(0.55), 1.0);
        // Un masque d'avant les plages se relit sans plage.
        let old: Mask =
            serde_json::from_str(r#"{"name":"Ciel","shape":{"kind":"full"},"local":{"exposure":-1.0}}"#).unwrap();
        assert_eq!(old.range, None);
    }
}
