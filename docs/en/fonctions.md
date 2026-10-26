# Features

What LÜMA can do, and what is coming, area by area: development, formats and colour,
export, the library. Every feature marked "done" is covered by the tests.

## Principles

- **Non-destructive**: an edit is a list of parameters, replayed on every render; the
  original is never modified.
- **Precise sliders**: each with its own name, range, default and direction;
  temperature and tint on the DNG scale ("5500 K, +10").
- **Its own engine**: written in Rust, 32-bit floating point, on the GPU, with its own
  rendering profiles (Color, Neutral, Vivid, Monochrome).
- **The XMP format**: settings read and written as `crs:` XMP, in a file next
  to the RAW or as presets.
- **An identity of its own**: interface, rendering and keyboard shortcuts (`\`, J, Z,
  copy‑paste, undo…) designed for everyday work.

Key: ✅ done and tested · 🟡 partial · ⏳ to do

## Develop module

| Panel | Feature | Status |
|---|---|---|
| Profile | Color, Neutral, Vivid, Monochrome; reads `crs:CameraProfile` | ✅ |
| | 3D `.cube` LUTs (like creative profiles), with adjustable amount, on the GPU too | ✅ |
| | Manufacturer profiles | ⏳ |
| Basic | As Shot white balance, temperature and tint (DNG scale) | ✅ |
| | Auto white balance, presets (Daylight, Cloudy, Shade, Tungsten, Fluorescent, Flash), eyedropper (W) | ✅ |
| | Exposure, Contrast | ✅ |
| | Highlights, Shadows (image-adaptive) | ✅ |
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
| | Advanced noise reduction (non-local means, wavelets), without AI | ⏳ |
| Lens Corrections | Lateral chromatic aberration estimated from the photo, distortion, vignetting (amount, midpoint), purple and green defringe | ✅ |
| | Lens profiles (database), longitudinal aberration | ⏳ |
| Transform | Vertical and horizontal perspective, rotate, aspect, scale (homography, fixed centre) | ✅ |
| | Automatic and guided straightening | ⏳ |
| Effects | Post-crop vignetting (amount, midpoint, roundness, feather, highlights) | ✅ |
| | Grain (amount, size, roughness), identical at every size | ✅ |
| Calibration | Shadows tint, hue and saturation of the primaries | ✅ |
| Tools | Crop and straighten (free angle), on-screen tool with locked aspect ratio (R) | ✅ |
| | Masks: linear gradient (M), radial gradient (Shift+M), brush with eraser (K); invert, amount, show overlay (O); exposure, contrast, highlights, shadows, whites, blacks, temperature, tint, saturation, clarity | ✅ |
| | Luminance and colour range masks, edge-aware brush, combinations; reading `crs:MaskGroupBasedCorrections` masks | ⏳ |
| | Healing, clone, red eye, remove | ⏳ |
| | History, undo and redo, copy‑paste settings | ✅ in LÜMA Studio |
| | Snapshots, sync across multiple photos | ⏳ |
| | Histogram, clipping indicator (J), before/after (`\`), 1:1 zoom | ✅ |
| | Panorama (perspective, cylindrical, spherical), reversible auto crop, DNG | ✅ |
| | Stacks: mean, median, maximum, starry sky | ✅ |
| | HDR merge, panorama boundary warp | ⏳ |
| | 100 presets with amount: colour film stocks, cinema, black & white, instant film; all measured on a test chart in every test run (distinct, no crushed images, identical renders on the GPU) | ✅ |
| | User presets (save, import `.xmp` files), export one at a time or as a folder in XMP format | ✅ |
| | Edited photos marked below the photo and on thumbnails (film, amount, edited panels) | ✅ |
| | Measuring a real film stock: a scan compared with the same scene shot digitally, automatic alignment, 3D LUT | ✅ |
| | Films stacked through masks, each in its own area and at its own amount, including highlights, shadows and clarity | ✅ |

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
| Resizing: long edge, short edge, width × height, megapixels, never enlarging (`--resize`) | ✅ |
| Export dialog: format, quality, bit depth, colour space, size, folder; export every photo in the filter | ✅ |
| Export presets (format, quality, bit depth, colour space, size, folder): web, full resolution, P3 display, print, and your own; `luma export-preset`, `luma develop --preset` | ✅ |
| File name templates: `{nom}` (or `{name}`), `{date}`, `{séquence}` (`{sequence}`), `{boîtier}` (`{camera}`), `{objectif}` (`{lens}`), `{iso}`, `{note}` (`{rating}`), `{libellé}` (`{label}`)…, subfolders with “/”; in the Export dialog, presets and `luma develop --name` | ✅ |
| Output sharpening, watermark, copying EXIF metadata | ⏳ |

## Library and interoperability

| Feature | Status |
|---|---|
| Reads XMP files (`IMG_0001.xmp` sidecar), writes the same format | ✅ automatic saving in the app |
| Folder filmstrip with thumbnails, ← → navigation | ✅ |
| Grid, ratings (0 to 5), flags (P, X, U), filter by rating and flag, stored in the XMP | ✅ |
| Local catalogue (`luma catalog`): folders and subfolders, capture date, camera, lens, ISO read from the header without decoding the image, incremental updates, sorted by date | ✅ |
| Instant thumbnails (the preview embedded in the raw file, in 15 ms), then rendered with the adjustments and saved losslessly for later visits | ✅ |
| Full-resolution working copy: identical to the original, faster to open, two and a half times less memory; “Clear cache” in one click | ✅ |
| Keywords in the XMP (`dc:subject`), read and written | ✅ |
| Search and filters (`luma catalog --search`): words with accents ignored; rating, flag, label, dates, ISO, focal length, camera, lens, keyword; counts of keywords, cameras and lenses | ✅ |
| Manual and smart collections (rule: rating, date, camera, lens, ISO, keywords…), saved in the catalogue | ✅ |
| Catalogue in the app, virtual copies | ⏳ |
| Importing existing catalogues | ⏳ |

## What sets LÜMA apart

- **Nothing is sent anywhere**: no account, no mandatory cloud.
- **100% Rust, verified**: `scripts/audit-rust.sh` checks the dependency graph on
  every build (170 crates, no native bindings and no C compiler).
- **Measured, not promised**: `luma bench` and CI publish actual render times.
