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
use std::path::{Path, PathBuf};

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
    /// Des repères de coupe aux coins de chaque photo, dans les marges et les espaces.
    pub crop_marks: bool,
    /// Une ligne de texte au bas de chaque page (un titre, un nom, des droits), dans la
    /// marge du bas ; vide : rien.
    pub text: String,
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
            crop_marks: false,
            text: String::new(),
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
    /// Les encres de la page pour une imprimante CMJN (quatre octets par pixel, 0 sans
    /// encre) ; `rgb` reste alors la page en sRGB, pour l'aperçu.
    pub cmyk: Option<Vec<u8>>,
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
            let mut p = page(pw, ph, &cells, chunk, printer);
            if layout.crop_marks {
                for (pic, cell) in chunk.iter().zip(&cells) {
                    crop_marks(&mut p, placed(pic, cell), layout.px(1.0), layout.px(4.0));
                }
            }
            if !layout.text.trim().is_empty() {
                footer(&mut p, layout);
            }
            p
        })
        .collect()
}

/// La place d'une photo dans sa case, en pixels : (x, y, largeur, hauteur).
fn placed(pic: &Picture, cell: &Cell) -> (usize, usize, usize, usize) {
    let (w, h) = (pic.width.min(cell.width), pic.height.min(cell.height));
    let x = cell.x + (cell.width - w) / 2;
    let y = if cell.caption > 0 {
        cell.y + cell.height - h
    } else {
        cell.y + (cell.height - h) / 2
    };
    (x, y, w, h)
}

