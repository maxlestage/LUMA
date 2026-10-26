//! Le filigrane texte des exports : une ligne de texte dessinée avec la police de
//! LÜMA (Ubuntu Light, embarquée), lissée par une couverture exacte de chaque pixel,
//! posée à l'un des neuf emplacements de l'image avec son opacité.
//!
//! Les contours des lettres sont lus par `skrifa` ; ils sont remplis ici par
//! accumulation des aires signées (chaque segment ajoute sa part de chaque pixel, une
//! somme par ligne donne la couverture) : pas de bibliothèque de rendu.

use luma_core::RgbImage;
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};

/// La police des filigranes.
const FONT: &[u8] = epaint_default_fonts::UBUNTU_LIGHT;

/// L'un des neuf emplacements du filigrane.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Position {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    #[default]
    BottomRight,
}

impl Position {
    pub const ALL: [Position; 9] = [
        Position::TopLeft,
        Position::Top,
        Position::TopRight,
        Position::Left,
        Position::Center,
        Position::Right,
        Position::BottomLeft,
        Position::Bottom,
        Position::BottomRight,
    ];

    /// `bottom-right`, `top`, `center`…
    pub fn id(self) -> &'static str {
        match self {
            Position::TopLeft => "top-left",
            Position::Top => "top",
            Position::TopRight => "top-right",
            Position::Left => "left",
            Position::Center => "center",
            Position::Right => "right",
            Position::BottomLeft => "bottom-left",
            Position::Bottom => "bottom",
            Position::BottomRight => "bottom-right",
        }
    }

    /// En clair, en français.
    pub fn label(self) -> &'static str {
        match self {
            Position::TopLeft => "En haut à gauche",
            Position::Top => "En haut",
            Position::TopRight => "En haut à droite",
            Position::Left => "À gauche",
            Position::Center => "Au centre",
            Position::Right => "À droite",
            Position::BottomLeft => "En bas à gauche",
            Position::Bottom => "En bas",
            Position::BottomRight => "En bas à droite",
        }
    }

    /// Lit `bottom-right` (ou `bas-droite`, `centre`…).
    pub fn parse(s: &str) -> Option<Position> {
        let s = s.trim().to_lowercase().replace(['_', ' '], "-");
        let fr = match s.as_str() {
            "haut-gauche" => "top-left",
            "haut" => "top",
            "haut-droite" => "top-right",
            "gauche" => "left",
            "centre" => "center",
            "droite" => "right",
            "bas-gauche" => "bottom-left",
            "bas" => "bottom",
            "bas-droite" => "bottom-right",
            other => other,
        };
        Position::ALL.into_iter().find(|p| p.id() == fr)
    }

    /// Les fractions (0, ½ ou 1) de l'espace libre à gauche et en haut.
    fn anchor(self) -> (f32, f32) {
        let x = match self {
            Position::TopLeft | Position::Left | Position::BottomLeft => 0.0,
            Position::Top | Position::Center | Position::Bottom => 0.5,
            _ => 1.0,
        };
        let y = match self {
            Position::TopLeft | Position::Top | Position::TopRight => 0.0,
            Position::Left | Position::Center | Position::Right => 0.5,
            _ => 1.0,
        };
        (x, y)
    }
}

/// Un filigrane texte.
#[derive(Clone, Debug, PartialEq)]
pub struct Watermark {
    pub text: String,
    pub position: Position,
    /// La taille du texte, en pour cent du petit côté de l'image (0,5 à 25).
    pub size: f32,
    /// 0 à 1.
    pub opacity: f32,
    /// Texte noir plutôt que blanc.
    pub dark: bool,
}

impl Watermark {
    /// Un filigrane blanc, en bas à droite, de taille et d'opacité raisonnables.
    pub fn new(text: &str) -> Watermark {
        Watermark {
            text: text.to_owned(),
            position: Position::BottomRight,
            size: 3.0,
            opacity: 0.7,
            dark: false,
        }
    }

