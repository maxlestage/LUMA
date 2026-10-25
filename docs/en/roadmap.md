# LÜMA — Roadmap (Apple first)

macOS and iOS/iPadOS are the priority platforms. Windows, Linux and Android are
built in CI from the first month (the engine must assume nothing about Apple), but their
apps only ship in phase 6.

Estimate: 24 to 36 months to a professional photo 1.0 with an experienced team.
This is an estimate, not a guarantee.

| Phase | Months | Apple (priority) | Other platforms |
|---|---|---|---|
| 1. Feasibility | M0–M3 | Minimal macOS and iOS apps in Rust (`winit` + `objc2`), `wgpu`/Metal rendering, `rawler` decoding on a corpus (Sony, Canon, Nikon, Fujifilm, ProRAW), automated signing and TestFlight | CI builds of the engine for Windows, Linux, Android |
| 2. Engine | M4–M6 | f32 pipeline, Display P3, EDR/HDR on XDR displays, zero-copy unified memory, edit graph, versioned project | Engine tests on the CPU (fallback) |
| 3. Alpha | M7–M12 | Mac: photo library, editing, export. iPhone/iPad: import from PhotoKit and Files, Apple Pencil masking, gestures. Public TestFlight | — |
| 4. Beta | M13–M18 | On-device AI (subject, sky, denoise) on Apple GPUs, camera/lens profiles, search, crash recovery | First Vulkan/DX12 port of the renderer |
| 5. 1.0 | M19–M24 | Mac App Store and App Store, accessibility, migration, documentation | — |
| 6. Expansion | M25–M36 | Video (timeline, colour grading), tethering, printing | Windows, Linux and Android apps on the same engine |

## Progress

**M0 – M3, feasibility: the development engine exists.** Done:

- Cargo workspace (`crates/`), "100% Rust" rule verified in CI against the actual
  dependency graph;
- reading Bayer, X‑Trans and linear DNG (ProRAW) RAW files, DNG-style colour,
  Malvar‑He‑Cutler and X‑Trans demosaicing;
- Lightroom's Develop module reproduced panel by panel (see
  [`parite-lightroom.md`](parite-lightroom.md)), reading and writing `crs:` XMP;
- JPEG, PNG, 8/16-bit TIFF export in sRGB or Display P3 with an ICC profile;
- 5 camera bodies developed on every push (Sony, Canon, Nikon, Fujifilm, iPhone), responsiveness
  measurements published;
- LÜMA Studio, the Mac app (`apps/studio`): complete Develop module, history,
  automatic XMP, filmstrip, Lightroom shortcuts, Library (ratings, flags,
  filter), Crop tool, Export dialog; finishing on the GPU (Metal), cached
  base image; linear, radial and brush masks;
  lens corrections and transform; universal `LÜMA.app` built by
  CI on an Apple Silicon runner.

Next steps, in order:

1. GPU rendering, continued: per-pixel finishing is done (`luma-gpu`, identical to the
   CPU); the neighbourhood stages remain (noise, local tone, sharpening),
   as does display without readback (texture shared with the interface);
2. in the app: catalogue (multiple folders, collections, keywords), presets;
3. AI masks (subject, sky), lens profiles, Auto Upright;
4. iOS: the app already runs (simulator, CI); PhotoKit, gestures (pinch to
   zoom), Apple Pencil for masks, signing and TestFlight remain.

## Validation milestones

- **M3**: a RAW file opens, can be adjusted and exported on Mac and on iPhone, with no forbidden dependency.
- **M6**: identical rendering on Mac/iPhone within a documented ΔE tolerance, P95 preview < 100 ms on Mac.
- **M12**: a photographer completes a full workflow (import → culling → editing → export) on Mac or iPad.
- **M18**: the advanced tools pass the quality, compatibility and performance tests.
- **M24**: stable version released on both App Stores, public compatibility matrix.

## Main risks

| Risk | Level | Response | Where we stand |
|---|---|---|---|
| RAW coverage | Very high | Real corpus, per-camera matrix published and extended with every release | **Done.** [`boitiers.md`](boitiers.md): 735 camera bodies from 20 brands, 714 with calibrated colour, generated from the decoder's database (`luma cameras --markdown`) and checked by CI on every push. 5 real RAW files developed on every test run (Sony, Canon, Nikon, Fujifilm, iPhone ProRAW); any DNG can be read. To extend: one more real RAW per release (Canon CR2, Olympus, Panasonic, Pentax). |
| Video without third-party libraries | Very high | Decoding via Apple's system frameworks, or a reduced scope; audit from M0 | **Decided: reduced scope.** v1 is a photo editor, without video. The audit: decoding H.264/HEVC/ProRes in pure Rust is not realistic; AVFoundation (a system framework, allowed) requires Objective‑C bindings, and therefore `unsafe` code, which is currently forbidden in LÜMA. If video comes back, it will live in an isolated `luma-platform` layer, the only one allowed to use `unsafe`, audited separately. |
| iOS integration in Rust (UIKit, Pencil, PhotoKit) | High | Prototype from M0, thin and tested `luma-platform` layer | **Prototype done**: the app runs on iPhone and iPad (simulator launched on every push, screenshot verified), Metal, RAW via the Files app. **Remaining**: PhotoKit (photo library), Apple Pencil (pressure for the brush), sharing. All of these require Objective‑C bindings: same rule as for video, an isolated `luma-platform` layer. |
| Colour less accurate than the competition | High | Colorimetric tests, reference renders | **Tests in place** (`crates/luma-render/tests/color.rs`): a ColorChecker chart seen by a real sensor (Sony A7 III), ΔE 2000 < 1 in daylight and under a 3200 K lamp (matrix interpolation), hues preserved through the full development; 100 films measured on a test chart (`luma-image/tests/presets.rs`), GPU identical to the CPU. **Measurement on a real camera**: `luma colorchecker photo.RAW` (a photo of a real chart, cropped) gives the ΔE 2000 of each patch. **Remaining**: photos of real charts in the corpus, and profiles that go beyond matrices (hue tables, like Adobe's DCPs). |
| Memory on iPhone | High | Tiles, bounded cache, progressive processing | **Caches bounded in bytes**: 600 MB on iPhone and iPad, 3 GB elsewhere (`luma_render::memory_budget`), oldest entries evicted first, tested. Previews at screen size. **Remaining**: tiled full-resolution export (today the whole image is held in memory: about 600 MB for 48 MP). |
| App Store review | Medium | Compliance with the guidelines from the alpha onwards, continuous TestFlight | **Ready on the app side**: privacy manifest (`PrivacyInfo.xcprivacy`: no tracking, no data collected), no encryption to declare, complete Info.plist. **Remaining**: an Apple Developer account (signing, TestFlight), and the subscription on iPhone and iPad through in-app purchases (an App Store rule, see [`premium.md`](premium.md)). |
