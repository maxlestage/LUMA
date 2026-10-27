//! L'impression : la mise en page (papier, orientation, marges), la planche contact
//! (une grille de photos par page, leur nom dessous), la conversion vers le profil de
//! l'imprimante, et le PDF à envoyer à l'imprimante (une image par page, à la
//! résolution d'impression, à la taille exacte du papier).
//!
//! Les mesures sont en millimètres ; une page devient une image de
//! `papier × résolution` pixels, blanche (le papier), où chaque photo est posée au
//! centre de sa case : entière (ajustée) ou remplissant la case (recadrée au centre).

use luma_color::OutputSpace;
use luma_color::proof::Conversion;
use luma_core::{Cancel, RgbImage};
use luma_project::Settings;

use crate::export::OutputSharpen;
use crate::{Photo, Size};

/// Un format de papier, en millimètres (portrait).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    pub name: &'static str,
    pub width: f32,
    pub height: f32,
}

/// Les formats courants des imprimantes photo.
pub const PAPERS: [Paper; 9] = [
    Paper {
        name: "A4",
        width: 210.0,
        height: 297.0,
    },
    Paper {
        name: "A3",
        width: 297.0,
        height: 420.0,
    },
    Paper {
        name: "A3+",
        width: 329.0,
        height: 483.0,
    },
    Paper {
        name: "A5",
        width: 148.0,
        height: 210.0,
    },
    Paper {
        name: "Letter",
        width: 215.9,
        height: 279.4,
    },
    Paper {
        name: "10x15",
        width: 101.6,
        height: 152.4,
    },
    Paper {
        name: "13x18",
        width: 127.0,
        height: 177.8,
    },
    Paper {
        name: "20x25",
        width: 203.2,
        height: 254.0,
    },
    Paper {
        name: "20x30",
        width: 203.2,
        height: 304.8,
    },
];

/// Un format par son nom (« A4 », « a3+ », « 10x15 »), ou « 210x297 » en millimètres.
pub fn paper(name: &str) -> Option<Paper> {
    let n = name.trim().to_lowercase().replace(['×', ' '], "x");
    if let Some(p) = PAPERS.iter().find(|p| p.name.to_lowercase() == n) {
        return Some(*p);
    }
    let (w, h) = n.trim_end_matches("mm").split_once('x')?;
    let (width, height) = (w.parse::<f32>().ok()?, h.parse::<f32>().ok()?);
    (width >= 20.0 && height >= 20.0 && width <= 2000.0 && height <= 2000.0).then_some(Paper {
        name: "",
        width,
        height,
    })
}

/// La mise en page.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub paper: Paper,
    pub landscape: bool,
    /// Les marges, en millimètres, de chaque côté.
    pub margin: f32,
    /// La grille : une photo par page (1 × 1), ou une planche contact.
    pub columns: usize,
    pub rows: usize,
    /// L'espace entre deux cases, en millimètres.
    pub gap: f32,
    /// Points par pouce.
    pub dpi: u32,
    /// Le nom de chaque photo sous elle.
    pub captions: bool,
    /// La photo remplit sa case (recadrée au centre) au lieu d'y tenir entière.
    pub fill: bool,
    /// L'accentuation pour le papier.
    pub sharpen: Option<OutputSharpen>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            paper: PAPERS[0],
            landscape: false,
            margin: 10.0,
            columns: 1,
            rows: 1,
            gap: 5.0,
            dpi: 300,
            captions: false,
            fill: false,
            sharpen: None,
        }
    }
}

/// Une case de la page, en pixels : la place de la photo et celle de son nom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    /// La hauteur de la bande du nom, sous la photo (0 sans nom).
    pub caption: usize,
}

/// La hauteur de la bande du nom, en millimètres.
const CAPTION_MM: f32 = 5.0;

impl Layout {
    /// La planche contact : `columns × rows` photos par page, avec leur nom.
    pub fn contact_sheet(paper: Paper, columns: usize, rows: usize) -> Layout {
        Layout {
            paper,
            columns,
            rows,
            gap: 4.0,
            captions: true,
            ..Layout::default()
        }
    }

    /// La page, en millimètres, dans son orientation.
    pub fn page_mm(&self) -> (f32, f32) {
        let (w, h) = (self.paper.width, self.paper.height);
        if self.landscape == (w < h) { (h, w) } else { (w, h) }
    }

    fn px(&self, mm: f32) -> usize {
        (mm / 25.4 * self.dpi as f32).round().max(0.0) as usize
    }

    /// La page, en pixels.
    pub fn page_px(&self) -> (usize, usize) {
        let (w, h) = self.page_mm();
        (self.px(w).max(1), self.px(h).max(1))
    }

    pub fn per_page(&self) -> usize {
        self.columns.max(1) * self.rows.max(1)
    }