    /// Pose le filigrane sur l'image (linéaire, à sa taille finale).
    pub fn apply(&self, img: &mut RgbImage) {
        let text = self.text.trim();
        let opacity = self.opacity.clamp(0.0, 1.0);
        if text.is_empty() || opacity <= 0.0 || img.width < 8 || img.height < 8 {
            return;
        }
        let short = img.width.min(img.height) as f32;
        let px = (short * self.size.clamp(0.5, 25.0) / 100.0).max(6.0);
        let Some(mask) = render_text(text, px) else {
            return;
        };
        // La marge : un peu plus qu'une demi‑hauteur de texte.
        let margin = (px * 0.6).round() as isize;
        let (ax, ay) = self.position.anchor();
        let free_x = img.width as isize - mask.width as isize - 2 * margin;
        let free_y = img.height as isize - mask.height as isize - 2 * margin;
        let x0 = margin + (free_x.max(0) as f32 * ax).round() as isize;
        let y0 = margin + (free_y.max(0) as f32 * ay).round() as isize;
        let color = if self.dark { 0.0 } else { 1.0 };
        for my in 0..mask.height {
            let y = y0 + my as isize;
            if y < 0 || y >= img.height as isize {
                continue;
            }
            for mx in 0..mask.width {
                let x = x0 + mx as isize;
                if x < 0 || x >= img.width as isize {
                    continue;
                }
                let a = mask.data[my * mask.width + mx] * opacity;
                if a > 0.0 {
                    // Le mélange se fait en valeurs perçues (gamma 2,2) : une opacité de
                    // 30 % se voit comme 30 %, pas comme un voile presque opaque.
                    let p = &mut img.data[y as usize * img.width + x as usize];
                    for c in p.iter_mut() {
                        let v = c.max(0.0).powf(1.0 / 2.2);
                        *c = (v + (color - v) * a).powf(2.2);
                    }
                }
            }
        }
    }

    /// En clair : « filigrane « © Maxime », en bas à droite, 3 %, 70 % ».
    pub fn describe(&self) -> String {
        format!(
            "filigrane « {} », {}, {} %, {} %{}",
            self.text.trim(),
            self.position.label().to_lowercase(),
            self.size,
            (self.opacity * 100.0).round(),
            if self.dark { ", noir" } else { "" }
        )
    }
}

/// La couverture (0 à 1) d'une ligne de texte de `px` pixels (taille de la police) :
/// largeur et hauteur ajustées au texte, une marge d'un pixel autour.
pub struct Mask {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

/// Dessine une ligne de texte ; `None` si la police ne peut pas être lue (ne devrait
/// pas arriver : elle est embarquée).
pub fn render_text(text: &str, px: f32) -> Option<Mask> {
    let font = skrifa::FontRef::new(FONT).ok()?;
    let size = Size::new(px);
    let location = LocationRef::default();
    let metrics = font.metrics(size, location);
    let advances = font.glyph_metrics(size, location);
    let charmap = font.charmap();
    let outlines = font.outline_glyphs();
    let space = charmap
        .map(' ')
        .and_then(|g| advances.advance_width(g))
        .unwrap_or(px * 0.25);
    // Les lettres, posées sur la ligne de base.
    let mut glyphs = Vec::new();
    let mut pen_x = 0.0f32;
    for ch in text.chars() {
        match charmap.map(ch) {
            Some(g) => {
                glyphs.push((g, pen_x));
                pen_x += advances.advance_width(g).unwrap_or(space);
            }
            None => pen_x += space,
        }
    }
    let ascent = metrics.ascent.max(px * 0.5);
    let descent = metrics.descent.min(0.0);
    let width = (pen_x.ceil() as usize).max(1) + 3;
    let height = ((ascent - descent).ceil() as usize).max(1) + 3;
    let mut r = Raster::new(width, height);
    for (g, x) in glyphs {
        if let Some(outline) = outlines.get(g) {
            let mut pen = Pen {
                raster: &mut r,
                dx: x + 1.0,
                baseline: ascent + 1.0,
                start: (0.0, 0.0),
                last: (0.0, 0.0),
            };
            let _ = outline.draw(DrawSettings::unhinted(size, location), &mut pen);
            pen.close();
        }
    }
    Some(r.coverage())
}

/// Les aires signées accumulées (une case de plus par ligne pour le bord droit).
struct Raster {
    width: usize,
    height: usize,
    acc: Vec<f32>,
}

impl Raster {
    fn new(width: usize, height: usize) -> Raster {
        Raster {
            width,
            height,
            acc: vec![0.0; width * height + 2],
        }
    }

