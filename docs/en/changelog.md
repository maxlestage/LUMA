# What's new

## 0.3.0 — upcoming

The professional photo library, step by step.

- **Faster film and camera-look measurement**: smoothing the LUT (400 passes over
  35,937 nodes) drops from 75 to 41 ms, with the same bits.
- **Lens distortion and vignetting for Canon, Nikon, Pentax…**: “Corrections from the
  camera's JPEG” (Lens corrections, when the file carries none that LÜMA reads)
  compares the photo with the JPEG the camera stored in the RAW, already corrected for
  its lens, and sets Distortion and Vignetting for the same geometry and the same
  edges; empty borders are removed if the photo isn't cropped. No AI: the same details
  found on both sides, fitted by least squares; for vignetting, the camera's tone curve
  is learned in the centre, then undone towards the edges. On all selected photos at
  once (selection button), in the background, each measured on its own (focal length
  changes everything).
- **The glimpse when a photo opens is two to three times faster**: the preview the
  camera stored in the RAW is shrunk on every core, with the same pixels. At 2048 px:
  RAF 177 → 77 ms, NEF 165 → 77 ms, DNG 284 → 140 ms, ORF 201 → 61 ms; measuring the
  camera's look benefits too.
- **The camera's look, measured**: “Measure the camera's look” (Films › My film stocks)
  compares the open RAW with the JPEG the camera stored in it and turns it into a film
  named after the camera (“Sony ILCE-7M3 JPEG”): its RAW files rendered the way it
  does, at an adjustable strength. No AI: the film measurement, applied to the same
  shot. On the seven brands of the test corpus, the difference from the camera's JPEG
  drops from 8–29 to under 2, in half a second. With other RAW files from the same
  camera selected (up to twelve), the measurement uses them all: more colours, a more
  faithful film on later photos; those from another camera are left aside. From the
  command line: `luma film-lut --camera`.
