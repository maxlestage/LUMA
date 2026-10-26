# LÜMA — Architecture

LÜMA is a professional photo editor written in Rust. This document sets out the technical
architecture; planning lives in [`roadmap.md`](roadmap.md).

## Decisions

| Question | Decision |
|---|---|
| Platforms | **macOS and iOS/iPadOS first**, then Windows, Linux and Android |
| Ambition | A complete professional photo editor: RAW, editing, masks, on-device AI, photo library, then video |
| "100% Rust" | All of LÜMA's code and its dependencies is Rust. No third-party native libraries (C, C++, Objective‑C, Swift, Kotlin). **System frameworks** (Metal, AppKit, UIKit, PhotoKit…) are called from Rust through Rust binding crates (`objc2`) and are not third-party dependencies |

## Why Apple first

- **A single GPU to master at the start**: Metal on Apple Silicon (M‑series Macs, iPhone, iPad).
  `wgpu` targets Metal natively; the Vulkan/DX12 backend will come with Windows, Linux and Android.
- **Unified memory**: the CPU, GPU and Neural Engine share the same memory. The pipeline can
  avoid copies between RAW decoding and GPU rendering.
- **Calibrated displays**: Display P3 everywhere, HDR (EDR) on XDR displays and recent iPhones.
  It is the best ground on which to validate 32-bit floating-point colour science.
- **Photographer audience**: a high proportion of photographers on the Mac, ProRAW on iPhone,
  Apple Pencil on iPad for masks.
- **The same engine**: the Rust core is identical on Mac and iPhone. The other platforms
  are still built in CI from the first month to guarantee that nothing in the engine depends on Apple.

## Layers

```
LÜMA Studio (UI)        interface, workspaces, tools — macOS, iOS, iPadOS
        │
LÜMA Core               projects, commands, history, tasks, orchestration
        │
 ┌──────┼──────────┬───────────┬──────────┐
Photo  Video     Compute     Library
RAW,   timeline, CPU, GPU,   catalogue,
color, grading   AI          metadata,
masks                        cache
        │
luma-platform           files, PhotoKit, camera, EDR display, GPU, permissions
        │
System                  Metal · AppKit · UIKit · PhotoKit · (later Vulkan, DX12, Android NDK)
```

Rule: the engine (`luma-raw`, `luma-color`, `luma-image`, `luma-mask`, `luma-render`) **never**
depends on `luma-platform`. Only `luma-ui` and `apps/*` see the platform.

## Cargo workspace

Current state: `luma-core`, `luma-color`, `luma-raw`, `luma-project`, `luma-image`,
`luma-render` and `luma-cli` exist and are tested (see the README). The other crates
below are planned.

```
luma/
├── Cargo.toml            workspace, pinned versions
├── rust-toolchain.toml
├── deny.toml             licences, sources, ban on -sys crates
├── crates/
│   ├── luma-core/        P0  commands, undo/redo, history, tasks, errors
│   ├── luma-project/     P0  versioned, migratable project format
│   ├── luma-raw/         P0  DNG, Bayer, X‑Trans, ProRAW, demosaicing, EXIF
│   ├── luma-color/       P0  ICC, XYZ, sRGB, Display P3, Rec.2020, EDR/HDR, f32
│   ├── luma-image/       P0  exposure, curves, HSL, sharpening, geometry…
│   ├── luma-render/      P0  graph, tiles, multi‑resolution cache, cancellation
│   ├── luma-gpu/         P0  wgpu (Metal first), WGSL shaders, CPU fallback
│   ├── luma-mask/        P1  luminance, colour, brush, gradients, booleans
│   ├── luma-library/     P1  catalogue, collections, ratings, duplicates, search
│   ├── luma-export/      P1  JPEG, PNG, TIFF, DNG, batches, presets
│   ├── luma-presets/     P1  styles and rendering profiles
│   ├── luma-ai/          P2  subjects, sky, denoise, super‑resolution (on-device)
│   ├── luma-privacy/     P2  AI consent, content-free diagnostics
│   ├── luma-sync/        P3  optional encrypted sync
│   ├── luma-plugins/     P3  sandboxed extensions, permissions
│   ├── luma-video/       P3  timeline, colour grading, rendering
│   ├── luma-audio/       P3
│   ├── luma-ui/          P0  adaptive interface (pointer, touch, Pencil)
│   └── luma-platform/    P0  macOS + iOS first; Windows/Linux/Android later
├── apps/
│   ├── macos/            .app bundle, direct download (no Mac App Store)
│   ├── ios/              iPhone + iPad, App Store / TestFlight
│   ├── desktop/          Windows, Linux (phase 6)
│   └── android/          (phase 6)
└── tests/
    ├── raw-corpus/       real RAW files per camera body
    ├── color-golden/     reference renders, documented ΔE tolerances
    ├── performance/      reproducible benchmarks (M‑series Mac, iPhone)
    ├── cross-platform/
    └── recovery/
```

## Non-destructive pipeline

```
RAW / JPEG / HEIF original (never modified)
  → decoding + camera profile (luma-raw, luma-color)
  → edit graph (parameters only): exposure → colour → masks → detail → effects
  → preview : low resolution, visible tiles, GPU, cancellable
  → export  : full resolution, f32, deterministic within a documented tolerance
```

Every edit is a parameter saved in the project. The same project produces the
same render on Mac and on iPhone, within a documented ΔE tolerance across GPUs.

## Dependency rules

- Dependencies pinned and audited, transitive ones included (`cargo deny`, `cargo tree`).
- No `-sys` crates, no `build.rs` that compiles C/C++, no FFI to a
  third-party native library. Only system frameworks are called, through Rust crates.
- `unsafe` kept to a minimum, documented and tested (Miri where possible).
- Licences and patents (video codecs in particular) checked before integration.

Example `deny.toml`:

```toml
[bans]
multiple-versions = "warn"
deny = [
    { name = "openssl-sys" },
    { name = "libraw-sys" },
    { name = "ffmpeg-sys-next" },
]

[sources]
unknown-registry = "deny"
unknown-git = "deny"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "Unicode-3.0"]
```

## Options to evaluate (M0–M3 prototype)

| Need | Rust option | To check |
|---|---|---|
| RAW | `rawler` | Camera coverage, ProRAW, X‑Trans, Rust purity of the dependency tree |
| Common formats | `image`, `jxl-oxide`, `psd`, `hayro-jpeg2000`, `resvg`; Apple ImageIO (`luma-apple`) | HEIF/HEIC and AVIF: system decoder (no pure-Rust HEVC codec) |
| GPU | `wgpu` (Metal) | Latency, EDR/HDR, behaviour on iPhone |
| Windowing | `winit` + `objc2` | AppKit/UIKit integration, Pencil, gestures |
| UI | `egui`, or a custom UI on `wgpu` | 120 Hz ProMotion smoothness, accessibility |
| AI | `burn`, `candle` | Metal backend, model size, purity |
| Video | `rav1e` (AV1) | H.264/HEVC: decoding via VideoToolbox (system) or exclusion |

## Performance targets (to be measured, not yet measured)

Reference: Apple Silicon MacBook Air/Pro with 16 GB; recent iPhone.

| Metric | Mac | iPhone |
|---|---|---|
| Simple command (P95) | < 50 ms | < 80 ms |
| Preview after an adjustment (P95) | < 100 ms | < 150 ms |
| Interface | 120 fps (ProMotion), 60 minimum | 120 fps (ProMotion), 60 minimum |
| Thumbnail (P95) | < 150 ms | < 250 ms |
| Cold start | < 3 s | < 3 s |
| Idle memory | < 500 MB | < 300 MB |