    /// Ajoute un segment (coordonnées en pixels, y vers le bas) : sa part signée de
    /// l'aire de chaque pixel qu'il traverse.
    fn line(&mut self, p0: (f32, f32), p1: (f32, f32)) {
        if (p0.1 - p1.1).abs() <= f32::EPSILON {
            return;
        }
        let (dir, p0, p1) = if p0.1 < p1.1 { (1.0, p0, p1) } else { (-1.0, p1, p0) };
        let max_x = (self.width - 2) as f32;
        let clamp = |x: f32| x.clamp(0.0, max_x);
        let dxdy = (p1.0 - p0.0) / (p1.1 - p0.1);
        let mut x = p0.0;
        let y_start = p0.1.max(0.0);
        if p0.1 < 0.0 {
            x -= p0.1 * dxdy;
        }
        let y_end = (p1.1.ceil() as usize).min(self.height);
        for y in (y_start as usize)..y_end {
            let line = y * self.width;
            let dy = ((y + 1) as f32).min(p1.1) - (y as f32).max(p0.1);
            let next = x + dxdy * dy;
            let d = dy * dir;
            let (x0, x1) = if x < next {
                (clamp(x), clamp(next))
            } else {
                (clamp(next), clamp(x))
            };
            let x0f = x0.floor();
            let x0i = x0f as usize;
            let x1c = x1.ceil();
            let x1i = x1c as usize;
            if x1i <= x0i + 1 {
                // Le segment reste dans un pixel : sa part va à ce pixel et au suivant.
                let mid = 0.5 * (x0 + x1) - x0f;
                self.acc[line + x0i] += d - d * mid;
                self.acc[line + x0i + 1] += d * mid;
            } else {
                let s = (x1 - x0).recip();
                let f0 = x0 - x0f;
                let a0 = 0.5 * s * (1.0 - f0) * (1.0 - f0);
                let f1 = x1 - x1c + 1.0;
                let am = 0.5 * s * f1 * f1;
                self.acc[line + x0i] += d * a0;
                if x1i == x0i + 2 {
                    self.acc[line + x0i + 1] += d * (1.0 - a0 - am);
                } else {
                    let a1 = s * (1.5 - f0);
                    self.acc[line + x0i + 1] += d * (a1 - a0);
                    for xi in x0i + 2..x1i - 1 {
                        self.acc[line + xi] += d * s;
                    }
                    let a2 = a1 + (x1i - x0i - 3) as f32 * s;
                    self.acc[line + x1i - 1] += d * (1.0 - a2 - am);
                }
                self.acc[line + x1i] += d * am;
            }
            x = next;
        }
    }