- **Reset everything**: in the language menu (“•••” on iPhone), after a confirmation
  that says what changes and what stays: the edits of the selected photos and of the
  open photo are cleared (ratings, keywords and files kept; Undo brings back the open
  photo's), and language, shortcuts, panels, filters and the export, print, slideshow,
  book and gallery choices go back to first launch. The library, presets and license
  stay.
- **Lighter folder tree**: rebuilt only when the library changes, no longer on every
  frame drawn (0.7 ms less per frame with 2,000 folders).
- **Import from the Files app on iPhone and iPad**: “From Files” under “Import photos”
  (and in the “•••” menu) opens the Files app: RAW files from a memory card in a card
  reader, from iCloud Drive or from a drive arrive in LÜMA, several at once.
- **“Sign in with Apple” on iPhone and iPad**: people who were given LÜMA Premium unlock
  it by signing in with their Apple Account from the Premium screen; the server checks
  Apple's token and the email against the list of gifted access.
- **LÜMA for Linux**: the same engine and the same rendering as on Mac, with the same
  subscription. An archive that installs without a password (`./install.sh`, with its
  launcher in the applications menu and RAW files opening through “Open With”), or a
  `.deb` package for Debian and Ubuntu; x86_64, Ubuntu 22.04, Debian 12, Fedora 36 or
  later, with a Vulkan graphics driver. The `luma` command line is included.
- **Faster**: colour noise reduction (on by default) is three times faster without
  changing a single pixel; full-resolution export takes 25 to 30% less time, and the
  preview with heavy adjustments is about 25% more responsive. Then the blurs: two
  buffers instead of six new images, so full-resolution sharpening is almost twice as
  fast (NEF 0.64 → 0.36 s), without changing a pixel. Then JPEG writing, on every
  core: 2.3 to 2.6 times faster, byte-for-byte identical files; a full-resolution 24 MP
  export in 1.8 s instead of 2.4. Then the tone rendering step, computed eight pixels
  at a time: 1.8 to 1.9 times faster (NEF 160 → 85 ms), byte-for-byte identical files.
  Then colour (profile and Saturation), the same way: three times faster (NEF 147 →
  48 ms). Then JPEG coding, another 13 to 15% shorter (24 MP: 200 → 175 ms), same
  bytes. Then the lens corrections recorded by the camera (Sony, Fujifilm, Olympus,
  Panasonic), eight pixels at a time: 30% faster (24 MP ARW: 385 → 270 ms), same files
  byte for byte. Then Vibrance, the same way: the colour stage 2.7 times faster when it
  is set (NEF 372 → 136 ms), same bytes.
  Then 16-bit TIFF and PNG exports: the image written straight to
  bytes, on every core, without a second copy: 20 to 28% shorter (24 MP: 372 → 299 ms)
  and 144 MB less memory, same bytes. Then batch export with a logo: the logo is read
  once for the whole batch (40 ms less per photo for a 1200 × 600 logo). Then the
  conversion to the output space (JPEG), eight pixels at a time: 24 MP in sRGB 230 →
  150 ms, same bytes; and for 16-bit TIFF and PNG: sRGB about 35% shorter (24 MP:
  295–366 → 190–204 ms), same bytes. Then colour with the mixer and colour grading: the
  trips to OKLab and back eight pixels at a time, about 30% shorter (2048 px preview:
  50–60 → 34–43 ms), same bits. Then local tone (highlights, shadows, clarity,
  texture): its three bases computed at the same time, 12 to 18% shorter (2048 px
  preview: 48–55 → 42–45 ms), same pixels.
  Then wavelet noise reduction, without a new image at each scale: 1.8 times faster
  (24 MP: 1.17 → 0.65 s), and deconvolution sharpening, eight pixels at a time: 28%
  shorter (0.83 → 0.60 s), same pixels. Then the edge-following filter (Shadows,
  Highlights, Clarity, mask and sky edges), without intermediate planes: almost twice
  as fast (24 MP: 87 → 47 ms), same values. Then JPEG export, converted to 8 bits strip
  by strip: 72 MB less memory for 24 MP, same files byte for byte. Then Highlights,
  Shadows, Clarity and Texture, applied in a single pass: 15 to 20% faster at full
  resolution (26 MP RAF: 0.84–0.92 → 0.72 s), same pixels. Then luminance noise
  reduction: 104 MB less memory for 26 MP, same pixels. Then the vignette: 37% shorter
  (24 MP: 108 → 68 ms), almost twice as fast with a roundness (400 → 210 ms), same
  pixels. Then Remove (circle or stroke): 28% shorter, then another 22% (almost twice
  as fast overall), same pixels. Then enlarging on export: 18% shorter (6 → 24 MP:
  440 → 360 ms), same pixels. Then the maximum JPEG size, found in three encodings
  most of the time instead of eight: exporting a 24 MP RAW under 2,000 KB 17% shorter
  (3.95 → 3.3 s), same files.
- **No more empty boxes on screen**: two film descriptions and the “Film measured”
  message contained characters missing from the app's font; a test now checks every
  text of the interface, in all three languages.
- **Reliability**: a full code review. Highlights no longer turn black when Whites
  is pushed hard; no more crashes on a malformed HEIC, ICC or DNG file, a one-pixel-wide
  image, a retouch outside the photo, a locked-ratio crop dragged to its smallest, or an
  oversized panorama. An export never overwrites an original photo; import no longer
  replaces a library XMP or a file on the second disk, and keeps each photo's XMP within
  a shot; restoring the oldest catalogue backup works, and an unreadable catalogue is
  kept in the backups. In the app, the previous photo's preview no longer covers the one
  being opened, feathered masks look as they export, and a GPU that stops responding
  lets the processor finish the preview. Two tests stand guard: damaged RAWs (a cut
  copy, scrambled bytes, an empty file) open as an image or an error, never a crash, and
  settings with absurd values (infinite, huge) render to sound pixels, in masks,
  retouches and liquify too. A long, very wide traced removal no longer freezes the
  export (24 MP: 2 minutes → 4 seconds), same pixels. In the app, a computation that
  fails on an unusual photo no longer freezes anything: the next preview arrives,
  opening, export, merge or film measurement report the error, and an export batch goes
  on with the others. No more settings lost all at once: a single unreadable mask,
  retouch or liquify stroke in an XMP (a shape from a newer version, a damaged value)
  no longer erases the others; a damaged export preset no longer loses the others, and
  an unreadable presets file is kept aside (`Exports.json.illisible`) before being
  replaced. The same goes for a photo's snapshots, the per-camera defaults and the
  metadata templates: a single damaged item no longer takes the others with it. A
  photo's XMP is written in one go (a temporary file renamed): an app stopped or a disk
  full during the write leaves the previous XMP whole, never a truncated one; likewise
  for a saved preset, a merge's XMP and the licence state (a key or a purchase is no
  longer lost on a truncated file). Exports too (JPEG, PNG, TIFF): a cut-off export no
  longer leaves a damaged file under the chosen name, nor damages the previous export of
  the same name. And measured films (`.cube`): a film no longer disappears from “My
  films” on a cut-off save.
- **No more wasted space**: a working duplicate or thumbnail whose writing fails (disk
  full) or is interrupted (app closed) no longer leaves a half-written file behind; the
  clean-up removes earlier ones (5.3 GB recovered on the test machine).
- **Lens corrections always applied**: a working duplicate written by a version from
  before the file's lens corrections were read (Sony, Fujifilm, Olympus, Panasonic, DNG)
  is made again once, instead of showing the photo without them.
- **Faster DNG distortion**: correcting the distortion recorded in a DNG (phones,
  cameras that record DNG) is about 35% faster (6 Mpx: 230 → 150 ms), without changing a
  pixel.
- **Exported slideshow: transitions and music**: choose a crossfade, a slide (the next
  photo pushes the previous one) or a crossfade with a slow zoom on each photo; music
  (MP3, AAC, Ogg, WAV, FLAC) copied into the folder and played in a loop, paused with the
  slideshow, muted with M (“Book, slideshow and gallery”, `luma slideshow --transition
  --music`). The app's full-screen slideshow moves from one photo to the next with the
  same transition (before: a hard cut). The slideshow's “Fade” label is now correctly
  translated in English and Spanish.
