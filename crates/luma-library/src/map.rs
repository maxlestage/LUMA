//! La carte des photos géolocalisées : la projection des tuiles libres (Mercator, des
//! carrés de 256 pixels, quatre fois plus à chaque niveau), la vue (centre et niveau de
//! zoom, fractionnaire), les tuiles qu'elle montre et les photos regroupées quand elles
//! sont trop proches à l'écran pour être vues une à une.
//!
//! Les coordonnées du « monde » vont de 0 à 1 : x de l'antiméridien ouest vers l'est,
//! y du nord vers le sud.

/// La taille d'une tuile, en pixels.
pub const TILE: f64 = 256.0;

/// Le niveau le plus fin des tuiles.
pub const MAX_ZOOM: f64 = 19.0;

/// Au‑delà, la projection n'a plus de sens (le carré du monde s'arrête là).
const MAX_LATITUDE: f64 = 85.051_128_78;

/// Les tuiles libres, et ce qu'il faut afficher sur la carte pour s'en servir.
pub const TILE_URL: &str = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
pub const ATTRIBUTION: &str = "© OpenStreetMap";

/// Latitude et longitude (degrés) vers le monde.
pub fn project(lat: f64, lon: f64) -> [f64; 2] {
    let lat = lat.clamp(-MAX_LATITUDE, MAX_LATITUDE).to_radians();
    let x = (lon + 180.0) / 360.0;
    let y = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0;
    [x, y]
}

/// Le monde vers latitude et longitude.
pub fn unproject([x, y]: [f64; 2]) -> [f64; 2] {
    let n = std::f64::consts::PI * (1.0 - 2.0 * y);
    [n.sinh().atan().to_degrees(), x * 360.0 - 180.0]
}

/// Une tuile : niveau, colonne et ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tile {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl Tile {
    pub fn url(&self) -> String {
        TILE_URL
            .replace("{z}", &self.z.to_string())
            .replace("{x}", &self.x.to_string())
            .replace("{y}", &self.y.to_string())
    }

    /// Son chemin dans le cache, `z/x/y.png`.
    pub fn file(&self) -> std::path::PathBuf {
        [self.z.to_string(), self.x.to_string(), format!("{}.png", self.y)]
            .iter()
            .collect()
    }
}

/// Ce que montre la carte : le point du monde au centre et le niveau de zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub center: [f64; 2],
    pub zoom: f64,
}

impl Default for View {
    /// Le monde entier.
    fn default() -> Self {
        View {
            center: [0.5, 0.5],
            zoom: 1.0,
        }
    }
}

impl View {
    /// La taille du monde à ce zoom, en pixels.
    pub fn scale(&self) -> f64 {
        TILE * self.zoom.exp2()
    }

    /// Une vue qui montre tous ces points (latitude, longitude) dans `size` pixels, avec
    /// une marge ; une seule photo est montrée de près.
    pub fn fit(points: impl IntoIterator<Item = [f64; 2]>, size: [f64; 2]) -> View {
        let mut lo = [f64::MAX; 2];
        let mut hi = [f64::MIN; 2];
        let mut any = false;
        for [lat, lon] in points {
            let p = project(lat, lon);
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
            any = true;
        }
        if !any {
            return View::default();
        }
        let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        // Une marge de 15 % de chaque côté.
        let room = [size[0] * 0.7, size[1] * 0.7];
        let zoom = (0..2)
            .map(|k| {
                let span = (hi[k] - lo[k]).max(1e-12);
                (room[k] / (span * TILE)).log2()
            })
            .fold(f64::MAX, f64::min)
            .clamp(1.0, 15.0);
        View { center, zoom }
    }

    /// Un point du monde à l'écran (pixels depuis le coin haut gauche d'une carte de
    /// `size` pixels).
    pub fn to_screen(&self, p: [f64; 2], size: [f64; 2]) -> [f64; 2] {
        let s = self.scale();
        // La copie du monde la plus proche du centre (la carte se répète d'est en ouest).
        let dx = p[0] - self.center[0];
        let dx = dx - dx.round();
        [dx * s + size[0] / 2.0, (p[1] - self.center[1]) * s + size[1] / 2.0]
    }

    /// Un point de l'écran dans le monde.
    pub fn to_world(&self, at: [f64; 2], size: [f64; 2]) -> [f64; 2] {
        let s = self.scale();
        [
            self.center[0] + (at[0] - size[0] / 2.0) / s,
            self.center[1] + (at[1] - size[1] / 2.0) / s,
        ]
    }

    /// Déplace la carte de tant de pixels (la carte suit le doigt ou la souris).
    pub fn pan(&mut self, by: [f64; 2]) {
        let s = self.scale();
        self.center[0] = (self.center[0] - by[0] / s).rem_euclid(1.0);
        self.center[1] = (self.center[1] - by[1] / s).clamp(0.0, 1.0);
    }

