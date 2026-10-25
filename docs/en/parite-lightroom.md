# LÜMA vs Lightroom Classic: feature-by-feature parity

LÜMA follows the **workflow** of Lightroom Classic: the same modules, the same
panels, the same sliders with the same names, ranges and defaults, and it reads
Lightroom's XMP files. Photographers find their bearings and their existing edits.

## What "identical" means, and what it cannot mean

| Carried over as is | Deliberately different |
|---|---|
| Organisation into modules (Library, Develop, Export…) and panels | The look: LÜMA has its own visual identity, not Adobe's |
| Name, range, default and direction of every slider | The rendering engine: Adobe's is secret. LÜMA has its own, written in Rust; a given value has the same meaning and the same magnitude, but does not produce exactly the same pixels |
| Settings exchanged in the XMP `crs:` format (read and write) | Adobe's profiles ("Adobe Color"…), which are protected: LÜMA offers its own equivalents (Color, Neutral, Vivid, Monochrome) |
| Temperature and tint on the DNG scale (the same "5500 K, +10") | Trademarks and logos |
| Keyboard shortcuts (`\`, J, Z, copy‑paste, undo…) | |

Key: ✅ done and tested · 🟡 partial · ⏳ to do

## Develop module

| Panel | Feature | Status |
|---|---|---|
| Profile | Color, Neutral, Vivid, Monochrome; reads `crs:CameraProfile` | ✅ |
| | 3D `.cube` LUTs (like creative profiles), with adjustable amount, on the GPU too | ✅ |
| | Manufacturer profiles ("Camera Standard"…) | ⏳ |
| Basic | As Shot white balance, temperature and tint (DNG scale) | ✅ |
| | Auto white balance, presets (Daylight, Cloudy, Shade, Tungsten, Fluorescent, Flash), eyedropper (W) | ✅ |
| | Exposure, Contrast | ✅ |
| | Highlights, Shadows (image-adaptive, as in Process 2012) | ✅ |
| | Whites, Blacks | 🟡 fixed, not yet image-adaptive |
| | Texture, Clarity, Dehaze | ✅ |
| | Vibrance, Saturation | ✅ |
| Tone Curve | Parametric (4 regions, 3 splits) | ✅ |
| | Point curves: luminance, red, green, blue | ✅ |
| Color Mixer | Hue, saturation and luminance for 8 colours | ✅ |
| | Black & white mix (±3 EV, `crs:GrayMixer*`) | ✅ |
| | Targeted Adjustment Tool | ⏳ |
| Color Grading | Shadows, midtones, highlights, global; blending, balance | ✅ |
| Detail | Sharpening: amount, radius, detail, masking | ✅ |
| | Luminance and colour noise reduction | ✅ |
| | AI Denoise, Super Resolution | ⏳ |
| Lens Corrections | Lateral chromatic aberration estimated from the photo, distortion, vignetting (amount, midpoint), purple and green defringe; Lightroom XMP names | ✅ |
| | Lens profiles (database), longitudinal aberration | ⏳ |
| Transform | Vertical and horizontal perspective, rotate, aspect, scale (homography, fixed centre) | ✅ |
| | Auto and Guided Upright | ⏳ |
| Effects | Post-crop vignetting (amount, midpoint, roundness, feather, highlights) | ✅ |
| | Grain (amount, size, roughness), identical at every size | ✅ |
| Calibration | Shadows tint, hue and saturation of the primaries | ✅ |
| Tools | Crop and straighten (free angle), on-screen tool with locked aspect ratio (R) | ✅ |
| | Masks: linear gradient (M), radial gradient (Shift+M), brush with eraser (K); invert, amount, show overlay (O); exposure, contrast, highlights, shadows, whites, blacks, temperature, tint, saturation, clarity | ✅ |
| | AI masks (subject, sky), luminance or colour range; reading masks from Lightroom XMP files | ⏳ |
| | Healing, clone, red eye, remove | ⏳ |
| | History, undo and redo, copy‑paste settings | ✅ in LÜMA Studio |
| | Snapshots, sync across multiple photos | ⏳ |
| | Histogram, clipping indicator (J), before/after (`\`), 1:1 zoom | ✅ |
| | Panorama (perspective, cylindrical, spherical), reversible auto crop, DNG | ✅ |
| | Stacks: mean, median, maximum, starry sky (beyond Lightroom) | ✅ |
| | HDR merge, panorama boundary warp | ⏳ |
| | 100 presets with amount: colour film stocks, cinema, black & white, instant film; all measured on a test chart in every test run (distinct, no crushed images, identical renders on the GPU) | ✅ |
| | User presets (save, import Lightroom `.xmp` files), export one at a time or as a folder in Lightroom format | ✅ |
| | Edited photos marked below the photo and on thumbnails (film, amount, edited panels) | ✅ |
| | Measuring a real film stock: a scan compared with the same scene shot digitally, automatic alignment, 3D LUT (beyond Lightroom) | ✅ |
| | Films stacked through masks, each in its own area and at its own amount, including highlights, shadows and clarity (beyond Lightroom) | ✅ |

## Formats and colour

| Feature | Status |
|---|---|
| Bayer RAW: Sony ARW, Canon CR3, Nikon NEF (and the other formats read by rawler) | ✅ 3 camera bodies checked continuously |
| X‑Trans RAW (Fujifilm) | ✅ dedicated demosaicing |
| Linear DNG, Apple ProRAW (with `BaselineExposure`) | ✅ |
| Rendered images: JPEG, PNG, TIFF, WebP, GIF, BMP, TGA, QOI, OpenEXR, HDR, PNM, ICO, DDS, JPEG XL, PSD, JPEG 2000, SVG (pure Rust); HEIC, HEIF, AVIF (Apple ImageIO); unknown extensions read by content | ✅ “Original” profile: identical on opening |
| DNG colour: two matrices interpolated by temperature | ✅ |
| 32-bit floating-point pipeline, linear Rec. 2020 working space | ✅ |
| Highlights clipped at the sensor rendered neutral (no pink skies) | ✅ |
| EXIF orientation | ✅ |

## Export

| Feature | Status |
|---|---|
| JPEG (quality), PNG and TIFF, 8 or 16 bits | ✅ |
| sRGB, Display P3, Rec. 2020, ProPhoto, with an embedded ICC profile | ✅ |
| Resizing (long edge) | ✅ |
| Export dialog: format, quality, bit depth, colour space, size, folder; export every photo in the filter | ✅ |
| Output sharpening, watermark, copying EXIF metadata, export presets | ⏳ |

## Library and interoperability

| Feature | Status |
|---|---|
| Reads Lightroom XMP files (`IMG_0001.xmp` sidecar), writes the same format | ✅ automatic saving in the app |
| Folder filmstrip with thumbnails, ← → navigation | ✅ |
| Grid, ratings (0 to 5), flags (P, X, U), filter by rating and flag, stored in the XMP | ✅ |
| Import, catalogue, collections, keywords, search | ⏳ |
| Importing a Lightroom catalogue (`.lrcat`) | ⏳ |

## What is already better, or will be

- **Nothing is sent anywhere**: no account, no mandatory cloud.
- **100% Rust, verified**: `scripts/audit-rust.sh` checks the dependency graph on
  every build (170 crates, no native bindings and no C compiler).
- **Measured, not promised**: `luma bench` and CI publish actual render times.
