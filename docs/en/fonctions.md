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
| | Manufacturer profiles measured on the camera itself: “Measure the camera's look” compares the open RAW with the JPEG the camera stored in it (its picture style at capture) and turns it into a reusable film named after the camera; with the other selected RAW files from the same camera (up to twelve), more colours are measured; `luma film-lut --camera` on one or more RAW files | ✅ |
| Basic | As Shot white balance, temperature and tint (DNG scale) | ✅ |
| | Auto white balance, presets (Daylight, Cloudy, Shade, Tungsten, Fluorescent, Flash), eyedropper (W) | ✅ |
| | Auto tone (“Auto tone”, `--auto-tone`): exposure, contrast, highlights and shadows read from the photo's histogram, no AI | ✅ |
| | Exposure, Contrast | ✅ |
| | Highlights, Shadows (image-adaptive) | ✅ |
| | Whites, Blacks (adaptive: bring the photo's real extremes to white and black) | ✅ |
| | Texture, Clarity, Dehaze | ✅ |
| | Vibrance, Saturation | ✅ |
| Tone Curve | Parametric (4 regions, 3 splits) | ✅ |
| | Point curves: luminance, red, green, blue | ✅ |
| | Point curve editor in the app (Light, Tone curve): a click adds a point, drag it, a double-click removes it; the parametric curve shown faintly underneath; undoable | ✅ |
| Color Mixer | Hue, saturation and luminance for 8 colours | ✅ |
| | Black & white mix (±3 EV, `crs:GrayMixer*`) | ✅ |
| | Targeted adjustment tool (T): drag on a colour in the photo to adjust that colour's hue, saturation or luminance | ✅ |
| Color Grading | Shadows, midtones, highlights, global; blending, balance | ✅ |
| | Split toning (shadows and highlights, balance; `crs:SplitToning*` read and written) | ✅ |
| | Channel mixer: each output channel made from the photo's three channels (−200 to +200%) | ✅ |
| | Camera profile measured on a ColorChecker chart (“Profile from a chart”, `luma colorchecker -o profile.xmp`), as a preset | ✅ |
| | Colors of two cameras matched from the same scene (“Match to the selected photo”, `luma match-colors`), as a preset | ✅ |
| Detail | Sharpening: amount, radius, detail, masking | ✅ |
| | Deconvolution sharpening (“Deconvolution” box, Richardson-Lucy), without AI | ✅ |
| | Luminance and colour noise reduction | ✅ |
| | Advanced noise reduction (wavelets, “Wavelets” box), without AI | ✅ |
| Lens Corrections | Lateral chromatic aberration estimated from the photo, distortion, vignetting (amount, midpoint), purple and green defringe | ✅ |
| | Lens corrections written in DNG files (`WarpRectilinear` and `FixVignetteRadial` opcodes): distortion, lateral chromatic aberration and vignetting, applied automatically at every size | ✅ |
| | Lens corrections of Sony ARW files (distortion, lateral chromatic aberration, vignetting, recorded by the camera), applied automatically with no empty border | ✅ |
| | Lens corrections of Fujifilm RAF files (distortion, lateral chromatic aberration, vignetting, recorded by the camera, crop mode included), applied automatically | ✅ |
| | Lens corrections of Olympus and OM System ORF files (distortion, lateral chromatic aberration) and Panasonic RW2 files (distortion), recorded by the camera, applied automatically | ✅ |
| | “Camera corrections” checkbox: on by default, saved in the XMP (`crs:LensProfileEnable`); turned off, the photo goes back to what the lens gave, and the distortion and vignetting sliders take over | ✅ |
| | Lens distortion and vignetting for other brands (Canon, Nikon, Pentax…) measured on the JPEG the camera stored in the RAW, already corrected (“Corrections from the camera's JPEG”): the Distortion and Vignetting sliders are set, empty borders removed; on all selected photos at once, in the background, each its own; `luma lens` (`--write` to the XMP) | ✅ |
| | Longitudinal chromatic aberration: “Auto defringe” looks for purple and green fringes along edges (more colourful than their surroundings) and sets the smallest defringe that removes them; a real object colour at the edge (leaves against the sky) is left alone | ✅ |
| Transform | Vertical and horizontal perspective, rotate, aspect, scale (homography, fixed centre) | ✅ |
| | Automatic (Shift+R, Shift+V) and guided straightening: two to four lines drawn on the photo set the perspective and rotation | ✅ |
| | Empty edges cropped: the largest centered frame, in the photo's proportions, with no empty edge after Transform or Distortion | ✅ |
| Effects | Post-crop vignetting (amount, midpoint, roundness, feather, highlights) | ✅ |
| | Grain (amount, size, roughness), identical at every size | ✅ |
| Calibration | Shadows tint, hue and saturation of the primaries | ✅ |
| Tools | Crop and straighten (free angle), on-screen tool with locked aspect ratio (R) | ✅ |
| | Crop overlays: thirds, golden ratio, diagonals, golden triangles, golden spiral, grid (O to switch, Shift+O to rotate); the same on the photo outside cropping (H: none, thirds… grid) | ✅ |
| | Auto straighten (Shift+R): horizon and verticals read with a Hough transform; nothing when in doubt (close-up, vanishing lines) | ✅ |
| | Auto verticals (Shift+V): the vanishing point of the verticals measured, the Transform's Vertical set to make them parallel | ✅ |
| | Masks: linear gradient (M), radial gradient (Shift+M), brush with eraser (K); invert, amount, show overlay (O); exposure, contrast, highlights, shadows, whites, blacks, temperature, tint, saturation, clarity, texture, sharpness (raised or softened), noise reduction, dehaze | ✅ |
| | Masks restricted to a luminance range (minimum, maximum, smoothness) or a colour range (hue, width, smoothness; greys left out), read on the photo; area shown (O) | ✅ |
| | “Sky” mask (“Select sky”): sky and clouds found on the photo, edges followed, reflections in water left out; no AI | ✅ |
| | Edge-aware brush: each dab paints only tones close to its centre, the area stops at edges in the photo | ✅ |
| | Mask combinations: add, subtract, intersect other shapes (each one invertible) | ✅ |
| | Duplicate a mask, or duplicate it inverted (settings at zero): the ground after the sky, without redrawing the area | ✅ |
| | Mask area shown (O) in red, green, blue, white or white on black, opacity from 10 to 100% | ✅ |
| | Reading the masks of an XMP from another application (`crs:MaskGroupBasedCorrections`, and the older gradient lists): linear and radial gradients, amount, inversion, local settings, brought into the crop | ✅ |
| | Brush, subject and range masks of an XMP from another application | ⏳ |
| | Healing and clone (Q): one click, the source found on its own, area and source movable; size, feather, opacity | ✅ |
| | Sensor spots: spot view (dust as dark dots), “Remove dust” in one click | ✅ |
| | Red eye: a circle on the eye, the red pupil turns black again, the skin stays | ✅ |
| | Moiré and defringe (local chromatic aberration) with the brush, in every mask | ✅ |
| | Remove: a circle over what is in the way (wire, passer-by, litter), filled with pieces of the surrounding photo, texture included, without AI | ✅ |
| | Traced removal: a stroke along a wire, a scratch or a cable, erased in one gesture; no AI | ✅ |
| | Traced healing and clone: a stroke along a wrinkle or a scratch, the source at the same offset all along | ✅ |
| | Liquify (Retouch): push, bloat, pucker with a brush (size, strength), strokes kept in the XMP, undoable; no AI | ✅ |
| | History, undo and redo, copy‑paste settings | ✅ in LÜMA Studio |
| | Selective copy and paste of adjustments (“Paste…”, Alt+Shift+Cmd+V): fourteen groups to choose from, crop and masks left to each photo by default | ✅ |
| | Sync a selection (Shift+Cmd+S): the open photo's adjustments to the other selected photos, group by group; auto white balance in batch, each photo its own | ✅ |
| | Default adjustments per camera and from an ISO (the most precise wins), starting point for photos without XMP; import film given to arriving photos (watched folder, tethered camera) | ✅ |
| | Named snapshots, stored in the XMP, restored in one click (a history step); sync across multiple photos | ✅ |
| | iPhone and iPad: smaller preview during a gesture (12 times faster), memory watched, caches freed before the system closes the app | ✅ |
| | Histogram, clipping indicator (J), before/after (`\`), before/after side by side (Y, “Side by side”: before on the left of a divider you drag, or on top with Alt+Y, with the same geometry; or a snapshot, “Compare”; “Export before / after” as one image, or for a whole selection, `luma develop --before-after`), a whole reference photo next to the photo being edited (right-click a thumbnail, “Use as reference”, Shift+Y) and “Match” to it (its colour distribution, as an “Accord …” film; `luma film-lut --reference`), “Export the pair” as one image, 1:1 zoom centred on the clicked point, with a navigator (the photo in small, the frame of the view, a click goes there) | ✅ |
| | Waveform, RGB parade, vectorscope (Shift+J) | ✅ |
| | Point measurement (RGB, Lab), per‑channel clipping | ✅ |
| | Panorama (perspective, cylindrical, spherical), reversible auto crop, DNG | ✅ |
| | Stacks: mean, median, maximum, starry sky | ✅ |
| | HDR merge: bracketed shots aligned, ghost-free, as DNG (`luma merge --stack hdr`) | ✅ |
| | Focus stacking: each shot aligned to its neighbor, the sharpest part of each (`luma merge --stack focus`) | ✅ |
| | Panorama boundary warp: ragged edges stretched to fill the rectangle instead of being cropped (“Warp the edges”, `luma merge --warp`) | ✅ |
| | 275 presets with amount: colour film stocks, cinema, black & white, instant film; all measured on a test chart in every test run (distinct, no crushed images, identical renders on the GPU) | ✅ |
| | User presets (save, import `.xmp` files), export one at a time or as a folder in XMP format | ✅ |
| | Edited photos marked below the photo and on thumbnails (film, amount, edited panels) | ✅ |
| | Measuring a real film stock: a scan compared with the same scene shot digitally, automatic alignment, 3D LUT | ✅ |
| | Films stacked through masks, each in its own area and at its own amount, including highlights, shadows and clarity | ✅ |

## Formats and colour

| Feature | Status |
|---|---|
| Bayer RAW: Sony ARW, Canon CR3, Nikon NEF (and the other common RAW formats) | ✅ 3 camera bodies checked continuously |
| X‑Trans RAW (Fujifilm) | ✅ dedicated demosaicing |
| Directional Bayer demosaicing: green follows edges (no zipper or beading on fine detail) | ✅ |
| Directional X‑Trans demosaicing: green follows edges, noisy shadows included | ✅ |
| Linear DNG, Apple ProRAW (with `BaselineExposure`) | ✅ |
| Rendered images: JPEG, PNG, TIFF, WebP, GIF, BMP, TGA, QOI, OpenEXR, HDR, PNM, ICO, DDS, JPEG XL, PSD, JPEG 2000, SVG; HEIC, HEIF, AVIF (Apple ImageIO); unknown extensions read by content | ✅ “Original” profile: identical on opening |
| DNG colour: two matrices interpolated by temperature | ✅ |
| 32-bit floating-point pipeline, linear Rec. 2020 working space | ✅ |
| Highlights clipped at the sensor rendered neutral (no pink skies) | ✅ |
| Highlight reconstruction: a clipped channel takes the color of its neighbors (“Reconstruct highlights” box, `--set highlights.reconstruction=on`) | ✅ |
| EXIF orientation | ✅ |
| ICC profiles read (versions 2 and 4): matrix and curves, lut8, lut16, A→B and B→A tables of printers and papers, four rendering intents (`luma icc`) | ✅ |
| On-screen soft proofing: the preview as it will print with a printer profile, perceptual or relative colorimetric intent (histogram included) | ✅ |
| Gamut warning: the colors the printer will not be able to reproduce, shown in blue on the preview (relative colorimetric round trip, beyond ΔE 3) | ✅ |
| Paper simulation (tone and brightness, absolute colorimetric) and black ink simulation (its real black); without them, black point compensation | ✅ |
| Printing (“Print…”, `luma print`): a PDF at the paper's exact size (A4, A3, A3+, A5, US Letter, 10 × 15 to 20 × 30 cm, or custom), portrait or landscape, margins, one photo per page or a contact sheet (grid, spacing, name under each photo), photos whole or filling their cell, resolution, output sharpening for matte or glossy paper; pages converted to the printer's RGB or CMYK profile (the soft-proofing one, black point compensated; in CMYK, a CMYK PDF, its inks lossless); crop marks at the corners of the photos, a line of text at the bottom of the page (`--crop-marks`, `--text`); the PDF opened in one click in the system's reader, to print (“Open …”, Mac, Linux, Windows), or sent as it is to a system printer, with the number of copies (“Send to printer”, Mac and Linux); or the pages as JPEG, one image per page, for a print service (`-o sheet.jpg`, photo book included) | ✅ |
| Photo book as a PDF (“Book, slideshow, gallery…”, `luma book`): cover (first photo, title, subtitle), pages of one, two or four photos according to their orientation, 20 × 20, 30 × 30, A4, 21 × 28, A3 formats, a text page after the cover and one at the end (`--opening`, `--closing`), the number of photos per page of your choice (“1,2,4”, `--pages`), the pages shown small and the order of the photos changed by dragging one onto another (swap) or onto the background of a page (move) and the number of photos on a page chosen (“Book pages…”, `--layout`); full-screen slideshow in the app (auto-advance, pause, arrows, the same transitions) and exported slideshow (`luma slideshow`): a self-contained folder, index.html and JPEGs, crossfade, slide or crossfade with a slow zoom (`--transition`), looping music (`--music`, M mutes it), shuffle (`--shuffle`, in the app too), keyboard, swipe | ✅ |
| Static web gallery (`luma gallery`, and from the app): grid of thumbnails loaded as they scroll in, photo shown large in one click (arrows, swipe, Esc), title, description, captions; JPEG optimized for the web (4:2:0 chroma and Huffman tables computed for the image: 9 to 29% lighter), in the Web preset and `--web` | ✅ |
| Basic video (`luma video`): a LÜMA film, an XMP preset or settings applied to every frame of a Motion JPEG AVI video (the format of many cameras), saved back in the same format, same size and same frame rate, the sound copied as is; with no native dependency | ✅ |

## Export

| Feature | Status |
|---|---|
| JPEG (quality), PNG and TIFF, 8 or 16 bits, lossless WebP (20 to 45% lighter than PNG, the same pixels) | ✅ |
| sRGB, Display P3, Rec. 2020, ProPhoto, with an embedded ICC profile | ✅ |
| Resizing: long edge, short edge, width × height, megapixels, no enlarging by default (`--resize`) | ✅ |
| Enlarging if needed (“Enlarge if needed”, `--enlarge`): halo-free Lanczos, up to four times each side and 200 MP, for large prints; no AI | ✅ |
| Print size (“Print (cm)”, `--resize 30x40cm@300`): the frame in centimetres and the resolution in dots per inch, written into the JPEG, the PNG and the TIFF | ✅ |
| Maximum JPEG size (“Maximum size”, `--max-kb 500`): the best quality that fits in the size, metadata included, for a website or a form that limits file sizes | ✅ |
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
| Batch rename (“Rename…”, `luma rename`): the export tokens (`{date}_{séquence}`…), the RAW and JPEG of a shot together, XMP and virtual copies follow, `-2` when a name is taken, rolled back on any failure; the library and its collections follow | ✅ |
| Catalogue backups (one a week in the app, the last ten kept; `luma catalog --backup`, `--restore`), missing photos listed, a moved folder found again with its collections (`--missing`, `--relocate`); in the app, folders as a tree (counts of the whole tree, photo-less paths merged, triangles that unfold), a folder removed from the library with its subfolders or opened in the file manager (right click, files kept), the “Maintenance…” window: missing photos by folder, located (neighbouring folders follow) or removed, backup on demand and restore after confirmation | ✅ |
| Duplicates in the catalogue (`luma catalog --duplicates`): same camera, same moment, files of the same size and type (a card imported twice, a copy in another folder), listed with the space they take; the RAW and JPEG of one shot are not duplicates; nothing is deleted | ✅ |
| Instant thumbnails (the preview embedded in the raw file, in 15 ms), then rendered with the adjustments and saved losslessly for later visits | ✅ |
| Full-resolution working copy: identical to the original, faster to open, two and a half times less memory; “Clear cache” in one click | ✅ |
| Keywords in the XMP (`dc:subject`), read and written | ✅ |
| The XMP a file carries inside (JPEG, TIFF, DNG, PNG, RAW) read when there is no sidecar: rating, label, keywords set by another application or by the camera (as attributes or elements), in the library and in the app; and the develop settings a RAW or DNG carries inside (never a JPEG's, already in its pixels) | ✅ |
| IPTC information in the XMP: title, caption, creator, copyright, location (sublocation, city, state, country); hierarchical keywords (“Places > France > Paris”, each level also in `dc:subject`); metadata templates given to the selected photos; in exported JPEGs | ✅ |
| Map of geotagged photos (GPS position from the EXIF): groups of nearby photos, drag, zoom, tap a group to see its photos; open tile base map kept on the device, usable offline; a photo without a position is placed by hand (the crosshair in the centre, “Place here”, written to its XMP), a whole selection at once | ✅ |
| Photos placed from a GPX track (`luma geotag --gpx`): the track's position at the moment of each shot (between the two points around it), camera clock offset (`--offset +02:00`, or guessed: `auto`), gaps in the track not bridged, and in the app (“Follow a GPX track…” on the map, Mac, Linux, Windows, and iPhone and iPad through the Files app), written to the XMP (`exif:GPSLatitude`); the library and the map read it, ahead of the file's own | ✅ |
| EXIF of developed images (JPEG, HEIC, AVIF, PNG, WebP, TIFF): capture date, camera, lens, settings and GPS position read as for a RAW; a phone's photos are sorted by their date and appear on the map | ✅ |
| Search and filters (`luma catalog --search`): words with accents ignored; rating, flag, label, dates (written, or relative to today: `date:30d`, `date:today`, `date:yesterday`), folder (`folder:wedding`; in the app, “All its photos” of a folder and its subfolders), ISO, rating as a range or exact (`rating:..2`, `rating:=0`), what is still missing (`without:rating`, `without:keyword`, `without:location`, `without:label`), focal length, camera, lens, keyword; counts of keywords, cameras and lenses | ✅ |
| Manual and smart collections (rule: rating, date, camera, lens, ISO, keywords…), saved in the catalogue; in the app, the current search saved as a smart collection (“Save as collection…”), a collection renamed or deleted with a right click; the “Collection” menu of the selected photos: put them in a manual collection (existing or new), remove them from the one shown | ✅ |
| Photos by date in the Library panel: each year with its number of photos, its months unfolded with a click; a year or a month shows its photos; “To do”: photos not yet rated, without keywords, without location, with their number; “Keywords”, “Cameras”, “Lenses”, “Ratings”, “Labels”: each value and its number of photos, a click shows its photos; a keyword renamed or removed in all its photos (right click, XMPs rewritten, everything else kept; `luma catalog --rename-keyword`, `--delete-keyword`, and for collections `--rename-collection`, `--remove-from`); search keeps spaces between quotes (`keyword:"sunset sky"`) | ✅ |
| The list of the photos shown (collection, search, date…) as a CSV table: file, date, camera, lens, ISO, aperture, shutter, focal length, rating, flag, label, keywords, location (“Export list (CSV)…”, `luma catalog --csv`) | ✅ |
| Keywords in the app: entry (several at once, separated by commas), added to and removed from every selected photo, the folder's keywords with their count, grid filter | ✅ |
| Catalog in the app: every folder you open is added (in the background); all photos, collections and folders, each with its number of photos; search and filters; views sorted by capture date | ✅ |
| Watched folder (tethered shooting, copied card): each complete photo enters the catalog and the filmstrip, and opens; `luma watch` | ✅ |
| Import from a memory card (“Import a card…”, `luma import`): the mounted card found on its own (DCIM folder), sorted by capture date, renamed from a template, copied to a second disk, each copy read back and compared with the original, the RAW, its JPEG and its XMP keeping the same name, photos already imported skipped, the card never modified, only the chosen days (one box per capture day, `--days`), and photo by photo on their previews (“Choose photo by photo”, `--skip`), the card ejected at the end on Mac, Linux and Windows (“Eject the card after importing”, `--eject`) | ✅ |
| iPhone and iPad: import from the Files app (“From Files” under “Import photos”, “•••” menu): a memory card in a card reader, iCloud Drive, a drive; several photos at once (RAW, HEIC, JPEG…), copied into LÜMA's folder like those from Photos | ✅ |
| On Mac, tethered camera (USB, or network if it allows) through the system framework: each photo taken arrives in the watched folder, shutter release from LÜMA for cameras that allow it | ✅ |
| Virtual copies: several developments of a photo without duplicating the file (one XMP per copy), each with its own rating, keywords and thumbnail, next to its original; exported as `IMG_0001-2`; deleted without touching the original; in the catalogue too (all photos, searches, collections), with their original's date and camera | ✅ |
| Importing existing catalogues | ⏳ |
| Customizable keyboard shortcuts (“Keyboard shortcuts…”: every command, its keys, a key already taken moves to the new command, back to the default keys; “Raccourcis.txt”); Tab hides the panels; workspaces (Culling, Develop, Retouching, Presentation and your own: tool and panels), Cmd+1 to Cmd+9 | ✅ |
| “Reset everything…” (language menu, and “•••” on iPhone), after a confirmation: the edits of the selected photos and of the open photo are cleared (their ratings, keywords and files stay; Undo brings back the open photo's), language, shortcuts, panels, filters and the export, print, slideshow, book and gallery choices go back to first launch; library, presets, templates, workspaces and license are kept | ✅ |
| File names, keywords and captions in every script (Japanese, Chinese, Korean, Arabic, Hebrew…): letters LÜMA's font lacks are taken, on demand, from the system fonts (Mac, Windows, Linux), instead of empty boxes | ✅ |

## What sets LÜMA apart

- **Nothing is sent anywhere**: no account, no mandatory cloud.
- **100% Rust, verified**: `scripts/audit-rust.sh` checks the dependency graph on
  every build (170 crates, no native bindings and no C compiler).
- **Measured, not promised**: `luma bench` and CI publish actual render times.
