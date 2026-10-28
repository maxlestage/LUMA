//! Le livre photo : une couverture (la première photo, le titre et le sous‑titre
//! dessous), puis des pages d'une, deux ou quatre photos, choisies d'après
//! l'orientation des photos pour qu'elles occupent bien la page ; le tout en PDF, prêt
//! pour un imprimeur ou une imprimante.
//!
//! Le rythme des pages : une photo seule, deux pages de deux, une page de quatre, et
//! ainsi de suite ; deux photos ne partagent une page que si elles ont la même
//! orientation (deux paysages l'une sous l'autre, deux portraits côte à côte).

use luma_color::proof::Conversion;

use crate::print::{self, Cell, Layout, Page, Paper, Picture};

/// Les formats de livre courants, en millimètres.
pub const FORMATS: [Paper; 5] = [
    Paper {
        name: "20x20",
        width: 200.0,
        height: 200.0,
    },
    Paper {
        name: "30x30",
        width: 300.0,
        height: 300.0,
    },
    Paper {
        name: "A4",
        width: 210.0,
        height: 297.0,
    },
    Paper {
        name: "21x28",
        width: 210.0,
        height: 280.0,
    },
    Paper {
        name: "A3",
        width: 297.0,
        height: 420.0,
    },
];

/// Le livre : son format, ses marges, son titre.
#[derive(Clone, Debug, PartialEq)]
pub struct Book {
    pub paper: Paper,
    pub landscape: bool,
    /// Les marges et l'espace entre les photos, en millimètres.
    pub margin: f32,
    pub gap: f32,
    pub dpi: u32,
    pub title: String,
    pub subtitle: String,
    /// Le nom de chaque photo dessous.
    pub captions: bool,
    /// Une page de texte après la couverture (une dédicace, un mot d'introduction) et
    /// une à la fin ; vides : pas de page. Une ligne par ligne du texte.
    pub opening: String,
    pub closing: String,
}

impl Default for Book {
    fn default() -> Self {
        Book {
            paper: FORMATS[0],
            landscape: false,
            margin: 15.0,
            gap: 6.0,
            dpi: 300,
            title: String::new(),
            subtitle: String::new(),
            captions: false,
            opening: String::new(),
            closing: String::new(),
        }
    }
}

/// Une page du livre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// La première photo, le titre et le sous‑titre.
    Cover,
    Single,
    /// Deux photos de même orientation.
    Pair,
    Four,
    /// Les pages de texte : après la couverture, et à la fin.
    Opening,
    Closing,
}

/// Une page et les rangs de ses photos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sheet {
    pub kind: Kind,
    pub photos: Vec<usize>,
}

/// Les pages du livre pour des photos de cette orientation (`true` : paysage), dans
/// leur ordre.
pub fn plan(landscape: &[bool]) -> Vec<Sheet> {
    let mut sheets = Vec::new();
    if landscape.is_empty() {
        return sheets;
    }
    sheets.push(Sheet {
        kind: Kind::Cover,
        photos: vec![0],
    });
    let mut i = 1;
    let mut beat = 0;
    while i < landscape.len() {
        let left = landscape.len() - i;
        let same = |n: usize| left >= n && landscape[i..i + n].iter().all(|&o| o == landscape[i]);
        let kind = match beat % 4 {
            3 if same(4) => Kind::Four,
            1..=3 if same(2) => Kind::Pair,
            _ => Kind::Single,
        };
        let n = match kind {
            Kind::Four => 4,
            Kind::Pair => 2,
            _ => 1,
        };
        sheets.push(Sheet {
            kind,
            photos: (i..i + n).collect(),
        });
        i += n;
        beat += 1;
    }
    sheets
}

impl Book {
    /// La mise en page d'impression de ce livre (papier, marges, résolution).
    pub fn layout(&self) -> Layout {
        Layout {
            paper: self.paper,
            landscape: self.landscape,
            margin: self.margin,
            gap: self.gap,
            dpi: self.dpi,
            captions: self.captions,
            ..Layout::default()
        }
    }