    /// Zoome d'un facteur (2 : deux fois plus près) autour d'un point de l'écran, qui
    /// reste sous le doigt.
    pub fn zoom_at(&mut self, factor: f64, at: [f64; 2], size: [f64; 2]) {
        let before = self.to_world(at, size);
        self.zoom = (self.zoom + factor.max(1e-6).log2()).clamp(0.0, MAX_ZOOM);
        let after = self.to_world(at, size);
        self.center[0] = (self.center[0] + before[0] - after[0]).rem_euclid(1.0);
        self.center[1] = (self.center[1] + before[1] - after[1]).clamp(0.0, 1.0);
    }

    /// Le niveau des tuiles à afficher : le plus proche du zoom.
    pub fn tile_zoom(&self) -> u8 {
        self.zoom.round().clamp(0.0, MAX_ZOOM) as u8
    }

    /// Les tuiles qui couvrent l'écran et leur place (coin haut gauche, côté en pixels),
    /// du centre vers les bords : les premières demandées sont celles qu'on regarde.
    pub fn tiles(&self, size: [f64; 2]) -> Vec<(Tile, [f64; 2], f64)> {
        let z = self.tile_zoom();
        let n = 1i64 << z;
        let side = self.scale() / n as f64;
        let first = self.to_world([0.0, 0.0], size);
        let last = self.to_world(size, size);
        let range = |a: f64, b: f64| ((a * n as f64).floor() as i64, (b * n as f64).floor() as i64);
        let (x0, x1) = range(first[0], last[0]);
        let (y0, y1) = range(first[1], last[1]);
        let mut out = Vec::new();
        for ty in y0.max(0)..=y1.min(n - 1) {
            for tx in x0..=x1 {
                let world = [tx as f64 / n as f64, ty as f64 / n as f64];
                // Position sans ramener à la copie la plus proche : les tuiles se suivent.
                let s = self.scale();
                let at = [
                    (world[0] - self.center[0]) * s + size[0] / 2.0,
                    (world[1] - self.center[1]) * s + size[1] / 2.0,
                ];
                let tile = Tile {
                    z,
                    x: tx.rem_euclid(n) as u32,
                    y: ty as u32,
                };
                out.push((tile, at, side));
            }
        }
        let mid = [size[0] / 2.0 - side / 2.0, size[1] / 2.0 - side / 2.0];
        out.sort_by(|a, b| {
            let d = |p: [f64; 2]| (p[0] - mid[0]).powi(2) + (p[1] - mid[1]).powi(2);
            d(a.1).total_cmp(&d(b.1))
        });
        out
    }
}

/// Des photos assez proches à l'écran pour être montrées ensemble.
#[derive(Clone, Debug, PartialEq)]
pub struct Cluster {
    /// Leur place moyenne à l'écran.
    pub at: [f64; 2],
    /// Leurs rangs dans la liste donnée, dans l'ordre de cette liste.
    pub members: Vec<usize>,
}