    /// Le nombre de pages pour `photos` photos.
    pub fn pages(&self, photos: usize) -> usize {
        photos.div_ceil(self.per_page())
    }

    /// Les cases d'une page, ligne par ligne. Vide si les marges ne laissent plus de
    /// place.
    pub fn cells(&self) -> Vec<Cell> {
        let (cols, rows) = (self.columns.max(1), self.rows.max(1));
        let (w, h) = self.page_mm();
        let inner_w = w - 2.0 * self.margin - self.gap * (cols - 1) as f32;
        let inner_h = h - 2.0 * self.margin - self.gap * (rows - 1) as f32;
        let (cw, ch) = (inner_w / cols as f32, inner_h / rows as f32);
        let caption = if self.captions { CAPTION_MM.min(ch / 3.0) } else { 0.0 };
        if cw < 1.0 || ch - caption < 1.0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                let x = self.margin + c as f32 * (cw + self.gap);
                let y = self.margin + r as f32 * (ch + self.gap);
                let (x0, y0) = (self.px(x), self.px(y));
                let (x1, y1) = (self.px(x + cw), self.px(y + ch));
                let band = self.px(caption);
                out.push(Cell {
                    x: x0,
                    y: y0,
                    width: (x1 - x0).max(1),
                    height: (y1 - y0).saturating_sub(band).max(1),
                    caption: band,
                });
            }
        }
        out
    }

    /// La taille à laquelle développer une image de `width × height` pour une case :
    /// entière dans la case, ou la couvrant (recadrée ensuite au centre).
    pub fn target(&self, cell: &Cell, width: usize, height: usize) -> (usize, usize) {
        let (w, h) = (width.max(1) as f64, height.max(1) as f64);
        let sx = cell.width as f64 / w;
        let sy = cell.height as f64 / h;
        let s = if self.fill { sx.max(sy) } else { sx.min(sy) };
        (((w * s).round() as usize).max(1), ((h * s).round() as usize).max(1))
    }
}

/// Une photo prête à poser : RVB 8 bits en sRGB, et son nom.
#[derive(Clone, Debug)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
    pub caption: String,
}

/// Développe une photo pour cette case : à la taille exacte (entière ou couvrant la
/// case), accentuée pour le papier.
pub fn develop(
    photo: &Photo,
    settings: &Settings,
    layout: &Layout,
    cell: &Cell,
    caption: String,
) -> Result<Picture, String> {
    let c = &settings.crop;
    let (fw, fh) = photo.full_size();
    let (cw, ch) = (
        (fw as f32 * (c.right - c.left).max(0.05)) as usize,
        (fh as f32 * (c.bottom - c.top).max(0.05)) as usize,
    );
    // L'orientation après rotation n'est connue qu'au rendu : un premier essai à la
    // taille de la case, puis la réduction exacte.
    let long = cell.width.max(cell.height);
    let (tw, th) = layout.target(cell, cw, ch);
    let need = tw.max(th).max(long);
    let frac = (c.right - c.left).min(c.bottom - c.top).max(0.05);
    let pre = (need as f32 / frac).ceil() as usize;
    let size = if pre >= fw.max(fh) { Size::Full } else { Size::Fit(pre) };
    let rendered = photo
        .render(settings, size, &Cancel::new())
        .map_err(|e| e.to_string())?;
    Ok(picture(&rendered.image, layout, cell, caption))
}

/// Une image développée (linéaire), mise à la taille de la case et accentuée.
pub fn picture(image: &RgbImage, layout: &Layout, cell: &Cell, caption: String) -> Picture {
    let (w, h) = layout.target(cell, image.width, image.height);
    let mut image = image.resize_area(w, h);
    if let Some(sh) = layout.sharpen {
        sh.apply(&mut image);
    }
    Picture {
        width: image.width,
        height: image.height,
        rgb: luma_image::encode8(&image, OutputSpace::Srgb),
        caption,
    }
}