- **Import photo by photo**: “Choose photo by photo” shows the preview of each photo
  on the card (the one embedded in the RAW, read in the background); a click leaves one
  on the card, with its JPEG; “All”, “None”; together with the chosen days (`luma import
  --skip photo`).
- **Snappier sliders**: every preview render copied its base separately, on a single
  core; the copy now happens in the first pass, on all cores. A render with basic
  settings is 4 to 23% shorter depending on the camera (iPhone DNG 120 → 97 ms, ORF 86 →
  67 ms), without changing a pixel.
- **The card ejected after importing** (Mac, Linux): “Eject the card after importing”,
  once every photo has been copied and verified (`luma import --eject`); if the system
  refuses (an app is still reading the card), the import says so, without failing.
- **Import on Windows**: the inserted card is found on its own (a drive from D: to Z:
  with a DCIM folder), and ejected after importing as on Mac and Linux. Cards are looked
  for in the background, every two seconds while the window is open: a card inserted
  later shows up, and a missing network drive no longer freezes the window.
- **Book pages**: “Book pages…” shows each page of the photo book small, with its
  photos in place; dragging a photo onto another swaps them, and the PDF follows the new
  order (cover included). Under each page, 1, 2 or 4: its photos; the pages before keep
  theirs, those after follow (“Automatic layout” to go back; `luma book --layout
  4,1,2`).
- **Move a book photo**: dropped onto the background of a page (outside its photos), a
  photo becomes its first one, the others shift by one place; dropped onto a photo, they
  still swap. Each photo in “Book pages” is also named for screen readers (“file · Page
  2”).
- **Snappier highlights, shadows, clarity and colour grading**: their last
  computations run eight pixels at a time, and the photo's reference points
  (percentiles) are found twice as fast; a render is 3 to 15% shorter when these
  settings move (shadows on an RW2: 78 → 66 ms; grading on an ARW: 68 → 60 ms), without
  changing a pixel.
- **iPhone DNGs open twice as fast**: a DNG already in RGB (ProRAW) is reduced straight
  from its pixels, without first copying the whole image (145 MB); and a quarter turn is
  done in tiles. First preview 220 → 109 ms, without changing a pixel; every portrait
  photo also gains on orientation (15 → 9 ms).
- **Masks from an XMP made elsewhere**: an XMP written by another raw developer brings
  its linear and radial gradients (`crs:MaskGroupBasedCorrections`, and the older
  gradient lists), with their amount, inversion and local settings (exposure, contrast,
  highlights, shadows, whites, blacks, temperature, tint, saturation, clarity, texture,
  sharpness, noise, dehaze), brought into the crop. Brushes and subject masks, made of
  pixels, are not taken over.
- **GPX track on iPhone and iPad**: “Follow a GPX track…” opens the Files app (iCloud
  Drive, a watch, a hiking app); the chosen track places the photos that have no
  position, as on Mac.
- **Faster first preview**: the sensor reduction that prepares it no longer looks up
  the colour of every photosite (one table per place in the pattern, and a separate path
  for Bayer sensors): 30% shorter (X‑Trans 90 → 58 ms); the first preview is 4 to 19%
  shorter (RAF 216 → 175 ms, NEF 127 → 110 ms), without changing a pixel.
- **CMYK printing**: a CMYK printer profile (a press, a print shop) is accepted for
  printing and the photo book; pages are converted to its four inks (black point
  compensated) and the PDF is in CMYK, its inks kept lossless (“Print…”, `luma print
  --profile`, `luma book --profile`).
- **Open the finished PDF**: “Open …” opens the last PDF, book, slideshow or gallery
  with the system's app (the PDF reader, to print from; the browser); the “Print” window
  stays open once the PDF is made. On Windows, links (account, payment) finally open:
  LÜMA was launching a Linux program there.
- **Slideshow in random order**: “Shuffle”, in the app (each photo once before a new
  draw) and in the exported slideshow (a new draw each time the page opens; `luma
  slideshow --shuffle`).
- **First photo opens faster**: the decoder's camera and lens databases (≈ 75 ms, once
  per session) load at app startup, while its window opens, rather than at the first
  photo: 60 to 95 ms less for it (NEF 173 → 80 ms, RAF 176 → 90 ms, ARW 160 → 98 ms).
- **Pages as JPEG**: printing and the photo book can also be saved as JPEG, one image
  per page (“JPEG” in the save window, `-o sheet.jpg`: `sheet-1.jpg`, `sheet-2.jpg`…),
  at the chosen resolution and in sRGB, for a book printing service or to share a
  contact sheet.
- **Ratings already set elsewhere**: without a sidecar, LÜMA reads the XMP the file
  carries inside (a JPEG, TIFF, DNG or PNG rated in another application; a RAW rated on
  the camera): its rating, label and keywords enter the library and the app. A sidecar
  always wins.
- **Lighter export in memory**: at full resolution (an export, 1:1 zoom), rendering
  happens in the base image itself instead of a copy: an export's peak memory drops by 19
  to 23% (24 Mpx: 1.24 → 0.96 GB), the files stay identical byte for byte.
