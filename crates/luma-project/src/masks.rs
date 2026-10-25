//! Panneau Masquage : des retouches locales, comme dans Lightroom. Chaque masque a une
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

/// Les curseurs d'un masque (plages de Lightroom : exposition ±4, le reste ±100).
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
