//! Le décodeur d'images d'Apple (ImageIO, celui de Photos et d'Aperçu) pour LÜMA : HEIC
//! et HEIF de l'iPhone, AVIF, et tout ce que le système lit, sur Mac, iPhone et iPad.
//!
//! C'est une liaison vers le système, comme celles de l'app (voir
//! `scripts/audit-rust.sh`) : rien n'est compilé en C. Le moteur reste en Rust pur ; il
//! reçoit ce décodeur par [`luma_raw::raster::set_system_decoder`] (voir [`install`]).
//!
//! L'image est dessinée par CoreGraphics dans un tampon RGB flottant en Display P3
//! linéaire étendu : aucune perte (10 bits et HDR compris), la couleur convertie par le
//! système depuis le profil du fichier, la transparence posée sur du blanc.
//!
//! Ce crate est le seul endroit de LÜMA où du code `unsafe` est permis : les appels à
//! ImageIO et à CoreGraphics, chacun justifié.

pub mod photos;
pub mod store;
pub mod tether;

/// Installe le décodeur du système dans le moteur (sans effet hors d'Apple).
pub fn install() {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    luma_raw::raster::set_system_decoder(apple::decode);
}

/// Les langues préférées de l'iPhone ou de l'iPad, en liste : `fr-FR,en-US`. `None`
/// ailleurs (le Mac a `defaults`, Linux ses variables d'environnement).
pub fn preferred_languages() -> Option<String> {
    #[cfg(target_os = "ios")]
    {
        let list: Vec<String> = objc2_foundation::NSLocale::preferredLanguages()
            .iter()
            .map(|l| l.to_string())
            .collect();
        (!list.is_empty()).then(|| list.join(","))
    }
    #[cfg(not(target_os = "ios"))]
    None
}