- **Settings a RAW carries inside**: a RAW or DNG developed in another application,
  which stored its settings in the file, opens with them when it has no sidecar (in the
  app and `luma develop`, `print`, `book`); never a JPEG's, already in its pixels.
  Ratings and labels written as elements are read too, and the embedded XMP is read
  only once per file (64 KB at most, kept in memory).
- **Library maintenance in the app**: “Maintenance…”, below the folders, lists the
  missing photos folder by folder; a folder is located by choosing its new place
  (when the whole parent folder moved, the neighbouring folders follow on their own)
  or removed from the library, collections included. Catalog backups are listed by
  date: one more on demand, or an older one put back after confirmation (the current
  library is backed up first).
- **Faster previews on the CPU**: the previous preview's image is reused as memory
  for the next one (no fresh memory at each edit, 64 MB kept at most), and the
  conversion for the screen is done in one pass, on all cores: a 4.4 MP preview 10 to
  15% faster, the conversion of a 24 MP image 38% faster (1:1 zoom, compare, look
  previews); the same pixels, byte for byte.
- **Send to printer**: in “Print…”, the finished PDF goes straight to a system
  printer (the default one first), with the number of copies, without going through
  the PDF viewer; on Mac and Linux, through the system's printing service. With no
  printer, the app says so and “Open” stays available.
- **Photos by date**: the Library panel lists all photos by year (the most recent
  first, with their number); a triangle unfolds the months; a click on a year or a
  month shows its photos.
- **Faster 8-bit conversion**: each byte of a JPEG or of the preview is read from a
  byte table instead of being interpolated then rounded (19% less for 24 MP, on export
  and on screen); the same bytes, checked for each of the billion possible numbers
  between 0 and 1.
- **Dates relative to today**: search and smart collections understand `date:30d`
  (the last thirty days, today included), `date:today` and `date:yesterday` (`30j`,
  `aujourd'hui`, `hier` in French); a “date:7d” collection follows the days.
- **Finding what is left to do**: search and smart collections take the rating as a
  range or exact (`rating:..2`, `rating:2..4`, `rating:=0` for photos not yet rated)
  and `without:`: `without:rating`, `without:keyword`, `without:location`,
  `without:label`, which can be combined.
- **JPEG and PNG five times faster to open**: an 8-bit-per-channel image (JPEG, PNG,
  TIFF, WebP, PSD…) is turned into linear values through a table of its 256 values
  (and its opacities), on all cores, with no intermediate floating-point image: a
  24 MP JPEG opens in 0.18 s instead of 0.9 s; the same values, to the bit. Likewise
  opaque 16-bit-per-channel images (TIFF, PNG), through a table of their 65,536
  values: a 24 MP 16-bit TIFF in 0.25 s instead of 0.95 s.
- **“To do” in the library**: the Library panel counts the photos not yet rated,
  without keywords and without location; a click shows them (a line disappears when
  none are left).
- **Every script on screen**: a file name, keyword or caption in Japanese, Chinese,
  Korean, Arabic or Hebrew is shown with a system font (Mac, Windows, Linux), found at
  the first missing letter, instead of small empty boxes; nothing is loaded while
  nothing is missing.
- **Smart collections from the app**: the current search is saved as a smart
  collection (“Save as collection…”, a name, Enter), which runs it again every time;
  a right click on a collection deletes it (its photos stay). Until now, only the
  command line could do it.
- **Manual collections from the app**: the “Collection” menu puts the open photo and
  the selected photos in a collection (an existing one, or a new one named on the
  spot), and removes them from the collection shown.
- **Faster and lighter 16-bit PNGs**: written by LÜMA on all cores (the image in
  bands, each filtered and compressed on its own, one stream): exporting a 24 MP photo
  as a 16-bit PNG takes 1.8 to 2.0 s instead of 2.5 to 2.7 s, for a file 10% lighter;
  the same pixels. 8-bit PNG keeps its encoder, already the fastest and lightest
  (measured).
- **Keywords in the library**: a folded “Keywords” section of the Library panel
  lists each keyword with its number of photos; a click shows its photos. Search keeps
  spaces between quotes (`keyword:"sunset sky"`, straight or curly quotes, or
  guillemets).
- **Cameras and lenses in the library**: two folded sections, “Cameras” and
  “Lenses”, like keywords: each camera and each lens with its number of photos, a
  click shows its photos (the whole name, spaces included).
- **A snappier library**: the panel's lists (folders, keywords, cameras, lenses) are
  counted without copying each value of each photo; for 100,000 photos, updating them
  after a change goes from about 200 ms to 90 ms; the same lists. The numbers of “To
  do” and of the collections are counted without sorting the photos: 11 ms instead
  of 36 ms for the three “To do” lines.
- **A keyword renamed or removed everywhere**: in the keyword list, a right click
  renames a keyword in every photo that has it, or removes it; their XMPs are
  rewritten (rating, flag and the rest stay), and a photo that already had the new
  name does not get it twice.
- **A collection renamed**: the right click on a collection renames it (the name is
  typed on the spot); it keeps its photos and its rule, takes its place in
  alphabetical order, and stays shown if it was; a name already taken is refused.