    /// Les pages du livre : celles des photos ([`plan`]), avec ses pages de texte.
    pub fn sheets(&self, landscape: &[bool]) -> Vec<Sheet> {
        let mut sheets = plan(landscape);
        if sheets.is_empty() {
            return sheets;
        }
        if !self.opening.trim().is_empty() {
            sheets.insert(
                1,
                Sheet {
                    kind: Kind::Opening,
                    photos: Vec::new(),
                },
            );
        }
        if !self.closing.trim().is_empty() {
            sheets.push(Sheet {
                kind: Kind::Closing,
                photos: Vec::new(),
            });
        }
        sheets
    }

    /// Les cases d'une page ; `landscape` : l'orientation de ses photos.
    pub fn cells(&self, kind: Kind, landscape: bool) -> Vec<Cell> {
        let mut l = self.layout();
        (l.columns, l.rows) = match kind {
            Kind::Opening | Kind::Closing => return Vec::new(),
            Kind::Cover | Kind::Single => (1, 1),
            Kind::Four => (2, 2),
            // Deux paysages l'un sous l'autre, deux portraits côte à côte.
            Kind::Pair if landscape => (1, 2),
            Kind::Pair => (2, 1),
        };
        if kind == Kind::Cover {
            // La photo dans les deux tiers du haut ; le titre dessous.
            l.captions = false;
            let mut cells = l.cells();
            if let Some(c) = cells.first_mut() {
                c.height = c.height * 2 / 3;
            }
            return cells;
        }
        l.cells()
    }

    /// Pose les photos d'une page (déjà développées pour ses cases) ; la couverture
    /// reçoit son titre et son sous‑titre.
    pub fn page(&self, kind: Kind, cells: &[Cell], pictures: &[Picture], printer: Option<&Conversion>) -> Page {
        let (w, h) = self.layout().page_px();
        let mut page = print::page(w, h, cells, pictures, None);
        if kind == Kind::Cover
            && let Some(photo) = cells.first()
        {
            let (_, ph) = self.layout().page_mm();
            let below = photo.y + photo.height;
            let room = h.saturating_sub(below + photo.y);
            let title_px = (ph / 25.4 * self.dpi as f32 * 0.045).max(8.0);
            let band = |y: usize, height: usize| Cell {
                y,
                height,
                caption: 0,
                ..*photo
            };
            print::write(&mut page, &self.title, &band(below + room / 6, room / 3), title_px, 30);
            print::write(
                &mut page,
                &self.subtitle,
                &band(below + room / 2, room / 4),
                title_px * 0.5,
                110,
            );
        }
        if matches!(kind, Kind::Opening | Kind::Closing) {
            let text = if kind == Kind::Opening {
                &self.opening
            } else {
                &self.closing
            };
            self.text_page(&mut page, text);
        }
        if let Some(c) = printer {
            c.apply_rgb(&mut page.rgb);
        }
        page
    }
}

impl Book {
    /// Le texte d'une page de texte : ses lignes centrées, au milieu de la page.
    fn text_page(&self, page: &mut Page, text: &str) {
        let (_, ph) = self.layout().page_mm();
        let size = (ph / 25.4 * self.dpi as f32 * 0.024).max(8.0);
        let lines: Vec<&str> = text.trim().lines().map(str::trim).collect();
        let step = (size * 1.7).round() as usize;
        let top = (page.height / 2).saturating_sub(step * lines.len() / 2);
        let margin = (self.margin / 25.4 * self.dpi as f32) as usize;
        for (k, line) in lines.iter().enumerate() {
            let band = Cell {
                x: margin,
                y: top + k * step,
                width: page.width.saturating_sub(2 * margin),
                height: step,
                caption: 0,
            };
            print::write(page, line, &band, size, 50);
        }
    }
}

