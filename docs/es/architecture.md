# LÜMA — Arquitectura

LÜMA es un editor de fotos profesional escrito en Rust. Este documento fija la arquitectura
técnica; la planificación se encuentra en [`roadmap.md`](roadmap.md).

## Decisiones

| Cuestión | Decisión |
|---|---|
| Plataformas | **macOS e iOS/iPadOS primero**, después Windows, Linux y Android |
| Ambición | Un editor fotográfico profesional completo: RAW, revelado, máscaras, IA local, fototeca y, más adelante, vídeo |
| «100 % Rust» | Todo el código de LÜMA y de sus dependencias está en Rust. Ninguna biblioteca nativa de terceros (C, C++, Objective‑C, Swift, Kotlin). Los **frameworks del sistema** (Metal, AppKit, UIKit, PhotoKit…) se llaman desde Rust mediante crates de enlace en Rust (`objc2`) y no son dependencias de terceros |

## Por qué Apple primero

- **Una sola GPU que dominar al principio**: Metal en Apple Silicon (Mac de la serie M, iPhone, iPad).
  `wgpu` se dirige a Metal de forma nativa; el backend Vulkan/DX12 llegará con Windows, Linux y Android.
- **Memoria unificada**: la CPU, la GPU y el Neural Engine comparten la misma memoria. El pipeline puede
  evitar copias entre la decodificación RAW y el renderizado en GPU.
- **Pantallas calibradas**: Display P3 en todas partes, HDR (EDR) en las pantallas XDR y los iPhone recientes.
  Es el mejor terreno para validar la colorimetría en coma flotante de 32 bits.
- **Público fotógrafo**: gran proporción de fotógrafos en Mac, ProRAW en iPhone,
  Apple Pencil en iPad para las máscaras.
- **El mismo motor**: el núcleo en Rust es idéntico en Mac y en iPhone. Las demás plataformas
  se siguen compilando en CI desde el primer mes para garantizar que nada del motor dependa de Apple.

## Capas

```
LÜMA Studio (UI)        interfaz, espacios de trabajo, herramientas — macOS, iOS, iPadOS
        │
LÜMA Core               proyectos, comandos, historial, tareas, orquestación
        │
 ┌──────┼──────────┬───────────┬──────────┐
Photo  Video     Compute     Library
RAW,   timeline, CPU, GPU,   catálogo,
color, etalon.   IA          metadatos,
masks                        caché
        │
luma-platform           archivos, PhotoKit, cámara, pantalla EDR, GPU, permisos
        │
Sistema                 Metal · AppKit · UIKit · PhotoKit · (más adelante Vulkan, DX12, Android NDK)
```

Regla: el motor (`luma-raw`, `luma-color`, `luma-image`, `luma-mask`, `luma-render`) **nunca**
depende de `luma-platform`. Solo `luma-ui` y `apps/*` ven la plataforma.

## Workspace de Cargo

Estado actual: `luma-core`, `luma-color`, `luma-raw`, `luma-project`, `luma-image`,
`luma-render` y `luma-cli` existen y están probados (véase el README). Los demás crates
de abajo están previstos.