- **Keywords and collections from the command line**: `luma catalog` renames a
  keyword in all its photos (`--rename-keyword old new`) or removes it
  (`--delete-keyword`), the XMPs rewritten without losing anything else; renames a
  collection (`--rename-collection`) and removes a photo from one (`--remove-from`):
  what the app does, through the same code.
- **Ratings and labels in the library**: two more folded sections, “Ratings” (the
  number of photos from 5 to 1 star) and “Labels” (each label and its number of
  photos); a click shows their photos.
- **Faster catalog reading and writing**: for 100,000 photos, writing goes from 59 to
  43 ms and reading from 108 to 78 ms (a fast hash for the text table, the number of
  the previous photo's folder, camera and lens reused without a lookup); the file is
  the same, byte for byte.
- **A folder removed from the library**: the right click on a folder of the Library
  panel removes it from the library (its photos are no longer tracked, nor in
  collections; subfolders included; the files are not touched) or opens it in the
  system's file manager.
- **The photo list as CSV**: “Export list (CSV)…” (and `luma catalog --csv`) writes
  the photos shown — a collection, a search, a date — as a table for a client or for
  archives: file, date, camera, lens, ISO, aperture, shutter, focal length, rating,
  flag, label, keywords, location.
- **Faster library views**: when showing all photos, a collection or a search, the
  app checks whether each photo is still there on all cores (and likewise for the
  missing photos of “Maintenance…”): 3 times faster on a local disk (20,000 photos:
  17 → 5 ms), more on an external or network drive; the same photos, in the same
  order.
- **Folders as a tree**: the Library panel arranges folders as a tree; each with the
  photos of its whole tree, a triangle that unfolds its subfolders, and photo-less
  paths merged into one line (“Photos/2024” rather than every step of the disk).
  Removed from the library, a folder takes its subfolders with it.
- **Searching by folder**: `folder:wedding` (or `dossier:`) keeps the photos whose
  folder path contains that text, case and accents aside, in search and smart
  collections; in the folder tree, the right click “All its photos” shows a folder
  and all its subfolders at once.
- **LÜMA for Windows**: Windows 10 and 11 (x64), rendered with DirectX 12 (or Vulkan);
  an installer that needs no administrator password (Start menu, uninstall) and a .zip
  archive to unpack anywhere, with the `luma.exe` command line. Settings, catalogue and
  presets in `%APPDATA%\LÜMA`, caches in `%LOCALAPPDATA%\LÜMA`. Built, installed,
  launched with a real RAW and uninstalled by CI on every push.
- **A one-month trial everywhere**: on Mac, Linux and Windows too, the full trial now
  lasts one month (14 days before), as on iPhone and iPad.
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
- **Directional demosaicing**: each pixel's green follows edges instead of crossing
  them; branches, hair and fine textures no longer show beading or zipper artefacts
  (4 dB better in our tests). Working copies are rebuilt once.
- **Point curves in the app** (Light, Tone curve): lightness, red, green and blue; a
  click adds a point, drag it, a double-click removes it.
- **Maximum JPEG size** (Export, “Maximum size”, or `--max-kb 500`): the best quality
  that fits in the size, metadata included, for a website or a form that limits file
  sizes; kept in presets.
- **Traced healing and clone** (Retouch): dragging along a wrinkle or a scratch fixes it
  in one gesture, the source following the stroke.
- **Print size** (Export, Dimensions, “Print (cm)”, or `--resize 30x40cm@300`): the
  frame in centimetres and the resolution in dots per inch, written into the JPEG, the
  PNG and the TIFF for the printer.
- **Duplicate a mask** (Masks): a copy, or an inverted copy with settings at zero, to
  adjust the ground after the sky without redrawing the area.
- **Traced removal** (Retouch, Remove): dragging along a wire, a scratch or a cable
  erases it in one gesture, replaced by the surrounding photo. No AI.
- **Auto tone** (Light, Tone, or `--auto-tone`): one click sets exposure, contrast,
  highlights and shadows from the photo's histogram, a starting point to refine. No
  AI.
- **Noise reduction in masks**: smooth lifted shadows or a noisy sky with the brush,
  keeping edges sharp, without softening the rest of the photo.
- **Sharpness in masks**: bring out the detail of eyes or a material with the brush,
  or soften a background, without touching the rest of the photo.
- **Directional X‑Trans demosaicing** (Fujifilm): green follows edges instead of
  averaging across them, even in noisy shadows (3 dB better in our tests). Working
  copies are rebuilt once.
- **Texture and Dehaze in masks**: soften skin or a background with the brush, clear a
  hazy sky in a gradient, without touching the rest of the photo.
- **Enlarge if needed** (Export, Dimensions, or `--enlarge`): a size larger than the
  photo enlarges it, up to four times each side, sharp and without halos around edges;
  for large prints. No AI.
- **Liquify** (Retouch): Push, Bloat and Pucker with a brush, with its size and
  strength; to straighten a line, refine a shape or fix a fold. Every stroke is kept in
  the XMP and can be undone. No AI.
- **Select sky** (Masks, Masking): one click and the sky becomes a mask, clouds
  included, following the edges of trees and roofs; a lake reflecting it stays out.
  No AI: only the colours and texture of the photo.
- **Remove** (Retouch, next to healing): a circle over what is in the way, a wire, a
  passer-by, litter, and LÜMA replaces it with pieces of the surrounding photo, texture
  and grain included. No AI: only the photo itself.
- **Panorama edges warped** (Merge, Panorama, “Warp the edges”, or
  `luma merge --pano … --warp`): instead of cropping a panorama's ragged edges, LÜMA
  gently stretches them to fill the whole rectangle; the whole scene is kept.
- **Empty edges cropped** (Geometry, Transform): after a perspective, rotation or
  distortion correction, one click crops to the largest frame with no black corner, in
  the photo's proportions.
- **Guided upright** (Geometry, Transform): drag along two to four edges that should
  be straight (a wall, a pole, the horizon); LÜMA sets the perspective and rotation
  right away. A single line only rotates the photo.
- **Colors matched between two cameras**: the same scene shot with both your cameras,
  even framed a little differently; select both photos, then “Match to the selected
  photo” (Channel mixer), or `luma match-colors`. The second camera takes on the colors
  of the first; saved as a preset, the correction applies to all its photos.
- **Camera profile**: shoot a ColorChecker chart, crop it tightly, then “Profile from a
  chart” (Channel mixer) or `luma colorchecker chart.ARW -o profile.xmp`: LÜMA computes
  the correction that makes its 24 colors exact (from 3.8 to 0.02 color error in our
  tests), white staying neutral. Saved as a preset, it applies to every photo from that
  camera.
- **Focus stacking** (Merge, “Focus”, or `luma merge --stack focus`): shots whose focus
  moves forward, in macro or landscape, become a single photo sharp from foreground to
  background. Each shot is aligned to its neighbor, and LÜMA takes the sharpest part of
  each.
- **HDR merge** (Merge, “HDR”, or `luma merge --stack hdr`): bracketed shots, even
  handheld, become a single DNG where shadows and sky keep their detail. LÜMA measures
  the exposure difference between shots itself, and whatever moved from one to the next
  (foliage, passers-by) leaves no ghost.
- **Highlight reconstruction**: the “Reconstruct highlights” box (Light, under Tone)
  gives a channel clipped by the sensor the color of its neighbors; a burnt sky turns
  orange again, a white towel gets its texture back. A pixel where everything clips
  stays white. Also on the command line (`--set highlights.reconstruction=on`).
- **Deconvolution sharpening**: the “Deconvolution” box in Sharpening undoes the lens
  blur instead of digging into edges; real detail comes back, without halos (1.6 dB
  better than unsharp masking in our tests). Visible at 100% and on export; also on
  the command line (`--set sharpen.deconvolution=on`).
- **Wavelet noise reduction**: the “Wavelets” box in Detail smooths luminance scale by
  scale, each according to the noise measured on the photo; at the same strength, more
  texture and edges are kept than with the usual filter (3.7 dB better in our tests),
  for about twice its time. No AI.
- **Auto verticals** (Shift+V, or the Transform button): a building shot while aiming
  upwards gets straight walls back; LÜMA measures where its verticals meet and sets the
  Vertical accordingly.
- **Auto straighten** (Shift+R, or “Auto straighten” in cropping): LÜMA reads the
  photo's horizon and verticals and levels it; when in doubt (a close-up made of
  vanishing lines), it leaves it alone.