/// Les repères de coupe d'une photo : à chaque coin, un trait horizontal et un trait
/// vertical, noirs, dans le prolongement de ses bords, à `gap` pixels d'elle et longs de
/// `len` (assez fins pour un massicot : un tiers de millimètre).
fn crop_marks(page: &mut Page, (x, y, w, h): (usize, usize, usize, usize), gap: usize, len: usize) {
    let thick = (gap / 3).max(1);
    let mut fill = |x0: isize, y0: isize, x1: isize, y1: isize| {
        for py in y0.max(0)..y1.min(page.height as isize) {
            for px in x0.max(0)..x1.min(page.width as isize) {
                let i = (py as usize * page.width + px as usize) * 3;
                page.rgb[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
    };
    let (x, y, w, h, g, l, t) = (
        x as isize,
        y as isize,
        w as isize,
        h as isize,
        gap as isize,
        len as isize,
        thick as isize,
    );
    for (cx, sx) in [(x, -1), (x + w, 1)] {
        for (cy, sy) in [(y, -1), (y + h, 1)] {
            // Le trait horizontal, dans le prolongement du bord du haut ou du bas.
            let (hx0, hx1) = if sx < 0 {
                (cx - g - l, cx - g)
            } else {
                (cx + g, cx + g + l)
            };
            let hy = if sy < 0 { cy } else { cy - t };
            fill(hx0, hy, hx1, hy + t);
            // Le trait vertical, dans le prolongement du bord de gauche ou de droite.
            let (vy0, vy1) = if sy < 0 {
                (cy - g - l, cy - g)
            } else {
                (cy + g, cy + g + l)
            };
            let vx = if sx < 0 { cx } else { cx - t };
            fill(vx, vy0, vx + t, vy1);
        }
    }
}

/// La ligne de texte du bas de la page, centrée dans la marge du bas, en gris foncé.
fn footer(page: &mut Page, layout: &Layout) {
    let band = layout.px(layout.margin);
    if band < 4 {
        return;
    }
    let cell = Cell {
        x: 0,
        y: page.height - band,
        width: page.width,
        height: band,
        caption: 0,
    };
    // Environ 9 points, moins dans une marge étroite.
    let size = (layout.dpi as f32 * 9.0 / 72.0).min(band as f32 * 0.5);
    write(page, layout.text.trim(), &cell, size, 60);
}

/// Une page de `width × height` pixels : chaque photo dans sa case, son nom dessous,
/// puis la conversion vers l'imprimante.
pub fn page(width: usize, height: usize, cells: &[Cell], pictures: &[Picture], printer: Option<&Conversion>) -> Page {
    let mut page = Page {
        width,
        height,
        rgb: vec![255; width * height * 3],
        cmyk: None,
    };
    for (pic, cell) in pictures.iter().zip(cells) {
        place(&mut page, pic, cell);
        if cell.caption > 0 && !pic.caption.is_empty() {
            caption(&mut page, &pic.caption, cell);
        }
    }
    if let Some(c) = printer {
        convert(&mut page, c);
    }
    page
}

/// Convertit une page vers l'imprimante : ses valeurs RVB, ou ses encres CMJN.
pub fn convert(page: &mut Page, c: &Conversion) {
    if c.is_cmyk() {
        page.cmyk = Some(c.to_cmyk(&page.rgb));
    } else {
        c.apply_rgb(&mut page.rgb);
    }
}

/// Pose une photo au centre de sa case (au bas de sa case quand son nom est dessous,
/// qui la suit alors de près) ; ce qui dépasse (case remplie) est coupé.
fn place(page: &mut Page, pic: &Picture, cell: &Cell) {
    let (dx, dy, w, h) = placed(pic, cell);
    let (sx, sy) = ((pic.width - w) / 2, (pic.height - h) / 2);
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
    let band = Cell {
        y: cell.y + cell.height,
        height: cell.caption,
        caption: 0,
        ..*cell
    };
    write(page, text, &band, cell.caption as f32 * 0.55, 60);
}

/// Une ligne de texte (police de LÜMA) de `px` pixels, centrée dans cette case, de
/// cette encre (gris, 0 noir) ; coupée si elle est trop longue.
pub fn write(page: &mut Page, text: &str, cell: &Cell, px: f32, ink: u8) {
    let Some(mask) = crate::watermark::render_text(text, px) else {
        return;
    };
    let w = mask.width.min(cell.width);
    let x0 = cell.x + (cell.width - w) / 2;
    let y0 = cell.y + cell.height.saturating_sub(mask.height) / 2;
    let ink = f32::from(ink);
    for y in 0..mask.height {
        let py = y0 + y;
        if py >= page.height {
            break;
        }
        for x in 0..w {
            let a = mask.data[y * mask.width + x].clamp(0.0, 1.0);
            if a > 0.0 && x0 + x < page.width {
                let i = (py * page.width + x0 + x) * 3;
                for c in 0..3 {
                    let v = f32::from(page.rgb[i + c]);
                    page.rgb[i + c] = (v + (ink - v) * a).round() as u8;
                }
            }
        }
    }
}

/// Une page compressée pour le PDF, tout de suite : la mémoire ne garde pas toutes les
/// pages en pleine résolution.
pub type Encoded = crate::pdf::Image;

/// La page en JPEG (qualité 95) ; ses encres CMJN, sans perte (un JPEG CMJN n'est pas
/// lu de la même façon par tous les lecteurs de PDF).
pub fn encode(page: &Page) -> Result<Encoded, String> {
    Ok(match &page.cmyk {
        Some(ink) => crate::pdf::Image {
            data: miniz_oxide::deflate::compress_to_vec_zlib(ink, 6),
            width: page.width,
            height: page.height,
            cmyk: true,
        },
        None => crate::pdf::Image {
            data: crate::jpeg::encode(&page.rgb, page.width, page.height, 95, &[], &[])?,
            width: page.width,
            height: page.height,
            cmyk: false,
        },
    })
}

/// Les pages d'une impression ou d'un livre, compressées à mesure : pour un PDF, ou une
/// image JPEG par page (selon l'extension du fichier demandé : `.jpg`, `.jpeg`), à
/// envoyer à un service d'impression ou à partager.
pub enum Pages {
    Pdf(Vec<Encoded>),
    /// Les JPEG des pages ; `srgb` : le profil sRGB y est inscrit (sans conversion vers
    /// une imprimante, les pages sont en sRGB).
    Jpeg {
        pages: Vec<Vec<u8>>,
        srgb: bool,
    },
}

/// Le fichier demandé est‑il un JPEG ?
pub fn is_jpeg(out: &Path) -> bool {
    out.extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| matches!(x.to_ascii_lowercase().as_str(), "jpg" | "jpeg"))
}

/// Les fichiers des pages en JPEG : `out` pour une seule, sinon `nom-1.jpg`,
/// `nom-2.jpg`… à côté.
pub fn page_paths(out: &Path, n: usize) -> Vec<PathBuf> {
    if n <= 1 {
        return vec![out.to_path_buf()];
    }
    let stem = out
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let ext = out
        .extension()
        .map_or_else(|| "jpg".into(), |x| x.to_string_lossy().into_owned());
    let width = n.to_string().len();
    (1..=n)
        .map(|k| out.with_file_name(format!("{stem}-{k:0width$}.{ext}")))
        .collect()
}

impl Pages {
    /// Pour ce fichier ; `srgb` : les pages restent en sRGB (pas d'imprimante).
    pub fn for_path(out: &Path, srgb: bool) -> Pages {
        if is_jpeg(out) {
            Pages::Jpeg {
                pages: Vec::new(),
                srgb,
            }
        } else {
            Pages::Pdf(Vec::new())
        }
    }

    /// Ajoute une page, compressée tout de suite. Une page CMJN ne va que dans un PDF : un
    /// JPEG CMJN n'est pas lu de la même façon par tous.
    pub fn push(&mut self, page: &Page, layout: &Layout) -> Result<(), String> {
        match self {
            Pages::Pdf(pages) => pages.push(encode(page)?),
            Pages::Jpeg { pages, srgb } => {
                if page.cmyk.is_some() {
                    return Err("en CMJN, les pages vont dans un PDF (choisissez un fichier .pdf)".into());
                }
                let icc = if *srgb {
                    luma_color::icc::profile(OutputSpace::Srgb)
                } else {
                    Vec::new()
                };
                let mut jpeg = crate::jpeg::encode(&page.rgb, page.width, page.height, 95, &[], &icc)?;
                crate::jpeg::set_density(&mut jpeg, layout.dpi);
                pages.push(jpeg);
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        match self {
            Pages::Pdf(p) => p.len(),
            Pages::Jpeg { pages, .. } => pages.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Écrit le PDF, ou les JPEG ; rend les fichiers écrits.
    pub fn write(self, out: &Path, layout: &Layout) -> Result<Vec<PathBuf>, String> {
        let err = |p: &Path, e: std::io::Error| format!("{} : {e}", p.display());
        match self {
            Pages::Pdf(pages) => {
                std::fs::write(out, pdf_of(layout, &pages)).map_err(|e| err(out, e))?;
                Ok(vec![out.to_path_buf()])
            }
            Pages::Jpeg { pages, .. } => {
                let paths = page_paths(out, pages.len());
                for (path, jpeg) in paths.iter().zip(&pages) {
                    std::fs::write(path, jpeg).map_err(|e| err(path, e))?;
                }
                Ok(paths)
            }
        }
    }
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
    fn crop_marks_and_a_line_of_text() {
        let l = Layout {
            paper: paper("100x100").unwrap(),
            dpi: 254,
            margin: 10.0,
            crop_marks: true,
            text: "© Maxime Lestage".into(),
            ..Layout::default()
        };
        // 100 mm à 254 points par pouce : 1000 pixels, 10 par millimètre.
        assert_eq!(l.page_px(), (1000, 1000));
        let cell = l.cells()[0];
        let (w, h) = l.target(&cell, 3000, 2000);
        let pages = compose(&l, &[solid(w, h, 200, "")], None);
        let p = &pages[0];
        let at = |x: usize, y: usize| &p.rgb[(y * p.width + x) * 3..(y * p.width + x) * 3 + 3];
        let (x, y, _, ph) = placed(&solid(w, h, 200, ""), &cell);
        // Au coin haut gauche : un trait horizontal à gauche de la photo, sur son bord du
        // haut, à 1 mm d'elle et long de 4 mm ; et un trait vertical au‑dessus.
        assert_eq!(at(x - 20, y), [0, 0, 0]);
        assert_eq!(at(x, y - 20), [0, 0, 0]);
        assert_eq!(at(x - 5, y), [255, 255, 255], "l'espace d'1 mm reste blanc");
        assert_eq!(at(x - 60, y), [255, 255, 255], "le trait ne fait que 4 mm");
        assert_eq!(at(x, y), [200, 200, 200], "la photo n'est pas touchée");
        assert_eq!(at(x - 20, y + ph - 1), [0, 0, 0], "le coin bas gauche aussi");
        // Le texte, dans la marge du bas.
        let dark = (900..1000)
            .flat_map(|y| (0..1000).map(move |x| (x, y)))
            .filter(|&(x, y)| at(x, y)[0] < 128)
            .count();
        assert!(dark > 50, "{dark}");
        // Sans rien : la page n'a que du blanc et la photo.
        let plain = compose(
            &Layout {
                crop_marks: false,
                text: String::new(),
                ..l.clone()
            },
            &[solid(w, h, 200, "")],
            None,
        );
        assert!(plain[0].rgb.iter().all(|&v| v == 255 || v == 200));
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

    /// Les pages en JPEG : une image par page, nommée d'après le fichier demandé, sa
    /// résolution inscrite et le profil sRGB ; une page CMJN refusée.
    #[test]
    fn pages_as_jpeg_files() {
        let dir = std::env::temp_dir().join(format!("luma-pages-jpeg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let l = Layout {
            dpi: 30,
            columns: 1,
            rows: 1,
            ..Layout::default()
        };
        let out = dir.join("Planche.JPG");
        assert!(is_jpeg(&out) && !is_jpeg(&dir.join("a.pdf")));
        let mut pages = Pages::for_path(&out, true);
        for page in compose(&l, &[solid(40, 40, 0, ""), solid(40, 40, 200, "")], None) {
            pages.push(&page, &l).unwrap();
        }
        let written = pages.write(&out, &l).unwrap();
        assert_eq!(written, [dir.join("Planche-1.JPG"), dir.join("Planche-2.JPG")]);
        let jpeg = std::fs::read(&written[0]).unwrap();
        // JFIF à 30 ppp, et le profil sRGB (segment ICC_PROFILE).
        assert_eq!((jpeg[13], u16::from_be_bytes([jpeg[14], jpeg[15]])), (1, 30));
        assert!(jpeg.windows(11).any(|w| w == b"ICC_PROFILE"));
        // Une seule page : le fichier demandé lui‑même.
        assert_eq!(page_paths(&out, 1), std::slice::from_ref(&out));
        assert_eq!(page_paths(&dir.join("l.jpg"), 12)[0], dir.join("l-01.jpg"));
        // En CMJN : pas de JPEG.
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../luma-color/tests/profils/encre-noire.icc"
        ))
        .unwrap();
        let c = Conversion::new(
            &luma_color::profile::Profile::parse(&bytes).unwrap(),
            luma_color::profile::Intent::Relative,
        )
        .unwrap();
        let page = &compose(&l, &[solid(40, 40, 0, "")], Some(&c))[0];
        assert!(
            Pages::for_path(&out, false)
                .push(page, &l)
                .unwrap_err()
                .contains("CMJN")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Vers une imprimante CMJN : la page garde ses encres, le PDF les porte en CMJN,
    /// sans perte ; le papier reste sans encre.
    #[test]
    fn a_cmyk_printer_gets_a_cmyk_pdf() {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../luma-color/tests/profils/encre-noire.icc"
        ))
        .unwrap();
        let profile = luma_color::profile::Profile::parse(&bytes).unwrap();
        let c = Conversion::new(&profile, luma_color::profile::Intent::Relative).unwrap();
        let l = Layout {
            dpi: 20,
            ..Layout::default()
        };
        let cells = l.cells();
        let pages = compose(&l, &[solid(40, 40, 0, "")], Some(&c));
        let ink = pages[0].cmyk.as_ref().expect("les encres de la page");
        assert_eq!(ink.len(), pages[0].width * pages[0].height * 4);
        // Le coin : du papier, sans encre ; la photo noire : de l'encre noire.
        assert_eq!(ink[..4], [0, 0, 0, 0]);
        let (x, y) = (cells[0].x + cells[0].width / 2, cells[0].y + cells[0].height / 2);
        let k = ink[(y * pages[0].width + x) * 4 + 3];
        assert!(k > 240, "{k}");
        let encoded = encode(&pages[0]).unwrap();
        assert!(encoded.cmyk);
        assert_eq!(
            miniz_oxide::inflate::decompress_to_vec_zlib(&encoded.data).unwrap(),
            *ink
        );
        let pdf = pdf_of(&l, &[encoded]);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/ColorSpace /DeviceCMYK") && text.contains("/Filter /FlateDecode"));
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
