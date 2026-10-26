# LÜMA — Architecture

LÜMA est un éditeur photo professionnel écrit en Rust. Ce document fixe l'architecture
technique ; la planification se trouve dans [`roadmap.md`](roadmap.md).

## Décisions

| Question | Décision |
|---|---|
| Plateformes | **macOS et iOS/iPadOS d'abord**, puis Windows, Linux et Android |
| Ambition | Un éditeur photo professionnel complet : RAW, retouche, masques, IA locale, photothèque, puis vidéo |
| « 100 % Rust » | Tout le code de LÜMA et de ses dépendances est en Rust. Aucune bibliothèque native tierce (C, C++, Objective‑C, Swift, Kotlin). Les **frameworks du système** (Metal, AppKit, UIKit, PhotoKit…) sont appelés depuis Rust, via des crates de liaison Rust (`objc2`), et ne sont pas des dépendances tierces |

## Pourquoi Apple d'abord

- **Un seul GPU à maîtriser au départ** : Metal sur Apple Silicon (Mac M‑series, iPhone, iPad).
  `wgpu` cible Metal nativement ; le backend Vulkan/DX12 viendra avec Windows, Linux et Android.
- **Mémoire unifiée** : CPU, GPU et Neural Engine partagent la même mémoire. Le pipeline peut
  éviter les copies entre le décodage RAW et le rendu GPU.
- **Écrans calibrés** : Display P3 partout, HDR (EDR) sur les écrans XDR et les iPhone récents.
  C'est le meilleur terrain pour valider la colorimétrie 32 bits flottants.
- **Public photographe** : forte proportion de photographes sur Mac, ProRAW sur iPhone,
  Apple Pencil sur iPad pour les masques.
- **Le même moteur** : le cœur Rust est identique sur Mac et iPhone. Les autres plateformes
  restent compilées en CI dès le premier mois pour garantir que rien ne dépend d'Apple dans le moteur.

## Couches

```
LÜMA Studio (UI)        interface, espaces de travail, outils — macOS, iOS, iPadOS
        │
LÜMA Core               projets, commandes, historique, tâches, orchestration
        │
 ┌──────┼──────────┬───────────┬──────────┐
Photo  Video     Compute     Library
RAW,   timeline, CPU, GPU,   catalogue,
color, étalon.   IA          métadonnées,
masks                        cache
        │
luma-platform           fichiers, PhotoKit, caméra, affichage EDR, GPU, permissions
        │
Système                 Metal · AppKit · UIKit · PhotoKit · (plus tard Vulkan, DX12, Android NDK)
```

Règle : le moteur (`luma-raw`, `luma-color`, `luma-image`, `luma-mask`, `luma-render`) ne
dépend **jamais** de `luma-platform`. Seuls `luma-ui` et `apps/*` voient la plateforme.

## Workspace Cargo

État actuel : `luma-core`, `luma-color`, `luma-raw`, `luma-project`, `luma-image`,
`luma-render` et `luma-cli` existent et sont testés (voir le README). Les autres crates
ci‑dessous sont prévues.