```
luma/
├── Cargo.toml            workspace, versiones fijadas
├── rust-toolchain.toml
├── deny.toml             licencias, orígenes, prohibición de los -sys
├── crates/
│   ├── luma-core/        P0  comandos, deshacer/rehacer, historial, tareas, errores
│   ├── luma-project/     P0  formato de proyecto versionado y migrable
│   ├── luma-raw/         P0  DNG, Bayer, X‑Trans, ProRAW, interpolación cromática, EXIF
│   ├── luma-color/       P0  ICC, XYZ, sRGB, Display P3, Rec.2020, EDR/HDR, f32
│   ├── luma-image/       P0  exposición, curvas, HSL, enfoque, geometría…
│   ├── luma-render/      P0  grafo, teselas, caché multirresolución, cancelación
│   ├── luma-gpu/         P0  wgpu (Metal primero), shaders WGSL, alternativa en CPU
│   ├── luma-mask/        P1  luminancia, color, pincel, degradados, booleanos
│   ├── luma-library/     P1  catálogo, colecciones, valoraciones, duplicados, búsqueda
│   ├── luma-export/      P1  JPEG, PNG, TIFF, DNG, lotes, ajustes preestablecidos
│   ├── luma-presets/     P1  estilos y perfiles de renderizado
│   ├── luma-ai/          P2  sujetos, cielo, reducción de ruido, superresolución (local)
│   ├── luma-privacy/     P2  consentimiento de IA, diagnósticos sin contenido
│   ├── luma-sync/        P3  sincronización cifrada opcional
│   ├── luma-plugins/     P3  extensiones aisladas, permisos
│   ├── luma-video/       P3  timeline, etalonaje, renderizado
│   ├── luma-audio/       P3
│   ├── luma-ui/          P0  interfaz adaptable (puntero, táctil, Pencil)
│   └── luma-platform/    P0  macOS + iOS primero; Windows/Linux/Android después
├── apps/
│   ├── macos/            paquete .app, descarga directa (sin Mac App Store)
│   ├── ios/              iPhone + iPad, App Store / TestFlight
│   ├── desktop/          Windows, Linux (fase 6)
│   └── android/          (fase 6)
└── tests/
    ├── raw-corpus/       archivos RAW reales por cámara
    ├── color-golden/     renderizados de referencia, tolerancias ΔE documentadas
    ├── performance/      bancos de pruebas reproducibles (Mac serie M, iPhone)
    ├── cross-platform/
    └── recovery/
```

## Pipeline no destructivo

```
Original RAW / JPEG / HEIF (nunca se modifica)
  → decodificación + perfil de cámara (luma-raw, luma-color)
  → grafo de revelado (solo parámetros): exposición → color → máscaras → detalle → efectos
  → vista previa: baja resolución, teselas visibles, GPU, cancelable
  → exportación: resolución completa, f32, determinista con tolerancia documentada
```

Cada ajuste es un parámetro guardado en el proyecto. El mismo proyecto produce el
mismo renderizado en Mac y en iPhone, con una tolerancia ΔE documentada entre GPU.

## Reglas de dependencias

- Dependencias fijadas y auditadas, transitivas incluidas (`cargo deny`, `cargo tree`).
- Ningún crate `-sys`, ningún `build.rs` que compile C/C++, ninguna FFI hacia una
  biblioteca nativa de terceros. Solo se llaman los frameworks del sistema, mediante crates de Rust.
- `unsafe` limitado, documentado y probado (con Miri cuando es posible).
- Licencias y patentes (en particular de los códecs de vídeo) verificadas antes de la integración.

Ejemplo de `deny.toml`:

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

## Opciones que evaluar (prototipo M0–M3)

| Necesidad | Opción en Rust | Por verificar |
|---|---|---|
| RAW | `rawler` | Cobertura de cámaras, ProRAW, X‑Trans, pureza Rust del árbol de dependencias |
| Formatos habituales | `image`, `jxl-oxide`, `psd`, `hayro-jpeg2000`, `resvg`; ImageIO de Apple (`luma-apple`) | HEIF/HEIC y AVIF: decodificador del sistema (no hay códec HEVC en Rust puro) |
| GPU | `wgpu` (Metal) | Latencia, EDR/HDR, comportamiento en iPhone |
| Ventanas | `winit` + `objc2` | Integración con AppKit/UIKit, Pencil, gestos |
| UI | `egui`, o UI propia sobre `wgpu` | Fluidez a 120 Hz ProMotion, accesibilidad |
| IA | `burn`, `candle` | Backend Metal, tamaño de los modelos, pureza |
| Vídeo | `rav1e` (AV1) | H.264/HEVC: decodificación mediante VideoToolbox (sistema) o exclusión |

## Objetivos de rendimiento (por medir, aún sin medir)

Referencia: MacBook Air/Pro con Apple Silicon y 16 GB; iPhone reciente.

| Medida | Mac | iPhone |
|---|---|---|
| Comando simple (P95) | < 50 ms | < 80 ms |
| Vista previa tras un ajuste (P95) | < 100 ms | < 150 ms |
| Interfaz | 120 fps (ProMotion), 60 como mínimo | 120 fps (ProMotion), 60 como mínimo |
| Miniatura (P95) | < 150 ms | < 250 ms |
| Arranque en frío | < 3 s | < 3 s |
| Memoria en reposo | < 500 MB | < 300 MB |
