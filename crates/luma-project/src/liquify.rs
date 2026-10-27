//! Fluidité : pousser, gonfler ou dégonfler une partie de la photo au pinceau.
//!
//! Chaque coup de pinceau garde son chemin (positions en fractions de l'image recadrée,
//! comme les retouches), sa taille (en fraction de la largeur) et sa force : la
//! déformation se recalcule à chaque rendu, à toutes les tailles, et tient en quelques
//! nombres dans le XMP.

use serde::{Deserialize, Serialize};

/// Ce que fait le pinceau.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarpKind {
    /// Pousser : la photo suit le pinceau.
    #[default]
    Push,
    /// Gonfler : la zone sous le pinceau grandit depuis son centre.
    Bloat,
    /// Dégonfler : la zone se resserre vers son centre.
    Pucker,
}

/// Un coup de pinceau.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Warp {
    #[serde(default)]
    pub kind: WarpKind,
    /// Le chemin du pinceau (au moins un point).
    pub points: Vec<(f32, f32)>,
    /// Le rayon, en fraction de la largeur.
    pub radius: f32,
    /// 0 à 100.
    #[serde(default = "half")]
    pub strength: f32,
}

fn half() -> f32 {
    50.0
}

impl Warp {
    pub fn new(kind: WarpKind, at: (f32, f32), radius: f32, strength: f32) -> Warp {
        Warp {
            kind,
            points: vec![at],
            radius,
            strength,
        }
    }

    /// Les valeurs ramenées dans leurs bornes (fichier abîmé ou écrit à la main).
    pub fn clamp(&mut self) {
        let fix = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        self.radius = fix(self.radius, 0.002, 0.5, 0.05);
        self.strength = fix(self.strength, 0.0, 100.0, 50.0);
        self.points.retain(|p| p.0.is_finite() && p.1.is_finite());
        for p in &mut self.points {
            *p = (p.0.clamp(-1.0, 2.0), p.1.clamp(-1.0, 2.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stroke_is_saved_and_clamped() {
        let mut w = Warp::new(WarpKind::Bloat, (0.4, 0.5), 0.08, 70.0);
        w.points.push((0.45, f32::NAN));
        w.radius = f32::INFINITY;
        w.clamp();
        assert_eq!(w.points, vec![(0.4, 0.5)]);
        assert_eq!(w.radius, 0.05);
        let json = serde_json::to_string(&w).unwrap();
        assert!(json.contains(r#""kind":"bloat""#), "{json}");
        assert_eq!(serde_json::from_str::<Warp>(&json).unwrap(), w);
    }
}