/// Regroupe les points (latitude, longitude) à moins de `radius` pixels les uns des
/// autres, case par case d'une grille de cette taille : le même résultat d'une image à
/// l'autre, et en temps linéaire même pour des dizaines de milliers de photos. Seuls
/// les groupes visibles à l'écran sont rendus.
pub fn clusters(points: &[[f64; 2]], view: &View, size: [f64; 2], radius: f64) -> Vec<Cluster> {
    let radius = radius.max(1.0);
    // Par case : le rang de sa première photo, la somme des places, les photos.
    type Cell = (usize, [f64; 2], Vec<usize>);
    let mut cells: std::collections::BTreeMap<(i64, i64), Cell> = Default::default();
    for (i, &[lat, lon]) in points.iter().enumerate() {
        let p = view.to_screen(project(lat, lon), size);
        if p[0] < -radius || p[1] < -radius || p[0] > size[0] + radius || p[1] > size[1] + radius {
            continue;
        }
        let key = ((p[0] / radius).floor() as i64, (p[1] / radius).floor() as i64);
        let cell = cells.entry(key).or_insert((i, [0.0; 2], Vec::new()));
        cell.1[0] += p[0];
        cell.1[1] += p[1];
        cell.2.push(i);
    }
    let mut out: Vec<(usize, Cluster)> = cells
        .into_values()
        .map(|(first, sum, members)| {
            let n = members.len() as f64;
            (
                first,
                Cluster {
                    at: [sum[0] / n, sum[1] / n],
                    members,
                },
            )
        })
        .collect();
    out.sort_by_key(|(first, _)| *first);
    out.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARIS: [f64; 2] = [48.8584, 2.2945];
    const ROME: [f64; 2] = [41.8902, 12.4922];

    fn close(a: [f64; 2], b: [f64; 2], eps: f64) -> bool {
        (a[0] - b[0]).abs() < eps && (a[1] - b[1]).abs() < eps
    }

    #[test]
    fn projection_round_trip() {
        assert!(close(project(0.0, 0.0), [0.5, 0.5], 1e-12));
        assert!(close(project(MAX_LATITUDE, -180.0), [0.0, 0.0], 1e-9));
        for p in [PARIS, ROME, [-33.8568, 151.2153], [64.1466, -21.9426]] {
            let w = project(p[0], p[1]);
            assert!(close(unproject(w), p, 1e-9), "{p:?}");
        }
        // La tuile de la tour Eiffel au niveau 15, comme partout ailleurs.
        let w = project(PARIS[0], PARIS[1]);
        let n = 2f64.powi(15);
        assert_eq!(((w[0] * n) as u32, (w[1] * n) as u32), (16_592, 11_272));
        let t = Tile {
            z: 15,
            x: 16_592,
            y: 11_272,
        };
        assert_eq!(t.url(), "https://tile.openstreetmap.org/15/16592/11272.png");
        assert_eq!(t.file(), std::path::Path::new("15/16592/11272.png"));
    }

    #[test]
    fn fit_shows_every_photo() {
        let size = [800.0, 600.0];
        let v = View::fit([PARIS, ROME], size);
        for p in [PARIS, ROME] {
            let s = v.to_screen(project(p[0], p[1]), size);
            assert!(s[0] > 50.0 && s[0] < 750.0 && s[1] > 50.0 && s[1] < 550.0, "{s:?}");
        }
        // Une photo seule : de près, au centre.
        let one = View::fit([PARIS], size);
        assert_eq!(one.zoom, 15.0);
        assert!(close(
            one.to_screen(project(PARIS[0], PARIS[1]), size),
            [400.0, 300.0],
            1e-6
        ));
        assert_eq!(View::fit([], size), View::default());
    }

    #[test]
    fn zoom_keeps_the_point_under_the_finger() {
        let size = [800.0, 600.0];
        let mut v = View::fit([PARIS, ROME], size);
        let at = [200.0, 150.0];
        let before = v.to_world(at, size);
        v.zoom_at(2.0, at, size);
        assert!(close(v.to_world(at, size), before, 1e-12));
        let z = v.zoom;
        v.zoom_at(0.5, at, size);
        assert!((v.zoom - (z - 1.0)).abs() < 1e-12);
        assert!(close(v.to_world(at, size), before, 1e-12));
        // La carte suit la souris.
        let p = v.to_screen(project(ROME[0], ROME[1]), size);
        v.pan([30.0, -20.0]);
        let q = v.to_screen(project(ROME[0], ROME[1]), size);
        assert!(close([q[0] - p[0], q[1] - p[1]], [30.0, -20.0], 1e-6));
    }

    #[test]
    fn tiles_cover_the_screen_from_the_centre() {
        let size = [800.0, 600.0];
        let v = View {
            center: project(PARIS[0], PARIS[1]),
            zoom: 12.0,
        };
        let tiles = v.tiles(size);
        // 800 × 600 : quatre à cinq colonnes, trois à quatre lignes.
        assert!((12..=20).contains(&tiles.len()), "{}", tiles.len());
        let (first, at, side) = tiles[0];
        assert_eq!(side, 256.0);
        assert!(at[0] <= 400.0 && at[0] + side >= 400.0 && at[1] <= 300.0 && at[1] + side >= 300.0);
        assert_eq!(first.z, 12);
        // Tout l'écran est couvert.
        for p in [[0.0, 0.0], [799.0, 0.0], [0.0, 599.0], [799.0, 599.0]] {
            assert!(
                tiles
                    .iter()
                    .any(|(_, a, s)| a[0] <= p[0] && p[0] < a[0] + s && a[1] <= p[1] && p[1] < a[1] + s),
                "{p:?}"
            );
        }
        // Au bord de l'antiméridien, les tuiles reprennent à l'autre bout du monde.
        let edge = View {
            center: [0.0, 0.5],
            zoom: 3.0,
        };
        let xs: Vec<u32> = edge.tiles(size).iter().map(|t| t.0.x).collect();
        assert!(xs.contains(&7) && xs.contains(&0), "{xs:?}");
    }

    #[test]
    fn nearby_photos_are_grouped() {
        let size = [800.0, 600.0];
        let paris2 = [48.8606, 2.3376];
        let points = [PARIS, ROME, paris2, [-33.8568, 151.2153]];
        let wide = View::fit([PARIS, ROME], size);
        let c = clusters(&points, &wide, size, 48.0);
        assert_eq!(c.len(), 2, "{c:?}");
        assert_eq!(c[0].members, [0, 2], "les deux de Paris ensemble");
        assert_eq!(c[1].members, [1]);
        // De près, les deux photos de Paris se séparent (et Rome sort de l'écran).
        let near = View {
            center: project(PARIS[0], PARIS[1]),
            zoom: 13.0,
        };
        let c = clusters(&points, &near, size, 48.0);
        assert_eq!(
            c.iter().map(|c| c.members.clone()).collect::<Vec<_>>(),
            [vec![0], vec![2]]
        );
    }
}