- **Crop overlays**: thirds, golden ratio, diagonals, golden triangles, golden spiral
  and grid; O switches to the next overlay while cropping, Shift+O rotates the spiral
  and the triangles.
- **Channel mixer** (Color tool): each red, green or blue channel made from a mix of
  the photo's three channels, from −200 to +200%; saved in the XMP and in presets,
  without slowing the render.
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
- **Duplicates** (“Find duplicates”): the same image in several files, copied,
  exported at another size or recompressed, found by its fingerprint and shown group
  by group. **In batch**: with several photos selected, the rating, flag and label go
  to all of them.
- **Virtual copies in the library**: they now also appear in “all photos”, searches and
  collections, with their original's date and camera and their own rating; a deleted
  copy leaves it.
- **Opening twice as fast**: sensor levels (black, white) normalized straight from
  its integers, on all cores, with no prior copy to floats; NEF 145 → 67 ms, ARW
  143 → 65, RW2 131 → 65, RAF 169 → 73, DNG 285 → 158, CR3 547 → 399, bit-identical
  data.
- **Sync a selection** (Shift+Cmd+S, or “Sync…” as soon as two photos are selected):
  the open photo's adjustments go to the others, group by group (crop and masks left
  to each one by default). **Batch auto balance**: each selected photo gets its own
  automatic white balance.
- **Default adjustments per camera and ISO** (“Default adjustments…”, Light tool): the
  open photo's adjustments become the starting point for this camera's photos without
  XMP, at any ISO or from its ISO (the most precise rule wins; crop and masks
  excepted). **Import film**: chosen in the watched folder, it is given to each
  arriving photo.
