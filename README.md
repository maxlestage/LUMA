# LÜMA

**La lumière, développée en Rust.** Un éditeur photo professionnel écrit en Rust : RAW,
couleur 32 bits flottants, masques, IA locale, photothèque. **macOS et iOS/iPadOS d'abord**,
puis Windows, Linux et Android sur le même moteur.

- [Architecture](docs/architecture.md) : couches, workspace Cargo, pipeline non destructif, règles « 100 % Rust »
- [Feuille de route](docs/roadmap.md) : phases M0 → M36, Apple d'abord, jalons et risques
- [Parité avec Lightroom Classic](docs/parite-lightroom.md) : chaque fonction, faite ou à faire
- [Site vitrine](site/) : animations, scènes 3D (objectif photo, Mac et iPhone), labo de retouche sur GPU

## Le moteur

Le moteur de développement est en Rust, sans aucune bibliothèque native (vérifié par
`scripts/audit-rust.sh`). Il reprend le module Développement de Lightroom Classic : mêmes
panneaux, mêmes curseurs, mêmes plages, et il lit les fichiers XMP de Lightroom.

| Crate | Rôle |
|---|---|
| `luma-core` | Images `f32`, flous rapides, filtre guidé, annulation, mesures |
| `luma-color` | Matrices, espaces (sRGB, Display P3, Rec. 2020, ProPhoto), température DNG, OKLab, profils ICC |
| `luma-raw` | Lecture des RAW (rawler), normalisation, couleur DNG, dématriçage Bayer et X‑Trans |
| `luma-project` | Les réglages (calqués sur Lightroom), JSON et XMP `crs:` |
| `luma-image` | Le développement : tonalité locale, rendu filmique, courbes, couleur, détail, effets, géométrie |
| `luma-render` | Une photo ouverte : sources en cache par taille, rendu annulable |
| `luma-cli` | `luma` : info, développement et export, XMP, mesures |

```sh
cargo build --release
target/release/luma info photo.ARW
target/release/luma develop photo.ARW -o photo.jpg --space p3 --set exposure=+0.5 --set shadows=40
target/release/luma develop photo.CR3 --size 2048 -o apercu.jpg    # lit photo.xmp s'il existe
target/release/luma xmp photo.NEF --set clarity=20 -o photo.xmp      # réglages au format Lightroom
target/release/luma bench photo.RAF                                   # réactivité mesurée
```

### Vérifier

```sh
scripts/fetch-corpus.sh          # 5 vrais RAW : Sony, Canon, Nikon, Fujifilm X-Trans, iPhone ProRAW
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace           # tests unitaires + développement de tout le corpus
scripts/audit-rust.sh
```

### Réactivité mesurée

Aperçu de 2048 px, développement + encodage Display P3, 15 rendus successifs comme un
curseur qu'on fait glisser. Machine de développement : 4 cœurs, sans GPU (le rendu Metal
viendra ensuite). Ce ne sont pas encore des mesures sur Mac ni sur iPhone.

| Boîtier | Premier aperçu | Réglages de base, P50 / P95 | Réglages lourds, P50 / P95 |
|---|---|---|---|
| Sony A7 III (ARW) | 166 ms | 110 / 126 ms | 247 / 270 ms |
| Canon EOS R5 (CR3) | 195 ms | 106 / 118 ms | 268 / 300 ms |
| Nikon Z 6 (NEF) | 157 ms | 102 / 113 ms | 230 / 242 ms |
| Fujifilm X‑T3 (RAF, X‑Trans) | 198 ms | 101 / 109 ms | 246 / 260 ms |
| iPhone 12 Pro (ProRAW) | 319 ms | 168 / 184 ms | 365 / 380 ms |

« Lourds » : Hautes lumières, Ombres, Clarté, Texture, Voile, Vibrance, mélangeur,
étalonnage, réduction du bruit, vignetage, tous actifs. L'objectif est un aperçu sous
100 ms (P95) sur Mac Apple Silicon ; la CI publie ces mesures à chaque push.

## Le site

Le site est une app [Paquets](https://github.com/maxlestage/Paquets) : du Rust de bout en bout.

| Partie | Où | En quoi |
|---|---|---|
| Serveur, pages, SEO, CSP, PWA, export statique | `site/src/server/` | Rust (Paquets, natif) |
| Photo du pipeline, générée en SVG | `site/src/server/photo.rs` | Rust |
| Îlots : labo (réglages, préréglages) et filtre des crates | `site/src/islands.rs` | Rust, rendu serveur puis hydraté en WebAssembly |
| Animations de page : préchargement, révélations, curseur, inclinaison, compteurs, manifeste, pipeline 3D, feuille de route, terminal | `site/src/client/motion.rs` | Rust → WebAssembly |
| Rendu du labo (WebGL) | `site/src/client/lab.rs`, `lab.frag` | Rust → WebAssembly, shader GLSL |
| Textes et données | `site/src/content.rs` | Rust, partagé serveur/navigateur |
| Scènes 3D : objectif photo, MacBook et iPhone | `site/public/js/` | three.js r170, embarqué (aucun CDN) |
| Styles | `site/public/site.css` | CSS |

La page est du HTML fini rendu par le serveur : elle se lit sans JavaScript, avec ses
balises SEO, et sa Content-Security-Policy n'autorise que son propre code.

### Construire et lancer

Tout se lance depuis `site/` :

```sh
# 1. Le bundle du navigateur (îlots + animations) → pkg/
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129
cargo build --lib --target wasm32-unknown-unknown --profile wasm-release
wasm-bindgen --target web --no-typescript --out-dir pkg \
  target/wasm32-unknown-unknown/wasm-release/luma_site.wasm

# 2. Le serveur, sur http://localhost:3000 (PORT sur Heroku)
cargo run --release

# Ou le site en fichiers statiques (GitHub Pages…), liens sous BASE_PATH
BASE_PATH=/LUMA cargo run --release -- export dist
```

Paquets et ses paquets sont des dépôts git privés : il faut un accès GitHub qui peut les lire.

### Vérifier

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --lib --target wasm32-unknown-unknown -- -D warnings
cargo test
```

### Déployer

- **GitHub Pages** : `.github/workflows/pages.yml` vérifie, construit et publie le site à chaque
  push sur `master`, sur `https://maxlestage.github.io/LUMA/`. Il faut le secret `REPOS_TOKEN`
  et, dans Settings → Pages, la source « GitHub Actions ».
- **Heroku, buildpack** : `app.json`, `RustConfig`, `heroku-build.sh` et `Procfile`. Ajouter la
  variable `REPOS_TOKEN` (un jeton GitHub qui lit les dépôts privés).
- **Docker** : `docker build --build-arg REPOS_TOKEN=… -t luma .`, ou `heroku.yml` sur la stack container.

### Apple d'abord

Pensé d'abord pour Safari sur Mac, iPhone et iPad : installable sur l'écran d'accueil (PWA,
icônes, `apple-touch-icon`), `100svh`, zones sûres (`env(safe-area-inset-*)`), densité de
pixels plafonnée à 2, moins de particules sur téléphone, rendu à la demande pour la scène
des appareils, et respect de « Réduire les animations ».
