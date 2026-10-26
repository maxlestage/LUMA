# What's new

## 0.3.0 — upcoming

The professional photo library, step by step.

- **Faster**: colour noise reduction (on by default) is three times faster without
  changing a single pixel; full-resolution export takes 25 to 30% less time, and the
  preview with heavy adjustments is about 25% more responsive. Then the blurs: two
  buffers instead of six new images, so full-resolution sharpening is almost twice as
  fast (NEF 0.64 → 0.36 s), without changing a pixel.

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
- **“Camera corrections” checkbox** (Geometry): on by default; turned off, the photo
  goes back to what the lens gave, and the distortion and vignetting sliders take over.
  Saved in the XMP.
- **OM System, Olympus and Panasonic lens corrections**: the distortion (and, for
  Olympus and OM System, the lateral chromatic aberration) that the camera records in
  every ORF or RW2 file is applied automatically. The test corpus now holds 7 real RAW
  files, including an OM‑5 Mark II and a Panasonic G9.
- **Fujifilm lens corrections**: those the camera records in every RAF file
  (distortion, lateral chromatic aberration, vignetting), crop mode included, are
  applied automatically; for example the barrel distortion of the XF 35mm F2.
- **Sony lens corrections**: the distortion, lateral chromatic aberration and
  vignetting that the camera records in every ARW file are applied automatically, with
  just enough enlargement to leave no empty border.
- **DNG lens corrections**: the distortion, lateral chromatic aberration and
  vignetting that the camera or the software wrote into a DNG (`WarpRectilinear` and
  `FixVignetteRadial` opcodes) are applied automatically, in the preview as on export.
- **Faster catalog on external drives**: to file a photo, LÜMA now reads only the
  beginning of the file (0.5 to 2 MB instead of 25 to 56 MB). Five RAW files from the
  corpus: 34 MB read instead of 189; a NEF is filed in 1.5 ms instead of 17 to 23, a
  RAF in 0.6 ms instead of 22 to 25 (cold reads).
- **Virtual copies**: several developments of the same photo without duplicating the
  file; each copy has its own settings, rating, keywords and thumbnail, sits next to
  its original and exports as `IMG_0001-2`. Deleting it never touches the original.
- **The catalog in the app** (Library): every folder you open is added in the
  background; all photos, collections and folders, each with its number of photos,
  and a search (“sea rating:3 date:2024”); these views are sorted by capture date, and
  the filmstrip, the grid and the arrow keys follow them.
- **Keywords in the app** (Library): type them (several at once, separated by
  commas) and they go to every selected photo (⌘‑click, Shift‑click); a cross removes
  one. The folder's keywords, with their count, filter the grid in one click (case and
  accents ignored).
- **Lighter catalogue**: a compact binary format in which every folder, camera, lens
  and keyword is written only once. For 100,000 photos: 7.4 MB instead of 33.6, read
  in 68 ms instead of 166, saved in 121 ms instead of 332. The previous catalogue is
  read and converted automatically.
- **Export presets**: format, quality, bit depth, colour space, size and folder saved
  under a name; four built in (web, full resolution, P3 display, print) plus your own,
  shared by the app and the command line (`luma export-preset`,
  `luma develop --preset Web`).
- **Exported file names**: a template such as `{date}_{sequence}` or
  `{year}/{month}/{camera}_{name}`, with the capture date, camera, lens, ISO, rating,
  label and position in the export; in the Export dialog, export presets and
  `luma develop --name`. Two photos with the same name never overwrite each other
  (`-2`, `-3`…).
- **Export dimensions**: long edge, short edge, width × height (the image fits inside
  the frame) or megapixels, never enlarging; in the Export dialog, presets and
  `luma develop --resize short:1080`.
- **Output sharpening**: for screen, matte paper or glossy paper, low, standard or
  high, applied to the image at its final size (on paper, the radius follows the print
  size); flat areas stay smooth.
- **Text watermark**: “© your name” drawn in the LÜMA typeface, anti-aliased to the
  pixel, in one of nine positions, with size, opacity, white or black; in the Export
  dialog, presets and `luma develop --watermark`.
- **Logo watermark**: your logo as a PNG, with its transparency and true colours, in
  one of nine positions, with size and opacity; alone or with the text; in the Export
  dialog, presets and `luma develop --logo signature.png`.
- **Export metadata**: the capture EXIF (camera, lens, settings, date) copied into
  JPEG and PNG files, your name and copyright, keywords, rating and label; the GPS
  location is removed by default, so a shared photo doesn't say where you live
  (`--keep-gps` keeps it). TIFF files get them too (EXIF, XMP), with exactly the same
  pixels as before.
- **Export to several presets at once**: tick “Web” and “Print” in the Export dialog
  and each photo goes to both in one go; save your settings as a preset;
  `luma export *.ARW --preset Web --preset Impression`.
- **Range masks**: a mask's area (gradient, brush, whole image) can keep only some
  tones (luminance range: minimum, maximum, smoothness) or some colours (colour range:
  hue, width, smoothness; greys are left out). Measured in OKLab on the photo, before
  the masks' adjustments; the area is shown in red (O).
- **Edge-aware brush**: tick “Follow edges” and the brush paints only tones close to
  the centre of each dab; the area stops at the edge of a sky, a wall, a face,
  without spilling over.

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
