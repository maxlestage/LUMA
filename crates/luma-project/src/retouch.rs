//! Retouche : le tampon (clonage) et le correcteur (réparation par diffusion).
//!
//! Chaque retouche est un disque de la photo recadrée, remplacé par un disque de même
//! taille pris ailleurs (la source). Le tampon copie la source telle quelle ; le
//! correcteur n'en garde que la texture et prend la lumière et la couleur du bord de la
//! zone, pour une réparation qui ne se voit pas. Les yeux rouges, eux, n'ont pas de
//! source : dans le disque, la pupille rouge redevient sombre.
//!
//! Les positions sont en fractions de l'image recadrée (0 à 1 en largeur et en
//! hauteur), le rayon en fraction de la largeur : la retouche suit la photo à toutes
//! les tailles de rendu.

use serde::{Deserialize, Serialize};

/// Copier la source, ou n'en garder que la texture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpotKind {
    /// Tampon : la source copiée telle quelle.
    Clone,
    /// Correcteur : la texture de la source, la lumière et la couleur du bord.
    #[default]
    Heal,
    /// Yeux rouges : les pixels d'un rouge franc du disque redeviennent sombres et
    /// neutres (la peau autour et le reflet blanc restent).
    RedEye,
    /// Suppression : le disque rempli par des morceaux de la photo qui l'entourent (ce
    /// qui gêne disparaît, la texture continue) ; pas de source à choisir.
    Remove,
}

impl SpotKind {
    /// Le tampon et le correcteur copient une source ; les yeux rouges et la suppression
    /// travaillent sur place.
    pub fn has_source(self) -> bool {
        matches!(self, SpotKind::Clone | SpotKind::Heal)
    }
}

/// Une retouche.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spot {
    #[serde(default)]
    pub kind: SpotKind,
    /// Le centre de la zone réparée.
    pub center: (f32, f32),
    /// Le centre de la zone copiée.
    pub source: (f32, f32),
    /// Le rayon, en fraction de la largeur.
    pub radius: f32,
    /// La largeur du fondu vers l'intérieur (0 à 100).
    #[serde(default = "half")]
    pub feather: f32,
    /// 0 à 100.
    #[serde(default = "full")]
    pub opacity: f32,
    /// Suppression tracée : les points suivants du tracé (après `center`), reliés ; vide
    /// pour un cercle. Un fil ou une rayure s'efface d'un seul geste.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<(f32, f32)>,
}

fn half() -> f32 {
    50.0
}

fn full() -> f32 {
    100.0
}

impl Spot {
    /// Une retouche en `center`, sa source à côté (à droite, ou à gauche près du bord
    /// droit), à deux rayons et demi.
    /// Pour les yeux rouges et la suppression, la source est le centre (elle ne sert pas).
    pub fn new(kind: SpotKind, center: (f32, f32), radius: f32) -> Spot {
        let dx = 2.5 * radius;
        let x = if !kind.has_source() {
            center.0
        } else if center.0 + dx + radius <= 1.0 {
            center.0 + dx
        } else {
            center.0 - dx
        };
        Spot {
            kind,
            center,
            source: (x, center.1),
            radius,
            feather: half(),
            opacity: full(),
            path: Vec::new(),
        }
    }

    /// Le tracé entier : le centre puis les points suivants.
    pub fn points(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        std::iter::once(self.center).chain(self.path.iter().copied())
    }

    /// Ramène les valeurs dans leurs plages.
    pub fn clamp(&mut self) {
        let fix = |v: &mut f32, lo: f32, hi: f32, d: f32| *v = if v.is_finite() { v.clamp(lo, hi) } else { d };
        for v in [
            &mut self.center.0,
            &mut self.center.1,
            &mut self.source.0,
            &mut self.source.1,
        ] {
            fix(v, -0.5, 1.5, 0.5);
        }
        fix(&mut self.radius, 0.001, 0.5, 0.02);
        fix(&mut self.feather, 0.0, 100.0, half());
        fix(&mut self.opacity, 0.0, 100.0, full());
        self.path.retain(|p| p.0.is_finite() && p.1.is_finite());
        for p in &mut self.path {
            *p = (p.0.clamp(-0.5, 1.5), p.1.clamp(-0.5, 1.5));
        }
        // Un tracé n'a de sens que pour la suppression.
        if self.kind != SpotKind::Remove {
            self.path.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_traced_removal_is_saved_and_only_for_removal() {
        let mut s = Spot::new(SpotKind::Remove, (0.2, 0.3), 0.01);
        s.path = vec![(0.4, 0.35), (f32::NAN, 0.1), (0.6, 2.0)];
        s.clamp();
        assert_eq!(s.path, vec![(0.4, 0.35), (0.6, 1.5)]);
        assert_eq!(s.points().count(), 3);
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Spot>(&json).unwrap(), s);
        // Un cercle n'écrit pas de tracé ; un correcteur n'en garde pas.
        let circle = Spot::new(SpotKind::Remove, (0.5, 0.5), 0.02);
        assert!(!serde_json::to_string(&circle).unwrap().contains("path"));
        let mut heal = Spot::new(SpotKind::Heal, (0.5, 0.5), 0.02);
        heal.path = vec![(0.6, 0.6)];
        heal.clamp();
        assert!(heal.path.is_empty());
    }
}
