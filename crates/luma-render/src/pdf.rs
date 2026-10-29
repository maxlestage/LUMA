//! Un PDF d'images : une page par image JPEG, posée sur toute la page. Le strict
//! nécessaire du format (PDF 1.4) : le catalogue, l'arbre des pages, et pour chaque
//! page son image (filtre DCTDecode, le JPEG tel quel ; ou les encres CMJN brutes,
//! filtre FlateDecode) et le dessin qui la pose.

/// L'image d'une page : un JPEG RVB, ou des encres CMJN (quatre octets par pixel, 0 sans
/// encre) compressées en zlib.
#[derive(Clone, Debug)]
pub struct Image {
    pub data: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub cmyk: bool,
}

/// Le PDF de ces images, sur des pages de `width × height` points (1/72 de pouce).
pub fn write(images: &[Image], width: f32, height: f32) -> Vec<u8> {
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    let mut object = |out: &mut Vec<u8>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    // 1 : le catalogue ; 2 : les pages ; puis trois objets par page (page, dessin, image).
    let kids: Vec<String> = (0..images.len()).map(|i| format!("{} 0 R", 3 + 3 * i)).collect();
    object(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
    object(
        &mut out,
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), images.len()).as_bytes(),
    );
    let (w, h) = (format!("{width:.2}"), format!("{height:.2}"));
    for (i, img) in images.iter().enumerate() {
        let (jpeg, pw, ph) = (&img.data, img.width, img.height);
        let (page, content, image) = (3 + 3 * i, 4 + 3 * i, 5 + 3 * i);
        object(
            &mut out,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] /Contents {content} 0 R /Resources << /XObject << /Im{page} {image} 0 R >> >> >>"
            )
            .as_bytes(),
        );
        let draw = format!("q {w} 0 0 {h} 0 0 cm /Im{page} Do Q");
        object(
            &mut out,
            format!("<< /Length {} >>\nstream\n{draw}\nendstream", draw.len()).as_bytes(),
        );
        let (space, filter) = if img.cmyk {
            ("DeviceCMYK", "FlateDecode")
        } else {
            ("DeviceRGB", "DCTDecode")
        };
        let mut body = format!(
            "<< /Type /XObject /Subtype /Image /Width {pw} /Height {ph} /ColorSpace /{space} /BitsPerComponent 8 /Filter /{filter} /Length {} >>\nstream\n",
            jpeg.len()
        )
        .into_bytes();
        body.extend_from_slice(jpeg);
        body.extend_from_slice(b"\nendstream");
        object(&mut out, &body);
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cross_reference_points_at_each_object() {
        let jpeg = crate::jpeg::encode(&[200; 16 * 8 * 3], 16, 8, 90, &[], &[]).unwrap();
        let page = Image {
            data: jpeg,
            width: 16,
            height: 8,
            cmyk: false,
        };
        let pdf = write(&[page.clone(), page], 595.28, 841.89);
        let text = String::from_utf8_lossy(&pdf);
        let start: usize = text
            .rsplit("startxref\n")
            .next()
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(pdf[start..].starts_with(b"xref\n0 9\n"));
        // Chaque entrée de la table mène au début de son objet.
        // La table est en ASCII : lue dans les octets (le texte approché décale les
        // positions après les JPEG).
        let table = String::from_utf8(pdf[start..].to_vec()).unwrap();
        for (n, line) in table.lines().skip(3).take(8).enumerate() {
            let at: usize = line[..10].parse().unwrap();
            assert!(
                pdf[at..].starts_with(format!("{} 0 obj", n + 1).as_bytes()),
                "objet {}",
                n + 1
            );
        }
        assert!(text.contains("/Count 2"));
        assert!(text.contains("/MediaBox [0 0 595.28 841.89]"));
    }
}
