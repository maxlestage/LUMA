# What's new

## 0.3.0 — upcoming

The professional photo library, step by step.

- **Faster**: colour noise reduction (on by default) is three times faster without
  changing a single pixel; full-resolution export takes 25 to 30% less time, and the
  preview with heavy adjustments is about 25% more responsive.

- **Local catalogue**: `luma catalog` adds folders (subfolders included) to the LÜMA
  library; capture date, camera, lens, ISO, aperture, shutter speed and focal length
  are read from the RAW header without decoding the image (5 RAW files in 94 ms), and
  afterwards only modified files are read again (under a millisecond). Photos are
  sorted by capture date.
- **Keywords**: read and written in the XMP (`dc:subject`), as every photo application
  does.
- **Search and filters**: `luma catalog --search "sea rating:3 date:2024-06 iso:..800
  camera:sony"`; words (file name, camera, lens, label, keywords, accents ignored) and
  filters (rating, flag, label, dates, ISO, focal length, camera, lens, keyword).
  100,000 photos searched in 4 to 25 ms. `--keywords` counts keywords, cameras and
  lenses.
- **Collections**, manual (hand‑picked photos from any folder) and smart (a rule
  written like a search, “rating:4 date:2024 camera:sony”, that follows the
  catalogue): `luma catalog --smart`, `--add`, `--collections`, `--collection`.
- **Export presets**: format, quality, bit depth, colour space, size and folder saved
  under a name; four built in (web, full resolution, P3 display, print) plus your own,
  shared by the app and the command line (`luma export-preset`,
  `luma develop --preset Web`).
- **Exported file names**: a template such as `{date}_{sequence}` or
  `{year}/{month}/{camera}_{name}`, with the capture date, camera, lens, ISO, rating,
  label and position in the export; in the Export dialog, export presets and
  `luma develop --name`. Two photos with the same name never overwrite each other
  (`-2`, `-3`…).

## 0.2.0 — October 2026

Every image format, three languages, and the store.

- **Every image format**: JPEG, PNG, TIFF, WebP, GIF, BMP, OpenEXR, HDR, JPEG XL, PSD,
  JPEG 2000, SVG… and iPhone HEIC, HEIF, AVIF (Mac, iPhone and iPad); an “Original”
  profile opens the image exactly as it is, and every tool applies to it.
- **Three languages**: French, English, Spanish (app and website).
- **LÜMA Premium through Stripe**: the licence key appears right after payment,
  checked by the app offline, renewed every day.
- **Mac app ready for Developer ID signing** and Apple notarisation.

## 0.1.0 — October 2026

The first version available for download: a 14-day trial, then LÜMA Premium.

- **RAW development**: a complete Develop module, panel by
  panel, 32-bit floating-point engine, finishing on the GPU (Metal), settings saved as XMP.
- **735 camera bodies** supported, 714 of them with calibrated colour; any DNG, including iPhone
  ProRAW.
- **100 films** verified by machine (colour film stocks, cinema, black & white,
  instant film); **your own film stocks measured** from a scan, as a 3D LUT; import of
  `.cube` LUTs.
- **Linear, radial and brush masks**; **films stacked** through masks, fully featured.
- **Stacking** (noise reduction, median, light trails, starry sky) and **panoramas**, as DNG.
- **Presets**: your own, imported or exported as XMP
  presets.
- **Mac** (Apple Silicon and Intel), **iPhone and iPad** (being prepared for the App
  Store), `luma` command-line tool.
