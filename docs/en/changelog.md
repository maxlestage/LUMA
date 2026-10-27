# What's new

## 0.3.0 — upcoming

The professional photo library, step by step.

- **Faster**: colour noise reduction (on by default) is three times faster without
  changing a single pixel; full-resolution export takes 25 to 30% less time, and the
  preview with heavy adjustments is about 25% more responsive. Then the blurs: two
  buffers instead of six new images, so full-resolution sharpening is almost twice as
  fast (NEF 0.64 → 0.36 s), without changing a pixel. Then JPEG writing, on every
  core: 2.3 to 2.6 times faster, byte-for-byte identical files; a full-resolution 24 MP
  export in 1.8 s instead of 2.4. Then the tone rendering step, computed eight pixels
  at a time: 1.8 to 1.9 times faster (NEF 160 → 85 ms), byte-for-byte identical files.

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
- **Combined masks**: “Combine” adds, subtracts or intersects another shape (linear
  or radial gradient, brush, each one invertible); a radial with another one
  subtracted becomes a ring, a subtracted brush spares a face under a gradient.
- **Mask area your way**: the area shown (O) can be red, green, blue, white or white
  on black, with an opacity from 10 to 100%.
- **Healing and clone** (Q): one click on a dust spot or a distracting detail, and
  LÜMA picks the part of the photo that replaces it. Healing keeps its texture and
  takes the light from the edge of the area; clone makes an exact copy. Area and
  source move with the mouse; size, feather, opacity. Saved in the XMP.
- **Moiré and defringe with the brush**: two more sliders in every mask. “Moiré”
  softens the false colours of a fabric or a grid, “Defringe” removes coloured fringes
  along edges; brightness does not move.
- **Red eye**: a circle on each eye, and the red pupil turns black again; the skin
  around it and the white catchlight stay as they are.
- **Sensor spots**: “Show spots” makes sensor dust stand out as dark dots; “Remove
  dust” finds it in even areas (a sky, a wall) and puts a healing spot on each one,
  leaving textures, wires and background blur alone.
- **ICC profiles read** (a step towards on-screen soft proofing): display profiles
  (matrix and curves) and printer and paper profiles (lut8, lut16, A→B and B→A
  tables, versions 2 and 4), in both directions and for all four rendering intents,
  in pure Rust; a truncated or broken profile is rejected harmlessly. `luma icc`
  describes a profile: class, space, intents, media white, and the device's white,
  black and primaries.
- **Faster, lighter**: the lens corrections recorded in RAW files (Sony, Fujifilm,
  OM System, Panasonic) no longer copy the image, 17 to 35% faster at full
  resolution (ARW 602 → 408 ms, RW2 523 → 339 ms), with bit-identical pixels;
  working duplicates are 5 to 9% lighter and read up to 17% faster (CR3 331 →
  274 ms), still lossless.
- **On-screen soft proofing** (below the histogram): choose the ICC profile of a
  printer and paper, and the preview shows the photo as it will come out, with the
  perceptual or relative colorimetric intent; the histogram follows. A view, not an
  adjustment: nothing is written to the photo.
- **Simpler iPhone app**: an Undo arrow in the top bar and a “•••” menu for the rest
  (redo, copy, paste, reset, language); a “Before” button that says what it shows;
  each tool named under its icon; in the tools sheet, sliders first.
- **LÜMA Premium on the App Store** (iPhone and iPad): both subscriptions at their App
  Store price, “Subscribe” and “Restore Purchases”; every purchase and renewal unlocks
  Premium. On iPhone and iPad, only the App Store sells the subscription; on Mac, only
  the website (Stripe).
- **Gamut warning** (in soft proofing): the colors the printer will not be able to
  reproduce turn blue on the preview, with or without the print simulation.
- **Paper and black ink** (in soft proofing): “Paper color” shows the paper's tone
  and brightness, “Black ink” the ink's real black, lighter than the screen's;
  without them, the paper white and the ink black are mapped to the screen's white
  and black.
- **Watched folder** (Library): photos arriving in a folder (tethered camera, copied
  card) enter the catalog and the filmstrip as soon as they are complete, and the
  latest one opens (or is only added, your choice); `luma watch folder` does the same
  from the command line.
- **Tethered camera, on Mac** (Watched folder): “Tether a camera” opens the plugged-in
  camera (USB, or network if it allows) through the system framework; each photo
  taken arrives in the watched folder and opens, those already on the card stay
  where they are; “Release shutter” for cameras that can be triggered remotely.
- **Full-screen culling** (F): the photo alone, rated from the keyboard (0 to 5),
  with its flags (P, X, U) and color labels (6 to 9: red, yellow, green, blue;
  purple with the mouse); the next photo comes by itself (A to turn it off), Esc to
  leave. Labels are written to the XMP under the format's names, which other photo
  software reads, and can be searched in French, English or Spanish.
- **Compare** (C): 2 to 4 photos side by side or in a grid (the selected ones,
  otherwise the open photo and the next one), each developed with its own
  adjustments; zoom and pan are shared by all, the loupe (L) magnifies the same spot
  of each one, a click opens the photo.
- **Simpler, leaner iPhone**: the welcome screen no longer redraws twice a second for
  nothing (only during an import from Photos); the top bar shows only what is useful:
  no “After” or export without a photo; soft proofing
  for print no longer clutters iPhone and iPad.
- **Bursts and exposure brackets stacked**: photos from the same camera less than
  two seconds apart fold behind the first one (“Burst ×5”, “Bracket ×3”); S expands
  them, “Merge the bracket” opens the Merge tool with the photos of an exposure
  bracket.

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