/// Une page : RVB 8 bits, blanche là où il n'y a rien.
#[derive(Clone, Debug)]
pub struct Page {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

/// Pose les photos sur leurs pages (dans l'ordre, case par case), puis convertit chaque
/// page vers l'imprimante si un profil est donné.
pub fn compose(layout: &Layout, pictures: &[Picture], printer: Option<&Conversion>) -> Vec<Page> {
    let cells = layout.cells();
    if cells.is_empty() {
        return Vec::new();
    }
    let (pw, ph) = layout.page_px();
    pictures
        .chunks(cells.len())
        .map(|chunk| {
            let mut page = Page {
                width: pw,
                height: ph,
                rgb: vec![255; pw * ph * 3],
            };
            for (pic, cell) in chunk.iter().zip(&cells) {
                place(&mut page, pic, cell);
                if cell.caption > 0 && !pic.caption.is_empty() {
                    caption(&mut page, &pic.caption, cell);
                }
            }
            if let Some(c) = printer {
                c.apply_rgb(&mut page.rgb);
            }
            page
        })
        .collect()
}

/// Pose une photo au centre de sa case (au bas de sa case quand son nom est dessous,
/// qui la suit alors de près) ; ce qui dépasse (case remplie) est coupé.
fn place(page: &mut Page, pic: &Picture, cell: &Cell) {
    let (w, h) = (pic.width.min(cell.width), pic.height.min(cell.height));
    let (sx, sy) = ((pic.width - w) / 2, (pic.height - h) / 2);
    let dx = cell.x + (cell.width - w) / 2;
    let dy = if cell.caption > 0 {
        cell.y + cell.height - h
    } else {
        cell.y + (cell.height - h) / 2
    };
    for y in 0..h {
        let (py, row) = (dy + y, sy + y);
        if py >= page.height {
            break;
        }
        let w = w.min(page.width.saturating_sub(dx));
        let from = &pic.rgb[(row * pic.width + sx) * 3..(row * pic.width + sx + w) * 3];
        page.rgb[(py * page.width + dx) * 3..(py * page.width + dx + w) * 3].copy_from_slice(from);
    }
}

/// Le nom sous la photo, centré dans sa bande, en gris foncé.
fn caption(page: &mut Page, text: &str, cell: &Cell) {
    let px = cell.caption as f32 * 0.55;
    let Some(mask) = crate::watermark::render_text(text, px) else {
        return;
    };
    let w = mask.width.min(cell.width);
    let x0 = cell.x + (cell.width - w) / 2;
    let y0 = cell.y + cell.height + cell.caption.saturating_sub(mask.height) / 2;
    const INK: f32 = 60.0;
    for y in 0..mask.height {
        let py = y0 + y;
        if py >= page.height {
            break;
        }
        for x in 0..w {
            let a = mask.data[y * mask.width + x].clamp(0.0, 1.0);
            if a > 0.0 {
                let i = (py * page.width + x0 + x) * 3;
                for c in 0..3 {
                    let v = f32::from(page.rgb[i + c]);
                    page.rgb[i + c] = (v + (INK - v) * a).round() as u8;
                }
            }
        }
    }
}

/// Une page en JPEG (qualité 95), pour le PDF : la page compressée tout de suite, la
/// mémoire ne garde pas toutes les pages en pleine résolution.
pub type Encoded = (Vec<u8>, usize, usize);

pub fn encode(page: &Page) -> Result<Encoded, String> {
    Ok((
        crate::jpeg::encode(&page.rgb, page.width, page.height, 95, &[], &[])?,
        page.width,
        page.height,
    ))
}

/// Le PDF de pages déjà compressées, à la taille du papier.
pub fn pdf_of(layout: &Layout, pages: &[Encoded]) -> Vec<u8> {
    let (w, h) = layout.page_mm();
    crate::pdf::write(pages, w / 25.4 * 72.0, h / 25.4 * 72.0)
}

/// Le PDF des pages.
pub fn pdf(layout: &Layout, pages: &[Page]) -> Result<Vec<u8>, String> {
    let images = pages.iter().map(encode).collect::<Result<Vec<_>, String>>()?;
    Ok(pdf_of(layout, &images))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, v: u8, caption: &str) -> Picture {
        Picture {
            width: w,
            height: h,
            rgb: vec![v; w * h * 3],
            caption: caption.into(),
        }
    }

    #[test]
    fn papers_by_name_and_size() {
        assert_eq!(paper("a4").unwrap().width, 210.0);
        assert_eq!(paper("A3+").unwrap().height, 483.0);
        assert_eq!(paper("10×15").unwrap().name, "10x15");
        let custom = paper("150x200mm").unwrap();
        assert_eq!((custom.width, custom.height), (150.0, 200.0));
        assert!(paper("géant").is_none() && paper("5x5").is_none());
    }