```
luma/
├── Cargo.toml            workspace, versions épinglées
├── rust-toolchain.toml
├── deny.toml             licences, sources, interdiction des -sys
├── crates/
│   ├── luma-core/        P0  commandes, undo/redo, historique, tâches, erreurs
│   ├── luma-project/     P0  format de projet versionné et migrable
│   ├── luma-raw/         P0  DNG, Bayer, X‑Trans, ProRAW, dématriçage, EXIF
│   ├── luma-color/       P0  ICC, XYZ, sRGB, Display P3, Rec.2020, EDR/HDR, f32
│   ├── luma-image/       P0  exposition, courbes, HSL, netteté, géométrie…
│   ├── luma-render/      P0  graphe, tuiles, cache multi‑résolution, annulation
│   ├── luma-gpu/         P0  wgpu (Metal d'abord), shaders WGSL, fallback CPU
│   ├── luma-mask/        P1  luminance, couleur, pinceau, dégradés, booléens
│   ├── luma-library/     P1  catalogue, collections, notes, doublons, recherche
│   ├── luma-export/      P1  JPEG, PNG, TIFF, DNG, lots, préréglages
│   ├── luma-presets/     P1  styles et profils de rendu
│   ├── luma-ai/          P2  sujets, ciel, débruitage, super‑résolution (local)
│   ├── luma-privacy/     P2  consentement IA, diagnostics sans contenu
│   ├── luma-sync/        P3  synchro chiffrée facultative
│   ├── luma-plugins/     P3  extensions isolées, permissions
│   ├── luma-video/       P3  timeline, étalonnage, rendu
│   ├── luma-audio/       P3
│   ├── luma-ui/          P0  interface adaptative (pointeur, tactile, Pencil)
│   └── luma-platform/    P0  macOS + iOS d'abord ; Windows/Linux/Android ensuite
├── apps/
│   ├── macos/            bundle .app, téléchargement direct (pas de Mac App Store)
│   ├── ios/              iPhone + iPad, App Store / TestFlight
│   ├── desktop/          Windows, Linux (phase 6)
│   └── android/          (phase 6)
└── tests/
    ├── raw-corpus/       fichiers RAW réels par boîtier
    ├── color-golden/     rendus de référence, tolérances ΔE documentées
    ├── performance/      bancs reproductibles (Mac M‑series, iPhone)
    ├── cross-platform/
    └── recovery/
```

## Pipeline non destructif

```
RAW / JPEG / HEIF original (jamais modifié)
  → décodage + profil caméra (luma-raw, luma-color)
  → graphe de retouche (paramètres seulement) : exposition → couleur → masques → détails → effets
  → aperçu  : basse résolution, tuiles visibles, GPU, annulable
  → export  : pleine résolution, f32, déterministe à tolérance documentée
```

Chaque retouche est un paramètre enregistré dans le projet. Le même projet produit le
même rendu sur Mac et sur iPhone, à une tolérance ΔE documentée entre GPU.

## Règles de dépendances

- Dépendances épinglées et auditées, transitives comprises (`cargo deny`, `cargo tree`).
- Aucune crate `-sys`, aucun `build.rs` qui compile du C/C++, aucun FFI vers une
  bibliothèque native tierce. Seuls les frameworks système sont appelés, via des crates Rust.
- `unsafe` limité, documenté, testé (Miri quand c'est possible).
- Licences et brevets (codecs vidéo en particulier) vérifiés avant intégration.

Exemple de `deny.toml` :

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

## Pistes à évaluer (prototype M0–M3)

| Besoin | Piste Rust | À vérifier |
|---|---|---|
| RAW | `rawler` | Couverture des boîtiers, ProRAW, X‑Trans, pureté Rust de l'arbre |
| Formats classiques | `image`, `jxl-oxide`, `psd`, `hayro-jpeg2000`, `resvg` ; ImageIO d'Apple (`luma-apple`) | HEIF/HEIC et AVIF : décodeur du système (pas de codec HEVC en Rust pur) |
| GPU | `wgpu` (Metal) | Latence, EDR/HDR, comportement sur iPhone |
| Fenêtres | `winit` + `objc2` | Intégration AppKit/UIKit, Pencil, gestes |
| UI | `egui`, ou UI maison sur `wgpu` | Fluidité 120 Hz ProMotion, accessibilité |
| IA | `burn`, `candle` | Backend Metal, taille des modèles, pureté |
| Vidéo | `rav1e` (AV1) | H.264/HEVC : décodage via VideoToolbox (système) ou exclusion |

## Objectifs de performance (à mesurer, pas encore mesurés)

Référence : MacBook Air/Pro Apple Silicon 16 Go ; iPhone récent.

| Mesure | Mac | iPhone |
|---|---|---|
| Commande simple (P95) | < 50 ms | < 80 ms |
| Aperçu après réglage (P95) | < 100 ms | < 150 ms |
| Interface | 120 i/s (ProMotion), 60 minimum | 120 i/s (ProMotion), 60 minimum |
| Vignette (P95) | < 150 ms | < 250 ms |
| Démarrage à froid | < 3 s | < 3 s |
| Mémoire au repos | < 500 Mo | < 300 Mo |