- **IPTC information** (Library, “Information”): title, caption, creator, copyright
  and shooting location, for one photo or all selected photos, in the XMP and in
  exported JPEGs; **hierarchical keywords** (“Places > France > Paris”); **metadata
  templates** (creator, copyright, location, keywords) saved and applied in one go.
- **Map** (Library, “Map”): the filmstrip's photos where they were taken (GPS
  position from the EXIF, kept in the library), grouped when close together; drag,
  zoom with the wheel or two fingers, tap a group to see its photos. Open tile base
  map, downloaded by the system and kept on the device; offline, those already kept.

- **Phone photos at their date and on the map**: the EXIF of developed images (JPEG,
  HEIC, AVIF, PNG, WebP, TIFF) is read like a RAW's — capture date, camera, lens,
  settings, GPS position. Photos already in the library are read again once.
- **A GPX track to place photos** (`luma geotag --gpx track.gpx`): each photo takes the
  track's position at the moment it was taken, written to its XMP; the camera clock's
  offset is given with `--offset`, or guessed (`--offset auto`). The library and the
  map show it at once. In the app on Mac: “Follow a GPX track…” on the map, the offset
  guessed.
- **Place a photo on the map by hand**: on the map, the displayed photo that has no
  position shows a crosshair in the centre; drag the map under it, then “Place here”
  writes it to its XMP and to the library. The chosen photos that have no position are
  placed at once (“Place the 12 photos here”).
- **Scopes** (above the histogram, or Shift+J): **waveform** (luminance, column by
  column), **RGB parade** (each channel on its own) and **vectorscope** (hue and
  saturation, with the 75% primary targets and the skin‑tone line), computed on the
  preview at every render, soft proofing included.
- **Point measurement**: below the histogram, the RGB (sRGB, 0 to 255) and Lab values
  under the pointer, read from the true colours even while clipping is shown.
  **Per‑channel clipping**: on the photo and in the histogram corner, the colour tells
  which channels are blown (red, yellow for red and green…, white for all three).
- **Batch rename** (“Rename…” on the selected photos, `luma rename`): a template with
  the export tokens (`{date}_{séquence}`, `{boîtier}-{nom}`…), with a preview of the new
  names before confirming; the RAW and JPEG of the same shot keep one name, the XMP and
  virtual copies follow, a name already taken gets `-2`; on any failure, everything goes
  back as it was. The library and its collections follow.
- **Snapshots** (Light tool, below the history): the adjustments of a moment, under a
  name, stored in the photo's XMP; one click goes back to them (a history step that can
  be undone).
- **Catalogue backups**: one a week, on its own, in “Sauvegardes” next to the catalogue
  (the last ten kept); `luma catalog --backup`, `--backups`, `--restore` (the replaced
  catalogue is backed up first). **Missing photos**: `--missing` lists them,
  `--relocate old new` finds a folder moved outside LÜMA, collections included.
- **Duplicates** (`luma catalog --duplicates`): the photos in the catalogue twice (same
  camera, same moment, files of the same size and type, like a card imported twice),
  listed with the space they take, without deleting anything.
- **Printing** (“Print…” on the selected photos, `luma print`): a PDF at the paper's
  exact size (A4, A3, A3+, 10 × 15 cm…), portrait or landscape, margins, one photo per
  page or a **contact sheet** (grid, name under each photo), photos whole or filling
  their cell, output sharpening for matte or glossy paper, page preview; pages
  converted to the **printer profile** (the soft-proofing one, black point
  compensated), to print without the driver's color management.
- **Crop marks and text at the bottom of the page** (“Print…”, `luma print --crop-marks
  --text`): thin lines at the corners of each photo, to cut with a guillotine, and a line
  of text (a title, a name, a copyright) in the bottom margin.
- **Photo book** (“Book, slideshow, gallery…”, `luma book`): a PDF with its cover (the first
  photo, the title and subtitle), then pages of one, two or four photos chosen from
  their orientation; 20 × 20, 30 × 30, A4, 21 × 28, A3 formats.
- **Text pages in the book** (“Book, slideshow, gallery…”, `luma book --opening
  --closing`): a page after the cover (a dedication, a few words of introduction) and a
  last page (a place, a date, acknowledgements), their lines centred on the page. And
  the layout of your choice: the number of photos on each page (“1,2,4”, repeated;
  `--pages`).
- **Slideshow**: full screen in the app (the photo alone, advancing by itself, Space to
  pause, Esc to leave), and **exported** (`luma slideshow`): a self-contained folder,
  index.html and the photos, to open in any browser or put online (crossfade, arrows,
  swipe, full screen).
- **Web gallery** (“Book, slideshow, gallery…”, `luma gallery`): a static folder to
  put online as is, the grid of thumbnails and each photo large in one click (arrows,
  swipe, Esc), with title, description and captions.
- **JPEG optimized for the web**: 4:2:0 chroma and Huffman tables computed for each
  image, 9 to 29% lighter on the test photos, for a barely measurable difference; in the
  Web preset, the export window (“Optimized for the web”) and `--web`.