    #[test]
    fn a4_at_300_dpi() {
        let mut l = Layout::default();
        assert_eq!(l.page_px(), (2480, 3508));
        l.landscape = true;
        assert_eq!(l.page_px(), (3508, 2480));
        // Une photo par page : la case, c'est la page moins les marges (1 cm).
        let c = l.cells();
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].x, c[0].y), (118, 118));
        assert_eq!((c[0].width, c[0].height), (3508 - 236, 2480 - 236));
        // 3 : 2 entière dans la case, ou la remplissant.
        assert_eq!(l.target(&c[0], 6000, 4000), (3272, 2181));
        l.fill = true;
        assert_eq!(l.target(&c[0], 6000, 4000), (3366, 2244));
        assert_eq!(l.target(&c[0], 4000, 6000).0, 3272);
    }

    #[test]
    fn contact_sheet_pages_and_cells() {
        let l = Layout::contact_sheet(PAPERS[0], 4, 5);
        assert_eq!(l.per_page(), 20);
        assert_eq!((l.pages(0), l.pages(20), l.pages(21)), (0, 1, 2));
        let cells = l.cells();
        assert_eq!(cells.len(), 20);
        // Les cases ne se chevauchent pas, et tiennent dans la page sans les marges.
        let (pw, ph) = l.page_px();
        for (i, a) in cells.iter().enumerate() {
            assert!(a.caption > 0);
            assert!(a.x >= 118 && a.x + a.width <= pw - 118 + 1);
            assert!(a.y >= 118 && a.y + a.height + a.caption <= ph - 118 + 1);
            for b in &cells[i + 1..] {
                let apart = a.x + a.width <= b.x
                    || b.x + b.width <= a.x
                    || a.y + a.height + a.caption <= b.y
                    || b.y + b.height + b.caption <= a.y;
                assert!(apart, "{a:?} {b:?}");
            }
        }
        // Des marges trop grandes : plus de case.
        let none = Layout {
            margin: 150.0,
            ..Layout::default()
        };
        assert!(none.cells().is_empty());
    }

    #[test]
    fn photos_are_centred_cropped_and_named() {
        let l = Layout {
            paper: paper("100x100").unwrap(),
            margin: 0.0,
            columns: 2,
            rows: 1,
            gap: 0.0,
            dpi: 254,
            captions: true,
            ..Layout::default()
        };
        // Deux cases de 500 px de large ; la seconde photo, trop large, est coupée.
        let cells = l.cells();
        assert_eq!(cells[0].width, 500);
        let pages = compose(
            &l,
            &[
                solid(100, 100, 0, "IMG_0001"),
                solid(900, 100, 128, ""),
                solid(10, 10, 0, ""),
            ],
            None,
        );
        assert_eq!(pages.len(), 2, "trois photos, deux par page");
        let p = &pages[0];
        let at = |x: usize, y: usize| p.rgb[(y * p.width + x) * 3];
        // Au bas de sa case, juste au‑dessus de son nom.
        let cy = cells[0].y + cells[0].height - 50;
        assert_eq!(at(250, cy), 0, "au milieu de sa case");
        assert_eq!(at(250, cells[0].y + cells[0].height - 101), 255);
        assert_eq!(at(150, cy), 255, "le papier autour");
        assert_eq!(at(199, cy), 255);
        assert_eq!(at(200, cy), 0);
        assert_eq!(at(500, cy), 128, "coupée au bord de sa case");
        assert_eq!(at(999, cy), 128);
        // Le nom est écrit sous la première photo, pas sous la seconde (sans nom).
        let band = |x0: usize, x1: usize| {
            (cells[0].y + cells[0].height..cells[0].y + cells[0].height + cells[0].caption)
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .filter(|&(x, y)| at(x, y) < 200)
                .count()
        };
        assert!(band(0, 500) > 50, "{}", band(0, 500));
        assert_eq!(band(500, 1000), 0);
    }

    #[test]
    fn the_printer_profile_converts_the_page() {
        let p3 = luma_color::profile::Profile::parse(&luma_color::icc::profile(OutputSpace::DisplayP3)).unwrap();
        let c = Conversion::new(&p3, luma_color::profile::Intent::Relative).unwrap();
        let l = Layout {
            paper: paper("50x50").unwrap(),
            margin: 0.0,
            dpi: 50,
            ..Layout::default()
        };
        let red = Picture {
            width: 98,
            height: 98,
            rgb: [255, 0, 0].repeat(98 * 98),
            caption: String::new(),
        };
        let page = &compose(&l, &[red], Some(&c))[0];
        let mid = (49 * page.width + 49) * 3;
        let px = &page.rgb[mid..mid + 3];
        assert!(px[0] < 245 && px[1] > 35, "le rouge du sRGB, en Display P3 : {px:?}");
    }

    #[test]
    fn pdf_of_two_pages() {
        let l = Layout {
            paper: paper("50x50").unwrap(),
            margin: 5.0,
            dpi: 72,
            ..Layout::default()
        };
        let pages = compose(&l, &[solid(40, 40, 30, ""), solid(40, 40, 200, "")], None);
        let bytes = pdf(&l, &pages).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(bytes.starts_with(b"%PDF-1.4"));
        assert_eq!(text.matches("/Type /Page ").count(), 2);
        assert!(text.contains("/MediaBox [0 0 141.73 141.73]"), "50 mm = 141,73 points");
        assert!(text.contains("/Filter /DCTDecode"));
        assert!(text.trim_end().ends_with("%%EOF"));
    }
}
