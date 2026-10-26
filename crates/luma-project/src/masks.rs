//! Panneau Masquage : des retouches locales. Chaque masque a une
//! forme (dégradé linéaire, dégradé radial, pinceau) et ses propres curseurs
//! (exposition, contraste, hautes lumières, ombres, blancs, noirs, température, teinte,
//! saturation, clarté).
//!
//! Les positions sont en fractions de l'image recadrée (0 à 1 en largeur et en
//! hauteur) : le masque suit la photo à toutes les tailles de rendu.
//!
//! Une plage ([`Range`]) peut restreindre la zone d'un masque aux tons (luminosité) ou
//! aux couleurs (teinte) de la photo : le ciel bleu sous un dégradé, les hautes
//! lumières d'un visage sous un pinceau.

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

/// Une plage : la zone du masque ne garde que les pixels de certains tons ou de
/// certaines couleurs. Mesurées en OKLab sur la photo linéaire, avant les retouches
/// des masques.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Range {
    /// La luminosité perçue (0 noir, 100 blanc) : plein effet de `low` à `high`, fondu
    /// de `feather` (0 à 100) de part et d'autre.
    Luminance { low: f32, high: f32, feather: f32 },
    /// La teinte (OKLab, en degrés) : plein effet à moins de `width` degrés de `hue`,
    /// fondu de `feather` (0 à 100) au‑delà ; les gris (sans couleur) sont laissés.
    Color { hue: f32, width: f32, feather: f32 },
}

impl Range {
    /// Les tons clairs.
    pub fn luminance() -> Range {
        Range::Luminance {
            low: 60.0,
            high: 100.0,
            feather: 30.0,
        }
    }

    /// Les bleus du ciel.
    pub fn color() -> Range {
        Range::Color {
            hue: 255.0,
            width: 30.0,
            feather: 50.0,
        }
    }

    /// Ramène les valeurs dans leurs plages (bornes dans l'ordre, teinte de 0 à 360).
    pub fn clamp(&mut self) {
        let fix = |v: &mut f32, lo: f32, hi: f32, d: f32| *v = if v.is_finite() { v.clamp(lo, hi) } else { d };
        match self {
            Range::Luminance { low, high, feather } => {
                fix(low, 0.0, 100.0, 0.0);
                fix(high, 0.0, 100.0, 100.0);
                if *low > *high {
                    std::mem::swap(low, high);
                }
                fix(feather, 0.0, 100.0, 0.0);
            }
            Range::Color { hue, width, feather } => {
                *hue = if hue.is_finite() { hue.rem_euclid(360.0) } else { 0.0 };
                fix(width, 0.0, 180.0, 30.0);
                fix(feather, 0.0, 100.0, 0.0);
            }
        }
    }
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
    /// La plage qui restreint la zone (tons ou couleurs), s'il y en a une.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    /// Pinceau : la zone suit les bords de la photo (filtre guidé par sa clarté).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub edges: bool,
    /// D'autres formes combinées à la première : ajoutées, retirées ou en intersection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
}

/// Comment une partie se combine à la zone du masque.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Combine {
    /// La zone s'étend à la partie.
    Add,
    /// La partie est retirée de la zone.
    Subtract,
    /// Seul ce qui est dans les deux reste.
    Intersect,
}

/// Une partie d'un masque combiné : une forme, éventuellement inversée.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub combine: Combine,
    pub shape: Shape,
    #[serde(default)]
    pub invert: bool,
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
            edges: false,
            parts: Vec::new(),
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
            edges: false,
            parts: Vec::new(),
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
            edges: false,
            parts: Vec::new(),
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
            edges: false,
            parts: Vec::new(),
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