- **Basic video** (`luma video`): a LÜMA film (“Portra 400”, “Tri-X 400”…), an XMP
  preset or settings applied to every frame of a short Motion JPEG video (the AVI format
  of many cameras), saved back in the same format: same size, same frame rate, the sound
  copied as is. Read and written in pure Rust.
- **Import from a card** (“Import a card…” in the Library, `luma import`): the mounted
  card is found on its own; the photos are sorted by capture date, renamed if you
  wish, also copied to a second disk; each copy is read back and compared with the
  original, the RAW, its JPEG and its XMP keep the same name, a photo already imported
  is skipped, the card is never modified.
- **Import only some days**: the import window lists the card's capture days, each with
  its number of photos; untick those you don't want (`luma import --days
  2026-10-26,2026-10-27`).
- **Customizable keyboard shortcuts** (language menu, “Keyboard shortcuts…”): every
  command and its keys; click, then press the new keys; a key already taken moves to
  the new command; back to the default keys, one by one or all. **Tab** hides the
  panels.
- **Workspaces**: Culling, Develop, Retouching, Presentation, and the ones you save (the
  tool and the panels), from Cmd+1 to Cmd+9.
- **iPhone: the photo grid realigned**: columns computed from the screen width, on the
  islands' margins, the file name above the photo instead of over it; the Grid, Map and
  Slideshow bar fits on one line (“Book, slideshow, gallery…” moves to “•••”); the
  histogram alone in the Photos panel. After an app update, photos follow the app's new
  folder instead of showing up in a second “Documents”.
- **iPhone and iPad: touches land where they should**: the image finally has the
  screen's size (it was made for the safe area, then stretched 11% vertically); the
  bottom of the panels shows (Snapshots, below the history), a finger scrolls the panel
  and hits what it aims at. The top bar holds its 40‑point buttons.
- **iPhone and iPad: exports go to your photo library**: every exported image is added
  to the Photos app as is (EXIF, IPTC and colour profile kept; iOS asks for permission
  the first time), and also stays in LÜMA/ (Files app). The Export window fits the
  iPhone screen, each setting under its title.
- **Films in collapsible families**: colour film, black and white, cinema, instant,
  my presets, my films; each title tells how many films the family holds, and only the
  applied film's family opens at first.
- **150 films**: 50 more. Colour film (Kodacolor 200, Portra 160VC and 400VC,
  Ektachrome 64, Fujicolor 100 Industrial, Superia Venus 800, Sensia 100, Precisa 100,
  Agfa Portrait 160, Konica VX 200, E100VS, Phoenix 200, Lomochrome Purple and
  Metropolis, Aerochrome), cinema (EXR 200T, Eterna 400T, F-64D, XTR 250, Super 16,
  70 mm, Neon, Horror, Romance, Documentary, Indian Summer, Urban Thriller), black and
  white (Delta 400, Kentmere 400, SFX 200, Fomapan 100, RPX 25, Pancro 400, CHS 100 II,
  T-Max P3200, Pan 400, Verichrome Pan), instant (Polaroid i-Type, Spectra and 669,
  Instax Wide, Impossible Project, Instamatic) and creative (Pastel, Autumn, Winter,
  Spring, Mist, Backlight, Duotone). All checked on the chart like the others:
  distinct, no crushed image, neutral black and white, same renders on the GPU.
- **Search films** (Films tool): by name or description, ignoring accents and case;
  only the matching films remain, in their opened families.
- **275 films**: 125 more. 35 colour film stocks (Gold 100, Pro Foto 100, Portra 100T,
  Ektar 25, Kodachrome 200, Fortia 50, Superia 1600, Vista 400, Ultra 50, Agfacolor
  CT18, Impresa 50, Redscale, Orwocolor NC 19…), 30 cinema stocks and looks (Vision
  320T, Vision2 250D and 100T, Eastman 5248 and 5287, Eterna 500, Kodachrome Super 8,
  Technicolor IB, Neo-Tokyo, Golden Age, Science Fiction, Giallo, French New Wave, VHS,
  Camcorder…), 25 black and white (Tri-X 320, T-Max 100, HP5 at 3200, Ortho 25,
  Silvermax, CMS 20, APX 400, ORWO UN54 and N74, Svema 125, P30, HIE, Neopan 400,
  Technical Pan…), 15 instant (Polaroid Duochrome, Blue, Type 100, 55, Time-Zero, Instax
  Mini, Square, Monochrome and Sepia, Photo Booth…) and 20 creative (Amber, Mint,
  Lavender, Ocean, Forest, Blue Night, Dawn, Dusk, Warm and Cool Matte, Chiaroscuro,
  Pop, Selenium, Copper…). Each black-and-white film has its own colour sensitivity.
  All checked on the chart: distinct from one another, no crushed image, neutral black
  and white, same renders on the GPU.
- **iPhone and iPad: a one-month trial, then LÜMA Premium for life at €89.99**: a single
  App Store purchase, no subscription, restorable on your other devices and shareable
  with your family. Nothing changes on Mac (14-day trial, then the subscription).

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