/// La mémoire que le système laisse encore à l'app, en octets : sur iPhone et iPad,
/// au‑delà, il la ferme sans prévenir. `None` ailleurs (pas de telle limite).
pub fn available_memory() -> Option<u64> {
    #[cfg(target_os = "ios")]
    {
        unsafe extern "C" {
            // libsystem (iOS 13 et suivants) : sans argument, sans effet de bord.
            fn os_proc_available_memory() -> usize;
        }
        // SAFETY: fonction du système sans argument ni état partagé, disponible depuis
        // iOS 13 (l'app vise iOS 16) ; elle rend un simple nombre d'octets.
        let bytes = unsafe { os_proc_available_memory() };
        (bytes > 0).then_some(bytes as u64)
    }
    #[cfg(not(target_os = "ios"))]
    None
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple {
    use std::path::Path;

    use luma_raw::Orientation;
    use luma_raw::raster::{Decoded, Space};
    use objc2_core_foundation::{CFData, CFNumber, CFRetained, CFString, CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{
        CGBitmapContextCreate, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo,
        CGImageComponentInfo, kCGColorSpaceExtendedLinearDisplayP3,
    };
    use objc2_image_io::{CGImageSource, kCGImagePropertyOrientation};

    /// Le plus grand côté accepté (au‑delà, la mémoire ne suivrait pas : 65 536² × 16 o).
    const MAX_SIDE: usize = 65_536;

    fn orientation(exif: i32) -> Orientation {
        match exif {
            2 => Orientation::FlipHorizontal,
            3 => Orientation::Rotate180,
            4 => Orientation::FlipVertical,
            5 => Orientation::Transpose,
            6 => Orientation::Rotate90,
            7 => Orientation::Transverse,
            8 => Orientation::Rotate270,
            _ => Orientation::Normal,
        }
    }

    /// L'orientation EXIF de l'image `index`, lue dans ses propriétés.
    fn read_orientation(source: &CGImageSource, index: usize) -> Orientation {
        // SAFETY: sans options ; la fonction rend un dictionnaire possédé ou rien.
        let Some(props) = (unsafe { source.properties_at_index(index, None) }) else {
            return Orientation::Normal;
        };
        // SAFETY: la clé est une constante CFString d'ImageIO, toujours définie.
        let key: &CFString = unsafe { kCGImagePropertyOrientation };
        // SAFETY: la clé est un CFString valide ; la valeur rendue est empruntée au
        // dictionnaire, vivant jusqu'à la fin de la fonction.
        let value = unsafe { props.value((key as *const CFString).cast()) };
        if value.is_null() {
            return Orientation::Normal;
        }
        // SAFETY: ImageIO documente cette propriété comme un CFNumber.
        let number: &CFNumber = unsafe { &*value.cast::<CFNumber>() };
        number.as_i32().map_or(Orientation::Normal, orientation)
    }

    pub fn decode(path: &Path) -> Option<Result<Decoded, String>> {
        let bytes = std::fs::read(path).ok()?;
        let data = CFData::from_bytes(&bytes);
        // SAFETY: sans options ; `data` vit jusqu'à la fin de la fonction.
        let source: CFRetained<CGImageSource> = unsafe { CGImageSource::with_data(&data, None) }?;
        // SAFETY: la source est valide.
        let index = unsafe { source.primary_image_index() };
        // SAFETY: sans options ; rend une image possédée, ou rien si le système ne
        // sait pas lire ce fichier.
        let image: CFRetained<CGImage> = unsafe { source.image_at_index(index, None) }?;
        let turn = read_orientation(&source, index);
        Some(draw(&image).map(|(width, height, data)| Decoded {
            width,
            height,
            data,
            space: Space::LINEAR_DISPLAY_P3,
            orientation: turn,
        }))
    }

    /// Dessine l'image dans un tampon RGBX flottant, Display P3 linéaire étendu, sur
    /// du blanc ; rend le RGB.
    fn draw(image: &CGImage) -> Result<(usize, usize, Vec<f32>), String> {
        let (w, h) = (CGImage::width(Some(image)), CGImage::height(Some(image)));
        if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
            return Err(format!("taille d'image impossible : {w} × {h}"));
        }
        // SAFETY: la constante de nom d'espace est toujours définie par CoreGraphics.
        let name: &CFString = unsafe { kCGColorSpaceExtendedLinearDisplayP3 };
        let space = CGColorSpace::with_name(Some(name)).ok_or("espace Display P3 linéaire indisponible")?;
        let mut pixels = vec![0.0f32; w * h * 4];
        let info =
            CGImageAlphaInfo::NoneSkipLast.0 | CGImageComponentInfo::Float.0 | CGImageByteOrderInfo::Order32Little.0;
        // SAFETY: `pixels` fait exactement `h` lignes de `w * 16` octets, et survit au
        // contexte (libéré à la fin du bloc, avant toute lecture de `pixels`).
        let drawn = unsafe {
            let ctx: Option<CFRetained<CGContext>> =
                CGBitmapContextCreate(pixels.as_mut_ptr().cast(), w, h, 32, w * 16, Some(&space), info);
            match ctx {
                Some(ctx) => {
                    let rect = CGRect {
                        origin: CGPoint { x: 0.0, y: 0.0 },
                        size: CGSize {
                            width: w as f64,
                            height: h as f64,
                        },
                    };
                    CGContext::set_rgb_fill_color(Some(&ctx), 1.0, 1.0, 1.0, 1.0);
                    CGContext::fill_rect(Some(&ctx), rect);
                    CGContext::draw_image(Some(&ctx), rect, Some(image));
                    CGContext::flush(Some(&ctx));
                    true
                }
                None => false,
            }
        };
        if !drawn {
            return Err("CoreGraphics n'a pas pu dessiner l'image".into());
        }
        let rgb = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        Ok((w, h, rgb))
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    /// Une vraie photo HEIC (fabriquée par `sips`, l'outil du système) s'ouvre, dans le
    /// bon sens et dans les bonnes couleurs.
    #[test]
    fn a_heic_opens_through_imageio() {
        super::install();
        let dir = std::env::temp_dir().join(format!("luma-apple-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        let heic = dir.join("a.heic");
        // 64 × 32 : moitié gauche rouge, moitié droite blanche.
        let img = image_rgb(64, 32);
        std::fs::write(&png, img).unwrap();
        let ok = std::process::Command::new("sips")
            .args(["-s", "format", "heic"])
            .arg(&png)
            .arg("--out")
            .arg(&heic)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            eprintln!("sips ne sait pas écrire de HEIC ici : test ignoré");
            return;
        }
        let photo = luma_raw::RawPhoto::open(&heic).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(photo.rendered);
        assert_eq!((photo.width, photo.height), (64, 32));
        let px = |x: usize, y: usize| {
            let o = (y * photo.width + x) * 3;
            [photo.data[o], photo.data[o + 1], photo.data[o + 2]]
        };
        let m = photo.camera_to_working(photo.as_shot_neutral);
        let white = luma_color::apply(&m, px(48, 16));
        assert!(white.iter().all(|v| (v - 1.0).abs() < 0.05), "blanc : {white:?}");
        let red = px(10, 16);
        assert!(
            red[0] > 0.5 && red[1] < 0.15 && red[2] < 0.15,
            "rouge (P3 linéaire) : {red:?}"
        );
    }

    /// Un PNG 64 × 32, fabriqué à la main (sans dépendance) : rouge à gauche, blanc à
    /// droite.
    fn image_rgb(w: u32, h: u32) -> Vec<u8> {
        // Une ligne : l'octet de filtre (0), puis les pixels.
        let mut row = vec![0u8];
        for x in 0..w {
            row.extend(if x < w / 2 { [255, 0, 0] } else { [255, 255, 255] });
        }
        let raw = row.repeat(h as usize);
        png_encode(w, h, &raw)
    }

    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }

    fn adler(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &d in data {
            a = (a + u32::from(d)) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn png_encode(w: u32, h: u32, raw: &[u8]) -> Vec<u8> {
        let chunk = |out: &mut Vec<u8>, kind: &[u8], data: &[u8]| {
            out.extend((data.len() as u32).to_be_bytes());
            let mut body = kind.to_vec();
            body.extend(data);
            out.extend(&body);
            out.extend(crc(&body).to_be_bytes());
        };
        // zlib « stocké » (sans compression), par blocs de 65 535 octets.
        let mut z = vec![0x78, 0x01];
        let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
        for (i, b) in blocks.iter().enumerate() {
            z.push(u8::from(i + 1 == blocks.len()));
            let n = b.len() as u16;
            z.extend(n.to_le_bytes());
            z.extend((!n).to_le_bytes());
            z.extend(*b);
        }
        z.extend(adler(raw).to_be_bytes());
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend(w.to_be_bytes());
        ihdr.extend(h.to_be_bytes());
        ihdr.extend([8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}