    /// La couverture : la somme des aires le long de chaque ligne.
    fn coverage(self) -> Mask {
        let mut data = vec![0.0; self.width * self.height];
        for y in 0..self.height {
            let mut sum = 0.0f32;
            for x in 0..self.width {
                sum += self.acc[y * self.width + x];
                data[y * self.width + x] = sum.abs().min(1.0);
            }
        }
        Mask {
            width: self.width,
            height: self.height,
            data,
        }
    }
}

/// Les contours d'une lettre, ramenés en segments dans le repère du dessin.
struct Pen<'a> {
    raster: &'a mut Raster,
    dx: f32,
    baseline: f32,
    start: (f32, f32),
    last: (f32, f32),
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        (x + self.dx, self.baseline - y)
    }

    fn to(&mut self, p: (f32, f32)) {
        self.raster.line(self.last, p);
        self.last = p;
    }

    /// Le nombre de segments d'une courbe : assez pour qu'elle reste lisse.
    fn steps(a: (f32, f32), b: (f32, f32)) -> usize {
        let d = (b.0 - a.0).abs() + (b.1 - a.1).abs();
        (d / 2.0).ceil().clamp(2.0, 24.0) as usize
    }
}

impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        let p = self.at(x, y);
        self.start = p;
        self.last = p;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.at(x, y);
        self.to(p);
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (p0, c, p1) = (self.last, self.at(cx, cy), self.at(x, y));
        let n = Pen::steps(p0, c) + Pen::steps(c, p1);
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            self.to((
                u * u * p0.0 + 2.0 * u * t * c.0 + t * t * p1.0,
                u * u * p0.1 + 2.0 * u * t * c.1 + t * t * p1.1,
            ));
        }
    }

    fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
        let (p0, c0, c1, p1) = (self.last, self.at(c0x, c0y), self.at(c1x, c1y), self.at(x, y));
        let n = Pen::steps(p0, c0) + Pen::steps(c0, c1) + Pen::steps(c1, p1);
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.to((
                a * p0.0 + b * c0.0 + c * c1.0 + d * p1.0,
                a * p0.1 + b * c0.1 + c * c1.1 + d * p1.1,
            ));
        }
    }

    fn close(&mut self) {
        if self.last != self.start {
            let start = self.start;
            self.to(start);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink(m: &Mask) -> f32 {
        m.data.iter().sum()
    }

    #[test]
    fn a_square_is_filled_exactly() {
        // Un carré de 4 × 4 pixels, décalé d'un demi‑pixel : 16 pixels d'aire en tout,
        // les pixels du bord à moitié couverts.
        let mut r = Raster::new(10, 10);
        let pts = [(2.5, 2.5), (6.5, 2.5), (6.5, 6.5), (2.5, 6.5), (2.5, 2.5)];
        for w in pts.windows(2) {
            r.line(w[0], w[1]);
        }
        let m = r.coverage();
        assert!((ink(&m) - 16.0).abs() < 1e-3, "{}", ink(&m));
        assert!((m.data[4 * 10 + 4] - 1.0).abs() < 1e-5);
        assert!((m.data[4 * 10 + 2] - 0.5).abs() < 1e-5);
        assert!((m.data[2 * 10 + 2] - 0.25).abs() < 1e-5);
        assert_eq!(m.data[0], 0.0);
    }

    #[test]
    fn text_is_drawn_with_its_letters() {
        let m = render_text("LÜMA", 40.0).unwrap();
        assert!(m.height >= 40 && m.height < 60, "{}", m.height);
        assert!(m.width > 60 && m.width < 160, "{}", m.width);
        assert!(m.data.iter().all(|v| (0.0..=1.0).contains(v)));
        // Des pixels pleins (le fût des lettres) et des bords lissés.
        assert!(m.data.iter().any(|&v| v > 0.99));
        assert!(m.data.iter().any(|&v| v > 0.2 && v < 0.8));
        // Plus de texte, plus d'encre ; deux fois plus grand, environ quatre fois.
        let a = ink(&render_text("LÜ", 40.0).unwrap());
        assert!(ink(&m) > a * 1.5);
        let big = ink(&render_text("LÜMA", 80.0).unwrap());
        assert!((big / ink(&m) - 4.0).abs() < 0.6, "{}", big / ink(&m));
        // Un caractère que la police n'a pas laisse un blanc, sans planter.
        assert!(render_text("a\u{10FFFF}b", 20.0).is_some());
    }

    #[test]
    fn the_watermark_lands_where_asked() {
        let img = RgbImage::from_fn(400, 300, |_, _| [0.2, 0.2, 0.2]);
        // L'encre d'un quart de l'image.
        let quadrant = |img: &RgbImage, right: bool, bottom: bool| -> f32 {
            let mut s = 0.0;
            for y in 0..150 {
                for x in 0..200 {
                    let p = img.data[(y + if bottom { 150 } else { 0 }) * 400 + x + if right { 200 } else { 0 }];
                    s += (p[1] - 0.2).abs();
                }
            }
            s
        };
        for (pos, right, bottom) in [
            (Position::BottomRight, true, true),
            (Position::TopLeft, false, false),
            (Position::TopRight, true, false),
            (Position::BottomLeft, false, true),
        ] {
            let mut a = img.clone();
            let w = Watermark {
                position: pos,
                size: 6.0,
                ..Watermark::new("© LÜMA")
            };
            w.apply(&mut a);
            let here = quadrant(&a, right, bottom);
            assert!(here > 10.0, "{pos:?}");
            for (r, b) in [(false, false), (false, true), (true, false), (true, true)] {
                if (r, b) != (right, bottom) {
                    assert_eq!(quadrant(&a, r, b), 0.0, "{pos:?}");
                }
            }
        }
        // Blanc éclaircit, noir assombrit ; l'opacité règle la force, jamais au‑delà.
        let mut white = img.clone();
        Watermark {
            size: 15.0,
            ..Watermark::new("x")
        }
        .apply(&mut white);
        assert!(white.data.iter().all(|p| p[0] >= 0.2 - 1e-5 && p[0] <= 1.0 + 1e-5));
        let mut black = img.clone();
        Watermark {
            dark: true,
            size: 15.0,
            ..Watermark::new("x")
        }
        .apply(&mut black);
        assert!(black.data.iter().all(|p| p[0] <= 0.2 + 1e-5 && p[0] >= 0.0));
        let peak = |img: &RgbImage| img.data.iter().map(|p| p[0]).fold(0.0, f32::max);
        let mut faint = img.clone();
        Watermark {
            opacity: 0.25,
            size: 15.0,
            ..Watermark::new("x")
        }
        .apply(&mut faint);
        // En valeurs perçues : 0,2 → 0,2^(1/2,2) ≈ 0,48, un quart du chemin vers le blanc.
        let expected = (0.2f32.powf(1.0 / 2.2) * 0.75 + 0.25).powf(2.2);
        assert!(peak(&faint) < peak(&white) && (peak(&faint) - expected).abs() < 0.02);
        // Texte vide ou opacité nulle : l'image ne bouge pas.
        for w in [
            Watermark::new("  "),
            Watermark {
                opacity: 0.0,
                ..Watermark::new("x")
            },
        ] {
            let mut same = img.clone();
            w.apply(&mut same);
            assert_eq!(same.data, img.data);
        }
        // Plus large que l'image : coupé, sans planter.
        let mut tiny = RgbImage::from_fn(40, 30, |_, _| [0.2; 3]);
        Watermark {
            size: 25.0,
            ..Watermark::new("un très long filigrane")
        }
        .apply(&mut tiny);
    }

    #[test]
    fn positions_read_and_written() {
        for p in Position::ALL {
            assert_eq!(Position::parse(p.id()), Some(p));
        }
        assert_eq!(Position::parse("bas-droite"), Some(Position::BottomRight));
        assert_eq!(Position::parse("Centre"), Some(Position::Center));
        assert_eq!(Position::parse("Top Left"), Some(Position::TopLeft));
        assert_eq!(Position::parse("dessus"), None);
        assert_eq!(
            Watermark::new("© Maxime").describe(),
            "filigrane « © Maxime », en bas à droite, 3 %, 70 %"
        );
    }
}