/// La photo est‑elle en paysage, une fois recadrée ?
pub fn is_landscape(photo: &crate::Photo, settings: &luma_project::Settings) -> bool {
    let (w, h) = photo.full_size();
    let c = &settings.crop;
    w as f32 * (c.right - c.left) >= h as f32 * (c.bottom - c.top)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(sheets: &[Sheet]) -> Vec<(Kind, usize)> {
        sheets.iter().map(|s| (s.kind, s.photos.len())).collect()
    }

    #[test]
    fn pages_of_text() {
        let b = Book {
            opening: "Pour Lou,\nqui était là.".into(),
            closing: "Islande, été 2026".into(),
            dpi: 72,
            ..Book::default()
        };
        let sheets = b.sheets(&[true; 3]);
        assert_eq!(sheets[1].kind, Kind::Opening);
        assert_eq!(sheets.last().unwrap().kind, Kind::Closing);
        assert_eq!(sheets.len(), plan(&[true; 3]).len() + 2);
        assert!(b.cells(Kind::Opening, true).is_empty());
        // Une page blanche, ses deux lignes au milieu.
        let page = b.page(Kind::Opening, &[], &[], None);
        let rows: Vec<usize> = (0..page.height)
            .filter(|&y| (0..page.width).any(|x| page.rgb[(y * page.width + x) * 3] < 128))
            .collect();
        assert!(!rows.is_empty());
        let (first, last) = (rows[0], *rows.last().unwrap());
        assert!(
            first > page.height / 3 && last < page.height * 2 / 3,
            "{first}..{last} sur {}",
            page.height
        );
        // Deux lignes : un blanc entre elles.
        assert!(rows.windows(2).any(|w| w[1] > w[0] + 1));
        // Sans texte : pas de page de texte.
        assert_eq!(Book::default().sheets(&[true; 3]), plan(&[true; 3]));
        assert!(Book::default().sheets(&[]).is_empty());
    }

    #[test]
    fn the_rhythm_of_the_pages() {
        // Dix paysages : couverture, une seule, deux, deux, quatre, puis une.
        let sheets = plan(&[true; 10]);
        assert_eq!(
            kinds(&sheets),
            [
                (Kind::Cover, 1),
                (Kind::Single, 1),
                (Kind::Pair, 2),
                (Kind::Pair, 2),
                (Kind::Four, 4)
            ]
        );
        let all: Vec<usize> = sheets.iter().flat_map(|s| s.photos.clone()).collect();
        assert_eq!(all, (0..10).collect::<Vec<_>>(), "chaque photo une fois, dans l'ordre");
        assert!(plan(&[]).is_empty());
        assert_eq!(kinds(&plan(&[false])), [(Kind::Cover, 1)]);
    }

    #[test]
    fn mixed_orientations_are_not_paired() {
        // Paysage, portrait, paysage, portrait… : jamais deux sur une page.
        let alternate: Vec<bool> = (0..6).map(|i| i % 2 == 0).collect();
        assert!(plan(&alternate).iter().all(|s| s.photos.len() == 1));
        // Deux portraits côte à côte, deux paysages l'un sous l'autre.
        let b = Book::default();
        let side = b.cells(Kind::Pair, false);
        assert_eq!(side[0].y, side[1].y);
        let stacked = b.cells(Kind::Pair, true);
        assert_eq!(stacked[0].x, stacked[1].x);
        assert!(stacked[1].y > stacked[0].y);
    }

    #[test]
    fn the_cover_has_its_title() {
        let b = Book {
            dpi: 60,
            title: "Islande".into(),
            subtitle: "Été 2026".into(),
            ..Book::default()
        };
        let cells = b.cells(Kind::Cover, true);
        let (w, h) = b.layout().page_px();
        let photo = Picture {
            width: cells[0].width,
            height: cells[0].height,
            rgb: vec![200; cells[0].width * cells[0].height * 3],
            caption: String::new(),
        };
        let page = b.page(Kind::Cover, &cells, &[photo], None);
        assert_eq!((page.width, page.height), (w, h));
        // De l'encre sous la photo (le titre), du papier blanc ailleurs.
        let below = cells[0].y + cells[0].height;
        let ink = (below..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| page.rgb[(y * w + x) * 3] < 150)
            .count();
        assert!(ink > 40, "{ink}");
        let blank = Book {
            title: String::new(),
            subtitle: String::new(),
            ..b.clone()
        };
        let photo = Picture {
            width: 4,
            height: 4,
            rgb: vec![200; 48],
            caption: String::new(),
        };
        let bare = blank.page(Kind::Cover, &cells, &[photo], None);
        assert!(bare.rgb[below * w * 3..].iter().all(|&v| v == 255));
    }
}
