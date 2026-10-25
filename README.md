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
| `luma-render` | Une photo ouverte : sources et « base » (étapes de voisinage) en cache, rendu annulable |
| `luma-gpu` | Les étapes par pixel (rendu, couleur, effets, encodage) en WGSL sur wgpu : Metal, Vulkan ; comparées au processeur à chaque test |
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

**Sur le GPU.** Pendant qu'on règle le contraste, les courbes, les couleurs, l'étalonnage
ou les effets, la base (bruit, tonalité locale) reste en cache et seule la finition est
refaite, sur le GPU dans l'app : le résultat est identique à celui du processeur (au plus
un niveau d'écart sur 255, vérifié à chaque push sur Metal et sur Vulkan). La CI macOS
publie le temps de finition mesuré sur Metal ; l'app affiche « GPU » à côté du temps de
rendu. `LUMA_GPU=0` force le processeur.

« Lourds » : Hautes lumières, Ombres, Clarté, Texture, Voile, Vibrance, mélangeur,
étalonnage, réduction du bruit, vignetage, tous actifs. L'objectif est un aperçu sous
100 ms (P95) sur Mac Apple Silicon ; la CI publie ces mesures à chaque push.

## LÜMA Studio, l'app Mac

![LÜMA Studio, module Développement](docs/studio.jpg)

`apps/studio` : l'application de développement, en Rust (egui, rendu wgpu sur Metal).
Deux modules, comme dans Lightroom Classic. **Bibliothèque** : grille du dossier, notes
de 0 à 5 étoiles, drapeaux retenue / rejetée, filtre appliqué aussi au film, métadonnées
EXIF. **Développement** : navigateur et historique à gauche,
histogramme et panneaux Base, Courbe des tonalités, Mélangeur de couleurs, Étalonnage
des couleurs, Détail, Effets, Étalonnage de l'appareil photo et Recadrage à droite, film
du dossier en bas. Les réglages sont enregistrés automatiquement dans un XMP au format
Lightroom à côté du RAW, qui n'est jamais modifié.

| Raccourci | Action |
|---|---|
| `\` | Avant / après |
| Cmd+Z, Maj+Cmd+Z | Annuler, rétablir (chaque geste est une étape d'historique) |
| Maj+Cmd+C, Maj+Cmd+V | Copier, coller les réglages |
| Maj+Cmd+R | Réinitialiser |
| Maj+Cmd+E | Exporter : la photo ou toutes celles du filtre ; JPEG, PNG ou TIFF 8/16 bits ; sRGB, Display P3, Rec. 2020 ou ProPhoto ; taille ; dossier |
| G, D | Bibliothèque, Développement |
| 0 à 5 ; P, X, U | Note ; retenir, rejeter, retirer le drapeau (enregistrés dans le XMP, `xmp:Rating` comme Lightroom) |
| R | Outil Recadrage : poignées, grille des tiers, proportions verrouillées ; Entrée valide, Échap annule |
| M, Maj+M, K | Masque : dégradé linéaire, dégradé radial, pinceau (Option gomme) ; O montre la zone ; chaque masque a ses curseurs |
| Cmd+O | Ouvrir un RAW (son dossier remplit le film) ; on peut aussi le déposer |
| ← → | Photo précédente, suivante |
| Z, J | Zoom 1:1, écrêtage |
| Double‑clic sur un curseur | Valeur par défaut ; Maj en glissant : réglage fin |

```sh
cargo run --release -p luma-studio -- ~/Photos/2026-10      # un dossier ou un RAW
scripts/bundle-macos.sh                                     # sur Mac : target/macos/LÜMA.app
```

La CI (`.github/workflows/macos.yml`) compile et teste l'app sur Apple Silicon et publie
`LUMA-macos.zip` (universelle, Apple Silicon et Intel) en artefact à chaque push. L'app
est signée « ad hoc » : au premier lancement, clic droit → Ouvrir. Un test rend toute
l'interface sans écran avec un vrai RAW et vérifie que chaque contrôle a un nom pour
VoiceOver.

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
