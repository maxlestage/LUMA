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
| | Whites, Blacks (adaptive: bring the photo's real extremes to white and black) | ✅ |
| | Texture, Clarity, Dehaze | ✅ |
| | Vibrance, Saturation | ✅ |
| Tone Curve | Parametric (4 regions, 3 splits) | ✅ |
| | Point curves: luminance, red, green, blue | ✅ |
| Color Mixer | Hue, saturation and luminance for 8 colours | ✅ |
| | Black & white mix (±3 EV, `crs:GrayMixer*`) | ✅ |
| | Targeted adjustment tool (T): drag on a colour in the photo to adjust that colour's hue, saturation or luminance | ✅ |
| Color Grading | Shadows, midtones, highlights, global; blending, balance | ✅ |
| | Split toning (shadows and highlights, balance; `crs:SplitToning*` read and written) | ✅ |
| | Channel mixer: each output channel made from the photo's three channels (−200 to +200%) | ✅ |
| Detail | Sharpening: amount, radius, detail, masking | ✅ |
| | Luminance and colour noise reduction | ✅ |
| | Advanced noise reduction (non-local means, wavelets), without AI | ⏳ |
| Lens Corrections | Lateral chromatic aberration estimated from the photo, distortion, vignetting (amount, midpoint), purple and green defringe | ✅ |
| | Lens corrections written in DNG files (`WarpRectilinear` and `FixVignetteRadial` opcodes): distortion, lateral chromatic aberration and vignetting, applied automatically at every size | ✅ |
| | Lens corrections of Sony ARW files (distortion, lateral chromatic aberration, vignetting, recorded by the camera), applied automatically with no empty border | ✅ |
| | Lens corrections of Fujifilm RAF files (distortion, lateral chromatic aberration, vignetting, recorded by the camera, crop mode included), applied automatically | ✅ |
| | Lens corrections of Olympus and OM System ORF files (distortion, lateral chromatic aberration) and Panasonic RW2 files (distortion), recorded by the camera, applied automatically | ✅ |
| | “Camera corrections” checkbox: on by default, saved in the XMP (`crs:LensProfileEnable`); turned off, the photo goes back to what the lens gave, and the distortion and vignetting sliders take over | ✅ |
| | Longitudinal chromatic aberration, corrections of other brands (Canon, Nikon, Pentax…) | ⏳ |
| Transform | Vertical and horizontal perspective, rotate, aspect, scale (homography, fixed centre) | ✅ |
| | Automatic and guided straightening | ⏳ |
| Effects | Post-crop vignetting (amount, midpoint, roundness, feather, highlights) | ✅ |
| | Grain (amount, size, roughness), identical at every size | ✅ |
| Calibration | Shadows tint, hue and saturation of the primaries | ✅ |
| Tools | Crop and straighten (free angle), on-screen tool with locked aspect ratio (R) | ✅ |
| | Crop overlays: thirds, golden ratio, diagonals, golden triangles, golden spiral, grid (O to switch, Shift+O to rotate) | ✅ |
| | Auto straighten (Shift+R): horizon and verticals read with a Hough transform; nothing when in doubt (close-up, vanishing lines) | ✅ |
| | Masks: linear gradient (M), radial gradient (Shift+M), brush with eraser (K); invert, amount, show overlay (O); exposure, contrast, highlights, shadows, whites, blacks, temperature, tint, saturation, clarity | ✅ |
| | Masks restricted to a luminance range (minimum, maximum, smoothness) or a colour range (hue, width, smoothness; greys left out), read on the photo; area shown (O) | ✅ |
| | Edge-aware brush: each dab paints only tones close to its centre, the area stops at edges in the photo | ✅ |
| | Mask combinations: add, subtract, intersect other shapes (each one invertible) | ✅ |
| | Mask area shown (O) in red, green, blue, white or white on black, opacity from 10 to 100% | ✅ |
| | Reading `crs:MaskGroupBasedCorrections` masks | ⏳ |
| | Healing and clone (Q): one click, the source found on its own, area and source movable; size, feather, opacity | ✅ |
| | Sensor spots: spot view (dust as dark dots), “Remove dust” in one click | ✅ |
| | Red eye: a circle on the eye, the red pupil turns black again, the skin stays | ✅ |
| | Moiré and defringe (local chromatic aberration) with the brush, in every mask | ✅ |
| | Remove | ⏳ |
| | History, undo and redo, copy‑paste settings | ✅ in LÜMA Studio |
| | Selective copy and paste of adjustments (“Paste…”, Alt+Shift+Cmd+V): fourteen groups to choose from, crop and masks left to each photo by default | ✅ |
| | Snapshots, sync across multiple photos | ⏳ |
| | iPhone and iPad: smaller preview during a gesture (12 times faster), memory watched, caches freed before the system closes the app | ✅ |
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
| Bayer RAW: Sony ARW, Canon CR3, Nikon NEF (and the other common RAW formats) | ✅ 3 camera bodies checked continuously |
| X‑Trans RAW (Fujifilm) | ✅ dedicated demosaicing |
| Linear DNG, Apple ProRAW (with `BaselineExposure`) | ✅ |
| Rendered images: JPEG, PNG, TIFF, WebP, GIF, BMP, TGA, QOI, OpenEXR, HDR, PNM, ICO, DDS, JPEG XL, PSD, JPEG 2000, SVG; HEIC, HEIF, AVIF (Apple ImageIO); unknown extensions read by content | ✅ “Original” profile: identical on opening |
| DNG colour: two matrices interpolated by temperature | ✅ |
| 32-bit floating-point pipeline, linear Rec. 2020 working space | ✅ |
| Highlights clipped at the sensor rendered neutral (no pink skies) | ✅ |
| EXIF orientation | ✅ |
| ICC profiles read (versions 2 and 4): matrix and curves, lut8, lut16, A→B and B→A tables of printers and papers, four rendering intents (`luma icc`) | ✅ |
| On-screen soft proofing: the preview as it will print with a printer profile, perceptual or relative colorimetric intent (histogram included) | ✅ |
| Gamut warning: the colors the printer will not be able to reproduce, shown in blue on the preview (relative colorimetric round trip, beyond ΔE 3) | ✅ |
| Paper simulation (tone and brightness, absolute colorimetric) and black ink simulation (its real black); without them, black point compensation | ✅ |

## Export

| Feature | Status |
|---|---|
| JPEG (quality), PNG and TIFF, 8 or 16 bits | ✅ |
| sRGB, Display P3, Rec. 2020, ProPhoto, with an embedded ICC profile | ✅ |
| Resizing: long edge, short edge, width × height, megapixels, never enlarging (`--resize`) | ✅ |
| Export dialog: format, quality, bit depth, colour space, size, folder; export every photo in the filter | ✅ |
| Export to several presets at once (Export dialog: checkboxes, use, save; `luma export --preset A --preset B`) | ✅ |
| Export presets (format, quality, bit depth, colour space, size, folder): web, full resolution, P3 display, print, and your own; `luma export-preset`, `luma develop --preset` | ✅ |
| File name templates: `{nom}` (or `{name}`), `{date}`, `{séquence}` (`{sequence}`), `{boîtier}` (`{camera}`), `{objectif}` (`{lens}`), `{iso}`, `{note}` (`{rating}`), `{libellé}` (`{label}`)…, subfolders with “/”; in the Export dialog, presets and `luma develop --name` | ✅ |
| Output sharpening: screen, matte paper, glossy paper; low, standard, high; at the final size (`--output-sharpen`) | ✅ |
| Text watermark (LÜMA font, anti-aliased): nine positions, size, opacity, white or black (`--watermark`) | ✅ |
| Logo watermark: a PNG with its transparency, faithful colours, nine positions, size, opacity (`--logo`) | ✅ |
| Export metadata: capture EXIF copied (camera, lens, settings, date), author and copyright, keywords, rating and label as XMP, GPS location removed by default (`--author`, `--keep-gps`) | ✅ |
| Metadata in TIFF files too: EXIF, ICC profile and XMP, TIFF written by LÜMA (same pixels, same size) | ✅ |

## Library and interoperability

| Feature | Status |
|---|---|
| Reads XMP files (`IMG_0001.xmp` sidecar), writes the same format | ✅ automatic saving in the app |
| Folder filmstrip with thumbnails, ← → navigation | ✅ |
| Grid, ratings (0 to 5), flags (P, X, U), filter by rating and flag, stored in the XMP | ✅ |
| Full-screen culling from the keyboard (F): ratings, flags, color labels (6 to 9), next photo by itself (A), Esc to leave; labels stored under the format's names in the XMP, found in French, English and Spanish | ✅ |
| Compare 2 to 4 photos (C): side by side then in a grid, each developed with its own adjustments; synchronized zoom and pan (scroll, double-click, drag), ×4 loupe at the same spot of every photo (L), a click opens the photo | ✅ |
| Stacks: bursts and exposure brackets recognized (same camera, less than two seconds apart; a changing exposure makes a bracket), folded behind the first photo (S to expand), “Merge the bracket” to the Merge tool | ✅ |
| Duplicates by image fingerprint (the same photo copied, exported at another size or recompressed), grouped side by side; ratings, flags and labels in batch on the selected photos | ✅ |
| Local catalogue (`luma catalog`): folders and subfolders, capture date, camera, lens, ISO read from the header without decoding the image, incremental updates, sorted by date | ✅ |
| Instant thumbnails (the preview embedded in the raw file, in 15 ms), then rendered with the adjustments and saved losslessly for later visits | ✅ |
| Full-resolution working copy: identical to the original, faster to open, two and a half times less memory; “Clear cache” in one click | ✅ |
| Keywords in the XMP (`dc:subject`), read and written | ✅ |
| Search and filters (`luma catalog --search`): words with accents ignored; rating, flag, label, dates, ISO, focal length, camera, lens, keyword; counts of keywords, cameras and lenses | ✅ |
| Manual and smart collections (rule: rating, date, camera, lens, ISO, keywords…), saved in the catalogue | ✅ |
| Keywords in the app: entry (several at once, separated by commas), added to and removed from every selected photo, the folder's keywords with their count, grid filter | ✅ |
| Catalog in the app: every folder you open is added (in the background); all photos, collections and folders, each with its number of photos; search and filters; views sorted by capture date | ✅ |
| Watched folder (tethered shooting, copied card): each complete photo enters the catalog and the filmstrip, and opens; `luma watch` | ✅ |
| On Mac, tethered camera (USB, or network if it allows) through the system framework: each photo taken arrives in the watched folder, shutter release from LÜMA for cameras that allow it | ✅ |
| Virtual copies: several developments of a photo without duplicating the file (one XMP per copy), each with its own rating, keywords and thumbnail, next to its original; exported as `IMG_0001-2`; deleted without touching the original | ✅ |
| Importing existing catalogues | ⏳ |

## What sets LÜMA apart

- **Nothing is sent anywhere**: no account, no mandatory cloud.
- **100% Rust, verified**: `scripts/audit-rust.sh` checks the dependency graph on
  every build (170 crates, no native bindings and no C compiler).
- **Measured, not promised**: `luma bench` and CI publish actual render times.
